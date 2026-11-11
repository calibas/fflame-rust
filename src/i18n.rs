//! Translations: `t!` and the locale.
//!
//! The strings live in `locales/*.yml` (one file a locale, nested maps of
//! double-quoted strings); `build.rs` (`generate_locale_table`) reads them
//! into a sorted table compiled into the binary, so nothing parses YAML at
//! run time. This replaced the `rust-i18n` crate, which brought 74 crates
//! into the app -- a regex engine among them -- to do the same lookups
//! (docs/projects/size-study.md §5); `t!` kept its syntax and meaning:
//!
//! ```rust,ignore
//! t!("app.title")                          // the current locale's text
//! t!("api.save_success", name = name)      // `%{name}` filled, by Display
//! t!(key, locale = "en")                   // a given locale
//! ```
//!
//! A key is looked up in the locale, then in each shorter form of it
//! (`zh-CN` -> `zh`), then in English; a key found nowhere comes back as
//! itself, its `%{...}` still filled. `t!` returns `Cow<'static, str>`:
//! borrowed from the table unless an argument was filled.
//!
//! # Supported languages
//!
//! English (en, the fallback), Spanish (es), Japanese (ja), Chinese
//! Simplified (zh-CN). A language is added by a `locales/<code>.yml` and a
//! row in [`supported_locales`].

use std::borrow::Cow;
use std::fmt::Display;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

// `LOCALES: &[(&str, &[(&str, &str)])]`, each locale's keys sorted, and
// `FALLBACK`, English's index in it.
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// The current locale's name, as set (empty until set: English).
static NAME: RwLock<String> = RwLock::new(String::new());
/// The current locale's lookup order: up to four indices into `LOCALES`,
/// one a byte, ended by `END`. Read by every `t!`, so not behind the lock.
static CHAIN: AtomicU32 = AtomicU32::new(u32::MAX);
const END: u8 = u8::MAX;

/// The tables to look in for `locale`, in order, packed as in `CHAIN`.
fn chain_of(locale: &str) -> u32 {
    let mut chain = [END; 4];
    let mut n = 0;
    let mut push = |i: usize| {
        let i = i as u8;
        if n < 4 && !chain[..n].contains(&i) {
            chain[n] = i;
            n += 1;
        }
    };
    let mut name = locale;
    loop {
        if let Some(i) = LOCALES.iter().position(|(code, _)| *code == name) {
            push(i);
        }
        // rust-i18n's rule, kept: `zh-CN` falls back to `zh`.
        match name.rfind('-') {
            Some(at) => name = name[..at].trim_end_matches("-x"),
            None => break,
        }
    }
    push(FALLBACK);
    u32::from_le_bytes(chain)
}

fn current_chain() -> u32 {
    match CHAIN.load(Ordering::Relaxed) {
        u32::MAX => chain_of("en"),
        chain => chain,
    }
}

/// The text for `key` in the locales `chain` names, if any has it.
fn find(key: &str, chain: u32) -> Option<&'static str> {
    chain.to_le_bytes().into_iter().take_while(|&i| i != END).find_map(|i| {
        let table = LOCALES[i as usize].1;
        table.binary_search_by(|(k, _)| (*k).cmp(key)).ok().map(|at| table[at].1)
    })
}

/// What `t!` expands to. `args` fill `%{name}`; none, and the text is
/// returned as written.
#[doc(hidden)]
pub fn translate(key: &str, locale: Option<&str>, args: &[(&str, &dyn Display)]) -> Cow<'static, str> {
    let chain = locale.map_or_else(current_chain, chain_of);
    match (find(key, chain), args.is_empty()) {
        (Some(text), true) => Cow::Borrowed(text),
        (Some(text), false) => Cow::Owned(fill(text, args)),
        (None, true) => Cow::Owned(key.to_string()),
        (None, false) => Cow::Owned(fill(key, args)),
    }
}

/// `text` with each `%{name}` among `args` replaced by its value; a
/// `%{name}` not among them is left as written.
fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut rest = text;
    while let Some(at) = rest.find("%{") {
        let Some(len) = rest[at + 2..].find('}') else { break };
        let name = &rest[at + 2..at + 2 + len];
        out.push_str(&rest[..at]);
        match args.iter().find(|(n, _)| *n == name) {
            Some((_, value)) => {
                use std::fmt::Write;
                let _ = write!(out, "{value}");
            }
            None => out.push_str(&rest[at..at + 3 + len]),
        }
        rest = &rest[at + 3 + len..];
    }
    out.push_str(rest);
    out
}

/// The current locale ("en" until one is set).
pub fn current_locale() -> String {
    let name = NAME.read().unwrap_or_else(|e| e.into_inner());
    if name.is_empty() { "en".to_string() } else { name.clone() }
}

/// Set the locale (e.g. "en", "es", "ja", "zh-CN"). One without a table
/// is kept as named and reads its fallbacks.
pub fn set_locale(locale: &str) {
    let mut name = NAME.write().unwrap_or_else(|e| e.into_inner());
    *name = locale.to_string();
    CHAIN.store(chain_of(locale), Ordering::Relaxed);
}

/// Every locale with a table, and its keys: for tests and tools.
pub fn tables() -> &'static [(&'static str, &'static [(&'static str, &'static str)])] {
    LOCALES
}


/// Locale information for UI display
#[derive(Debug, Clone, PartialEq)]
pub struct LocaleInfo {
    pub code: &'static str,
    pub name: &'static str,
    pub native_name: &'static str,
    pub icon: &'static str, // Language code or emoji for display
}

impl LocaleInfo {
    pub const fn new(code: &'static str, name: &'static str, native_name: &'static str, icon: &'static str) -> Self {
        Self { code, name, native_name, icon }
    }

    /// Get display text that works without CJK fonts loaded
    /// Format: "🌐 EN English" - icon and code are ASCII, name is readable
    pub fn display_text(&self) -> String {
        format!("{} {} {}", self.icon, self.code.to_uppercase(), self.name)
    }
}

/// Get list of supported locales with display names
pub fn supported_locales() -> Vec<LocaleInfo> {
    vec![
        LocaleInfo::new("en", "English", "English", "🌐"),
        LocaleInfo::new("es", "Spanish", "Español", "🌐"),
        LocaleInfo::new("ja", "Japanese", "日本語", "🌐"),
        LocaleInfo::new("zh-CN", "Chinese (Simplified)", "简体中文", "🌐"),
        // Add more as translation files are created:
        // LocaleInfo::new("zh-TW", "Chinese (Traditional)", "繁體中文", "🌐"),
        // LocaleInfo::new("ko", "Korean", "한국어", "🌐"),
        // LocaleInfo::new("fr", "French", "Français", "🌐"),
        // LocaleInfo::new("de", "German", "Deutsch", "🌐"),
        // LocaleInfo::new("ru", "Russian", "Русский", "🌐"),
        // LocaleInfo::new("ar", "Arabic", "العربية", "🌐"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_locale() {
        let locale = current_locale();
        assert_eq!(locale, "en");
    }

    #[test]
    fn test_supported_locales() {
        let locales = supported_locales();
        assert!(!locales.is_empty());
        assert_eq!(locales[0].code, "en");
    }

    /// Every literal key the code asks for has English text. A missing
    /// one is drawn as the key itself -- the undo history read
    /// "history.param.sim_terrain_cam_yaw" for a hundred-odd settings
    /// added without a label. Scans the sources for `t!("...")` and the
    /// history's `I18nKey::simple` / `with_params` keys; English is the
    /// fallback every other locale ends on, so it is the one that must be
    /// whole.
    #[test]
    fn every_key_the_code_uses_has_english_text() {
        fn keys_in(text: &str, out: &mut Vec<String>) {
            // And every history action description, which is passed as a
            // plain string and looked up when the history is drawn.
            let mut from = 0;
            while let Some(at) = text[from..].find("\"history.action.") {
                let start = from + at + 1;
                let end = text[start..].find('"').map_or(start, |e| start + e);
                from = end.max(start + 1);
                let line_start = text[..start].rfind('\n').map_or(0, |p| p + 1);
                if text[line_start..start].trim_start().starts_with("//") {
                    continue;
                }
                let key = &text[start..end];
                // (Not this scan's own needle, which ends at the dot.)
                if !key.ends_with('.') && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                    out.push(key.to_string());
                }
            }
            for marker in ["t!(", "I18nKey::simple(", "I18nKey::with_params("] {
                let mut from = 0;
                while let Some(at) = text[from..].find(marker) {
                    let start = from + at;
                    from = start + marker.len();
                    // Not in a comment.
                    let line_start = text[..start].rfind('\n').map_or(0, |p| p + 1);
                    if text[line_start..start].trim_start().starts_with("//") {
                        continue;
                    }
                    let rest = text[from..].trim_start();
                    let Some(rest) = rest.strip_prefix('"') else { continue };
                    let Some(end) = rest.find('"') else { continue };
                    let key = &rest[..end];
                    if key.contains('.') && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-') {
                        out.push(key.to_string());
                    }
                }
            }
        }
        fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    keys_in(&std::fs::read_to_string(&path).unwrap(), out);
                }
            }
        }
        let mut keys = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut keys);
        keys.sort();
        keys.dedup();
        assert!(keys.len() > 1000, "the scan found the keys: {}", keys.len());
        // A missing key comes back as itself: what the check below reads.
        // (Built, or the scan would find it as a key in use.)
        let probe = ["history", "param", "no_such_setting"].join(".");
        assert_eq!(t!(probe.as_str(), locale = "en"), probe.as_str());
        let missing: Vec<&String> = keys.iter().filter(|k| t!(k.as_str(), locale = "en") == k.as_str()).collect();
        assert!(missing.is_empty(), "{} keys have no English text:\n  {}", missing.len(), missing.iter().map(|k| k.as_str()).collect::<Vec<_>>().join("\n  "));
    }

    /// The names inside `%{...}`: what `t!` fills.
    fn placeholders(value: &str) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let mut rest = value;
        while let Some(at) = rest.find("%{") {
            rest = &rest[at + 2..];
            if let Some(end) = rest.find('}') {
                out.insert(rest[..end].to_string());
                rest = &rest[end..];
            }
        }
        out
    }

    /// The translations stay in step with English:
    ///
    /// - no locale has a key English lacks. Keys get renamed in en.yml
    ///   and the old ones lingered in es / ja / zh-CN, translated and
    ///   never shown, while the renamed key fell back to English;
    /// - a translation takes the same `%{...}` arguments as English.
    ///   Three of them had dropped theirs, so the performance panel read
    ///   "FPS" with no number;
    /// - English writes every placeholder as `%{x}`. `t!` leaves a
    ///   bare `{x}` alone, so "Rendering… {percent}%" was shown with the
    ///   braces in it.
    #[test]
    fn translations_match_english_keys_and_placeholders() {
        let table = |code: &str| tables().iter().find(|(c, _)| *c == code).map(|(_, t)| *t).unwrap();
        let en: std::collections::HashMap<&str, &str> = table("en").iter().copied().collect();
        assert!(en.len() > 1000, "the table holds {} en keys", en.len());

        let mut problems = Vec::new();
        for (key, value) in &en {
            let mut rest = *value;
            while let Some(at) = rest.find('{') {
                if !rest[..at].ends_with('%') && rest[at + 1..].find('}').is_some_and(|e| {
                    let name = &rest[at + 1..at + 1 + e];
                    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                }) {
                    problems.push(format!("en {key}: a placeholder without % -- {value}"));
                    break;
                }
                rest = &rest[at + 1..];
            }
        }
        for (name, entries) in tables() {
            if *name == "en" {
                continue;
            }
            for &(key, value) in *entries {
                match en.get(key) {
                    None => problems.push(format!("{name} {key}: not in en.yml")),
                    Some(english) if placeholders(english) != placeholders(value) => problems.push(format!(
                        "{name} {key}: takes {:?}, English {:?}",
                        placeholders(value),
                        placeholders(english)
                    )),
                    Some(_) => {}
                }
            }
        }
        problems.sort();
        assert!(problems.is_empty(), "{} problem(s):\n  {}", problems.len(), problems.join("\n  "));
    }

    /// egui's fonts have no glyphs for the invisible emoji plumbing
    /// characters, and egui does not treat them as zero-width — each
    /// one renders as a tofu box next to the emoji it was meant to
    /// style. "⚠️" is really ⚠ + U+FE0F, and the FE0F drew as a square
    /// in every dialog title that used it. Emoji themselves are fine;
    /// only the invisible modifiers break.
    #[test]
    fn no_invisible_emoji_modifiers_in_translations() {
        let bad = [
            ('\u{FE0F}', "VARIATION SELECTOR-16 (emoji presentation)"),
            ('\u{FE0E}', "VARIATION SELECTOR-15 (text presentation)"),
            ('\u{200D}', "ZERO WIDTH JOINER (emoji sequences)"),
        ];
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("locales dir exists") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("yml") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (line_no, line) in text.lines().enumerate() {
                for (ch, name) in bad {
                    if line.contains(ch) {
                        offenders.push(format!(
                            "{}:{} contains {} — {}",
                            path.file_name().unwrap().to_string_lossy(),
                            line_no + 1,
                            name,
                            line.trim()
                        ));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these render as tofu boxes in egui — use the bare emoji \
             (⚠ not ⚠️):\n  {}",
            offenders.join("\n  ")
        );
    }
}
