# cargoterm

[![CI](https://github.com/GoCelesteAI/cargoterm/actions/workflows/ci.yml/badge.svg)](https://github.com/GoCelesteAI/cargoterm/actions/workflows/ci.yml)
[![Release](https://github.com/GoCelesteAI/cargoterm/actions/workflows/release.yml/badge.svg)](https://github.com/GoCelesteAI/cargoterm/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An AI-augmented terminal in Rust. Type plain commands like you would in any shell — or type in plain English and let a local LLM translate it into a shell command for you.

```text
cargoterm 0.1.0 — type 'exit' or Ctrl-D to quit
~/code/cargoterm >>> pwd
/Users/you/code/cargoterm
~/code/cargoterm >>> who am i
[auto: whoami]
you
~/code/cargoterm >>> show present directory size
[interpreted: du -sh .]
[what this does: Reports the total disk usage of the current directory in human-readable form.]
run? [Y/n] Y
 12M    .
~/code/cargoterm >>> and sort the files here by size
[interpreted: ls -lS]
[what this does: Lists files in the current directory sorted by size, largest first.]
run? [Y/n] Y
...
```

Everything runs **locally** — the LLM lives on your machine via [Ollama](https://ollama.com). Nothing leaves your box.

## Features

- Cwd-aware `>>>` REPL with readline editing and persistent history (`~/.cargoterm_history`)
- Built-ins: `cd`, `pwd`, `exit`
- Transparent pass-through to any command on your `PATH` (`ls`, `whoami`, `git`, …)
- Natural-language fallback routed to a local LLM (Qwen via Ollama by default)
- **Allowlist auto-approve** — known read-only commands (`pwd`, `whoami`, `ls`, `date`, `du`, …) run immediately when the LLM emits them clean
- **Confirmation gate with explanation** for anything else — the command is shown alongside a plain-English description of what it will do
- Denylist blocks obviously destructive operations (`rm`, `sudo`, `dd`, `mkfs`, …) outright
- Context memory: the last 5 turns are fed back to the model, so follow-ups like *"and its size"* or *"list files there"* work
- `cargoterm --setup` health-checks Ollama and offers to pull the default model

## Quickstart

### Option A — Homebrew (macOS & Linux, recommended)

```sh
brew install gocelesteai/tap/cargoterm
brew install ollama            # if you don't have it yet
ollama serve &                 # or launch the Ollama app on macOS
cargoterm --setup              # verifies ollama, pulls the default model
cargoterm                      # drops you into the >>> REPL
```

Upgrades: `brew upgrade gocelesteai/tap/cargoterm`.

### Option B — prebuilt binary (manual)

Grab the latest release for your platform from [Releases](https://github.com/GoCelesteAI/cargoterm/releases) and extract it onto your `PATH`:

```sh
# Pick the tarball matching your platform:
#   cargoterm-<version>-aarch64-apple-darwin.tar.gz    (macOS Apple Silicon)
#   cargoterm-<version>-x86_64-apple-darwin.tar.gz     (macOS Intel)
#   cargoterm-<version>-x86_64-unknown-linux-gnu.tar.gz (Linux x86_64)

curl -L -o cargoterm.tar.gz \
  https://github.com/GoCelesteAI/cargoterm/releases/latest/download/cargoterm-<version>-<target>.tar.gz
tar -xzf cargoterm.tar.gz
sudo mv cargoterm-*/cargoterm /usr/local/bin/
```

On macOS you may need to clear the quarantine attribute the first time:

```sh
xattr -d com.apple.quarantine /usr/local/bin/cargoterm
```

Then install [Ollama](https://ollama.com), run `cargoterm --setup`, and you're done.

### Option C — build from source

```sh
# 1. Install Rust (skip if you already have it)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Install and start Ollama (macOS shown; see ollama.com for Linux)
brew install ollama
ollama serve &          # or launch the Ollama app

# 3. Install cargoterm
git clone https://github.com/GoCelesteAI/cargoterm.git
cd cargoterm
cargo install --path .

# 4. Verify + pull the default model (~9 GB, one-time)
cargoterm --setup

# 5. Start the REPL from anywhere
cargoterm
```

`cargo install` compiles in release mode and drops the binary at `~/.cargo/bin/cargoterm`. That directory is on your `PATH` if you installed Rust via rustup, so typing `cargoterm` from any directory drops you into the `>>>` prompt.

`cargoterm --setup` runs a three-step health check (Ollama binary → daemon → default model) and, if the model is missing, offers to pull it for you via `ollama pull`. Run it anytime something feels broken.

### Running from source (for development)

```sh
git clone https://github.com/GoCelesteAI/cargoterm.git
cd cargoterm
cargo run
```

## Requirements

- **Rust** stable, 2024 edition — [rustup.rs](https://rustup.rs)
- **[Ollama](https://ollama.com)** running locally on `http://localhost:11434`
- A model pulled and available (default: `qwen3:14b`)
- **Unix-like OS** — macOS or Linux. Windows is not currently supported.

If Ollama isn't running, cargoterm still starts and runs direct shell commands fine — only natural-language input will error with a hint telling you to start Ollama.

## Usage

Anything that looks like a shell command is executed as one:

```text
>>> ls -la
>>> git status
>>> cd ~/projects
```

Anything that doesn't is sent to the model, which replies with a single shell command plus a plain-English description. Known-safe read-only commands run immediately; anything else is shown to you for approval:

```text
>>> what day is it
[auto: date]
Wed Apr 15 10:27:31 PDT 2026

>>> free up space
[interpreted: rm -rf ~/.cache/*]
[blocked: contains 'rm']

>>> how big is my cache folder
[interpreted: du -sh ~/Library/Caches]
[what this does: Reports the total disk usage of your user cache directory.]
run? [Y/n]
```

Press `Y` / Enter to run, `n` to cancel.

### Follow-ups

The last 5 turns (your input, the command that ran, and its output, truncated) are passed back to the model on each call, so references to previous turns resolve naturally:

```text
>>> list files here
>>> sort them by size
>>> what's the biggest one
```

## Configuration

cargoterm reads an optional TOML config file on startup. The default location follows XDG:

```
$XDG_CONFIG_HOME/cargoterm/config.toml
# falls back to ~/.config/cargoterm/config.toml
```

Pass `--config PATH` to override. All keys are optional — anything missing falls back to the built-in default, so the file can be as small as you need.

```toml
[ollama]
host = "http://localhost:11434"
model = "qwen3:14b"
timeout_secs = 60

[safety]
# Commands containing any of these tokens are refused outright.
deny = ["rm", "sudo", "mkfs", "dd", "shutdown", "reboot", ":(){", "chmod"]

# Clean commands (no shell metacharacters) whose first token is here
# will auto-run without a confirmation prompt.
allow = ["pwd", "whoami", "ls", "date", "du", "cat", "head", "tail", "stat", "file"]
```

To see the effective merged config:

```sh
cargoterm --print-config
```

Want to swap to a smaller local model? Create the file above and change `model` to e.g. `qwen3:8b` or `llama3:8b`, then `ollama pull <that model>` and you're done — no rebuild needed.

## Safety

LLMs can and do produce wrong, surprising, or dangerous shell commands. cargoterm mitigates this with layered gates, applied in order to every LLM-generated command:

1. **Denylist** — anything containing `rm`, `sudo`, `mkfs`, `dd`, `shutdown`, `reboot`, `chmod`, or a fork-bomb pattern is refused outright.
2. **Allowlist auto-approve** — known read-only commands (`pwd`, `whoami`, `ls`, `date`, `du`, `cat`, `head`, `tail`, `file`, `stat`, …) run immediately, but *only* when the command contains no shell metacharacters (`|`, `&`, `;`, `>`, `<`, `` ` ``, `$`). A command like `ls | rm -rf /` does **not** auto-approve.
3. **Explanation + confirmation** — anything else is shown with its command AND a plain-English description from the model of what it does. You must press `Y` / Enter to run.
4. **Untrusted output** — captured command output fed back into the model's context is flagged as untrusted, so the model is less likely to follow injected instructions hiding in files, log lines, or git output.

Direct commands you type yourself (matching a binary on `PATH`) are **not** gated — cargoterm trusts you to know what `rm` does when you type it.

The denylist is defensive, not exhaustive. Read what you run.

## Architecture

```text
src/
├── main.rs      REPL, dispatcher, built-ins, confirm/deny/allow gates
├── config.rs    TOML config loader + defaults
├── ollama.rs    HTTP client + strict JSON prompt for the model
├── history.rs   Ring buffer of recent turns, rendered into the prompt
└── setup.rs     `cargoterm --setup` health check + model pull
```

Dispatch order for any input line:

1. Built-in? (`cd`, `pwd`, `exit`) → handle in-process.
2. First token on `PATH`? → exec directly, capture output.
3. Otherwise → send to Ollama. Denylist → block. Allowlist (clean, no metachars) → run. Else → confirm with explanation, then run via `$SHELL -c`.

## Roadmap

- [ ] Streaming command output instead of buffered capture
- [ ] Session transcript export

## Contributing

Issues and PRs welcome. Please open an issue before starting substantial work so design can be discussed first.

```sh
cargo build
cargo test
cargo clippy -- -D warnings
```

## License

MIT. See [LICENSE](LICENSE).
