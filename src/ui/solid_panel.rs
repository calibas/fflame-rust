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
/// see docs/archive/projects/solid-rendering.md. A solid and the terrains read
/// the shading, the material, the occlusion strength and the lights, and
/// an untouched set lights them with a default sun.
pub fn lighting_rig(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    view: crate::camera::gesture::ViewKind,
) {
    use crate::camera::gesture::ViewKind;
    let flame = matches!(view, ViewKind::Flame2d | ViewKind::Flame3d);
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
    if !flame && shading.rig_untouched() {
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

    // Whether the rig lights anything. A flame's shade pass runs above
    // strength 0. A solid's or a terrain's rig is in use once it is
    // touched, at ANY strength: the path tracer lights it fully, where
    // the lit tier mixes it in by the strength -- so its lights and
    // highlight must show even at 0, or a picture is shaped by settings
    // the panel hides (field-reported: a terrain's default highlight,
    // glinting in the path tracer with the lights out of sight).
    let rig_in_use = if flame { shading.shading_strength > 0.0 } else { !shading.rig_untouched() };
    if !flame && rig_in_use && !(shading.shading_strength > 0.0) {
        ui.label(egui::RichText::new(t!("view.lit_strength_zero")).small().weak());
    }

    let slider = |ui: &mut egui::Ui, config_manager: &mut ConfigManager, path: ConfigPath, label: &str, tip: &str, value: f32, range: std::ops::RangeInclusive<f32>, step: f64| {
        let mut v = value;
        let response = ui.add(super::VkbSlider::new(&mut v, range).text(t!(label).as_ref()).step_by(step));
        let response = if tip.is_empty() { response } else { response.on_hover_text(t!(tip)) };
        if response.changed() {
            let _ = config_manager.update_param(path, v.into());
        }
    };

    if rig_in_use {
        let mut global_sliders: Vec<(ConfigPath, &str, f32, std::ops::RangeInclusive<f32>, f64)> = vec![
            (ConfigPath::SolidAmbient, "view.solid_ambient", shading.ambient, 0.0..=1.0, 0.01),
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

        // A solid's camera, for a point light's placing and its scale:
        // where the eye is, and how far from its target.
        let eye = if view == ViewKind::Solid && shading.lights.iter().any(|l| l.enabled) { solid_eye(&config) } else { None };
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
                if !flame && light.enabled {
                    light_kind_picker(ui, config_manager, li, &light, view, eye);
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
                // What this view does with the light's kind: a kind it does
                // not offer is said, and the light is off here.
                if let super::visibility::Vis::Grey(why) = super::visibility::light_kind(light.kind, view) {
                    ui.label(egui::RichText::new(t!(why)).small().weak());
                }
                light_controls(ui, config_manager, li, &light, eye);
            }
        }
    }

    // The material (docs/archive/projects/materials.md): one per picture, and
    // every tier reads all of it -- the lit tiers and the path tracer
    // alike. Its coat and glow show wherever a surface is lit; the
    // diffuse and the highlight are the rig's, so over a solid's or a
    // terrain's untouched rig -- a default sun -- editing them would
    // replace that sun, and they wait for the rig to be in use.
    if rig_in_use || !flame {
        ui.add_space(6.0);
        ui.label(t!("view.material_section").as_ref()).on_hover_text(t!("view.material_tip"));
        if rig_in_use {
            slider(ui, config_manager, ConfigPath::SolidDiffuse, "view.solid_diffuse", "view.material_diffuse_tip", shading.diffuse, 0.0..=2.0, 0.01);
            slider(ui, config_manager, ConfigPath::SolidSpecular, "view.material_highlight", "view.material_highlight_tip", shading.specular, 0.0..=2.0, 0.01);
            slider(ui, config_manager, ConfigPath::SolidShininess, "view.material_highlight_size", "view.material_highlight_size_tip", shading.shininess, 1.0..=128.0, 1.0);
        }
        slider(ui, config_manager, ConfigPath::SolidGloss, "view.material_gloss", "view.material_gloss_tip", shading.gloss, 0.0..=1.0, 0.01);
        if shading.gloss > 0.0 {
            slider(ui, config_manager, ConfigPath::SolidRoughness, "view.material_roughness", "view.material_roughness_tip", shading.roughness, 0.02..=1.0, 0.01);
        }
        slider(ui, config_manager, ConfigPath::SolidGlow, "view.material_glow", "view.material_glow_tip", shading.glow, 0.0..=4.0, 0.01);
        ui.add_space(6.0);
    }
}

/// A solid's eye in the world and its distance to the target -- where a
/// point light is placed, and the scale its controls move in. None where
/// the config renders no solid.
fn solid_eye(config: &crate::config::FractalConfig) -> Option<([f64; 3], f64)> {
    #[cfg(feature = "engine-escape")]
    {
        let registry = crate::variations::global_registry();
        let ifs3 = crate::escape::ifs::solid_analysis(config, &registry)?;
        let cam = crate::escape::ifs::solid_camera(&config.escape, &ifs3);
        Some((cam.eye, cam.distance))
    }
    #[cfg(not(feature = "engine-escape"))]
    {
        let _ = config;
        None
    }
}

fn light_kind_label(kind: crate::config::LightKind) -> String {
    use crate::config::LightKind;
    match kind {
        LightKind::Directional => t!("view.light_kind_directional").to_string(),
        LightKind::Point => t!("view.light_kind_point").to_string(),
        LightKind::Headlamp => t!("view.light_kind_headlamp").to_string(),
    }
}

/// The light's position and reach set to the camera's eye and distance,
/// as changes: Place at Camera, and a light first made a point.
fn at_camera(li: usize, eye: ([f64; 3], f64)) -> Vec<(ConfigPath, crate::config::ConfigValue)> {
    let mut changes: Vec<(ConfigPath, crate::config::ConfigValue)> = ["x", "y", "z"]
        .iter()
        .enumerate()
        .map(|(k, axis)| (ConfigPath::SolidLightParam { index: li, param: axis.to_string() }, crate::config::ConfigValue::Double(eye.0[k])))
        .collect();
    changes.push((ConfigPath::SolidLightParam { index: li, param: "reach".into() }, (eye.1 as f32).into()));
    changes
}

/// A light's kind (lights-in-the-fractal, L6): each the view offers, and
/// the ones it does not, greyed with the reason. A light first made a
/// point is placed at the camera, so it lights where you are.
fn light_kind_picker(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    li: usize,
    light: &crate::config::SolidLight,
    view: crate::camera::gesture::ViewKind,
    eye: Option<([f64; 3], f64)>,
) {
    use crate::config::LightKind;
    let mut pick = None;
    egui::ComboBox::from_id_salt(("light_kind", li))
        .selected_text(light_kind_label(light.kind))
        .show_ui(ui, |ui| {
            for kind in LightKind::ALL {
                let vis = super::visibility::light_kind(kind, view);
                let r = ui.add_enabled(vis.is_show(), egui::Button::selectable(light.kind == kind, light_kind_label(kind)));
                let r = match vis {
                    super::visibility::Vis::Grey(why) => r.on_disabled_hover_text(t!(why)),
                    _ => r,
                };
                if r.clicked() && light.kind != kind {
                    pick = Some(kind);
                }
            }
        })
        .response
        .on_hover_text(t!("view.light_kind_tip"));
    if let Some(kind) = pick {
        let mut changes = vec![(ConfigPath::SolidLightParam { index: li, param: "kind".into() }, (kind.index() as f32).into())];
        if kind == LightKind::Point && light.position == [0.0; 3] {
            if let Some(eye) = eye {
                changes.extend(at_camera(li, eye));
            }
        }
        let _ = config_manager.update_batch(changes, "history.action.solid_light_kind".to_string());
    }
}

/// A light's own controls, by its kind, and its intensity.
fn light_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    li: usize,
    light: &crate::config::SolidLight,
    eye: Option<([f64; 3], f64)>,
) {
    use crate::config::LightKind;
    let param = |p: &str| ConfigPath::SolidLightParam { index: li, param: p.to_string() };
    let slider = |ui: &mut egui::Ui, config_manager: &mut ConfigManager, p: &str, label: &str, tip: &str, value: f32, range: std::ops::RangeInclusive<f32>, log: bool| {
        let mut v = value;
        let mut s = super::VkbSlider::new(&mut v, range).text(t!(label).as_ref()).logarithmic(log);
        if log {
            s = s.clamping(egui::SliderClamping::Never);
        } else {
            s = s.step_by(0.01);
        }
        let r = ui.add(s);
        let r = if tip.is_empty() { r } else { r.on_hover_text(t!(tip)) };
        if r.changed() {
            let _ = config_manager.update_param(param(p), v.into());
        }
    };
    match light.kind {
        LightKind::Directional => {
            slider(ui, config_manager, "azimuth", "view.light_azimuth", "", light.azimuth, -180.0..=180.0, false);
            slider(ui, config_manager, "elevation", "view.light_elevation", "", light.elevation, -90.0..=90.0, false);
        }
        LightKind::Point => {
            // The position in f64, dragged at the camera's scale.
            let speed = eye.map_or(0.01, |e| e.1 * 0.005);
            ui.horizontal(|ui| {
                ui.label(t!("view.light_position")).on_hover_text(t!("view.light_position_tip"));
                let mut moved = None;
                for (k, axis) in ["x", "y", "z"].iter().enumerate() {
                    let mut v = light.position[k];
                    if ui.add(egui::DragValue::new(&mut v).speed(speed).max_decimals(12).prefix(format!("{} ", axis.to_uppercase()))).changed() {
                        moved = Some((k, v));
                    }
                }
                if let Some((k, v)) = moved {
                    let _ = config_manager.update_param(param(["x", "y", "z"][k]), crate::config::ConfigValue::Double(v));
                }
            });
            if let Some(eye) = eye {
                if ui.button(t!("view.light_place_at_camera")).on_hover_text(t!("view.light_place_at_camera_tip")).clicked() {
                    let _ = config_manager.update_batch(at_camera(li, eye), "history.action.solid_light_place".to_string());
                }
            }
            // The reach on a log scale about the camera's distance, so it
            // spans the scales a deep view moves through.
            let d = eye.map_or(1.0, |e| e.1 as f32).max(1.0e-30);
            slider(ui, config_manager, "reach", "view.light_reach", "view.light_reach_point_tip", light.reach, d * 1.0e-3..=d * 1.0e3, true);
        }
        LightKind::Headlamp => {
            slider(ui, config_manager, "reach", "view.light_reach", "view.light_reach_headlamp_tip", light.reach, 0.02..=50.0, true);
            slider(ui, config_manager, "cone", "view.light_cone", "view.light_cone_tip", light.cone, 1.0..=180.0, false);
            slider(ui, config_manager, "lift", "view.light_lift", "view.light_lift_tip", light.lift, 0.0..=1.0, false);
        }
    }
    slider(ui, config_manager, "intensity", "view.light_intensity", "", light.intensity, 0.0..=4.0, false);
}
