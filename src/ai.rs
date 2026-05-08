use crate::config::Config;
use serde::{Deserialize, Serialize};

pub struct AiResponse {
    pub commands: Vec<String>,
    pub explanation: String,
}

pub trait AiProvider {
    fn ask(&self, input: &str) -> Result<AiResponse, String>;
}

pub fn create_provider(config: &Config) -> Box<dyn AiProvider> {
    match config.provider.as_str() {
        "claude" => Box::new(ClaudeProvider::new(config.clone())),
        "gemini" => Box::new(GeminiProvider::new(config.clone())),
        _        => Box::new(OpenAiCompatProvider::new(config.clone())),
    }
}

// ─── System prompt ───────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r#"You are mygit — a smart, practical git assistant.
The user describes what they want in plain English. You return the exact git commands needed.

Return ONLY valid JSON — no markdown fences, no text outside the JSON:
{
  "commands": ["git command 1", "git command 2"],
  "explanation": "Short friendly explanation of what these commands do."
}

═══════════════════════════════════════════
BRANCH NAMING
═══════════════════════════════════════════
- Always use 'main' as the default branch (modern GitHub standard since 2020)
- When user says "push", always push to 'main' unless they specify another branch
- First push to a new repo: git push -u origin main

═══════════════════════════════════════════
REMOTE MANAGEMENT
═══════════════════════════════════════════
- NEVER use 'git remote add origin' — it fails if origin already exists
- ALWAYS use 'git remote set-url origin URL' to set or update the remote
- Remote URLs with credentials: https://USERNAME:TOKEN@github.com/USERNAME/REPO.git

═══════════════════════════════════════════
GITIGNORE — VERY IMPORTANT
═══════════════════════════════════════════
When the user wants to push/init a project, ALWAYS check if a .gitignore is needed.
If the project has any of these, include the right gitignore content:

Rust projects (has Cargo.toml):
  /target
  Cargo.lock        ← only ignore this for libraries, keep it for binaries

Python projects (has .py files or requirements.txt):
  __pycache__/
  *.py[cod]
  *.egg-info/
  dist/
  build/
  .venv/
  venv/
  env/
  .env
  *.egg

Node.js projects (has package.json):
  node_modules/
  dist/
  build/
  .env
  .env.local
  npm-debug.log*
  yarn-debug.log*
  .DS_Store

Java projects (has .java files or pom.xml):
  target/
  *.class
  *.jar
  *.war
  .gradle/
  build/

Go projects (has .go files or go.mod):
  vendor/
  *.exe
  *.test
  *.out

C/C++ projects:
  *.o
  *.a
  *.so
  *.out
  build/

General (always include these):
  .DS_Store
  .idea/
  .vscode/
  *.log
  *.tmp
  .env
  .env.*
  !.env.example

When creating a .gitignore, use a single command like:
  printf 'line1\nline2\n' > .gitignore

After creating .gitignore, always run:
  git rm -r --cached .
  git add .
  git commit -m "add .gitignore and remove tracked build artifacts"

This removes any already-tracked files that should be ignored.

═══════════════════════════════════════════
COMMAND RULES
═══════════════════════════════════════════
- Never combine commands with && — each command is a separate entry in the array
- Never use interactive commands (git rebase -i, git add -p) — they need a terminal
- For destructive operations (--force, --hard, reset) warn clearly in the explanation
- split 'git add && git commit' into two separate commands always

═══════════════════════════════════════════
COMMON OPERATIONS
═══════════════════════════════════════════
"save my work" / "commit":
  git add .
  git commit -m "descriptive message based on context"

"push" (existing remote):
  git push origin main

"push" (new repo, first time):
  git remote set-url origin https://USER:TOKEN@github.com/USER/REPO.git
  git push -u origin main

"pull" / "get latest":
  git pull origin main

"undo last commit but keep files":
  git reset --soft HEAD~1

"undo last commit and discard changes" (warn: destructive):
  git reset --hard HEAD~1

"create branch":
  git checkout -b branch-name

"switch branch":
  git checkout branch-name

"merge branch into main":
  git checkout main
  git merge branch-name

"stash work":
  git stash

"restore stash":
  git stash pop

"show history":
  git log --oneline -10

"show what changed":
  git status
  git diff

"delete branch":
  git branch -d branch-name

If the request is unclear or not git-related, return commands: [] and explain in the explanation."#;

// ─── JSON parser ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RawResponse {
    commands: Vec<String>,
    explanation: String,
}

fn parse_response(text: &str) -> Result<AiResponse, String> {
    let clean = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let raw: RawResponse = serde_json::from_str(clean).map_err(|e| {
        format!("AI returned unexpected output ({}). Raw: {}", e, &text[..text.len().min(300)])
    })?;

    Ok(AiResponse {
        commands: raw.commands,
        explanation: raw.explanation,
    })
}

// ─── OpenAI-compatible (DeepSeek · OpenAI · custom) ─────────────────────────

struct OpenAiCompatProvider {
    config: Config,
    client: reqwest::blocking::Client,
}

impl OpenAiCompatProvider {
    fn new(config: Config) -> Self {
        Self { config, client: reqwest::blocking::Client::new() }
    }
}

#[derive(Serialize)]
struct OaiRequest<'a> {
    model: &'a str,
    messages: Vec<OaiMessage>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct OaiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OaiResponse {
    choices: Vec<OaiChoice>,
}

#[derive(Deserialize)]
struct OaiChoice {
    message: OaiMessage,
}

impl AiProvider for OpenAiCompatProvider {
    fn ask(&self, input: &str) -> Result<AiResponse, String> {
        let url = format!("{}/chat/completions", self.config.api_base());
        let body = OaiRequest {
            model: self.config.effective_model(),
            messages: vec![
                OaiMessage { role: "system".into(), content: SYSTEM_PROMPT.into() },
                OaiMessage { role: "user".into(),   content: input.into() },
            ],
            temperature: 0.1,
        };

        let resp = self.client
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API error {}: {}", status, &body[..body.len().min(200)]));
        }

        let oai: OaiResponse = resp.json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let text = oai.choices.into_iter().next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        parse_response(&text)
    }
}

// ─── Anthropic Claude ────────────────────────────────────────────────────────

struct ClaudeProvider {
    config: Config,
    client: reqwest::blocking::Client,
}

impl ClaudeProvider {
    fn new(config: Config) -> Self {
        Self { config, client: reqwest::blocking::Client::new() }
    }
}

#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<ClaudeMessage<'a>>,
}

#[derive(Serialize)]
struct ClaudeMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeBlock>,
}

#[derive(Deserialize)]
struct ClaudeBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

impl AiProvider for ClaudeProvider {
    fn ask(&self, input: &str) -> Result<AiResponse, String> {
        let body = ClaudeRequest {
            model: self.config.effective_model(),
            max_tokens: 1024,
            system: SYSTEM_PROMPT,
            messages: vec![ClaudeMessage { role: "user", content: input }],
        };

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API error {}: {}", status, &body[..body.len().min(200)]));
        }

        let cr: ClaudeResponse = resp.json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let text = cr.content.into_iter()
            .find(|b| b.kind == "text")
            .and_then(|b| b.text)
            .unwrap_or_default();

        parse_response(&text)
    }
}

// ─── Google Gemini ───────────────────────────────────────────────────────────

struct GeminiProvider {
    config: Config,
    client: reqwest::blocking::Client,
}

impl GeminiProvider {
    fn new(config: Config) -> Self {
        Self { config, client: reqwest::blocking::Client::new() }
    }
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    system_instruction: GeminiSystem,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiSystem {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContentOwned,
}

#[derive(Deserialize)]
struct GeminiContentOwned {
    parts: Vec<GeminiPart>,
}

impl AiProvider for GeminiProvider {
    fn ask(&self, input: &str) -> Result<AiResponse, String> {
        let model = self.config.effective_model();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, self.config.api_key
        );

        let body = GeminiRequest {
            system_instruction: GeminiSystem {
                parts: vec![GeminiPart { text: SYSTEM_PROMPT.into() }],
            },
            contents: vec![GeminiContent {
                role: "user".into(),
                parts: vec![GeminiPart { text: input.into() }],
            }],
        };

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API error {}: {}", status, &body[..body.len().min(200)]));
        }

        let gr: GeminiResponse = resp.json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let text = gr.candidates.into_iter().next()
            .and_then(|c| c.content.parts.into_iter().next())
            .map(|p| p.text)
            .unwrap_or_default();

        parse_response(&text)
    }
}
