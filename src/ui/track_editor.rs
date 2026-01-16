//! Animation track editor UI
//!
//! Provides controls for adding, editing, and deleting animation tracks.

use egui::Ui;
use rust_i18n::t;
use crate::animation::{
    Animation, AnimationController, CircularTrack, EasingFunction, Interpolation,
    Keyframe, OscillatorType, Track, TrackSource,
};
use crate::config::FractalConfig;
use crate::effects::{global_effect_registry, EffectInstance};
use crate::variations::global_registry;

/// UI state for track editor
#[derive(Default)]
pub struct TrackEditorState {
    /// Track path being edited for keyframes (None = no keyframe editor open)
    pub editing_keyframes_for: Option<String>,
    /// Whether the "add track" dialog is open
    pub add_track_dialog_open: bool,
    /// Selected track type for new track
    pub new_track_type: NewTrackType,
    /// Selected target for new track
    pub new_track_target: String,
    /// Selected second target for circular tracks
    pub new_track_target_y: String,
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
        // Add enabled toggle (note: "Enabled" with capital E to match ConfigPath parsing)
        params.push((
            format!("{} → Enabled", info.display_name),
            format!("{}.{}.Enabled", prefix, index),
        ));

        // Add each parameter
        for param_def in &info.parameters {
            params.push((
                format!("{} → {}", info.display_name, param_def.display_name),
                format!("{}.{}.{}", prefix, index, param_def.name),
            ));
        }
    }
}

/// Render the track editor section
pub fn render_track_editor(
    ui: &mut Ui,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    config: &FractalConfig,
) {
    let has_animation = controller.animation.is_some();

    // Animation header with name and duration
    if let Some(ref mut animation) = controller.animation {
        ui.horizontal(|ui| {
            ui.label(t!("track_editor.name"));
            ui.text_edit_singleline(&mut animation.name);
        });

        ui.horizontal(|ui| {
            ui.label(t!("track_editor.duration"));
            ui.add(egui::DragValue::new(&mut animation.duration)
                .range(0.1..=3600.0)
                .speed(0.1)
                .suffix("s"));
        });

        ui.separator();
    }

    // Track list header
    let track_count = controller.animation.as_ref()
        .map(|a| a.tracks.len() + a.circular_tracks.len())
        .unwrap_or(0);

    egui::CollapsingHeader::new(t!("track_editor.tracks_header", count = track_count))
        .default_open(true)
        .show(ui, |ui| {
            // Add track button
            if ui.add_enabled(has_animation, egui::Button::new(t!("track_editor.add_track"))).clicked() {
                state.add_track_dialog_open = true;
            }

            // Add track dialog
            if state.add_track_dialog_open && has_animation {
                render_add_track_dialog(ui, controller, state, config);
            }

            ui.separator();

            // Render existing tracks
            if let Some(ref mut animation) = controller.animation {
                render_tracks(ui, animation, state);
            }
        });

    // Keyframe editor (shown when editing a track's keyframes)
    if let Some(ref track_path) = state.editing_keyframes_for.clone() {
        if let Some(ref mut animation) = controller.animation {
            render_keyframe_editor(ui, animation, &track_path, state);
        }
    }
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

                    for (i, keyframe) in keyframes.iter_mut().enumerate() {
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
                        }
                    });
                });
        }
    } else {
        // Track was deleted while editing
        state.editing_keyframes_for = None;
    }
}
