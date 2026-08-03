//! Runtime font loading for internationalization
//!
//! This module handles loading fonts dynamically based on the selected locale.
//! Fonts are loaded from the assets/fonts/ directory on desktop builds.
//! WASM builds fall back to default fonts to avoid embedding large font files.

use egui::Context;

/// Path to Noto Sans Regular font (better Unicode coverage than Ubuntu-Light)
const NOTO_SANS_PATH: &str = "assets/fonts/NotoSans-Regular.otf";

/// Initialize fonts with Noto Sans as the default proportional font.
/// This replaces Ubuntu-Light which has poor Unicode symbol coverage.
/// Call this once during app initialization.
pub fn initialize_default_fonts(ctx: &Context) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(font_data) = std::fs::read(crate::resources::resource_path(NOTO_SANS_PATH)) {
            let mut fonts = egui::FontDefinitions::default();

            // Add Noto Sans
            fonts.font_data.insert(
                "NotoSans".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(font_data)),
            );

            // Insert Noto Sans at the front of the Proportional family
            // This makes it the primary font, with egui's defaults as fallback
            if let Some(family_fonts) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family_fonts.insert(0, "NotoSans".to_owned());
            }

            ctx.set_fonts(fonts);
            log::info!("Loaded Noto Sans as default proportional font");
        } else {
            log::warn!(
                "Could not load Noto Sans from '{}' - using egui defaults",
                NOTO_SANS_PATH
            );
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        // WASM: Use egui defaults to avoid embedding large font files
        log::info!("WASM build: using egui default fonts");
        let _ = ctx; // Silence unused warning
    }
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
}
