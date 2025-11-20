/// Render the Help panel content (keyboard shortcuts and documentation)
pub fn render_help_content(ui: &mut egui::Ui) {
    ui.heading("Keyboard Shortcuts");
    ui.separator();

    ui.label("View Navigation:");
    ui.label("  ⬅⬆⬇➡ - Pan view");
    ui.label("  + / -      - Zoom in/out");
    ui.label("  Numpad +/-  - Zoom in/out");

    ui.separator();
    ui.label("Editing:");
    ui.label("  Ctrl+Z   - Undo");
    ui.label("  Ctrl+Y   - Redo");

    ui.separator();
    ui.label("Mouse Controls:");
    ui.label("  Drag     - Pan view");
    ui.label("  Wheel    - Zoom (toward cursor)");

}

