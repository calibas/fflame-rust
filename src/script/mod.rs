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
pub mod store;

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

/// Strip inline markdown syntax, leaving the text.
///
/// # Why this is client-side, unlike variations and effects
///
/// Variations and effects carry `description_plain` on the wire: their
/// prose is authored metadata with no client-side source to re-derive
/// from, so the stripped copy has to travel and both consumers agree on
/// one result.
///
/// A script's description is different in kind — it is *derived from the
/// source*, by [`parse_doc`], and the source is authoritative and always
/// present. Storing a stripped copy server-side would be a derivation of
/// a derivation: a third representation of the same bytes, able to go
/// stale against a source the client re-reads on every load anyway.
///
/// # What it does and does not touch
///
/// **Inline only.** Block structure — `# Heading`, indented table
/// blocks, list markers — is left exactly as it is, because the Scripts
/// panel already understands those and renders them structurally. A
/// stripper that also ate `# ` would silently disable the panel's
/// heading detection.
///
/// Indented lines are skipped **entirely**, not merely preserved: they
/// are code blocks, so their contents are literal. Agreeing with the
/// renderer there is not pedantry — `lsystem.rhai` documents the turtle
/// roll symbols in an indented table with a doubled backslash, which
/// reads as an escape sequence to anything that strips inline syntax.
///
/// So: code spans, links, images, and `*`/`_` emphasis.
///
/// # The underscore rule earns its keep
///
/// This codebase's prose is full of `snake_case` — `basic_random`,
/// `run_script`, `lsystem_plant`. Naive `_`-emphasis stripping turns
/// "a `run_script` call from basic_random" into mangled text, and would
/// do it to the very scripts that ship. So `_` only opens a span when
/// the character before it is not alphanumeric, and only closes when the
/// character after it is not — CommonMark's intraword rule, and the
/// reason `*` and `_` cannot share a code path.
pub fn strip_markdown(text: &str) -> String {
    // Line by line: a delimiter never pairs across a line break, and
    // keeping the split means block structure survives untouched.
    text.lines()
        .map(|line| {
            // An indented line is a code block — literal in markdown, and
            // already rendered verbatim as a monospace table by the
            // Scripts panel. The stripper has to agree with the renderer,
            // or the panel shows one thing and the description another.
            // `lsystem.rhai`'s symbol table is the live case: it
            // documents the turtle roll symbols with a doubled
            // backslash, which reads as an escape sequence anywhere else.
            if line.starts_with(char::is_whitespace) {
                line.to_string()
            } else {
                strip_markdown_line(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Length of the run of `ch` starting at `i`.
fn md_run_len(b: &[char], i: usize, ch: char) -> usize {
    let mut n = 0;
    while i + n < b.len() && b[i + n] == ch {
        n += 1;
    }
    n
}

/// A markdown punctuation character a lone `\` may escape.
///
/// `\` itself is absent because a run of two or more is handled as
/// content before this is consulted — see the `'\\'` arm of
/// [`strip_markdown_line`] for why.
fn md_escapable(c: char) -> bool {
    "`*_{}[]()#+-.!>~|".contains(c)
}

/// Find the closing run for an emphasis span opening at `i`.
///
/// Returns the closer's start index. Requires the same run length, so
/// `*a**` does not pair — and applies the flanking rules that keep
/// arithmetic (`2 * 3`) and identifiers (`snake_case`) intact.
fn md_find_emphasis_close(b: &[char], i: usize, ch: char, n: usize) -> Option<usize> {
    // Left-flanking: the run must be followed by non-whitespace.
    if b.get(i + n).is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    // Intraword `_` does not open.
    if ch == '_' && i > 0 && b[i - 1].is_alphanumeric() {
        return None;
    }

    let mut j = i + n;
    while j < b.len() {
        if b[j] != ch {
            j += 1;
            continue;
        }
        let m = md_run_len(b, j, ch);
        // Right-flanking: preceded by non-whitespace, and non-empty span.
        let closes = m == n
            && j > i + n
            && !b[j - 1].is_whitespace()
            && (ch != '_' || b.get(j + m).is_none_or(|c| !c.is_alphanumeric()));
        if closes {
            return Some(j);
        }
        j += m;
    }
    None
}

/// `[text](url)` / `![alt](url)` — returns `(text range, index after)`.
fn md_parse_link(b: &[char], open: usize) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut j = open;
    while j < b.len() {
        match b[j] {
            '\\' => j += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    if j >= b.len() || b[j] != ']' || b.get(j + 1) != Some(&'(') {
        return None;
    }
    // Scan the destination to its closing paren.
    let mut k = j + 2;
    let mut pdepth = 1usize;
    while k < b.len() && pdepth > 0 {
        match b[k] {
            '\\' => k += 1,
            '(' => pdepth += 1,
            ')' => pdepth -= 1,
            _ => {}
        }
        k += 1;
    }
    if pdepth != 0 {
        return None;
    }
    Some((j, k))
}

fn strip_markdown_line(line: &str) -> String {
    let b: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;

    while i < b.len() {
        match b[i] {
            // A RUN of backslashes is content, not escaping.
            //
            // The deviation from CommonMark lives here, and it has to be
            // decided on the run rather than per character: taking
            // `\\+` one at a time, the first backslash is literal and
            // the second escapes the `+`, so one backslash silently
            // disappears — which is what `X=F[\\+X][/-X]` in
            // `lsystem_plant.rhai` is. Deciding on the whole run makes
            // `\\ /` and `\\+X` behave the same way, which is the only
            // version a reader can predict.
            '\\' => {
                let n = md_run_len(&b, i, '\\');
                if n >= 2 {
                    out.extend(&b[i..i + n]);
                    i += n;
                } else if b.get(i + 1).is_some_and(|c| md_escapable(*c)) {
                    out.push(b[i + 1]);
                    i += 2;
                } else {
                    out.push('\\');
                    i += 1;
                }
            }
            // A code span's contents are literal — no further stripping.
            '`' => {
                let n = md_run_len(&b, i, '`');
                let mut j = i + n;
                let close = loop {
                    if j >= b.len() {
                        break None;
                    }
                    if b[j] == '`' && md_run_len(&b, j, '`') == n {
                        break Some(j);
                    }
                    j += 1;
                };
                match close {
                    Some(j) => {
                        out.extend(&b[i + n..j]);
                        i = j + n;
                    }
                    None => {
                        out.push('`');
                        i += 1;
                    }
                }
            }
            // Images strip to their alt text, same as a link's label.
            '!' if b.get(i + 1) == Some(&'[') => {
                match md_parse_link(&b, i + 1) {
                    Some((text_end, after)) => {
                        out.push_str(&strip_markdown_line(
                            &b[i + 2..text_end].iter().collect::<String>(),
                        ));
                        i = after;
                    }
                    None => {
                        out.push('!');
                        i += 1;
                    }
                }
            }
            '[' => match md_parse_link(&b, i) {
                Some((text_end, after)) => {
                    out.push_str(&strip_markdown_line(
                        &b[i + 1..text_end].iter().collect::<String>(),
                    ));
                    i = after;
                }
                None => {
                    out.push('[');
                    i += 1;
                }
            },
            ch @ ('*' | '_') => {
                let n = md_run_len(&b, i, ch);
                // Runs of 3+ are rare and ambiguous; passing one through
                // beats mangling it.
                let close = if n <= 2 {
                    md_find_emphasis_close(&b, i, ch, n)
                } else {
                    None
                };
                match close {
                    Some(j) => {
                        // Recurse so nesting works: `**bold *and* more**`.
                        out.push_str(&strip_markdown_line(
                            &b[i + n..j].iter().collect::<String>(),
                        ));
                        i = j + n;
                    }
                    None => {
                        out.extend(&b[i..i + n]);
                        i += n;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
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
    /// The script generates a palette. Lets a panel other than the
    /// Scripts panel — the Palette Editor — offer it, which is how
    /// palette generation reaches the place people look for it without
    /// a second implementation behind a Rust generator.
    pub palette: bool,
}

impl ScriptFlags {
    /// Every flag name this build understands, for error messages.
    pub const KNOWN: &'static [&'static str] = &["norng", "palette"];

    /// Apply one flag by name.
    pub fn set(&mut self, name: &str) -> Result<(), String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "norng" => self.no_rng = true,
            "palette" => self.palette = true,
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
