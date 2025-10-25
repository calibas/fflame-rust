use crate::scene::transforms::{Flame, RenderMode};

/// Render the View window with navigation controls
#[allow(clippy::too_many_arguments)]
pub fn render_view_window(
    ctx: &egui::Context,
    show_view: &mut bool,
    zoom: &mut f32,
    pan_x: &mut f32,
    pan_y: &mut f32,
    rotation: &mut f32,
    camera_rotation_x: &mut f32,
    camera_rotation_y: &mut f32,
    flame: &Flame,
    view_changed: &mut bool,
    camera_rotation_changed: &mut bool,
) {
    egui::Window::new("View")
        .open(show_view)
        .show(ctx, |ui| {
            ui.label("Zoom");
            ui.horizontal(|ui| {
                if ui.button("➕ Zoom In").clicked() {
                    *zoom *= 1.5;
                    *view_changed = true;
                }
                if ui.button("➖ Zoom Out").clicked() {
                    *zoom /= 1.5;
                    *view_changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Value:");
                if ui.add(egui::DragValue::new(zoom).speed(0.01).range(0.01..=10000.0)).changed() {
                    *view_changed = true;
                }
            });

            ui.separator();

            ui.label("Pan");
            ui.horizontal(|ui| {
                ui.label("X:");
                if ui.add(egui::DragValue::new(pan_x).speed(0.01)).changed() {
                    *view_changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Y:");
                if ui.add(egui::DragValue::new(pan_y).speed(0.01)).changed() {
                    *view_changed = true;
                }
            });

            // Pan step size depends on zoom (more zoomed in = smaller steps)
            let pan_step = 0.1 / *zoom;

            ui.separator();
            ui.label("Arrow Controls");

            // Pre-calculate rotation for arrow controls
            // Negate rotation to convert screen space to fractal space
            let cos_r = (-*rotation).cos();
            let sin_r = (-*rotation).sin();

            // Arrow keys layout
            ui.horizontal(|ui| {
                ui.add_space(30.0);
                if ui.button("  ^  ").clicked() {
                    // Up in screen space: (0, -1), rotate to fractal space
                    let screen_dx = 0.0;
                    let screen_dy = -pan_step;
                    *pan_x += screen_dx * cos_r - screen_dy * sin_r;
                    *pan_y += screen_dx * sin_r + screen_dy * cos_r;
                    *view_changed = true;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("  <  ").clicked() {
                    // Left in screen space: (-1, 0), rotate to fractal space
                    let screen_dx = -pan_step;
                    let screen_dy = 0.0;
                    *pan_x += screen_dx * cos_r - screen_dy * sin_r;
                    *pan_y += screen_dx * sin_r + screen_dy * cos_r;
                    *view_changed = true;
                }
                if ui.button("  v  ").clicked() {
                    // Down in screen space: (0, 1), rotate to fractal space
                    let screen_dx = 0.0;
                    let screen_dy = pan_step;
                    *pan_x += screen_dx * cos_r - screen_dy * sin_r;
                    *pan_y += screen_dx * sin_r + screen_dy * cos_r;
                    *view_changed = true;
                }
                if ui.button("  >  ").clicked() {
                    // Right in screen space: (1, 0), rotate to fractal space
                    let screen_dx = pan_step;
                    let screen_dy = 0.0;
                    *pan_x += screen_dx * cos_r - screen_dy * sin_r;
                    *pan_y += screen_dx * sin_r + screen_dy * cos_r;
                    *view_changed = true;
                }
            });

            ui.separator();

            ui.label("Rotation");
            ui.horizontal(|ui| {
                // Convert radians to degrees for display
                let mut degrees = rotation.to_degrees();
                if ui.add(egui::Slider::new(&mut degrees, -180.0..=180.0).suffix("°")).changed() {
                    *rotation = degrees.to_radians();
                    *view_changed = true;
                }
            });

            // 3D Camera rotation controls (only visible in 3D mode)
            if matches!(flame.render_mode, RenderMode::ThreeD) {
                ui.separator();
                ui.label("3D Camera Rotation");

                ui.horizontal(|ui| {
                    ui.label("Pitch (X):");
                    let mut degrees_x = camera_rotation_x.to_degrees();
                    if ui.add(egui::Slider::new(&mut degrees_x, -180.0..=180.0).suffix("°")).changed() {
                        *camera_rotation_x = degrees_x.to_radians();
                        *camera_rotation_changed = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Yaw (Y):");
                    let mut degrees_y = camera_rotation_y.to_degrees();
                    if ui.add(egui::Slider::new(&mut degrees_y, -180.0..=180.0).suffix("°")).changed() {
                        *camera_rotation_y = degrees_y.to_radians();
                        *camera_rotation_changed = true;
                    }
                });
            }

            ui.separator();

            if ui.button("🔄 Reset View").clicked() {
                *zoom = 1.0;
                *pan_x = 0.0;
                *pan_y = 0.0;
                *rotation = 0.0;
                *camera_rotation_x = 0.0;
                *camera_rotation_y = 0.0;
                *view_changed = true;
                *camera_rotation_changed = true;
            }
        });
}
