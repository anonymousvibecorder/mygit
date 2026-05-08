mod ai;
mod config;
mod git;

use colored::Colorize;
use std::io::{self, Write};

fn main() {
    let mut config = match config::load_or_setup() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {}", "error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    println!(
        "\n{} — {}\n",
        "mygit".cyan().bold(),
        "your AI git assistant".dimmed()
    );
    println!(
        "  provider: {}   model: {}   github: {}\n",
        config.provider.yellow(),
        config.effective_model().dimmed(),
        if config.has_github() {
            config.github_username.as_deref().unwrap_or("").green().to_string()
        } else {
            "not connected (say \"connect my github\")".red().to_string()
        }
    );
    println!("{}", "  Just tell me what you want to do. Type 'exit' to quit.\n".dimmed());

    let provider = ai::create_provider(&config);

    loop {
        print!("{} ", "you →".green().bold());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let input = input.trim();
        if input.is_empty() { continue; }
        if matches!(input, "exit" | "quit" | "q") { break; }

        // Built-in: connect GitHub account
        if is_github_setup_request(input) {
            if let Err(e) = config::setup_github(&mut config) {
                eprintln!("  {} {}", "error:".red(), e);
            }
            println!();
            continue;
        }

        // Built-in: show config path
        if matches!(input, "config" | "config path" | "where is my config") {
            println!("  {}\n", config::config_path().display().to_string().dimmed());
            continue;
        }

        println!();
        print!("  {} Thinking...", "⟳".cyan());
        io::stdout().flush().unwrap();

        // Inject GitHub context into the request so the AI can use it
        let enriched = enrich_input(input, &config);

        match provider.ask(&enriched) {
            Ok(response) => {
                print!("\r                          \r");

                println!("  {}", response.explanation.dimmed());
                println!();

                if response.commands.is_empty() {
                    println!("  {}\n", "(no commands needed)".yellow());
                    continue;
                }

                for cmd in &response.commands {
                    println!("  {} {}", "▶".yellow(), cmd.bold());
                }
                println!();

                print!("  Run? {} ", "[Y/n]".dimmed());
                io::stdout().flush().unwrap();

                let mut confirm = String::new();
                io::stdin().read_line(&mut confirm).unwrap();
                let confirm = confirm.trim().to_lowercase();

                println!();
                if confirm.is_empty() || confirm == "y" || confirm == "yes" {
                    for cmd in &response.commands {
                        println!("  {} {}", "$".dimmed(), cmd.bold());
                        git::run(cmd);
                        println!();
                    }
                } else {
                    println!("  {}", "Skipped.".dimmed());
                }
            }
            Err(e) => {
                print!("\r                          \r");
                eprintln!("  {} {}", "error:".red().bold(), e);
            }
        }

        println!();
    }

    println!("{}", "\nBye! 👋".cyan());
}

/// Detect when the user wants to connect their GitHub account.
fn is_github_setup_request(input: &str) -> bool {
    let lower = input.to_lowercase();
    (lower.contains("connect") || lower.contains("setup") || lower.contains("set up") || lower.contains("add"))
        && lower.contains("github")
}

/// Add GitHub context to the prompt so the AI can build correct commands.
fn enrich_input(input: &str, config: &config::Config) -> String {
    if !config.has_github() {
        return input.to_string();
    }

    let username = config.github_username.as_deref().unwrap_or("");
    let token    = config.github_token.as_deref().unwrap_or("");

    // Extract repo name from current directory for convenience
    let repo_name = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "repo".to_string());

    format!(
        "{}\n\n[GitHub context: username={}, token={}, current_dir_name={}. \
        When building remote URLs use: https://{}:{}@github.com/{}/REPONAME.git]",
        input, username, token, repo_name, username, token, username
    )
}
