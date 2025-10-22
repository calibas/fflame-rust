use crate::scene::transforms::Flame;
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

/// Render the Triangle Editor window
pub fn render_triangle_editor_window(
    ctx: &egui::Context,
    show_triangle_editor: &mut bool,
    flame: &mut Flame,
    flame_changed: &mut bool,
    triangle_drag_started: &mut bool,
    triangle_dragging: &mut bool,
    triangle_drag_ended: &mut bool,
) {
    egui::Window::new("Triangle Editor")
        .open(show_triangle_editor)
        .default_size([500.0, 600.0])
        .show(ctx, |ui| {
            ui.heading("Affine Transform Visualizer");
            ui.label("Displays the coordinate system for each transform as triangles (O, X, Y)");
            ui.separator();

            // Transform selector (persisted across frames)
            let mut selected_transform = ui.ctx().data_mut(|d| {
                d.get_persisted::<usize>(egui::Id::new("triangle_editor_selected_transform"))
                    .unwrap_or(0)
            });

            // Clamp selection to valid range
            if flame.transforms.is_empty() {
                ui.label("No transforms available");
                return;
            }
            selected_transform = selected_transform.min(flame.transforms.len() - 1);

            ui.horizontal(|ui| {
                ui.label("Transform:");
                let old_selection = selected_transform;
                egui::ComboBox::new("triangle_editor_transform_selector", "")
                    .selected_text(format!("Transform {}", selected_transform))
                    .show_ui(ui, |ui| {
                        for i in 0..flame.transforms.len() {
                            ui.selectable_value(&mut selected_transform, i, format!("Transform {}", i));
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
                MouseMode::Scale => "Click and drag up/down to scale the triangle",
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

            // Get current triangle for selected transform
            if let Some(transform) = flame.transforms.get_mut(selected_transform) {
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
                                    *triangle_drag_started = true;
                                } else if pos.distance(x_pos) < hit_radius {
                                    drag_target = DragTarget::XPoint;
                                    *triangle_drag_started = true;
                                } else if pos.distance(y_pos) < hit_radius {
                                    drag_target = DragTarget::YPoint;
                                    *triangle_drag_started = true;
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

                                transform.from_triangle(o, x, y);
                                *flame_changed = true;
                                *triangle_dragging = true;
                            }
                        }
                    }

                    MouseMode::Translate => {
                        // Start drag on any point in canvas
                        if drag_start_pos.is_none() && response.dragged() {
                            drag_start_pos = response.interact_pointer_pos();
                            *triangle_drag_started = true;
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

                                transform.from_triangle(o, x, y);
                                *flame_changed = true;
                                *triangle_dragging = true;

                                drag_start_pos = Some(current_pos);
                            }
                        }
                    }

                    MouseMode::Rotate => {
                        // Capture initial mouse position
                        if drag_start_pos.is_none() && response.dragged() {
                            drag_start_pos = response.interact_pointer_pos();
                            *triangle_drag_started = true;
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

                                transform.from_triangle(o, x, y);
                                *flame_changed = true;
                                *triangle_dragging = true;

                                drag_start_pos = Some(current_pos);
                            }
                        }
                    }

                    MouseMode::Scale => {
                        // Capture initial mouse position
                        if drag_start_pos.is_none() && response.dragged() {
                            drag_start_pos = response.interact_pointer_pos();
                            *triangle_drag_started = true;
                        }

                        // Scale X and Y from O
                        if let (Some(start_pos), Some(current_pos)) = (drag_start_pos, response.interact_pointer_pos()) {
                            if response.dragged() {
                                let delta_y = (start_pos.y - current_pos.y) * 0.005; // Sensitivity (up = bigger)
                                let scale_factor = 1.0 + delta_y;

                                // Scale X and Y vectors from O
                                let x_vec = [x[0] - o[0], x[1] - o[1]];
                                let y_vec = [y[0] - o[0], y[1] - o[1]];

                                x = [o[0] + x_vec[0] * scale_factor, o[1] + x_vec[1] * scale_factor];
                                y = [o[0] + y_vec[0] * scale_factor, o[1] + y_vec[1] * scale_factor];

                                transform.from_triangle(o, x, y);
                                *flame_changed = true;
                                *triangle_dragging = true;

                                drag_start_pos = Some(current_pos);
                            }
                        }
                    }
                }

                // Clear drag on release
                if !response.dragged() {
                    // Detect transition from dragging to not dragging
                    if was_dragging {
                        *triangle_drag_ended = true;
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
                let alpha = if i == selected_transform { 255 } else { 80 };
                let color = Color32::from_rgba_unmultiplied(
                    base_color.r(),
                    base_color.g(),
                    base_color.b(),
                    alpha,
                );

                // Draw lines O→X and O→Y
                painter.line_segment([o_pos, x_pos], Stroke::new(2.0, color));
                painter.line_segment([o_pos, y_pos], Stroke::new(2.0, color));

                // Draw points with highlighting for active drag target
                let point_radius = if i == selected_transform { 6.0 } else { 4.0 };

                // Highlight the point being dragged
                if i == selected_transform {
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
                if i == selected_transform {
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

            ui.separator();

            // Display coordinates for selected transform
            if let Some(transform) = flame.transforms.get(selected_transform) {
                let (o, x, y) = transform.to_triangle();

                ui.label("Triangle Coordinates:");
                ui.horizontal(|ui| {
                    ui.monospace(format!("O: ({:.3}, {:.3})", o[0], o[1]));
                });
                ui.horizontal(|ui| {
                    ui.monospace(format!("X: ({:.3}, {:.3})", x[0], x[1]));
                });
                ui.horizontal(|ui| {
                    ui.monospace(format!("Y: ({:.3}, {:.3})", y[0], y[1]));
                });

                ui.separator();

                ui.label("Affine Coefficients:");
                ui.horizontal(|ui| {
                    ui.monospace(format!("a: {:.3}  b: {:.3}  e: {:.3}", transform.a, transform.b, transform.e));
                });
                ui.horizontal(|ui| {
                    ui.monospace(format!("c: {:.3}  d: {:.3}  f: {:.3}", transform.c, transform.d, transform.f));
                });

                ui.separator();

                // Control buttons
                if let Some(transform) = flame.transforms.get_mut(selected_transform) {
                    if ui.button("Reset to Identity").clicked() {
                        transform.reset_to_identity();
                        *flame_changed = true;
                    }
                }
            }
        });
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
