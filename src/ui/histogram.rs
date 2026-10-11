//! Histogram visualization widget for density distribution
//!
//! Displays a density histogram with optional Levels controls overlay.

use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use crate::config::{ConfigManager, ConfigPath, UpdateType};
use crate::renderer::{DensityHistogram, HISTOGRAM_BINS};
use rust_i18n::t;

/// Height of the histogram widget in pixels
const HISTOGRAM_HEIGHT: f32 = 120.0;

/// Render a density histogram visualization with levels from config
pub fn render_histogram_with_config(
    ui: &mut egui::Ui,
    histogram: &DensityHistogram,
    config_manager: &ConfigManager,
) {
    let config = config_manager.active_config();
    let levels = LevelsDisplay {
        input_black: config.levels_low,
        input_white: config.levels_high,
        midpoint: gamma_to_midpoint(config.levels_gamma),
    };
    render_histogram_impl(ui, histogram, Some(&levels))
}

/// Internal histogram rendering implementation
fn render_histogram_impl(
    ui: &mut egui::Ui,
    histogram: &DensityHistogram,
    levels: Option<&LevelsDisplay>,
) {
    let available_width = ui.available_width().max(200.0);
    let (response, painter) = ui.allocate_painter(
        Vec2::new(available_width, HISTOGRAM_HEIGHT),
        egui::Sense::click_and_drag(),
    );

    let rect = response.rect;

    // Draw background
    painter.rect_filled(rect, 2.0, Color32::from_gray(25));

    // Draw border
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(60)), egui::StrokeKind::Outside);

    if !histogram.valid {
        // Show placeholder text
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No histogram data",
            egui::FontId::default(),
            Color32::from_gray(100),
        );
        return;
    }

    // Calculate scaling
    let max_count = histogram.max_bin_count().max(1);
    let bar_width = rect.width() / HISTOGRAM_BINS as f32;

    // Draw histogram bars
    for (i, &count) in histogram.bins.iter().enumerate() {
        if count == 0 {
            continue;
        }

        // Use logarithmic scaling for bar height to show both small and large values
        let log_count = (count as f32).ln_1p();
        let log_max = (max_count as f32).ln_1p();
        let normalized_height = log_count / log_max;

        let bar_height = normalized_height * (rect.height() - 4.0);
        let x = rect.left() + i as f32 * bar_width;
        let y = rect.bottom() - 2.0 - bar_height;

        let bar_rect = Rect::from_min_size(
            Pos2::new(x, y),
            Vec2::new(bar_width.max(1.0), bar_height),
        );

        // Color gradient based on position (represents density value)
        let t = i as f32 / (HISTOGRAM_BINS - 1) as f32;
        let color = density_color(t);
        painter.rect_filled(bar_rect, 0.0, color);
    }

    // Draw levels markers if provided
    if let Some(levels) = levels {
        draw_levels_markers(&painter, rect, histogram, levels);
    }

    // Draw statistics text
    let stats_text = format!(
        "Min: {:.1}  Max: {:.1}  Median: {:.1}",
        histogram.min_density,
        histogram.max_density,
        histogram.percentile_50
    );
    painter.text(
        Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        stats_text,
        egui::FontId::proportional(10.0),
        Color32::from_gray(180),
    );
}

/// Draw the levels markers (low/high/midpoint triangles)
/// Uses black/white triangle colors for Photoshop-like visual convention:
/// - Black triangle = low density threshold (below this → background color)
/// - White triangle = high density threshold (above this → full fractal color)
/// - Gray triangle = midpoint/gamma control
fn draw_levels_markers(
    painter: &egui::Painter,
    rect: Rect,
    histogram: &DensityHistogram,
    levels: &LevelsDisplay,
) {
    let triangle_size = 8.0;
    let y_bottom = rect.bottom() + 2.0;

    // Convert levels values to screen positions
    let black_x = levels_to_screen_x(levels.input_black, histogram, rect);
    let white_x = levels_to_screen_x(levels.input_white, histogram, rect);
    let mid_x = black_x + (white_x - black_x) * levels.midpoint;

    // Draw black point triangle (pointing up, at bottom)
    let black_triangle = [
        Pos2::new(black_x, y_bottom),
        Pos2::new(black_x - triangle_size / 2.0, y_bottom + triangle_size),
        Pos2::new(black_x + triangle_size / 2.0, y_bottom + triangle_size),
    ];
    painter.add(egui::Shape::convex_polygon(
        black_triangle.to_vec(),
        Color32::BLACK,
        Stroke::new(1.0, Color32::WHITE),
    ));

    // Draw white point triangle
    let white_triangle = [
        Pos2::new(white_x, y_bottom),
        Pos2::new(white_x - triangle_size / 2.0, y_bottom + triangle_size),
        Pos2::new(white_x + triangle_size / 2.0, y_bottom + triangle_size),
    ];
    painter.add(egui::Shape::convex_polygon(
        white_triangle.to_vec(),
        Color32::WHITE,
        Stroke::new(1.0, Color32::BLACK),
    ));

    // Draw midpoint triangle (gray)
    let mid_triangle = [
        Pos2::new(mid_x, y_bottom),
        Pos2::new(mid_x - triangle_size / 2.0, y_bottom + triangle_size),
        Pos2::new(mid_x + triangle_size / 2.0, y_bottom + triangle_size),
    ];
    painter.add(egui::Shape::convex_polygon(
        mid_triangle.to_vec(),
        Color32::GRAY,
        Stroke::new(1.0, Color32::WHITE),
    ));

    // Draw vertical lines for black and white points
    painter.line_segment(
        [Pos2::new(black_x, rect.top()), Pos2::new(black_x, rect.bottom())],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 100)),
    );
    painter.line_segment(
        [Pos2::new(white_x, rect.top()), Pos2::new(white_x, rect.bottom())],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 100)),
    );
}

/// Convert a levels value (in × mean density units) to screen X
/// coordinate on the raw-density histogram.
///
/// Levels values are stored in "× mean density" units after the
/// scale-invariance change. To position the marker on a histogram
/// whose x-axis is raw density, multiply by `sample_density` first.
fn levels_to_screen_x(density_x_mean: f32, histogram: &DensityHistogram, rect: Rect) -> f32 {
    if !histogram.valid || histogram.max_density <= 0.0 {
        return rect.left();
    }

    let raw_density = density_x_mean * histogram.sample_density.max(f32::EPSILON);
    let bin = histogram.density_to_bin(raw_density);
    let normalized = bin as f32 / (HISTOGRAM_BINS - 1) as f32;
    rect.left() + normalized * rect.width()
}

/// Generate a color for a given density position (0-1)
fn density_color(t: f32) -> Color32 {
    // Gradient from dark blue (low density) to bright cyan (high density)
    let r = (t * 100.0) as u8;
    let g = (100.0 + t * 155.0) as u8;
    let b = (150.0 + t * 105.0) as u8;
    Color32::from_rgb(r, g, b)
}

/// Display-only levels data for histogram markers
struct LevelsDisplay {
    input_black: f32,
    input_white: f32,
    midpoint: f32,
}

/// Convert gamma value to midpoint position (0-1)
/// gamma=1.0 -> midpoint=0.5 (linear)
fn gamma_to_midpoint(gamma: f32) -> f32 {
    1.0 / (1.0 + gamma.clamp(0.1, 10.0))
}

/// Render Levels controls using ConfigManager for state management
pub fn render_levels_controls_managed(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    histogram: &DensityHistogram,
) -> UpdateType {
    let mut max_update = UpdateType::None;
    let config = config_manager.active_config();

    // Read current values from config
    let mut levels_enabled = config.levels_enabled;
    let levels_low = config.levels_low;
    let levels_high = config.levels_high;
    let levels_gamma = config.levels_gamma;

    // Slider values are in "× mean density" units (matching the
    // shader's scale-invariant Levels). `histogram.sample_density`
    // is the mean iterations per pixel captured at readback; divide
    // raw-density numbers (percentiles, max_density) by it to land
    // in the same units the slider speaks.
    let mean = histogram.sample_density.max(f32::EPSILON);
    // Practical slider ceiling: max observed density relative to mean.
    // Default to 10× mean as a sane upper bound if max_density is
    // suspiciously low or zero (e.g. on first frame).
    let slider_max = (histogram.max_density / mean).max(10.0);

    ui.horizontal(|ui| {
        // Levels enable toggle. Off by default (Apo-matching); when off
        // the shader bypasses the per-pixel opacity remap entirely and
        // the rest of the controls have no effect.
        if ui.checkbox(&mut levels_enabled, t!("levels.title")).changed() {
            if let Ok(update) = config_manager.update_param(
                ConfigPath::LevelsEnabled,
                levels_enabled.into(),
            ) {
                max_update = max_update.max(update);
            }
        }

        // Auto button - one-shot apply histogram percentiles (× mean)
        ui.add_enabled_ui(histogram.valid && levels_enabled, |ui| {
            if ui.button(t!("levels.auto")).clicked() {
                if let Ok(update) = config_manager.update_param(
                    ConfigPath::LevelsLow,
                    (histogram.percentile_1 / mean).into()
                ) {
                    max_update = max_update.max(update);
                }
                if let Ok(update) = config_manager.update_param(
                    ConfigPath::LevelsHigh,
                    (histogram.percentile_99 / mean).into()
                ) {
                    max_update = max_update.max(update);
                }
            }
        });
    });

    // Sliders are grayed out when Levels is off — clarifies that the
    // controls are no-op until the toggle is enabled.
    ui.add_enabled_ui(levels_enabled, |ui| {
        // Low threshold slider (× mean density)
        ui.horizontal(|ui| {
            ui.label(t!("levels.low"));
            let range = 0.0..=slider_max;
            let mut temp_low = levels_low;
            if ui.add(super::VkbSlider::new(&mut temp_low, range).logarithmic(true)
                .clamping(egui::SliderClamping::Never)).changed() {
                if let Ok(update) = config_manager.update_param(ConfigPath::LevelsLow, temp_low.into()) {
                    max_update = max_update.max(update);
                }
            }
        });

        // High threshold slider (× mean density)
        ui.horizontal(|ui| {
            ui.label(t!("levels.high"));
            let range = 0.0..=slider_max;
            let mut temp_high = levels_high;
            if ui.add(super::VkbSlider::new(&mut temp_high, range).logarithmic(true)
                .clamping(egui::SliderClamping::Never)).changed() {
                if let Ok(update) = config_manager.update_param(ConfigPath::LevelsHigh, temp_high.into()) {
                    max_update = max_update.max(update);
                }
            }
        });

        // Midtones slider - controls the density-to-opacity curve
        ui.horizontal(|ui| {
            ui.label(t!("levels.midtones"));
            let mut temp_gamma = levels_gamma;
            if ui.add(super::VkbSlider::new(&mut temp_gamma, 0.001..=10.0).logarithmic(true)).changed() {
                if let Ok(update) = config_manager.update_param(ConfigPath::LevelsGamma, temp_gamma.into()) {
                    max_update = max_update.max(update);
                }
            }
        });
    });

    max_update
}
