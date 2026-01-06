//! PNG Export panel
//!
//! Floating panel for exporting fractals to PNG images.

use rust_i18n::t;
use crate::config::ConfigManager;

/// Render the Export panel content
pub fn render_export_content(
    ui: &mut egui::Ui,
    png_export_with_background: &mut bool,
    png_export_transparent: &mut bool,
    export_width: &mut u32,
    export_height: &mut u32,
    use_custom_export_size: &mut bool,
    config_manager: &mut ConfigManager,
    viewport_size: Option<(u32, u32)>,
) {
    ui.heading(t!("export.heading"));

    ui.add_space(8.0);

    // Export buttons
    ui.horizontal(|ui| {
        if ui.button(t!("export.with_background")).clicked() {
            *png_export_with_background = true;
        }

        if ui.button(t!("export.transparent")).clicked() {
            *png_export_transparent = true;
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Custom size options
    ui.checkbox(use_custom_export_size, t!("export.use_custom_size"));

    ui.add_enabled_ui(*use_custom_export_size, |ui| {
        ui.horizontal(|ui| {
            ui.label(t!("export.width"));
            if ui.add(egui::DragValue::new(export_width).range(64..=8192).speed(10)).changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportWidth,
                    (*export_width).into()
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("export.height"));
            if ui.add(egui::DragValue::new(export_height).range(64..=8192).speed(10)).changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportHeight,
                    (*export_height).into()
                );
            }
        });
    });

    // Show actual export resolution
    let (actual_width, actual_height) = if *use_custom_export_size {
        (*export_width, *export_height)
    } else {
        viewport_size.unwrap_or((800, 600))
    };
    ui.label(t!("export.resolution", width = actual_width, height = actual_height));
}
