//! Internationalization (i18n) support
//!
//! This module provides translation support using the rust-i18n crate.
//! Translation files are stored in the `locales/` directory at the project root.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::i18n::t;
//!
//! // Simple translation
//! let title = t!("app.title");
//!
//! // With variables
//! let message = t!("messages.welcome", name = "Alice");
//! ```
//!
//! # Supported Languages
//!
//! - English (en) - Default
//! - Spanish (es)
//! - Japanese (ja)
//! - Chinese Simplified (zh-CN)
//! - More languages can be added by creating .yml files in locales/

// Note: rust-i18n::i18n!() is initialized in lib.rs

/// Get current locale
pub fn current_locale() -> String {
    rust_i18n::locale().to_string()
}

/// Set locale (e.g., "en", "es", "fr", "de", "ja", "zh-CN")
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}

// Re-export the t! macro for convenience
pub use rust_i18n::t;

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
        assert_eq!(rust_i18n::t!(probe.as_str(), locale = "en"), probe.as_str());
        let missing: Vec<&String> = keys.iter().filter(|k| rust_i18n::t!(k.as_str(), locale = "en") == k.as_str()).collect();
        assert!(missing.is_empty(), "{} keys have no English text:\n  {}", missing.len(), missing.iter().map(|k| k.as_str()).collect::<Vec<_>>().join("\n  "));
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
