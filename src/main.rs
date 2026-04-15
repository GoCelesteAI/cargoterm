mod config;
mod exec;
mod history;
mod ollama;
mod setup;

use anyhow::Result;
use config::Config;
use history::History;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const SHELL_METACHARS: &[char] = &['|', '&', ';', '>', '<', '`', '$', '\\', '\n'];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("cargoterm {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let cfg_override = extract_flag_value(&args, "--config").map(PathBuf::from);
    let (cfg, cfg_path) = config::load(cfg_override.as_deref())?;

    if args.iter().any(|a| a == "--print-config") {
        if let Some(p) = &cfg_path {
            println!(
                "# source: {}{}",
                p.display(),
                if p.exists() { "" } else { " (not present)" }
            );
        }
        print!("{}", config::render(&cfg));
        return Ok(());
    }

    if args.iter().any(|a| a == "--setup") {
        return setup::run(&cfg, cfg_path.as_deref()).await;
    }

    let mut rl = DefaultEditor::new()?;
    let history_path: PathBuf = home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cargoterm_history");
    let _ = rl.load_history(&history_path);

    let mut hist = History::new();

    println!(
        "cargoterm {} — type 'exit' or Ctrl-D to quit",
        env!("CARGO_PKG_VERSION")
    );

    loop {
        let prompt = build_prompt();
        match rl.readline(&prompt) {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }
                rl.add_history_entry(input)?;
                if !dispatch(input, &cfg, &mut hist).await {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("readline error: {e}");
                break;
            }
        }
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}

async fn dispatch(input: &str, cfg: &Config, hist: &mut History) -> bool {
    let mut parts = input.split_whitespace();
    let first = parts.next().unwrap_or("");
    let args: Vec<&str> = parts.collect();

    match first {
        "exit" | "quit" => return false,
        "cd" => {
            let target = args.first().copied().unwrap_or("~");
            let path = expand_tilde(target);
            match env::set_current_dir(&path) {
                Ok(()) => hist.push(input, input, ""),
                Err(e) => eprintln!("cd: {}: {e}", path.display()),
            }
            return true;
        }
        "pwd" => {
            let out = match env::current_dir() {
                Ok(p) => p.display().to_string(),
                Err(e) => {
                    eprintln!("pwd: {e}");
                    return true;
                }
            };
            println!("{out}");
            hist.push(input, "pwd", &out);
            return true;
        }
        _ => {}
    }

    if is_on_path(first) {
        let captured = exec::run_direct(first, &args).await;
        hist.push(input, input, &captured);
        return true;
    }

    match ollama::interpret(&cfg.ollama, input, &hist.render()).await {
        Ok(interp) => {
            if let Some(bad) = deny_hit(&cfg.safety.deny, &interp.cmd) {
                eprintln!("[blocked: contains '{bad}'] {}", interp.cmd);
                return true;
            }
            let should_run = if is_safe_auto(&cfg.safety.allow, &interp.cmd) {
                println!("[auto: {}]", interp.cmd);
                true
            } else {
                confirm(&interp.cmd, &interp.explain)
            };
            if should_run {
                let captured = exec::run_shell(&interp.cmd).await;
                hist.push(input, &interp.cmd, &captured);
            }
        }
        Err(e) => eprintln!("cargoterm: {e}"),
    }
    true
}

fn is_on_path(cmd: &str) -> bool {
    if cmd.is_empty() {
        return false;
    }
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|p| p.join(cmd).is_file())
}

fn deny_hit<'a>(deny: &'a [String], cmd: &str) -> Option<&'a str> {
    let lower = cmd.to_lowercase();
    deny.iter()
        .find(|bad| {
            lower
                .split(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != '(' && c != '{')
                .any(|tok| tok == bad.as_str())
        })
        .map(|s| s.as_str())
}

fn has_metachars(cmd: &str) -> bool {
    cmd.chars().any(|c| SHELL_METACHARS.contains(&c))
}

fn is_safe_auto(allow: &[String], cmd: &str) -> bool {
    if has_metachars(cmd) {
        return false;
    }
    let first = cmd.split_whitespace().next().unwrap_or("");
    allow.iter().any(|a| a == first)
}

fn extract_flag_value(args: &[String], flag: &str) -> Option<String> {
    for (i, a) in args.iter().enumerate() {
        if a == flag {
            return args.get(i + 1).cloned();
        }
        if let Some(rest) = a.strip_prefix(&format!("{flag}=")) {
            return Some(rest.to_string());
        }
    }
    None
}

fn confirm(cmd: &str, explain: &str) -> bool {
    println!("[interpreted: {cmd}]");
    if !explain.is_empty() {
        println!("[what this does: {explain}]");
    }
    print!("run? [Y/n] ");
    let _ = io::stdout().flush();
    let mut buf = String::new();
    if io::stdin().read_line(&mut buf).is_err() {
        return false;
    }
    let s = buf.trim().to_lowercase();
    s.is_empty() || s == "y" || s == "yes"
}

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(stripped) = p.strip_prefix('~')
        && let Some(home) = home_dir()
    {
        let rest = stripped.trim_start_matches('/');
        return if rest.is_empty() {
            home
        } else {
            home.join(rest)
        };
    }
    PathBuf::from(p)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn build_prompt() -> String {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("?"));
    format!("{} >>> ", shorten_path(&cwd))
}

fn shorten_path(p: &Path) -> String {
    if let Some(home) = home_dir()
        && let Ok(rest) = p.strip_prefix(&home)
    {
        let rest_str = rest.to_string_lossy();
        return if rest_str.is_empty() {
            "~".to_string()
        } else {
            format!("~/{rest_str}")
        };
    }
    p.display().to_string()
}

fn print_help() {
    println!(
        "cargoterm {} — AI-augmented terminal

USAGE:
    cargoterm [OPTIONS]

OPTIONS:
    --setup             Check that Ollama and the default model are ready
    --config PATH       Use an alternate config file (default: $XDG_CONFIG_HOME/cargoterm/config.toml)
    --print-config      Print the effective config (merged with defaults) and exit
    -h, --help          Show this help
    -V, --version       Show version

Inside the REPL: type shell commands normally, or plain English to let the
local LLM translate. LLM-generated commands show a confirmation prompt
unless they are on the known-safe allowlist.",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(not(unix))]
compile_error!("cargoterm currently targets Unix-like systems (macOS/Linux)");

#[cfg(test)]
mod tests {
    use super::{deny_hit, extract_flag_value, has_metachars, is_safe_auto, shorten_path};
    use crate::config::SafetyConfig;
    use std::path::PathBuf;

    fn safety() -> SafetyConfig {
        SafetyConfig::default()
    }

    #[test]
    fn blocks_rm() {
        assert!(deny_hit(&safety().deny, "rm -rf /").is_some());
    }
    #[test]
    fn blocks_sudo() {
        assert!(deny_hit(&safety().deny, "sudo ls").is_some());
    }
    #[test]
    fn allows_pwd() {
        assert!(deny_hit(&safety().deny, "pwd").is_none());
    }
    #[test]
    fn allows_whoami() {
        assert!(deny_hit(&safety().deny, "whoami").is_none());
    }

    #[test]
    fn auto_whoami() {
        assert!(is_safe_auto(&safety().allow, "whoami"));
    }
    #[test]
    fn auto_pwd() {
        assert!(is_safe_auto(&safety().allow, "pwd"));
    }
    #[test]
    fn auto_ls_with_flags() {
        assert!(is_safe_auto(&safety().allow, "ls -la"));
    }
    #[test]
    fn not_auto_git() {
        assert!(!is_safe_auto(&safety().allow, "git status"));
    }
    #[test]
    fn not_auto_pipe_even_if_safe_first() {
        assert!(!is_safe_auto(&safety().allow, "ls | rm -rf /"));
    }
    #[test]
    fn not_auto_redirect() {
        assert!(!is_safe_auto(&safety().allow, "cat /etc/passwd > /tmp/out"));
    }
    #[test]
    fn not_auto_subshell() {
        assert!(!is_safe_auto(&safety().allow, "echo $(rm -rf /)"));
    }
    #[test]
    fn not_auto_backtick() {
        assert!(!is_safe_auto(&safety().allow, "echo `rm -rf /`"));
    }

    #[test]
    fn flag_value_separated() {
        let args: Vec<String> = ["cargoterm", "--config", "/tmp/c.toml"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            extract_flag_value(&args, "--config"),
            Some("/tmp/c.toml".to_string())
        );
    }
    #[test]
    fn flag_value_equals() {
        let args: Vec<String> = ["cargoterm", "--config=/tmp/c.toml"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            extract_flag_value(&args, "--config"),
            Some("/tmp/c.toml".to_string())
        );
    }
    #[test]
    fn flag_value_missing() {
        let args: Vec<String> = ["cargoterm"].iter().map(|s| s.to_string()).collect();
        assert_eq!(extract_flag_value(&args, "--config"), None);
    }
    #[test]
    fn metachar_detects_pipe() {
        assert!(has_metachars("a | b"));
    }
    #[test]
    fn metachar_ignores_dash() {
        assert!(!has_metachars("ls -la"));
    }

    #[test]
    fn shorten_leaves_unrelated_paths() {
        let out = shorten_path(&PathBuf::from("/etc/hosts"));
        assert_eq!(out, "/etc/hosts");
    }
    #[test]
    fn shorten_contracts_home_subpath() {
        if let Some(home) = std::env::var_os("HOME") {
            let p = PathBuf::from(&home).join("projects/foo");
            assert_eq!(shorten_path(&p), "~/projects/foo");
        }
    }
    #[test]
    fn shorten_contracts_home_itself() {
        if let Some(home) = std::env::var_os("HOME") {
            assert_eq!(shorten_path(&PathBuf::from(home)), "~");
        }
    }
}
