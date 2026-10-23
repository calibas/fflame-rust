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
#[cfg(any(test, target_arch = "wasm32"))]
fn export_bytes_per_pixel(spatial_filter: bool, solid: bool, lighting: bool, dof: bool) -> u64 {
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
    if dof {
        // Post-process DoF output texture (Rgba32Float).
        b += 16;
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
fn max_export_pixels(spatial_filter: bool, solid: bool, lighting: bool, dof: bool) -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        const EXPORT_GPU_BUDGET_BYTES: u64 = 3_328_000_000;
        EXPORT_GPU_BUDGET_BYTES / export_bytes_per_pixel(spatial_filter, solid, lighting, dof)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Desktop exports route through HighResExporter, whose big
        // allocations are bounded: the GPU-tiles histogram is capped at
        // GPU_HISTOGRAM_BUDGET (larger falls to the CPU-histogram
        // path), tonemap/shade tile in strips, and the depth buffer is
        // gated against one storage binding. A flat generous cap on
        // total pixels is therefore still honest with solid/lighting on.
        let _ = (spatial_filter, solid, lighting, dof);
        400_000_000
    }
}

/// Minimum export dimension.
const MIN_EXPORT_DIM: u32 = crate::config::MIN_IMAGE_DIM;

/// What the export budget weighs of a config: the spatial filter, a solid,
/// its lighting, its depth of field.
fn export_features(config: &crate::config::FractalConfig) -> (bool, bool, bool, bool) {
    let solid = config.solid_depth_active();
    (config.filter_radius > 0.0, solid, solid && config.solid_shading.active(), solid && config.dof_blur_strength > 0.0)
}

/// The size a PNG export renders at: the picture's (`image_size`, the
/// View panel's Output Size), scaled down with its aspect kept to what
/// the export can hold -- the GPU's per-axis texture limit (half of it,
/// supersampled) and the pixel budget for the config's features (a
/// quarter of it, supersampled).
pub fn fit_export_size(config: &crate::config::FractalConfig, supersample: bool, max_texture_dimension: u32) -> (u32, u32) {
    let (spatial, solid, lighting, dof) = export_features(config);
    let max_px = (max_export_pixels(spatial, solid, lighting, dof) / if supersample { 4 } else { 1 }) as f64;
    let ceiling = if supersample { max_texture_dimension / 2 } else { max_texture_dimension }.max(MIN_EXPORT_DIM);
    let (mut w, mut h) = (config.image_size.0.max(MIN_EXPORT_DIM) as f64, config.image_size.1.max(MIN_EXPORT_DIM) as f64);
    let axis = (ceiling as f64 / w.max(h)).min(1.0);
    let budget = (max_px / (w * h)).sqrt().min(1.0);
    let k = axis.min(budget);
    w = (w * k).floor();
    h = (h * k).floor();
    ((w as u32).clamp(MIN_EXPORT_DIM, ceiling), (h as u32).clamp(MIN_EXPORT_DIM, ceiling))
}

/// Render the Export panel content. Live progress is shown by the menu
/// bar's progress bar (`render_progress.rs`); `export_active`
/// only gates the buttons so a second export can't be started mid-render.
///
/// The size is the picture's (`FractalConfig::image_size`), set in the
/// View panel; `max_export_dimension` is the GPU's
/// `max_texture_dimension_2d`, which with the megapixel budget fits it
/// ([`fit_export_size`]).
pub fn render_export_content(
    ui: &mut egui::Ui,
    png_export_with_background: &mut bool,
    png_export_transparent: &mut bool,
    png_export_current: &mut bool,
    png_export_premultiplied: &mut bool,
    png_export_supersample: &mut bool,
    config_manager: &mut ConfigManager,
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
        // The viewport's picture as it stands -- its size, its
        // iterations so far -- saved at once, not rendered again.
        if ui.button(t!("export.current_image")).on_hover_text(t!("export.current_image_tip")).clicked() {
            *png_export_current = true;
        }
    });

    // Transparent-alpha mode. Default (off): straight-alpha reconstruction —
    // a normal "flatten over black" in an image editor reproduces the opaque
    // export. On: premultiplied alpha, for After Effects / Nuke / linear
    // compositing pipelines that interpret premultiplied PNGs.
    ui.checkbox(png_export_premultiplied, t!("export.premultiplied_alpha"))
        .on_hover_text(t!("export.premultiplied_alpha_tooltip"));

    // Privacy. Every export normally carries the complete FractalConfig,
    // which is what makes an image reproducible — and what leaks a
    // flame somebody would rather keep.
    {
        let mut strip = config_manager.system_settings().png_export_strip_metadata;
        if ui
            .checkbox(&mut strip, t!("export.strip_metadata"))
            .on_hover_text(t!("export.strip_metadata_tooltip"))
            .changed()
        {
            let _ = config_manager.update_system_setting(
                crate::config::ConfigPath::SystemPngStripMetadata,
                strip.into(),
            );
        }
    }

    // 2× supersampled antialiasing (desktop export paths only for now).
    #[cfg(not(target_arch = "wasm32"))]
    ui.checkbox(png_export_supersample, t!("export.supersample"))
        .on_hover_text(t!("export.supersample_tooltip"));
    #[cfg(target_arch = "wasm32")]
    {
        let _ = &png_export_supersample;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // The picture's size, and what this export makes of it.
    let config = config_manager.active_config();
    let (w, h) = config.image_size;
    let (ow, oh) = fit_export_size(config, *png_export_supersample, max_export_dimension);
    ui.label(t!("export.resolution", width = w, height = h));
    ui.small(t!("export.size_from_view"));
    if (ow, oh) != (w, h) {
        ui.colored_label(
            egui::Color32::from_rgb(230, 180, 90),
            t!("export.size_fitted", width = ow, height = oh),
        );
    }

    // The pixel budget, which the spatial filter (Apophysis `filter`),
    // a solid, its lighting and 2x supersampling lower -- most on WASM,
    // where the whole image lives in VRAM.
    let (spatial_filter, solid_on, lighting_on, dof_on) = export_features(config);
    let ss_factor: u64 = if *png_export_supersample { 4 } else { 1 };
    let max_px = max_export_pixels(spatial_filter, solid_on, lighting_on, dof_on) / ss_factor;
    let mp = (ow as u64 * oh as u64) as f64 / 1e6;
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
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;

    /// The picture's size, as is when it fits; past the GPU's per-axis
    /// limit or the pixel budget, scaled down whole -- the shape kept --
    /// to the tighter of them; supersampling halves the axis limit and
    /// quarters the budget.
    #[test]
    fn an_export_is_the_pictures_size_fitted() {
        let at = |w: u32, h: u32| {
            let mut c = crate::config::FractalConfig::default();
            c.image_size = (w, h);
            c
        };
        let max_dim = 32768;
        assert_eq!(fit_export_size(&at(1920, 1080), false, max_dim), (1920, 1080));
        assert_eq!(fit_export_size(&at(1080, 1920), false, max_dim), (1080, 1920));
        // Over the 400 MP budget: 32000 x 16000 is 512 MP.
        let (w, h) = fit_export_size(&at(32000, 16000), false, max_dim);
        assert!((w as u64) * (h as u64) <= 400_000_000, "{w}x{h}");
        assert!((w as f64 / h as f64 - 2.0).abs() < 1e-3, "the shape kept: {w}x{h}");
        // Past the axis limit first.
        let (w, h) = fit_export_size(&at(20000, 1000), false, 16384);
        assert_eq!(w, 16384);
        assert!((w as f64 / h as f64 - 20.0).abs() < 0.05, "{w}x{h}");
        // Supersampled: half the axis limit, a quarter of the budget.
        let (w, h) = fit_export_size(&at(16000, 9000), true, max_dim);
        assert!(w <= max_dim / 2 && (w as u64) * (h as u64) <= 100_000_000, "{w}x{h}");
    }

    #[test]
    fn spatial_filter_raises_per_pixel_cost() {
        assert_eq!(export_bytes_per_pixel(false, false, false, false), 52);
        assert_eq!(export_bytes_per_pixel(true, false, false, false), 68);
        assert!(export_bytes_per_pixel(true, false, false, false) > export_bytes_per_pixel(false, false, false, false));
        // Solid adds the depth word + accum-depth tracker; lighting adds
        // the shade-output and normal ping-pongs.
        assert_eq!(export_bytes_per_pixel(false, true, false, false), 60);
        assert_eq!(export_bytes_per_pixel(false, true, true, false), 124);
        assert_eq!(export_bytes_per_pixel(false, true, true, true), 140);
    }
}
