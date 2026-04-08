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
///
/// Returns the rect where the histogram was drawn (for overlay controls)
pub fn render_histogram_with_config(
    ui: &mut egui::Ui,
    histogram: &DensityHistogram,
    config_manager: &ConfigManager,
) -> HistogramResponse {
    let config = config_manager.active_config();
    let levels = LevelsDisplay {
        input_black: config.levels_low,
        input_white: config.levels_high,
        midpoint: gamma_to_midpoint(config.levels_gamma),
    };
    render_histogram_impl(ui, histogram, Some(&levels))
}

/// Render a density histogram visualization
///
/// Returns the rect where the histogram was drawn (for overlay controls)
pub fn render_histogram(
    ui: &mut egui::Ui,
    histogram: &DensityHistogram,
    levels: Option<&LevelsState>,
) -> HistogramResponse {
    let levels_display = levels.map(|l| LevelsDisplay {
        input_black: l.input_black,
        input_white: l.input_white,
        midpoint: l.midpoint,
    });
    render_histogram_impl(ui, histogram, levels_display.as_ref())
}

/// Internal histogram rendering implementation
fn render_histogram_impl(
    ui: &mut egui::Ui,
    histogram: &DensityHistogram,
    levels: Option<&LevelsDisplay>,
) -> HistogramResponse {
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
        return HistogramResponse {
            rect,
            clicked_position: None,
        };
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

    // Handle click to get density at position
    let clicked_position = if response.clicked() {
        response.interact_pointer_pos().map(|pos| {
            let x_normalized = (pos.x - rect.left()) / rect.width();
            x_normalized.clamp(0.0, 1.0)
        })
    } else {
        None
    };

    HistogramResponse {
        rect,
        clicked_position,
    }
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

/// Convert a levels value (density) to screen X coordinate
fn levels_to_screen_x(density: f32, histogram: &DensityHistogram, rect: Rect) -> f32 {
    if !histogram.valid || histogram.max_density <= 0.0 {
        return rect.left();
    }

    let bin = histogram.density_to_bin(density);
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

/// Response from rendering the histogram
pub struct HistogramResponse {
    /// The rect where the histogram was drawn
    pub rect: Rect,
    /// If clicked, the normalized X position (0-1) that was clicked
    pub clicked_position: Option<f32>,
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

/// State for the Levels controls
///
/// Note: In fractal flames, density represents opacity/coverage, NOT brightness:
/// - Low density = transparent → shows BACKGROUND color (could be any color!)
/// - High density = opaque → shows FRACTAL color from palette (could also be any color!)
///
/// A black palette on white background would show white in sparse areas.
/// A bright palette on black background would show black in sparse areas.
///
/// The "black/white" terminology matches Photoshop's Levels UI convention for familiarity,
/// but the actual effect is on transparency (background vs fractal), not darkness/lightness.
#[derive(Clone, Debug)]
pub struct LevelsState {
    /// Input low threshold (density) - pixels below this show background color
    /// UI shows as "Black" to match Photoshop Levels convention
    pub input_black: f32,
    /// Input high threshold (density) - pixels above this show full fractal color
    /// UI shows as "White" to match Photoshop Levels convention
    pub input_white: f32,
    /// Midpoint/gamma (0-1, where 0.5 = linear)
    /// Controls how density values between low and high are mapped
    pub midpoint: f32,
    /// Whether auto-levels is enabled
    pub auto_enabled: bool,
}

impl Default for LevelsState {
    fn default() -> Self {
        Self {
            input_black: 0.0,
            input_white: 1000.0, // Will be auto-set from histogram
            midpoint: 0.5,       // Linear (gamma = 1.0)
            auto_enabled: true,
        }
    }
}

impl LevelsState {
    /// Create new levels state
    pub fn new() -> Self {
        Self::default()
    }

    /// Update from histogram percentiles (auto-levels)
    pub fn update_from_histogram(&mut self, histogram: &DensityHistogram) {
        if !histogram.valid || !self.auto_enabled {
            return;
        }

        // Use 1st and 99th percentiles for auto black/white points
        self.input_black = histogram.percentile_1;
        self.input_white = histogram.percentile_99;
    }

    /// Convert midpoint position (0-1) to gamma value
    /// midpoint=0.5 -> gamma=1.0 (linear)
    /// midpoint<0.5 -> gamma>1.0 (darken midtones)
    /// midpoint>0.5 -> gamma<1.0 (brighten midtones)
    pub fn gamma(&self) -> f32 {
        // Map 0-1 to gamma range (approximately 0.1 to 10.0)
        // Using: gamma = (1 - midpoint) / midpoint for midpoint in (0, 1)
        // This gives gamma=1 at midpoint=0.5
        let clamped = self.midpoint.clamp(0.01, 0.99);
        (1.0 - clamped) / clamped
    }

    /// Set gamma and update midpoint
    pub fn set_gamma(&mut self, gamma: f32) {
        // Inverse of gamma(): midpoint = 1 / (1 + gamma)
        self.midpoint = 1.0 / (1.0 + gamma.clamp(0.1, 10.0));
    }
}

/// Render Levels controls (sliders for black/white/midpoint)
pub fn render_levels_controls(
    ui: &mut egui::Ui,
    levels: &mut LevelsState,
    histogram: &DensityHistogram,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label("Levels");
        if ui.checkbox(&mut levels.auto_enabled, "Auto").changed() {
            changed = true;
            if levels.auto_enabled {
                levels.update_from_histogram(histogram);
            }
        }
    });

    ui.add_enabled_ui(!levels.auto_enabled, |ui| {
        // Black point slider
        ui.horizontal(|ui| {
            ui.label("Black:");
            let range = 0.0..=histogram.max_density.max(100.0);
            if ui.add(super::VkbSlider::new(&mut levels.input_black, range).logarithmic(true)).changed() {
                changed = true;
            }
        });

        // White point slider
        ui.horizontal(|ui| {
            ui.label("White:");
            let range = 0.0..=histogram.max_density.max(100.0);
            if ui.add(super::VkbSlider::new(&mut levels.input_white, range).logarithmic(true)).changed() {
                changed = true;
            }
        });
    });

    // Midpoint/gamma slider (always enabled)
    ui.horizontal(|ui| {
        ui.label("Gamma:");
        let mut gamma = levels.gamma();
        if ui.add(super::VkbSlider::new(&mut gamma, 0.1..=10.0).logarithmic(true)).changed() {
            levels.set_gamma(gamma);
            changed = true;
        }
    });

    changed
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
    let levels_low = config.levels_low;
    let levels_high = config.levels_high;
    let levels_gamma = config.levels_gamma;

    ui.horizontal(|ui| {
        ui.label(t!("levels.title"));

        // Auto button - one-shot apply histogram percentiles
        ui.add_enabled_ui(histogram.valid, |ui| {
            if ui.button(t!("levels.auto")).clicked() {
                // Set low to 1st percentile, high to 99th percentile
                if let Ok(update) = config_manager.update_param(
                    ConfigPath::LevelsLow,
                    histogram.percentile_1.into()
                ) {
                    max_update = max_update.max(update);
                }
                if let Ok(update) = config_manager.update_param(
                    ConfigPath::LevelsHigh,
                    histogram.percentile_99.into()
                ) {
                    max_update = max_update.max(update);
                }
            }
        });
    });

    // Low threshold slider (density threshold for background)
    ui.horizontal(|ui| {
        ui.label(t!("levels.low"));
        let range = 0.0..=histogram.max_density.max(100.0);
        let mut temp_low = levels_low;
        if ui.add(super::VkbSlider::new(&mut temp_low, range).logarithmic(true)
            .clamping(egui::SliderClamping::Never)).changed() {
            if let Ok(update) = config_manager.update_param(ConfigPath::LevelsLow, temp_low.into()) {
                max_update = max_update.max(update);
            }
        }
    });

    // High threshold slider (density threshold for full opacity)
    ui.horizontal(|ui| {
        ui.label(t!("levels.high"));
        let range = 0.0..=histogram.max_density.max(100.0);
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

    max_update
}
