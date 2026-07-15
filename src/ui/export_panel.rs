//! PNG Export panel
//!
//! Floating panel for exporting fractals to PNG images.

use rust_i18n::t;
use crate::config::ConfigManager;

/// Peak GPU memory the export renderer holds *per output pixel*: histogram
/// (16 B, u32×4) + accumulation ping-pong (2× Rgba32Float = 32 B) + fractal
/// output (4 B) = 52 B/px, plus the histogram spatial-filter scratch (16 B)
/// when the flame uses it (`filter_radius > 0`). Color effects and analytic
/// blur don't raise the peak — color runs after the iteration buffers are
/// freed, and the analytic-blur buffers are low-res.
///
/// Solid rendering adds the per-pixel depth word + the accumulator's
/// depth-ownership tracker (8 B); lighting adds the shade-output
/// ping-pong and the normal ping-pong (4× Rgba32Float = 64 B). These
/// dominate on WASM, where the FlameRenderer holds the whole image.
fn export_bytes_per_pixel(spatial_filter: bool, solid: bool, lighting: bool) -> u64 {
    let mut b = 52;
    if spatial_filter {
        b += 16;
    }
    if solid {
        b += 8;
    }
    if lighting {
        b += 64;
    }
    b
}

/// Maximum total export size in pixels, given the flame's features.
///
/// The real cost (GPU memory, render time) tracks total pixels, not either
/// dimension alone, so the cap is on the product — both width/height fields
/// share it. On WASM the FlameRenderer holds the whole image in VRAM, so the
/// cap is a fixed GPU-memory budget ÷ the per-pixel cost, which shrinks when
/// the spatial filter adds its scratch buffer. The budget (~3.33 GB) is the
/// confirmed-working peak: 8000² with no spatial filter. On desktop, large
/// in-app exports tile through HighResExporter with bounded memory, so a fixed
/// generous cap (400 MP = 20000²) applies regardless of features.
fn max_export_pixels(spatial_filter: bool, solid: bool, lighting: bool) -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        const EXPORT_GPU_BUDGET_BYTES: u64 = 3_328_000_000;
        EXPORT_GPU_BUDGET_BYTES / export_bytes_per_pixel(spatial_filter, solid, lighting)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Desktop exports route through HighResExporter, whose big
        // allocations are bounded: the GPU-tiles histogram is capped at
        // GPU_HISTOGRAM_BUDGET (larger falls to the CPU-histogram
        // path), tonemap/shade tile in strips, and the depth buffer is
        // gated against one storage binding. A flat generous cap on
        // total pixels is therefore still honest with solid/lighting on.
        let _ = (spatial_filter, solid, lighting);
        400_000_000
    }
}

/// Minimum export dimension.
const MIN_EXPORT_DIM: u32 = 64;

/// Clamp `dim` (the just-edited field) so `dim * other` stays within `max_px`,
/// also respecting the per-dimension GPU texture cap `max_dim`. The other field
/// is left untouched, so editing one shrinks only the one you're editing.
fn clamp_to_budget(dim: &mut u32, other: u32, max_dim: u32, max_px: u64) {
    let other = (other as u64).max(1);
    let max_for_dim = (max_px / other).max(MIN_EXPORT_DIM as u64);
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

    // Feature-aware pixel budget: the histogram spatial filter (Apophysis
    // `filter`) adds a full-res scratch buffer, so it lowers the max export
    // size — most on WASM, where the whole image lives in VRAM.
    let spatial_filter = config_manager.active_config().filter_radius > 0.0;
    let (solid_on, lighting_on) = {
        let c = config_manager.active_config();
        let solid = c.solid_strength > 0.0
            && matches!(c.render_mode, crate::scene::transforms::RenderMode::ThreeD);
        (solid, solid && c.solid_shading.active())
    };
    let max_px = max_export_pixels(spatial_filter, solid_on, lighting_on);
    // Per-axis widget ceiling: the GPU's max texture size (the pixel budget
    // constrains the product within it). Guard against a degenerate 0.
    let dim_ceiling = max_export_dimension.max(MIN_EXPORT_DIM);

    // Keep the current size within the (feature-dependent) budget — e.g. when
    // the flame's spatial filter toggles on and lowers the cap. Shrink
    // proportionally so the aspect ratio is preserved.
    let total_px = *export_width as u64 * *export_height as u64;
    if total_px > max_px {
        let scale = (max_px as f64 / total_px as f64).sqrt();
        let w = ((*export_width as f64 * scale) as u32).clamp(MIN_EXPORT_DIM, dim_ceiling);
        let h = ((*export_height as f64 * scale) as u32).clamp(MIN_EXPORT_DIM, dim_ceiling);
        if w != *export_width || h != *export_height {
            *export_width = w;
            *export_height = h;
            let _ = config_manager.update_system_setting(crate::config::ConfigPath::SystemExportWidth, (*export_width).into());
            let _ = config_manager.update_system_setting(crate::config::ConfigPath::SystemExportHeight, (*export_height).into());
        }
    }

    ui.add_enabled_ui(*use_custom_export_size, |ui| {
        ui.horizontal(|ui| {
            ui.label(t!("export.width"));
            if ui.add(super::VkbDragValue::new(export_width).range(MIN_EXPORT_DIM..=dim_ceiling).speed(10)).changed() {
                // Shared pixel budget: shrink the field just edited so the total
                // stays within `max_px` (height left as-is).
                clamp_to_budget(export_width, *export_height, dim_ceiling, max_px);
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportWidth,
                    (*export_width).into()
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("export.height"));
            if ui.add(super::VkbDragValue::new(export_height).range(MIN_EXPORT_DIM..=dim_ceiling).speed(10)).changed() {
                clamp_to_budget(export_height, *export_width, dim_ceiling, max_px);
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemExportHeight,
                    (*export_height).into()
                );
            }
        });

        // Pixel-budget readout. `max_px` drops when the spatial filter is on.
        let mp = (*export_width as u64 * *export_height as u64) as f64 / 1e6;
        let max_mp = max_px as f64 / 1e6;
        let mut factors: Vec<&str> = Vec::new();
        if spatial_filter {
            factors.push("spatial filter");
        }
        if lighting_on {
            factors.push("lighting");
        } else if solid_on {
            factors.push("solid");
        }
        let suffix = if factors.is_empty() {
            String::new()
        } else {
            format!(" ({})", factors.join(", "))
        };
        ui.label(
            egui::RichText::new(format!("{:.1} MP / {:.0} MP max{}", mp, max_mp, suffix))
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
        let max_px = max_export_pixels(false, false, false); // desktop: 400 MP
        // Over budget at this height → width shrinks so the product fits.
        let mut w = 30000;
        clamp_to_budget(&mut w, 20000, max_dim, max_px);
        assert!((w as u64) * 20000 <= max_px);
        assert_eq!(w, 20000); // 400M / 20000

        // Wide aspect allowed within the budget, but the GPU axis cap binds first.
        let mut w2 = 60000;
        clamp_to_budget(&mut w2, 8000, max_dim, max_px);
        assert_eq!(w2, max_dim);
        assert!((w2 as u64) * 8000 <= max_px);

        // Tiny other dimension: budget allows huge width, GPU cap still bounds it.
        let mut w3 = 1_000_000;
        clamp_to_budget(&mut w3, MIN_EXPORT_DIM, max_dim, max_px);
        assert_eq!(w3, max_dim);

        // Already within budget → unchanged.
        let mut w4 = 12000;
        clamp_to_budget(&mut w4, 12000, max_dim, max_px);
        assert_eq!(w4, 12000);
    }

    #[test]
    fn spatial_filter_raises_per_pixel_cost() {
        assert_eq!(export_bytes_per_pixel(false, false, false), 52);
        assert_eq!(export_bytes_per_pixel(true, false, false), 68);
        assert!(export_bytes_per_pixel(true, false, false) > export_bytes_per_pixel(false, false, false));
        // Solid adds the depth word + accum-depth tracker; lighting adds
        // the shade-output and normal ping-pongs.
        assert_eq!(export_bytes_per_pixel(false, true, false), 60);
        assert_eq!(export_bytes_per_pixel(false, true, true), 124);
    }
}
