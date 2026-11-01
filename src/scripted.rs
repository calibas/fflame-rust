//! What every script a person writes shares, whatever it compiles to:
//! its identity and its labels (docs/projects/rhai-plugins.md, design
//! rules 7 and 8).
//!
//! - **Identity.** Built-ins keep their string ids (`vignette`,
//!   `julian`). Everything else is a UUID written into the script's own
//!   text -- `INFO.id` for the GPU kinds -- minted when it is created
//!   ([`mint_id`]), and its version is the SHA-256 of its source
//!   ([`version_of`]).
//! - **Labels.** What a person reads is a string, or a map of locale to
//!   string ([`Label`]), resolved at the moment it is shown: the current
//!   locale, then its language, then a plain string, then `en`, then the
//!   first entry.
//! - **What is registered** ([`Ledger`]): which version of each id was
//!   last registered, so carrying a source again costs nothing and one
//!   that failed is not retried every frame.

use std::collections::HashMap;

/// A label: one text for every language, or texts by locale.
///
/// Kept as written (an entry's locale `""` is the plain string), so
/// whatever is shown is resolved when it is shown: a change of language
/// needs nothing recompiled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Label(Vec<(String, String)>);

impl Label {
    /// One text for every language.
    pub fn text(s: impl Into<String>) -> Self {
        let s = s.into();
        if s.is_empty() {
            Self::default()
        } else {
            Self(vec![(String::new(), s)])
        }
    }

    /// Texts by locale, in the order written. Empty texts are dropped.
    pub fn locales(entries: impl IntoIterator<Item = (String, String)>) -> Self {
        Self(entries.into_iter().filter(|(_, t)| !t.is_empty()).collect())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The text for the app's current locale.
    pub fn resolve(&self) -> String {
        self.resolve_in(&rust_i18n::locale())
    }

    /// The text for `locale`: that locale, then another of its language
    /// (`zh` for `zh-CN`), then the plain string, then `en`, then the
    /// first written. Empty when the label is.
    pub fn resolve_in(&self, locale: &str) -> String {
        let language = |l: &str| l.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
        let wanted = language(locale);
        let find = |pred: &dyn Fn(&str) -> bool| self.0.iter().find(|(l, _)| pred(l)).map(|(_, t)| t.clone());
        find(&|l| l.eq_ignore_ascii_case(locale))
            .or_else(|| find(&|l| !l.is_empty() && language(l) == wanted))
            .or_else(|| find(&|l| l.is_empty()))
            .or_else(|| find(&|l| language(l) == "en"))
            .or_else(|| self.0.first().map(|(_, t)| t.clone()))
            .unwrap_or_default()
    }

    /// The locales written, `""` for a plain string.
    pub fn locale_names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(l, _)| l.as_str())
    }
}

impl From<&str> for Label {
    fn from(s: &str) -> Self {
        Self::text(s)
    }
}

impl From<String> for Label {
    fn from(s: String) -> Self {
        Self::text(s)
    }
}

/// A fresh id: a random (version 4) UUID, lowercase, hyphenated.
pub fn mint_id() -> String {
    let mut bits: u128 = rand::random();
    // Version 4 and the RFC 4122 variant.
    bits = (bits & !(0xf << 76)) | (0x4 << 76);
    bits = (bits & !(0x3 << 62)) | (0x2 << 62);
    let hex = format!("{bits:032x}");
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// Whether `s` is a UUID as [`mint_id`] writes one (any version):
/// lowercase hex in the 8-4-4-4-12 groups. A built-in's string id never
/// is, which is how the two kinds of identity tell themselves apart.
pub fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_digit() || ('a'..='f').contains(&c),
        })
}

/// A source's version: the SHA-256 of its text, lowercase hex.
pub fn version_of(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(source.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// What a [`Ledger`] remembers of one id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registered {
    /// This version is registered.
    Ok(String),
    /// This version failed, with why: not retried until the source
    /// changes.
    Failed(String, String),
}

/// Which version of each id was last registered, and whether it took.
///
/// Every kind that registers scripts carried by a config (`ensure`) asks
/// this first, so carrying a source again is free and a failure is
/// reported once rather than every frame.
#[derive(Debug, Default)]
pub struct Ledger {
    entries: HashMap<String, Registered>,
}

impl Ledger {
    /// Whether `version` of `id` needs registering: it is not the version
    /// last registered or last refused.
    pub fn needs(&self, id: &str, version: &str) -> bool {
        match self.entries.get(id) {
            Some(Registered::Ok(v)) | Some(Registered::Failed(v, _)) => v != version,
            None => true,
        }
    }

    pub fn record(&mut self, id: &str, version: &str, result: Result<(), String>) {
        let entry = match result {
            Ok(()) => Registered::Ok(version.to_string()),
            Err(why) => Registered::Failed(version.to_string(), why),
        };
        self.entries.insert(id.to_string(), entry);
    }

    pub fn get(&self, id: &str) -> Option<&Registered> {
        self.entries.get(id)
    }

    /// Why `id`'s registered version failed, when it did.
    pub fn failure(&self, id: &str) -> Option<&str> {
        match self.entries.get(id) {
            Some(Registered::Failed(_, why)) => Some(why),
            _ => None,
        }
    }

    pub fn forget(&mut self, id: &str) {
        self.entries.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_resolves_by_locale_then_language_then_plain_then_english() {
        let l = Label::locales([
            ("en".to_string(), "Spiral".to_string()),
            ("ja".to_string(), "螺旋".to_string()),
            ("zh-CN".to_string(), "螺旋线".to_string()),
        ]);
        assert_eq!(l.resolve_in("ja"), "螺旋");
        assert_eq!(l.resolve_in("zh-CN"), "螺旋线");
        assert_eq!(l.resolve_in("zh-TW"), "螺旋线", "another of its language");
        assert_eq!(l.resolve_in("es"), "Spiral", "English when nothing closer");
        let plain = Label::text("Amount");
        assert_eq!(plain.resolve_in("ja"), "Amount");
        let mixed = Label::locales([("".to_string(), "Amount".to_string()), ("ja".to_string(), "量".to_string())]);
        assert_eq!(mixed.resolve_in("ja"), "量");
        assert_eq!(mixed.resolve_in("es"), "Amount", "the plain string before English");
        let only = Label::locales([("fr".to_string(), "Quantité".to_string())]);
        assert_eq!(only.resolve_in("ja"), "Quantité", "the first written, last");
        assert!(Label::text("").is_empty());
        assert_eq!(Label::default().resolve_in("en"), "");
    }

    #[test]
    fn a_minted_id_is_a_version_4_uuid_and_ids_differ() {
        let a = mint_id();
        let b = mint_id();
        assert!(is_uuid(&a), "{a}");
        assert_ne!(a, b);
        assert_eq!(&a[14..15], "4", "version 4: {a}");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"), "RFC 4122 variant: {a}");
        assert!(!is_uuid("vignette"));
        assert!(!is_uuid("3F2B9C1E-0000-4000-8000-000000000000"), "lowercase only");
    }

    #[test]
    fn a_version_is_the_sources_sha256() {
        assert_eq!(version_of(""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_ne!(version_of("a"), version_of("b"));
    }

    #[test]
    fn the_ledger_registers_each_version_once() {
        let mut l = Ledger::default();
        assert!(l.needs("x", "v1"));
        l.record("x", "v1", Ok(()));
        assert!(!l.needs("x", "v1"));
        assert!(l.needs("x", "v2"), "a new version registers");
        l.record("x", "v2", Err("bad".into()));
        assert!(!l.needs("x", "v2"), "a failure is not retried");
        assert_eq!(l.failure("x"), Some("bad"));
        assert!(l.needs("x", "v1"), "going back is a change too");
    }
}
