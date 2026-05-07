use crate::config::Config;
use serde::{Deserialize, Serialize};

/// What the AI returns after understanding your request.
pub struct AiResponse {
    /// The git commands to run, in order.
    pub commands: Vec<String>,
    /// A short human-friendly explanation of what they do.
    pub explanation: String,
}

/// All AI providers implement this trait.
pub trait AiProvider {
    fn ask(&self, input: &str) -> Result<AiResponse, String>;
}

/// Build the right provider based on the config.
pub fn create_provider(config: &Config) -> Box<dyn AiProvider> {
    match config.provider.as_str() {
        "claude" => Box::new(ClaudeProvider::new(config.clone())),
        "gemini" => Box::new(GeminiProvider::new(config.clone())),
        // deepseek, openai, and custom all use the OpenAI-compatible endpoint
        _ => Box::new(OpenAiCompatProvider::new(config.clone())),
    }
}

// ─── System prompt ──────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r#"You are mygit — a friendly, practical git assistant.
The user will describe what they want to do in plain English.

Your job:
1. Figure out the git commands they need
2. Return ONLY valid JSON, nothing else — no markdown fences, no explanation outside the JSON

Response format (strict):
{
  "commands": ["git command 1", "git command 2"],
  "explanation": "One or two sentences explaining what these commands do."
}

Rules:
- commands must only contain git commands (git add, git commit, git push, git pull, git status, git log, git branch, git checkout, git merge, git stash, etc.)
- For commit messages: if the user provides one use it, otherwise invent a short descriptive one
- If the request is ambiguous or has nothing to do with git, return commands: [] and explain in the explanation field
- Warn about destructive operations (--force, --hard reset, etc.) in the explanation
- Never include shell operators like && in a single command string — split them into separate commands in the array"#;

// ─── Shared JSON parser ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RawResponse {
    commands: Vec<String>,
    explanation: String,
}

fn parse_response(text: &str) -> Result<AiResponse, String> {
    // Strip markdown code fences some models add despite instructions
    let clean = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let raw: RawResponse = serde_json::from_str(clean).map_err(|e| {
        format!(
            "AI returned unexpected output ({}).\nRaw: {}",
            e,
            &text[..text.len().min(300)]
        )
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
        Self {
            config,
            client: reqwest::blocking::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OaiRequest<'a> {
    model: &'a str,
    messages: Vec<OaiMessage<'a>>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct OaiMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Deserialize)]
struct OaiResponse {
    choices: Vec<OaiChoice>,
}

#[derive(Deserialize)]
struct OaiChoice {
    message: OaiMessageOwned,
}

#[derive(Deserialize)]
struct OaiMessageOwned {
    content: String,
}

impl AiProvider for OpenAiCompatProvider {
    fn ask(&self, input: &str) -> Result<AiResponse, String> {
        let url = format!("{}/chat/completions", self.config.api_base());
        let model = self.config.effective_model();

        let body = OaiRequest {
            model,
            messages: vec![
                OaiMessage {
                    role: "system",
                    content: SYSTEM_PROMPT.to_string(),
                },
                OaiMessage {
                    role: "user",
                    content: input.to_string(),
                },
            ],
            temperature: 0.1,
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API returned {}: {}", status, &body[..body.len().min(200)]));
        }

        let oai: OaiResponse = resp
            .json()
            .map_err(|e| format!("Failed to parse API response: {}", e))?;

        let text = oai
            .choices
            .into_iter()
            .next()
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
        Self {
            config,
            client: reqwest::blocking::Client::new(),
        }
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
        let url = "https://api.anthropic.com/v1/messages";
        let model = self.config.effective_model();

        let body = ClaudeRequest {
            model,
            max_tokens: 1024,
            system: SYSTEM_PROMPT,
            messages: vec![ClaudeMessage {
                role: "user",
                content: input,
            }],
        };

        let resp = self
            .client
            .post(url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API returned {}: {}", status, &body[..body.len().min(200)]));
        }

        let cr: ClaudeResponse = resp
            .json()
            .map_err(|e| format!("Failed to parse API response: {}", e))?;

        let text = cr
            .content
            .into_iter()
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
        Self {
            config,
            client: reqwest::blocking::Client::new(),
        }
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
                parts: vec![GeminiPart {
                    text: SYSTEM_PROMPT.to_string(),
                }],
            },
            contents: vec![GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart {
                    text: input.to_string(),
                }],
            }],
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("Network error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(format!("API returned {}: {}", status, &body[..body.len().min(200)]));
        }

        let gr: GeminiResponse = resp
            .json()
            .map_err(|e| format!("Failed to parse API response: {}", e))?;

        let text = gr
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content.parts.into_iter().next())
            .map(|p| p.text)
            .unwrap_or_default();

        parse_response(&text)
    }
}
