use rust_i18n::t;

/// Render the Help panel content (keyboard shortcuts and documentation)
pub fn render_help_content(ui: &mut egui::Ui) {
    ui.heading(t!("help.keyboard_shortcuts_heading"));
    ui.separator();

    ui.label(t!("help.view_navigation"));
    ui.label(t!("help.pan_view"));
    ui.label(t!("help.zoom_plus_minus"));
    ui.label(t!("help.zoom_numpad"));

    ui.separator();
    ui.label(t!("help.editing"));
    ui.label(t!("help.undo_shortcut"));
    ui.label(t!("help.redo_shortcut"));

    ui.separator();
    ui.label(t!("help.mouse_controls"));
    ui.label(t!("help.drag_pan"));
    ui.label(t!("help.wheel_zoom"));
}
