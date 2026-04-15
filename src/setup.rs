use anyhow::Result;
use serde::Deserialize;
use std::env;
use std::io::{self, Write};
use std::process::Command;
use std::time::Duration;

use crate::config::Config;

#[derive(Deserialize)]
struct TagsResp {
    models: Vec<TagEntry>,
}

#[derive(Deserialize)]
struct TagEntry {
    name: String,
}

pub async fn run(cfg: &Config, cfg_path: Option<&std::path::Path>) -> Result<()> {
    println!("cargoterm setup\n");

    match cfg_path {
        Some(p) if p.exists() => println!("  config:  {}", p.display()),
        Some(p) => println!("  config:  {} (not present — defaults in use)", p.display()),
        None => println!("  config:  <no config path available>"),
    }
    println!("  host:    {}", cfg.ollama.host);
    println!("  model:   {}\n", cfg.ollama.model);

    let step1 = check_ollama_binary();
    report("Ollama binary on PATH", &step1);

    let step2 = check_endpoint_reachable(&cfg.ollama.host).await;
    report("Ollama daemon reachable", &step2);

    if step2.is_err() {
        println!("\nStart Ollama first. On macOS:");
        println!("  brew install ollama     # if not installed");
        println!("  ollama serve &          # or launch the Ollama app");
        println!("Then re-run `cargoterm --setup`.");
        return Ok(());
    }

    let have_model = check_model_installed(&cfg.ollama.host, &cfg.ollama.model).await?;
    report(
        &format!("Default model `{}` installed", cfg.ollama.model),
        &if have_model { Ok(()) } else { Err(anyhow::anyhow!("missing")) },
    );

    if !have_model {
        if prompt_yes_no(&format!(
            "\nPull `{}` now? (large download) [Y/n] ",
            cfg.ollama.model
        )) {
            pull_model(&cfg.ollama.model)?;
        } else {
            println!("Skipped. cargoterm will fail on natural-language input until the model is pulled.");
            return Ok(());
        }
    }

    println!("\nAll set. Run `cargoterm` to start the REPL.");
    Ok(())
}

fn check_ollama_binary() -> Result<()> {
    let Some(path) = env::var_os("PATH") else {
        return Err(anyhow::anyhow!("PATH not set"));
    };
    let found = env::split_paths(&path).any(|p| p.join("ollama").is_file());
    if found {
        Ok(())
    } else {
        Err(anyhow::anyhow!("`ollama` not found on PATH — install from https://ollama.com"))
    }
}

async fn check_endpoint_reachable(host: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let url = format!("{}/api/tags", host.trim_end_matches('/'));
    client.get(&url).send().await?.error_for_status()?;
    Ok(())
}

async fn check_model_installed(host: &str, model: &str) -> Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let url = format!("{}/api/tags", host.trim_end_matches('/'));
    let tags: TagsResp = client.get(&url).send().await?.error_for_status()?.json().await?;
    Ok(tags.models.iter().any(|m| m.name == model))
}

fn pull_model(model: &str) -> Result<()> {
    println!();
    let status = Command::new("ollama").arg("pull").arg(model).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("ollama pull exited with {status}"));
    }
    Ok(())
}

fn report(label: &str, result: &Result<()>) {
    match result {
        Ok(()) => println!("  [ok]    {label}"),
        Err(e) => println!("  [fail]  {label}: {e}"),
    }
}

fn prompt_yes_no(q: &str) -> bool {
    print!("{q}");
    let _ = io::stdout().flush();
    let mut buf = String::new();
    if io::stdin().read_line(&mut buf).is_err() {
        return false;
    }
    let s = buf.trim().to_lowercase();
    s.is_empty() || s == "y" || s == "yes"
}
