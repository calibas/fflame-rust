//! Sandboxed flame scripting (Rhai).
//!
//! User-written scripts that generate or modify flames, safe to share:
//! a script can only call functions we register here, and runs under
//! hard execution budgets. See
//! [docs/projects/flame-scripting.md](../../docs/projects/flame-scripting.md).
//!
//! Two kinds, declared by the script itself:
//!
//! * **Generator** — builds a flame from a default config.
//! * **Modifier** — transforms a *copy* of the current config.
//!
//! Both are seeded from a pinned PRNG algorithm, so script + seed
//! reproduces a flame byte-for-byte on desktop, web, and Python.

pub mod anim;
pub mod color;
pub mod api;
pub mod builtins;
/// Headless `generate` command (desktop only — needs the filesystem).
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
pub mod host;
pub mod library;

#[cfg(test)]
mod tests;

pub use host::{ScriptHost, ScriptOutcome};

/// What a script does, declared by its `script(name, kind)` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptKind {
    /// Builds a flame from scratch (starts from a default config).
    Generator,
    /// Transforms an existing flame (starts from the current config).
    Modifier,
}

impl ScriptKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "generator" | "generate" | "gen" => Some(Self::Generator),
            "modifier" | "modify" | "mod" => Some(Self::Modifier),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Generator => "generator",
            Self::Modifier => "modifier",
        }
    }
}

/// A parameter the script declares for the UI to render.
///
/// Collected by running the script in [`host::Mode::Collect`] before the
/// real run — see the two-phase design in the project doc.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamDecl {
    Float { key: String, label: String, default: f64, min: f64, max: f64 },
    Int { key: String, label: String, default: i64, min: i64, max: i64 },
    Bool { key: String, label: String, default: bool },
    Choice { key: String, label: String, options: Vec<String>, default: usize },
    /// Free text — L-system rules, names, anything the other kinds can't
    /// express. `max_len` keeps a pasted novel out of the flame.
    Text { key: String, label: String, default: String, max_len: usize },
    /// A colour, rendered with the same picker the rest of the app uses.
    Color { key: String, label: String, default: [f32; 3] },
}

impl ParamDecl {
    pub fn key(&self) -> &str {
        match self {
            Self::Float { key, .. }
            | Self::Int { key, .. }
            | Self::Bool { key, .. }
            | Self::Choice { key, .. }
            | Self::Text { key, .. }
            | Self::Color { key, .. } => key,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Float { label, .. }
            | Self::Int { label, .. }
            | Self::Bool { label, .. }
            | Self::Choice { label, .. }
            | Self::Text { label, .. }
            | Self::Color { label, .. } => label,
        }
    }
}

/// A value supplied for a declared parameter (from the UI or `--set`).
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    Choice(usize),
    Text(String),
    Color([f32; 3]),
}

/// Turn `trace_a` into `Trace A` for UI labels.
pub(crate) fn humanize(key: &str) -> String {
    key.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Script metadata gathered by the collect pass.
#[derive(Debug, Clone, Default)]
pub struct ScriptMeta {
    pub name: String,
    /// `None` when the script never called `script(...)`.
    pub kind: Option<ScriptKind>,
    pub params: Vec<ParamDecl>,
    /// Optional switches from `script(name, kind, [...])`.
    pub flags: ScriptFlags,
}

/// A script's own documentation: the comment block at the top of the
/// file, read as prose.
///
/// The same convention the variation definitions use — a doc block
/// above the thing it describes, plain prose with optional `# Heading`
/// sections. Read from the SOURCE rather than from a `description(...)`
/// call, which means it costs authors nothing (every shipped script
/// already opens with one), needs no new syntax, and still shows for a
/// script that fails to compile — exactly when a reader most wants to
/// know what it was meant to do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptDoc {
    /// The title line these blocks conventionally open with, if there
    /// is one. Dropped from the prose rather than shown: it repeats the
    /// script's name, which the picker is already displaying.
    pub title: String,
    /// The first real paragraph — a line or two of orientation, worth
    /// showing without being asked for.
    pub summary: String,
    /// Everything after it. Often long: `lsystem.rhai` carries a symbol
    /// table and a list of rules to try, so the panel keeps this behind
    /// a disclosure.
    pub body: String,
}

impl ScriptDoc {
    pub fn is_empty(&self) -> bool {
        self.summary.is_empty() && self.body.is_empty()
    }
}

/// Whether a body line reads as a section heading.
///
/// Two conventions, both already in use: `# Heading`, as the variation
/// doc blocks are written, and a bare capitalised line like
/// `HOW IT WORKS`, which is what the shipped scripts actually use.
/// Indented lines are never headings — that is table content.
pub fn doc_line_is_heading(line: &str) -> bool {
    if line.starts_with("# ") {
        return true;
    }
    if line.starts_with(char::is_whitespace) || line.trim().len() < 3 {
        return false;
    }
    // A heading may carry a parenthetical aside in ordinary case —
    // `SYMBOLS  (Prusinkiewicz & Lindenmayer, ...)` — so judge the part
    // before it.
    let head = line.split('(').next().unwrap_or(line).trim();
    let letters: Vec<char> = head.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 3 && letters.iter().all(|c| c.is_uppercase())
}

/// Read the leading comment block of a script as its documentation.
///
/// Takes the run of `//` lines at the top of the file, stopping at the
/// first line of actual code, so a comment sitting *inside* the script
/// is never mistaken for its description.
pub fn parse_doc(source: &str) -> ScriptDoc {
    let mut lines: Vec<String> = Vec::new();
    for raw in source.lines() {
        let trimmed = raw.trim_start();
        if trimmed.is_empty() {
            // A blank line before any comment is just leading space; one
            // between comment lines is a paragraph break.
            if !lines.is_empty() {
                lines.push(String::new());
            }
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("//") else {
            break; // First real statement — the header is over.
        };
        // Accept `///` too, so a block pasted from Rust reads the same.
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        lines.push(rest.strip_prefix(' ').unwrap_or(rest).trim_end().to_string());
    }

    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    // Split into paragraphs, so a leading title line can be recognised.
    let para_end = |from: usize| -> usize {
        lines[from..]
            .iter()
            .position(|l| l.is_empty())
            .map(|i| from + i)
            .unwrap_or(lines.len())
    };

    let mut at = 0;
    let first_end = para_end(at);

    // These blocks open with the script's name on its own line. Showing
    // that as the description would just repeat the picker's label, so
    // take it as a title and let the NEXT paragraph be the summary. The
    // tell is a lone line with no sentence-ending punctuation.
    let mut title = String::new();
    if first_end == at + 1 && first_end < lines.len() {
        let candidate = &lines[at];
        if !candidate.ends_with(['.', '!', '?', ':', ',']) && !doc_line_is_heading(candidate) {
            title = candidate.clone();
            at = first_end + 1;
        }
    }

    let summary_end = para_end(at);
    let summary = lines[at..summary_end].join(" ").trim().to_string();
    let body = lines
        .get(summary_end + 1..)
        .unwrap_or(&[])
        .join("\n")
        .trim_end()
        .to_string();

    ScriptDoc { title, summary, body }
}

/// Optional switches a script may declare, as
/// `script("Turntable", "modifier", ["norng"])`.
///
/// Deliberately a small set of named booleans rather than a free-form
/// bag: a flag exists to change what the UI does, so each one has to be
/// understood by the panel anyway. Adding one is a field here, a match
/// arm below, and its use — and a script asking for a flag this build
/// doesn't know is an error naming the ones it does, because a silently
/// ignored switch looks like the feature is broken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptFlags {
    /// The script ignores the seed, so the panel hides the seed field,
    /// Reroll, and the batch controls — all three are ways of asking for
    /// a different random result, which this script has none of.
    pub no_rng: bool,
}

impl ScriptFlags {
    /// Every flag name this build understands, for error messages.
    pub const KNOWN: &'static [&'static str] = &["norng"];

    /// Apply one flag by name.
    pub fn set(&mut self, name: &str) -> Result<(), String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "norng" => self.no_rng = true,
            other => {
                return Err(format!(
                    "unknown script flag `{other}` — this build knows: {}",
                    Self::KNOWN.join(", ")
                ))
            }
        }
        Ok(())
    }
}

/// A script failure, carrying source position when Rhai knows it.
///
/// Position matters more than usual here: the audience is people with
/// little coding experience, so "line 7: unknown variation" is the
/// difference between a fixable mistake and a dead end.
#[derive(Debug, Clone)]
pub struct ScriptError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl ScriptError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), line: None, column: None }
    }
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "line {l}:{c}: {}", self.message),
            (Some(l), None) => write!(f, "line {l}: {}", self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ScriptError {}
