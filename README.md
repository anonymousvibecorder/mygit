# mygit 🦀

> AI-powered git assistant — just tell it what you want in plain English.

```
you → push all my changes with the message "fix login bug"
  This will stage all changes, commit with your message, and push to the remote.

  ▶ git add .
  ▶ git commit -m "fix login bug"
  ▶ git push

  Run? [Y/n]
```

## Features

- 🗣️ **Natural language** — "save my work", "undo my last commit", "show what changed"
- 🔌 **Multi-provider** — DeepSeek, OpenAI, Claude, Gemini, or any OpenAI-compatible API
- ✅ **Always confirms** before running anything
- 🚀 **Single binary** — no runtime, no dependencies after build
- 🐧 **Cross-platform** — Linux, macOS, Windows

## Install

### From source (requires [Rust](https://rustup.rs))

```bash
git clone https://github.com/YOUR_USERNAME/mygit
cd mygit
cargo build --release
sudo cp target/release/mygit /usr/local/bin/
```

### First run

```bash
mygit
```

It will ask you to pick an AI provider and enter your API key. Config is saved to
`~/.config/mygit/config.toml` (Linux/macOS) or `%APPDATA%\mygit\config.toml` (Windows).

## Get an API key

| Provider | Link | Notes |
|----------|------|-------|
| DeepSeek | https://platform.deepseek.com/api_keys | Recommended — cheap & great at code |
| OpenAI | https://platform.openai.com/api-keys | |
| Anthropic | https://console.anthropic.com/settings/keys | |
| Google | https://aistudio.google.com/apikey | |

## Usage examples

```
you → what changed in my repo
you → save everything with message "add dark mode"
you → push my changes
you → undo my last commit but keep the files
you → create a new branch called feature/auth
you → switch to main branch
you → show my last 5 commits
you → pull the latest changes
you → stash my work
you → merge the feature branch into main
```

## Configuration

`~/.config/mygit/config.toml`:

```toml
provider = "deepseek"
api_key = "sk-..."

# Optional overrides:
# model = "deepseek-reasoner"
# base_url = "http://localhost:11434/v1"   # for local models like Ollama
```

### Using a local model (Ollama)

```toml
provider = "custom"
api_key = "ollama"
base_url = "http://localhost:11434/v1"
model = "codellama"
```

## Contributing

Pull requests welcome! Some ideas:

- [ ] `--explain` flag to explain a git command without running it
- [ ] Shell completions
- [ ] Git history-aware context (pass `git log --oneline -5` to the AI)
- [ ] `mygit undo` — smart undo last action

## License

MIT
