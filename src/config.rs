use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub provider: String,
    pub api_key: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub github_username: Option<String>,
    pub github_token: Option<String>,
}

impl Config {
    pub fn effective_model(&self) -> &str {
        self.model.as_deref().unwrap_or_else(|| match self.provider.as_str() {
            "deepseek" => "deepseek-chat",
            "openai"   => "gpt-4o-mini",
            "claude"   => "claude-sonnet-4-20250514",
            "gemini"   => "gemini-1.5-flash",
            _          => "gpt-4o-mini",
        })
    }

    pub fn api_base(&self) -> &str {
        self.base_url.as_deref().unwrap_or_else(|| match self.provider.as_str() {
            "deepseek" => "https://api.deepseek.com/v1",
            "openai"   => "https://api.openai.com/v1",
            "claude"   => "https://api.anthropic.com",
            "gemini"   => "https://generativelanguage.googleapis.com",
            _          => "https://api.openai.com/v1",
        })
    }

    /// Build the authenticated GitHub remote URL for a given repo name.
    pub fn github_remote(&self, repo: &str) -> Option<String> {
        let username = self.github_username.as_deref()?;
        let token    = self.github_token.as_deref()?;
        Some(format!(
            "https://{}:{}@github.com/{}/{}.git",
            username, token, username, repo
        ))
    }

    pub fn has_github(&self) -> bool {
        self.github_username.is_some() && self.github_token.is_some()
    }
}

pub fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("mygit");
    path.push("config.toml");
    path
}

pub fn load_or_setup() -> Result<Config, String> {
    let path = config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        toml::from_str(&content)
            .map_err(|e| format!("Invalid config: {}\n  Fix: {}", e, path.display()))
    } else {
        println!("\n  Welcome to {} — let's set you up.\n", "mygit".cyan().bold());
        let config = setup_wizard()?;
        save(&config)?;
        println!("\n  Config saved to {}\n", config_path().display());
        Ok(config)
    }
}

pub fn save(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialise: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write config: {}", e))
}

fn setup_wizard() -> Result<Config, String> {
    println!("  Which AI provider?\n");
    println!("    1. DeepSeek  (recommended)");
    println!("    2. OpenAI");
    println!("    3. Claude");
    println!("    4. Gemini");
    println!("    5. Custom\n");
    print!("  Choice [1]: ");
    io::stdout().flush().unwrap();
    let mut choice = String::new();
    io::stdin().read_line(&mut choice).unwrap();
    let provider = match choice.trim() {
        "2" => "openai",
        "3" => "claude",
        "4" => "gemini",
        "5" => "custom",
        _   => "deepseek",
    }.to_string();

    println!();
    print!("  AI API key: ");
    io::stdout().flush().unwrap();
    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key).unwrap();
    let api_key = api_key.trim().to_string();
    if api_key.is_empty() {
        return Err("API key cannot be empty.".into());
    }

    let base_url = if provider == "custom" {
        print!("  Base URL: ");
        io::stdout().flush().unwrap();
        let mut url = String::new();
        io::stdin().read_line(&mut url).unwrap();
        let url = url.trim().to_string();
        if url.is_empty() { None } else { Some(url) }
    } else { None };

    // GitHub (optional)
    println!();
    print!("  GitHub username (or press Enter to skip): ");
    io::stdout().flush().unwrap();
    let mut gh_user = String::new();
    io::stdin().read_line(&mut gh_user).unwrap();
    let gh_user = gh_user.trim().to_string();

    let (github_username, github_token) = if gh_user.is_empty() {
        (None, None)
    } else {
        println!("  GitHub token (github.com → Settings → Developer settings → Personal access tokens → repo scope):");
        print!("  Token: ");
        io::stdout().flush().unwrap();
        let mut token = String::new();
        io::stdin().read_line(&mut token).unwrap();
        let token = token.trim().to_string();
        if token.is_empty() { (None, None) } else { (Some(gh_user), Some(token)) }
    };

    Ok(Config { provider, api_key, model: None, base_url, github_username, github_token })
}

/// Called mid-session when user says "connect my github" or similar.
pub fn setup_github(config: &mut Config) -> Result<(), String> {
    println!();
    print!("  GitHub username: ");
    io::stdout().flush().unwrap();
    let mut user = String::new();
    io::stdin().read_line(&mut user).unwrap();
    let user = user.trim().to_string();
    if user.is_empty() { return Err("Username cannot be empty".into()); }

    print!("  GitHub token: ");
    io::stdout().flush().unwrap();
    let mut token = String::new();
    io::stdin().read_line(&mut token).unwrap();
    let token = token.trim().to_string();
    if token.is_empty() { return Err("Token cannot be empty".into()); }

    config.github_username = Some(user.clone());
    config.github_token    = Some(token);
    save(config)?;
    println!("  {} GitHub connected as {}!", "✓".green(), user.yellow());
    Ok(())
}
