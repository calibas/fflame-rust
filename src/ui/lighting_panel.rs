use crate::camera::gesture::ViewKind;
use crate::config::{ConfigManager, ConfigPath, ConfigValue, FractalConfig, SolidShadingSettings};
use rust_i18n::t;

/// The Lighting & Material panel (docs/projects/3d-look-and-tools.md, P3):
/// every lit 3D view's lighting, shadows, material and surface in one
/// place, in sections -- Rendering, Sky, Lights, Shading, Shadows,
/// Material, Surface -- each drawn where the view has it. A solid's
/// shadows and occlusion are formula parameters in the file and a
/// terrain's its own fields; the panel draws them with the rest of the
/// light. The panel type is `SolidLighting`, the name saved layouts know.
pub fn render_lighting_panel_content(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    let config = config_manager.active_config().clone();
    let view = crate::camera::gesture::view_kind(&config);
    if !view.is_3d() {
        ui.label(egui::RichText::new(t!("solid.requires_3d")).small().weak());
        return;
    }
    let rig = Rig::of(&config, view);
    let section = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
        let r = egui::CollapsingHeader::new(egui::RichText::new(t!(title)).strong())
            .id_salt(("lighting_section", title.to_string()))
            .default_open(true)
            .show(ui, |ui| body(ui));
        if title == "lighting.material" {
            r.header_response.on_hover_text(t!("view.material_tip"));
        }
    };

    #[cfg(feature = "engine-escape")]
    if let Some((pt, paths, tier_path, tier, id)) = super::view_controls::path_of(&config) {
        section(ui, "lighting.rendering", &mut |ui| {
            super::escape_panel::show_render_tier(ui, config_manager, &pt, &paths, tier_path.clone(), tier, id)
        });
        section(ui, "lighting.sky", &mut |ui| {
            super::escape_panel::show_sky(ui, config_manager, config.background_color, &pt, &paths, tier)
        });
    }
    section(ui, "lighting.lights", &mut |ui| lights(ui, config_manager, &config, view, &rig));
    section(ui, "lighting.shading", &mut |ui| shading(ui, config_manager, &config, view, &rig));
    section(ui, "lighting.shadows", &mut |ui| shadows(ui, config_manager, &config, view, &rig));
    if rig.in_use || view != ViewKind::Flame3d {
        section(ui, "lighting.material", &mut |ui| material(ui, config_manager, &rig));
    }
    if matches!(view, ViewKind::Flame3d | ViewKind::EscapeTerrain) {
        section(ui, "lighting.surface", &mut |ui| surface(ui, config_manager, &config, view));
    }
}

/// The lighting rig as the panel draws and writes it.
///
/// A solid's or a terrain's UNTOUCHED rig is drawn with a default key
/// light (`SolidShadingSettings::rig_untouched`), not with its stored
/// values. Its rows show from the start (P3), with what the first change
/// makes the rig: that change takes the rig over -- full strength, no
/// ambient, a terrain's sun where it was (`render_mode::rig_taken_over`,
/// the 3D look's own) -- with it, one undo step. Before, a change made
/// the stored rig current at its stored strength of 0: the lit tier fell
/// to the bare colours.
struct Rig {
    /// What the rows show: the stored rig, or the one taking it over makes.
    shown: SolidShadingSettings,
    /// The changes that take an untouched rig over; empty once it is
    /// touched, and in a 3D flame, whose untouched rig is no lighting.
    take_over: Vec<(ConfigPath, ConfigValue)>,
    /// Whether the rig's rows draw: always over a solid or a terrain; in a
    /// 3D flame once its strength is above 0 (at 0 the shade pass is off).
    in_use: bool,
    /// The view is a 3D flame's: the splat pipeline's own controls.
    flame: bool,
}

impl Rig {
    fn of(config: &FractalConfig, view: ViewKind) -> Self {
        let flame = view == ViewKind::Flame3d;
        let shading = &config.solid_shading;
        if flame {
            return Self { shown: shading.clone(), take_over: Vec::new(), in_use: shading.shading_strength > 0.0, flame };
        }
        if !shading.rig_untouched() {
            return Self { shown: shading.clone(), take_over: Vec::new(), in_use: true, flame };
        }
        let take_over = super::render_mode::rig_taken_over(config);
        let mut after = config.clone();
        for (path, value) in &take_over {
            let _ = ConfigManager::apply_value_detached(&mut after, crate::config::EditingTarget::Main, path, value.clone());
        }
        Self { shown: after.solid_shading, take_over, in_use: true, flame }
    }

    /// Whether the picture is the default key light's, not the rows'.
    fn untouched(&self) -> bool {
        !self.take_over.is_empty()
    }

    /// One of the rig's values changed.
    fn set(&self, config_manager: &mut ConfigManager, path: ConfigPath, value: ConfigValue) {
        if self.untouched() {
            self.set_many(config_manager, vec![(path, value)], "history.action.lighting_take_over");
        } else {
            let _ = config_manager.update_param(path, value);
        }
    }

    /// Several of the rig's values, one undo step: taking an untouched rig
    /// over with them.
    fn set_many(&self, config_manager: &mut ConfigManager, changes: Vec<(ConfigPath, ConfigValue)>, description: &str) {
        let mut all: Vec<_> = self.take_over.iter().filter(|(p, _)| !changes.iter().any(|(q, _)| q == p)).cloned().collect();
        all.extend(changes);
        let description = if self.untouched() { "history.action.lighting_take_over" } else { description };
        let _ = config_manager.update_batch(all, description.to_string());
    }

    fn slider(
        &self,
        ui: &mut egui::Ui,
        config_manager: &mut ConfigManager,
        path: ConfigPath,
        label: &str,
        tip: &str,
        value: f32,
        range: std::ops::RangeInclusive<f32>,
        step: f64,
    ) {
        let mut v = value;
        let r = ui.add(super::VkbSlider::new(&mut v, range).text(t!(label).as_ref()).step_by(step));
        let r = if tip.is_empty() { r } else { r.on_hover_text(t!(tip)) };
        if r.changed() {
            self.set(config_manager, path, v.into());
        }
    }
}

/// The four lights.
fn lights(ui: &mut egui::Ui, config_manager: &mut ConfigManager, config: &FractalConfig, view: ViewKind, rig: &Rig) {
    if rig.untouched() {
        // Said, so the rows below are not read as the picture's.
        ui.label(egui::RichText::new(t!("view.lighting_default_rig")).small().weak());
    }
    if !rig.in_use {
        ui.label(egui::RichText::new(t!("lighting.flame_unlit")).small().weak());
        return;
    }
    // Whose space the directional lights are in. No conversion: the
    // lights keep their angles, read in the other space (the Fly Mode
    // section's button converts).
    match super::visibility::light_frame(view) {
        super::visibility::Vis::Show => {
            let mut follow = config.solid_shading.lights_follow_camera;
            if ui.checkbox(&mut follow, t!("lighting.lights_follow_camera").as_ref()).on_hover_text(t!("lighting.lights_follow_camera_tip")).changed() {
                let _ = config_manager.update_param(ConfigPath::SolidLightsFollowCamera, follow.into());
            }
        }
        super::visibility::Vis::Grey(why) => {
            let mut on = true;
            ui.add_enabled(false, egui::Checkbox::new(&mut on, t!("lighting.lights_follow_camera").as_ref())).on_disabled_hover_text(t!(why));
        }
        super::visibility::Vis::Hide => {}
    }
    let shading = &rig.shown;
    // A solid's camera, for a point light's placing and its scale:
    // where the eye is, and how far from its target.
    let eye = if view == ViewKind::Solid && shading.lights.iter().any(|l| l.enabled) { solid_eye(config) } else { None };
    for li in 0..4usize {
        let light = shading.lights[li];
        ui.horizontal(|ui| {
            let mut enabled = light.enabled;
            if ui.checkbox(&mut enabled, format!("{} {}", t!("view.solid_light"), li + 1)).changed() {
                rig.set(config_manager, ConfigPath::SolidLightEnabled { index: li }, enabled.into());
            }
            if !rig.flame && light.enabled {
                light_kind_picker(ui, config_manager, rig, li, &light, view, eye);
            }
            let mut col = light.color;
            if ui.color_edit_button_rgb(&mut col).changed() {
                rig.set_many(
                    config_manager,
                    vec![
                        (ConfigPath::SolidLightParam { index: li, param: "color_r".into() }, col[0].into()),
                        (ConfigPath::SolidLightParam { index: li, param: "color_g".into() }, col[1].into()),
                        (ConfigPath::SolidLightParam { index: li, param: "color_b".into() }, col[2].into()),
                    ],
                    "history.action.solid_light_color",
                );
            }
        });
        if light.enabled {
            // What this view does with the light's kind: a kind it does
            // not offer is said, and the light is off here.
            if let super::visibility::Vis::Grey(why) = super::visibility::light_kind(light.kind, view) {
                ui.label(egui::RichText::new(t!(why)).small().weak());
            }
            light_controls(ui, config_manager, rig, li, &light, eye);
        }
    }
}

/// How strongly the rig lights, the light from everywhere, and the
/// occlusion that stands in for the sky in the lit tier.
fn shading(ui: &mut egui::Ui, config_manager: &mut ConfigManager, config: &FractalConfig, view: ViewKind, rig: &Rig) {
    let shading = &rig.shown;
    rig.slider(ui, config_manager, ConfigPath::ShadingStrength, "view.shading_strength", "view.tooltip_shading_strength", shading.shading_strength, 0.0..=1.0, 0.01);
    if !rig.flame && !(shading.shading_strength > 0.0) {
        ui.label(egui::RichText::new(t!("view.lit_strength_zero")).small().weak());
    }
    if !rig.in_use {
        return;
    }
    rig.slider(ui, config_manager, ConfigPath::SolidAmbient, "view.solid_ambient", "lighting.ambient_tip", shading.ambient, 0.0..=1.0, 0.01);
    if rig.flame {
        rig.slider(ui, config_manager, ConfigPath::SsaoStrength, "view.ssao_strength", "", shading.ssao_strength, 0.0..=1.0, 0.01);
        rig.slider(ui, config_manager, ConfigPath::SsaoRadius, "view.ssao_radius", "", shading.ssao_radius, 0.01..=1.0, 0.01);
        rig.slider(ui, config_manager, ConfigPath::NormalSmoothing, "view.normal_smoothing", "", shading.normal_smoothing as f32, 0.0..=3.0, 1.0);
        rig.slider(ui, config_manager, ConfigPath::GapFill, "view.gap_fill", "", shading.gap_fill as f32, 0.0..=3.0, 1.0);
        return;
    }
    // The walk's occlusion, the lit tier's stand-in for the sky the path
    // tracer traces: hidden where only the path tracer runs.
    if lit_tier_runs(config, view) {
        rig.slider(ui, config_manager, ConfigPath::SsaoStrength, "view.occlusion_strength", "lighting.occlusion_strength_tip", shading.ssao_strength, 0.0..=1.0, 0.01);
        match view {
            #[cfg(feature = "engine-escape")]
            ViewKind::Solid => solid_params(ui, config_manager, config, &["occlusion"]),
            #[cfg(feature = "terrain")]
            ViewKind::EscapeTerrain => {
                let t = &config.escape.terrain;
                float_row(ui, config_manager, "escape_panel.terrain_occlusion", "escape_panel.terrain_occlusion_tip", ConfigPath::EscapeTerrainOcclusion, t.occlusion, 0.0..=0.05);
            }
            #[cfg(all(feature = "terrain", feature = "engine-sim"))]
            ViewKind::SimTerrain => {
                let t = &config.sim.terrain;
                float_row(ui, config_manager, "escape_panel.terrain_occlusion", "sim_panel.terrain_occlusion_tip", ConfigPath::SimTerrainOcclusion, t.occlusion, 0.0..=0.2);
            }
            _ => {}
        }
    }
}

/// The traced shadows: a solid's and a terrain's, a 3D flame's shadow
/// maps.
fn shadows(ui: &mut egui::Ui, config_manager: &mut ConfigManager, config: &FractalConfig, view: ViewKind, rig: &Rig) {
    match view {
        ViewKind::Flame3d => {
            if rig.in_use {
                // Splat-resolution shadow maps (Stage 2).
                rig.slider(ui, config_manager, ConfigPath::SolidShadowStrength, "view.shadow_strength", "", rig.shown.shadow_strength, 0.0..=1.0, 0.01);
            } else {
                ui.label(egui::RichText::new(t!("lighting.flame_unlit")).small().weak());
            }
        }
        #[cfg(feature = "engine-escape")]
        ViewKind::Solid => {
            solid_params(ui, config_manager, config, &["shadow", "shadow_sharpness"]);
            ray_tolerance(ui, config_manager, config.solid_shading.ray_tolerance);
        }
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => {
            let t = &config.escape.terrain;
            terrain_shadows(ui, config_manager, [t.shadow, t.shadow_sharpness], [ConfigPath::EscapeTerrainShadow, ConfigPath::EscapeTerrainShadowSharpness]);
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => {
            let t = &config.sim.terrain;
            terrain_shadows(ui, config_manager, [t.shadow, t.shadow_sharpness], [ConfigPath::SimTerrainShadow, ConfigPath::SimTerrainShadowSharpness]);
        }
        _ => {
            let _ = (config_manager, config);
        }
    }
}

/// The material (docs/archive/projects/materials.md): one per picture,
/// and every tier reads all of it. The diffuse and the highlight are the
/// rig's (taking an untouched one over); the coat and the glow are not,
/// and leave it untouched.
fn material(ui: &mut egui::Ui, config_manager: &mut ConfigManager, rig: &Rig) {
    let shading = &rig.shown;
    if rig.in_use {
        rig.slider(ui, config_manager, ConfigPath::SolidDiffuse, "view.solid_diffuse", "view.material_diffuse_tip", shading.diffuse, 0.0..=2.0, 0.01);
        rig.slider(ui, config_manager, ConfigPath::SolidSpecular, "view.material_highlight", "view.material_highlight_tip", shading.specular, 0.0..=2.0, 0.01);
        rig.slider(ui, config_manager, ConfigPath::SolidShininess, "view.material_highlight_size", "view.material_highlight_size_tip", shading.shininess, 1.0..=128.0, 1.0);
    }
    let coat = |ui: &mut egui::Ui, config_manager: &mut ConfigManager, path: ConfigPath, label: &str, tip: &str, value: f32, range: std::ops::RangeInclusive<f32>| {
        let mut v = value;
        if ui.add(super::VkbSlider::new(&mut v, range).text(t!(label).as_ref()).step_by(0.01)).on_hover_text(t!(tip)).changed() {
            let _ = config_manager.update_param(path, v.into());
        }
    };
    coat(ui, config_manager, ConfigPath::SolidGloss, "view.material_gloss", "view.material_gloss_tip", shading.gloss, 0.0..=1.0);
    coat(ui, config_manager, ConfigPath::SolidMetallic, "view.material_metallic", "view.material_metallic_tip", shading.metallic, 0.0..=1.0);
    // The albedo's scale: what the path tracer's bounces keep. Not a 3D
    // flame's, which has no bounces (its Diffuse darkens it alike).
    if !rig.flame {
        coat(ui, config_manager, ConfigPath::SolidReflectance, "view.material_reflectance", "view.material_reflectance_tip", shading.reflectance, 0.0..=1.0);
    }
    if shading.gloss > 0.0 || shading.metallic > 0.0 {
        coat(ui, config_manager, ConfigPath::SolidRoughness, "view.material_roughness", "view.material_roughness_tip", shading.roughness, 0.02..=1.0);
    }
    coat(ui, config_manager, ConfigPath::SolidGlow, "view.material_glow", "view.material_glow_tip", shading.glow, 0.0..=4.0);
}

/// What the surface is beyond its material: a 3D flame's solidity, a
/// terrain's texture and its lake.
fn surface(ui: &mut egui::Ui, config_manager: &mut ConfigManager, config: &FractalConfig, view: ViewKind) {
    match view {
        ViewKind::Flame3d => {
            // Solid rendering (occlusion): nearest-depth culling. Our own
            // extension, docs/archive/projects/solid-rendering.md.
            let row = |ui: &mut egui::Ui, config_manager: &mut ConfigManager, path: ConfigPath, label: &str, tip: &str, value: f32, range: std::ops::RangeInclusive<f32>, step: f64| {
                let mut v = value;
                if ui.add(super::VkbSlider::new(&mut v, range).text(t!(label).as_ref()).step_by(step)).on_hover_text(t!(tip)).changed() {
                    let _ = config_manager.update_param(path, v.into());
                }
            };
            row(ui, config_manager, ConfigPath::SolidStrength, "view.solid_strength", "view.tooltip_solid_strength", config.solid_strength, 0.0..=1.0, 0.01);
            row(ui, config_manager, ConfigPath::SurfaceThickness, "view.surface_thickness", "view.tooltip_surface_thickness", config.surface_thickness, 0.001..=0.5, 0.001);
        }
        #[cfg(all(feature = "terrain", feature = "engine-escape"))]
        ViewKind::EscapeTerrain => {
            let esc = &config.escape;
            super::escape_panel::surface_texture_controls(ui, config_manager, esc, &esc.shading);
            if esc.terrain.interior == crate::config::escape::TerrainInterior::Lake {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.terrain_lake_tint"));
                    let mut rgb = esc.terrain.lake_tint;
                    if ui.color_edit_button_rgb(&mut rgb).on_hover_text(t!("escape_panel.terrain_lake_tint_tip")).changed() {
                        let _ = config_manager.update_param(ConfigPath::EscapeTerrainLakeTint, ConfigValue::ColorRgb(rgb));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.terrain_lake_roughness"));
                    let mut v = esc.terrain.lake_roughness;
                    if ui
                        .add(egui::Slider::new(&mut v, 0.02..=1.0).logarithmic(true))
                        .on_hover_text(t!("escape_panel.terrain_lake_roughness_tip"))
                        .changed()
                    {
                        let _ = config_manager.update_param(ConfigPath::EscapeTerrainLakeRoughness, v.into());
                    }
                });
            }
        }
        _ => {
            let _ = (config_manager, config);
        }
    }
}

/// Whether the lit tier draws this view at any time: its tier is not Path
/// traced alone. (Auto draws lit while the view moves.)
fn lit_tier_runs(config: &FractalConfig, view: ViewKind) -> bool {
    use crate::config::escape::RenderTier;
    let tier = match view {
        ViewKind::Solid => config.escape.solid_tier,
        ViewKind::EscapeTerrain => config.escape.terrain.tier,
        ViewKind::SimTerrain => config.sim.terrain.tier,
        _ => return true,
    };
    tier != RenderTier::PathTraced
}

/// How close a solid's rays leaving a surface -- its shadows, the path
/// tracer's bounces -- may pass it and count as a hit
/// (`SolidShadingSettings::ray_tolerance`): Fine is the exact one,
/// Medium and Coarse march fewer steps.
#[cfg(feature = "engine-escape")]
fn ray_tolerance(ui: &mut egui::Ui, config_manager: &mut ConfigManager, value: f32) {
    const STEPS: [(f32, &str); 3] = [(1.0, "lighting.ray_tolerance_fine"), (3.0, "lighting.ray_tolerance_medium"), (10.0, "lighting.ray_tolerance_coarse")];
    // The nearest step names a value a file holds between them.
    let current = STEPS.iter().min_by(|a, b| (a.0 - value).abs().total_cmp(&(b.0 - value).abs())).map_or(STEPS[0].1, |s| s.1);
    ui.horizontal(|ui| {
        ui.label(t!("lighting.ray_tolerance"));
        egui::ComboBox::from_id_salt("solid_ray_tolerance")
            .selected_text(t!(current))
            .show_ui(ui, |ui| {
                for (v, key) in STEPS {
                    if ui.selectable_label(current == key, t!(key)).clicked() && v != value {
                        let _ = config_manager.update_param(ConfigPath::SolidRayTolerance, v.into());
                    }
                }
            })
            .response
            .on_hover_text(t!("lighting.ray_tolerance_tip"));
    });
}

/// A solid's formula parameters named, from its def (its bounds and
/// tooltips; the Escape panel's control).
#[cfg(feature = "engine-escape")]
fn solid_params(ui: &mut egui::Ui, config_manager: &mut ConfigManager, config: &FractalConfig, names: &[&str]) {
    let Some(def) = crate::escape::ifs::get_ifs(&config.escape.formula) else {
        return;
    };
    for name in names {
        let Some(p) = def.parameters.iter().find(|p| p.name == *name) else {
            continue;
        };
        let mut v = config.escape.formula_params.get(p.name).copied().unwrap_or(p.default);
        let slow = crate::escape::is_slow_param(def.name, p.name);
        if super::escape_panel::param_control(ui, &mut v, p, "lighting", slow) {
            let _ = config_manager.update_param(ConfigPath::EscapeFormulaParam { param: p.name.to_string() }, v.into());
        }
    }
}

/// A terrain's traced shadows: `[shadow, sharpness]` and their paths.
#[cfg(feature = "terrain")]
fn terrain_shadows(ui: &mut egui::Ui, config_manager: &mut ConfigManager, values: [f32; 2], paths: [ConfigPath; 2]) {
    let [shadow, sharpness] = values;
    let [p_shadow, p_sharpness] = paths;
    float_row(ui, config_manager, "escape_panel.terrain_shadow", "escape_panel.terrain_shadow_tip", p_shadow, shadow, 0.0..=1.0);
    let mut v = sharpness;
    if ui
        .add(super::VkbSlider::new(&mut v, 1.0..=128.0).logarithmic(true).text(t!("escape_panel.terrain_shadow_sharpness")))
        .on_hover_text(t!("escape_panel.terrain_shadow_sharpness_tip"))
        .changed()
    {
        let _ = config_manager.update_param(p_sharpness, v.into());
    }
}

#[cfg(feature = "terrain")]
fn float_row(ui: &mut egui::Ui, config_manager: &mut ConfigManager, label: &str, tip: &str, path: ConfigPath, value: f32, range: std::ops::RangeInclusive<f32>) {
    let mut v = value;
    if ui.add(super::VkbSlider::new(&mut v, range).text(t!(label))).on_hover_text(t!(tip)).changed() {
        let _ = config_manager.update_param(path, v.into());
    }
}

/// A solid's eye in the world and its distance to the target -- where a
/// point light is placed, and the scale its controls move in. None where
/// the config renders no solid.
fn solid_eye(config: &FractalConfig) -> Option<([f64; 3], f64)> {
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
fn at_camera(li: usize, eye: ([f64; 3], f64)) -> Vec<(ConfigPath, ConfigValue)> {
    let mut changes: Vec<(ConfigPath, ConfigValue)> = ["x", "y", "z"]
        .iter()
        .enumerate()
        .map(|(k, axis)| (ConfigPath::SolidLightParam { index: li, param: axis.to_string() }, ConfigValue::Double(eye.0[k])))
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
    rig: &Rig,
    li: usize,
    light: &crate::config::SolidLight,
    view: ViewKind,
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
        rig.set_many(config_manager, changes, "history.action.solid_light_kind");
    }
}

/// A light's own controls, by its kind, and its intensity.
fn light_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    rig: &Rig,
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
            rig.set(config_manager, param(p), v.into());
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
                    rig.set(config_manager, param(["x", "y", "z"][k]), ConfigValue::Double(v));
                }
            });
            if let Some(eye) = eye {
                if ui.button(t!("view.light_place_at_camera")).on_hover_text(t!("view.light_place_at_camera_tip")).clicked() {
                    rig.set_many(config_manager, at_camera(li, eye), "history.action.solid_light_place");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::RenderMode;

    /// The text a panel draws, from its second frame.
    fn drawn(config_manager: &mut ConfigManager, draw: &mut dyn FnMut(&mut egui::Ui, &mut ConfigManager)) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        for _ in 0..2 {
            let full = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| draw(ui, config_manager));
            });
            out.clear();
            for s in &full.shapes {
                walk(&s.shape, &mut out);
            }
        }
        out
    }

    fn lighting_text(config: &FractalConfig) -> Vec<String> {
        let mut m = ConfigManager::new(config.clone());
        drawn(&mut m, &mut |ui, m| render_lighting_panel_content(ui, m))
    }

    #[cfg(feature = "engine-escape")]
    fn escape_text(config: &FractalConfig) -> Vec<String> {
        let mut m = ConfigManager::new(config.clone());
        drawn(&mut m, &mut |ui, m| super::super::escape_panel::render_escape_content(ui, m, &mut None, None, &mut None, &mut false))
    }

    fn view_text(config: &FractalConfig) -> Vec<String> {
        let mut m = ConfigManager::new(config.clone());
        let flame = config.flame.clone();
        let deep = super::super::DeepZoom::default();
        drawn(&mut m, &mut |ui, m| {
            let (mut toggle, mut open) = (false, false);
            super::super::view::render_view_content(ui, m, &flame, false, &mut toggle, &deep, None, &mut open)
        })
    }

    /// Not a test: what the panel draws in each view, for review.
    #[test]
    #[ignore]
    fn print_the_panel() {
        let mut views = Vec::new();
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.formula = "mandelbulb_solid".to_string();
        views.push(("solid", c.clone()));
        c.escape.formula = "mandelbrot".to_string();
        c.escape.terrain.enabled = true;
        c.escape.terrain.interior = crate::config::escape::TerrainInterior::Lake;
        views.push(("terrain", c));
        let mut f = FractalConfig::default();
        f.render_mode = RenderMode::ThreeD;
        f.solid_shading.shading_strength = 0.6;
        views.push(("flame", f));
        for (name, c) in views {
            println!("==== {name}: {:?}", lighting_text(&c));
        }
    }

    fn has(text: &[String], what: &str) -> bool {
        text.iter().any(|t| t == what)
    }

    /// A solid's light is drawn in the Lighting & Material panel alone
    /// (3d-look-and-tools P3): its Shadows, Shadow Sharpness and Occlusion
    /// Reach -- formula parameters -- there and not among the Escape
    /// panel's parameters, and the rig no longer in the View panel.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn a_solids_light_is_drawn_in_the_lighting_panel_alone() {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.formula = "mandelbulb_solid".to_string();
        let lighting = lighting_text(&c);
        for want in ["Shadow Sharpness", "Occlusion Reach"] {
            assert!(has(&lighting, want), "the panel lacks {want}: {lighting:?}");
        }
        for section in ["lighting.rendering", "lighting.sky", "lighting.lights", "lighting.shading", "lighting.shadows", "lighting.material"] {
            assert!(has(&lighting, &t!(section)), "the panel lacks its {section}");
        }
        assert!(has(&lighting, &t!("view.shading_strength")), "an untouched rig's rows show");
        let escape = escape_text(&c);
        for gone in ["Shadow Sharpness", "Occlusion Reach"] {
            assert!(!has(&escape, gone), "the Escape panel still draws {gone}");
        }
        assert!(has(&escape, &t!("escape_panel.lighting_elsewhere")), "and says where they went");
        let view = view_text(&c);
        assert!(!has(&view, &t!("view.shading_strength")), "the View panel still draws the rig");
        assert!(has(&view, &t!("view.open_lighting_panel")));
        // Path traced: occlusion, the lit tier's, hides.
        c.escape.solid_tier = crate::config::escape::RenderTier::PathTraced;
        let lighting = lighting_text(&c);
        assert!(!has(&lighting, "Occlusion Reach") && !has(&lighting, &t!("view.occlusion_strength")));
        assert!(has(&lighting, "Shadow Sharpness"));
    }

    /// A terrain's surface texture and its lake are the panel's Surface,
    /// not the Escape panel's.
    #[cfg(all(feature = "engine-escape", feature = "terrain"))]
    #[test]
    fn a_terrains_surface_is_drawn_in_the_lighting_panel() {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.terrain.enabled = true;
        c.escape.terrain.interior = crate::config::escape::TerrainInterior::Lake;
        c.escape.shading.enabled = true;
        let lighting = lighting_text(&c);
        for key in ["lighting.surface", "escape_panel.shading_texture", "escape_panel.terrain_lake_tint", "escape_panel.terrain_shadow", "escape_panel.terrain_occlusion"] {
            assert!(has(&lighting, &t!(key)), "the panel lacks {key}");
        }
        let escape = escape_text(&c);
        for key in ["escape_panel.shading_texture", "escape_panel.terrain_lake_tint", "escape_panel.terrain_lake_roughness"] {
            assert!(!has(&escape, &t!(key)), "the Escape panel still draws {key}");
        }
        assert!(has(&escape, &t!("escape_panel.lake_elsewhere")) && has(&escape, &t!("escape_panel.texture_elsewhere")));
        // A plane's relief keeps its texture in the Escape panel.
        c.escape.terrain.enabled = false;
        assert!(has(&escape_text(&c), &t!("escape_panel.shading_texture")));
    }

    /// An untouched rig's first change takes it over -- strength 1, no
    /// ambient, the change -- in one undo step; the coat is not the rig and
    /// leaves it untouched; a terrain's sun stays where it was; a 3D
    /// flame's untouched rig is no lighting, and is not taken over.
    #[test]
    fn the_first_change_takes_an_untouched_rig_over() {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.formula = "mandelbulb_solid".to_string();
        let view = crate::camera::gesture::view_kind(&c);
        let mut m = ConfigManager::new(c.clone());
        let _ = m.update_param(ConfigPath::SolidGloss, 0.3f32.into());
        assert!(m.active_config().solid_shading.rig_untouched(), "the coat");
        let rig = Rig::of(m.active_config(), view);
        assert!(rig.untouched());
        assert_eq!((rig.shown.shading_strength, rig.shown.ambient), (1.0, 0.0), "the rows show what the change makes it");
        rig.set(&mut m, ConfigPath::SolidDiffuse, 0.5f32.into());
        let s = &m.active_config().solid_shading;
        assert_eq!((s.shading_strength, s.ambient, s.diffuse), (1.0, 0.0, 0.5));
        m.undo().expect("undo");
        assert!(m.active_config().solid_shading.rig_untouched(), "one undo step");
        // Touched: a change is only itself.
        let mut touched = c.clone();
        touched.solid_shading.shading_strength = 0.5;
        let mut m = ConfigManager::new(touched.clone());
        let rig = Rig::of(&touched, view);
        assert!(!rig.untouched());
        rig.set(&mut m, ConfigPath::SolidDiffuse, 0.5f32.into());
        assert_eq!((m.active_config().solid_shading.shading_strength, m.active_config().solid_shading.ambient), (0.5, 0.25));
        // A terrain's sun.
        #[cfg(feature = "terrain")]
        {
            let mut t = FractalConfig::default();
            t.render_mode = RenderMode::Escape;
            t.escape.terrain.enabled = true;
            let mut m = ConfigManager::new(t.clone());
            Rig::of(&t, ViewKind::EscapeTerrain).set(&mut m, ConfigPath::SolidLightParam { index: 0, param: "intensity".into() }, 0.8f32.into());
            let l = m.active_config().solid_shading.lights[0];
            assert_eq!((l.azimuth, l.elevation, l.intensity), (crate::escape::terrain::DEFAULT_SUN.0, crate::escape::terrain::DEFAULT_SUN.1, 0.8));
        }
        // A 3D flame.
        let mut f = FractalConfig::default();
        f.render_mode = RenderMode::ThreeD;
        let rig = Rig::of(&f, ViewKind::Flame3d);
        assert!(!rig.untouched() && !rig.in_use);
    }
}
