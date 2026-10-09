use crate::config::{ConfigManager, ConfigPath};
use rust_i18n::t;

/// The Solid Lighting panel. Its controls are the View panel's Lighting &
/// Material section now (camera-unification C5); the panel type stays so
/// saved layouts that dock it still load, and draws that same section.
pub fn render_solid_panel_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
) {
    let config = config_manager.active_config().clone();
    if !crate::camera::gesture::view_kind(&config).is_3d() {
        ui.label(
            egui::RichText::new(t!("solid.requires_3d"))
                .small()
                .weak(),
        );
        ui.separator();
    }
    super::view_controls::lighting_3d(ui, config_manager);
}

/// The lights and the lit tier's shading, shared by every 3D view.
///
/// `flame` is the 3D flame pipeline's set: occlusion (solid strength and
/// surface thickness) and the splat shade pass's own controls (SSAO
/// radius, normal smoothing, gap fill, shadow maps) -- our own extension,
/// see docs/projects/solid-rendering.md. A solid and the terrains read
/// the shading, the material, the occlusion strength and the lights, and
/// an untouched set lights them with a default sun.
pub fn lighting_rig(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: bool,
) {
    // Clone config to avoid borrow conflicts (same pattern as view.rs).
    let config = config_manager.active_config().clone();

    if flame {
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
        ui.add_space(8.0);
    }

    // Deferred lighting (shade pass). Works with or without occlusion —
    // any nonzero shading strength turns on the depth capture by itself.
    ui.label(t!("view.lighting_section").as_ref());

    let shading = config.solid_shading.clone();
    if !flame && crate::config::SolidShadingSettings::is_default(&shading) {
        // A solid's or a terrain's untouched rig is a default sun, where
        // a flame's is no lighting: said, so the first change here is not
        // a surprise.
        ui.label(egui::RichText::new(t!("view.lighting_default_rig")).small().weak());
    }
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
        let mut global_sliders: Vec<(ConfigPath, &str, f32, std::ops::RangeInclusive<f32>, f64)> = vec![
            (ConfigPath::SolidAmbient, "view.solid_ambient", shading.ambient, 0.0..=1.0, 0.01),
            (ConfigPath::SolidDiffuse, "view.solid_diffuse", shading.diffuse, 0.0..=2.0, 0.01),
            (ConfigPath::SolidSpecular, "view.solid_specular", shading.specular, 0.0..=2.0, 0.01),
            (ConfigPath::SolidShininess, "view.solid_shininess", shading.shininess, 1.0..=128.0, 1.0),
        ];
        if flame {
            global_sliders.extend([
                (ConfigPath::SsaoStrength, "view.ssao_strength", shading.ssao_strength, 0.0..=1.0, 0.01),
                (ConfigPath::SsaoRadius, "view.ssao_radius", shading.ssao_radius, 0.01..=1.0, 0.01),
                (ConfigPath::NormalSmoothing, "view.normal_smoothing", shading.normal_smoothing as f32, 0.0..=3.0, 1.0),
                (ConfigPath::GapFill, "view.gap_fill", shading.gap_fill as f32, 0.0..=3.0, 1.0),
                // Splat-resolution shadow maps (Stage 2).
                (ConfigPath::SolidShadowStrength, "view.shadow_strength", shading.shadow_strength, 0.0..=1.0, 0.01),
            ]);
        } else {
            // The walk's ambient occlusion, which the same field scales.
            global_sliders.push((ConfigPath::SsaoStrength, "view.occlusion_strength", shading.ssao_strength, 0.0..=1.0, 0.01));
        }
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
