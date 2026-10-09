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
