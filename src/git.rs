use colored::Colorize;
use std::process::Command;

/// Run a single git command string via the shell and print the result.
pub fn run(cmd: &str) {
    let result = Command::new("sh").arg("-c").arg(cmd).output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !stdout.trim().is_empty() {
                for line in stdout.trim().lines() {
                    println!("  {}", line);
                }
            }

            if !stderr.trim().is_empty() {
                // git writes a lot of normal progress info to stderr (e.g. "Enumerating objects...")
                // Only colour it red if the command actually failed
                let colour = if output.status.success() {
                    "normal"
                } else {
                    "error"
                };
                for line in stderr.trim().lines() {
                    if colour == "error" {
                        eprintln!("  {}", line.red());
                    } else {
                        println!("  {}", line.dimmed());
                    }
                }
            }

            if output.status.success() {
                println!("  {}", "✓".green());
            } else {
                let code = output.status.code().unwrap_or(-1);
                eprintln!("  {} exited with code {}", "✗".red(), code);
            }
        }
        Err(e) => {
            eprintln!("  {} couldn't run command: {}", "✗".red(), e);
        }
    }
}
