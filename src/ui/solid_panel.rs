use crate::config::{ConfigManager, ConfigPath};
use rust_i18n::t;

/// Solid Rendering & Lighting panel: occlusion (solid strength +
/// surface thickness) and the deferred shade pass (lighting, SSAO,
/// shadow maps, per-light controls). Our own extension — see
/// docs/projects/solid-rendering.md. Moved out of the View panel's
/// Depth Effects section (2026-07-16, user request).
pub fn render_solid_panel_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
) {
    // Clone config to avoid borrow conflicts (same pattern as view.rs).
    let config = config_manager.active_config().clone();

    if !matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
        ui.label(
            egui::RichText::new(t!("solid.requires_3d"))
                .small()
                .weak(),
        );
        ui.separator();
    }

    // Solid rendering (occlusion): nearest-depth culling.
    ui.label(t!("view.solid_section").as_ref());

    let mut solid_strength = config.solid_strength;
    let response = ui.add(
        super::VkbSlider::new(&mut solid_strength, 0.0..=1.0)
            .text(t!("view.solid_strength").as_ref())
            .step_by(0.01)
    ).on_hover_text(t!("view.tooltip_solid_strength"));
    if response.changed() {
        let _ = config_manager.update_param(
            ConfigPath::SolidStrength,
            solid_strength.into()
        );
    }

    let mut surface_thickness = config.surface_thickness;
    let response = ui.add(
        super::VkbSlider::new(&mut surface_thickness, 0.001..=0.5)
            .text(t!("view.surface_thickness").as_ref())
            .step_by(0.001)
    ).on_hover_text(t!("view.tooltip_surface_thickness"));
    if response.changed() {
        let _ = config_manager.update_param(
            ConfigPath::SurfaceThickness,
            surface_thickness.into()
        );
    }

    // Deferred lighting (shade pass). Works with or without occlusion —
    // any nonzero shading strength turns on the depth capture by itself.
    ui.add_space(8.0);
    ui.label(t!("view.lighting_section").as_ref());

    let shading = config.solid_shading.clone();
    let mut shading_strength = shading.shading_strength;
    let response = ui.add(
        super::VkbSlider::new(&mut shading_strength, 0.0..=1.0)
            .text(t!("view.shading_strength").as_ref())
            .step_by(0.01)
    ).on_hover_text(t!("view.tooltip_shading_strength"));
    if response.changed() {
        let _ = config_manager.update_param(
            ConfigPath::ShadingStrength,
            shading_strength.into()
        );
    }

    if shading.shading_strength > 0.0 {
        let global_sliders: [(ConfigPath, &str, f32, std::ops::RangeInclusive<f32>, f64); 9] = [
            (ConfigPath::SolidAmbient, "view.solid_ambient", shading.ambient, 0.0..=1.0, 0.01),
            (ConfigPath::SolidDiffuse, "view.solid_diffuse", shading.diffuse, 0.0..=2.0, 0.01),
            (ConfigPath::SolidSpecular, "view.solid_specular", shading.specular, 0.0..=2.0, 0.01),
            (ConfigPath::SolidShininess, "view.solid_shininess", shading.shininess, 1.0..=128.0, 1.0),
            (ConfigPath::SsaoStrength, "view.ssao_strength", shading.ssao_strength, 0.0..=1.0, 0.01),
            (ConfigPath::SsaoRadius, "view.ssao_radius", shading.ssao_radius, 0.01..=1.0, 0.01),
            (ConfigPath::NormalSmoothing, "view.normal_smoothing", shading.normal_smoothing as f32, 0.0..=3.0, 1.0),
            (ConfigPath::GapFill, "view.gap_fill", shading.gap_fill as f32, 0.0..=3.0, 1.0),
            // Splat-resolution shadow maps (Stage 2).
            (ConfigPath::SolidShadowStrength, "view.shadow_strength", shading.shadow_strength, 0.0..=1.0, 0.01),
        ];
        for (path, label, value, range, step) in global_sliders {
            let mut v = value;
            let response = ui.add(
                super::VkbSlider::new(&mut v, range)
                    .text(t!(label).as_ref())
                    .step_by(step)
            );
            if response.changed() {
                let _ = config_manager.update_param(path, v.into());
            }
        }

        for li in 0..4usize {
            let light = shading.lights[li];
            ui.horizontal(|ui| {
                let mut enabled = light.enabled;
                if ui.checkbox(&mut enabled, format!("{} {}", t!("view.solid_light"), li + 1)).changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::SolidLightEnabled { index: li },
                        enabled.into()
                    );
                }
                let mut col = light.color;
                if ui.color_edit_button_rgb(&mut col).changed() {
                    let _ = config_manager.update_batch(
                        vec![
                            (ConfigPath::SolidLightParam { index: li, param: "color_r".into() }, col[0].into()),
                            (ConfigPath::SolidLightParam { index: li, param: "color_g".into() }, col[1].into()),
                            (ConfigPath::SolidLightParam { index: li, param: "color_b".into() }, col[2].into()),
                        ],
                        "history.action.solid_light_color".to_string(),
                    );
                }
            });
            if light.enabled {
                let light_sliders: [(&str, &str, f32, std::ops::RangeInclusive<f32>, f64); 3] = [
                    ("azimuth", "view.light_azimuth", light.azimuth, -180.0..=180.0, 1.0),
                    ("elevation", "view.light_elevation", light.elevation, -90.0..=90.0, 1.0),
                    ("intensity", "view.light_intensity", light.intensity, 0.0..=4.0, 0.01),
                ];
                for (param, label, value, range, step) in light_sliders {
                    let mut v = value;
                    let response = ui.add(
                        super::VkbSlider::new(&mut v, range)
                            .text(t!(label).as_ref())
                            .step_by(step)
                    );
                    if response.changed() {
                        let _ = config_manager.update_param(
                            ConfigPath::SolidLightParam { index: li, param: param.into() },
                            v.into()
                        );
                    }
                }
            }
        }
    }
}
