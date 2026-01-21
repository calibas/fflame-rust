//! Animation track editor UI
//!
//! Provides controls for adding, editing, and deleting animation tracks.
//!
//! Phase 2 adds visual timeline bars for tracks aligned with the scrubber.
//! Phase 3 adds interactions: click to seek, keyframe hover/click.

use egui::{Ui, Color32, Rect, Pos2, Stroke, CornerRadius, Sense};
use rust_i18n::t;
use crate::animation::{
    Animation, AnimationController, CircularTrack, EasingFunction, Interpolation,
    Keyframe, OscillatorType, Track, TrackSource,
};
use crate::config::FractalConfig;
use crate::effects::{global_effect_registry, EffectInstance};
use crate::variations::global_registry;
use super::animation_panel::TimelineLayout;

/// UI state for track editor
#[derive(Default)]
pub struct TrackEditorState {
    /// Track path being edited for keyframes (None = no keyframe editor open)
    pub editing_keyframes_for: Option<String>,
    /// Index of keyframe to highlight/scroll to when opening editor (set by clicking a dot)
    pub selected_keyframe_index: Option<usize>,
    /// Whether the "add track" dialog is open
    pub add_track_dialog_open: bool,
    /// Selected track type for new track
    pub new_track_type: NewTrackType,
    /// Selected target for new track
    pub new_track_target: String,
    /// Selected second target for circular tracks
    pub new_track_target_y: String,
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
            name: t!("track_editor.category_transform", index = i).to_string(),
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
        if ui.add_enabled(has_animation, egui::Button::new(t!("track_editor.add_track"))).clicked() {
            state.add_track_dialog_open = true;
        }
    });

    // Add track dialog
    if state.add_track_dialog_open && has_animation {
        render_add_track_dialog(ui, controller, state, config);
    }

    ui.separator();

    // Render tracks - visual bars if timeline layout provided, otherwise inline controls
    if let Some(ref mut animation) = controller.animation {
        if let Some(layout) = timeline_layout {
            response = render_tracks_visual(ui, animation, state, layout);
        } else {
            render_tracks(ui, animation, state);
        }
    }

    // Keyframe editor (shown when editing a track's keyframes)
    if let Some(ref track_path) = state.editing_keyframes_for.clone() {
        if let Some(ref mut animation) = controller.animation {
            render_keyframe_editor(ui, animation, &track_path, state);
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
    let track_keys: Vec<String> = animation.tracks.keys().cloned().collect();
    let mut track_to_delete: Option<String> = None;
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
    let keyframe_color = Color32::from_rgb(255, 200, 100);
    let keyframe_hover_color = Color32::from_rgb(255, 255, 150);
    let position_line_color = Color32::from_rgb(255, 80, 80);

    // Calculate position line X
    let position_x = layout.position_x();

    // Track all track rects for position line drawing
    let mut all_track_rects: Vec<Rect> = Vec::new();

    // Store bar area info for later position line calculation
    let mut bar_area_info: Option<(f32, f32, f32)> = None; // (bar_left, bar_right, bar_scale)

    for path in track_keys {
        if let Some(track) = animation.tracks.get(&path) {
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

                // Background bar (full timeline extent)
                let bg_rect = Rect::from_min_max(
                    Pos2::new(bar_left, rect.center().y - BAR_HEIGHT / 2.0),
                    Pos2::new(bar_right, rect.center().y + BAR_HEIGHT / 2.0),
                );
                painter.rect_filled(bg_rect, CornerRadius::same(2), bar_bg_color);

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

                // Handle keyframe click - open editor with that keyframe selected
                if let Some(kf_idx) = clicked_keyframe {
                    state.editing_keyframes_for = Some(path.clone());
                    state.selected_keyframe_index = Some(kf_idx);
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

                // Draw label on the left
                let label_rect = Rect::from_min_max(
                    rect.left_top(),
                    Pos2::new(rect.left() + LABEL_WIDTH - 4.0, rect.bottom()),
                );
                // Truncate label if too long
                let display_name = if path.len() > 12 {
                    format!("{}...", &path[..12])
                } else {
                    path.clone()
                };
                painter.text(
                    label_rect.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    &display_name,
                    egui::FontId::proportional(12.0),
                    ui.visuals().text_color(),
                );

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
                            state.editing_keyframes_for = Some(path.clone());
                            state.selected_keyframe_index = None; // No specific keyframe selected
                        }
                        if ui.small_button("X").on_hover_text(t!("track_editor.delete_track")).clicked() {
                            track_to_delete = Some(path.clone());
                        }
                    });
                },
            );
        }
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
    if let Some(path) = track_to_delete {
        animation.remove_track(&path);
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

/// Render the add track dialog
fn render_add_track_dialog(
    ui: &mut Ui,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    config: &FractalConfig,
) {
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(8.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.label(t!("track_editor.add_new_track"));

            // Track type selection
            ui.horizontal(|ui| {
                ui.label(t!("track_editor.type"));
                egui::ComboBox::from_id_salt("new_track_type")
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

            // Target parameter selection
            let categories = animatable_parameters(config);

            ui.horizontal(|ui| {
                ui.label(t!("track_editor.target"));
                egui::ComboBox::from_id_salt("new_track_target")
                    .selected_text(if state.new_track_target.is_empty() {
                        t!("track_editor.select").to_string()
                    } else {
                        state.new_track_target.clone()
                    })
                    .show_ui(ui, |ui| {
                        for category in &categories {
                            ui.separator();
                            ui.label(&category.name);
                            for (display_name, path) in &category.params {
                                if ui.selectable_label(
                                    state.new_track_target == *path,
                                    display_name
                                ).clicked() {
                                    state.new_track_target = path.to_string();
                                }
                            }
                        }
                    });
            });

            // Second target for circular tracks
            if state.new_track_type == NewTrackType::Circular {
                ui.horizontal(|ui| {
                    ui.label(t!("track_editor.target_y"));
                    egui::ComboBox::from_id_salt("new_track_target_y")
                        .selected_text(if state.new_track_target_y.is_empty() {
                            t!("track_editor.select").to_string()
                        } else {
                            state.new_track_target_y.clone()
                        })
                        .show_ui(ui, |ui| {
                            for category in &categories {
                                ui.separator();
                                ui.label(&category.name);
                                for (display_name, path) in &category.params {
                                    if ui.selectable_label(
                                        state.new_track_target_y == *path,
                                        display_name
                                    ).clicked() {
                                        state.new_track_target_y = path.to_string();
                                    }
                                }
                            }
                        });
                });
            }

            // Add/Cancel buttons
            ui.horizontal(|ui| {
                let can_add = match state.new_track_type {
                    NewTrackType::Circular => !state.new_track_target.is_empty() && !state.new_track_target_y.is_empty(),
                    _ => !state.new_track_target.is_empty(),
                };

                if ui.add_enabled(can_add, egui::Button::new(t!("track_editor.add"))).clicked() {
                    if let Some(ref mut animation) = controller.animation {
                        match state.new_track_type {
                            NewTrackType::Keyframe => {
                                let track = Track {
                                    source: TrackSource::Keyframes {
                                        keyframes: vec![
                                            Keyframe {
                                                time: 0.0,
                                                value: serde_json::json!(0.0),
                                                easing: EasingFunction::Linear,
                                            },
                                            Keyframe {
                                                time: animation.duration,
                                                value: serde_json::json!(1.0),
                                                easing: EasingFunction::Linear,
                                            },
                                        ],
                                    },
                                    interpolation: Interpolation::Linear,
                                };
                                animation.add_track_str(state.new_track_target.clone(), track);
                            }
                            NewTrackType::Oscillator => {
                                let track = Track {
                                    source: TrackSource::Oscillator {
                                        oscillator_type: OscillatorType::Sine,
                                        center: 0.0,
                                        amplitude: 1.0,
                                        frequency: 0.5,
                                        phase: 0.0,
                                    },
                                    interpolation: Interpolation::Linear,
                                };
                                animation.add_track_str(state.new_track_target.clone(), track);
                            }
                            NewTrackType::Circular => {
                                let track = CircularTrack::new(
                                    state.new_track_target.clone(),
                                    state.new_track_target_y.clone(),
                                    0.0, 0.0,  // center
                                    0.5,       // radius
                                    0.1,       // speed (rev/s)
                                );
                                animation.add_circular_track(track);
                            }
                        }
                    }
                    state.add_track_dialog_open = false;
                    state.new_track_target.clear();
                    state.new_track_target_y.clear();
                }

                if ui.button(t!("track_editor.cancel")).clicked() {
                    state.add_track_dialog_open = false;
                    state.new_track_target.clear();
                    state.new_track_target_y.clear();
                }
            });
        });
}

/// Render the list of existing tracks
fn render_tracks(ui: &mut Ui, animation: &mut Animation, state: &mut TrackEditorState) {
    // Collect track keys to avoid borrow issues
    let track_keys: Vec<String> = animation.tracks.keys().cloned().collect();
    let mut track_to_delete: Option<String> = None;

    for path in track_keys {
        if let Some(track) = animation.tracks.get_mut(&path) {
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
                                track_to_delete = Some(path.clone());
                            }
                        });
                    });

                    // Track-specific controls
                    match &mut track.source {
                        TrackSource::Keyframes { .. } => {
                            ui.horizontal(|ui| {
                                ui.label(t!("track_editor.interpolation"));
                                egui::ComboBox::from_id_salt(format!("interp_{}", path))
                                    .selected_text(format!("{:?}", track.interpolation))
                                    .width(100.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Step, t!("track_editor.interpolation_step").as_ref());
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Linear, t!("track_editor.interpolation_linear").as_ref());
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Smooth, t!("track_editor.interpolation_smooth").as_ref());
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Sinusoidal, t!("track_editor.interpolation_sinusoidal").as_ref());
                                    });
                            });

                            if ui.small_button(t!("track_editor.edit_keyframes")).clicked() {
                                state.editing_keyframes_for = Some(path.clone());
                            }
                        }
                        TrackSource::Oscillator { oscillator_type, center, amplitude, frequency, phase } => {
                            ui.horizontal(|ui| {
                                ui.label(t!("track_editor.osc_type"));
                                egui::ComboBox::from_id_salt(format!("osc_type_{}", path))
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
    }

    // Delete track if requested
    if let Some(path) = track_to_delete {
        animation.remove_track(&path);
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

/// Render the keyframe editor for a specific track
fn render_keyframe_editor(
    ui: &mut Ui,
    animation: &mut Animation,
    track_path: &str,
    state: &mut TrackEditorState,
) {
    let duration = animation.duration;

    if let Some(track) = animation.tracks.get_mut(track_path) {
        if let TrackSource::Keyframes { ref mut keyframes } = track.source {
            egui::Window::new(t!("track_editor.keyframe_window_title", path = track_path))
                .collapsible(false)
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    // Header row
                    ui.horizontal(|ui| {
                        ui.label(t!("track_editor.time"));
                        ui.add_space(40.0);
                        ui.label(t!("track_editor.value"));
                        ui.add_space(40.0);
                        ui.label(t!("track_editor.easing"));
                    });

                    ui.separator();

                    // Keyframe rows
                    let mut keyframe_to_delete: Option<usize> = None;
                    let keyframe_count = keyframes.len();
                    let selected_idx = state.selected_keyframe_index;

                    for (i, keyframe) in keyframes.iter_mut().enumerate() {
                        // Highlight selected keyframe (from clicking on dot)
                        let is_selected = selected_idx == Some(i);

                        let frame = if is_selected {
                            egui::Frame::new()
                                .fill(Color32::from_rgba_unmultiplied(100, 150, 255, 60))
                                .inner_margin(2.0)
                        } else {
                            egui::Frame::NONE
                        };

                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Time
                                ui.add(egui::DragValue::new(&mut keyframe.time)
                                .range(0.0..=duration)
                                .speed(0.01)
                                .suffix("s"));

                            // Value (as f64)
                            let mut value_f64 = keyframe.value.as_f64().unwrap_or(0.0);
                            if ui.add(egui::DragValue::new(&mut value_f64).speed(0.01)).changed() {
                                keyframe.value = serde_json::json!(value_f64);
                            }

                            // Easing
                            egui::ComboBox::from_id_salt(format!("easing_{}_{}", track_path, i))
                                .selected_text(format!("{:?}", keyframe.easing))
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::Linear, t!("track_editor.easing_linear").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseIn, t!("track_editor.easing_easein").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseOut, t!("track_editor.easing_easeout").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInOut, t!("track_editor.easing_easeinout").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInCubic, t!("track_editor.easing_easeincubic").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseOutCubic, t!("track_editor.easing_easeoutcubic").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInOutCubic, t!("track_editor.easing_easeinoutcubic").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInSine, t!("track_editor.easing_easeinsine").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseOutSine, t!("track_editor.easing_easeoutsine").as_ref());
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInOutSine, t!("track_editor.easing_easeinoutsine").as_ref());
                                });

                                // Delete button (only if more than 1 keyframe)
                                if keyframe_count > 1 {
                                    if ui.small_button("X").clicked() {
                                        keyframe_to_delete = Some(i);
                                    }
                                }
                            });
                        });
                    }

                    // Delete keyframe if requested
                    if let Some(i) = keyframe_to_delete {
                        keyframes.remove(i);
                    }

                    ui.separator();

                    // Add keyframe button
                    ui.horizontal(|ui| {
                        if ui.button(t!("track_editor.add_keyframe")).clicked() {
                            // Add at the end with interpolated value
                            let last_time = keyframes.last().map(|k| k.time).unwrap_or(0.0);
                            let last_value = keyframes.last()
                                .and_then(|k| k.value.as_f64())
                                .unwrap_or(0.0);

                            keyframes.push(Keyframe {
                                time: (last_time + 1.0).min(duration),
                                value: serde_json::json!(last_value),
                                easing: EasingFunction::Linear,
                            });
                        }

                        if ui.button(t!("track_editor.sort_by_time")).clicked() {
                            keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                        }

                        if ui.button(t!("track_editor.done")).clicked() {
                            // Sort keyframes by time before closing
                            keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                            state.editing_keyframes_for = None;
                            state.selected_keyframe_index = None;
                        }
                    });
                });
        }
    } else {
        // Track was deleted while editing
        state.editing_keyframes_for = None;
        state.selected_keyframe_index = None;
    }
}
