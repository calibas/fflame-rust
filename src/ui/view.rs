use crate::scene::transforms::{Flame, RenderMode};
use crate::config::{ConfigManager, ConfigPath};
use rust_i18n::t;

/// Render view controls content (for docking panels)
pub fn render_view_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &Flame,
) {
    use crate::config::slider::LazyUndoUi;

    // Clone config to avoid borrow conflicts
    let config = config_manager.active_config().clone();

    ui.label(t!("view.zoom")).on_hover_text(t!("view.tooltip_zoom"));
    ui.horizontal(|ui| {
        if ui.button(t!("view.zoom_in").as_ref()).clicked() {
            let new_zoom = config.zoom * 1.5;
            let _ = config_manager.update_param(
                ConfigPath::Zoom,
                new_zoom.into());
        }
        if ui.button(t!("view.zoom_out").as_ref()).clicked() {
            let new_zoom = config.zoom / 1.5;
            let _ = config_manager.update_param(
                ConfigPath::Zoom,
                new_zoom.into()
            );
        }
    });
    ui.horizontal(|ui| {
        ui.label(t!("view.zoom_value"));
        let _ = ui.lazy_drag(config_manager, ConfigPath::Zoom, 0.01, "");
    });

    ui.separator();

    ui.label(t!("view.pan")).on_hover_text(t!("view.tooltip_pan"));
    ui.horizontal(|ui| {
        ui.label(t!("view.pan_x"));
        let mut pan_x = config.pan_x;
        let response_x = ui.add(
            egui::DragValue::new(&mut pan_x)
                .speed(0.001 / config.zoom)
                .custom_formatter(|v, _| format!("{:.7}", v))
        );
        if response_x.changed() {
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (pan_x, config.pan_y).into()
            );
        }

        ui.label(t!("view.pan_y"));
        let mut pan_y = config.pan_y;
        let response_y = ui.add(
            egui::DragValue::new(&mut pan_y)
                .speed(0.001 / config.zoom)
                .custom_formatter(|v, _| format!("{:.7}", v))
        );
        if response_y.changed() {
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (config.pan_x, pan_y).into()
            );
        }
    });

    // Pan step size depends on zoom
    let pan_step = 0.1 / config.zoom;

    ui.separator();
    ui.label(t!("view.arrow_controls"));

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
                (new_pan_x, new_pan_y).into()
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
                (new_pan_x, new_pan_y).into()
            );
        }
        if ui.button("  v  ").clicked() {
            let screen_dx = 0.0;
            let screen_dy = pan_step;
            let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
            let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (new_pan_x, new_pan_y).into()
            );
        }
        if ui.button("  >  ").clicked() {
            let screen_dx = pan_step;
            let screen_dy = 0.0;
            let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
            let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (new_pan_x, new_pan_y).into()
            );
        }
    });

    ui.separator();

    ui.horizontal(|ui| {
        ui.label(t!("view.rotation")).on_hover_text(t!("view.tooltip_rotation"));
        let mut degrees = config.rotation.to_degrees();
        let response = ui.add(egui::Slider::new(&mut degrees, -180.0..=180.0).suffix("°"));
        if response.changed() {
            let new_rotation = degrees.to_radians();
            let _ = config_manager.update_param(
                ConfigPath::Rotation,
                new_rotation.into()
            );
        }
    });

    ui.separator();

    // 3D Rendering Controls
    ui.label(t!("view.render_mode")).on_hover_text(t!("view.tooltip_render_mode"));
    ui.horizontal(|ui| {
        let was_2d = matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::TwoD);
        if ui.selectable_label(was_2d, t!("view.mode_2d").as_ref()).clicked() {
            if let Err(e) = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::TwoD.into()
            ) {
                log::error!("Failed to update render mode: {}", e);
            }
        }
        if ui.selectable_label(!was_2d, t!("view.mode_3d").as_ref()).clicked() {
            if let Err(e) = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::ThreeD.into()
            ) {
                log::error!("Failed to update render mode: {}", e);
            }
        }
    });

    // Show perspective control only in 3D mode
    if matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
        let mut perspective = config.flame.perspective_strength;
        let response = ui.add(
            egui::Slider::new(&mut perspective, 0.0..=10.0)
                .text(t!("view.perspective").as_ref())
                .step_by(0.1)
        ).on_hover_text(t!("view.tooltip_perspective"));
        let changed = response.changed();
        if changed {
            if let Err(e) = config_manager.update_param(
                ConfigPath::PerspectiveStrength,
                perspective.into()
            ) {
                log::error!("Failed to update perspective strength: {}", e);
            }
        }
    }

    ui.separator();

    // 3D Camera rotation controls (only visible in 3D mode)
    if matches!(flame.render_mode, RenderMode::ThreeD) {
        ui.separator();
        ui.label(t!("view.camera_3d"));

        ui.horizontal(|ui| {
            ui.label(t!("view.camera_pitch")).on_hover_text(t!("view.tooltip_camera_pitch"));
            let mut degrees_x = config.camera_rotation_x.to_degrees();
            let response = ui.add(egui::Slider::new(&mut degrees_x, -180.0..=180.0).suffix("°"));
            if response.changed() {
                let new_camera_x = degrees_x.to_radians();
                let _ = config_manager.update_param(
                    ConfigPath::CameraRotationX,
                    new_camera_x.into()
                );
            }
        });

        ui.horizontal(|ui| {
            ui.label(t!("view.camera_yaw")).on_hover_text(t!("view.tooltip_camera_yaw"));
            let mut degrees_y = config.camera_rotation_y.to_degrees();
            let response = ui.add(egui::Slider::new(&mut degrees_y, -180.0..=180.0).suffix("°"));
            if response.changed() {
                let new_camera_y = degrees_y.to_radians();
                let _ = config_manager.update_param(
                    ConfigPath::CameraRotationY,
                    new_camera_y.into()
                );
            }
        });

        ui.horizontal(|ui| {
            ui.label(t!("view.camera_z")).on_hover_text(t!("view.tooltip_camera_z"));
            let _ = ui.lazy_drag(config_manager, ConfigPath::CameraZ, 0.01, "");
        });
    }

    ui.separator();

    if ui.button(t!("view.reset").as_ref()).clicked() {
        let _ = config_manager.update_batch(
            vec![
                (ConfigPath::Zoom, 1.0.into()),
                (ConfigPath::Pan, (0.0, 0.0).into()),
                (ConfigPath::Rotation, 0.0.into()),
                (ConfigPath::CameraRotationX, 0.0.into()),
                (ConfigPath::CameraRotationY, 0.0.into()),
                (ConfigPath::CameraZ, 0.0.into()),
            ],
            "Reset View".to_string()
        );
    }
}
