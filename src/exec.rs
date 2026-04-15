use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub async fn run_direct(cmd: &str, args: &[&str]) -> String {
    let mut command = Command::new(cmd);
    command.args(args);
    stream(command, cmd).await
}

pub async fn run_shell(line: &str) -> String {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut command = Command::new(shell);
    command.arg("-c").arg(line);
    stream(command, line).await
}

async fn stream(mut command: Command, label: &str) -> String {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cargoterm: {label}: {e}");
            return String::new();
        }
    };

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let mut out_lines = BufReader::new(stdout).lines();
    let mut err_lines = BufReader::new(stderr).lines();

    let mut captured = String::new();
    let mut out_done = false;
    let mut err_done = false;

    while !out_done || !err_done {
        tokio::select! {
            res = out_lines.next_line(), if !out_done => {
                match res {
                    Ok(Some(line)) => {
                        println!("{line}");
                        captured.push_str(&line);
                        captured.push('\n');
                    }
                    _ => out_done = true,
                }
            }
            res = err_lines.next_line(), if !err_done => {
                match res {
                    Ok(Some(line)) => {
                        eprintln!("{line}");
                        captured.push_str(&line);
                        captured.push('\n');
                    }
                    _ => err_done = true,
                }
            }
        }
    }

    if let Err(e) = child.wait().await {
        eprintln!("cargoterm: wait: {e}");
    }

    captured
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn captures_stdout_and_stderr() {
        let captured = run_shell("echo hi; echo err 1>&2").await;
        assert!(captured.contains("hi"), "missing stdout in {captured:?}");
        assert!(captured.contains("err"), "missing stderr in {captured:?}");
    }

    #[tokio::test]
    async fn captures_interleaved_lines_in_order() {
        let captured = run_shell("printf 'a\\nb\\nc\\n'").await;
        let idx_a = captured.find('a').expect("no a");
        let idx_b = captured.find('b').expect("no b");
        let idx_c = captured.find('c').expect("no c");
        assert!(idx_a < idx_b && idx_b < idx_c);
    }

    #[tokio::test]
    async fn missing_binary_returns_empty() {
        let captured = run_direct("definitely_not_a_real_binary_xyz", &[]).await;
        assert!(captured.is_empty());
    }
}
