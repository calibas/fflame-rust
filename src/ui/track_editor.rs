//! Animation track editor UI
//!
//! Provides controls for adding, editing, and deleting animation tracks.

use egui::Ui;
use crate::animation::{
    Animation, AnimationController, CircularTrack, EasingFunction, Interpolation,
    Keyframe, OscillatorType, Track, TrackSource,
};

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
    name: &'static str,
    params: Vec<(&'static str, &'static str)>, // (display_name, path_string)
}

/// Get list of animatable parameters organized by category
fn animatable_parameters(transform_count: usize) -> Vec<ParameterCategory> {
    let mut categories = vec![
        ParameterCategory {
            name: "View",
            params: vec![
                ("Zoom", "Zoom"),
                ("Rotation", "Rotation"),
                ("Camera Rotation X", "CameraRotationX"),
                ("Camera Rotation Y", "CameraRotationY"),
                ("Camera Z", "CameraZ"),
            ],
        },
        ParameterCategory {
            name: "Tone Mapping",
            params: vec![
                ("Exposure", "Exposure"),
                ("Gamma", "Gamma"),
                ("Gamma Threshold", "GammaThreshold"),
                ("Brightness", "Brightness"),
                ("Vibrancy", "Vibrancy"),
                ("Saturation", "Saturation"),
                ("Hue Shift", "HueShift"),
                ("Value Scale", "ValueScale"),
                ("Alpha Blend Low", "AlphaBlendLow"),
                ("Alpha Blend High", "AlphaBlendHigh"),
                ("Density Scale", "DensityScale"),
            ],
        },
        ParameterCategory {
            name: "Color",
            params: vec![
                ("Palette Rotation", "PaletteRotation"),
                ("Speed Factor", "SpeedFactor"),
                ("Histogram Color Scale", "HistogramColorScale"),
            ],
        },
        ParameterCategory {
            name: "Rendering",
            params: vec![
                ("Blend Factor", "BlendFactor"),
                ("Perspective Strength", "PerspectiveStrength"),
                ("Low Density Smoothing", "LowDensitySmoothing"),
            ],
        },
    ];

    // Add transform-specific parameters
    for i in 0..transform_count {
        let mut params = vec![
            (format!("Weight"), format!("Transform.{}.Weight", i)),
            (format!("Color"), format!("Transform.{}.Color", i)),
            (format!("Color Speed"), format!("Transform.{}.ColorSpeed", i)),
            (format!("Opacity"), format!("Transform.{}.Opacity", i)),
        ];

        // High-level transform operations (translate, rotate, scale)
        params.push((format!("Origin X (Translate)"), format!("Transform.{}.OriginX", i)));
        params.push((format!("Origin Y (Translate)"), format!("Transform.{}.OriginY", i)));
        params.push((format!("Rotation"), format!("Transform.{}.Rotation", i)));
        params.push((format!("Scale"), format!("Transform.{}.Scale", i)));

        // Raw affine parameters (for advanced users)
        for param in ['A', 'B', 'C', 'D', 'E', 'F', 'G'] {
            params.push((format!("Affine {}", param), format!("Transform.{}.Affine.{}", i, param)));
        }

        categories.push(ParameterCategory {
            name: Box::leak(format!("Transform {}", i).into_boxed_str()),
            params: params.into_iter().map(|(d, p)| {
                (Box::leak(d.into_boxed_str()) as &'static str,
                 Box::leak(p.into_boxed_str()) as &'static str)
            }).collect(),
        });
    }

    categories
}

/// Render the track editor section
pub fn render_track_editor(
    ui: &mut Ui,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    transform_count: usize,
) {
    let has_animation = controller.animation.is_some();

    // Animation header with name and duration
    if let Some(ref mut animation) = controller.animation {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut animation.name);
        });

        ui.horizontal(|ui| {
            ui.label("Duration:");
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

    egui::CollapsingHeader::new(format!("Tracks ({})", track_count))
        .default_open(true)
        .show(ui, |ui| {
            // Add track button
            if ui.add_enabled(has_animation, egui::Button::new("+ Add Track")).clicked() {
                state.add_track_dialog_open = true;
            }

            // Add track dialog
            if state.add_track_dialog_open && has_animation {
                render_add_track_dialog(ui, controller, state, transform_count);
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
    transform_count: usize,
) {
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(8.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.label("Add New Track");

            // Track type selection
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt("new_track_type")
                    .selected_text(match state.new_track_type {
                        NewTrackType::Keyframe => "Keyframe",
                        NewTrackType::Oscillator => "Oscillator",
                        NewTrackType::Circular => "Circular",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.new_track_type, NewTrackType::Keyframe, "Keyframe");
                        ui.selectable_value(&mut state.new_track_type, NewTrackType::Oscillator, "Oscillator");
                        ui.selectable_value(&mut state.new_track_type, NewTrackType::Circular, "Circular");
                    });
            });

            // Target parameter selection
            let categories = animatable_parameters(transform_count);

            ui.horizontal(|ui| {
                ui.label("Target:");
                egui::ComboBox::from_id_salt("new_track_target")
                    .selected_text(if state.new_track_target.is_empty() {
                        "Select..."
                    } else {
                        &state.new_track_target
                    })
                    .show_ui(ui, |ui| {
                        for category in &categories {
                            ui.separator();
                            ui.label(category.name);
                            for (display_name, path) in &category.params {
                                if ui.selectable_label(
                                    state.new_track_target == *path,
                                    *display_name
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
                    ui.label("Target Y:");
                    egui::ComboBox::from_id_salt("new_track_target_y")
                        .selected_text(if state.new_track_target_y.is_empty() {
                            "Select..."
                        } else {
                            &state.new_track_target_y
                        })
                        .show_ui(ui, |ui| {
                            for category in &categories {
                                ui.separator();
                                ui.label(category.name);
                                for (display_name, path) in &category.params {
                                    if ui.selectable_label(
                                        state.new_track_target_y == *path,
                                        *display_name
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

                if ui.add_enabled(can_add, egui::Button::new("Add")).clicked() {
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

                if ui.button("Cancel").clicked() {
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
                            TrackSource::Keyframes { keyframes } => format!("Keyframes ({})", keyframes.len()),
                            TrackSource::Oscillator { oscillator_type, .. } => format!("{:?}", oscillator_type),
                        };
                        ui.strong(&path);
                        ui.label(format!("({})", track_type_str));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").on_hover_text("Delete track").clicked() {
                                track_to_delete = Some(path.clone());
                            }
                        });
                    });

                    // Track-specific controls
                    match &mut track.source {
                        TrackSource::Keyframes { .. } => {
                            ui.horizontal(|ui| {
                                ui.label("Interpolation:");
                                egui::ComboBox::from_id_salt(format!("interp_{}", path))
                                    .selected_text(format!("{:?}", track.interpolation))
                                    .width(100.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Step, "Step");
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Linear, "Linear");
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Smooth, "Smooth");
                                        ui.selectable_value(&mut track.interpolation, Interpolation::Sinusoidal, "Sinusoidal");
                                    });
                            });

                            if ui.small_button("Edit Keyframes").clicked() {
                                state.editing_keyframes_for = Some(path.clone());
                            }
                        }
                        TrackSource::Oscillator { oscillator_type, center, amplitude, frequency, phase } => {
                            ui.horizontal(|ui| {
                                ui.label("Type:");
                                egui::ComboBox::from_id_salt(format!("osc_type_{}", path))
                                    .selected_text(format!("{:?}", oscillator_type))
                                    .width(80.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(oscillator_type, OscillatorType::Sine, "Sine");
                                        ui.selectable_value(oscillator_type, OscillatorType::Triangle, "Triangle");
                                        ui.selectable_value(oscillator_type, OscillatorType::Sawtooth, "Sawtooth");
                                        ui.selectable_value(oscillator_type, OscillatorType::Square, "Square");
                                    });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Center:");
                                ui.add(egui::DragValue::new(center).speed(0.01));
                                ui.label("Amplitude:");
                                ui.add(egui::DragValue::new(amplitude).speed(0.01));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Frequency:");
                                ui.add(egui::DragValue::new(frequency).speed(0.01).suffix(" Hz"));
                                ui.label("Phase:");
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
                    ui.label("(Circular)");

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").on_hover_text("Delete track").clicked() {
                            circular_to_delete = Some(i);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Center:");
                    ui.add(egui::DragValue::new(&mut circular.center_x).speed(0.01).prefix("X: "));
                    ui.add(egui::DragValue::new(&mut circular.center_y).speed(0.01).prefix("Y: "));
                });

                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    ui.add(egui::DragValue::new(&mut circular.radius).speed(0.01));
                    ui.label("Speed:");
                    ui.add(egui::DragValue::new(&mut circular.speed).speed(0.01).suffix(" rev/s"));
                });

                ui.horizontal(|ui| {
                    ui.label("Phase:");
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
            egui::Window::new(format!("Keyframes: {}", track_path))
                .collapsible(false)
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    // Header row
                    ui.horizontal(|ui| {
                        ui.label("Time");
                        ui.add_space(40.0);
                        ui.label("Value");
                        ui.add_space(40.0);
                        ui.label("Easing");
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
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::Linear, "Linear");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseIn, "EaseIn");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseOut, "EaseOut");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInOut, "EaseInOut");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInCubic, "EaseInCubic");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseOutCubic, "EaseOutCubic");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInOutCubic, "EaseInOutCubic");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInSine, "EaseInSine");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseOutSine, "EaseOutSine");
                                    ui.selectable_value(&mut keyframe.easing, EasingFunction::EaseInOutSine, "EaseInOutSine");
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
                        if ui.button("+ Add Keyframe").clicked() {
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

                        if ui.button("Sort by Time").clicked() {
                            keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                        }

                        if ui.button("Done").clicked() {
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
