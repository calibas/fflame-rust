use crate::scene::transforms::{Flame, RenderMode};
use crate::config::{ConfigManager, ConfigPath, UpdateType};

/// Render the View window with navigation controls
#[allow(clippy::too_many_arguments)]
pub fn render_view_window(
    ctx: &egui::Context,
    show_view: &mut bool,
    config_manager: &mut ConfigManager,
    zoom: &mut f32,
    pan_x: &mut f32,
    pan_y: &mut f32,
    rotation: &mut f32,
    camera_rotation_x: &mut f32,
    camera_rotation_y: &mut f32,
    flame: &Flame,
    view_changed: &mut bool,
    camera_rotation_changed: &mut bool,
) -> UpdateType {
    let mut max_update = UpdateType::None;
    egui::Window::new("View")
        .open(show_view)
        .show(ctx, |ui| {
            use crate::config::slider::LazyUndoUi;
            use crate::config::ConfigValue;

            ui.label("Zoom");
            ui.horizontal(|ui| {
                if ui.button("➕ Zoom In").clicked() {
                    let new_zoom = *zoom * 1.5;
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::Zoom,
                        new_zoom.into(),
                        false  // Immediate capture for button click
                    ) {
                        *zoom = new_zoom;
                        *view_changed = true;
                        max_update = max_update.max(update_type);
                    }
                }
                if ui.button("➖ Zoom Out").clicked() {
                    let new_zoom = *zoom / 1.5;
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::Zoom,
                        new_zoom.into(),
                        false  // Immediate capture for button click
                    ) {
                        *zoom = new_zoom;
                        *view_changed = true;
                        max_update = max_update.max(update_type);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Value:");
                if let Ok(result) = ui.lazy_drag(config_manager, ConfigPath::Zoom, 0.01, "") {
                    if result.changed {
                        *zoom = config_manager.active_config().zoom;
                        *view_changed = result.should_capture;
                    }
                    max_update = max_update.max(result.update_type);
                }
            });

            ui.separator();

            ui.label("Pan");
            ui.horizontal(|ui| {
                ui.label("X:");
                if let Ok(result) = ui.lazy_drag(config_manager, ConfigPath::PanX, 0.01, "") {
                    if result.changed {
                        *pan_x = config_manager.active_config().pan_x;
                        *view_changed = result.should_capture;
                    }
                    max_update = max_update.max(result.update_type);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Y:");
                if let Ok(result) = ui.lazy_drag(config_manager, ConfigPath::PanY, 0.01, "") {
                    if result.changed {
                        *pan_y = config_manager.active_config().pan_y;
                        *view_changed = result.should_capture;
                    }
                    max_update = max_update.max(result.update_type);
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
                    let new_pan_x = *pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = *pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Up".to_string(),
                        false
                    ) {
                        *pan_x = new_pan_x;
                        *pan_y = new_pan_y;
                        *view_changed = true;
                        max_update = max_update.max(update_type);
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("  <  ").clicked() {
                    // Left in screen space: (-1, 0), rotate to fractal space
                    let screen_dx = -pan_step;
                    let screen_dy = 0.0;
                    let new_pan_x = *pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = *pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Left".to_string(),
                        false
                    ) {
                        *pan_x = new_pan_x;
                        *pan_y = new_pan_y;
                        *view_changed = true;
                        max_update = max_update.max(update_type);
                    }
                }
                if ui.button("  v  ").clicked() {
                    // Down in screen space: (0, 1), rotate to fractal space
                    let screen_dx = 0.0;
                    let screen_dy = pan_step;
                    let new_pan_x = *pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = *pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Down".to_string(),
                        false
                    ) {
                        *pan_x = new_pan_x;
                        *pan_y = new_pan_y;
                        *view_changed = true;
                        max_update = max_update.max(update_type);
                    }
                }
                if ui.button("  >  ").clicked() {
                    // Right in screen space: (1, 0), rotate to fractal space
                    let screen_dx = pan_step;
                    let screen_dy = 0.0;
                    let new_pan_x = *pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = *pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Right".to_string(),
                        false
                    ) {
                        *pan_x = new_pan_x;
                        *pan_y = new_pan_y;
                        *view_changed = true;
                        max_update = max_update.max(update_type);
                    }
                }
            });

            ui.separator();

            ui.label("Rotation");
            ui.horizontal(|ui| {
                // Convert radians to degrees for display
                let mut degrees = rotation.to_degrees();
                if ui.add(egui::Slider::new(&mut degrees, -180.0..=180.0).suffix("°")).changed() {
                    let new_rotation = degrees.to_radians();
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::Rotation,
                        new_rotation.into(),
                        true  // Lazy undo for slider drag
                    ) {
                        *rotation = config_manager.active_config().rotation;
                        *view_changed = true;
                        max_update = max_update.max(update_type);
                    }
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
                        let new_camera_x = degrees_x.to_radians();
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::CameraRotationX,
                            new_camera_x.into(),
                            true  // Lazy undo for slider drag
                        ) {
                            *camera_rotation_x = config_manager.active_config().camera_rotation_x;
                            *camera_rotation_changed = true;
                            max_update = max_update.max(update_type);
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Yaw (Y):");
                    let mut degrees_y = camera_rotation_y.to_degrees();
                    if ui.add(egui::Slider::new(&mut degrees_y, -180.0..=180.0).suffix("°")).changed() {
                        let new_camera_y = degrees_y.to_radians();
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::CameraRotationY,
                            new_camera_y.into(),
                            true  // Lazy undo for slider drag
                        ) {
                            *camera_rotation_y = config_manager.active_config().camera_rotation_y;
                            *camera_rotation_changed = true;
                            max_update = max_update.max(update_type);
                        }
                    }
                });
            }

            ui.separator();

            if ui.button("🔄 Reset View").clicked() {
                if let Ok(update_type) = config_manager.update_batch(
                    vec![
                        (ConfigPath::Zoom, 1.0.into()),
                        (ConfigPath::PanX, 0.0.into()),
                        (ConfigPath::PanY, 0.0.into()),
                        (ConfigPath::Rotation, 0.0.into()),
                        (ConfigPath::CameraRotationX, 0.0.into()),
                        (ConfigPath::CameraRotationY, 0.0.into()),
                    ],
                    "Reset View".to_string(),
                    false
                ) {
                    *zoom = 1.0;
                    *pan_x = 0.0;
                    *pan_y = 0.0;
                    *rotation = 0.0;
                    *camera_rotation_x = 0.0;
                    *camera_rotation_y = 0.0;
                    *view_changed = true;
                    *camera_rotation_changed = true;
                    max_update = max_update.max(update_type);
                }
            }
        });

    max_update
}
