//! The escape-time formula language: Ultra Fractal's, which accepts
//! Fractint's shape, compiled to WGSL.
//!
//! The plan, the language reference this follows and the order of work
//! are in `docs/projects/escape-formula-language.md`. In brief: a file
//! holds entries (`Name { ... }`), an entry holds sections (`init:`,
//! `loop:`, `bailout:`, `final:`, `default:` ...), and an entry is
//! either a fractal FORMULA (`.ufm`, `.frm`) or a COLOURING (`.ucl`).
//! The front end (lexer, parser, checker) turns an entry into a typed
//! program; the back ends evaluate it on the CPU or emit WGSL.

pub mod ast;
pub mod bigeval;
pub mod builtins;
pub mod check;
pub mod delta;
pub mod derive;
pub mod hybrid;
pub mod interp;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod perturb;
pub mod rhai;
pub mod to_rhai;
pub mod switch;
pub mod wgsl;

/// Which language a file is written in. Ultra Fractal's is the native
/// one; Fractint's differs in a list of numeric conventions (phase 2 of
/// the plan), not in its grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dialect {
    UltraFractal,
    Fractint,
}

/// What an entry is: a fractal formula or a colouring algorithm. The
/// file extension says which (`.ufm` / `.frm` formulas, `.ucl`
/// colourings); the sections must agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Formula,
    Coloring,
}

/// A byte range in the source text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The smallest span covering both.
    pub fn to(self, other: Span) -> Span {
        Span { start: self.start.min(other.start), end: self.end.max(other.end) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A compiler message, anchored to the source.
#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub severity: Severity,
    pub span: Span,
    pub message: String,
}

impl Diag {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Diag { severity: Severity::Error, span, message: message.into() }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Diag { severity: Severity::Warning, span, message: message.into() }
    }

    /// `line:col: message`, 1-based, for the editor and for tests.
    pub fn render(&self, source: &str) -> String {
        let (line, col) = line_col(source, self.span.start);
        let kind = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        format!("{line}:{col}: {kind}: {}", self.message)
    }
}

/// 1-based line and column (in characters) of a byte offset.
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |s| s.chars().count()) + 1;
    (line, col)
}

/// Whether any message is an error.
pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}
