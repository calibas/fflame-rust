//! PNG Export panel
//!
//! Floating panel for exporting fractals to PNG images.

use rust_i18n::t;
use crate::config::ConfigManager;

/// Render the Export panel content. Live progress is shown by the global
/// export overlay (`export_status::render_export_overlay`); `export_active`
/// only gates the buttons so a second export can't be started mid-render.
pub fn render_export_content(
    ui: &mut egui::Ui,
    png_export_with_background: &mut bool,
    png_export_transparent: &mut bool,
    export_width: &mut u32,
    export_height: &mut u32,
    use_custom_export_size: &mut bool,
    png_export_premultiplied: &mut bool,
    config_manager: &mut ConfigManager,
    viewport_size: Option<(u32, u32)>,
    export_active: bool,
) {
    ui.heading(t!("export.heading"));

    ui.add_space(4.0);

    // Export buttons (disabled while an export is running)
    ui.add_enabled_ui(!export_active, |ui| {
        ui.horizontal(|ui| {
            if ui.button(t!("export.with_background")).clicked() {
                *png_export_with_background = true;
            }

            if ui.button(t!("export.transparent")).clicked() {
                *png_export_transparent = true;
            }
        });
    });

    // Transparent-alpha mode. Default (off): straight-alpha reconstruction —
    // a normal "flatten over black" in an image editor reproduces the opaque
    // export. On: premultiplied alpha, for After Effects / Nuke / linear
    // compositing pipelines that interpret premultiplied PNGs.
    ui.checkbox(png_export_premultiplied, t!("export.premultiplied_alpha"))
        .on_hover_text(t!("export.premultiplied_alpha_tooltip"));

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Show actual export resolution
    let (actual_width, actual_height) = if *use_custom_export_size {
        (*export_width, *export_height)
    } else {
        viewport_size.unwrap_or((800, 600))
    };
    ui.label(t!("export.resolution", width = actual_width, height = actual_height));

    // Custom size options
    ui.checkbox(use_custom_export_size, t!("export.use_custom_size"));

    ui.add_enabled_ui(*use_custom_export_size, |ui| {
        ui.horizontal(|ui| {
            ui.label(t!("export.width"));
            if ui.add(super::VkbDragValue::new(export_width).range(64..=12000).speed(10)).changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportWidth,
                    (*export_width).into()
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("export.height"));
            if ui.add(super::VkbDragValue::new(export_height).range(64..=12000).speed(10)).changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportHeight,
                    (*export_height).into()
                );
            }
        });
    });
}
