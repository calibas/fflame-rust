use rust_i18n::t;

use crate::config::{ConfigManager, ConfigPath};

/// Render the main Help panel with intro and links
pub fn render_help_panel_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    open_preset_library: &mut bool,
    open_random_generator: &mut bool,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Introduction section
        ui.heading(t!("help.welcome_heading"));
        ui.add_space(8.0);

        // Getting Started section
        ui.heading(t!("help.getting_started_heading"));
        ui.separator();
        ui.add_space(4.0);

        // Presets explanation
        ui.label(t!("help.presets_description"));
        if ui.link(t!("help.open_preset_browser")).clicked() {
            *open_preset_library = true;
        }
        ui.add_space(8.0);

        // Random Generator explanation
        ui.label(t!("help.random_description"));
        if ui.link(t!("help.open_random_generator")).clicked() {
            *open_random_generator = true;
        }
        ui.add_space(8.0);

        ui.label(t!("help.tutorial_description"));
        if ui.link(t!("help.view_tutorial")).clicked() {
            let _ = webbrowser::open("https://github.com/calibas/fflame-rust/tree/main/docs/tutorials/README.md");
        }
        ui.add_space(12.0);

        // Hide on startup checkbox
        ui.separator();
        let mut hide_on_startup = !config_manager.system_settings().show_help_on_startup;
        if ui.checkbox(&mut hide_on_startup, t!("help.hide_on_startup")).changed() {
            let _ = config_manager.update_system_setting(
                ConfigPath::SystemShowHelpOnStartup,
                (!hide_on_startup).into(),
            );
        }
    });
}


/// Platform suffix for shortcut strings: `_macos`, `_web`, or none.
///
/// Windows and Linux share the base keys — Ctrl, Alt, F-keys behave the
/// same — so only the two genuinely different platforms carry a suffix.
const PLATFORM_SUFFIX: &str = if cfg!(target_arch = "wasm32") {
    "_web"
} else if cfg!(target_os = "macos") {
    "_macos"
} else {
    ""
};

/// A shortcut string, using the platform-specific variant when one
/// exists and the base string when it does not.
///
/// The fallback is the point. Adding a shortcut means adding ONE key;
/// only the lines that genuinely differ per platform need a `_macos` or
/// `_web` twin, and forgetting one is invisible rather than broken. The
/// alternative — a full parallel set per platform — triples the
/// translator's work across four locales and guarantees drift the first
/// time someone adds a binding.
///
/// rust-i18n returns the key itself for a missing entry, which is how a
/// missing variant is detected.
fn shortcut(key: &str) -> std::borrow::Cow<'static, str> {
    if !PLATFORM_SUFFIX.is_empty() {
        let specific = format!("help.{key}{PLATFORM_SUFFIX}");
        let translated = t!(&specific);
        if translated != specific {
            return std::borrow::Cow::Owned(translated.into_owned());
        }
    }
    let base = format!("help.{key}");
    std::borrow::Cow::Owned(t!(&base).into_owned())
}

/// Render keyboard shortcuts panel content.
///
/// Every line goes through `shortcut()`, so the panel describes THIS
/// platform. That matters beyond politeness: the app already binds
/// Cmd on macOS (see `app::input`, which selects `super_key()` there),
/// so a panel hard-coded to "Ctrl+Z" was misdocumenting behaviour that
/// works correctly.
pub fn render_keyboard_shortcuts_content(ui: &mut egui::Ui) {
    ui.heading(t!("help.keyboard_shortcuts_heading"));
    ui.separator();

    ui.label(t!("help.view_navigation"));
    ui.label(shortcut("pan_view").as_ref());
    ui.label(shortcut("zoom_plus_minus").as_ref());
    ui.label(shortcut("zoom_numpad").as_ref());
    ui.label(shortcut("full_screen").as_ref());
    ui.label(shortcut("fly_mode").as_ref());

    ui.separator();
    ui.label(t!("help.editing"));
    ui.label(shortcut("undo_shortcut").as_ref());
    ui.label(shortcut("redo_shortcut").as_ref());
    ui.label(shortcut("play_pause_animation").as_ref());

    ui.separator();
    ui.label(t!("help.mouse_controls"));
    ui.label(shortcut("drag_pan").as_ref());
    ui.label(shortcut("wheel_zoom").as_ref());
    ui.label(shortcut("alt_drag_rotate").as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key with no platform variant must fall back to the base string,
    /// and never surface a raw key name to the user. This is the property
    /// that makes adding a shortcut a one-key job.
    #[test]
    fn missing_platform_variant_falls_back_to_base() {
        let s = shortcut("pan_view");
        assert!(!s.contains("help."), "leaked a translation key: {s}");
        assert!(s.contains("Pan view"), "unexpected base string: {s}");
    }

    /// Where a variant exists it must win on that platform — and the base
    /// must still be the one Windows/Linux see.
    #[test]
    fn platform_variant_wins_where_it_exists() {
        let undo = shortcut("undo_shortcut");
        assert!(!undo.contains("help."), "leaked a translation key: {undo}");
        if cfg!(target_os = "macos") && !cfg!(target_arch = "wasm32") {
            assert!(undo.contains("Cmd"), "macOS should say Cmd: {undo}");
            // The app binds super_key() on macOS (app::input), so this is
            // documenting real behaviour, not aspiration.
            assert!(!undo.contains("Ctrl"), "macOS must not say Ctrl: {undo}");
        } else if !cfg!(target_arch = "wasm32") {
            assert!(undo.contains("Ctrl"), "non-mac should say Ctrl: {undo}");
        }
    }

    /// Every shortcut the panel renders must resolve on this platform.
    /// Catches a typo'd key or a deleted base string.
    #[test]
    fn every_rendered_shortcut_resolves() {
        for key in [
            "pan_view", "zoom_plus_minus", "zoom_numpad", "full_screen",
            "fly_mode", "undo_shortcut", "redo_shortcut", "drag_pan",
            "wheel_zoom", "alt_drag_rotate",
        ] {
            let s = shortcut(key);
            assert!(!s.contains("help."), "`{key}` does not resolve: {s}");
            assert!(!s.trim().is_empty(), "`{key}` is empty");
        }
    }
}
