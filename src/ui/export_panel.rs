//! PNG Export panel
//!
//! Floating panel for exporting fractals to PNG images.

use rust_i18n::t;
use crate::config::ConfigManager;

/// Progress state for PNG export (shared between UI and export thread)
#[derive(Clone, Default)]
pub struct PngExportProgress {
    /// Whether export is currently in progress
    pub is_exporting: bool,
    /// Current iterations rendered
    pub current_iterations: u64,
    /// Total iterations to render
    pub total_iterations: u64,
    /// Status message
    pub status: String,
}

impl PngExportProgress {
    /// Get progress as a fraction (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.total_iterations == 0 {
            0.0
        } else {
            (self.current_iterations as f64 / self.total_iterations as f64) as f32
        }
    }
}

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
    export_progress: &PngExportProgress,
) {
    ui.heading(t!("export.heading"));

    ui.add_space(4.0);

    // Show progress if exporting
    if export_progress.is_exporting {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(&export_progress.status);
        });
        let progress = export_progress.progress();
        ui.add(egui::ProgressBar::new(progress).show_percentage());
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
    }

    // Export buttons (disabled while exporting)
    ui.add_enabled_ui(!export_progress.is_exporting, |ui| {
        ui.horizontal(|ui| {
            if ui.button(t!("export.with_background")).clicked() {
                *png_export_with_background = true;
            }

            if ui.button(t!("export.transparent")).clicked() {
                *png_export_transparent = true;
            }
        });
    });

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
            if ui.add(super::VkbDragValue::new(export_width).range(64..=8192).speed(10)).changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportWidth,
                    (*export_width).into()
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("export.height"));
            if ui.add(super::VkbDragValue::new(export_height).range(64..=8192).speed(10)).changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportHeight,
                    (*export_height).into()
                );
            }
        });
    });
}
