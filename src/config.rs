use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub safety: SafetyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub host: String,
    pub model: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub deny: Vec<String>,
    pub allow: Vec<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: "http://localhost:11434".to_string(),
            model: "qwen3:14b".to_string(),
            timeout_secs: 60,
        }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            deny: default_deny(),
            allow: default_allow(),
        }
    }
}

fn default_deny() -> Vec<String> {
    [
        "rm", "sudo", "mkfs", "dd", "shutdown", "reboot", ":(){", "chmod",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

fn default_allow() -> Vec<String> {
    [
        "pwd", "whoami", "hostname", "uname", "date", "uptime", "id", "groups", "tty", "arch",
        "ls", "cat", "head", "tail", "wc", "file", "stat", "readlink", "basename", "dirname", "df",
        "du", "echo", "printf", "which", "type", "env", "printenv", "history",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

pub fn default_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("cargoterm").join("config.toml"));
    }
    env::var_os("HOME").map(|h| {
        PathBuf::from(h)
            .join(".config")
            .join("cargoterm")
            .join("config.toml")
    })
}

pub fn load(path_override: Option<&Path>) -> Result<(Config, Option<PathBuf>)> {
    let path = match path_override {
        Some(p) => Some(p.to_path_buf()),
        None => default_path(),
    };

    let Some(p) = path else {
        return Ok((Config::default(), None));
    };

    if !p.exists() {
        return Ok((Config::default(), Some(p)));
    }

    let text =
        fs::read_to_string(&p).with_context(|| format!("reading config file {}", p.display()))?;
    let cfg: Config =
        toml::from_str(&text).with_context(|| format!("parsing config file {}", p.display()))?;
    Ok((cfg, Some(p)))
}

pub fn render(cfg: &Config) -> String {
    toml::to_string_pretty(cfg).unwrap_or_else(|e| format!("<serialization error: {e}>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_sane_model() {
        let cfg = Config::default();
        assert_eq!(cfg.ollama.model, "qwen3:14b");
        assert!(cfg.ollama.timeout_secs > 0);
    }

    #[test]
    fn defaults_include_rm_in_deny() {
        let cfg = Config::default();
        assert!(cfg.safety.deny.iter().any(|s| s == "rm"));
    }

    #[test]
    fn defaults_include_pwd_in_allow() {
        let cfg = Config::default();
        assert!(cfg.safety.allow.iter().any(|s| s == "pwd"));
    }

    #[test]
    fn round_trip_serializes() {
        let cfg = Config::default();
        let s = render(&cfg);
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.ollama.model, cfg.ollama.model);
        assert_eq!(back.safety.deny.len(), cfg.safety.deny.len());
    }

    #[test]
    fn partial_config_keeps_defaults() {
        let s = "[ollama]\nhost = \"http://localhost:11434\"\nmodel = \"llama3:8b\"\ntimeout_secs = 30\n";
        let cfg: Config = toml::from_str(s).unwrap();
        assert_eq!(cfg.ollama.model, "llama3:8b");
        assert!(cfg.safety.deny.iter().any(|x| x == "rm"));
    }
}
