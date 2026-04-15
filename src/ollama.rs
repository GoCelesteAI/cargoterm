use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const ENDPOINT: &str = "http://localhost:11434/api/generate";
const MODEL: &str = "qwen3:14b";

const SYSTEM: &str = "You translate a user's natural-language request into a single POSIX shell \
command for macOS. Rules:\n\
1. Respond with ONLY a JSON object: {\"cmd\": \"<shell command>\"}\n\
2. No explanation, no markdown, no code fences, no thinking tags.\n\
3. Use standard Unix commands (pwd, whoami, ls, date, uname, df, etc.).\n\
4. If the request is unsafe or unclear, return {\"cmd\": \"\"}.\n\
5. The user may refer to a previous turn (\"and its size\", \"list files there\"). \
Use the Previous turns block for context when resolving such references.\n\
Examples:\n\
Input: who am i -> {\"cmd\": \"whoami\"}\n\
Input: present directory -> {\"cmd\": \"pwd\"}\n\
Input: what day is it -> {\"cmd\": \"date\"}\n";

#[derive(Serialize)]
struct GenerateReq<'a> {
    model: &'a str,
    prompt: String,
    system: &'a str,
    stream: bool,
    format: &'a str,
    think: bool,
}

#[derive(Deserialize)]
struct GenerateResp {
    response: String,
}

#[derive(Deserialize)]
struct CmdOut {
    cmd: String,
}

pub async fn interpret(user_input: &str, history: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let prompt = if history.is_empty() {
        format!("Current request: {user_input}")
    } else {
        format!("{history}\nCurrent request: {user_input}")
    };

    let req = GenerateReq {
        model: MODEL,
        prompt,
        system: SYSTEM,
        stream: false,
        format: "json",
        think: false,
    };

    let resp: GenerateResp = client
        .post(ENDPOINT)
        .json(&req)
        .send()
        .await
        .context("failed to reach ollama at localhost:11434 — is it running?")?
        .error_for_status()?
        .json()
        .await
        .context("ollama returned non-JSON response")?;

    let parsed: CmdOut = serde_json::from_str(resp.response.trim())
        .with_context(|| format!("model did not return valid JSON: {}", resp.response))?;

    let cmd = parsed.cmd.trim().to_string();
    if cmd.is_empty() {
        return Err(anyhow!("model declined to produce a command"));
    }
    Ok(cmd)
}
