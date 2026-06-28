//! PNG Export panel
//!
//! Floating panel for exporting fractals to PNG images.

use rust_i18n::t;
use crate::config::ConfigManager;

/// Maximum total export size, in pixels. The real cost (GPU/CPU memory and
/// render time) tracks total pixels, not either dimension alone, so the cap is
/// on the product — both width/height fields share it. Desktop allows 400 MP
/// (20000²); WASM is far more constrained (browser GPU + single-binding
/// histogram), so 64 MP (8000²).
#[cfg(not(target_arch = "wasm32"))]
const MAX_EXPORT_PIXELS: u64 = 400_000_000;
#[cfg(target_arch = "wasm32")]
const MAX_EXPORT_PIXELS: u64 = 64_000_000;

/// Minimum export dimension.
const MIN_EXPORT_DIM: u32 = 64;

/// Clamp `dim` (the just-edited field) so `dim * other` stays within the
/// megapixel budget, also respecting the per-dimension GPU texture cap. The
/// other field is left untouched, so editing one shrinks only the one you're
/// editing.
fn clamp_to_budget(dim: &mut u32, other: u32, max_dim: u32) {
    let other = (other as u64).max(1);
    let max_for_dim = (MAX_EXPORT_PIXELS / other).max(MIN_EXPORT_DIM as u64);
    let ceil = (max_for_dim.min(max_dim as u64)) as u32;
    *dim = (*dim).clamp(MIN_EXPORT_DIM, ceil.max(MIN_EXPORT_DIM));
}

/// Render the Export panel content. Live progress is shown by the global
/// export overlay (`export_status::render_export_overlay`); `export_active`
/// only gates the buttons so a second export can't be started mid-render.
///
/// `max_export_dimension` is the GPU's `max_texture_dimension_2d` — the hard
/// per-axis ceiling. Within it, the megapixel budget is the real limit.
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
    max_export_dimension: u32,
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

    // Per-axis widget ceiling: the GPU's max texture size (the megapixel budget
    // constrains the product within it). Guard against a degenerate 0.
    let dim_ceiling = max_export_dimension.max(MIN_EXPORT_DIM);

    ui.add_enabled_ui(*use_custom_export_size, |ui| {
        ui.horizontal(|ui| {
            ui.label(t!("export.width"));
            if ui.add(super::VkbDragValue::new(export_width).range(MIN_EXPORT_DIM..=dim_ceiling).speed(10)).changed() {
                // Shared megapixel budget: shrink the field just edited so the
                // total stays within MAX_EXPORT_PIXELS (height left as-is).
                clamp_to_budget(export_width, *export_height, dim_ceiling);
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportWidth,
                    (*export_width).into()
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("export.height"));
            if ui.add(super::VkbDragValue::new(export_height).range(MIN_EXPORT_DIM..=dim_ceiling).speed(10)).changed() {
                clamp_to_budget(export_height, *export_width, dim_ceiling);
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportHeight,
                    (*export_height).into()
                );
            }
        });

        // Megapixel readout — the actual shared limit.
        let mp = (*export_width as u64 * *export_height as u64) as f64 / 1e6;
        let max_mp = MAX_EXPORT_PIXELS as f64 / 1e6;
        ui.label(
            egui::RichText::new(format!("{:.1} MP / {:.0} MP max", mp, max_mp))
                .small()
                .color(egui::Color32::GRAY),
        );
    });
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;

    #[test]
    fn budget_clamp_caps_total_pixels() {
        let max_dim = 32768;
        // Over budget at this height → width shrinks so the product fits.
        let mut w = 30000;
        clamp_to_budget(&mut w, 20000, max_dim);
        assert!((w as u64) * 20000 <= MAX_EXPORT_PIXELS);
        assert_eq!(w, 20000); // 400M / 20000

        // Wide aspect allowed within the budget, but the GPU axis cap binds first.
        let mut w2 = 60000;
        clamp_to_budget(&mut w2, 8000, max_dim);
        assert_eq!(w2, max_dim);
        assert!((w2 as u64) * 8000 <= MAX_EXPORT_PIXELS);

        // Tiny other dimension: budget allows huge width, GPU cap still bounds it.
        let mut w3 = 1_000_000;
        clamp_to_budget(&mut w3, MIN_EXPORT_DIM, max_dim);
        assert_eq!(w3, max_dim);

        // Already within budget → unchanged.
        let mut w4 = 12000;
        clamp_to_budget(&mut w4, 12000, max_dim);
        assert_eq!(w4, 12000);
    }
}
