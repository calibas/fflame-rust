use rust_i18n::t;

/// Render the Config Import/Export dialog content
pub fn render_config_dialog_content(
    ui: &mut egui::Ui,
    config_json_buffer: &mut String,
    config_export_json: &mut Option<String>,
    config_import_json: &mut Option<String>,
    config_save_file: &mut bool,
    config_load_file: &mut bool,
    flame_xml_import_file: &mut bool,
    flame_xml_export_file: &mut bool,
) {
    ui.heading(t!("config_dialog.heading"));
    ui.label(t!("config_dialog.description"));
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button(t!("config_dialog.export_clipboard")).clicked() {
            *config_export_json = Some(String::new()); // Will be filled in app.rs
        }

        if ui.button(t!("config_dialog.save_as_fflame")).clicked() {
            *config_save_file = true;
        }
    });

    ui.separator();
    ui.label(t!("config_dialog.import_from_json"));

    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            ui.text_edit_multiline(config_json_buffer);
        });

    ui.horizontal(|ui| {
        if ui.button(t!("config_dialog.import")).clicked() && !config_json_buffer.is_empty() {
            *config_import_json = Some(config_json_buffer.clone());
        }

        if ui.button(t!("config_dialog.clear")).clicked() {
            config_json_buffer.clear();
        }

        if ui.button(t!("config_dialog.load_fflame")).clicked() {
            *config_load_file = true;
        }
    });

    ui.separator();
    ui.label(t!("config_dialog.import_flame_xml_heading"));
    ui.horizontal(|ui| {
        if ui.button(t!("config_dialog.load_flame_xml")).clicked() {
            *flame_xml_import_file = true;
        }
        if ui.button(t!("config_dialog.save_flame_xml")).clicked() {
            *flame_xml_export_file = true;
        }
        ui.label(t!("config_dialog.flame_xml_format_hint"));
    });
}
