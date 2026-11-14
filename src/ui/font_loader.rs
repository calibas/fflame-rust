//! Runtime font loading for internationalization
//!
//! This module handles loading fonts dynamically based on the selected locale.
//! Fonts are loaded from the assets/fonts/ directory on desktop builds.
//! WASM builds fall back to default fonts to avoid embedding large font files.
//!
//! **The UI's symbols** are drawn the same on every platform -- Windows,
//! macOS, Linux and the web: behind egui's own fonts in each family come
//! `UISymbols` (embedded: the arrows' and the language names' few
//! characters egui lacks, cut from the repository's Noto fonts by
//! `scripts/build_ui_symbols_font.py`) and Hack, egui's monospace font, which
//! every build already carries and which has the arrows, geometric shapes
//! and mathematical symbols. `every_drawn_character_has_a_glyph` fails when
//! a UI string uses a character none of them has: run the script, or pick
//! a character egui's icon font has.

use egui::Context;

/// Path to Noto Sans Regular font (better Unicode coverage than Ubuntu-Light)
const NOTO_SANS_PATH: &str = "assets/fonts/NotoSans-Regular.otf";

/// The UI's symbols egui's own fonts lack (the module's docs), embedded.
const UI_SYMBOLS: &[u8] = include_bytes!("../../assets/fonts/UISymbols.ttf");

/// egui's fonts with the symbol fallbacks behind them in every family, and
/// `primary` (Noto Sans, on the desktop) first in the proportional one.
fn font_definitions(primary: Option<Vec<u8>>) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("UISymbols".to_owned(), std::sync::Arc::new(egui::FontData::from_static(UI_SYMBOLS)));
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.push("UISymbols".to_owned());
        // egui's monospace font, already embedded: arrows, shapes, maths.
        if !family.iter().any(|f| f == "Hack") {
            family.push("Hack".to_owned());
        }
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.push("UISymbols".to_owned());
    }
    if let Some(data) = primary {
        fonts.font_data.insert("NotoSans".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        // The primary font, with egui's defaults as fallback.
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.insert(0, "NotoSans".to_owned());
        }
    }
    fonts
}

/// Initialize fonts: the symbol fallbacks on every platform, and on the
/// desktop Noto Sans as the default proportional font (it replaces
/// Ubuntu-Light, which has poor Unicode symbol coverage). Call this once
/// during app initialization.
pub fn initialize_default_fonts(ctx: &Context) {
    #[cfg(not(target_arch = "wasm32"))]
    let primary = match std::fs::read(crate::resources::resource_path(NOTO_SANS_PATH)) {
        Ok(data) => {
            log::info!("Loaded Noto Sans as default proportional font");
            Some(data)
        }
        Err(_) => {
            log::warn!("Could not load Noto Sans from '{}' - using egui defaults", NOTO_SANS_PATH);
            None
        }
    };
    // WASM: egui's defaults, to avoid a large font in the download (the
    // symbols embedded either way).
    #[cfg(target_arch = "wasm32")]
    let primary = {
        let _ = NOTO_SANS_PATH;
        None
    };
    ctx.set_fonts(font_definitions(primary));
}

/// Ensure the appropriate font is loaded for the given locale
///
/// Returns true if font was loaded successfully, false if fallback is needed
pub fn ensure_font_for_locale(ctx: &Context, locale: &str) -> bool {
    // Languages that work with egui's default font (Latin, Cyrillic, Greek)
    if matches!(
        locale,
        "en" | "es" | "fr" | "de" | "ru" | "pt" | "it" | "pl" | "uk" | "el"
    ) {
        // Reset to default font (in case user switched from CJK)
        reset_to_default_font(ctx);
        return true;
    }

    // For CJK and other special languages, try to load font
    let font_config = get_font_config(locale);

    if let Some(config) = font_config {
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Desktop: Load from filesystem
            if let Ok(font_data) = std::fs::read(crate::resources::resource_path(&config.path)) {
                apply_font(ctx, &config.name, font_data);
                log::info!("Loaded font for locale '{}': {}", locale, config.path);
                return true;
            } else {
                log::warn!("Font file not found: {} - falling back to default", config.path);
                return false;
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // WASM: Fonts not embedded, fall back to default
            log::warn!(
                "CJK font required for locale '{}' but not available in WASM build - use default language",
                locale
            );
            return false;
        }
    }

    // Unknown locale or no font config
    log::warn!("No font configuration for locale '{}' - falling back to default", locale);
    false
}

/// Font configuration for a locale
struct FontConfig {
    name: String,
    path: String,
}

/// Get font configuration for a specific locale
fn get_font_config(locale: &str) -> Option<FontConfig> {
    match locale {
        "ja" => Some(FontConfig {
            name: "Noto Sans JP".to_string(),
            path: "assets/fonts/NotoSansJP-Regular.otf".to_string(),
        }),
        "zh-CN" => Some(FontConfig {
            name: "Noto Sans SC".to_string(),
            path: "assets/fonts/NotoSansSC-Regular.otf".to_string(),
        }),
        "zh-TW" => Some(FontConfig {
            name: "Noto Sans TC".to_string(),
            path: "assets/fonts/NotoSansTC-Regular.otf".to_string(),
        }),
        "ko" => Some(FontConfig {
            name: "Noto Sans KR".to_string(),
            path: "assets/fonts/NotoSansKR-Regular.otf".to_string(),
        }),
        "ar" => Some(FontConfig {
            name: "Noto Sans Arabic".to_string(),
            path: "assets/fonts/NotoSansArabic-Regular.ttf".to_string(),
        }),
        "he" => Some(FontConfig {
            name: "Noto Sans Hebrew".to_string(),
            path: "assets/fonts/NotoSansHebrew-Regular.ttf".to_string(),
        }),
        _ => None,
    }
}

/// Apply a loaded font to the egui context
fn apply_font(ctx: &Context, font_name: &str, font_data: Vec<u8>) {
    // Get mutable FontDefinitions
    let mut fonts = egui::FontDefinitions::default();

    // Copy current font definitions
    ctx.fonts(|f| {
        fonts.font_data = f.definitions().font_data.clone();
        fonts.families = f.definitions().families.clone();
    });

    // Remove old custom fonts (if any)
    fonts.font_data.retain(|name, _| !name.starts_with("custom_"));

    // Add new font
    let custom_name = format!("custom_{}", font_name);
    fonts.font_data.insert(
        custom_name.clone(),
        std::sync::Arc::new(egui::FontData::from_owned(font_data)),
    );

    // Insert at the beginning for priority (fallback to default if glyph missing)
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(family_fonts) = fonts.families.get_mut(&family) {
            // Remove old custom fonts from family
            family_fonts.retain(|name| !name.starts_with("custom_"));
            // Insert new font at front
            family_fonts.insert(0, custom_name.clone());
        }
    }

    ctx.set_fonts(fonts);
}

/// Debug: Print font configuration and test glyph availability
/// Call this after fonts are initialized to see which fonts are loaded
/// and which font provides each test character.
pub fn debug_font_info(ctx: &Context) {
    log::info!("=== FONT DEBUG INFO ===");

    // Print font families
    ctx.fonts(|fonts| {
        let definitions = fonts.definitions();

        log::info!("Loaded font data:");
        for name in definitions.font_data.keys() {
            log::info!("  - {}", name);
        }

        log::info!("Font families:");
        for (family, names) in &definitions.families {
            log::info!("  {:?}: {:?}", family, names);
        }
    });

    log::info!("=== END FONT DEBUG ===");
}

/// Reset to egui's default font
fn reset_to_default_font(ctx: &Context) {
    // Get mutable FontDefinitions
    let mut fonts = egui::FontDefinitions::default();

    // Copy current font definitions
    ctx.fonts(|f| {
        fonts.font_data = f.definitions().font_data.clone();
        fonts.families = f.definitions().families.clone();
    });

    // Remove all custom fonts
    fonts.font_data.retain(|name, _| !name.starts_with("custom_"));

    // Remove custom fonts from families
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(family_fonts) = fonts.families.get_mut(&family) {
            family_fonts.retain(|name| !name.starts_with("custom_"));
        }
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latin_languages_use_default() {
        assert!(matches!("en", "en" | "es" | "fr" | "de" | "ru"));
        assert!(matches!("es", "en" | "es" | "fr" | "de" | "ru"));
        assert!(!matches!("ja", "en" | "es" | "fr" | "de" | "ru"));
    }

    #[test]
    fn test_font_config_exists_for_cjk() {
        assert!(get_font_config("ja").is_some());
        assert!(get_font_config("zh-CN").is_some());
        assert!(get_font_config("ko").is_some());
    }

    #[test]
    fn test_no_config_for_latin() {
        assert!(get_font_config("en").is_none());
        assert!(get_font_config("es").is_none());
    }

    /// Every non-ASCII character the UI may draw, and one place it is: the
    /// string literals (with their `\u{..}` escapes) of `src/` -- test
    /// modules, `src/bin` and comment lines left out -- and the values of
    /// `locales/en.yml`. What `scripts/build_ui_symbols_font.py` reads.
    fn drawn_characters() -> Vec<(char, String)> {
        fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("src").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|n| n != "bin") {
                        rust_files(&path, out);
                    }
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        rust_files(&root.join("src"), &mut files);
        let mut found: Vec<(char, String)> = Vec::new();
        let mut note = |c: char, at: String| {
            if !c.is_ascii() && !found.iter().any(|(k, _)| *k == c) {
                found.push((c, at));
            }
        };
        for path in files {
            let text = std::fs::read_to_string(&path).expect("read");
            let code = text.split("#[cfg(test)]").next().unwrap_or("");
            for (n, line) in code.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let mut chars = line.chars().peekable();
                let mut in_string = false;
                while let Some(c) = chars.next() {
                    match (in_string, c) {
                        (false, '"') | (true, '"') => in_string = !in_string,
                        (true, '\\') => {
                            if chars.next() == Some('u') && chars.peek() == Some(&'{') {
                                chars.next();
                                let hex: String = chars.by_ref().take_while(|h| *h != '}').collect();
                                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                    note(ch, format!("{}:{}", path.display(), n + 1));
                                }
                            }
                        }
                        (true, c) => note(c, format!("{}:{}", path.display(), n + 1)),
                        _ => {}
                    }
                }
            }
        }
        let en = std::fs::read_to_string(root.join("locales/en.yml")).expect("en.yml");
        for (n, line) in en.lines().enumerate() {
            if !line.trim_start().starts_with('#') {
                for c in line.chars() {
                    note(c, format!("locales/en.yml:{}", n + 1));
                }
            }
        }
        found
    }

    /// No icon is a square: every character the UI may draw has a glyph in
    /// the fonts every platform has (the web's: egui's, `UISymbols` and
    /// Hack, with no Noto Sans from disk), proportional and monospace.
    #[test]
    fn every_drawn_character_has_a_glyph() {
        use egui::epaint::text::{Fonts, TextOptions};
        let mut fonts = Fonts::new(TextOptions::default(), font_definitions(None));
        // Not drawn: variation selectors and the joiner (kept out of the
        // translations by `no_invisible_emoji_modifiers_in_translations`).
        let invisible = ['\u{FE0E}', '\u{FE0F}', '\u{200D}', '\u{200B}'];
        let mut missing = Vec::new();
        for (c, at) in drawn_characters() {
            if invisible.contains(&c) || c.is_control() {
                continue;
            }
            for id in [egui::FontId::proportional(14.0), egui::FontId::monospace(14.0)] {
                if !fonts.has_glyph(&id, c) {
                    missing.push(format!("U+{:04X} {c} ({:?}) at {at}", c as u32, id.family));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "characters drawn as a square -- run `python scripts/build_ui_symbols_font.py`, or use one egui's icon font has:\n  {}",
            missing.join("\n  ")
        );
    }
}
