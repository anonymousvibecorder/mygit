mod ai;
mod config;
mod git;

use colored::Colorize;
use std::io::{self, Write};

fn main() {
    let config = match config::load_or_setup() {
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
        "  provider: {}   model: {}\n",
        config.provider.yellow(),
        config.effective_model().dimmed()
    );
    println!("{}", "  Just tell me what you want to do. Type 'exit' to quit.\n".dimmed());

    let provider = ai::create_provider(&config);

    loop {
        print!("{} ", "you →".green().bold());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) | Err(_) => break, // EOF
            Ok(_) => {}
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input, "exit" | "quit" | "q") {
            break;
        }

        // Special: show config path
        if matches!(input, "config" | "where is my config" | "config path") {
            println!(
                "  {}\n",
                config::config_path().display().to_string().dimmed()
            );
            continue;
        }

        println!();
        print!("  {} Thinking...", "⟳".cyan());
        io::stdout().flush().unwrap();

        match provider.ask(input) {
            Ok(response) => {
                // Clear the "Thinking..." line
                print!("\r  {}                    \r", " ".repeat(20));

                println!("  {}", response.explanation.dimmed());
                println!();

                if response.commands.is_empty() {
                    println!("  {}", "(no commands needed)".yellow());
                    println!();
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
                print!("\r  {}                    \r", " ".repeat(20));
                eprintln!("  {} {}", "error:".red().bold(), e);
            }
        }

        println!();
    }

    println!("{}", "\nBye! 👋".cyan());
}
