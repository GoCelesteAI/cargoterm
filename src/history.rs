use std::collections::VecDeque;

const MAX_TURNS: usize = 5;
const MAX_OUTPUT_CHARS: usize = 500;

pub struct Turn {
    pub input: String,
    pub cmd: String,
    pub output: String,
}

pub struct History {
    turns: VecDeque<Turn>,
}

impl History {
    pub fn new() -> Self {
        Self { turns: VecDeque::with_capacity(MAX_TURNS) }
    }

    pub fn push(&mut self, input: &str, cmd: &str, output: &str) {
        if self.turns.len() == MAX_TURNS {
            self.turns.pop_front();
        }
        self.turns.push_back(Turn {
            input: input.to_string(),
            cmd: cmd.to_string(),
            output: truncate(output, MAX_OUTPUT_CHARS),
        });
    }

    pub fn render(&self) -> String {
        if self.turns.is_empty() {
            return String::new();
        }
        let mut out = String::from("Previous turns (most recent last):\n");
        for t in &self.turns {
            out.push_str(&format!(
                "- user: {}\n  cmd: {}\n  output: {}\n",
                one_line(&t.input),
                one_line(&t.cmd),
                one_line(&t.output),
            ));
        }
        out
    }
}

fn truncate(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let head: String = trimmed.chars().take(max).collect();
        format!("{head}…")
    }
}

fn one_line(s: &str) -> String {
    s.replace('\n', " ⏎ ")
}
