//! The View controls every mode shares (camera-unification C5): one
//! widget for an angle, and each camera's own controls, so the View
//! panel's 2D and 3D layouts -- and any panel that shows a camera -- draw
//! them alike.

use crate::config::{ConfigManager, ConfigPath, ConfigValue};
use rust_i18n::t;

/// An angle stored in radians, shown in degrees on a −180..180 slider
/// that takes a typed value beyond the range (the user's rule: no
/// radians in the UI; unlocked when you want to go round). The new value
/// in radians when it changed.
pub fn angle_slider(ui: &mut egui::Ui, radians: f32, label: &str) -> Option<f32> {
    let mut deg = radians.to_degrees();
    let changed = ui
        .add(
            egui::Slider::new(&mut deg, -180.0..=180.0)
                .text(label)
                .suffix("°")
                .fixed_decimals(1)
                .clamping(egui::SliderClamping::Never),
        )
        .changed();
    changed.then(|| deg.to_radians())
}

/// A zoom shown as a magnification on a logarithmic slider, typed values
/// beyond the range allowed.
fn zoom_slider(ui: &mut egui::Ui, zoom: f32, label: &str, range: std::ops::RangeInclusive<f32>) -> Option<f32> {
    let mut z = zoom;
    let changed = ui
        .add(
            egui::Slider::new(&mut z, range)
                .logarithmic(true)
                .text(label)
                .suffix("×")
                .clamping(egui::SliderClamping::Never),
        )
        .changed();
    changed.then_some(z)
}

/// The 2D camera, the same section in every 2D view (camera-unification
/// C5) -- the flame's, the escape plane's, a simulation's, and a 3D
/// flame's picture after its projection: the zoom, the centre, the turn,
/// an arrow pad and Reset. The buttons go through `camera::gesture`, so
/// they move the picture as the keys and the mouse do; the fields are
/// each camera's own, in its own units (the escape centre as exact
/// decimals).
pub fn camera_2d(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    use crate::camera::gesture::{self, ViewKind};
    let config = config_manager.active_config().clone();
    let kind = gesture::view_kind(&config);
    let apply = |config_manager: &mut ConfigManager, edit: Option<crate::camera::CameraEdit>| {
        if let Some(edit) = edit {
            edit.apply(config_manager);
        }
    };

    // ---- Zoom ----
    ui.horizontal(|ui| {
        if ui.button("−").on_hover_text(t!("view.zoom_out").as_ref()).clicked() {
            apply(config_manager, gesture::zoom(&config, 1.0 / 1.5, None, [1000.0, 1000.0]));
        }
        match kind {
            ViewKind::EscapePlane => {
                // Base 10, the unit deep-zoom tools report, while the
                // engine keeps base 2 (`EscapeConfig::zoom_log10`): a
                // magnification past 1e300 is no slider's.
                let mut z10 = config.escape.zoom_log10();
                let resp = ui
                    .add(egui::DragValue::new(&mut z10).speed(0.02 * std::f64::consts::LOG10_2).max_decimals(4).prefix("10^"))
                    .on_hover_text(t!("escape_panel.tooltip_zoom_log10"));
                if resp.changed() {
                    let z2 = crate::config::escape::EscapeConfig::log10_to_log2(z10);
                    let _ = config_manager.update_param(ConfigPath::EscapeZoomLog2, (z2 as f32).into());
                }
                ui.label(egui::RichText::new(super::escape_panel::magnification_label(config.escape.zoom_log10())).weak());
            }
            #[cfg(feature = "engine-sim")]
            ViewKind::Sim2d => {
                if let Some(z) = zoom_slider(ui, config.sim.view.zoom, t!("view.zoom").as_ref(), 0.1..=64.0) {
                    let _ = config_manager.update_param(ConfigPath::SimViewZoom, z.into());
                }
            }
            _ => {
                if let Some(z) = zoom_slider(ui, config.zoom, t!("view.zoom").as_ref(), 0.05..=1.0e4) {
                    let _ = config_manager.update_param(ConfigPath::Zoom, z.into());
                }
            }
        }
        if ui.button("+").on_hover_text(t!("view.zoom_in").as_ref()).clicked() {
            apply(config_manager, gesture::zoom(&config, 1.5, None, [1000.0, 1000.0]));
        }
    })
    .response
    .on_hover_text(t!("view.tooltip_zoom"));

    // ---- Centre ----
    match kind {
        ViewKind::EscapePlane => {
            for (label, value, path) in [
                ("re", &config.escape.center_re, ConfigPath::EscapeCenterRe),
                ("im", &config.escape.center_im, ConfigPath::EscapeCenterIm),
            ] {
                ui.horizontal(|ui| {
                    ui.label(format!("{label}:"));
                    // An exact decimal string (deep-zoom ready); an
                    // unparseable intermediate state falls back to the
                    // default centre at render time and corrects as you
                    // type.
                    let mut text = value.clone();
                    if ui.text_edit_singleline(&mut text).changed() {
                        let _ = config_manager.update_param(path.clone(), ConfigValue::String(text));
                    }
                });
            }
        }
        #[cfg(feature = "engine-sim")]
        ViewKind::Sim2d => {
            let v = config.sim.view.clone();
            ui.horizontal(|ui| {
                ui.label(t!("view.center").as_ref());
                let mut x = v.center_x;
                if ui.add(egui::DragValue::new(&mut x).speed(0.002).max_decimals(4).prefix("x ")).changed() {
                    let _ = config_manager.update_param(ConfigPath::SimViewCenterX, x.into());
                }
                let mut y = v.center_y;
                if ui.add(egui::DragValue::new(&mut y).speed(0.002).max_decimals(4).prefix("y ")).changed() {
                    let _ = config_manager.update_param(ConfigPath::SimViewCenterY, y.into());
                }
            });
        }
        _ => {
            ui.horizontal(|ui| {
                ui.label(t!("view.center").as_ref()).on_hover_text(t!("view.tooltip_pan"));
                let step = 0.001 / config.zoom.max(1.0e-6) as f64;
                let (mut x, mut y) = (config.pan_x, config.pan_y);
                let cx = ui.add(egui::DragValue::new(&mut x).speed(step).max_decimals(7).prefix("x "));
                let cy = ui.add(egui::DragValue::new(&mut y).speed(step).max_decimals(7).prefix("y "));
                if cx.changed() || cy.changed() {
                    let _ = config_manager.update_param(ConfigPath::Pan, (x, y).into());
                }
            });
        }
    }

    // ---- Turn ----
    let (rotation, path) = match kind {
        ViewKind::EscapePlane => (config.escape.rotation, ConfigPath::EscapeRotation),
        #[cfg(feature = "engine-sim")]
        ViewKind::Sim2d => (config.sim.view.rotation, ConfigPath::SimViewRotation),
        _ => (config.rotation, ConfigPath::Rotation),
    };
    if let Some(r) = angle_slider(ui, rotation, t!("view.rotation").as_ref()) {
        let _ = config_manager.update_param(path, r.into());
    }

    // ---- Arrows and Reset ----
    // A press moves the picture 5% of the view, as an arrow key does.
    ui.horizontal(|ui| {
        let step = 50.0;
        for (label, drag) in [("◀", [step, 0.0]), ("▲", [0.0, step]), ("▼", [0.0, -step]), ("▶", [-step, 0.0])] {
            if ui.small_button(label).clicked() {
                apply(config_manager, gesture::pan(&config, drag, [1000.0, 1000.0]));
            }
        }
        ui.separator();
        if ui.button(t!("view.reset").as_ref()).clicked() {
            apply(config_manager, gesture::reset(&config));
        }
    });
}

/// How the escape picture is drawn: its antialiasing, what the view is
/// drawn at when that is not the setting, and how the samples combine.
#[cfg(feature = "engine-escape")]
pub fn escape_image(ui: &mut egui::Ui, config_manager: &mut ConfigManager, viewport_aa: Option<super::EscapeAa>) {
    use crate::config::escape::DownsampleMode;
    let esc = config_manager.active_config().escape.clone();
    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.supersample"));
        let max = crate::escape::renderer::MAX_SUPERSAMPLE;
        let current = esc.supersample.clamp(1, max);
        let label_for = |n: u32| match n {
            1 => t!("escape_panel.supersample_off").to_string(),
            n => format!("{n}\u{00d7} ({} samples)", n * n),
        };
        egui::ComboBox::from_id_salt("escape_supersample")
            .selected_text(label_for(current))
            .show_ui(ui, |ui| {
                // Whole factors only, and not every one: 5x and 7x
                // cost more than 4x and 6x for no visible gain.
                for n in [1u32, 2, 3, 4, 6, 8] {
                    if n > max {
                        continue;
                    }
                    if ui.selectable_label(n == current, label_for(n)).clicked() && n != current {
                        let _ = config_manager.update_param(ConfigPath::EscapeSupersample, ConfigValue::UInt(n));
                    }
                }
            })
            .response
            .on_hover_text(t!("escape_panel.tooltip_supersample"));
    });
    // What the view is actually drawn at, when that is not the setting:
    // the frame governor during playback, or the view's size.
    if let Some(aa) = viewport_aa.filter(|aa| aa.in_use != aa.requested) {
        let (text, tip) = match aa.reason {
            Some(super::AaReason::Governor) => (
                t!("escape_panel.aa_in_use_governor", used = aa.in_use),
                t!("escape_panel.aa_in_use_governor_tip"),
            ),
            _ => (t!("escape_panel.aa_in_use_size", used = aa.in_use), t!("escape_panel.aa_in_use_size_tip")),
        };
        ui.label(egui::RichText::new(text).small().weak()).on_hover_text(tip);
    }
    // How those samples are combined. Only meaningful when there is
    // more than one of them.
    if esc.supersample > 1 {
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.downsample"));
            let cur = esc.downsample;
            let label_for = |m: DownsampleMode| match m {
                DownsampleMode::Box => t!("escape_panel.downsample_box"),
                DownsampleMode::Perceptual => t!("escape_panel.downsample_perceptual"),
                DownsampleMode::Vivid => t!("escape_panel.downsample_vivid"),
            };
            egui::ComboBox::from_id_salt("escape_downsample")
                .selected_text(label_for(cur))
                .show_ui(ui, |ui| {
                    for m in [DownsampleMode::Box, DownsampleMode::Perceptual, DownsampleMode::Vivid] {
                        if ui.selectable_label(cur == m, label_for(m).as_ref()).clicked() && cur != m {
                            let _ = config_manager
                                .update_param(ConfigPath::EscapeDownsample, ConfigValue::String(m.as_str().to_string()));
                        }
                    }
                })
                .response
                .on_hover_text(t!("escape_panel.tooltip_downsample"));
        });
    }
}

/// How a simulation's grid is shown: the resolve filters either way of
/// the fit, the fit itself, and tiling a periodic field.
#[cfg(feature = "engine-sim")]
pub fn sim_image(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    use crate::config::sim::{SimDownscale, SimFit, SimUpscale};
    let sim = config_manager.active_config().sim.clone();
    ui.horizontal(|ui| {
        ui.label(t!("sim_panel.upscale").as_ref());
        egui::ComboBox::from_id_salt("sim_upscale")
            .selected_text(sim.upscale.name())
            .show_ui(ui, |ui| {
                for n in SimUpscale::NAMES {
                    if ui.selectable_label(sim.upscale.name() == *n, *n).clicked() {
                        let _ = config_manager.update_param(ConfigPath::SimUpscale, (*n).to_string().into());
                    }
                }
            })
            .response
            .on_hover_text(t!("sim_panel.upscale_tip"));
        ui.label(t!("sim_panel.fit").as_ref());
        egui::ComboBox::from_id_salt("sim_fit")
            .selected_text(sim.fit.name())
            .show_ui(ui, |ui| {
                for n in SimFit::NAMES {
                    if ui.selectable_label(sim.fit.name() == *n, *n).clicked() {
                        let _ = config_manager.update_param(ConfigPath::SimFit, (*n).to_string().into());
                    }
                }
            })
            .response
            .on_hover_text(t!("sim_panel.fit_tip"));
        ui.label(t!("sim_panel.downscale").as_ref());
        egui::ComboBox::from_id_salt("sim_downscale")
            .selected_text(sim.downscale.name())
            .show_ui(ui, |ui| {
                for n in SimDownscale::NAMES {
                    if ui.selectable_label(sim.downscale.name() == *n, *n).clicked() {
                        let _ = config_manager.update_param(ConfigPath::SimDownscale, (*n).to_string().into());
                    }
                }
            });
    });
    let periodic = sim.boundary == crate::config::sim::SimBoundary::Periodic;
    let mut tile = sim.view.tile;
    let hint = if periodic { t!("view.tile_tip") } else { t!("view.tile_needs_periodic") };
    if ui
        .add_enabled(periodic, egui::Checkbox::new(&mut tile, t!("view.tile").as_ref()))
        .on_hover_text(hint.as_ref())
        .on_disabled_hover_text(hint.as_ref())
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::SimViewTile, tile.into());
    }
}

/// One camera angle: [`angle_slider`] with its tooltip -- which says
/// where that camera's zero is, since each keeps its own (the panel shows
/// the number the file stores).
fn angle_row(ui: &mut egui::Ui, config_manager: &mut ConfigManager, label: &str, tip: &str, path: ConfigPath, radians: f32) {
    let mut deg = radians.to_degrees();
    if ui
        .add(
            egui::Slider::new(&mut deg, -180.0..=180.0)
                .text(label)
                .suffix("°")
                .fixed_decimals(1)
                .clamping(egui::SliderClamping::Never),
        )
        .on_hover_text(tip)
        .changed()
    {
        let _ = config_manager.update_param(path, deg.to_radians().into());
    }
}

/// A pinhole camera's vertical field of view in degrees, 3..170 (the
/// renderers clamp what is typed beyond).
fn fov_row(ui: &mut egui::Ui, config_manager: &mut ConfigManager, path: ConfigPath, radians: f32) {
    let mut deg = radians.to_degrees();
    if ui
        .add(
            egui::Slider::new(&mut deg, 3.0..=170.0)
                .text(t!("escape_panel.camera_fov").as_ref())
                .suffix("°")
                .fixed_decimals(1),
        )
        .on_hover_text(t!("escape_panel.camera_fov_tip"))
        .changed()
    {
        let _ = config_manager.update_param(path, deg.to_radians().into());
    }
}

/// Where the eye is, read out: what Position X/Y/Z is for a camera that
/// stores a target and a distance instead.
fn position_readout(ui: &mut egui::Ui, eye: [f64; 3], decimals: usize, tip: &str) {
    ui.horizontal(|ui| {
        ui.label(t!("view.position").as_ref()).on_hover_text(tip);
        for (axis, v) in ["X", "Y", "Z"].iter().zip(eye) {
            ui.label(egui::RichText::new(format!("{axis} {v:.decimals$}")).monospace().weak()).on_hover_text(tip);
        }
    });
}

/// An escape zoom as 10^n, the plane's way: for a solid it sets the
/// distance to the target, for a terrain the world's scale.
#[cfg(feature = "engine-escape")]
fn escape_zoom_row(ui: &mut egui::Ui, config_manager: &mut ConfigManager, esc: &crate::config::escape::EscapeConfig) {
    ui.horizontal(|ui| {
        ui.label(t!("view.zoom").as_ref());
        let mut z10 = esc.zoom_log10();
        if ui
            .add(egui::DragValue::new(&mut z10).speed(0.02 * std::f64::consts::LOG10_2).max_decimals(4).prefix("10^"))
            .on_hover_text(t!("escape_panel.tooltip_zoom_log10"))
            .changed()
        {
            let z2 = crate::config::escape::EscapeConfig::log10_to_log2(z10);
            let _ = config_manager.update_param(ConfigPath::EscapeZoomLog2, (z2 as f32).into());
        }
        ui.label(egui::RichText::new(super::escape_panel::magnification_label(esc.zoom_log10())).weak());
    });
}

/// The 3D camera, one section for every 3D view (camera-unification C5):
/// where the eye is, what it looks at and how far, the angles in degrees,
/// the projection, Reset, and fly mode. Each camera's fields are its own
/// -- a flame's position and perspective, mode D's exact target, a
/// terrain's ground target and its zoom as the scale -- in one order
/// under one set of names; a camera that stores a target shows where its
/// eye is as a readout.
pub fn camera_3d(ui: &mut egui::Ui, config_manager: &mut ConfigManager, fly_mode_active: bool, fly_mode_toggle_requested: &mut bool) {
    use crate::camera::gesture::{self, ViewKind};
    let config = config_manager.active_config().clone();
    let kind = gesture::view_kind(&config);
    match kind {
        ViewKind::Flame3d => {
            use crate::config::slider::LazyUndoUi;
            // JWildfire's cam_pos: it carries the whole camera, and the
            // projection's viewpoint sits 1/perspective behind it.
            for (label, tip, path) in [
                (t!("view.camera_x"), t!("view.tooltip_camera_x"), ConfigPath::CameraX),
                (t!("view.camera_y"), t!("view.tooltip_camera_y"), ConfigPath::CameraY),
                (t!("view.camera_z"), t!("view.tooltip_camera_z"), ConfigPath::CameraZ),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label.as_ref()).on_hover_text(tip.as_ref());
                    let _ = ui.lazy_drag(config_manager, path, 0.01, "");
                });
            }
            angle_row(ui, config_manager, &t!("view.camera_pitch"), &t!("view.tooltip_camera_pitch"), ConfigPath::CameraRotationX, config.camera_rotation_x);
            angle_row(ui, config_manager, &t!("view.camera_yaw"), &t!("view.tooltip_camera_yaw"), ConfigPath::CameraRotationY, config.camera_rotation_y);
            angle_row(ui, config_manager, &t!("view.camera_bank"), &t!("view.tooltip_camera_bank"), ConfigPath::CameraBank, config.camera_bank);
            ui.label(egui::RichText::new(t!("view.flame_roll_is_rotation")).small().weak());
            let mut perspective = config.perspective_strength;
            if ui
                .add(super::VkbSlider::new(&mut perspective, 0.0..=10.0).text(t!("view.perspective").as_ref()).step_by(0.01))
                .on_hover_text(t!("view.tooltip_perspective"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::PerspectiveStrength, perspective.into());
            }
        }
        #[cfg(feature = "engine-escape")]
        ViewKind::Solid => {
            let esc = config.escape.clone();
            let eye = {
                let registry = crate::variations::global_registry();
                crate::escape::ifs::solid_analysis(&config, &registry)
                    .map(|ifs3| crate::escape::ifs::solid_camera(&esc, &ifs3).eye)
            };
            if let Some(eye) = eye {
                position_readout(ui, eye, 6, &t!("view.position_tip_solid"));
            }
            // The target as decimal strings, so a deep zoom's approach to
            // a point keeps its digits; blank is the attractor's centre.
            ui.horizontal(|ui| {
                ui.label(t!("escape_panel.camera_target")).on_hover_text(t!("escape_panel.camera_target_tip"));
                for (name, path, value) in [
                    ("X", ConfigPath::EscapeCamTargetX, &esc.cam_target_x),
                    ("Y", ConfigPath::EscapeCamTargetY, &esc.cam_target_y),
                    ("Z", ConfigPath::EscapeCamTargetZ, &esc.cam_target_z),
                ] {
                    let mut text = value.clone();
                    ui.label(name);
                    if ui
                        .add(egui::TextEdit::singleline(&mut text).desired_width(78.0).hint_text(t!("escape_panel.camera_target_auto")))
                        .on_hover_text(t!("escape_panel.camera_target_tip"))
                        .changed()
                    {
                        let _ = config_manager.update_param(path, ConfigValue::String(text.trim().to_string()));
                    }
                }
            });
            escape_zoom_row(ui, config_manager, &esc);
            angle_row(ui, config_manager, &t!("escape_panel.camera_pitch"), &t!("escape_panel.camera_pitch_tip"), ConfigPath::EscapeCamPitch, esc.cam_pitch);
            angle_row(ui, config_manager, &t!("escape_panel.camera_yaw"), &t!("escape_panel.camera_yaw_tip"), ConfigPath::EscapeCamYaw, esc.cam_yaw);
            angle_row(ui, config_manager, &t!("escape_panel.camera_bank"), &t!("escape_panel.camera_bank_tip"), ConfigPath::EscapeCamBank, esc.cam_bank);
            angle_row(ui, config_manager, &t!("view.roll"), &t!("view.roll_tip"), ConfigPath::EscapeRotation, esc.rotation);
            fov_row(ui, config_manager, ConfigPath::EscapeCamFov, esc.cam_fov);
        }
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => {
            let esc = config.escape.clone();
            // The eye in the plane's units: its offset from the target
            // in view widths, a view width being 4·2^-zoom.
            let cam = crate::escape::footprint::terrain_camera(&esc);
            let unit = 4.0 * (-esc.zoom_log2).exp2();
            let (cx, cy) = esc.center_f64();
            let top = esc.terrain.height as f64;
            let eye = [cx + cam.eye_rel[0] * unit, cy + cam.eye_rel[1] * unit, (top + cam.eye_rel[2]) * unit];
            position_readout(ui, eye, 9, &t!("view.position_tip_terrain"));
            // The target: the ground at the view's centre, exact decimals.
            for (axis, value, path) in [
                ("re", &esc.center_re, ConfigPath::EscapeCenterRe),
                ("im", &esc.center_im, ConfigPath::EscapeCenterIm),
            ] {
                ui.horizontal(|ui| {
                    ui.label(format!("{} {axis}:", t!("escape_panel.camera_target")))
                        .on_hover_text(t!("view.terrain_target_tip"));
                    let mut text = value.clone();
                    if ui.text_edit_singleline(&mut text).changed() {
                        let _ = config_manager.update_param(path.clone(), ConfigValue::String(text));
                    }
                });
            }
            escape_zoom_row(ui, config_manager, &esc);
            angle_row(ui, config_manager, &t!("escape_panel.camera_pitch"), &t!("escape_panel.camera_pitch_tip"), ConfigPath::EscapeCamPitch, esc.cam_pitch);
            angle_row(ui, config_manager, &t!("escape_panel.camera_yaw"), &t!("view.terrain_yaw_tip"), ConfigPath::EscapeCamYaw, esc.cam_yaw);
            angle_row(ui, config_manager, &t!("escape_panel.camera_bank"), &t!("escape_panel.camera_bank_tip"), ConfigPath::EscapeCamBank, esc.cam_bank);
            fov_row(ui, config_manager, ConfigPath::EscapeCamFov, esc.cam_fov);
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => {
            let t = config.sim.terrain.clone();
            // The eye in grid widths from the grid's corner. A grid bound
            // to the window is read at 1080p: its angles are the
            // window's, its numbers near enough.
            let (gw, gh) = crate::sim::SimRenderer::grid_for(&config.sim, 1920, 1080);
            let cam = crate::sim::terrain::sim_terrain_camera(&config, gw, gh);
            let w = gw.max(2) as f64;
            position_readout(ui, [cam.eye[0] / w, cam.eye[1] / w, cam.eye[2] / w], 3, &t!("view.position_tip_sim"));
            for (label, path, value) in [
                (t!("sim_panel.terrain_target_x"), ConfigPath::SimTerrainTargetX, t.target_x),
                (t!("sim_panel.terrain_target_y"), ConfigPath::SimTerrainTargetY, t.target_y),
            ] {
                let mut v = value;
                if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).text(label.as_ref())).changed() {
                    let _ = config_manager.update_param(path, v.into());
                }
            }
            let mut d = t.cam_distance;
            if ui
                .add(egui::Slider::new(&mut d, 0.05..=8.0).logarithmic(true).text(t!("sim_panel.terrain_distance").as_ref()))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimTerrainCamDistance, d.into());
            }
            angle_row(ui, config_manager, &t!("escape_panel.camera_pitch"), &t!("escape_panel.camera_pitch_tip"), ConfigPath::SimTerrainCamPitch, t.cam_pitch);
            angle_row(ui, config_manager, &t!("escape_panel.camera_yaw"), &t!("view.sim_terrain_yaw_tip"), ConfigPath::SimTerrainCamYaw, t.cam_yaw);
            angle_row(ui, config_manager, &t!("escape_panel.camera_bank"), &t!("escape_panel.camera_bank_tip"), ConfigPath::SimTerrainCamBank, t.cam_bank);
            fov_row(ui, config_manager, ConfigPath::SimTerrainCamFov, t.cam_fov);
        }
        _ => return,
    }
    // A flame's Reset is the 2D Camera section's: one camera, one button.
    if kind != ViewKind::Flame3d && ui.button(t!("view.reset").as_ref()).clicked() {
        if let Some(edit) = gesture::reset(&config) {
            edit.apply(config_manager);
        }
    }
    fly_controls(ui, config_manager, kind, fly_mode_active, fly_mode_toggle_requested);
}

/// Fly mode: the toggle and its settings, one block for every 3D camera.
/// The sensitivity and Invert Y turn every orbit too.
fn fly_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    kind: crate::camera::gesture::ViewKind,
    fly_mode_active: bool,
    fly_mode_toggle_requested: &mut bool,
) {
    if super::visibility::fly_mode(kind).is_show() {
        let fly_label = if fly_mode_active { t!("view.fly_mode_on") } else { t!("view.fly_mode_off") };
        if ui.button(fly_label.as_ref()).on_hover_text(t!("view.tooltip_fly_mode")).clicked() {
            *fly_mode_toggle_requested = true;
        }
    }
    egui::CollapsingHeader::new(t!("view.fly_mode_settings").as_ref())
        .default_open(false)
        .show(ui, |ui| {
            // FreeLook turns about the screen's axes and can roll; FPS
            // keeps the horizon level. Q/E's rise follows the choice.
            use crate::storage::FlyCameraMode;
            let mode = config_manager.system_settings().fly_camera_mode;
            ui.horizontal(|ui| {
                ui.label(t!("view.fly_camera_mode"));
                for (m, label, tip, value) in [
                    (FlyCameraMode::FreeLook, t!("view.fly_camera_mode_free_look"), t!("view.tooltip_fly_camera_mode_free_look"), "free_look"),
                    (FlyCameraMode::Fps, t!("view.fly_camera_mode_fps"), t!("view.tooltip_fly_camera_mode_fps"), "fps"),
                ] {
                    if ui.selectable_label(mode == m, label.as_ref()).on_hover_text(tip.as_ref()).clicked() && mode != m {
                        let _ = config_manager.update_system_setting(ConfigPath::SystemFlyCameraMode, value.into());
                    }
                }
            });
            let s = config_manager.system_settings().clone();
            let mut sensitivity = s.fly_mouse_sensitivity;
            ui.horizontal(|ui| {
                ui.label(t!("view.fly_sensitivity"));
                if ui.add(super::VkbSlider::new(&mut sensitivity, 0.0005..=0.05).logarithmic(true)).changed() {
                    let _ = config_manager.update_system_setting(ConfigPath::SystemFlyMouseSensitivity, sensitivity.into());
                }
            });
            let mut speed = s.fly_move_speed;
            ui.horizontal(|ui| {
                ui.label(t!("view.fly_move_speed"));
                if ui.add(super::VkbSlider::new(&mut speed, 0.05..=20.0).logarithmic(true)).changed() {
                    let _ = config_manager.update_system_setting(ConfigPath::SystemFlyMoveSpeed, speed.into());
                }
            });
            let mut sprint = s.fly_sprint_multiplier;
            ui.horizontal(|ui| {
                ui.label(t!("view.fly_sprint_multiplier"));
                if ui.add(super::VkbSlider::new(&mut sprint, 1.0..=20.0)).changed() {
                    let _ = config_manager.update_system_setting(ConfigPath::SystemFlySprintMultiplier, sprint.into());
                }
            });
            let mut invert_y = s.fly_invert_y;
            if ui.checkbox(&mut invert_y, t!("view.fly_invert_y").as_ref()).changed() {
                let _ = config_manager.update_system_setting(ConfigPath::SystemFlyInvertY, invert_y.into());
            }
        });
}

/// A plain float slider writing `path`, its range a suggestion.
#[allow(clippy::too_many_arguments)]
fn float_row(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    label: &str,
    tip: &str,
    path: ConfigPath,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    log: bool,
) {
    let mut v = value;
    if ui
        .add(super::VkbSlider::new(&mut v, range).logarithmic(log).text(label))
        .on_hover_text(tip)
        .changed()
    {
        let _ = config_manager.update_param(path, v.into());
    }
}

/// What lies between the eye and the picture, in every 3D view: fog --
/// a flame's and mode D's, the same two fields -- with a flame's depth
/// weighting, and a terrain's reach and haze.
pub fn atmosphere_3d(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    use crate::camera::gesture::{self, ViewKind};
    let config = config_manager.active_config().clone();
    let kind = gesture::view_kind(&config);
    if matches!(kind, ViewKind::Flame3d | ViewKind::Solid) {
        ui.label(t!("view.fog_section").as_ref());
        float_row(ui, config_manager, &t!("view.fog_strength"), &t!("view.tooltip_fog_strength"), ConfigPath::FogStrength, config.fog_strength, 0.0..=5.0, false);
        float_row(ui, config_manager, &t!("view.fog_start"), &t!("view.tooltip_fog_start"), ConfigPath::FogStart, config.fog_start, -5.0..=5.0, false);
    }
    match kind {
        ViewKind::Flame3d => {
            // Density weighting by depth: compensation only means
            // something with perspective (the weight is 1 without), and
            // the far fade thins density where fog recolours it.
            ui.add_space(8.0);
            ui.label(t!("view.depth_density_section").as_ref());
            float_row(ui, config_manager, &t!("view.depth_density_compensation"), &t!("view.tooltip_depth_density_compensation"), ConfigPath::DepthDensityCompensation, config.depth_density_compensation, 0.0..=1.0, false);
            ui.add_space(8.0);
            ui.label(t!("view.far_density_fade_section").as_ref());
            float_row(ui, config_manager, &t!("view.far_density_fade"), &t!("view.tooltip_far_density_fade"), ConfigPath::FarDensityFade, config.far_density_fade, 0.0..=5.0, false);
            float_row(ui, config_manager, &t!("view.far_density_fade_start"), &t!("view.tooltip_far_density_fade_start"), ConfigPath::FarDensityFadeStart, config.far_density_fade_start, -5.0..=5.0, false);
        }
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => {
            let t = &config.escape.terrain;
            float_row(ui, config_manager, &t!("view.far"), &t!("escape_panel.terrain_far_tip"), ConfigPath::EscapeTerrainFar, t.far, 0.5..=64.0, true);
            float_row(ui, config_manager, &t!("escape_panel.terrain_haze"), &t!("escape_panel.terrain_haze_tip"), ConfigPath::EscapeTerrainHaze, t.haze, 0.0..=1.0, false);
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => {
            let t = &config.sim.terrain;
            // A single grid ends at its edge; only a repeated one reaches
            // a horizon to fog.
            if t.tiling == crate::config::sim::SimTiling::Repeat {
                float_row(ui, config_manager, &t!("view.far"), &t!("sim_panel.terrain_far_tip"), ConfigPath::SimTerrainFar, t.far, 0.5..=64.0, true);
                float_row(ui, config_manager, &t!("escape_panel.terrain_haze"), &t!("escape_panel.terrain_haze_tip"), ConfigPath::SimTerrainHaze, t.haze, 0.0..=1.0, false);
            } else {
                ui.label(egui::RichText::new(t!("view.far_needs_repeat")).small().weak());
            }
        }
        _ => {}
    }
}

/// The path tracer's settings, their paths, and the tier for a 3D view
/// other than a flame.
#[cfg(feature = "engine-escape")]
type PathOf = (
    crate::config::escape::PathTraceConfig,
    super::escape_panel::PathPaths,
    ConfigPath,
    crate::config::escape::RenderTier,
    &'static str,
);

#[cfg(feature = "engine-escape")]
fn path_of(config: &crate::config::FractalConfig) -> Option<PathOf> {
    use crate::camera::gesture::{self, ViewKind};
    match gesture::view_kind(config) {
        ViewKind::Solid => Some((
            config.escape.path.clone(),
            super::escape_panel::escape_path_paths(),
            ConfigPath::EscapeSolidTier,
            config.escape.solid_tier,
            "solid_tier",
        )),
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => Some((
            config.escape.path.clone(),
            super::escape_panel::escape_path_paths(),
            ConfigPath::EscapeTerrainTier,
            config.escape.terrain.tier,
            "terrain_tier",
        )),
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => Some((
            config.sim.terrain.path.clone(),
            super::escape_panel::PathPaths {
                samples: ConfigPath::SimPathSamples,
                bounces: ConfigPath::SimPathBounces,
                environment: ConfigPath::SimPathEnvironment,
                gloss: ConfigPath::SimPathGloss,
                roughness: ConfigPath::SimPathRoughness,
                emission: ConfigPath::SimPathEmission,
                aperture: ConfigPath::SimPathAperture,
                focus: ConfigPath::SimPathFocus,
                denoise: ConfigPath::SimPathDenoise,
                sky: ConfigPath::SimPathSky,
                zenith: ConfigPath::SimPathZenith,
            },
            ConfigPath::SimTerrainTier,
            config.sim.terrain.tier,
            "sim_terrain_tier",
        )),
        _ => None,
    }
}

/// Depth of field, in every 3D view: a flame's focus and blur, the path
/// tracer's aperture and focus where it runs (the lit tier draws
/// everything sharp).
pub fn depth_of_field_3d(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    use crate::camera::gesture::{self, ViewKind};
    let config = config_manager.active_config().clone();
    if gesture::view_kind(&config) == ViewKind::Flame3d {
        float_row(ui, config_manager, &t!("view.dof_focus_distance"), &t!("view.tooltip_dof_focus_distance"), ConfigPath::DofFocusDistance, config.dof_focus_distance, -5.0..=5.0, false);
        float_row(ui, config_manager, &t!("view.dof_blur_strength"), &t!("view.tooltip_dof_blur_strength"), ConfigPath::DofBlurStrength, config.dof_blur_strength, 0.0..=1.0, false);
        return;
    }
    #[cfg(feature = "engine-escape")]
    if let Some((pt, paths, _, tier, _)) = path_of(&config) {
        if tier == crate::config::escape::RenderTier::Lit {
            ui.label(egui::RichText::new(t!("view.dof_needs_path")).small().weak());
        } else {
            super::escape_panel::show_path_dof(ui, config_manager, &pt, &paths);
        }
    }
}

/// How a 3D view is lit and what its surfaces are made of: the render
/// tier and the path tracer (samples, sky, material), a terrain's shadows
/// and occlusion, and the lights and the lit tier's shading -- the Solid
/// Lighting panel's (camera-unification C5: in the View panel for now,
/// its own panel if it grows enough).
pub fn lighting_3d(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    use crate::camera::gesture::{self, ViewKind};
    let config = config_manager.active_config().clone();
    let kind = gesture::view_kind(&config);
    #[cfg(feature = "engine-escape")]
    if let Some((pt, paths, tier_path, tier, id)) = path_of(&config) {
        super::escape_panel::show_path_tracing(ui, config_manager, &pt, &paths, tier_path, tier, id);
        ui.separator();
    }
    match kind {
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => {
            let t = &config.escape.terrain;
            terrain_shadows(
                ui,
                config_manager,
                [t.shadow, t.shadow_sharpness, t.occlusion],
                t.tier,
                [ConfigPath::EscapeTerrainShadow, ConfigPath::EscapeTerrainShadowSharpness, ConfigPath::EscapeTerrainOcclusion],
                0.05,
                &t!("escape_panel.terrain_occlusion_tip"),
            );
            ui.separator();
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => {
            let t = &config.sim.terrain;
            terrain_shadows(
                ui,
                config_manager,
                [t.shadow, t.shadow_sharpness, t.occlusion],
                t.tier,
                [ConfigPath::SimTerrainShadow, ConfigPath::SimTerrainShadowSharpness, ConfigPath::SimTerrainOcclusion],
                0.2,
                &t!("sim_panel.terrain_occlusion_tip"),
            );
            ui.separator();
        }
        _ => {}
    }
    super::solid_panel::lighting_rig(ui, config_manager, matches!(kind, ViewKind::Flame2d | ViewKind::Flame3d));
}

/// A terrain's traced shadows and its occlusion: `[shadow, sharpness,
/// occlusion]` and their paths. Occlusion is the lit tier's stand-in for
/// the sky the path tracer traces, so it hides when only that runs.
#[cfg(feature = "terrain")]
fn terrain_shadows(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    values: [f32; 3],
    tier: crate::config::escape::RenderTier,
    paths: [ConfigPath; 3],
    occlusion_max: f32,
    occlusion_tip: &str,
) {
    let [shadow, sharpness, occlusion] = values;
    let [p_shadow, p_sharpness, p_occlusion] = paths;
    float_row(ui, config_manager, &t!("escape_panel.terrain_shadow"), &t!("escape_panel.terrain_shadow_tip"), p_shadow, shadow, 0.0..=1.0, false);
    float_row(ui, config_manager, &t!("escape_panel.terrain_shadow_sharpness"), &t!("escape_panel.terrain_shadow_sharpness_tip"), p_sharpness, sharpness, 1.0..=128.0, true);
    if tier != crate::config::escape::RenderTier::PathTraced {
        float_row(ui, config_manager, &t!("escape_panel.terrain_occlusion"), occlusion_tip, p_occlusion, occlusion, 0.0..=occlusion_max, false);
    }
}
