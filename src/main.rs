mod history;
mod ollama;
mod setup;

use anyhow::Result;
use history::History;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const DENY: &[&str] = &["rm", "sudo", "mkfs", "dd", "shutdown", "reboot", ":(){", "chmod"];

const SAFE_ALLOW: &[&str] = &[
    "pwd", "whoami", "hostname", "uname", "date", "uptime", "id", "groups", "tty", "arch",
    "ls", "cat", "head", "tail", "wc", "file", "stat", "readlink", "basename", "dirname",
    "df", "du", "echo", "printf", "which", "type", "env", "printenv", "history",
];

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
    if args.iter().any(|a| a == "--setup") {
        return setup::run().await;
    }

    let mut rl = DefaultEditor::new()?;
    let history_path: PathBuf = home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cargoterm_history");
    let _ = rl.load_history(&history_path);

    let mut hist = History::new();

    println!("cargoterm {} — type 'exit' or Ctrl-D to quit", env!("CARGO_PKG_VERSION"));

    loop {
        let prompt = build_prompt();
        match rl.readline(&prompt) {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }
                rl.add_history_entry(input)?;
                if !dispatch(input, &mut hist).await {
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

async fn dispatch(input: &str, hist: &mut History) -> bool {
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
        let captured = run_external(first, &args);
        hist.push(input, input, &captured);
        return true;
    }

    match ollama::interpret(input, &hist.render()).await {
        Ok(interp) => {
            if let Some(bad) = deny_hit(&interp.cmd) {
                eprintln!("[blocked: contains '{bad}'] {}", interp.cmd);
                return true;
            }
            let should_run = if is_safe_auto(&interp.cmd) {
                println!("[auto: {}]", interp.cmd);
                true
            } else {
                confirm(&interp.cmd, &interp.explain)
            };
            if should_run {
                let captured = run_shell(&interp.cmd);
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

fn deny_hit(cmd: &str) -> Option<&'static str> {
    let lower = cmd.to_lowercase();
    DENY.iter().copied().find(|bad| {
        lower.split(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != '(' && c != '{')
            .any(|tok| tok == *bad)
    })
}

fn has_metachars(cmd: &str) -> bool {
    cmd.chars().any(|c| SHELL_METACHARS.contains(&c))
}

fn is_safe_auto(cmd: &str) -> bool {
    if has_metachars(cmd) {
        return false;
    }
    let first = cmd.split_whitespace().next().unwrap_or("");
    SAFE_ALLOW.contains(&first)
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

fn run_external(cmd: &str, args: &[&str]) -> String {
    match Command::new(cmd).args(args).output() {
        Ok(o) => emit(&o.stdout, &o.stderr),
        Err(e) => {
            eprintln!("cargoterm: {cmd}: {e}");
            String::new()
        }
    }
}

fn run_shell(line: &str) -> String {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    match Command::new(shell).arg("-c").arg(line).output() {
        Ok(o) => emit(&o.stdout, &o.stderr),
        Err(e) => {
            eprintln!("cargoterm: {e}");
            String::new()
        }
    }
}

fn emit(stdout: &[u8], stderr: &[u8]) -> String {
    let out = String::from_utf8_lossy(stdout).into_owned();
    let err = String::from_utf8_lossy(stderr).into_owned();
    if !out.is_empty() {
        print!("{out}");
        if !out.ends_with('\n') {
            println!();
        }
    }
    if !err.is_empty() {
        eprint!("{err}");
        if !err.ends_with('\n') {
            eprintln!();
        }
    }
    if err.is_empty() { out } else { format!("{out}{err}") }
}

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(stripped) = p.strip_prefix('~') {
        if let Some(home) = home_dir() {
            let rest = stripped.trim_start_matches('/');
            return if rest.is_empty() { home } else { home.join(rest) };
        }
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
    if let Some(home) = home_dir() {
        if let Ok(rest) = p.strip_prefix(&home) {
            let rest_str = rest.to_string_lossy();
            return if rest_str.is_empty() {
                "~".to_string()
            } else {
                format!("~/{rest_str}")
            };
        }
    }
    p.display().to_string()
}

fn print_help() {
    println!(
        "cargoterm {} — AI-augmented terminal

USAGE:
    cargoterm [OPTIONS]

OPTIONS:
    --setup       Check that Ollama and the default model are ready
    -h, --help    Show this help
    -V, --version Show version

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
    use super::{deny_hit, has_metachars, is_safe_auto, shorten_path};
    use std::path::PathBuf;

    #[test]
    fn blocks_rm() {
        assert!(deny_hit("rm -rf /").is_some());
    }
    #[test]
    fn blocks_sudo() {
        assert!(deny_hit("sudo ls").is_some());
    }
    #[test]
    fn allows_pwd() {
        assert!(deny_hit("pwd").is_none());
    }
    #[test]
    fn allows_whoami() {
        assert!(deny_hit("whoami").is_none());
    }

    #[test]
    fn auto_whoami() {
        assert!(is_safe_auto("whoami"));
    }
    #[test]
    fn auto_pwd() {
        assert!(is_safe_auto("pwd"));
    }
    #[test]
    fn auto_ls_with_flags() {
        assert!(is_safe_auto("ls -la"));
    }
    #[test]
    fn not_auto_git() {
        assert!(!is_safe_auto("git status"));
    }
    #[test]
    fn not_auto_pipe_even_if_safe_first() {
        assert!(!is_safe_auto("ls | rm -rf /"));
    }
    #[test]
    fn not_auto_redirect() {
        assert!(!is_safe_auto("cat /etc/passwd > /tmp/out"));
    }
    #[test]
    fn not_auto_subshell() {
        assert!(!is_safe_auto("echo $(rm -rf /)"));
    }
    #[test]
    fn not_auto_backtick() {
        assert!(!is_safe_auto("echo `rm -rf /`"));
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
