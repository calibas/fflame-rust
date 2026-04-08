use crate::scene::transforms::Flame;
use crate::config::{ConfigManager, ConfigPath, AffineParam, UpdateType};
use egui::{Color32, Pos2, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use rust_i18n::t;

/// Mouse interaction modes for the triangle editor
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
enum MouseMode {
    MovePoints,   // Drag individual O, X, Y points
    Translate,    // Drag to move entire triangle
    Rotate,       // Drag to rotate around O
    Scale,        // Drag to scale from O
}

impl Default for MouseMode {
    fn default() -> Self {
        MouseMode::Rotate
    }
}

/// Which affine to edit in the triangle editor
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
enum AffineTarget {
    Pre,
    Post,
}

impl Default for AffineTarget {
    fn default() -> Self {
        AffineTarget::Pre
    }
}

/// Core triangle editor rendering (shared by window and panel)
fn render_triangle_editor_core(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &mut Flame,
) -> UpdateType {
    let mut max_update = UpdateType::None;
            let compact = config_manager.system_settings().compact_mode.unwrap_or(false);
            if !compact {
                ui.heading(t!("triangle_editor.heading"));
                ui.label(t!("triangle_editor.description"));
                ui.separator();
            }

            // Transform selector (persisted across frames)
            // Option<usize>: Some(i) for regular transform, None for final transform
            let mut selected_transform = ui.ctx().data_mut(|d| {
                d.get_persisted::<Option<usize>>(egui::Id::new("triangle_editor_selected_transform"))
                    .unwrap_or(Some(0))
            });

            // Clamp selection to valid range
            if flame.transforms.is_empty() {
                ui.label(t!("triangle_editor.no_transforms"));
                return max_update;
            }
            if let Some(idx) = selected_transform {
                if idx >= flame.transforms.len() {
                    selected_transform = Some(flame.transforms.len() - 1);
                }
            }

            ui.horizontal(|ui| {
                if !compact {
                    ui.label(t!("triangle_editor.transform_label"))
                        .on_hover_text(t!("triangle_editor.tooltip_transform_selector"));
                }
                let old_selection = selected_transform;
                let display_text = match selected_transform {
                    Some(i) => t!("triangle_editor.transform_n", n = i + 1).to_string(),
                    None => t!("triangle_editor.transform_final").to_string(),
                };
                egui::ComboBox::new("triangle_editor_transform_selector", "")
                    .selected_text(display_text)
                    .show_ui(ui, |ui| {
                        // Regular transforms
                        for i in 0..flame.transforms.len() {
                            ui.selectable_value(&mut selected_transform, Some(i), t!("triangle_editor.transform_n", n = i + 1).to_string());
                        }
                        // Final transform (if it exists)
                        if flame.final_transform.is_some() {
                            ui.separator();
                            ui.selectable_value(&mut selected_transform, None, t!("triangle_editor.transform_final").to_string());
                        }
                    });

                // Persist selection if changed
                if selected_transform != old_selection {
                    ui.ctx().data_mut(|d| {
                        d.insert_persisted(egui::Id::new("triangle_editor_selected_transform"), selected_transform);
                    });
                }
            });

            // Mouse mode selector (persisted across frames)
            let mut mouse_mode = ui.ctx().data_mut(|d| {
                d.get_persisted::<MouseMode>(egui::Id::new("triangle_editor_mouse_mode"))
                    .unwrap_or_default()
            });

            ui.horizontal_wrapped(|ui| {
                if !compact {
                    ui.label(t!("triangle_editor.mode_label"));
                }
                let old_mode = mouse_mode;

                if ui.selectable_value(&mut mouse_mode, MouseMode::Rotate, format!("↻ {}", t!("triangle_editor.mode_rotate")))
                    .on_hover_text(t!("triangle_editor.tooltip_mode_rotate")).changed() {}
                if ui.selectable_value(&mut mouse_mode, MouseMode::Translate, format!("↔ {}", t!("triangle_editor.mode_translate")))
                    .on_hover_text(t!("triangle_editor.tooltip_mode_translate")).changed() {}
                if ui.selectable_value(&mut mouse_mode, MouseMode::Scale, format!("🔺 {}", t!("triangle_editor.mode_scale")))
                    .on_hover_text(t!("triangle_editor.tooltip_mode_scale")).changed() {}
                if ui.selectable_value(&mut mouse_mode, MouseMode::MovePoints, format!("✏ {}", t!("triangle_editor.mode_move_points")))
                    .on_hover_text(t!("triangle_editor.tooltip_mode_move_points")).changed() {}

                // Persist mode if changed
                if mouse_mode != old_mode {
                    ui.ctx().data_mut(|d| {
                        d.insert_persisted(egui::Id::new("triangle_editor_mouse_mode"), mouse_mode);
                    });
                }
            });

            // Mode description
            if !compact {
                let mode_desc = match mouse_mode {
                    MouseMode::MovePoints => t!("triangle_editor.mode_desc_move_points"),
                    MouseMode::Translate => t!("triangle_editor.mode_desc_translate"),
                    MouseMode::Rotate => t!("triangle_editor.mode_desc_rotate"),
                    MouseMode::Scale => t!("triangle_editor.mode_desc_scale"),
                };
                ui.label(egui::RichText::new(mode_desc.as_ref()).italics().small());
            }

            // Pre/Post affine toggle - only show when selected transform has post-affine enabled
            let selected_has_post_affine = match selected_transform {
                Some(idx) => flame.transforms.get(idx).map(|t| t.post_affine_enabled).unwrap_or(false),
                None => flame.final_transform.as_ref().map(|t| t.post_affine_enabled).unwrap_or(false),
            };

            let mut affine_target = ui.ctx().data_mut(|d| {
                d.get_persisted::<AffineTarget>(egui::Id::new("triangle_editor_affine_target"))
                    .unwrap_or_default()
            });

            // Reset to Pre if post-affine not enabled
            if !selected_has_post_affine {
                affine_target = AffineTarget::Pre;
            }

            if selected_has_post_affine {
                ui.horizontal(|ui| {
                    ui.label(t!("triangle_editor.affine_target"));
                    let old_target = affine_target;
                    ui.selectable_value(&mut affine_target, AffineTarget::Pre, t!("triangle_editor.affine_pre"))
                        .on_hover_text(t!("triangle_editor.tooltip_affine_pre"));
                    ui.selectable_value(&mut affine_target, AffineTarget::Post, t!("triangle_editor.affine_post"))
                        .on_hover_text(t!("triangle_editor.tooltip_affine_post"));
                    if affine_target != old_target {
                        ui.ctx().data_mut(|d| {
                            d.insert_persisted(egui::Id::new("triangle_editor_affine_target"), affine_target);
                        });
                    }
                });
            }

            ui.separator();

            // Canvas for drawing triangles (smaller in compact mode to avoid scroll conflicts)
            let max_side = if compact { 200.0 } else { 500.0 };
            let canvas_side = ui.available_width().clamp(100.0, max_side);
            let canvas_size = Vec2::new(canvas_side, canvas_side);
            let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::drag());
            let rect = response.rect;

            // Calculate dynamic bounds from the selected transform's vertices
            let mut max_extent = 2.0f32; // Minimum bounds
            let selected_xform = match selected_transform {
                Some(i) => flame.transforms.get(i),
                None => flame.final_transform.as_ref(),
            };
            if let Some(transform) = selected_xform {
                let (o, x, y) = transform.to_triangle_apophysis();
                max_extent = max_extent
                    .max(o[0].abs())
                    .max(o[1].abs())
                    .max(x[0].abs())
                    .max(x[1].abs())
                    .max(y[0].abs())
                    .max(y[1].abs());
                if transform.post_affine_enabled {
                    let (po, px, py) = transform.post_to_triangle_apophysis();
                    max_extent = max_extent
                        .max(po[0].abs()).max(po[1].abs())
                        .max(px[0].abs()).max(px[1].abs())
                        .max(py[0].abs()).max(py[1].abs());
                }
            }
            // Add flat padding (not percentage) to prevent feedback loop when dragging near edge
            // Using percentage (e.g., * 1.2) causes: drag → extent grows → point moves → extent grows more
            let target_extent = max_extent + 0.5;

            // Get previous extent from persisted state
            let prev_extent = ui.ctx().data_mut(|d| {
                d.get_persisted::<f32>(egui::Id::new("triangle_editor_extent"))
                    .unwrap_or(target_extent)
            });

            // While dragging, only allow extent to grow (never shrink) to prevent jumpiness
            // Shrinking only happens on mouse release
            let is_dragging = response.dragged();
            let padded_extent = if is_dragging {
                prev_extent.max(target_extent) // Only grow while dragging
            } else {
                target_extent // Allow shrinking on release
            };

            // Persist the current extent for next frame
            ui.ctx().data_mut(|d| {
                d.insert_persisted(egui::Id::new("triangle_editor_extent"), padded_extent);
            });

            // Define coordinate mapping: fractal space [-extent, extent] → canvas pixels
            let world_min = -padded_extent;
            let world_max = padded_extent;
            let world_size = world_max - world_min;

            // Helper function: world coordinates → canvas pixels
            let to_canvas = |world_pos: [f32; 2]| -> Pos2 {
                let x = rect.min.x + ((world_pos[0] - world_min) / world_size) * rect.width();
                let y = rect.max.y - ((world_pos[1] - world_min) / world_size) * rect.height(); // Y flipped
                Pos2::new(x, y)
            };

            // Helper function: canvas pixels → world coordinates
            let to_world = |canvas_pos: Pos2| -> [f32; 2] {
                let x = world_min + ((canvas_pos.x - rect.min.x) / rect.width()) * world_size;
                let y = world_max - ((canvas_pos.y - rect.min.y) / rect.height()) * world_size; // Y flipped
                [x, y]
            };

            // Draw canvas background
            painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 20));

            // Draw X/Y axes only (no grid lines)
            let axis_color = Color32::from_rgb(180, 180, 180);

            // X axis (horizontal line at y=0)
            let x_axis_start = to_canvas([world_min, 0.0]);
            let x_axis_end = to_canvas([world_max, 0.0]);
            painter.line_segment([x_axis_start, x_axis_end], Stroke::new(1.0, axis_color));

            // Y axis (vertical line at x=0)
            let y_axis_start = to_canvas([0.0, world_min]);
            let y_axis_end = to_canvas([0.0, world_max]);
            painter.line_segment([y_axis_start, y_axis_end], Stroke::new(1.0, axis_color));

            // === INTERACTION: Mouse dragging with different modes ===
            #[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
            enum DragTarget {
                None,
                Origin,
                XPoint,
                YPoint,
            }

            // Persistent drag state
            let mut drag_target = ui.ctx().data_mut(|d| {
                d.get_persisted::<DragTarget>(egui::Id::new("triangle_editor_drag_target"))
                    .unwrap_or(DragTarget::None)
            });

            // Drag start position (for relative operations)
            let mut drag_start_pos = ui.ctx().data_mut(|d| {
                d.get_persisted::<Option<Pos2>>(egui::Id::new("triangle_editor_drag_start_pos"))
                    .unwrap_or(None)
            });

            // Helper to create affine changes for either regular or final transform
            // Supports both pre-affine and post-affine based on affine_target
            let make_affine_changes = |xform: &crate::scene::transforms::Transform| -> Vec<(ConfigPath, crate::config::ConfigValue)> {
                match (selected_transform, affine_target) {
                    (Some(index), AffineTarget::Pre) => vec![
                        (ConfigPath::TransformAffine { index, param: AffineParam::A }, xform.a.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::B }, xform.b.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::C }, xform.c.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::D }, xform.d.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::E }, xform.e.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::F }, xform.f.into()),
                    ],
                    (Some(index), AffineTarget::Post) => vec![
                        (ConfigPath::TransformPostAffine { index, param: AffineParam::A }, xform.post_a.into()),
                        (ConfigPath::TransformPostAffine { index, param: AffineParam::B }, xform.post_b.into()),
                        (ConfigPath::TransformPostAffine { index, param: AffineParam::C }, xform.post_c.into()),
                        (ConfigPath::TransformPostAffine { index, param: AffineParam::D }, xform.post_d.into()),
                        (ConfigPath::TransformPostAffine { index, param: AffineParam::E }, xform.post_e.into()),
                        (ConfigPath::TransformPostAffine { index, param: AffineParam::F }, xform.post_f.into()),
                    ],
                    (None, AffineTarget::Pre) => vec![
                        (ConfigPath::FinalTransformAffine { param: AffineParam::A }, xform.a.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::B }, xform.b.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::C }, xform.c.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::D }, xform.d.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::E }, xform.e.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::F }, xform.f.into()),
                    ],
                    (None, AffineTarget::Post) => vec![
                        (ConfigPath::FinalTransformPostAffine { param: AffineParam::A }, xform.post_a.into()),
                        (ConfigPath::FinalTransformPostAffine { param: AffineParam::B }, xform.post_b.into()),
                        (ConfigPath::FinalTransformPostAffine { param: AffineParam::C }, xform.post_c.into()),
                        (ConfigPath::FinalTransformPostAffine { param: AffineParam::D }, xform.post_d.into()),
                        (ConfigPath::FinalTransformPostAffine { param: AffineParam::E }, xform.post_e.into()),
                        (ConfigPath::FinalTransformPostAffine { param: AffineParam::F }, xform.post_f.into()),
                    ],
                }
            };

            // Helper to sync transform back from config
            let sync_transform = |xform: &mut crate::scene::transforms::Transform, cfg_mgr: &ConfigManager| {
                match selected_transform {
                    Some(index) => *xform = cfg_mgr.active_config().flame.transforms[index].clone(),
                    None => *xform = cfg_mgr.active_config().flame.final_transform.as_ref().unwrap().clone(),
                }
            };

            // Helper for display text
            let transform_name = match selected_transform {
                Some(i) => t!("triangle_editor.transform_n", n = i + 1).to_string(),
                None => t!("triangle_editor.transform_final").to_string(),
            };

            // Get current triangle for selected transform (regular or final)
            let transform_mut = match selected_transform {
                Some(idx) => flame.transforms.get_mut(idx),
                None => flame.final_transform.as_mut(),
            };

            if let Some(transform) = transform_mut {
                let (mut o, mut x, mut y) = match affine_target {
                    AffineTarget::Pre => transform.to_triangle_apophysis(),
                    AffineTarget::Post => transform.post_to_triangle_apophysis(),
                };

                let o_pos = to_canvas(o);
                let x_pos = to_canvas(x);
                let y_pos = to_canvas(y);

                // Handle different mouse modes
                match mouse_mode {
                    MouseMode::MovePoints => {
                        let hit_radius = 10.0; // Pixels

                        // Check if we're starting a new drag
                        if drag_target == DragTarget::None && response.dragged() {
                            if let Some(pos) = response.interact_pointer_pos() {
                                if pos.distance(o_pos) < hit_radius {
                                    drag_target = DragTarget::Origin;
                                } else if pos.distance(x_pos) < hit_radius {
                                    drag_target = DragTarget::XPoint;
                                } else if pos.distance(y_pos) < hit_radius {
                                    drag_target = DragTarget::YPoint;
                                }

                                // Start modify session if we found a hit
                                if drag_target != DragTarget::None {
                                    if let Some(index) = selected_transform {
                                        let _ = config_manager.start_modify_transform(index);
                                    }
                                }
                            }
                        }

                        // Update position during drag
                        if drag_target != DragTarget::None && response.dragged() {
                            if let Some(mouse_pos) = response.interact_pointer_pos() {
                                let world_pos = to_world(mouse_pos);

                                match drag_target {
                                    DragTarget::Origin => o = world_pos,
                                    DragTarget::XPoint => x = world_pos,
                                    DragTarget::YPoint => y = world_pos,
                                    DragTarget::None => {}
                                }

                                // Apply triangle changes via update_batch
                                match affine_target {
                                    AffineTarget::Pre => transform.from_triangle_apophysis(o, x, y),
                                    AffineTarget::Post => transform.post_from_triangle_apophysis(o, x, y),
                                }
                                let changes = make_affine_changes(transform);
                                if let Ok(update_type) = config_manager.update_batch(changes, "history.action.triangle_edit_move".to_string()) {
                                    // Sync transform from active_config for live preview
                                    sync_transform(transform, config_manager);
                                    max_update = max_update.max(update_type);
                                }
                            }
                        }
                    }

                    MouseMode::Translate => {
                        // Start drag on any point in canvas
                        if drag_start_pos.is_none() && response.dragged() {
                            drag_start_pos = response.interact_pointer_pos();
                            // Start modify session
                            if let Some(index) = selected_transform {
                                let _ = config_manager.start_modify_transform(index);
                            }
                        }

                        // Translate entire triangle
                        if let (Some(start_pos), Some(current_pos)) = (drag_start_pos, response.interact_pointer_pos()) {
                            if response.dragged() {
                                let delta = current_pos - start_pos;
                                let world_delta = [
                                    delta.x * world_size / rect.width(),
                                    -delta.y * world_size / rect.height(), // Y flipped
                                ];

                                o[0] += world_delta[0];
                                o[1] += world_delta[1];
                                x[0] += world_delta[0];
                                x[1] += world_delta[1];
                                y[0] += world_delta[0];
                                y[1] += world_delta[1];

                                // Apply triangle changes via update_batch
                                match affine_target {
                                    AffineTarget::Pre => transform.from_triangle_apophysis(o, x, y),
                                    AffineTarget::Post => transform.post_from_triangle_apophysis(o, x, y),
                                }
                                let changes = make_affine_changes(transform);
                                if let Ok(update_type) = config_manager.update_batch(changes, "history.action.triangle_edit_translate".to_string()) {
                                    sync_transform(transform, config_manager);
                                    max_update = max_update.max(update_type);
                                }

                                drag_start_pos = Some(current_pos);
                            }
                        }
                    }

                    MouseMode::Rotate => {
                        // Capture initial mouse position
                        if drag_start_pos.is_none() && response.dragged() {
                            drag_start_pos = response.interact_pointer_pos();
                            // Start modify session
                            if let Some(index) = selected_transform {
                                let _ = config_manager.start_modify_transform(index);
                            }
                        }

                        // Rotate X and Y around O based on angle from O
                        if let (Some(start_pos), Some(current_pos)) = (drag_start_pos, response.interact_pointer_pos()) {
                            if response.dragged() {
                                // Calculate angles from O to start and current mouse positions
                                let start_vec = start_pos - o_pos;
                                let current_vec = current_pos - o_pos;

                                let angle_start = start_vec.y.atan2(start_vec.x);
                                let angle_current = current_vec.y.atan2(current_vec.x);
                                let angle_delta = -(angle_current - angle_start); // Negate for correct direction

                                // Rotate X and Y vectors around O
                                let x_vec = [x[0] - o[0], x[1] - o[1]];
                                let y_vec = [y[0] - o[0], y[1] - o[1]];

                                let cos_a = angle_delta.cos();
                                let sin_a = angle_delta.sin();

                                let x_rot = [
                                    cos_a * x_vec[0] - sin_a * x_vec[1],
                                    sin_a * x_vec[0] + cos_a * x_vec[1],
                                ];
                                let y_rot = [
                                    cos_a * y_vec[0] - sin_a * y_vec[1],
                                    sin_a * y_vec[0] + cos_a * y_vec[1],
                                ];

                                x = [o[0] + x_rot[0], o[1] + x_rot[1]];
                                y = [o[0] + y_rot[0], o[1] + y_rot[1]];

                                // Apply triangle changes via update_batch
                                match affine_target {
                                    AffineTarget::Pre => transform.from_triangle_apophysis(o, x, y),
                                    AffineTarget::Post => transform.post_from_triangle_apophysis(o, x, y),
                                }
                                let changes = make_affine_changes(transform);
                                if let Ok(update_type) = config_manager.update_batch(changes, "history.action.triangle_edit_rotate".to_string()) {
                                    sync_transform(transform, config_manager);
                                    max_update = max_update.max(update_type);
                                }

                                drag_start_pos = Some(current_pos);
                            }
                        }
                    }

                    MouseMode::Scale => {
                        // Capture initial mouse position
                        if drag_start_pos.is_none() && response.dragged() {
                            drag_start_pos = response.interact_pointer_pos();
                            // Start modify session
                            if let Some(index) = selected_transform {
                                let _ = config_manager.start_modify_transform(index);
                            }
                        }

                        // Scale along perpendicular axis to X-Y line
                        if let (Some(start_pos), Some(current_pos)) = (drag_start_pos, response.interact_pointer_pos()) {
                            if response.dragged() {
                                // Calculate X-Y vector in world space
                                let xy_vec = [x[0] - y[0], x[1] - y[1]];
                                let xy_len = (xy_vec[0] * xy_vec[0] + xy_vec[1] * xy_vec[1]).sqrt();

                                if xy_len > 1e-6 {
                                    // Perpendicular vector to X-Y line (rotate 90°)
                                    let perp_vec = [-xy_vec[1] / xy_len, xy_vec[0] / xy_len];

                                    // Convert O to canvas space
                                    let o_canvas = to_canvas(o);

                                    // Calculate mouse movement vector
                                    let mouse_delta = [current_pos.x - start_pos.x, current_pos.y - start_pos.y];

                                    // Convert perpendicular vector to canvas space direction
                                    let perp_canvas_end = to_canvas([o[0] + perp_vec[0], o[1] + perp_vec[1]]);
                                    let perp_canvas_vec = [perp_canvas_end.x - o_canvas.x, perp_canvas_end.y - o_canvas.y];
                                    let perp_canvas_len = (perp_canvas_vec[0] * perp_canvas_vec[0] + perp_canvas_vec[1] * perp_canvas_vec[1]).sqrt();

                                    if perp_canvas_len > 1e-6 {
                                        let perp_canvas_normalized = [perp_canvas_vec[0] / perp_canvas_len, perp_canvas_vec[1] / perp_canvas_len];

                                        // Project mouse movement onto perpendicular axis
                                        let projection = mouse_delta[0] * perp_canvas_normalized[0] + mouse_delta[1] * perp_canvas_normalized[1];

                                        // Scale factor: further from origin = scale up, closer = scale down
                                        let scale_factor = 1.0 + projection * 0.005;

                                        // Scale X and Y vectors from O
                                        let x_vec = [x[0] - o[0], x[1] - o[1]];
                                        let y_vec = [y[0] - o[0], y[1] - o[1]];

                                        x = [o[0] + x_vec[0] * scale_factor, o[1] + x_vec[1] * scale_factor];
                                        y = [o[0] + y_vec[0] * scale_factor, o[1] + y_vec[1] * scale_factor];

                                        // Apply triangle changes via update_batch
                                        match affine_target {
                                            AffineTarget::Pre => transform.from_triangle_apophysis(o, x, y),
                                            AffineTarget::Post => transform.post_from_triangle_apophysis(o, x, y),
                                        }
                                        let changes = make_affine_changes(transform);
                                        if let Ok(update_type) = config_manager.update_batch(changes, "history.action.triangle_edit_scale".to_string()) {
                                            sync_transform(transform, config_manager);
                                            max_update = max_update.max(update_type);
                                        }
                                    }
                                }

                                drag_start_pos = Some(current_pos);
                            }
                        }
                    }
                }

                // Clear drag on release
                let was_dragging = drag_target != DragTarget::None || drag_start_pos.is_some();
                if !response.dragged() && was_dragging {
                    // Drag ended - commit modify session if active
                    if config_manager.is_in_modify_session() {
                        let mode_name = match mouse_mode {
                            MouseMode::MovePoints => "Move Points",
                            MouseMode::Translate => "Translate",
                            MouseMode::Rotate => "Rotate",
                            MouseMode::Scale => "Scale",
                        };
                        let description = match selected_transform {
                            Some(i) => format!("Triangle Edit {} (Transform {})", mode_name, i + 1),
                            None => format!("Triangle Edit {} (Final)", mode_name),
                        };
                        if let Ok(update) = config_manager.commit_modify_transform(description) {
                            max_update = max_update.max(update);
                        }
                    }
                    drag_target = DragTarget::None;
                    drag_start_pos = None;
                }

                // Persist drag state
                ui.ctx().data_mut(|d| {
                    d.insert_persisted(egui::Id::new("triangle_editor_drag_target"), drag_target);
                    d.insert_persisted(egui::Id::new("triangle_editor_drag_start_pos"), drag_start_pos);
                });
            }

            // Draw all transforms as semi-transparent triangles
            for (i, transform) in flame.transforms.iter().enumerate() {
                let (o, x, y) = transform.to_triangle_apophysis();

                let o_pos = to_canvas(o);
                let x_pos = to_canvas(x);
                let y_pos = to_canvas(y);

                // Color per transform
                let base_color = get_transform_color(i);
                let alpha = if Some(i) == selected_transform { 255 } else { 80 };
                let color = Color32::from_rgba_unmultiplied(
                    base_color.r(),
                    base_color.g(),
                    base_color.b(),
                    alpha,
                );

                // Draw lines O→X and O→Y
                painter.line_segment([o_pos, x_pos], Stroke::new(2.0, color));
                painter.line_segment([o_pos, y_pos], Stroke::new(2.0, color));

                // Draw semi-transparent line between X and Y to complete the triangle
                let transparent_color = Color32::from_rgba_unmultiplied(
                    base_color.r(),
                    base_color.g(),
                    base_color.b(),
                    (alpha as f32 * 0.5) as u8,  // 50% of current alpha for semi-transparency
                );
                painter.line_segment([x_pos, y_pos], Stroke::new(1.5, transparent_color));

                // Draw points with highlighting for active drag target
                let point_radius = if Some(i) == selected_transform { 6.0 } else { 4.0 };

                // Highlight the point being dragged
                if Some(i) == selected_transform {
                    let highlight_radius = point_radius + 3.0;
                    match drag_target {
                        DragTarget::Origin => {
                            painter.circle_stroke(o_pos, highlight_radius, Stroke::new(2.0, Color32::WHITE));
                        }
                        DragTarget::XPoint => {
                            painter.circle_stroke(x_pos, highlight_radius, Stroke::new(2.0, Color32::WHITE));
                        }
                        DragTarget::YPoint => {
                            painter.circle_stroke(y_pos, highlight_radius, Stroke::new(2.0, Color32::WHITE));
                        }
                        DragTarget::None => {}
                    }
                }

                painter.circle_filled(o_pos, point_radius, color);
                painter.circle_filled(x_pos, point_radius, color);
                painter.circle_filled(y_pos, point_radius, color);

                // Labels for selected transform
                if Some(i) == selected_transform {
                    painter.text(
                        o_pos + Vec2::new(-15.0, -15.0),
                        egui::Align2::CENTER_CENTER,
                        "O",
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        x_pos + Vec2::new(10.0, -10.0),
                        egui::Align2::CENTER_CENTER,
                        "X",
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        y_pos + Vec2::new(10.0, -10.0),
                        egui::Align2::CENTER_CENTER,
                        "Y",
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                }
            }

            // Draw post-affine triangles for transforms that have post-affine enabled
            for (i, transform) in flame.transforms.iter().enumerate() {
                if !transform.post_affine_enabled {
                    continue;
                }
                let (o, x, y) = transform.post_to_triangle_apophysis();

                let o_pos = to_canvas(o);
                let x_pos = to_canvas(x);
                let y_pos = to_canvas(y);

                let base_color = get_transform_color(i);
                // Dimmer than pre-affine, brighter when selected in post mode
                let is_active_post = Some(i) == selected_transform && affine_target == AffineTarget::Post;
                let alpha: u8 = if is_active_post { 200 } else { 50 };
                let color = Color32::from_rgba_unmultiplied(
                    base_color.r(),
                    base_color.g(),
                    base_color.b(),
                    alpha,
                );

                // Draw dashed lines for post-affine triangles
                let draw_dashed = |start: Pos2, end: Pos2| {
                    let dash_length = 6.0;
                    let gap_length = 4.0;
                    let total_length = start.distance(end);
                    if total_length < 0.001 { return; }
                    let direction = (end - start) / total_length;
                    let mut pos = 0.0;
                    while pos < total_length {
                        let dash_start = start + direction * pos;
                        let dash_end_pos = (pos + dash_length).min(total_length);
                        let dash_end = start + direction * dash_end_pos;
                        painter.line_segment([dash_start, dash_end], Stroke::new(1.5, color));
                        pos += dash_length + gap_length;
                    }
                };

                draw_dashed(o_pos, x_pos);
                draw_dashed(o_pos, y_pos);
                draw_dashed(x_pos, y_pos);

                // Draw small square points for post-affine (to distinguish from pre-affine circles)
                let point_size = if is_active_post { 5.0 } else { 3.0 };
                let half = point_size / 2.0;
                for pos in [o_pos, x_pos, y_pos] {
                    painter.rect_filled(
                        egui::Rect::from_min_size(Pos2::new(pos.x - half, pos.y - half), Vec2::splat(point_size)),
                        0.0,
                        color,
                    );
                }

                // Labels for selected post-affine
                if is_active_post {
                    for (pos, label) in [(o_pos, "O'"), (x_pos, "X'"), (y_pos, "Y'")] {
                        painter.text(
                            pos + Vec2::new(-15.0, -15.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(12.0),
                            color,
                        );
                    }
                }
            }

            // Draw final transform if present (light grey, distinct style)
            if let Some(final_xform) = &flame.final_transform {
                let (o, x, y) = final_xform.to_triangle_apophysis();

                let o_pos = to_canvas(o);
                let x_pos = to_canvas(x);
                let y_pos = to_canvas(y);

                // Light grey color for final transform
                let final_color = Color32::from_rgb(180, 180, 180);
                let alpha = 200; // Semi-transparent to distinguish from regular transforms

                let color = Color32::from_rgba_unmultiplied(
                    final_color.r(),
                    final_color.g(),
                    final_color.b(),
                    alpha,
                );

                // Draw lines with dashed style (simulated with dots)
                // Draw O→X and O→Y with dashed appearance
                let draw_dashed_line = |start: Pos2, end: Pos2| {
                    let dash_length = 10.0;
                    let gap_length = 5.0;
                    let total_length = start.distance(end);
                    let direction = (end - start) / total_length;

                    let mut pos = 0.0;
                    while pos < total_length {
                        let dash_start = start + direction * pos;
                        let dash_end_pos = (pos + dash_length).min(total_length);
                        let dash_end = start + direction * dash_end_pos;
                        painter.line_segment([dash_start, dash_end], Stroke::new(2.0, color));
                        pos += dash_length + gap_length;
                    }
                };

                draw_dashed_line(o_pos, x_pos);
                draw_dashed_line(o_pos, y_pos);
                draw_dashed_line(x_pos, y_pos);

                // Draw points (larger to make them stand out)
                let point_radius = 7.0;
                painter.circle_filled(o_pos, point_radius, color);
                painter.circle_filled(x_pos, point_radius, color);
                painter.circle_filled(y_pos, point_radius, color);

                // Labels only when final transform is selected
                if selected_transform.is_none() {
                    painter.text(
                        o_pos + Vec2::new(-15.0, 15.0),
                        egui::Align2::CENTER_CENTER,
                        "O",
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        x_pos + Vec2::new(10.0, 10.0),
                        egui::Align2::CENTER_CENTER,
                        "X",
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        y_pos + Vec2::new(10.0, 10.0),
                        egui::Align2::CENTER_CENTER,
                        "Y",
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                }
            }

            ui.separator();

            // Editable coordinates for selected transform
            let transform_for_coords = match selected_transform {
                Some(idx) => flame.transforms.get_mut(idx),
                None => flame.final_transform.as_mut(),
            };

            if let Some(transform) = transform_for_coords {
                // --- Quick action buttons (directly under canvas) ---
                {
                    // Helper macro to get current triangle based on affine target
                    macro_rules! get_triangle {
                        ($t:expr) => {
                            match affine_target {
                                AffineTarget::Pre => $t.to_triangle_apophysis(),
                                AffineTarget::Post => $t.post_to_triangle_apophysis(),
                            }
                        };
                    }

                    // Helper closure to apply triangle changes via ConfigManager
                    let mut apply_triangle_change = |transform_ref: &crate::scene::transforms::Transform,
                                                       o: [f32; 2], x: [f32; 2], y: [f32; 2],
                                                       description: &str| {
                        let mut temp = transform_ref.clone();
                        match affine_target {
                            AffineTarget::Pre => temp.from_triangle_apophysis(o, x, y),
                            AffineTarget::Post => temp.post_from_triangle_apophysis(o, x, y),
                        }
                        let changes = make_affine_changes(&temp);
                        config_manager.update_batch(changes, description.to_string())
                    };

                    // Quick actions: 5-column grid (2 rows)
                    // Row 1: [empty] [^ Up] [empty] [↻ CW] [🔺 Up]
                    // Row 2: [< Left] [v Down] [> Right] [↺ CCW] [🔻 Down]
                    egui::Grid::new("triangle_quick_actions").min_col_width(0.0).show(ui, |ui| {
                        // Row 1
                        ui.label("");
                        if ui.button(" ^ ").on_hover_text(t!("triangle_editor.tooltip_translate_up")).clicked() {
                            let (mut o_new, mut x_new, mut y_new) = get_triangle!(transform);
                            o_new[1] += 0.1; x_new[1] += 0.1; y_new[1] += 0.1;
                            if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                &format!("history.action.translate_up|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        ui.label("");
                        if ui.button("↻")
                            .on_hover_text(t!("triangle_editor.tooltip_rotate_cw")).clicked()
                        {
                            let angle = -15.0_f32.to_radians();
                            let (cos_a, sin_a) = (angle.cos(), angle.sin());
                            let (o_curr, x_curr, y_curr) = get_triangle!(transform);
                            let x_vec = [x_curr[0] - o_curr[0], x_curr[1] - o_curr[1]];
                            let y_vec = [y_curr[0] - o_curr[0], y_curr[1] - o_curr[1]];
                            let x_new = [o_curr[0] + x_vec[0]*cos_a - x_vec[1]*sin_a, o_curr[1] + x_vec[0]*sin_a + x_vec[1]*cos_a];
                            let y_new = [o_curr[0] + y_vec[0]*cos_a - y_vec[1]*sin_a, o_curr[1] + y_vec[0]*sin_a + y_vec[1]*cos_a];
                            if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                &format!("history.action.rotate_cw|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.button(format!("🔺 {}", t!("triangle_editor.scale")))
                            .on_hover_text(t!("triangle_editor.tooltip_scale_up")).clicked()
                        {
                            let (o_curr, x_curr, y_curr) = get_triangle!(transform);
                            let x_new = [o_curr[0] + (x_curr[0]-o_curr[0])*1.1, o_curr[1] + (x_curr[1]-o_curr[1])*1.1];
                            let y_new = [o_curr[0] + (y_curr[0]-o_curr[0])*1.1, o_curr[1] + (y_curr[1]-o_curr[1])*1.1];
                            if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                &format!("history.action.scale_up|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        ui.end_row();

                        // Row 2
                        if ui.button(" < ").on_hover_text(t!("triangle_editor.tooltip_translate_left")).clicked() {
                            let (mut o_new, mut x_new, mut y_new) = get_triangle!(transform);
                            o_new[0] -= 0.1; x_new[0] -= 0.1; y_new[0] -= 0.1;
                            if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                &format!("history.action.translate_left|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.button(" v ").on_hover_text(t!("triangle_editor.tooltip_translate_down")).clicked() {
                            let (mut o_new, mut x_new, mut y_new) = get_triangle!(transform);
                            o_new[1] -= 0.1; x_new[1] -= 0.1; y_new[1] -= 0.1;
                            if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                &format!("history.action.translate_down|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.button(" > ").on_hover_text(t!("triangle_editor.tooltip_translate_right")).clicked() {
                            let (mut o_new, mut x_new, mut y_new) = get_triangle!(transform);
                            o_new[0] += 0.1; x_new[0] += 0.1; y_new[0] += 0.1;
                            if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                &format!("history.action.translate_right|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.button("↺")
                            .on_hover_text(t!("triangle_editor.tooltip_rotate_ccw")).clicked()
                        {
                            let angle = 15.0_f32.to_radians();
                            let (cos_a, sin_a) = (angle.cos(), angle.sin());
                            let (o_curr, x_curr, y_curr) = get_triangle!(transform);
                            let x_vec = [x_curr[0] - o_curr[0], x_curr[1] - o_curr[1]];
                            let y_vec = [y_curr[0] - o_curr[0], y_curr[1] - o_curr[1]];
                            let x_new = [o_curr[0] + x_vec[0]*cos_a - x_vec[1]*sin_a, o_curr[1] + x_vec[0]*sin_a + x_vec[1]*cos_a];
                            let y_new = [o_curr[0] + y_vec[0]*cos_a - y_vec[1]*sin_a, o_curr[1] + y_vec[0]*sin_a + y_vec[1]*cos_a];
                            if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                &format!("history.action.rotate_ccw|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.button(format!("🔻 {}", t!("triangle_editor.scale")))
                            .on_hover_text(t!("triangle_editor.tooltip_scale_down")).clicked()
                        {
                            let (o_curr, x_curr, y_curr) = get_triangle!(transform);
                            let x_new = [o_curr[0] + (x_curr[0]-o_curr[0])*0.9, o_curr[1] + (x_curr[1]-o_curr[1])*0.9];
                            let y_new = [o_curr[0] + (y_curr[0]-o_curr[0])*0.9, o_curr[1] + (y_curr[1]-o_curr[1])*0.9];
                            if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                &format!("history.action.scale_down|name={}", transform_name)) {
                                max_update = max_update.max(update);
                            }
                        }
                        ui.end_row();
                    });
                }

                ui.separator();

                // --- Tabbed view: Triangle Coords / Affine Coefficients ---
                #[derive(Clone, Copy, PartialEq)]
                enum CoordTab { TriangleCoords, AffineCoeffs }

                let mut coord_tab = ui.ctx().data_mut(|d| {
                    d.get_persisted::<u8>(egui::Id::new("triangle_editor_coord_tab"))
                        .map(|v| if v == 1 { CoordTab::AffineCoeffs } else { CoordTab::TriangleCoords })
                        .unwrap_or(CoordTab::TriangleCoords)
                });

                ui.horizontal(|ui| {
                    if ui.selectable_label(coord_tab == CoordTab::TriangleCoords, t!("triangle_editor.triangle_coords_tab")).clicked() {
                        coord_tab = CoordTab::TriangleCoords;
                    }
                    if ui.selectable_label(coord_tab == CoordTab::AffineCoeffs, t!("triangle_editor.affine_coeffs_tab")).clicked() {
                        coord_tab = CoordTab::AffineCoeffs;
                    }
                });

                ui.ctx().data_mut(|d| {
                    d.insert_persisted(egui::Id::new("triangle_editor_coord_tab"),
                        match coord_tab { CoordTab::TriangleCoords => 0u8, CoordTab::AffineCoeffs => 1u8 });
                });

                match coord_tab {
                    CoordTab::TriangleCoords => {
                        let (mut o, mut x, mut y) = match affine_target {
                            AffineTarget::Pre => transform.to_triangle_apophysis(),
                            AffineTarget::Post => transform.post_to_triangle_apophysis(),
                        };

                        let mut coords_changed = false;
                        let mut dragging = false;
                        let mut drag_stopped = false;

                        ui.horizontal(|ui| {
                            ui.label(t!("triangle_editor.point_x")).on_hover_text(t!("triangle_editor.tooltip_point_x"));
                            let x0_resp = ui.add(super::VkbDragValue::new(&mut x[0]).speed(0.01).prefix(format!("{} ", t!("triangle_editor.coord_x"))));
                            let x1_resp = ui.add(super::VkbDragValue::new(&mut x[1]).speed(0.01).prefix(format!("{} ", t!("triangle_editor.coord_y"))));
                            coords_changed |= x0_resp.changed() || x1_resp.changed();
                            dragging |= x0_resp.dragged() || x1_resp.dragged();
                            drag_stopped |= x0_resp.drag_stopped() || x1_resp.drag_stopped();
                        });
                        ui.horizontal(|ui| {
                            ui.label(t!("triangle_editor.point_y")).on_hover_text(t!("triangle_editor.tooltip_point_y"));
                            let y0_resp = ui.add(super::VkbDragValue::new(&mut y[0]).speed(0.01).prefix(format!("{} ", t!("triangle_editor.coord_x"))));
                            let y1_resp = ui.add(super::VkbDragValue::new(&mut y[1]).speed(0.01).prefix(format!("{} ", t!("triangle_editor.coord_y"))));
                            coords_changed |= y0_resp.changed() || y1_resp.changed();
                            dragging |= y0_resp.dragged() || y1_resp.dragged();
                            drag_stopped |= y0_resp.drag_stopped() || y1_resp.drag_stopped();
                        });
                        ui.horizontal(|ui| {
                            ui.label(t!("triangle_editor.point_o")).on_hover_text(t!("triangle_editor.tooltip_point_o"));
                            let o0_resp = ui.add(super::VkbDragValue::new(&mut o[0]).speed(0.01).prefix(format!("{} ", t!("triangle_editor.coord_x"))));
                            let o1_resp = ui.add(super::VkbDragValue::new(&mut o[1]).speed(0.01).prefix(format!("{} ", t!("triangle_editor.coord_y"))));
                            coords_changed |= o0_resp.changed() || o1_resp.changed();
                            dragging |= o0_resp.dragged() || o1_resp.dragged();
                            drag_stopped |= o0_resp.drag_stopped() || o1_resp.drag_stopped();
                        });

                        if coords_changed {
                            let mut temp_transform = transform.clone();
                            match affine_target {
                                AffineTarget::Pre => temp_transform.from_triangle_apophysis(o, x, y),
                                AffineTarget::Post => temp_transform.post_from_triangle_apophysis(o, x, y),
                            }
                            let changes = make_affine_changes(&temp_transform);
                            if let Ok(update) = config_manager.update_batch(
                                changes,
                                format!("history.action.triangle_edit_coords|name={}", transform_name)
                            ) {
                                max_update = max_update.max(update);
                            }
                        }
                    }
                    CoordTab::AffineCoeffs => {
                        let mut dragging = false;
                        let mut drag_stopped = false;

                        let make_coeff_path = |param: AffineParam| -> ConfigPath {
                            match (selected_transform, affine_target) {
                                (Some(index), AffineTarget::Pre) => ConfigPath::TransformAffine { index, param },
                                (Some(index), AffineTarget::Post) => ConfigPath::TransformPostAffine { index, param },
                                (None, AffineTarget::Pre) => ConfigPath::FinalTransformAffine { param },
                                (None, AffineTarget::Post) => ConfigPath::FinalTransformPostAffine { param },
                            }
                        };

                        let (mut val_a, mut val_b, mut val_c, mut val_d, mut val_e, mut val_f) = match affine_target {
                            AffineTarget::Pre => (transform.a, transform.b, transform.c, transform.d, transform.e, transform.f),
                            AffineTarget::Post => (transform.post_a, transform.post_b, transform.post_c, transform.post_d, transform.post_e, transform.post_f),
                        };

                        // Apophysis displays b, c, f with opposite sign
                        let mut display_b = -val_b;
                        let mut display_c = -val_c;
                        let mut display_f = -val_f;

                        ui.horizontal(|ui| {
                            let a_resp = ui.add(super::VkbDragValue::new(&mut val_a).speed(0.01).prefix("a: "))
                                .on_hover_text(t!("triangle_editor.tooltip_affine_a"));
                            if a_resp.changed() {
                                if let Ok(update) = config_manager.update_param(make_coeff_path(AffineParam::A), val_a.into()) {
                                    max_update = max_update.max(update);
                                }
                            }
                            dragging |= a_resp.dragged();
                            drag_stopped |= a_resp.drag_stopped();

                            let b_resp = ui.add(super::VkbDragValue::new(&mut display_b).speed(0.01).prefix("b: "))
                                .on_hover_text(t!("triangle_editor.tooltip_affine_b"));
                            if b_resp.changed() {
                                val_b = -display_b;
                                if let Ok(update) = config_manager.update_param(make_coeff_path(AffineParam::B), val_b.into()) {
                                    max_update = max_update.max(update);
                                }
                            }
                            dragging |= b_resp.dragged();
                            drag_stopped |= b_resp.drag_stopped();

                            let e_resp = ui.add(super::VkbDragValue::new(&mut val_e).speed(0.01).prefix("e: "))
                                .on_hover_text(t!("triangle_editor.tooltip_affine_e"));
                            if e_resp.changed() {
                                if let Ok(update) = config_manager.update_param(make_coeff_path(AffineParam::E), val_e.into()) {
                                    max_update = max_update.max(update);
                                }
                            }
                            dragging |= e_resp.dragged();
                            drag_stopped |= e_resp.drag_stopped();
                        });
                        ui.horizontal(|ui| {
                            let c_resp = ui.add(super::VkbDragValue::new(&mut display_c).speed(0.01).prefix("c: "))
                                .on_hover_text(t!("triangle_editor.tooltip_affine_c"));
                            if c_resp.changed() {
                                val_c = -display_c;
                                if let Ok(update) = config_manager.update_param(make_coeff_path(AffineParam::C), val_c.into()) {
                                    max_update = max_update.max(update);
                                }
                            }
                            dragging |= c_resp.dragged();
                            drag_stopped |= c_resp.drag_stopped();

                            let d_resp = ui.add(super::VkbDragValue::new(&mut val_d).speed(0.01).prefix("d: "))
                                .on_hover_text(t!("triangle_editor.tooltip_affine_d"));
                            if d_resp.changed() {
                                if let Ok(update) = config_manager.update_param(make_coeff_path(AffineParam::D), val_d.into()) {
                                    max_update = max_update.max(update);
                                }
                            }
                            dragging |= d_resp.dragged();
                            drag_stopped |= d_resp.drag_stopped();

                            let f_resp = ui.add(super::VkbDragValue::new(&mut display_f).speed(0.01).prefix("f: "))
                                .on_hover_text(t!("triangle_editor.tooltip_affine_f"));
                            if f_resp.changed() {
                                val_f = -display_f;
                                if let Ok(update) = config_manager.update_param(make_coeff_path(AffineParam::F), val_f.into()) {
                                    max_update = max_update.max(update);
                                }
                            }
                            dragging |= f_resp.dragged();
                            drag_stopped |= f_resp.drag_stopped();
                        });
                    }
                }

                ui.separator();

                // Reset to Identity (at the very bottom)
                if ui.button(t!("triangle_editor.reset_identity").as_ref())
                    .on_hover_text(t!("triangle_editor.tooltip_reset_identity")).clicked()
                {
                    let mut identity_transform = crate::scene::transforms::Transform::new();
                    identity_transform.a = 1.0;
                    identity_transform.d = 1.0;
                    let changes = make_affine_changes(&identity_transform);
                    if let Ok(update) = config_manager.update_batch(
                        changes,
                        format!("history.action.triangle_reset_identity|name={}", transform_name)
                    ) {
                        max_update = max_update.max(update);
                    }
                }
            }

    max_update
}

/// Render the Triangle Editor panel content (visual affine editor)
///
/// This is the panel version without the Window wrapper.
pub fn render_triangle_editor_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &mut Flame,
) -> UpdateType {
    render_triangle_editor_core(ui, config_manager, flame)
}

/// Get a distinct color for each transform index
fn get_transform_color(index: usize) -> Color32 {
    let colors = [
        Color32::from_rgb(255, 100, 100), // Red
        Color32::from_rgb(100, 255, 100), // Green
        Color32::from_rgb(100, 100, 255), // Blue
        Color32::from_rgb(255, 255, 100), // Yellow
        Color32::from_rgb(255, 100, 255), // Magenta
        Color32::from_rgb(100, 255, 255), // Cyan
        Color32::from_rgb(255, 150, 100), // Orange
        Color32::from_rgb(150, 100, 255), // Purple
    ];
    colors[index % colors.len()]
}
