/// Render the Help window with keyboard shortcuts and documentation
pub fn render_help_window(
    ctx: &egui::Context,
    show_help: &mut bool,
) {
    egui::Window::new("Help")
        .open(show_help)
        .show(ctx, |ui| {
            ui.heading("Keyboard Shortcuts");
            ui.separator();

            ui.label("View Navigation:");
            ui.label("  ← ↑ ↓ →  - Pan view");
            ui.label("  + / -     - Zoom in/out");
            ui.label("  Numpad +/- - Zoom in/out");

            ui.separator();
            ui.label("Editing:");
            ui.label("  Ctrl+Z   - Undo");
            ui.label("  Ctrl+Y   - Redo");

            ui.separator();
            ui.label("Mouse Controls:");
            ui.label("  Drag     - Pan view");
            ui.label("  Wheel    - Zoom (toward cursor)");

            ui.separator();
            ui.heading("Documentation");
            ui.label("Documentation files are in the docs/ folder:");
            ui.label("  • STATUS.md - Implementation status");
            ui.label("  • ARCHITECTURE.md - Code organization");
            ui.label("  • TESTING-GUIDE.md - Testing guide");
            ui.label("  • QUICKSTART-WASM.md - WASM build guide");
        });
}
