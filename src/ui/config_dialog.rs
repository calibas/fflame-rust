/// Render the Config Import/Export dialog window
pub fn render_config_dialog(
    ctx: &egui::Context,
    show_config_window: &mut bool,
    config_json_buffer: &mut String,
    config_export_json: &mut Option<String>,
    config_import_json: &mut Option<String>,
    config_save_file: &mut bool,
    config_load_file: &mut bool,
    apophysis_import_file: &mut bool,
) {
    if !*show_config_window {
        return;
    }

    egui::Window::new("Import/Export Configuration")
        .open(show_config_window)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.heading("Fractal Configuration");
            ui.label("Export/import all settings except iterations and max iterations.");
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("📋 Export to Clipboard").clicked() {
                    *config_export_json = Some(String::new()); // Will be filled in app.rs
                }

                if ui.button("💾 Save as .fflame").clicked() {
                    *config_save_file = true;
                }
            });

            ui.separator();
            ui.label("Import from JSON:");

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    ui.text_edit_multiline(config_json_buffer);
                });

            ui.horizontal(|ui| {
                if ui.button("✅ Import").clicked() && !config_json_buffer.is_empty() {
                    *config_import_json = Some(config_json_buffer.clone());
                }

                if ui.button("🗑 Clear").clicked() {
                    config_json_buffer.clear();
                }

                if ui.button("📁 Load .fflame").clicked() {
                    *config_load_file = true;
                }
            });

            ui.separator();
            ui.label("Import from Apophysis:");
            ui.horizontal(|ui| {
                if ui.button("🔥 Load Apophysis .flame").clicked() {
                    *apophysis_import_file = true;
                }
                ui.label("(XML format from Apophysis 7X)");
            });
        });
}
