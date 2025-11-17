use crate::scene::transforms::{Flame, RenderMode};
use crate::config::{ConfigManager, ConfigPath};

/// Render view controls content (for docking panels)
pub fn render_view_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &Flame,
) {
    use crate::config::slider::LazyUndoUi;

    // Clone config to avoid borrow conflicts
    let config = config_manager.active_config().clone();

    ui.label("Zoom");
    ui.horizontal(|ui| {
        if ui.button("➕ Zoom In").clicked() {
            let new_zoom = config.zoom * 1.5;
            let _ = config_manager.update_param(
                ConfigPath::Zoom,
                new_zoom.into(),
                false
            );
        }
        if ui.button("➖ Zoom Out").clicked() {
            let new_zoom = config.zoom / 1.5;
            let _ = config_manager.update_param(
                ConfigPath::Zoom,
                new_zoom.into(),
                false
            );
        }
    });
    ui.horizontal(|ui| {
        ui.label("Value:");
        let _ = ui.lazy_drag(config_manager, ConfigPath::Zoom, 0.01, "");
    });

    ui.separator();

    ui.label("Pan");
    ui.horizontal(|ui| {
        ui.label(format!("({:.3}, {:.3})", config.pan_x, config.pan_y));
        ui.label("(use mouse drag or arrow keys/buttons)");
    });

    // Pan step size depends on zoom
    let pan_step = 0.1 / config.zoom;

    ui.separator();
    ui.label("Arrow Controls");

    // Pre-calculate rotation for arrow controls
    let cos_r = (-config.rotation).cos();
    let sin_r = (-config.rotation).sin();

    // Arrow keys layout
    ui.horizontal(|ui| {
        ui.add_space(30.0);
        if ui.button("  ^  ").clicked() {
            let screen_dx = 0.0;
            let screen_dy = -pan_step;
            let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
            let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (new_pan_x, new_pan_y).into(),
                false
            );
        }
    });
    ui.horizontal(|ui| {
        if ui.button("  <  ").clicked() {
            let screen_dx = -pan_step;
            let screen_dy = 0.0;
            let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
            let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (new_pan_x, new_pan_y).into(),
                false
            );
        }
        if ui.button("  v  ").clicked() {
            let screen_dx = 0.0;
            let screen_dy = pan_step;
            let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
            let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (new_pan_x, new_pan_y).into(),
                false
            );
        }
        if ui.button("  >  ").clicked() {
            let screen_dx = pan_step;
            let screen_dy = 0.0;
            let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
            let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (new_pan_x, new_pan_y).into(),
                false
            );
        }
    });

    ui.separator();

    ui.label("Rotation");
    ui.horizontal(|ui| {
        let mut degrees = config.rotation.to_degrees();
        let response = ui.add(egui::Slider::new(&mut degrees, -180.0..=180.0).suffix("°"));
        if response.changed() {
            let new_rotation = degrees.to_radians();
            let _ = config_manager.update_param(
                ConfigPath::Rotation,
                new_rotation.into(),
                response.dragged()
            );
        }
        if response.drag_stopped() && config_manager.is_in_preview_mode() {
            let _ = config_manager.force_commit_preview(&ConfigPath::Rotation);
        }
    });

    ui.separator();

    // 3D Rendering Controls
    ui.label("Render Mode");
    ui.horizontal(|ui| {
        let was_2d = matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::TwoD);
        if ui.selectable_label(was_2d, "2D").clicked() {
            if let Err(e) = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::TwoD.into(),
                false,
            ) {
                log::error!("Failed to update render mode: {}", e);
            }
        }
        if ui.selectable_label(!was_2d, "3D").clicked() {
            if let Err(e) = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::ThreeD.into(),
                false,
            ) {
                log::error!("Failed to update render mode: {}", e);
            }
        }
    });

    // Show projection controls only in 3D mode
    if matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
        ui.label("Projection");
        ui.horizontal(|ui| {
            let is_ortho = matches!(config.flame.projection, crate::scene::transforms::ProjectionType::Orthographic);
            if ui.selectable_label(is_ortho, "Orthographic").clicked() {
                if let Err(e) = config_manager.update_param(
                    ConfigPath::ProjectionType,
                    crate::scene::transforms::ProjectionType::Orthographic.into(),
                    false,
                ) {
                    log::error!("Failed to update projection type: {}", e);
                }
            }
            if ui.selectable_label(!is_ortho, "Perspective").clicked() {
                if let Err(e) = config_manager.update_param(
                    ConfigPath::ProjectionType,
                    crate::scene::transforms::ProjectionType::Perspective { strength: 2.0 }.into(),
                    false,
                ) {
                    log::error!("Failed to update projection type: {}", e);
                }
            }
        });

        // Perspective strength slider
        if let crate::scene::transforms::ProjectionType::Perspective { mut strength } = config.flame.projection {
            let response = ui.add(egui::Slider::new(&mut strength, 0.5..=10.0).text("Perspective Strength"));
            if response.changed() {
                if let Err(e) = config_manager.update_param(
                    ConfigPath::ProjectionType,
                    crate::scene::transforms::ProjectionType::Perspective { strength }.into(),
                    response.dragged(),
                ) {
                    log::error!("Failed to update perspective strength: {}", e);
                }
            }
            if response.drag_stopped() && config_manager.is_in_preview_mode() {
                if let Err(e) = config_manager.force_commit_preview(&ConfigPath::ProjectionType) {
                    log::error!("Failed to commit perspective strength preview: {}", e);
                }
            }
        }
    }

    ui.separator();

    // 3D Camera rotation controls (only visible in 3D mode)
    if matches!(flame.render_mode, RenderMode::ThreeD) {
        ui.separator();
        ui.label("3D Camera Rotation");

        ui.horizontal(|ui| {
            ui.label("Pitch (X):");
            let mut degrees_x = config.camera_rotation_x.to_degrees();
            let response = ui.add(egui::Slider::new(&mut degrees_x, -180.0..=180.0).suffix("°"));
            if response.changed() {
                let new_camera_x = degrees_x.to_radians();
                let _ = config_manager.update_param(
                    ConfigPath::CameraRotationX,
                    new_camera_x.into(),
                    true
                );
            }
            if response.drag_stopped() && config_manager.is_in_preview_mode() {
                let _ = config_manager.force_commit_preview(&ConfigPath::CameraRotationX);
            }
        });

        ui.horizontal(|ui| {
            ui.label("Yaw (Y):");
            let mut degrees_y = config.camera_rotation_y.to_degrees();
            let response = ui.add(egui::Slider::new(&mut degrees_y, -180.0..=180.0).suffix("°"));
            if response.changed() {
                let new_camera_y = degrees_y.to_radians();
                let _ = config_manager.update_param(
                    ConfigPath::CameraRotationY,
                    new_camera_y.into(),
                    true
                );
            }
            if response.drag_stopped() && config_manager.is_in_preview_mode() {
                let _ = config_manager.force_commit_preview(&ConfigPath::CameraRotationY);
            }
        });

        ui.horizontal(|ui| {
            ui.label("Z Position:");
            let _ = ui.lazy_drag(config_manager, ConfigPath::CameraZ, 0.01, "");
        });
    }

    ui.separator();

    if ui.button("🔄 Reset View").clicked() {
        let _ = config_manager.update_batch(
            vec![
                (ConfigPath::Zoom, 1.0.into()),
                (ConfigPath::Pan, (0.0, 0.0).into()),
                (ConfigPath::Rotation, 0.0.into()),
                (ConfigPath::CameraRotationX, 0.0.into()),
                (ConfigPath::CameraRotationY, 0.0.into()),
                (ConfigPath::CameraZ, 0.0.into()),
            ],
            "Reset View".to_string(),
            false
        );
    }
}
