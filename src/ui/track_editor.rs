//! Animation track editor UI
//!
//! Provides controls for adding, editing, and deleting animation tracks.
//!
//! Phase 2 adds visual timeline bars for tracks aligned with the scrubber.
//! Phase 3 adds interactions: click to seek, keyframe hover/click.

use egui::{Ui, Color32, Rect, Pos2, Stroke, CornerRadius, Sense, ScrollArea};
use rust_i18n::t;
use crate::animation::{
    Animation, AnimationController, CircularTrack, EasingFunction, Interpolation,
    Keyframe, OscillatorType, Track, TrackSource,
};
use crate::config::{ConfigPath, FractalConfig};
use crate::effects::{global_effect_registry, EffectInstance};
use crate::scene::transforms::Flame;
use crate::variations::global_registry;
use super::animation_panel::TimelineLayout;
use super::target_selector::{TargetSelectorState, render_target_selector};

/// UI state for track editor
#[derive(Default)]
pub struct TrackEditorState {
    /// Selected track type for new track
    pub new_track_type: NewTrackType,
    /// Selected target for new track
    pub new_track_target: String,
    /// Selected second target for circular tracks
    pub new_track_target_y: String,
    /// Unified Track Editor panel state
    pub track_editor_panel_open: bool,
    /// Track index being edited in the unified panel (None = adding new track)
    pub editing_track_index: Option<usize>,
    /// Target selector state for the unified panel
    pub target_selector_state: TargetSelectorState,
    /// Target selector state for Y axis (circular tracks)
    pub target_selector_state_y: TargetSelectorState,
    /// Oscillator parameters for editing
    pub oscillator_params: OscillatorParams,
    /// Circular track parameters for editing
    pub circular_params: CircularParams,
    /// Preview keyframes for Add Track mode (before track is created)
    pub preview_keyframes: Vec<Keyframe>,
    /// Interpolation mode for preview keyframes
    pub preview_interpolation: Interpolation,
}

/// Oscillator parameters for track editor
#[derive(Clone)]
pub struct OscillatorParams {
    pub oscillator_type: OscillatorType,
    pub center: f64,
    pub amplitude: f64,
    pub frequency: f64,
    pub phase: f64,
}

impl Default for OscillatorParams {
    fn default() -> Self {
        Self {
            oscillator_type: OscillatorType::Sine,
            center: 0.0,
            amplitude: 1.0,
            frequency: 0.5,
            phase: 0.0,
        }
    }
}

/// Circular track parameters for track editor
#[derive(Clone)]
pub struct CircularParams {
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub speed: f64,
    pub phase: f64,
}

impl Default for CircularParams {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            radius: 0.5,
            speed: 0.1,
            phase: 0.0,
        }
    }
}

/// Response from track editor rendering (Phase 3)
#[derive(Default)]
pub struct TrackEditorResponse {
    /// User clicked on timeline/track area - seek to this time
    pub seek_to_time: Option<f64>,
}

/// Type of track to add
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum NewTrackType {
    #[default]
    Keyframe,
    Oscillator,
    Circular,
}

/// Category of animatable parameters
struct ParameterCategory {
    name: String,
    params: Vec<(String, String)>, // (display_name, path_string)
}

/// Convert internal 0-based path string to 1-based display string
///
/// Internal paths use 0-based indices for backward compatibility with saved files,
/// but UI displays 1-based indices to match the rest of the application.
fn path_to_display(path: &str) -> String {
    // Match "Transform.N." pattern and increment the number
    if let Some(rest) = path.strip_prefix("Transform.") {
        if let Some(dot_pos) = rest.find('.') {
            if let Ok(index) = rest[..dot_pos].parse::<usize>() {
                return format!("Transform.{}.{}", index + 1, &rest[dot_pos + 1..]);
            }
        }
    }
    // Return unchanged for non-transform paths
    path.to_string()
}

/// Get list of animatable parameters organized by category
fn animatable_parameters(config: &FractalConfig) -> Vec<ParameterCategory> {
    let registry = global_registry();
    let effect_registry = global_effect_registry();
    let flame = &config.flame;
    let transform_count = flame.transforms.len();

    let mut categories = vec![
        ParameterCategory {
            name: t!("track_editor.category_view").to_string(),
            params: vec![
                (t!("track_editor.param_zoom").to_string(), "Zoom".to_string()),
                (t!("track_editor.param_pan_x").to_string(), "PanX".to_string()),
                (t!("track_editor.param_pan_y").to_string(), "PanY".to_string()),
                (t!("track_editor.param_rotation").to_string(), "Rotation".to_string()),
                (t!("track_editor.param_camera_rotation_x").to_string(), "CameraRotationX".to_string()),
                (t!("track_editor.param_camera_rotation_y").to_string(), "CameraRotationY".to_string()),
                (t!("track_editor.param_camera_z").to_string(), "CameraZ".to_string()),
            ],
        },
        ParameterCategory {
            name: t!("track_editor.category_tone_mapping").to_string(),
            params: vec![
                (t!("track_editor.param_exposure").to_string(), "Exposure".to_string()),
                (t!("track_editor.param_gamma").to_string(), "Gamma".to_string()),
                (t!("track_editor.param_gamma_threshold").to_string(), "GammaThreshold".to_string()),
                (t!("track_editor.param_brightness").to_string(), "Brightness".to_string()),
                (t!("track_editor.param_vibrancy").to_string(), "Vibrancy".to_string()),
                (t!("track_editor.param_saturation").to_string(), "Saturation".to_string()),
                (t!("track_editor.param_hue_shift").to_string(), "HueShift".to_string()),
                (t!("track_editor.param_value_scale").to_string(), "ValueScale".to_string()),
                (t!("track_editor.param_alpha_blend_low").to_string(), "AlphaBlendLow".to_string()),
                (t!("track_editor.param_alpha_blend_high").to_string(), "AlphaBlendHigh".to_string()),
                (t!("track_editor.param_density_scale").to_string(), "DensityScale".to_string()),
            ],
        },
        ParameterCategory {
            name: t!("track_editor.category_color").to_string(),
            params: vec![
                (t!("track_editor.param_palette_rotation").to_string(), "PaletteRotation".to_string()),
                (t!("track_editor.param_speed_factor").to_string(), "SpeedFactor".to_string()),
                (t!("track_editor.param_histogram_color_scale").to_string(), "HistogramColorScale".to_string()),
                (t!("track_editor.param_background_r").to_string(), "BackgroundColorR".to_string()),
                (t!("track_editor.param_background_g").to_string(), "BackgroundColorG".to_string()),
                (t!("track_editor.param_background_b").to_string(), "BackgroundColorB".to_string()),
            ],
        },
        ParameterCategory {
            name: t!("track_editor.category_rendering").to_string(),
            params: vec![
                (t!("track_editor.param_blend_factor").to_string(), "BlendFactor".to_string()),
                (t!("track_editor.param_perspective_strength").to_string(), "PerspectiveStrength".to_string()),
                (t!("track_editor.param_low_density_smoothing").to_string(), "LowDensitySmoothing".to_string()),
            ],
        },
    ];

    // Add transform-specific parameters
    for i in 0..transform_count {
        let transform = &flame.transforms[i];

        let mut params = vec![
            (t!("track_editor.param_weight").to_string(), format!("Transform.{}.Weight", i)),
            (t!("track_editor.param_color").to_string(), format!("Transform.{}.Color", i)),
            (t!("track_editor.param_color_speed").to_string(), format!("Transform.{}.ColorSpeed", i)),
            (t!("track_editor.param_opacity").to_string(), format!("Transform.{}.Opacity", i)),
        ];

        // High-level transform operations (translate, rotate, scale)
        params.push((t!("track_editor.param_origin_x").to_string(), format!("Transform.{}.OriginX", i)));
        params.push((t!("track_editor.param_origin_y").to_string(), format!("Transform.{}.OriginY", i)));
        params.push((t!("track_editor.param_rotation").to_string(), format!("Transform.{}.Rotation", i)));
        params.push((t!("track_editor.param_scale").to_string(), format!("Transform.{}.Scale", i)));

        // Raw affine parameters (for advanced users)
        for param in ['A', 'B', 'C', 'D', 'E', 'F', 'G'] {
            params.push((t!("track_editor.param_affine", param = param).to_string(), format!("Transform.{}.Affine.{}", i, param)));
        }

        // Active variations for this transform
        let mut active_variations: Vec<(&String, &f32)> = transform.variations
            .iter()
            .filter(|(_, weight)| **weight != 0.0)
            .collect();
        // Sort by name for consistent ordering
        active_variations.sort_by_key(|(name, _)| *name);

        for (var_name, _weight) in &active_variations {
            // Add variation weight
            let display_name = registry.get(var_name)
                .map(|v| v.display_name.clone())
                .unwrap_or_else(|| var_name.to_string());
            params.push((
                t!("track_editor.param_variation_weight", name = display_name.as_str()).to_string(),
                format!("Transform.{}.Variation.{}", i, var_name),
            ));

            // Add variation parameters if any
            if let Some(var_info) = registry.get(var_name) {
                for param_def in &var_info.parameters {
                    params.push((
                        t!("track_editor.param_variation_param", name = display_name.as_str(), param = param_def.display_name.as_str()).to_string(),
                        format!("Transform.{}.VariationParam.{}.{}", i, var_name, param_def.name),
                    ));
                }
            }
        }

        categories.push(ParameterCategory {
            name: t!("track_editor.category_transform", index = i + 1).to_string(),
            params,
        });
    }

    // Add Final Transform category if final transform is enabled
    if let Some(ref final_xform) = flame.final_transform {
        let mut params = vec![
            (t!("track_editor.param_origin_x").to_string(), "FinalTransform.OriginX".to_string()),
            (t!("track_editor.param_origin_y").to_string(), "FinalTransform.OriginY".to_string()),
            (t!("track_editor.param_rotation").to_string(), "FinalTransform.Rotation".to_string()),
            (t!("track_editor.param_scale").to_string(), "FinalTransform.Scale".to_string()),
            (t!("track_editor.param_color").to_string(), "FinalTransform.Color".to_string()),
            (t!("track_editor.param_color_speed").to_string(), "FinalTransform.ColorSpeed".to_string()),
        ];

        // Raw affine parameters (for advanced users)
        for param in ['A', 'B', 'C', 'D', 'E', 'F', 'G'] {
            params.push((t!("track_editor.param_affine", param = param).to_string(), format!("FinalTransform.Affine.{}", param)));
        }

        // Active variations for final transform
        let mut active_variations: Vec<(&String, &f32)> = final_xform.variations
            .iter()
            .filter(|(_, weight)| **weight != 0.0)
            .collect();
        active_variations.sort_by_key(|(name, _)| *name);

        for (var_name, _weight) in &active_variations {
            let display_name = registry.get(var_name)
                .map(|v| v.display_name.clone())
                .unwrap_or_else(|| var_name.to_string());
            params.push((
                t!("track_editor.param_variation_weight", name = display_name.as_str()).to_string(),
                format!("FinalTransform.Variation.{}", var_name),
            ));

            if let Some(var_info) = registry.get(var_name) {
                for param_def in &var_info.parameters {
                    params.push((
                        t!("track_editor.param_variation_param", name = display_name.as_str(), param = param_def.display_name.as_str()).to_string(),
                        format!("FinalTransform.VariationParam.{}.{}", var_name, param_def.name),
                    ));
                }
            }
        }

        categories.push(ParameterCategory {
            name: t!("track_editor.category_final_transform").to_string(),
            params,
        });
    }

    // Add Density Effects category
    if !config.density_effects.is_empty() {
        let mut params = Vec::new();
        for (i, effect) in config.density_effects.iter().enumerate() {
            add_effect_params(&mut params, effect, i, "DensityEffect", &effect_registry);
        }
        if !params.is_empty() {
            categories.push(ParameterCategory {
                name: t!("track_editor.category_density_effects").to_string(),
                params,
            });
        }
    }

    // Add Color Effects category
    if !config.color_effects.is_empty() {
        let mut params = Vec::new();
        for (i, effect) in config.color_effects.iter().enumerate() {
            add_effect_params(&mut params, effect, i, "ColorEffect", &effect_registry);
        }
        if !params.is_empty() {
            categories.push(ParameterCategory {
                name: t!("track_editor.category_color_effects").to_string(),
                params,
            });
        }
    }

    categories
}

/// Helper to add effect parameters to the params list
fn add_effect_params(
    params: &mut Vec<(String, String)>,
    effect: &EffectInstance,
    index: usize,
    prefix: &str,
    registry: &crate::effects::EffectRegistry,
) {
    if let Some(info) = registry.get(&effect.effect_type) {
        let effect_name = info.translated_name();
        // Add enabled toggle (note: "Enabled" with capital E to match ConfigPath parsing)
        params.push((
            format!("{} → Enabled", effect_name),
            format!("{}.{}.Enabled", prefix, index),
        ));

        // Add each parameter
        for param_def in &info.parameters {
            params.push((
                format!("{} → {}", effect_name, info.translated_param_name(&param_def.name)),
                format!("{}.{}.{}", prefix, index, param_def.name),
            ));
        }
    }
}

/// Get the current value of a parameter from the config based on track path
/// Returns None if the path is not recognized or the value cannot be extracted
fn get_parameter_value(config: &FractalConfig, path: &str) -> Option<f64> {
    let flame = &config.flame;

    // Direct config fields
    match path {
        "Zoom" => return Some(config.zoom as f64),
        "PanX" => return Some(config.pan_x as f64),
        "PanY" => return Some(config.pan_y as f64),
        "Rotation" => return Some(config.rotation as f64),
        "CameraRotationX" => return Some(config.camera_rotation_x as f64),
        "CameraRotationY" => return Some(config.camera_rotation_y as f64),
        "CameraZ" => return Some(config.camera_z as f64),
        "Exposure" => return Some(config.exposure as f64),
        "Gamma" => return Some(config.gamma as f64),
        "GammaThreshold" => return Some(config.gamma_threshold as f64),
        "Brightness" => return Some(config.brightness as f64),
        "Vibrancy" => return Some(config.vibrancy as f64),
        "Saturation" => return Some(config.saturation as f64),
        "HueShift" => return Some(config.hue_shift as f64),
        "AlphaBlendLow" => return Some(config.alpha_blend_low as f64),
        "AlphaBlendHigh" => return Some(config.alpha_blend_high as f64),
        "DensityScale" => return Some(config.density_scale as f64),
        "PaletteRotation" => return Some(config.palette_rotation as f64),
        "SpeedFactor" => return Some(config.speed_factor as f64),
        "HistogramColorScale" => return Some(config.histogram_color_scale as f64),
        "BackgroundColorR" => return Some(config.background_color[0] as f64),
        "BackgroundColorG" => return Some(config.background_color[1] as f64),
        "BackgroundColorB" => return Some(config.background_color[2] as f64),
        "BlendFactor" => return Some(config.blend_factor as f64),
        "PerspectiveStrength" => return Some(flame.perspective_strength as f64),
        _ => {}
    }

    // Parse Transform paths: Transform.{index}.{field}
    if path.starts_with("Transform.") {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() >= 3 {
            if let Ok(idx) = parts[1].parse::<usize>() {
                if idx < flame.transforms.len() {
                    let transform = &flame.transforms[idx];
                    match parts[2] {
                        "Weight" => return Some(transform.weight as f64),
                        "Color" => return Some(transform.color as f64),
                        "ColorSpeed" => return Some(transform.color_speed as f64),
                        "Opacity" => return Some(transform.opacity as f64),
                        "Affine" if parts.len() >= 4 => {
                            return match parts[3] {
                                "A" => Some(transform.a as f64),
                                "B" => Some(transform.b as f64),
                                "C" => Some(transform.c as f64),
                                "D" => Some(transform.d as f64),
                                "E" => Some(transform.e as f64),
                                "F" => Some(transform.f as f64),
                                "G" => Some(transform.g as f64),
                                _ => None,
                            };
                        }
                        "Variation" if parts.len() >= 4 => {
                            let var_name = parts[3];
                            return transform.variations.get(var_name).map(|w| *w as f64);
                        }
                        "VariationParam" if parts.len() >= 5 => {
                            let var_name = parts[3];
                            let param_name = parts[4];
                            // variation_params uses flat keys: "variation_name.param_name"
                            let key = format!("{}.{}", var_name, param_name);
                            return transform.variation_params.get(&key).map(|v| *v as f64);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Default: value not found
    None
}

/// Render the track editor section
///
/// If `timeline_layout` is provided, renders visual track bars aligned with the scrubber.
/// Otherwise, renders the traditional inline track controls.
///
/// Returns a response containing any seek requests from clicking on tracks.
pub fn render_track_editor(
    ui: &mut Ui,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    config: &FractalConfig,
    timeline_layout: Option<TimelineLayout>,
) -> TrackEditorResponse {
    let has_animation = controller.animation.is_some();
    let mut response = TrackEditorResponse::default();

    // Track list header with Add Track button
    let track_count = controller.animation.as_ref()
        .map(|a| a.tracks.len() + a.circular_tracks.len())
        .unwrap_or(0);

    ui.horizontal(|ui| {
        ui.strong(t!("track_editor.tracks_header", count = track_count));
        ui.separator();
        let add_button = egui::Button::new(t!("track_editor.add_track"))
            .fill(egui::Color32::from_rgb(60, 120, 60));
        if ui.add_enabled(has_animation, add_button).clicked() {
            open_add_track_panel(state);
        }
    });

    ui.separator();

    // Render tracks - visual bars if timeline layout provided, otherwise inline controls
    if let Some(ref mut animation) = controller.animation {
        if let Some(layout) = timeline_layout {
            response = render_tracks_visual(ui, animation, state, layout);
        } else {
            render_tracks(ui, animation, state);
        }
    }

    response
}

/// Render tracks as visual timeline bars (Phase 2+3)
///
/// Layout per track:
/// ```text
/// Label        |--●====●========●---|     [Edit] [Delete]
///              ^  ^    ^        ^   ^
///              |  |    keyframe |   |
///              |  first_kf      |   last_kf
///              timeline_start   timeline_end
/// ```
///
/// Phase 3 interactions:
/// - Click on bar area to seek to that time
/// - Hover on keyframe dot to see value tooltip
/// - Click on keyframe dot to open editor for that track
fn render_tracks_visual(
    ui: &mut Ui,
    animation: &mut Animation,
    state: &mut TrackEditorState,
    layout: TimelineLayout,
) -> TrackEditorResponse {
    let mut track_to_delete: Option<usize> = None;
    let mut response = TrackEditorResponse::default();

    // Constants for visual rendering
    const TRACK_HEIGHT: f32 = 24.0;
    const LABEL_WIDTH: f32 = 100.0;
    const BUTTON_WIDTH: f32 = 80.0;
    const KEYFRAME_RADIUS: f32 = 5.0;
    const KEYFRAME_HIT_RADIUS: f32 = 8.0; // Larger hit area for easier clicking
    const BAR_HEIGHT: f32 = 8.0;

    // Colors
    let bar_color = Color32::from_rgb(80, 120, 180);
    let bar_bg_color = Color32::from_gray(60);
    let bar_hover_color = Color32::from_gray(75); // Subtle hover effect
    let keyframe_color = Color32::from_rgb(255, 200, 100);
    let keyframe_hover_color = Color32::from_rgb(255, 255, 150);
    let position_line_color = Color32::from_rgb(255, 80, 80);

    // Calculate position line X
    let position_x = layout.position_x();

    // Track all track rects for position line drawing
    let mut all_track_rects: Vec<Rect> = Vec::new();

    // Store bar area info for later position line calculation
    let mut bar_area_info: Option<(f32, f32, f32)> = None; // (bar_left, bar_right, bar_scale)

    for (track_index, track) in animation.tracks.iter().enumerate() {
        let path = track.target.clone();
        {
            // Get time range for this track
            let (first_time, last_time) = match &track.source {
                TrackSource::Keyframes { keyframes } => {
                    if keyframes.is_empty() {
                        (0.0, layout.duration)
                    } else {
                        let first = keyframes.first().map(|k| k.time).unwrap_or(0.0);
                        let last = keyframes.last().map(|k| k.time).unwrap_or(layout.duration);
                        (first, last)
                    }
                }
                TrackSource::Oscillator { .. } => {
                    // Oscillators span full duration
                    (0.0, layout.duration)
                }
            };

            // Allocate full row with click sensing
            let (rect, row_response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), TRACK_HEIGHT),
                Sense::click(),
            );

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();

                // Calculate bar area (between label and buttons)
                let bar_left = rect.left() + LABEL_WIDTH;
                let bar_right = rect.right() - BUTTON_WIDTH;
                let bar_scale = if (layout.bar_right - layout.bar_left).abs() > 0.001 {
                    (bar_right - bar_left) / (layout.bar_right - layout.bar_left)
                } else {
                    1.0
                };

                // Store bar area info for position line
                if bar_area_info.is_none() {
                    bar_area_info = Some((bar_left, bar_right, bar_scale));
                }

                // Background bar (full timeline extent) - with hover highlight
                let bg_rect = Rect::from_min_max(
                    Pos2::new(bar_left, rect.center().y - BAR_HEIGHT / 2.0),
                    Pos2::new(bar_right, rect.center().y + BAR_HEIGHT / 2.0),
                );
                let bg_color = if row_response.hovered() { bar_hover_color } else { bar_bg_color };
                painter.rect_filled(bg_rect, CornerRadius::same(2), bg_color);

                // Track bar (from first to last keyframe)
                let adjusted_bar_left = bar_left + (layout.time_to_x(first_time) - layout.bar_left) * bar_scale;
                let adjusted_bar_right = bar_left + (layout.time_to_x(last_time) - layout.bar_left) * bar_scale;

                let track_rect = Rect::from_min_max(
                    Pos2::new(adjusted_bar_left.max(bar_left).min(bar_right), rect.center().y - BAR_HEIGHT / 2.0),
                    Pos2::new(adjusted_bar_right.max(bar_left).min(bar_right), rect.center().y + BAR_HEIGHT / 2.0),
                );
                painter.rect_filled(track_rect, CornerRadius::same(2), bar_color);

                // Keyframe dots with hover/click interaction
                let mut clicked_keyframe: Option<usize> = None;
                let pointer_pos = ui.ctx().pointer_hover_pos();

                if let TrackSource::Keyframes { keyframes } = &track.source {
                    for (kf_idx, keyframe) in keyframes.iter().enumerate() {
                        let kf_x = bar_left + (layout.time_to_x(keyframe.time) - layout.bar_left) * bar_scale;
                        if kf_x >= bar_left && kf_x <= bar_right {
                            let kf_pos = Pos2::new(kf_x, rect.center().y);

                            // Check if mouse is hovering over this keyframe
                            let is_hovered = pointer_pos.map_or(false, |pos| {
                                (pos - kf_pos).length() <= KEYFRAME_HIT_RADIUS
                            });

                            // Draw keyframe dot (highlighted if hovered)
                            let color = if is_hovered { keyframe_hover_color } else { keyframe_color };
                            painter.circle_filled(kf_pos, KEYFRAME_RADIUS, color);
                            // Add subtle outline on hover
                            if is_hovered {
                                painter.circle_stroke(kf_pos, KEYFRAME_RADIUS + 1.0, Stroke::new(1.5, Color32::WHITE));
                            }

                            // Show tooltip on hover using manual tooltip window
                            if is_hovered {
                                let value_str = keyframe.value.as_f64()
                                    .map(|v| format!("{:.3}", v))
                                    .unwrap_or_else(|| format!("{}", keyframe.value));

                                // Create a tooltip area near the pointer
                                let tooltip_id = egui::Id::new(format!("kf_tooltip_{}_{}", path, kf_idx));
                                #[allow(deprecated)]
                                egui::containers::show_tooltip(
                                    ui.ctx(),
                                    egui::LayerId::new(egui::Order::Tooltip, tooltip_id),
                                    tooltip_id,
                                    |ui| {
                                        ui.label(format!(
                                            "{}: {} @ {:.2}s",
                                            path, value_str, keyframe.time
                                        ));
                                    },
                                );

                                // Check for click on keyframe
                                if row_response.clicked() {
                                    clicked_keyframe = Some(kf_idx);
                                }
                            }
                        }
                    }
                }

                // Handle keyframe click - open Track Editor panel
                if let Some(_kf_idx) = clicked_keyframe {
                    open_edit_track_panel(state, track_index, track);
                }
                // Handle bar area click (not on keyframe) - seek to that time
                else if row_response.clicked() {
                    if let Some(pos) = pointer_pos {
                        if pos.x >= bar_left && pos.x <= bar_right {
                            // Convert click position to time
                            let t = (pos.x - bar_left) / (bar_right - bar_left);
                            let time = t as f64 * layout.duration;
                            response.seek_to_time = Some(time);
                        }
                    }
                }

                // Show pointer cursor when hovering over the bar area
                if row_response.hovered() {
                    if let Some(pos) = pointer_pos {
                        if pos.x >= bar_left && pos.x <= bar_right {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                }

                // Draw label on the left with tooltip for full name
                let label_rect = Rect::from_min_max(
                    rect.left_top(),
                    Pos2::new(rect.left() + LABEL_WIDTH - 4.0, rect.bottom()),
                );
                // Convert to 1-based display (internal paths use 0-based for compatibility)
                let display_path = path_to_display(&path);
                // Truncate label if too long (show more characters)
                let display_name = if display_path.len() > 14 {
                    format!("{}...", &display_path[..14])
                } else {
                    display_path.clone()
                };
                painter.text(
                    label_rect.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    &display_name,
                    egui::FontId::proportional(12.0),
                    ui.visuals().text_color(),
                );
                // Add tooltip with full track name on hover
                let label_response = ui.interact(label_rect, egui::Id::new(format!("track_label_{}", track_index)), Sense::hover());
                if label_response.hovered() {
                    label_response.on_hover_text(&display_path);
                }

                all_track_rects.push(bg_rect);
            }

            // Buttons on the right (rendered in UI layer for interaction)
            ui.allocate_ui_at_rect(
                Rect::from_min_max(
                    Pos2::new(rect.right() - BUTTON_WIDTH, rect.top()),
                    rect.right_bottom(),
                ),
                |ui| {
                    ui.horizontal_centered(|ui| {
                        if ui.small_button(t!("track_editor.edit")).clicked() {
                            open_edit_track_panel(state, track_index, track);
                        }
                        if ui.small_button("X").on_hover_text(t!("track_editor.delete_track")).clicked() {
                            track_to_delete = Some(track_index);
                        }
                    });
                },
            );
        } // end of ui.is_rect_visible block
    }

    // Draw position line through all tracks
    if !all_track_rects.is_empty() {
        if let Some((bar_left, bar_right, bar_scale)) = bar_area_info {
            let painter = ui.painter();
            let first_rect = all_track_rects.first().unwrap();
            let last_rect = all_track_rects.last().unwrap();

            // Calculate position line X in the bar area
            let pos_x = bar_left + (position_x - layout.bar_left) * bar_scale;

            if pos_x >= bar_left && pos_x <= bar_right {
                painter.line_segment(
                    [
                        Pos2::new(pos_x, first_rect.top() - 4.0),
                        Pos2::new(pos_x, last_rect.bottom() + 4.0),
                    ],
                    Stroke::new(2.0, position_line_color),
                );
            }
        }
    }

    // Delete track if requested
    if let Some(index) = track_to_delete {
        animation.remove_track(index);
    }

    // Circular tracks (rendered as simple rows for now)
    let mut circular_to_delete: Option<usize> = None;

    for (i, circular) in animation.circular_tracks.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let label = format!("{}, {}", circular.target_x, circular.target_y);
            ui.label(&label);
            ui.label(t!("track_editor.circular_label"));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("X").on_hover_text(t!("track_editor.delete_track")).clicked() {
                    circular_to_delete = Some(i);
                }
            });
        });
    }

    // Delete circular track if requested
    if let Some(i) = circular_to_delete {
        animation.remove_circular_track(i);
    }

    response
}

/// Render the list of existing tracks
fn render_tracks(ui: &mut Ui, animation: &mut Animation, state: &mut TrackEditorState) {
    let mut track_to_delete: Option<usize> = None;
    let track_count = animation.tracks.len();

    for track_index in 0..track_count {
        let track = &mut animation.tracks[track_index];
        let path = track.target.clone();

        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(4.0)
            .corner_radius(2.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Track name and type
                    let track_type_str = match &track.source {
                        TrackSource::Keyframes { keyframes } => t!("track_editor.keyframes_count", count = keyframes.len()).to_string(),
                        TrackSource::Oscillator { oscillator_type, .. } => format!("{:?}", oscillator_type),
                    };
                    ui.strong(&path);
                    ui.label(format!("({})", track_type_str));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").on_hover_text(t!("track_editor.delete_track").as_ref()).clicked() {
                            track_to_delete = Some(track_index);
                        }
                    });
                });

                // Track-specific controls
                match &mut track.source {
                    TrackSource::Keyframes { .. } => {
                        ui.horizontal(|ui| {
                            ui.label(t!("track_editor.interpolation"));
                            egui::ComboBox::from_id_salt(format!("interp_{}", track_index))
                                .selected_text(format!("{:?}", track.interpolation))
                                .width(100.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut track.interpolation, Interpolation::Step, t!("track_editor.interpolation_step").as_ref());
                                    ui.selectable_value(&mut track.interpolation, Interpolation::Linear, t!("track_editor.interpolation_linear").as_ref());
                                    ui.selectable_value(&mut track.interpolation, Interpolation::Smooth, t!("track_editor.interpolation_smooth").as_ref());
                                    ui.selectable_value(&mut track.interpolation, Interpolation::Sinusoidal, t!("track_editor.interpolation_sinusoidal").as_ref());
                                });
                        });
                    }
                    TrackSource::Oscillator { oscillator_type, center, amplitude, frequency, phase } => {
                        ui.horizontal(|ui| {
                            ui.label(t!("track_editor.osc_type"));
                            egui::ComboBox::from_id_salt(format!("osc_type_{}", track_index))
                                .selected_text(format!("{:?}", oscillator_type))
                                .width(80.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(oscillator_type, OscillatorType::Sine, t!("track_editor.osc_sine").as_ref());
                                    ui.selectable_value(oscillator_type, OscillatorType::Triangle, t!("track_editor.osc_triangle").as_ref());
                                    ui.selectable_value(oscillator_type, OscillatorType::Sawtooth, t!("track_editor.osc_sawtooth").as_ref());
                                    ui.selectable_value(oscillator_type, OscillatorType::Square, t!("track_editor.osc_square").as_ref());
                                });
                        });

                        ui.horizontal(|ui| {
                            ui.label(t!("track_editor.center"));
                            ui.add(egui::DragValue::new(center).speed(0.01));
                            ui.label(t!("track_editor.amplitude"));
                            ui.add(egui::DragValue::new(amplitude).speed(0.01));
                        });

                        ui.horizontal(|ui| {
                            ui.label(t!("track_editor.frequency"));
                            ui.add(egui::DragValue::new(frequency).speed(0.01).suffix(" Hz"));
                            ui.label(t!("track_editor.phase"));
                            ui.add(egui::DragValue::new(phase).speed(0.01));
                        });
                    }
                }
            });

        ui.add_space(4.0);
    }

    // Delete track if requested
    if let Some(index) = track_to_delete {
        animation.remove_track(index);
    }

    // Circular tracks
    let mut circular_to_delete: Option<usize> = None;

    for (i, circular) in animation.circular_tracks.iter_mut().enumerate() {
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(4.0)
            .corner_radius(2.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("{}, {}", circular.target_x, circular.target_y));
                    ui.label(t!("track_editor.circular_label"));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").on_hover_text(t!("track_editor.delete_track").as_ref()).clicked() {
                            circular_to_delete = Some(i);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(t!("track_editor.center"));
                    ui.add(egui::DragValue::new(&mut circular.center_x).speed(0.01).prefix("X: "));
                    ui.add(egui::DragValue::new(&mut circular.center_y).speed(0.01).prefix("Y: "));
                });

                ui.horizontal(|ui| {
                    ui.label(t!("track_editor.radius"));
                    ui.add(egui::DragValue::new(&mut circular.radius).speed(0.01));
                    ui.label(t!("track_editor.speed"));
                    ui.add(egui::DragValue::new(&mut circular.speed).speed(0.01).suffix(" rev/s"));
                });

                ui.horizontal(|ui| {
                    ui.label(t!("track_editor.phase"));
                    ui.add(egui::DragValue::new(&mut circular.phase).speed(0.01).suffix(" rad"));
                });
            });

        ui.add_space(4.0);
    }

    // Delete circular track if requested
    if let Some(i) = circular_to_delete {
        animation.remove_circular_track(i);
    }
}

// ============================================================================
// UNIFIED TRACK EDITOR PANEL (Phase 7)
// ============================================================================

/// Render the unified Track Editor panel as a window
///
/// This panel combines Add Track and Edit Track functionality:
/// - Hierarchical target selector
/// - Track type selection (Keyframes, Oscillator, Circular)
/// - Type-specific parameter subpanels
/// - Auto-creates/updates track when target and type are valid
pub fn render_track_editor_panel(
    ctx: &egui::Context,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    flame: &Flame,
    config: &FractalConfig,
    current_time: f64,
) {
    if !state.track_editor_panel_open {
        return;
    }

    let is_editing = state.editing_track_index.is_some();
    let title = if is_editing {
        t!("track_editor.edit_track_title")
    } else {
        t!("track_editor.add_track_title")
    };

    let mut open = state.track_editor_panel_open;

    egui::Window::new(title)
        .open(&mut open)
        .resizable(true)
        .default_width(350.0)
        .default_height(450.0)
        .show(ctx, |ui| {
            render_track_editor_panel_content(ui, controller, state, flame, config, current_time);
        });

    // Close if either X button clicked (open=false) or explicitly closed by code
    state.track_editor_panel_open = open && state.track_editor_panel_open;
}

/// Render the content of the Track Editor panel
fn render_track_editor_panel_content(
    ui: &mut Ui,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    flame: &Flame,
    config: &FractalConfig,
    current_time: f64,
) {
    let Some(ref mut animation) = controller.animation else {
        ui.label(t!("track_editor.no_animation"));
        return;
    };

    let is_editing = state.editing_track_index.is_some();
    let duration = animation.duration;

    // =========================================================================
    // TYPE SELECTOR (Add mode only)
    // =========================================================================
    if !is_editing {
        ui.horizontal(|ui| {
            ui.label(t!("track_editor.type"));
            egui::ComboBox::from_id_salt("track_type_selector")
                .selected_text(match state.new_track_type {
                    NewTrackType::Keyframe => t!("track_editor.type_keyframe"),
                    NewTrackType::Oscillator => t!("track_editor.type_oscillator"),
                    NewTrackType::Circular => t!("track_editor.type_circular"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.new_track_type, NewTrackType::Keyframe, t!("track_editor.type_keyframe").as_ref());
                    ui.selectable_value(&mut state.new_track_type, NewTrackType::Oscillator, t!("track_editor.type_oscillator").as_ref());
                    ui.selectable_value(&mut state.new_track_type, NewTrackType::Circular, t!("track_editor.type_circular").as_ref());
                });
        });

        ui.separator();
    }

    // =========================================================================
    // TARGET SELECTOR (Hierarchical or read-only)
    // =========================================================================
    ui.label(t!("track_editor.target"));

    if is_editing {
        // Edit mode: Show target as read-only label
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.strong(&state.new_track_target);
        });
    } else {
        // Add mode: Show editable target selector
        // Show current selection
        if !state.new_track_target.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.strong(&state.new_track_target);
                if ui.small_button("X").clicked() {
                    state.new_track_target.clear();
                }
            });
        }

        // Hierarchical target selector
        egui::CollapsingHeader::new(if state.new_track_target.is_empty() {
            t!("track_editor.select_target")
        } else {
            t!("track_editor.change_target")
        })
        .default_open(state.new_track_target.is_empty())
        .show(ui, |ui| {
            if let Some(path) = render_target_selector(
                ui,
                &mut state.target_selector_state,
                flame,
                config,
                if state.new_track_target.is_empty() { None } else { Some(&state.new_track_target) },
            ) {
                state.new_track_target = path.to_string_key();
                // Auto-initialize values based on track type (Add mode only)
                if !is_editing {
                    match state.new_track_type {
                        NewTrackType::Keyframe => {
                            initialize_preview_keyframes(state, config, duration);
                        }
                        NewTrackType::Oscillator => {
                            initialize_oscillator_center(state, config);
                        }
                        NewTrackType::Circular => {
                            initialize_circular_centers(state, config);
                        }
                    }
                }
            }
        });
    }

    // Second target for Circular tracks
    if state.new_track_type == NewTrackType::Circular {
        ui.separator();
        ui.label(t!("track_editor.target_y"));

        if is_editing {
            // Edit mode: Show target Y as read-only label
            ui.horizontal(|ui| {
                ui.label("→");
                ui.strong(&state.new_track_target_y);
            });
        } else {
            // Add mode: Show editable target Y selector
            if !state.new_track_target_y.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("→");
                    ui.strong(&state.new_track_target_y);
                    if ui.small_button("✕").clicked() {
                        state.new_track_target_y.clear();
                    }
                });
            }

            egui::CollapsingHeader::new(if state.new_track_target_y.is_empty() {
                t!("track_editor.select_target_y")
            } else {
                t!("track_editor.change_target_y")
            })
            .default_open(state.new_track_target_y.is_empty())
            .show(ui, |ui| {
                if let Some(path) = render_target_selector(
                    ui,
                    &mut state.target_selector_state_y,
                    flame,
                    config,
                    if state.new_track_target_y.is_empty() { None } else { Some(&state.new_track_target_y) },
                ) {
                    state.new_track_target_y = path.to_string_key();
                    // Auto-initialize Y center for circular track
                    if !is_editing {
                        initialize_circular_centers(state, config);
                    }
                }
            });
        }
    }

    ui.separator();

    // =========================================================================
    // TYPE-SPECIFIC SUBPANELS
    // =========================================================================
    match state.new_track_type {
        NewTrackType::Keyframe => {
            render_keyframe_subpanel(ui, animation, state, current_time, duration, config);
        }
        NewTrackType::Oscillator => {
            render_oscillator_subpanel(ui, state);
        }
        NewTrackType::Circular => {
            render_circular_subpanel(ui, state);
        }
    }

    // In Edit mode, apply changes immediately after each frame
    // This makes oscillator/circular parameter changes and type switches take effect instantly
    if is_editing {
        update_or_create_track(animation, state, duration, config);
    }

    ui.separator();

    // =========================================================================
    // ACTION BUTTONS (Add mode only)
    // =========================================================================
    if !is_editing {
        let can_create = match state.new_track_type {
            NewTrackType::Circular => !state.new_track_target.is_empty() && !state.new_track_target_y.is_empty(),
            _ => !state.new_track_target.is_empty(),
        };

        let add_button = egui::Button::new(t!("track_editor.add_track"));
        let add_button = if can_create {
            add_button.fill(egui::Color32::from_rgb(60, 120, 60))
        } else {
            add_button
        };
        if ui.add_enabled(can_create, add_button).clicked() {
            // Create the track and close panel
            update_or_create_track(animation, state, duration, config);
            close_track_editor_panel(state);
        }
    }
}

/// Render keyframe-specific options subpanel
fn render_keyframe_subpanel(
    ui: &mut Ui,
    animation: &mut Animation,
    state: &mut TrackEditorState,
    current_time: f64,
    duration: f64,
    config: &FractalConfig,
) {
    ui.label(t!("track_editor.keyframes_section"));

    // If editing an existing keyframe track, show the keyframes
    if let Some(track_index) = state.editing_track_index {
        if let Some(track) = animation.get_track_mut(track_index) {
            if let TrackSource::Keyframes { ref mut keyframes } = track.source {
                // Interpolation selector
                ui.horizontal(|ui| {
                    ui.label(t!("track_editor.interpolation"));
                    egui::ComboBox::from_id_salt("keyframe_interpolation")
                        .selected_text(format!("{:?}", track.interpolation))
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut track.interpolation, Interpolation::Step, t!("track_editor.interpolation_step").as_ref());
                            ui.selectable_value(&mut track.interpolation, Interpolation::Linear, t!("track_editor.interpolation_linear").as_ref());
                            ui.selectable_value(&mut track.interpolation, Interpolation::Smooth, t!("track_editor.interpolation_smooth").as_ref());
                            ui.selectable_value(&mut track.interpolation, Interpolation::Sinusoidal, t!("track_editor.interpolation_sinusoidal").as_ref());
                        });
                });

                // Keyframe list (scrollable)
                ui.label(format!("{}: {}", t!("track_editor.keyframe_count"), keyframes.len()));

                ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        let mut to_delete: Option<usize> = None;
                        let kf_count = keyframes.len();

                        for (i, kf) in keyframes.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                // Time
                                ui.add(egui::DragValue::new(&mut kf.time)
                                    .range(0.0..=duration)
                                    .speed(0.01)
                                    .suffix("s")
                                    .min_decimals(2));

                                // Value
                                let mut value = kf.value.as_f64().unwrap_or(0.0);
                                if ui.add(egui::DragValue::new(&mut value).speed(0.01).min_decimals(3)).changed() {
                                    kf.value = serde_json::json!(value);
                                }

                                // Easing
                                egui::ComboBox::from_id_salt(format!("kf_ease_{}", i))
                                    .selected_text(format!("{:?}", kf.easing))
                                    .width(120.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut kf.easing, EasingFunction::Linear, t!("track_editor.easing_linear").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseIn, t!("track_editor.easing_easein").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseOut, t!("track_editor.easing_easeout").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOut, t!("track_editor.easing_easeinout").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInCubic, t!("track_editor.easing_easeincubic").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutCubic, t!("track_editor.easing_easeoutcubic").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutCubic, t!("track_editor.easing_easeinoutcubic").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInSine, t!("track_editor.easing_easeinsine").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutSine, t!("track_editor.easing_easeoutsine").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutSine, t!("track_editor.easing_easeinoutsine").as_ref());
                                    });

                                // Delete (if more than 1)
                                if kf_count > 1 && ui.small_button("✕").clicked() {
                                    to_delete = Some(i);
                                }
                            });
                        }

                        if let Some(i) = to_delete {
                            keyframes.remove(i);
                        }
                    });

                // Add keyframe buttons
                ui.horizontal(|ui| {
                    if ui.button(t!("track_editor.add_at_current")).clicked() {
                        let current_value = get_parameter_value(config, &state.new_track_target).unwrap_or(0.0);
                        keyframes.push(Keyframe {
                            time: current_time.clamp(0.0, duration),
                            value: serde_json::json!(current_value),
                            easing: EasingFunction::Linear,
                        });
                        keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                    }

                    if ui.button(t!("track_editor.add_keyframe")).clicked() {
                        let last_time = keyframes.last().map(|k| k.time).unwrap_or(0.0);
                        let last_value = keyframes.last().and_then(|k| k.value.as_f64()).unwrap_or(0.0);
                        keyframes.push(Keyframe {
                            time: (last_time + 1.0).min(duration),
                            value: serde_json::json!(last_value),
                            easing: EasingFunction::Linear,
                        });
                    }

                    if ui.button(t!("track_editor.sort_by_time")).clicked() {
                        keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                    }
                });

                return;
            }
        }
    }

    // Add mode: show preview keyframes editor
    if !state.preview_keyframes.is_empty() {
        // Interpolation selector for preview
        ui.horizontal(|ui| {
            ui.label(t!("track_editor.interpolation"));
            egui::ComboBox::from_id_salt("preview_interpolation")
                .selected_text(format!("{:?}", state.preview_interpolation))
                .width(100.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Step, t!("track_editor.interpolation_step").as_ref());
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Linear, t!("track_editor.interpolation_linear").as_ref());
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Smooth, t!("track_editor.interpolation_smooth").as_ref());
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Sinusoidal, t!("track_editor.interpolation_sinusoidal").as_ref());
                });
        });

        // Preview keyframe list
        ui.label(format!("{}: {}", t!("track_editor.keyframe_count"), state.preview_keyframes.len()));

        ScrollArea::vertical()
            .max_height(150.0)
            .show(ui, |ui| {
                let mut to_delete: Option<usize> = None;
                let kf_count = state.preview_keyframes.len();

                for (i, kf) in state.preview_keyframes.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        // Time
                        ui.add(egui::DragValue::new(&mut kf.time)
                            .range(0.0..=duration)
                            .speed(0.01)
                            .suffix("s")
                            .min_decimals(2));

                        // Value
                        let mut value = kf.value.as_f64().unwrap_or(0.0);
                        if ui.add(egui::DragValue::new(&mut value).speed(0.01).min_decimals(3)).changed() {
                            kf.value = serde_json::json!(value);
                        }

                        // Easing
                        egui::ComboBox::from_id_salt(format!("preview_kf_ease_{}", i))
                            .selected_text(format!("{:?}", kf.easing))
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut kf.easing, EasingFunction::Linear, t!("track_editor.easing_linear").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseIn, t!("track_editor.easing_easein").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseOut, t!("track_editor.easing_easeout").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOut, t!("track_editor.easing_easeinout").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInCubic, t!("track_editor.easing_easeincubic").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutCubic, t!("track_editor.easing_easeoutcubic").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutCubic, t!("track_editor.easing_easeinoutcubic").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInSine, t!("track_editor.easing_easeinsine").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutSine, t!("track_editor.easing_easeoutsine").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutSine, t!("track_editor.easing_easeinoutsine").as_ref());
                            });

                        // Delete (if more than 1)
                        if kf_count > 1 && ui.small_button("✕").clicked() {
                            to_delete = Some(i);
                        }
                    });
                }

                if let Some(i) = to_delete {
                    state.preview_keyframes.remove(i);
                }
            });

        // Add keyframe buttons
        ui.horizontal(|ui| {
            if ui.button(t!("track_editor.add_at_current")).clicked() {
                let current_value = get_parameter_value(config, &state.new_track_target).unwrap_or(0.0);
                state.preview_keyframes.push(Keyframe {
                    time: current_time.clamp(0.0, duration),
                    value: serde_json::json!(current_value),
                    easing: EasingFunction::Linear,
                });
                state.preview_keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
            }

            if ui.button(t!("track_editor.add_keyframe")).clicked() {
                let last_time = state.preview_keyframes.last().map(|k| k.time).unwrap_or(0.0);
                let last_value = state.preview_keyframes.last().and_then(|k| k.value.as_f64()).unwrap_or(0.0);
                state.preview_keyframes.push(Keyframe {
                    time: (last_time + 1.0).min(duration),
                    value: serde_json::json!(last_value),
                    easing: EasingFunction::Linear,
                });
            }

            if ui.button(t!("track_editor.sort_by_time")).clicked() {
                state.preview_keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
            }
        });
    } else {
        // No target selected yet
        ui.label(t!("track_editor.select_target_first"));
    }
}

/// Render oscillator-specific options subpanel
fn render_oscillator_subpanel(ui: &mut Ui, state: &mut TrackEditorState) {
    ui.label(t!("track_editor.oscillator_section"));

    // Waveform type
    ui.horizontal(|ui| {
        ui.label(t!("track_editor.waveform"));
        egui::ComboBox::from_id_salt("osc_waveform")
            .selected_text(oscillator_type_name(&state.oscillator_params.oscillator_type))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.oscillator_params.oscillator_type, OscillatorType::Sine, t!("track_editor.waveform_sine").as_ref());
                ui.selectable_value(&mut state.oscillator_params.oscillator_type, OscillatorType::Triangle, t!("track_editor.waveform_triangle").as_ref());
                ui.selectable_value(&mut state.oscillator_params.oscillator_type, OscillatorType::Sawtooth, t!("track_editor.waveform_sawtooth").as_ref());
                ui.selectable_value(&mut state.oscillator_params.oscillator_type, OscillatorType::Square, t!("track_editor.waveform_square").as_ref());
            });
    });

    // Parameters
    ui.horizontal(|ui| {
        ui.label(t!("track_editor.osc_center"));
        ui.add(egui::DragValue::new(&mut state.oscillator_params.center).speed(0.01));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.osc_amplitude"));
        ui.add(egui::DragValue::new(&mut state.oscillator_params.amplitude).speed(0.01));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.osc_frequency"));
        ui.add(egui::DragValue::new(&mut state.oscillator_params.frequency).speed(0.01).suffix(" Hz"));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.osc_phase"));
        ui.add(egui::DragValue::new(&mut state.oscillator_params.phase).speed(0.01).range(0.0..=1.0));
    });
}

/// Render circular track-specific options subpanel
fn render_circular_subpanel(ui: &mut Ui, state: &mut TrackEditorState) {
    ui.label(t!("track_editor.circular_section"));

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.circ_center_x"));
        ui.add(egui::DragValue::new(&mut state.circular_params.center_x).speed(0.01));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.circ_center_y"));
        ui.add(egui::DragValue::new(&mut state.circular_params.center_y).speed(0.01));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.circ_radius"));
        ui.add(egui::DragValue::new(&mut state.circular_params.radius).speed(0.01));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.circ_speed"));
        ui.add(egui::DragValue::new(&mut state.circular_params.speed).speed(0.01).suffix(" rev/s"));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.circ_phase"));
        ui.add(egui::DragValue::new(&mut state.circular_params.phase).speed(0.01));
    });
}

/// Create or update a track based on current state
/// Returns the index of the created/updated track (for switching to edit mode)
fn update_or_create_track(
    animation: &mut Animation,
    state: &mut TrackEditorState,
    duration: f64,
    config: &FractalConfig,
) -> Option<usize> {
    match state.new_track_type {
        NewTrackType::Keyframe => {
            if let Some(track_index) = state.editing_track_index {
                // Update existing track
                if let Some(track) = animation.get_track_mut(track_index) {
                    track.target = state.new_track_target.clone();
                    // Keep existing keyframes, just update target
                }
                Some(track_index)
            } else {
                // Create new track using preview keyframes (auto-filled when target selected)
                let keyframes = if state.preview_keyframes.is_empty() {
                    // Fallback if no preview keyframes
                    let initial_value = get_parameter_value(config, &state.new_track_target).unwrap_or(0.0);
                    vec![
                        Keyframe {
                            time: 0.0,
                            value: serde_json::json!(initial_value),
                            easing: EasingFunction::Linear,
                        },
                        Keyframe {
                            time: duration,
                            value: serde_json::json!(initial_value),
                            easing: EasingFunction::Linear,
                        },
                    ]
                } else {
                    state.preview_keyframes.clone()
                };
                let mut track = Track::new(
                    state.new_track_target.clone(),
                    TrackSource::Keyframes { keyframes },
                );
                track.interpolation = state.preview_interpolation;
                let new_index = animation.add_track(track);
                Some(new_index)
            }
        }
        NewTrackType::Oscillator => {
            if let Some(track_index) = state.editing_track_index {
                // Update existing track
                if let Some(track) = animation.get_track_mut(track_index) {
                    track.target = state.new_track_target.clone();
                    track.source = TrackSource::Oscillator {
                        oscillator_type: state.oscillator_params.oscillator_type,
                        center: state.oscillator_params.center,
                        amplitude: state.oscillator_params.amplitude,
                        frequency: state.oscillator_params.frequency,
                        phase: state.oscillator_params.phase,
                    };
                }
                Some(track_index)
            } else {
                // Create new track
                let track = Track::new(
                    state.new_track_target.clone(),
                    TrackSource::Oscillator {
                        oscillator_type: state.oscillator_params.oscillator_type,
                        center: state.oscillator_params.center,
                        amplitude: state.oscillator_params.amplitude,
                        frequency: state.oscillator_params.frequency,
                        phase: state.oscillator_params.phase,
                    },
                );
                let new_index = animation.add_track(track);
                Some(new_index)
            }
        }
        NewTrackType::Circular => {
            // Circular tracks use a different index space
            if let Some(encoded_index) = state.editing_track_index {
                if encoded_index >= usize::MAX / 2 {
                    // This is a circular track edit (encoded as usize::MAX - index)
                    let circular_index = usize::MAX - encoded_index;
                    if let Some(track) = animation.circular_tracks.get_mut(circular_index) {
                        track.target_x = state.new_track_target.clone();
                        track.target_y = state.new_track_target_y.clone();
                        track.center_x = state.circular_params.center_x;
                        track.center_y = state.circular_params.center_y;
                        track.radius = state.circular_params.radius;
                        track.speed = state.circular_params.speed;
                        track.phase = state.circular_params.phase;
                    }
                    return Some(encoded_index);
                }
            }
            // Create new circular track
            let track = CircularTrack {
                target_x: state.new_track_target.clone(),
                target_y: state.new_track_target_y.clone(),
                center_x: state.circular_params.center_x,
                center_y: state.circular_params.center_y,
                radius: state.circular_params.radius,
                speed: state.circular_params.speed,
                phase: state.circular_params.phase,
            };
            let new_index = animation.add_circular_track(track);
            // Return encoded index for circular tracks
            Some(usize::MAX - new_index)
        }
    }
}

/// Close the track editor panel and reset state
fn close_track_editor_panel(state: &mut TrackEditorState) {
    state.track_editor_panel_open = false;
    state.editing_track_index = None;
    state.new_track_target.clear();
    state.new_track_target_y.clear();
    state.target_selector_state = TargetSelectorState::default();
    state.target_selector_state_y = TargetSelectorState::default();
    state.preview_keyframes.clear();
    state.preview_interpolation = Interpolation::Linear;
}

/// Open the track editor panel to add a new track
pub fn open_add_track_panel(state: &mut TrackEditorState) {
    state.track_editor_panel_open = true;
    state.editing_track_index = None;
    state.new_track_target.clear();
    state.new_track_target_y.clear();
    state.new_track_type = NewTrackType::Keyframe;
    state.oscillator_params = OscillatorParams::default();
    state.circular_params = CircularParams::default();
    state.target_selector_state = TargetSelectorState::default();
    state.target_selector_state_y = TargetSelectorState::default();
    state.preview_keyframes.clear();
    state.preview_interpolation = Interpolation::Linear;
}

/// Open the track editor panel to edit an existing track by index
pub fn open_edit_track_panel(state: &mut TrackEditorState, track_index: usize, track: &Track) {
    state.track_editor_panel_open = true;
    state.editing_track_index = Some(track_index);
    state.new_track_target = track.target.clone();
    state.new_track_type = match &track.source {
        TrackSource::Keyframes { .. } => NewTrackType::Keyframe,
        TrackSource::Oscillator { oscillator_type, center, amplitude, frequency, phase } => {
            state.oscillator_params = OscillatorParams {
                oscillator_type: *oscillator_type,
                center: *center,
                amplitude: *amplitude,
                frequency: *frequency,
                phase: *phase,
            };
            NewTrackType::Oscillator
        }
    };
    state.target_selector_state = TargetSelectorState::default();
}

/// Open the track editor panel to edit a circular track by index
pub fn open_edit_circular_track_panel(state: &mut TrackEditorState, circular_index: usize, track: &CircularTrack) {
    state.track_editor_panel_open = true;
    // For circular tracks, store index with offset to distinguish from regular tracks
    // We use a convention: editing_track_index for circular tracks stores usize::MAX - circular_index
    state.editing_track_index = Some(usize::MAX - circular_index);
    state.new_track_type = NewTrackType::Circular;
    state.new_track_target = track.target_x.clone();
    state.new_track_target_y = track.target_y.clone();
    state.circular_params = CircularParams {
        center_x: track.center_x,
        center_y: track.center_y,
        radius: track.radius,
        speed: track.speed,
        phase: track.phase,
    };
    state.target_selector_state = TargetSelectorState::default();
    state.target_selector_state_y = TargetSelectorState::default();
}

/// Get short name for easing function
fn easing_short_name(easing: &EasingFunction) -> &'static str {
    match easing {
        EasingFunction::Linear => "Lin",
        EasingFunction::EaseIn => "In",
        EasingFunction::EaseOut => "Out",
        EasingFunction::EaseInOut => "I/O",
        EasingFunction::EaseInQuad => "In²",
        EasingFunction::EaseOutQuad => "Out²",
        EasingFunction::EaseInOutQuad => "I/O²",
        EasingFunction::EaseInCubic => "In³",
        EasingFunction::EaseOutCubic => "Out³",
        EasingFunction::EaseInOutCubic => "I/O³",
        EasingFunction::EaseInSine => "InS",
        EasingFunction::EaseOutSine => "OutS",
        EasingFunction::EaseInOutSine => "I/OS",
    }
}

/// Get display name for oscillator type
fn oscillator_type_name(osc_type: &OscillatorType) -> String {
    match osc_type {
        OscillatorType::Sine => t!("track_editor.waveform_sine").to_string(),
        OscillatorType::Triangle => t!("track_editor.waveform_triangle").to_string(),
        OscillatorType::Sawtooth => t!("track_editor.waveform_sawtooth").to_string(),
        OscillatorType::Square => t!("track_editor.waveform_square").to_string(),
    }
}

/// Get the current value for a ConfigPath from the fractal config
/// Returns None if the path doesn't map to a numeric value
pub fn get_current_value(config: &FractalConfig, path: &ConfigPath) -> Option<f64> {
    use crate::config::AffineParam;

    match path {
        // View parameters
        ConfigPath::Zoom => Some(config.zoom as f64),
        ConfigPath::PanX => Some(config.pan_x as f64),
        ConfigPath::PanY => Some(config.pan_y as f64),
        ConfigPath::Rotation => Some(config.rotation as f64),
        ConfigPath::CameraRotationX => Some(config.camera_rotation_x as f64),
        ConfigPath::CameraRotationY => Some(config.camera_rotation_y as f64),
        ConfigPath::CameraZ => Some(config.camera_z as f64),

        // Tone mapping
        ConfigPath::Exposure => Some(config.exposure as f64),
        ConfigPath::Gamma => Some(config.gamma as f64),
        ConfigPath::GammaThreshold => Some(config.gamma_threshold as f64),
        ConfigPath::Brightness => Some(config.brightness as f64),
        ConfigPath::Vibrancy => Some(config.vibrancy as f64),
        ConfigPath::Saturation => Some(config.saturation as f64),
        ConfigPath::HueShift => Some(config.hue_shift as f64),
        ConfigPath::DensityScale => Some(config.density_scale as f64),
        ConfigPath::LevelsLow => Some(config.levels_low as f64),
        ConfigPath::LevelsHigh => Some(config.levels_high as f64),
        ConfigPath::LevelsGamma => Some(config.levels_gamma as f64),

        // Color
        ConfigPath::PaletteRotation => Some(config.palette_rotation as f64),
        ConfigPath::PaletteSqueeze => Some(config.palette_squeeze as f64),
        ConfigPath::SpeedFactor => Some(config.speed_factor as f64),
        ConfigPath::BackgroundColorR => Some(config.background_color[0] as f64),
        ConfigPath::BackgroundColorG => Some(config.background_color[1] as f64),
        ConfigPath::BackgroundColorB => Some(config.background_color[2] as f64),

        // Rendering
        ConfigPath::HistogramColorScale => Some(config.histogram_color_scale as f64),
        ConfigPath::BlendFactor => Some(config.blend_factor as f64),
        ConfigPath::PerspectiveStrength => Some(config.flame.perspective_strength as f64),

        // Transform parameters
        ConfigPath::TransformWeight { index } => {
            config.flame.transforms.get(*index).map(|t| t.weight as f64)
        }
        ConfigPath::TransformColor { index } => {
            config.flame.transforms.get(*index).map(|t| t.color as f64)
        }
        ConfigPath::TransformColorSpeed { index } => {
            config.flame.transforms.get(*index).map(|t| t.color_speed as f64)
        }
        ConfigPath::TransformOpacity { index } => {
            config.flame.transforms.get(*index).map(|t| t.opacity as f64)
        }
        ConfigPath::TransformAffine { index, param } => {
            config.flame.transforms.get(*index).map(|t| {
                match param {
                    AffineParam::A => t.a as f64,
                    AffineParam::B => t.b as f64,
                    AffineParam::C => t.c as f64,
                    AffineParam::D => t.d as f64,
                    AffineParam::E => t.e as f64,
                    AffineParam::F => t.f as f64,
                    AffineParam::G => t.g as f64,
                }
            })
        }
        ConfigPath::TransformOriginX { index } => {
            config.flame.transforms.get(*index).map(|t| t.e as f64)
        }
        ConfigPath::TransformOriginY { index } => {
            config.flame.transforms.get(*index).map(|t| -t.f as f64)
        }
        ConfigPath::TransformRotation { index } => {
            config.flame.transforms.get(*index).map(|t| t.rotation() as f64)
        }
        ConfigPath::TransformScale { index } => {
            config.flame.transforms.get(*index).map(|t| t.scale() as f64)
        }
        ConfigPath::TransformVariation { index, variation } => {
            config.flame.transforms.get(*index).and_then(|t| {
                t.variations.get(variation).map(|&v| v as f64)
            })
        }

        // Final transform
        ConfigPath::FinalTransformAffine { param } => {
            config.flame.final_transform.as_ref().map(|t| {
                match param {
                    AffineParam::A => t.a as f64,
                    AffineParam::B => t.b as f64,
                    AffineParam::C => t.c as f64,
                    AffineParam::D => t.d as f64,
                    AffineParam::E => t.e as f64,
                    AffineParam::F => t.f as f64,
                    AffineParam::G => t.g as f64,
                }
            })
        }
        ConfigPath::FinalTransformOriginX => {
            config.flame.final_transform.as_ref().map(|t| t.e as f64)
        }
        ConfigPath::FinalTransformOriginY => {
            config.flame.final_transform.as_ref().map(|t| -t.f as f64)
        }
        ConfigPath::FinalTransformRotation => {
            config.flame.final_transform.as_ref().map(|t| t.rotation() as f64)
        }
        ConfigPath::FinalTransformScale => {
            config.flame.final_transform.as_ref().map(|t| t.scale() as f64)
        }

        // Non-numeric or complex types
        _ => None,
    }
}

/// Determine the auto-fill end value for a parameter
/// Most parameters just return the same value (start = end)
/// Special parameters like rotation get +2π for a full cycle
pub fn get_auto_fill_end_value(path: &ConfigPath, start_value: f64) -> f64 {
    use std::f64::consts::TAU; // 2π

    match path {
        // Rotation parameters: add full rotation
        ConfigPath::TransformRotation { .. } |
        ConfigPath::FinalTransformRotation |
        ConfigPath::Rotation |
        ConfigPath::CameraRotationX |
        ConfigPath::CameraRotationY => start_value + TAU,

        // Color index: add 1.0 for full palette cycle
        ConfigPath::TransformColor { .. } => start_value + 1.0,

        // Palette rotation: add 1.0 for full rotation
        ConfigPath::PaletteRotation => start_value + 1.0,

        // All other parameters: same as start (user adjusts manually)
        _ => start_value,
    }
}

/// Initialize preview keyframes based on the selected target
pub fn initialize_preview_keyframes(
    state: &mut TrackEditorState,
    config: &FractalConfig,
    duration: f64,
) {
    // Parse the target string to ConfigPath
    let path = match ConfigPath::from_string_key(&state.new_track_target) {
        Some(p) => p,
        None => {
            // Can't parse path, use default keyframes
            state.preview_keyframes = vec![
                Keyframe { time: 0.0, value: serde_json::json!(0.0), easing: EasingFunction::Linear },
                Keyframe { time: duration, value: serde_json::json!(1.0), easing: EasingFunction::Linear },
            ];
            return;
        }
    };

    // Get current value
    let start_value = get_current_value(config, &path).unwrap_or(0.0);
    let end_value = get_auto_fill_end_value(&path, start_value);

    state.preview_keyframes = vec![
        Keyframe { time: 0.0, value: serde_json::json!(start_value), easing: EasingFunction::Linear },
        Keyframe { time: duration, value: serde_json::json!(end_value), easing: EasingFunction::Linear },
    ];
    state.preview_interpolation = Interpolation::Linear;
}

/// Initialize oscillator center from current value
pub fn initialize_oscillator_center(
    state: &mut TrackEditorState,
    config: &FractalConfig,
) {
    if let Some(path) = ConfigPath::from_string_key(&state.new_track_target) {
        if let Some(value) = get_current_value(config, &path) {
            state.oscillator_params.center = value;
        }
    }
}

/// Initialize circular track centers from current values
pub fn initialize_circular_centers(
    state: &mut TrackEditorState,
    config: &FractalConfig,
) {
    if let Some(path_x) = ConfigPath::from_string_key(&state.new_track_target) {
        if let Some(value) = get_current_value(config, &path_x) {
            state.circular_params.center_x = value;
        }
    }
    if let Some(path_y) = ConfigPath::from_string_key(&state.new_track_target_y) {
        if let Some(value) = get_current_value(config, &path_y) {
            state.circular_params.center_y = value;
        }
    }
}
