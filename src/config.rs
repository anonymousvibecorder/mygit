use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// Which AI provider to use: "deepseek", "openai", "claude", "gemini", "custom"
    pub provider: String,

    /// API key for the chosen provider
    pub api_key: String,

    /// Override the default model (optional)
    pub model: Option<String>,

    /// Override the API base URL — useful for local models or proxies (optional)
    pub base_url: Option<String>,
}

impl Config {
    /// Returns the model to use, falling back to a sensible per-provider default.
    pub fn effective_model(&self) -> &str {
        self.model.as_deref().unwrap_or_else(|| match self.provider.as_str() {
            "deepseek" => "deepseek-chat",
            "openai" => "gpt-4o-mini",
            "claude" => "claude-sonnet-4-20250514",
            "gemini" => "gemini-1.5-flash",
            _ => "gpt-4o-mini",
        })
    }

    /// Returns the base API URL, falling back to the provider default.
    pub fn api_base(&self) -> &str {
        self.base_url.as_deref().unwrap_or_else(|| match self.provider.as_str() {
            "deepseek" => "https://api.deepseek.com/v1",
            "openai" => "https://api.openai.com/v1",
            "claude" => "https://api.anthropic.com",
            "gemini" => "https://generativelanguage.googleapis.com",
            _ => "https://api.openai.com/v1",
        })
    }
}

/// Returns the path to the config file.
/// On Linux/macOS: ~/.config/mygit/config.toml
/// On Windows:     %APPDATA%\mygit\config.toml
pub fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("mygit");
    path.push("config.toml");
    path
}

/// Loads the config file, or runs the first-time setup wizard if it doesn't exist.
pub fn load_or_setup() -> Result<Config, String> {
    let path = config_path();

    if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config at {}: {}", path.display(), e))?;
        toml::from_str(&content)
            .map_err(|e| format!("Config file is invalid: {}\n  Fix it at: {}", e, path.display()))
    } else {
        println!(
            "\n  👋  Welcome to {} — let's get you set up.\n",
            "mygit".cyan()
        );
        let config = setup_wizard()?;
        save(&config)?;
        println!("\n  ✅  Config saved to {}\n", config_path().display());
        Ok(config)
    }
}

fn setup_wizard() -> Result<Config, String> {
    println!("  Which AI provider do you want to use?\n");
    println!("    1. DeepSeek  (recommended — cheap, fast, great at code)");
    println!("    2. OpenAI    (GPT-4o-mini by default)");
    println!("    3. Claude    (Anthropic)");
    println!("    4. Gemini    (Google)");
    println!("    5. Custom    (any OpenAI-compatible API)\n");
    print!("  Choice [1]: ");
    io::stdout().flush().unwrap();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).unwrap();

    let provider = match choice.trim() {
        "2" => "openai",
        "3" => "claude",
        "4" => "gemini",
        "5" => "custom",
        _ => "deepseek",
    }
    .to_string();

    println!();
    println!("  Get your API key:");
    match provider.as_str() {
        "deepseek" => println!("    → https://platform.deepseek.com/api_keys"),
        "openai" => println!("    → https://platform.openai.com/api-keys"),
        "claude" => println!("    → https://console.anthropic.com/settings/keys"),
        "gemini" => println!("    → https://aistudio.google.com/apikey"),
        _ => {}
    }
    println!();
    print!("  API key: ");
    io::stdout().flush().unwrap();

    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key).unwrap();
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err("API key cannot be empty.".into());
    }

    let base_url = if provider == "custom" {
        println!();
        print!("  Base URL (e.g. https://api.openai.com/v1): ");
        io::stdout().flush().unwrap();
        let mut url = String::new();
        io::stdin().read_line(&mut url).unwrap();
        let url = url.trim().to_string();
        if url.is_empty() {
            None
        } else {
            Some(url)
        }
    } else {
        None
    };

    Ok(Config {
        provider,
        api_key,
        model: None,
        base_url,
    })
}

fn save(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let content =
        toml::to_string_pretty(config).map_err(|e| format!("Failed to serialise config: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write config to {}: {}", path.display(), e))
}


