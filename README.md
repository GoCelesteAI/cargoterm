# cargoterm

An AI-augmented terminal in Rust. Type plain commands like you would in any shell — or type in plain English and let a local LLM translate it into a shell command for you.

```text
cargoterm 0.1 — type 'exit' or Ctrl-D to quit
>>> pwd
/Volumes/wd/code/Rust/cargoterm
>>> show present directory
[interpreted: pwd] run? [Y/n] Y
/Volumes/wd/code/Rust/cargoterm
>>> who am i
[interpreted: whoami] run? [Y/n] Y
somaria
>>> and its size
[interpreted: du -sh . ] run? [Y/n] Y
...
```

Everything runs **locally** — the LLM lives on your machine via [Ollama](https://ollama.com). Nothing leaves your box.

## Features

- `>>>` REPL with readline editing and persistent history (`~/.cargoterm_history`)
- Built-ins: `cd`, `pwd`, `exit`
- Transparent pass-through to any command on your `PATH` (`ls`, `whoami`, `git`, …)
- Natural-language fallback routed to a local LLM (Qwen via Ollama by default)
- **Confirmation prompt** before executing any LLM-generated command
- Denylist blocks obviously destructive operations (`rm`, `sudo`, `dd`, `mkfs`, …) before the confirmation step
- Context memory: the last 5 turns are fed back to the model, so follow-ups like *"and its size"* or *"list files there"* work

## Quickstart

Four steps to go from zero to `>>>`:

```sh
# 1. Install Rust (skip if you already have it)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Install and start Ollama (macOS shown; see ollama.com for Linux)
brew install ollama
ollama serve &          # or launch the Ollama app

# 3. Pull the default model (one-time, ~9 GB)
ollama pull qwen3:14b

# 4. Install cargoterm
git clone https://github.com/GoCelesteAI/cargoterm.git
cd cargoterm
cargo install --path .
```

`cargo install` compiles in release mode and drops the binary at `~/.cargo/bin/cargoterm`. That directory is on your `PATH` if you installed Rust via rustup, so from any directory you can now just type:

```sh
cargoterm
```

and get the `>>>` prompt.

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

Anything that doesn't is sent to the model, which replies with a single shell command. You are always shown the interpretation and asked to confirm:

```text
>>> what day is it
[interpreted: date] run? [Y/n]
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

Currently hard-coded in `src/ollama.rs`:

| Setting  | Default                             |
| -------- | ----------------------------------- |
| Endpoint | `http://localhost:11434/api/generate` |
| Model    | `qwen3:14b`                         |
| Timeout  | 60s                                 |

Editing those constants and rebuilding is the current way to swap models. A proper config file is on the roadmap.

## Safety

LLMs can and do produce wrong, surprising, or dangerous shell commands. cargoterm mitigates this with two gates:

1. **Denylist.** Any LLM-generated command containing `rm`, `sudo`, `mkfs`, `dd`, `shutdown`, `reboot`, `chmod`, or a fork-bomb pattern is refused outright.
2. **Confirmation.** Every LLM-generated command is shown in full and requires your explicit approval before execution.

Direct commands you type yourself (matching a binary on `PATH`) are **not** gated — cargoterm trusts you to know what `rm` does when you type it.

The denylist is defensive, not exhaustive. Read what you run.

## Architecture

```text
src/
├── main.rs      REPL, dispatcher, built-ins, confirm/deny gates
├── ollama.rs    HTTP client + strict JSON prompt for the model
└── history.rs   Ring buffer of recent turns, rendered into the prompt
```

Dispatch order for any input line:

1. Built-in? (`cd`, `pwd`, `exit`) → handle in-process.
2. First token on `PATH`? → exec directly, capture output.
3. Otherwise → send to Ollama, confirm, run via `$SHELL -c`.

## Roadmap

- [ ] Prebuilt binaries on GitHub Releases (macOS arm64/x86_64, Linux x86_64)
- [ ] Homebrew formula (`brew install gocelesteai/tap/cargoterm`)
- [ ] `cargoterm --setup` — verify Ollama, pull the default model
- [ ] Config file (`~/.config/cargoterm/config.toml`) — model, endpoint, denylist
- [ ] Streaming command output instead of buffered capture
- [ ] Prompt that shows the current working directory
- [ ] Whitelist of safe read-only commands that skip the confirmation prompt
- [ ] Optional explanation mode: ask the model *why* before running
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
