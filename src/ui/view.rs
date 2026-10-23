use crate::scene::transforms::{Flame, PostSymmetryType, RenderMode};
use crate::config::{ConfigManager, ConfigPath, FractalConfig};
use rust_i18n::t;

/// Render view controls content (for docking panels)
pub fn render_view_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    _flame: &Flame,
    fly_mode_active: bool,
    fly_mode_toggle_requested: &mut bool,
    deep_zoom: &super::DeepZoom,
    escape_aa: Option<super::EscapeAa>,
    open_lighting_panel: &mut bool,
) {
    // Clone config to avoid borrow conflicts
    let config = config_manager.active_config().clone();

    // One View panel for every mode (camera-unification C5): the camera
    // the viewport shows, then what that mode draws the picture with,
    // then -- for a flame -- its own sections below.
    use crate::camera::gesture::{self, ViewKind};
    let kind = gesture::view_kind(&config);

    // ── The picture's size, every mode's: what the exports render at,
    // and the shape the viewport can letterbox to.
    egui::CollapsingHeader::new(t!("view.output_size").as_ref())
        .default_open(true)
        .show(ui, |ui| super::view_controls::output_size(ui, config_manager));

    // ── Camera: the 2D views, and a 3D flame's picture after its
    // projection (its pan, zoom and roll).
    if !kind.is_3d() || kind == ViewKind::Flame3d {
        egui::CollapsingHeader::new(t!("view.camera").as_ref())
            .default_open(true)
            .show(ui, |ui| super::view_controls::camera_2d(ui, config_manager));
    }
    // ── The 3D camera, every 3D view's: position, target, angles,
    // projection, fly mode.
    if kind.is_3d() {
        egui::CollapsingHeader::new(t!("view.camera_3d").as_ref())
            .default_open(true)
            .show(ui, |ui| {
                super::view_controls::camera_3d(ui, config_manager, fly_mode_active, fly_mode_toggle_requested)
            });
    }

    // ── What each engine draws the picture with.
    match kind {
        #[cfg(feature = "engine-escape")]
        ViewKind::EscapePlane | ViewKind::Solid | ViewKind::EscapeTerrain => {
            // A lens bends a plane's pixels and a solid's rays; a
            // terrain's camera has none.
            if kind != ViewKind::EscapeTerrain {
                super::escape_panel::show_lens_section(ui, config_manager);
            }
            egui::CollapsingHeader::new(t!("view.image").as_ref())
                .default_open(true)
                .show(ui, |ui| super::view_controls::escape_image(ui, config_manager, escape_aa));
        }
        #[cfg(feature = "engine-sim")]
        ViewKind::Sim2d => {
            egui::CollapsingHeader::new(t!("view.image").as_ref())
                .default_open(true)
                .show(ui, |ui| super::view_controls::sim_image(ui, config_manager));
        }
        _ => {}
    }

    // ── Every 3D view's: what lies between the eye and the picture, and
    // its focus. How it is lit and what it is made of is the Lighting &
    // Material panel's (3d-look-and-tools P3).
    if kind.is_3d() {
        egui::CollapsingHeader::new(t!("view.atmosphere").as_ref())
            .default_open(false)
            .show(ui, |ui| super::view_controls::atmosphere_3d(ui, config_manager));
        egui::CollapsingHeader::new(t!("view.depth_of_field").as_ref())
            .default_open(false)
            .show(ui, |ui| super::view_controls::depth_of_field_3d(ui, config_manager));
        if ui
            .button(t!("view.open_lighting_panel").as_ref())
            .on_hover_text(t!("view.open_lighting_panel_tip"))
            .clicked()
        {
            *open_lighting_panel = true;
        }
    }
    if !matches!(kind, ViewKind::Flame2d | ViewKind::Flame3d) {
        return;
    }

    // ── Deep zoom ────────────────────────────────────────────────
    // Both controls are REQUESTS; the renderer decides per view and
    // the readout says what it decided. A ticked box that silently
    // does nothing is the failure mode worth designing against here,
    // because declining is the common case.
    if crate::ui::visibility::control(
        crate::ui::visibility::Control::DeepZoom,
        config.render_mode,
        config.tonemap_mode,
    )
    .is_show()
    {
        egui::CollapsingHeader::new(t!("view.deep_zoom").as_ref())
            .default_open(false)
            .show(ui, |ui| {
                let mut auto_exposure = config.auto_exposure;
                if ui
                    .checkbox(&mut auto_exposure, t!("view.auto_exposure").as_ref())
                    .on_hover_text(t!("view.tooltip_auto_exposure"))
                    .changed()
                {
                    let _ =
                        config_manager.update_param(ConfigPath::AutoExposure, auto_exposure.into());
                }
                if config.auto_exposure {
                    // The measured number, not the request. Shown as a
                    // percentage because that is what it is: the share
                    // of plotted samples the frame is holding.
                    ui.label(t!(
                        "view.coverage_reading",
                        percent = format!("{:.3}", deep_zoom.coverage * 100.0)
                    ));
                }

                ui.add_space(4.0);

                // Focused Rendering (cylinder targeting): the switch and
                // what it is doing, shared with the Paths panel.
                super::paths_panel::focused_rendering(ui, config_manager, &config, deep_zoom);
            });
    }

    ui.separator();

    // The flame's render mode: 2D or 3D.
    ui.label(t!("view.render_mode")).on_hover_text(t!("view.tooltip_render_mode"));
    ui.horizontal(|ui| {
        // Through the shared helper, so this control resets the tone
        // mapping on the same terms every other one does.
        for (mode, label, tip) in [
            (RenderMode::TwoD, t!("view.mode_2d"), t!("view.tooltip_mode_2d")),
            (RenderMode::ThreeD, t!("view.mode_3d"), t!("view.tooltip_mode_3d")),
        ] {
            if ui.selectable_label(config.render_mode == mode, label.as_ref()).on_hover_text(tip.as_ref()).clicked() {
                if let Err(e) = super::render_mode::switch_render_mode(config_manager, mode) {
                    log::error!("Failed to update render mode: {}", e);
                }
            }
        }
    });

    if kind == ViewKind::Flame3d {
        // Preserve Z — JWildfire's `preserve_z` flag. Defaults to
        // off (Apo/JWF default) so flames with variations that scale
        // Z by >1 (e.g. spherical at high weight) don't explode and
        // poison the camera transform via `0·∞ = NaN`.
        let mut preserve_z = config.preserve_z;
        let response = ui.checkbox(&mut preserve_z, t!("view.preserve_z").as_ref())
            .on_hover_text(t!("view.tooltip_preserve_z"));
        if response.changed() {
            let _ = config_manager.update_param(
                ConfigPath::PreserveZ,
                preserve_z.into(),
            );
        }
    }

    ui.separator();

    render_post_symmetry_section(ui, config_manager, &config);
}

/// Post-symmetry section. Type dropdown + the per-mode controls
/// gated by which axis-vs-Point is active. JWildfire-compat: the
/// values round-trip through `flame.post_symmetry` and the GPU
/// HAS_POST_SYMMETRY gate updates automatically when the type
/// changes (forces a shader rebuild via ShaderConstants).
fn render_post_symmetry_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    config: &FractalConfig,
) {
    use crate::config::slider::LazyUndoUi;
    let _ = config_manager;

    let ps = &config.flame.post_symmetry;
    let current_ty = ps.ty;

    ui.label(t!("view.post_symmetry")).on_hover_text(t!("view.tooltip_post_symmetry"));

    // Type dropdown.
    let type_label = match current_ty {
        PostSymmetryType::None => t!("view.post_symmetry_none"),
        PostSymmetryType::XAxis => t!("view.post_symmetry_x_axis"),
        PostSymmetryType::YAxis => t!("view.post_symmetry_y_axis"),
        PostSymmetryType::Point => t!("view.post_symmetry_point"),
    };
    egui::ComboBox::from_label(t!("view.post_symmetry_type").as_ref())
        .selected_text(type_label)
        .show_ui(ui, |ui| {
            for (ty, lbl) in [
                (PostSymmetryType::None, t!("view.post_symmetry_none")),
                (PostSymmetryType::XAxis, t!("view.post_symmetry_x_axis")),
                (PostSymmetryType::YAxis, t!("view.post_symmetry_y_axis")),
                (PostSymmetryType::Point, t!("view.post_symmetry_point")),
            ] {
                if ui.selectable_label(current_ty == ty, lbl).clicked() && current_ty != ty {
                    let _ = config_manager.update_param(
                        ConfigPath::PostSymmetryType,
                        (ty.as_u32() as i32).into(),
                    );
                }
            }
        });

    if current_ty == PostSymmetryType::None {
        return;
    }

    // Center always shown (relevant to every non-None mode).
    ui.horizontal(|ui| {
        ui.label(t!("view.post_symmetry_center"));
        let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryCenterX, 0.01, "X");
        let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryCenterY, 0.01, "Y");
    });

    match current_ty {
        PostSymmetryType::XAxis | PostSymmetryType::YAxis => {
            // Axis modes: distance pans the mirror along the axis,
            // rotation pre-rotates around the center.
            ui.horizontal(|ui| {
                ui.label(t!("view.post_symmetry_distance"))
                    .on_hover_text(t!("view.tooltip_post_symmetry_distance"));
                let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryDistance, 0.01, "");
            });
            ui.horizontal(|ui| {
                ui.label(t!("view.post_symmetry_rotation"))
                    .on_hover_text(t!("view.tooltip_post_symmetry_rotation"));
                let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryRotation, 0.5, "°");
            });
        }
        PostSymmetryType::Point => {
            // Point mode: just order. Distance/rotation are ignored.
            ui.horizontal(|ui| {
                ui.label(t!("view.post_symmetry_order"))
                    .on_hover_text(t!("view.tooltip_post_symmetry_order"));
                let mut order = ps.order as i32;
                let response = ui.add(egui::Slider::new(&mut order, 1..=32));
                super::vkb_sync_full(ui, &response, &order.to_string(), "integer", Some(1.0), Some(32.0));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::PostSymmetryOrder,
                        order.into(),
                    );
                }
            });
        }
        PostSymmetryType::None => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_dock::egui;

    /// Lay the View panel out for real, in every view it serves
    /// (camera-unification C5): a panel that compiles can still panic at
    /// layout -- a duplicate widget id, an empty slider range -- and the
    /// visual suite renders fractals, not panels. Two frames, so the
    /// second takes egui's "widget already exists" path.
    fn lay_out(config: FractalConfig) {
        // As it ships, and with every folded section's body drawn: lit
        // (the lights' rows show) and path traced (the lens shows).
        let mut lit = config.clone();
        lit.solid_shading.shading_strength = 0.5;
        lit.solid_shading.lights[1].enabled = true;
        // A light of each kind (lights-in-the-fractal, L6): the kind
        // picker and each kind's controls, offered or said to be off.
        lit.solid_shading.lights[2].enabled = true;
        lit.solid_shading.lights[2].kind = crate::config::LightKind::Point;
        lit.solid_shading.lights[2].position = [0.1, 0.2, 0.3];
        lit.solid_shading.lights[3].enabled = true;
        lit.solid_shading.lights[3].kind = crate::config::LightKind::Headlamp;
        lit.escape.solid_tier = crate::config::escape::RenderTier::PathTraced;
        lit.escape.terrain.tier = crate::config::escape::RenderTier::PathTraced;
        lit.sim.terrain.tier = crate::config::escape::RenderTier::PathTraced;
        for config in [config, lit] {
            let ctx = egui::Context::default();
            let mut manager = ConfigManager::new(config);
            let flame = manager.active_config().flame.clone();
            let deep = super::super::DeepZoom::default();
            let mut toggle = false;
            for _ in 0..2 {
                let _ = ctx.run_ui(Default::default(), |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        let mut open = false;
                        render_view_content(ui, &mut manager, &flame, false, &mut toggle, &deep, None, &mut open);
                        ui.separator();
                        use super::super::view_controls as vc;
                        vc::atmosphere_3d(ui, &mut manager);
                        vc::depth_of_field_3d(ui, &mut manager);
                        super::super::lighting_panel::render_lighting_panel_content(ui, &mut manager);
                    });
                });
            }
        }
    }

    /// The leaves of a config's JSON that differ between two configs, by
    /// their dotted paths.
    fn differences(a: &serde_json::Value, b: &serde_json::Value, at: &str, out: &mut Vec<String>) {
        match (a, b) {
            (serde_json::Value::Object(x), serde_json::Value::Object(y)) => {
                for k in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                    let n = serde_json::Value::Null;
                    differences(x.get(k).unwrap_or(&n), y.get(k).unwrap_or(&n), &format!("{at}.{k}"), out);
                }
            }
            (serde_json::Value::Array(x), serde_json::Value::Array(y)) if x.len() == y.len() => {
                for (i, (p, q)) in x.iter().zip(y).enumerate() {
                    differences(p, q, &format!("{at}[{i}]"), out);
                }
            }
            _ if a != b => out.push(format!("{at}: {a} -> {b}")),
            _ => {}
        }
    }

    /// Showing the View panel writes nothing (user-reported 2026-10-21:
    /// switching to it redrew 3D scenes -- its field of view slider
    /// rounded the default 40.107 degrees to the 40.1 it shows), nor does
    /// the Lighting panel: every bundled preset in a 3D view,
    /// each view's default, and a solid under a sky whose horizon and
    /// stars are past their sliders' ranges, drawn for two frames -- no
    /// change to the config, nothing for the renderer to do.
    #[test]
    fn showing_the_view_panel_writes_nothing() {
        use crate::scene::transforms::RenderMode;
        let mut configs: Vec<(String, FractalConfig)> = crate::resources::load_presets_with_fallback()
            .into_iter()
            .filter(|c| c.render_mode != RenderMode::TwoD)
            .map(|c| (c.flame.name.clone(), c))
            .collect();
        let mut d = FractalConfig::default();
        d.render_mode = RenderMode::ThreeD;
        configs.push(("default 3D".into(), d));
        #[cfg(feature = "engine-escape")]
        {
            let mut e = FractalConfig::default();
            e.render_mode = RenderMode::Escape;
            e.escape.formula = "quaternion_julia_solid".to_string();
            configs.push(("default solid".into(), e.clone()));
            e.escape.solid_tier = crate::config::escape::RenderTier::PathTraced;
            e.escape.path.environment_map = "sunflowers".to_string();
            e.escape.path.environment_horizon = 60.0;
            e.escape.path.environment_stars = 8.0;
            configs.push(("solid under a sky past its sliders".into(), e));
            #[cfg(feature = "terrain")]
            {
                let mut t = FractalConfig::default();
                t.render_mode = RenderMode::Escape;
                t.escape.terrain.enabled = true;
                configs.push(("default terrain".into(), t));
            }
        }
        #[cfg(feature = "engine-sim")]
        {
            let mut s = FractalConfig::default();
            s.render_mode = RenderMode::Simulation;
            s.sim.terrain.enabled = true;
            configs.push(("default simulation terrain".into(), s));
        }
        let mut offenders = Vec::new();
        for (name, config) in configs {
            let before = serde_json::to_value(&config).expect("serializes");
            let ctx = egui::Context::default();
            let mut manager = ConfigManager::new(config);
            manager.clear_pending_actions();
            let flame = manager.active_config().flame.clone();
            let deep = super::super::DeepZoom::default();
            let mut toggle = false;
            for _ in 0..2 {
                let _ = ctx.run_ui(Default::default(), |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        let mut open = false;
                        render_view_content(ui, &mut manager, &flame, false, &mut toggle, &deep, None, &mut open);
                        super::super::lighting_panel::render_lighting_panel_content(ui, &mut manager);
                    });
                });
            }
            let after = serde_json::to_value(manager.active_config()).expect("serializes");
            let mut diff = Vec::new();
            differences(&before, &after, "", &mut diff);
            let acted = format!("{:?}", manager.get_pending_actions()) != format!("{:?}", crate::config::UpdateAction::none());
            if !diff.is_empty() || acted || manager.history_len() > 0 {
                offenders.push(format!("{name}: {diff:?}, history {}, actions {:?}", manager.history_len(), manager.get_pending_actions()));
            }
        }
        assert!(offenders.is_empty(), "showing the View and Lighting panels wrote:
{}", offenders.join("
"));
    }

    /// A letterboxed viewport is the largest rect of the picture's shape in
    /// its panel, centred, in whole pixels: bars beside a wide panel, above
    /// and below a tall one, none when the shapes agree.
    #[test]
    fn a_letterbox_is_the_pictures_shape_centred() {
        use super::super::view_controls::letterbox_rect;
        let panel = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(1000.0, 400.0));
        let r = letterbox_rect(panel, (1920, 1080));
        assert_eq!(r.size(), egui::vec2(711.0, 400.0), "bars beside: {r:?}");
        assert!((r.center() - panel.center()).length() < 0.51);
        let tall = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(500.0, 900.0));
        let r = letterbox_rect(tall, (1920, 1080));
        assert_eq!(r.size(), egui::vec2(500.0, 281.0), "bars above and below: {r:?}");
        let same = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0));
        assert_eq!(letterbox_rect(same, (1920, 1080)), same);
    }

    /// The Output Size edits the picture's size through the history: a side
    /// within the range, clamped to it, and a preset both sides in one step
    /// that undo takes back whole.
    #[test]
    fn the_output_size_is_the_pictures_and_undoable() {
        let mut m = ConfigManager::new(FractalConfig::default());
        let _ = m.update_param(ConfigPath::ImageWidth, 1000u32.into());
        assert_eq!(m.active_config().image_size, (1000, 1080));
        let _ = m.update_param(ConfigPath::ImageHeight, 1u32.into());
        assert_eq!(m.active_config().image_size.1, crate::config::MIN_IMAGE_DIM);
        let before = m.active_config().image_size;
        let _ = m.update_batch(
            vec![(ConfigPath::ImageWidth, 3840u32.into()), (ConfigPath::ImageHeight, 2160u32.into())],
            "history.action.output_size".to_string(),
        );
        assert_eq!(m.active_config().image_size, (3840, 2160));
        m.undo().expect("an undo");
        assert_eq!(m.active_config().image_size, before);
    }

    #[test]
    fn the_view_panel_lays_out_in_every_view() {
        use crate::scene::transforms::RenderMode;
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::TwoD;
        lay_out(c.clone());
        c.render_mode = RenderMode::ThreeD;
        lay_out(c.clone());
        #[cfg(feature = "engine-escape")]
        {
            let mut e = FractalConfig::default();
            e.render_mode = RenderMode::Escape;
            lay_out(e.clone());
            e.escape.formula = "quaternion_julia_solid".to_string();
            lay_out(e.clone());
            #[cfg(feature = "terrain")]
            {
                let mut t = FractalConfig::default();
                t.render_mode = RenderMode::Escape;
                t.escape.terrain.enabled = true;
                lay_out(t);
            }
        }
        #[cfg(feature = "engine-sim")]
        {
            let mut s = FractalConfig::default();
            s.render_mode = RenderMode::Simulation;
            lay_out(s.clone());
            s.sim.terrain.enabled = true;
            lay_out(s);
        }
    }
}
