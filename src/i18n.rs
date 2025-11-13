//! Internationalization (i18n) support
//!
//! This module provides translation support using the rust-i18n crate.
//! Translation files are stored in the `locales/` directory at the project root.
//!
//! # Usage
//!
//! ```rust
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
//! - More languages can be added by creating .yml files in locales/

// Initialize rust-i18n with the locales directory
rust_i18n::i18n!("locales", fallback = "en");

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
}

impl LocaleInfo {
    pub const fn new(code: &'static str, name: &'static str, native_name: &'static str) -> Self {
        Self { code, name, native_name }
    }
}

/// Get list of supported locales with display names
pub fn supported_locales() -> Vec<LocaleInfo> {
    vec![
        LocaleInfo::new("en", "English", "English"),
        LocaleInfo::new("es", "Spanish", "Español"),
        // Add more as translation files are created:
        // LocaleInfo::new("fr", "French", "Français"),
        // LocaleInfo::new("de", "German", "Deutsch"),
        // LocaleInfo::new("ja", "Japanese", "日本語"),
        // LocaleInfo::new("zh-CN", "Chinese (Simplified)", "简体中文"),
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

    // Translation tests require the full app context with assets
    // They work in release mode, but not in unit tests
    // Test translation manually by running the app
}
