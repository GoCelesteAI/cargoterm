use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Entry {
    pub input: String,
    pub cmd: String,
    pub explain: Option<String>,
    pub origin: Origin,
    pub output: String,
}

pub enum Origin {
    Builtin,
    Direct,
    Auto,
    Confirmed,
}

pub struct Transcript {
    entries: Vec<Entry>,
    started_epoch: u64,
}

impl Transcript {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            started_epoch: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# cargoterm session\n\n");
        out.push_str(&format!(
            "Started at unix epoch {} (UTC).\n\n",
            self.started_epoch
        ));
        for (i, e) in self.entries.iter().enumerate() {
            out.push_str(&format!("## turn {}\n\n", i + 1));
            out.push_str(&format!("**input:** `{}`\n\n", fence_safe(&e.input)));
            let tag = match e.origin {
                Origin::Builtin => "builtin",
                Origin::Direct => "direct",
                Origin::Auto => "auto",
                Origin::Confirmed => "confirmed",
            };
            out.push_str(&format!(
                "**executed:** `{}` _({tag})_\n\n",
                fence_safe(&e.cmd)
            ));
            if let Some(exp) = &e.explain
                && !exp.is_empty()
            {
                out.push_str(&format!("**what this does:** {exp}\n\n"));
            }
            out.push_str("```\n");
            out.push_str(e.output.trim_end_matches('\n'));
            if !e.output.is_empty() && !e.output.ends_with('\n') {
                out.push('\n');
            } else if e.output.is_empty() {
                out.push_str("(no output)\n");
            } else {
                out.push('\n');
            }
            out.push_str("```\n\n");
        }
        out
    }

    pub fn save(&self, path: &Path) -> Result<PathBuf> {
        let final_path = resolve_unique(path);
        if let Some(parent) = final_path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(&final_path, self.render_markdown())
            .with_context(|| format!("writing {}", final_path.display()))?;
        Ok(final_path)
    }

    pub fn default_filename(&self) -> String {
        format!("cargoterm-session-{}.md", self.started_epoch)
    }
}

fn fence_safe(s: &str) -> String {
    s.replace('`', "'")
}

fn resolve_unique(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("session");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("md");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for i in 2..1000 {
        let candidate = parent.join(format!("{stem}-{i}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(input: &str, cmd: &str, output: &str) -> Entry {
        Entry {
            input: input.to_string(),
            cmd: cmd.to_string(),
            explain: None,
            origin: Origin::Direct,
            output: output.to_string(),
        }
    }

    #[test]
    fn empty_transcript_reports_empty() {
        let t = Transcript::new();
        assert!(t.is_empty());
    }

    #[test]
    fn renders_header_and_turns() {
        let mut t = Transcript::new();
        t.push(entry("pwd", "pwd", "/tmp\n"));
        t.push(entry("whoami", "whoami", "alice\n"));
        let md = t.render_markdown();
        assert!(md.contains("# cargoterm session"));
        assert!(md.contains("## turn 1"));
        assert!(md.contains("## turn 2"));
        assert!(md.contains("/tmp"));
        assert!(md.contains("alice"));
    }

    #[test]
    fn save_writes_file_and_returns_path() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("out.md");
        let mut t = Transcript::new();
        t.push(entry("pwd", "pwd", "/tmp\n"));
        let written = t.save(&target).unwrap();
        assert_eq!(written, target);
        assert!(written.exists());
        let body = fs::read_to_string(&written).unwrap();
        assert!(body.contains("/tmp"));
    }

    #[test]
    fn save_picks_unique_path_when_exists() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("out.md");
        fs::write(&target, "existing").unwrap();
        let mut t = Transcript::new();
        t.push(entry("pwd", "pwd", "/tmp\n"));
        let written = t.save(&target).unwrap();
        assert_ne!(written, target);
        assert!(
            written
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("-2")
        );
    }

    #[test]
    fn fence_safe_replaces_backticks() {
        assert_eq!(fence_safe("echo `date`"), "echo 'date'");
    }

    #[test]
    fn empty_output_renders_marker() {
        let mut t = Transcript::new();
        t.push(entry("cd /tmp", "cd /tmp", ""));
        let md = t.render_markdown();
        assert!(md.contains("(no output)"));
    }
}
