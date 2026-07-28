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
}

impl ParamDecl {
    pub fn key(&self) -> &str {
        match self {
            Self::Float { key, .. }
            | Self::Int { key, .. }
            | Self::Bool { key, .. }
            | Self::Choice { key, .. }
            | Self::Text { key, .. } => key,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Float { label, .. }
            | Self::Int { label, .. }
            | Self::Bool { label, .. }
            | Self::Choice { label, .. }
            | Self::Text { label, .. } => label,
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
