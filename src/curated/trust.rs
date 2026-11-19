//! Trusting code from the server (docs/projects/curated-content.md,
//! *Trust*, step 5).
//!
//! A fractal or an animation from the server may carry Rhai -- a custom
//! variation, an escape formula or colouring, a simulation model, an
//! effect, a script track -- which runs on the GPU (or, a track, on the
//! CPU) as soon as it loads. **Curated code runs without asking**: the
//! client asks the server whether each source's hash is curated
//! (`POST /curated/resolve`, a hash alone). **Anything else asks once**:
//! the app holds the load, and the UI asks whether to run it -- once, for
//! the session, or always (remembered by hash, on this device).
//!
//! What this module holds is the pure part and the question: the app does
//! the request and the holding (`app::trust`), the UI draws the question
//! (`ui::trust_window`).

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use serde_json::Value;

use crate::config::FractalConfig;

/// One source a fractal or an animation carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    /// Its kind as the server names it (`variation`, `formula`, `track`,
    /// ...), `None` when it could not be read.
    pub kind: Option<String>,
    /// Its content hash (`scripted::version_of`).
    pub hash: String,
    /// What a person calls it: its `INFO` title, or its kind.
    pub title: String,
}

fn info_title(source: &str) -> Option<String> {
    // `INFO = #{ title: "..." ... }`: enough for a label, no parse.
    let at = source.find("title")?;
    let rest = &source[at + 5..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string()).filter(|t| !t.is_empty())
}

/// A source's code: its kind, hash and title.
pub fn code_of_source(source: &str) -> Code {
    code_of(source)
}

fn code_of(source: &str) -> Code {
    let kind = crate::storage::library::rhai_kind(source).wire().map(str::to_string);
    let title = info_title(source).unwrap_or_else(|| kind.clone().unwrap_or_else(|| "Rhai".into()));
    Code { kind, hash: crate::scripted::version_of(source), title }
}

/// Every Rhai source a value holds as a carried source (`CustomSource`:
/// `{dialect: "rhai", entry, source}`, wherever it sits -- a flame's,
/// a subflame's, an escape config's, a simulation layer's, an effect's,
/// and the variations each one carries).
fn walk(v: &Value, out: &mut Vec<Code>) {
    match v {
        Value::Object(o) => {
            if o.get("dialect").and_then(Value::as_str) == Some("rhai") && o.get("entry").is_some() {
                if let Some(source) = o.get("source").and_then(Value::as_str) {
                    out.push(code_of(source));
                }
            }
            for child in o.values() {
                walk(child, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|c| walk(c, out)),
        _ => {}
    }
}

fn dedup(mut code: Vec<Code>) -> Vec<Code> {
    let mut seen = HashSet::new();
    code.retain(|c| seen.insert(c.hash.clone()));
    code
}

/// The code a config carries.
pub fn in_config(config: &FractalConfig) -> Vec<Code> {
    let mut out = Vec::new();
    if let Ok(v) = config.to_json_value() {
        walk(&v, &mut out);
    }
    dedup(out)
}

/// The code an animation carries: its script tracks, and its base
/// config's.
pub fn in_animation(animation: &crate::animation::Animation) -> Vec<Code> {
    let mut out: Vec<Code> = animation
        .tracks
        .iter()
        .filter_map(|t| match &t.source {
            crate::animation::TrackSource::Script { source, .. } => Some(code_of(source)),
            _ => None,
        })
        .collect();
    if let Some(base) = &animation.base_config {
        out.extend(in_config(base));
    }
    dedup(out)
}

/// Hashes trusted this session (curated, or the user said Load).
static SESSION: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
/// Hashes trusted always (the user said so), loaded from and saved to
/// this device's storage.
static ALWAYS: LazyLock<Mutex<Option<HashSet<String>>>> = LazyLock::new(Default::default);

const ALWAYS_FILE: &str = "trusted_code.json";

fn always() -> std::sync::MutexGuard<'static, Option<HashSet<String>>> {
    let mut guard = ALWAYS.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        // Tests keep it in memory: the app's data folder is the user's.
        #[cfg(not(test))]
        let read = crate::storage::backend::read_file(std::path::Path::new(ALWAYS_FILE))
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default();
        #[cfg(test)]
        let read: Vec<String> = Vec::new();
        *guard = Some(read.into_iter().collect());
    }
    guard
}

/// Whether code with this hash runs without asking.
pub fn is_trusted(hash: &str) -> bool {
    SESSION.lock().is_ok_and(|s| s.contains(hash)) || always().as_ref().is_some_and(|s| s.contains(hash))
}

/// Trust these hashes for the session: they are curated, or the user
/// said Load.
pub fn trust_for_session<'a>(hashes: impl IntoIterator<Item = &'a str>) {
    if let Ok(mut s) = SESSION.lock() {
        s.extend(hashes.into_iter().map(str::to_string));
    }
}

/// Trust these hashes always, on this device.
pub fn trust_always<'a>(hashes: impl IntoIterator<Item = &'a str>) {
    let mut guard = always();
    let set = guard.get_or_insert_with(HashSet::new);
    set.extend(hashes.into_iter().map(str::to_string));
    if cfg!(test) {
        return;
    }
    let mut list: Vec<&String> = set.iter().collect();
    list.sort();
    if let Ok(json) = serde_json::to_string(&list) {
        if let Err(e) = crate::storage::backend::write_file(std::path::Path::new(ALWAYS_FILE), &json) {
            log::warn!("trusted code not saved: {e}");
        }
    }
}

/// What of `code` is not trusted yet.
pub fn untrusted(code: &[Code]) -> Vec<Code> {
    code.iter().filter(|c| !is_trusted(&c.hash)).cloned().collect()
}

// ------------------------------------------------------------ the question

/// What the app asks the user about.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub id: u64,
    /// What would load: a fractal's or an animation's name.
    pub what: String,
    /// Whose it is, when the server says.
    pub owner: Option<String>,
    /// The code nobody reviewed.
    pub code: Vec<Code>,
}

/// The user's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Load it, and trust this code for the session.
    Load,
    /// Load it, and trust this code from now on, on this device.
    Always,
    /// Don't load it.
    No,
}

static QUESTION: Mutex<Option<Question>> = Mutex::new(None);
static ANSWER: Mutex<Option<(u64, Answer)>> = Mutex::new(None);

/// Ask (the UI shows it until answered); a new question replaces an
/// unanswered one.
pub fn ask(q: Question) {
    if let Ok(mut slot) = QUESTION.lock() {
        *slot = Some(q);
    }
}

/// The question to show, if any.
pub fn question() -> Option<Question> {
    QUESTION.lock().ok()?.clone()
}

/// The user answered question `id`: it is no longer shown, and the
/// trust it grants is recorded.
pub fn answer(id: u64, a: Answer) {
    let Some(q) = QUESTION.lock().ok().and_then(|mut slot| slot.take_if(|q| q.id == id)) else { return };
    let hashes = q.code.iter().map(|c| c.hash.as_str());
    match a {
        Answer::Load => trust_for_session(hashes),
        Answer::Always => trust_always(hashes),
        Answer::No => {}
    }
    if let Ok(mut slot) = ANSWER.lock() {
        *slot = Some((id, a));
    }
}

/// The answer to question `id`, once given.
pub fn take_answer(id: u64) -> Option<Answer> {
    let mut slot = ANSWER.lock().ok()?;
    slot.take_if(|(i, _)| *i == id).map(|(_, a)| a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_says_what_code_it_carries() {
        let mut config = FractalConfig::default();
        assert!(in_config(&config).is_empty());
        let src = "const INFO = #{ title: \"Spiral\", id: \"5b0f8c1e-2d3a-4b5c-8d9e-0f1a2b3c4d5e\" };\nfn start(pixel, p, ctx) { #{ z: pixel } }\nfn step(s, p, ctx) { s }\nfn escaped(s, p, ctx) { false }\n";
        config.escape.custom_formula = Some(crate::config::escape::CustomSource {
            dialect: crate::config::escape::CustomDialect::Rhai,
            entry: "u5b0f8c1e2d3a4b5c8d9e0f1a2b3c4d5e_12345678".into(),
            source: src.into(),
            variations: Vec::new(),
        });
        let code = in_config(&config);
        assert_eq!(code.len(), 1, "{code:?}");
        assert_eq!(code[0].title, "Spiral");
        assert_eq!(code[0].hash, crate::scripted::version_of(src));
        assert_eq!(code[0].kind.as_deref(), Some("formula"));
    }

    #[test]
    fn an_answer_trusts_what_it_was_asked_about() {
        let hash = format!("test-{}", crate::scripted::mint_id());
        let q = Question { id: 7_000_001, what: "x".into(), owner: None, code: vec![Code { kind: None, hash: hash.clone(), title: "t".into() }] };
        assert!(!is_trusted(&hash));
        ask(q.clone());
        assert_eq!(question().map(|q| q.id), Some(q.id));
        answer(q.id, Answer::Load);
        assert!(question().is_none_or(|other| other.id != q.id));
        assert_eq!(take_answer(q.id), Some(Answer::Load));
        assert!(is_trusted(&hash), "Load trusts it for the session");
        assert_eq!(untrusted(&q.code), vec![]);
    }
}
