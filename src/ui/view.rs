use crate::scene::transforms::{Flame, RenderMode};
use crate::config::{ConfigManager, ConfigPath, UpdateType};

/// Render the View window with navigation controls
///
/// Note: Config change tracking is now handled by ConfigManager.get_pending_actions()
pub fn render_view_window(
    ctx: &egui::Context,
    show_view: &mut bool,
    config_manager: &mut ConfigManager,
    flame: &Flame,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    egui::Window::new("View")
        .open(show_view)
        .show(ctx, |ui| {
            use crate::config::slider::LazyUndoUi;

            // Clone config to avoid borrow conflicts (allows mutation of config_manager in closures)
            let config = config_manager.active_config().clone();

            ui.label("Zoom");
            ui.horizontal(|ui| {
                if ui.button("➕ Zoom In").clicked() {
                    let new_zoom = config.zoom * 1.5;
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::Zoom,
                        new_zoom.into(),
                        false  // Immediate capture for button click
                    ) {
                        max_update = max_update.max(update_type);
                    }
                }
                if ui.button("➖ Zoom Out").clicked() {
                    let new_zoom = config.zoom / 1.5;
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::Zoom,
                        new_zoom.into(),
                        false  // Immediate capture for button click
                    ) {
                        max_update = max_update.max(update_type);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Value:");
                if let Ok(result) = ui.lazy_drag(config_manager, ConfigPath::Zoom, 0.01, "") {
                    max_update = max_update.max(result.update_type);
                }
            });

            ui.separator();

            ui.label("Pan");
            ui.horizontal(|ui| {
                ui.label("X:");
                if let Ok(result) = ui.lazy_drag(config_manager, ConfigPath::PanX, 0.01, "") {
                    max_update = max_update.max(result.update_type);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Y:");
                if let Ok(result) = ui.lazy_drag(config_manager, ConfigPath::PanY, 0.01, "") {
                    max_update = max_update.max(result.update_type);
                }
            });

            // Pan step size depends on zoom (more zoomed in = smaller steps)
            let pan_step = 0.1 / config.zoom;

            ui.separator();
            ui.label("Arrow Controls");

            // Pre-calculate rotation for arrow controls
            // Negate rotation to convert screen space to fractal space
            let cos_r = (-config.rotation).cos();
            let sin_r = (-config.rotation).sin();

            // Arrow keys layout
            ui.horizontal(|ui| {
                ui.add_space(30.0);
                if ui.button("  ^  ").clicked() {
                    // Up in screen space: (0, -1), rotate to fractal space
                    let screen_dx = 0.0;
                    let screen_dy = -pan_step;
                    let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Up".to_string(),
                        false
                    ) {
                        max_update = max_update.max(update_type);
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("  <  ").clicked() {
                    // Left in screen space: (-1, 0), rotate to fractal space
                    let screen_dx = -pan_step;
                    let screen_dy = 0.0;
                    let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Left".to_string(),
                        false
                    ) {
                        max_update = max_update.max(update_type);
                    }
                }
                if ui.button("  v  ").clicked() {
                    // Down in screen space: (0, 1), rotate to fractal space
                    let screen_dx = 0.0;
                    let screen_dy = pan_step;
                    let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Down".to_string(),
                        false
                    ) {
                        max_update = max_update.max(update_type);
                    }
                }
                if ui.button("  >  ").clicked() {
                    // Right in screen space: (1, 0), rotate to fractal space
                    let screen_dx = pan_step;
                    let screen_dy = 0.0;
                    let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                    let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                    if let Ok(update_type) = config_manager.update_batch(
                        vec![
                            (ConfigPath::PanX, new_pan_x.into()),
                            (ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Pan Right".to_string(),
                        false
                    ) {
                        max_update = max_update.max(update_type);
                    }
                }
            });

            ui.separator();

            ui.label("Rotation");
            ui.horizontal(|ui| {
                // Convert radians to degrees for display
                let mut degrees = config.rotation.to_degrees();
                if ui.add(egui::Slider::new(&mut degrees, -180.0..=180.0).suffix("°")).changed() {
                    let new_rotation = degrees.to_radians();
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::Rotation,
                        new_rotation.into(),
                        true  // Lazy undo for slider drag
                    ) {
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
                    let mut degrees_x = config.camera_rotation_x.to_degrees();
                    if ui.add(egui::Slider::new(&mut degrees_x, -180.0..=180.0).suffix("°")).changed() {
                        let new_camera_x = degrees_x.to_radians();
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::CameraRotationX,
                            new_camera_x.into(),
                            true  // Lazy undo for slider drag
                        ) {
                            max_update = max_update.max(update_type);
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Yaw (Y):");
                    let mut degrees_y = config.camera_rotation_y.to_degrees();
                    if ui.add(egui::Slider::new(&mut degrees_y, -180.0..=180.0).suffix("°")).changed() {
                        let new_camera_y = degrees_y.to_radians();
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::CameraRotationY,
                            new_camera_y.into(),
                            true  // Lazy undo for slider drag
                        ) {
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
                    max_update = max_update.max(update_type);
                }
            }
        });

    max_update
}
