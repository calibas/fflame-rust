use rust_i18n::t;

use crate::config::{ConfigManager, ConfigPath};

/// Render the main Help panel with intro, links, and keyboard shortcuts
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

        ui.label(t!("help.intro_fractal_flames"));
        ui.add_space(4.0);
        ui.label(t!("help.intro_program"));
        ui.add_space(12.0);

        // Getting Started section
        ui.heading(t!("help.getting_started_heading"));
        ui.separator();
        ui.add_space(4.0);

        // Presets explanation
        ui.strong(t!("help.presets_title"));
        ui.label(t!("help.presets_description"));
        if ui.link(t!("help.open_preset_browser")).clicked() {
            *open_preset_library = true;
        }
        ui.add_space(8.0);

        // Random Generator explanation
        ui.strong(t!("help.random_title"));
        ui.label(t!("help.random_description"));
        if ui.link(t!("help.open_random_generator")).clicked() {
            *open_random_generator = true;
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
        ui.add_space(12.0);

        // Keyboard Shortcuts section
        ui.heading(t!("help.keyboard_shortcuts_heading"));
        ui.separator();

        ui.label(t!("help.view_navigation"));
        ui.label(t!("help.pan_view"));
        ui.label(t!("help.zoom_plus_minus"));
        ui.label(t!("help.zoom_numpad"));
        ui.label(t!("help.full_screen"));

        ui.separator();
        ui.label(t!("help.editing"));
        ui.label(t!("help.undo_shortcut"));
        ui.label(t!("help.redo_shortcut"));

        ui.separator();
        ui.label(t!("help.mouse_controls"));
        ui.label(t!("help.drag_pan"));
        ui.label(t!("help.wheel_zoom"));
    });
}

/// Render only keyboard shortcuts (for backward compatibility if needed)
pub fn render_help_content(ui: &mut egui::Ui) {
    ui.heading(t!("help.keyboard_shortcuts_heading"));
    ui.separator();

    ui.label(t!("help.view_navigation"));
    ui.label(t!("help.pan_view"));
    ui.label(t!("help.zoom_plus_minus"));
    ui.label(t!("help.zoom_numpad"));
    ui.label(t!("help.full_screen"));

    ui.separator();
    ui.label(t!("help.editing"));
    ui.label(t!("help.undo_shortcut"));
    ui.label(t!("help.redo_shortcut"));

    ui.separator();
    ui.label(t!("help.mouse_controls"));
    ui.label(t!("help.drag_pan"));
    ui.label(t!("help.wheel_zoom"));
}
