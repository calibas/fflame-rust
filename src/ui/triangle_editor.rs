use crate::scene::transforms::Flame;
use crate::config::{ConfigManager, ConfigPath, AffineParam, UpdateType};
use egui::{Color32, Pos2, Stroke, Vec2};
use serde::{Deserialize, Serialize};

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
        MouseMode::MovePoints
    }
}

/// Core triangle editor rendering (shared by window and panel)
fn render_triangle_editor_core(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &mut Flame,
) -> UpdateType {
    let mut max_update = UpdateType::None;
            ui.heading("Affine Transform Visualizer");
            ui.label("Displays the coordinate system for each transform as triangles (O, X, Y)");
            ui.separator();

            // Transform selector (persisted across frames)
            // Option<usize>: Some(i) for regular transform, None for final transform
            let mut selected_transform = ui.ctx().data_mut(|d| {
                d.get_persisted::<Option<usize>>(egui::Id::new("triangle_editor_selected_transform"))
                    .unwrap_or(Some(0))
            });

            // Clamp selection to valid range
            if flame.transforms.is_empty() {
                ui.label("No transforms available");
                return max_update;
            }
            if let Some(idx) = selected_transform {
                if idx >= flame.transforms.len() {
                    selected_transform = Some(flame.transforms.len() - 1);
                }
            }

            ui.horizontal(|ui| {
                ui.label("Transform:");
                let old_selection = selected_transform;
                let display_text = match selected_transform {
                    Some(i) => format!("Transform {}", i + 1),
                    None => "Transform [Final]".to_string(),
                };
                egui::ComboBox::new("triangle_editor_transform_selector", "")
                    .selected_text(display_text)
                    .show_ui(ui, |ui| {
                        // Regular transforms
                        for i in 0..flame.transforms.len() {
                            ui.selectable_value(&mut selected_transform, Some(i), format!("Transform {}", i + 1));
                        }
                        // Final transform (if it exists)
                        if flame.final_transform.is_some() {
                            ui.separator();
                            ui.selectable_value(&mut selected_transform, None, "Transform [Final]");
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

            ui.horizontal(|ui| {
                ui.label("Mode:");
                let old_mode = mouse_mode;

                ui.selectable_value(&mut mouse_mode, MouseMode::MovePoints, "🎯 Move Points");
                ui.selectable_value(&mut mouse_mode, MouseMode::Translate, "↔ Translate");
                ui.selectable_value(&mut mouse_mode, MouseMode::Rotate, "↻ Rotate");
                ui.selectable_value(&mut mouse_mode, MouseMode::Scale, "⇄ Scale");

                // Persist mode if changed
                if mouse_mode != old_mode {
                    ui.ctx().data_mut(|d| {
                        d.insert_persisted(egui::Id::new("triangle_editor_mouse_mode"), mouse_mode);
                    });
                }
            });

            // Mode description
            let mode_desc = match mouse_mode {
                MouseMode::MovePoints => "Click and drag O, X, or Y points individually",
                MouseMode::Translate => "Click and drag anywhere to move the entire triangle",
                MouseMode::Rotate => "Click and drag to rotate the triangle around O",
                MouseMode::Scale => "Click and drag along perpendicular axis to X-Y line to scale",
            };
            ui.label(egui::RichText::new(mode_desc).italics().small());

            ui.separator();

            // Canvas for drawing triangles
            let canvas_size = Vec2::new(400.0, 400.0);
            let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::drag());
            let rect = response.rect;

            // Define coordinate mapping: fractal space [-2, 2] → canvas pixels
            let world_min = -2.0f32;
            let world_max = 2.0f32;
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

            // Draw grid
            let grid_spacing = 0.5; // Grid every 0.5 units
            let grid_color = Color32::from_rgb(40, 40, 40);
            let axis_color = Color32::from_rgb(60, 60, 60);

            for i in 0..=((world_size / grid_spacing) as i32) {
                let world_coord = world_min + (i as f32) * grid_spacing;

                // Vertical lines
                let p1 = to_canvas([world_coord, world_min]);
                let p2 = to_canvas([world_coord, world_max]);
                let color = if world_coord.abs() < 0.01 { axis_color } else { grid_color };
                painter.line_segment([p1, p2], Stroke::new(1.0, color));

                // Horizontal lines
                let p1 = to_canvas([world_min, world_coord]);
                let p2 = to_canvas([world_max, world_coord]);
                let color = if world_coord.abs() < 0.01 { axis_color } else { grid_color };
                painter.line_segment([p1, p2], Stroke::new(1.0, color));
            }

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

            // Track if we were dragging in the previous frame
            let was_dragging = drag_target != DragTarget::None || drag_start_pos.is_some();

            // Helper to create affine changes for either regular or final transform
            let make_affine_changes = |xform: &crate::scene::transforms::Transform| -> Vec<(ConfigPath, crate::config::ConfigValue)> {
                match selected_transform {
                    Some(index) => vec![
                        (ConfigPath::TransformAffine { index, param: AffineParam::A }, xform.a.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::B }, xform.b.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::C }, xform.c.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::D }, xform.d.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::E }, xform.e.into()),
                        (ConfigPath::TransformAffine { index, param: AffineParam::F }, xform.f.into()),
                    ],
                    None => vec![
                        (ConfigPath::FinalTransformAffine { param: AffineParam::A }, xform.a.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::B }, xform.b.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::C }, xform.c.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::D }, xform.d.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::E }, xform.e.into()),
                        (ConfigPath::FinalTransformAffine { param: AffineParam::F }, xform.f.into()),
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
                Some(i) => format!("Transform {}", i + 1),
                None => "Transform [Final]".to_string(),
            };

            // Get current triangle for selected transform (regular or final)
            let transform_mut = match selected_transform {
                Some(idx) => flame.transforms.get_mut(idx),
                None => flame.final_transform.as_mut(),
            };

            if let Some(transform) = transform_mut {
                let (mut o, mut x, mut y) = transform.to_triangle();

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
                                transform.from_triangle(o, x, y);
                                let changes = make_affine_changes(transform);
                                if let Ok(update_type) = config_manager.update_batch(changes, "Triangle Edit (Move Points)".to_string(), true) {
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
                                transform.from_triangle(o, x, y);
                                let changes = make_affine_changes(transform);
                                if let Ok(update_type) = config_manager.update_batch(changes, "Triangle Edit (Translate)".to_string(), true) {
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
                                transform.from_triangle(o, x, y);
                                let changes = make_affine_changes(transform);
                                if let Ok(update_type) = config_manager.update_batch(changes, "Triangle Edit (Rotate)".to_string(), true) {
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
                                        transform.from_triangle(o, x, y);
                                        let changes = make_affine_changes(transform);
                                        if let Ok(update_type) = config_manager.update_batch(changes, "Triangle Edit (Scale)".to_string(), true) {
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
                if !response.dragged() {
                    // Drag ended - force commit any pending preview changes
                    if was_dragging && config_manager.is_in_preview_mode() {
                        // Force commit with any affine parameter (they all return IterationReset)
                        let commit_path = match selected_transform {
                            Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::A },
                            None => ConfigPath::FinalTransformAffine { param: AffineParam::A },
                        };
                        if let Ok(_) = config_manager.force_commit_preview(&commit_path) {
                            log::debug!("Triangle Editor: Force-committed preview on drag end");
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
                let (o, x, y) = transform.to_triangle();

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

            // Draw final transform if present (light grey, distinct style)
            if let Some(final_xform) = &flame.final_transform {
                let (o, x, y) = final_xform.to_triangle();

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
                let (mut o, mut x, mut y) = transform.to_triangle();

                ui.label(format!("Triangle Coordinates ({}):", transform_name));

                let mut coords_changed = false;
                let mut dragging = false;
                let mut drag_stopped = false;

                ui.columns(2, |columns| {
                    // Left column: Coordinates
                    columns[0].vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("X:");
                            let x0_resp = ui.add(egui::DragValue::new(&mut x[0]).speed(0.01).prefix("x: "));
                            let x1_resp = ui.add(egui::DragValue::new(&mut x[1]).speed(0.01).prefix("y: "));
                            coords_changed |= x0_resp.changed() || x1_resp.changed();
                            dragging |= x0_resp.dragged() || x1_resp.dragged();
                            drag_stopped |= x0_resp.drag_stopped() || x1_resp.drag_stopped();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Y:");
                            let y0_resp = ui.add(egui::DragValue::new(&mut y[0]).speed(0.01).prefix("x: "));
                            let y1_resp = ui.add(egui::DragValue::new(&mut y[1]).speed(0.01).prefix("y: "));
                            coords_changed |= y0_resp.changed() || y1_resp.changed();
                            dragging |= y0_resp.dragged() || y1_resp.dragged();
                            drag_stopped |= y0_resp.drag_stopped() || y1_resp.drag_stopped();
                        });
                        ui.horizontal(|ui| {
                            ui.label("O:");
                            let o0_resp = ui.add(egui::DragValue::new(&mut o[0]).speed(0.01).prefix("x: "));
                            let o1_resp = ui.add(egui::DragValue::new(&mut o[1]).speed(0.01).prefix("y: "));
                            coords_changed |= o0_resp.changed() || o1_resp.changed();
                            dragging |= o0_resp.dragged() || o1_resp.dragged();
                            drag_stopped |= o0_resp.drag_stopped() || o1_resp.drag_stopped();
                        });
                    });

                    // Right column: Quick action buttons
                    columns[1].vertical(|ui| {
                        ui.label("Quick Actions:");

                        // Helper closure to apply triangle changes via ConfigManager
                        let mut apply_triangle_change = |transform_ref: &crate::scene::transforms::Transform,
                                                           o: [f32; 2], x: [f32; 2], y: [f32; 2],
                                                           description: &str| {
                            let mut temp = transform_ref.clone();
                            temp.from_triangle(o, x, y);
                            let changes = match selected_transform {
                                Some(index) => vec![
                                    (ConfigPath::TransformAffine { index, param: AffineParam::A }, temp.a.into()),
                                    (ConfigPath::TransformAffine { index, param: AffineParam::B }, temp.b.into()),
                                    (ConfigPath::TransformAffine { index, param: AffineParam::C }, temp.c.into()),
                                    (ConfigPath::TransformAffine { index, param: AffineParam::D }, temp.d.into()),
                                    (ConfigPath::TransformAffine { index, param: AffineParam::E }, temp.e.into()),
                                    (ConfigPath::TransformAffine { index, param: AffineParam::F }, temp.f.into()),
                                ],
                                None => vec![
                                    (ConfigPath::FinalTransformAffine { param: AffineParam::A }, temp.a.into()),
                                    (ConfigPath::FinalTransformAffine { param: AffineParam::B }, temp.b.into()),
                                    (ConfigPath::FinalTransformAffine { param: AffineParam::C }, temp.c.into()),
                                    (ConfigPath::FinalTransformAffine { param: AffineParam::D }, temp.d.into()),
                                    (ConfigPath::FinalTransformAffine { param: AffineParam::E }, temp.e.into()),
                                    (ConfigPath::FinalTransformAffine { param: AffineParam::F }, temp.f.into()),
                                ],
                            };
                            config_manager.update_batch(changes, description.to_string(), false)
                        };

                        // Translate arrow keys layout (matching View panel)
                        ui.horizontal(|ui| {
                            ui.add_space(30.0);
                            if ui.button("  ^  ").clicked() {
                                let (mut o_new, mut x_new, mut y_new) = transform.to_triangle();
                                o_new[1] += 0.1;
                                x_new[1] += 0.1;
                                y_new[1] += 0.1;
                                if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                    &format!("Translate up ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("  <  ").clicked() {
                                let (mut o_new, mut x_new, mut y_new) = transform.to_triangle();
                                o_new[0] -= 0.1;
                                x_new[0] -= 0.1;
                                y_new[0] -= 0.1;
                                if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                    &format!("Translate left ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                            if ui.button("  v  ").clicked() {
                                let (mut o_new, mut x_new, mut y_new) = transform.to_triangle();
                                o_new[1] -= 0.1;
                                x_new[1] -= 0.1;
                                y_new[1] -= 0.1;
                                if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                    &format!("Translate down ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                            if ui.button("  >  ").clicked() {
                                let (mut o_new, mut x_new, mut y_new) = transform.to_triangle();
                                o_new[0] += 0.1;
                                x_new[0] += 0.1;
                                y_new[0] += 0.1;
                                if let Ok(update) = apply_triangle_change(transform, o_new, x_new, y_new,
                                    &format!("Translate right ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                        });

                        ui.add_space(5.0);

                        // Rotate buttons
                        ui.horizontal(|ui| {
                            if ui.button("↻ Rotate CW").clicked() {
                                let angle = -15.0_f32.to_radians();
                                let cos_a = angle.cos();
                                let sin_a = angle.sin();
                                let (o_curr, x_curr, y_curr) = transform.to_triangle();

                                let x_vec = [x_curr[0] - o_curr[0], x_curr[1] - o_curr[1]];
                                let y_vec = [y_curr[0] - o_curr[0], y_curr[1] - o_curr[1]];

                                let x_rot = [x_vec[0] * cos_a - x_vec[1] * sin_a, x_vec[0] * sin_a + x_vec[1] * cos_a];
                                let y_rot = [y_vec[0] * cos_a - y_vec[1] * sin_a, y_vec[0] * sin_a + y_vec[1] * cos_a];

                                let x_new = [o_curr[0] + x_rot[0], o_curr[1] + x_rot[1]];
                                let y_new = [o_curr[0] + y_rot[0], o_curr[1] + y_rot[1]];

                                if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                    &format!("Rotate CW ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                            if ui.button("↺ Rotate CCW").clicked() {
                                let angle = 15.0_f32.to_radians();
                                let cos_a = angle.cos();
                                let sin_a = angle.sin();
                                let (o_curr, x_curr, y_curr) = transform.to_triangle();

                                let x_vec = [x_curr[0] - o_curr[0], x_curr[1] - o_curr[1]];
                                let y_vec = [y_curr[0] - o_curr[0], y_curr[1] - o_curr[1]];

                                let x_rot = [x_vec[0] * cos_a - x_vec[1] * sin_a, x_vec[0] * sin_a + x_vec[1] * cos_a];
                                let y_rot = [y_vec[0] * cos_a - y_vec[1] * sin_a, y_vec[0] * sin_a + y_vec[1] * cos_a];

                                let x_new = [o_curr[0] + x_rot[0], o_curr[1] + x_rot[1]];
                                let y_new = [o_curr[0] + y_rot[0], o_curr[1] + y_rot[1]];

                                if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                    &format!("Rotate CCW ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                        });

                        // Scale buttons
                        ui.horizontal(|ui| {
                            if ui.button("⇄ Scale Up").clicked() {
                                let (o_curr, x_curr, y_curr) = transform.to_triangle();
                                let x_vec = [x_curr[0] - o_curr[0], x_curr[1] - o_curr[1]];
                                let y_vec = [y_curr[0] - o_curr[0], y_curr[1] - o_curr[1]];

                                let x_new = [o_curr[0] + x_vec[0] * 1.1, o_curr[1] + x_vec[1] * 1.1];
                                let y_new = [o_curr[0] + y_vec[0] * 1.1, o_curr[1] + y_vec[1] * 1.1];

                                if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                    &format!("Scale up ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                            if ui.button("⇄ Scale Down").clicked() {
                                let (o_curr, x_curr, y_curr) = transform.to_triangle();
                                let x_vec = [x_curr[0] - o_curr[0], x_curr[1] - o_curr[1]];
                                let y_vec = [y_curr[0] - o_curr[0], y_curr[1] - o_curr[1]];

                                let x_new = [o_curr[0] + x_vec[0] * 0.9, o_curr[1] + x_vec[1] * 0.9];
                                let y_new = [o_curr[0] + y_vec[0] * 0.9, o_curr[1] + y_vec[1] * 0.9];

                                if let Ok(update) = apply_triangle_change(transform, o_curr, x_new, y_new,
                                    &format!("Scale down ({})", transform_name)) {
                                    max_update = max_update.max(update);
                                }
                            }
                        });
                    });
                });

                if coords_changed {
                    // Convert triangle coords to affine parameters
                    let mut temp_transform = transform.clone();
                    temp_transform.from_triangle(o, x, y);

                    // Batch update all affine parameters via ConfigManager
                    let changes = make_affine_changes(&temp_transform);

                    // Use lazy=true while dragging
                    if let Ok(update) = config_manager.update_batch(
                        changes,
                        format!("Edit triangle coordinates ({})", transform_name),
                        dragging && !drag_stopped // lazy while dragging
                    ) {
                        max_update = max_update.max(update);
                    }
                }

                // Force commit preview when drag stops
                if drag_stopped && config_manager.is_in_preview_mode() {
                    // Use first affine parameter as representative for batch
                    let commit_path = match selected_transform {
                        Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::A },
                        None => ConfigPath::FinalTransformAffine { param: AffineParam::A },
                    };
                    if let Ok(update) = config_manager.force_commit_preview(&commit_path) {
                        max_update = max_update.max(update);
                    }
                    config_manager.reset_lazy_undo();
                }

                ui.separator();

                ui.label("Affine Coefficients:");

                // Track drag state for preview mode
                let mut dragging = false;
                let mut drag_stopped = false;

                ui.horizontal(|ui| {
                    let a_resp = ui.add(egui::DragValue::new(&mut transform.a).speed(0.01).prefix("a: "));
                    if a_resp.changed() {
                        let path = match selected_transform {
                            Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::A },
                            None => ConfigPath::FinalTransformAffine { param: AffineParam::A },
                        };
                        if let Ok(update) = config_manager.update_param(
                            path,
                            transform.a.into(),
                            a_resp.dragged()
                        ) {
                            max_update = max_update.max(update);
                        }
                    }
                    dragging |= a_resp.dragged();
                    drag_stopped |= a_resp.drag_stopped();

                    let b_resp = ui.add(egui::DragValue::new(&mut transform.b).speed(0.01).prefix("b: "));
                    if b_resp.changed() {
                        let path = match selected_transform {
                            Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::B },
                            None => ConfigPath::FinalTransformAffine { param: AffineParam::B },
                        };
                        if let Ok(update) = config_manager.update_param(
                            path,
                            transform.b.into(),
                            b_resp.dragged()
                        ) {
                            max_update = max_update.max(update);
                        }
                    }
                    dragging |= b_resp.dragged();
                    drag_stopped |= b_resp.drag_stopped();

                    let e_resp = ui.add(egui::DragValue::new(&mut transform.e).speed(0.01).prefix("e: "));
                    if e_resp.changed() {
                        let path = match selected_transform {
                            Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::E },
                            None => ConfigPath::FinalTransformAffine { param: AffineParam::E },
                        };
                        if let Ok(update) = config_manager.update_param(
                            path,
                            transform.e.into(),
                            e_resp.dragged()
                        ) {
                            max_update = max_update.max(update);
                        }
                    }
                    dragging |= e_resp.dragged();
                    drag_stopped |= e_resp.drag_stopped();
                });
                ui.horizontal(|ui| {
                    let c_resp = ui.add(egui::DragValue::new(&mut transform.c).speed(0.01).prefix("c: "));
                    if c_resp.changed() {
                        let path = match selected_transform {
                            Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::C },
                            None => ConfigPath::FinalTransformAffine { param: AffineParam::C },
                        };
                        if let Ok(update) = config_manager.update_param(
                            path,
                            transform.c.into(),
                            c_resp.dragged()
                        ) {
                            max_update = max_update.max(update);
                        }
                    }
                    dragging |= c_resp.dragged();
                    drag_stopped |= c_resp.drag_stopped();

                    let d_resp = ui.add(egui::DragValue::new(&mut transform.d).speed(0.01).prefix("d: "));
                    if d_resp.changed() {
                        let path = match selected_transform {
                            Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::D },
                            None => ConfigPath::FinalTransformAffine { param: AffineParam::D },
                        };
                        if let Ok(update) = config_manager.update_param(
                            path,
                            transform.d.into(),
                            d_resp.dragged()
                        ) {
                            max_update = max_update.max(update);
                        }
                    }
                    dragging |= d_resp.dragged();
                    drag_stopped |= d_resp.drag_stopped();

                    let f_resp = ui.add(egui::DragValue::new(&mut transform.f).speed(0.01).prefix("f: "));
                    if f_resp.changed() {
                        let path = match selected_transform {
                            Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::F },
                            None => ConfigPath::FinalTransformAffine { param: AffineParam::F },
                        };
                        if let Ok(update) = config_manager.update_param(
                            path,
                            transform.f.into(),
                            f_resp.dragged()
                        ) {
                            max_update = max_update.max(update);
                        }
                    }
                    dragging |= f_resp.dragged();
                    drag_stopped |= f_resp.drag_stopped();
                });

                // Force commit preview when drag stops
                if drag_stopped && config_manager.is_in_preview_mode() {
                    let commit_path = match selected_transform {
                        Some(index) => ConfigPath::TransformAffine { index, param: AffineParam::A },
                        None => ConfigPath::FinalTransformAffine { param: AffineParam::A },
                    };
                    if let Ok(update) = config_manager.force_commit_preview(&commit_path) {
                        max_update = max_update.max(update);
                    }
                    config_manager.reset_lazy_undo();
                }

                ui.separator();

                // Control buttons
                {
                    if ui.button("Reset to Identity").clicked() {
                        let mut identity_transform = crate::scene::transforms::Transform::new();
                        identity_transform.a = 1.0;
                        identity_transform.d = 1.0;
                        let changes = make_affine_changes(&identity_transform);
                        if let Ok(update) = config_manager.update_batch(
                            changes,
                            format!("Reset to identity ({})", transform_name),
                            false
                        ) {
                            max_update = max_update.max(update);
                        }
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
