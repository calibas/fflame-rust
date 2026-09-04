//! The Simulation panel.
//!
//! Mirrors `escape_panel.rs` in shape — a mode toggle, then the
//! registry-driven controls — with one section escape has no need for:
//! **transport**. A simulation is stateful, so Run / Pause / Step /
//! Reset are as much a part of using it as any parameter, and the step
//! counter is the state's identity.
//!
//! The parameter controls are generated from the registry definitions
//! rather than written out, so a model added in phase 2 gets its UI for
//! free — the same property the variation and formula panels have.

use crate::config::delta::ConfigPath;
use crate::config::manager::ConfigManager;
use crate::config::sim::{SimBoundary, SimDownscale, SimGrid, SimInit, SimUpscale};
use crate::scene::transforms::RenderMode;
use crate::sim::{SimParamDef, COLORINGS, MODELS};
use rust_i18n::t;

/// Transport state, owned by the App (it is view state, not config).
pub struct SimUiState<'a> {
    pub running: &'a mut bool,
    pub step_once: &'a mut bool,
    pub reseed: &'a mut bool,
    /// Steps completed in the live run, for the readout.
    pub step_index: u32,
    /// Grid actually in use, which a bound grid makes non-obvious.
    pub grid: (u32, u32),
}

/// A registry parameter control: dropdown when it has `choices`,
/// slider otherwise. Same shape as the escape panel's, over this
/// engine's param type.
fn param_control(ui: &mut egui::Ui, v: &mut f32, p: &'static SimParamDef, salt: &str) -> bool {
    if p.choices.is_empty() {
        let range = p.min..=p.max;
        // Logarithmic where the interesting band is orders of magnitude
        // below the maximum; Gray-Scott's feed and kill are not, so
        // this stays linear and the drag is fine-grained instead.
        let speed = ((p.max - p.min) as f64 / 500.0).max(1e-6);
        return ui
            .add(
                egui::Slider::new(v, range)
                    .text(p.display_name)
                    .drag_value_speed(speed),
            )
            .on_hover_text(p.tooltip)
            .changed();
    }
    // Round rather than truncate, and clamp: a config written before
    // the choice list existed (or hand-edited) must land on a real
    // entry rather than panicking the index.
    let last = p.choices.len() - 1;
    let mut idx = if v.is_finite() {
        (v.round().max(0.0) as usize).min(last)
    } else {
        0
    };
    let before = idx;
    ui.horizontal(|ui| {
        ui.label(p.display_name);
        egui::ComboBox::from_id_salt((salt, p.name))
            .selected_text(p.choices[idx])
            .show_ui(ui, |ui| {
                for (i, c) in p.choices.iter().enumerate() {
                    ui.selectable_value(&mut idx, i, *c);
                }
            })
            .response
            .on_hover_text(p.tooltip);
    });
    if idx != before {
        *v = idx as f32;
        return true;
    }
    false
}

pub fn render_sim_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    workspace_request: &mut Option<super::workspace::WorkspaceLayout>,
    state: SimUiState<'_>,
) {
    let config = config_manager.active_config().clone();
    let sim = config.sim.clone();
    let active = config.render_mode == RenderMode::Simulation;

    // ---- One toggle, not a mode picker ----
    // Same reasoning as the escape panel: 2D/3D is meaningless here,
    // and offering it alongside would read as Simulation being a third
    // kind of flame. Leaving returns to 3D unconditionally.
    let label = if active {
        t!("sim_panel.toggle_off")
    } else {
        t!("sim_panel.toggle_on")
    };
    if ui
        .add(egui::Button::new(label.as_ref()).selected(active))
        .on_hover_text(t!("sim_panel.toggle_tip"))
        .clicked()
    {
        let target = if active { RenderMode::ThreeD } else { RenderMode::Simulation };
        if let Err(e) = super::escape_panel::switch_render_mode(config_manager, target) {
            log::error!("Failed to switch render mode: {e}");
        } else if !active {
            *state.reseed = true;
            workspace_request.replace(super::workspace::WorkspaceLayout::Simulation);
        }
    }

    if !active {
        ui.separator();
        ui.label(t!("sim_panel.inactive_hint"));
        return;
    }

    ui.separator();

    // ---- Transport ----
    // The section escape has no analogue for. A simulation's picture is
    // "the state at step N", so the counter is not decoration: it is
    // what makes a still identifiable.
    ui.horizontal(|ui| {
        let run_label = if *state.running {
            t!("sim_panel.pause")
        } else {
            t!("sim_panel.run")
        };
        if ui.button(run_label.as_ref()).clicked() {
            *state.running = !*state.running;
        }
        if ui
            .add_enabled(!*state.running, egui::Button::new(t!("sim_panel.step").as_ref()))
            .on_hover_text(t!("sim_panel.step_tip"))
            .clicked()
        {
            *state.step_once = true;
        }
        if ui
            .button(t!("sim_panel.reset").as_ref())
            .on_hover_text(t!("sim_panel.reset_tip"))
            .clicked()
        {
            *state.reseed = true;
        }
    });
    ui.label(
        t!(
            "sim_panel.step_readout",
            step = state.step_index.to_string(),
            width = state.grid.0.to_string(),
            height = state.grid.1.to_string()
        )
        .as_ref(),
    );

    ui.separator();

    // ---- Model ----
    let model = crate::sim::model_or_default(&sim.model);
    ui.horizontal(|ui| {
        ui.label(t!("sim_panel.model").as_ref());
        egui::ComboBox::from_id_salt("sim_model")
            .selected_text(model.display_name)
            .show_ui(ui, |ui| {
                for m in MODELS {
                    if ui
                        .selectable_label(m.name == model.name, m.display_name)
                        .on_hover_text(m.description)
                        .clicked()
                        && m.name != model.name
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::SimModel, m.name.to_string().into());
                    }
                }
            });
    });

    if !model.presets.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("sim_panel.presets").as_ref());
            for pre in model.presets {
                if ui.small_button(pre.display_name).clicked() {
                    // A preset is its parameters AND its measured step
                    // count together: applying the numbers without the
                    // steps shows the pattern half-formed.
                    let mut changes: Vec<(ConfigPath, crate::config::delta::ConfigValue)> = pre
                        .params
                        .iter()
                        .map(|(k, v)| {
                            (
                                ConfigPath::SimModelParam { param: (*k).to_string() },
                                (*v).into(),
                            )
                        })
                        .collect();
                    changes.push((ConfigPath::SimSteps, pre.steps.into()));
                    let _ = config_manager
                        .update_batch(changes, "history.action.sim_preset".to_string());
                    *state.reseed = true;
                }
            }
        });
    }

    for (i, p) in model.parameters.iter().enumerate() {
        let _ = i;
        let mut v = sim.model_param(p.name, p.default);
        if param_control(ui, &mut v, p, "sim_model") {
            let _ = config_manager.update_param(
                ConfigPath::SimModelParam { param: p.name.to_string() },
                v.into(),
            );
        }
    }

    ui.separator();

    // ---- Grid ----
    // The control that most needs explaining, so it says what it will
    // do rather than only what it is: a bound grid re-simulates on
    // resize and on export at a different size.
    let mut bound = sim.grid.is_bound();
    if ui
        .checkbox(&mut bound, t!("sim_panel.bind_grid").as_ref())
        .on_hover_text(t!("sim_panel.bind_grid_tip"))
        .changed()
    {
        let _ = config_manager.update_param(
            ConfigPath::SimGridMode,
            if bound { "viewport" } else { "fixed" }.to_string().into(),
        );
        *state.reseed = true;
    }
    match sim.grid {
        SimGrid::Viewport { scale } => {
            let mut v = scale;
            if ui
                .add(egui::Slider::new(&mut v, 0.125..=4.0).text(t!("sim_panel.grid_scale").as_ref()))
                .on_hover_text(t!("sim_panel.grid_scale_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimGridScale, v.into());
            }
        }
        SimGrid::Fixed { width, height } => {
            ui.horizontal(|ui| {
                let mut w = width;
                let mut h = height;
                ui.label(t!("sim_panel.grid_size").as_ref());
                if ui.add(egui::DragValue::new(&mut w).range(16..=8192)).changed() {
                    let _ = config_manager.update_param(ConfigPath::SimGridWidth, w.into());
                    *state.reseed = true;
                }
                ui.label("x");
                if ui.add(egui::DragValue::new(&mut h).range(16..=8192)).changed() {
                    let _ = config_manager.update_param(ConfigPath::SimGridHeight, h.into());
                    *state.reseed = true;
                }
            });
        }
    }

    // ---- Resolve filters ----
    ui.horizontal(|ui| {
        ui.label(t!("sim_panel.upscale").as_ref());
        egui::ComboBox::from_id_salt("sim_upscale")
            .selected_text(sim.upscale.name())
            .show_ui(ui, |ui| {
                for n in SimUpscale::NAMES {
                    if ui.selectable_label(sim.upscale.name() == *n, *n).clicked() {
                        let _ = config_manager
                            .update_param(ConfigPath::SimUpscale, (*n).to_string().into());
                    }
                }
            })
            .response
            .on_hover_text(t!("sim_panel.upscale_tip"));
        ui.label(t!("sim_panel.downscale").as_ref());
        egui::ComboBox::from_id_salt("sim_downscale")
            .selected_text(sim.downscale.name())
            .show_ui(ui, |ui| {
                for n in SimDownscale::NAMES {
                    if ui.selectable_label(sim.downscale.name() == *n, *n).clicked() {
                        let _ = config_manager
                            .update_param(ConfigPath::SimDownscale, (*n).to_string().into());
                    }
                }
            });
    });

    ui.separator();

    // ---- Seed, init, boundary: everything that restarts the run ----
    ui.horizontal(|ui| {
        let mut seed = sim.seed as u32;
        ui.label(t!("sim_panel.seed").as_ref());
        if ui.add(egui::DragValue::new(&mut seed)).changed() {
            let _ = config_manager.update_param(ConfigPath::SimSeed, seed.into());
            *state.reseed = true;
        }
        if ui.small_button(t!("sim_panel.randomize").as_ref()).clicked() {
            let n: u32 = rand::random();
            let _ = config_manager.update_param(ConfigPath::SimSeed, n.into());
            *state.reseed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label(t!("sim_panel.init").as_ref());
        egui::ComboBox::from_id_salt("sim_init")
            .selected_text(sim.init.kind_name())
            .show_ui(ui, |ui| {
                for k in SimInit::KINDS {
                    if ui.selectable_label(sim.init.kind_name() == *k, *k).clicked() {
                        let _ = config_manager
                            .update_param(ConfigPath::SimInitKind, (*k).to_string().into());
                        *state.reseed = true;
                    }
                }
            })
            .response
            .on_hover_text(t!("sim_panel.init_tip"));
    });
    // Only the fields this init kind actually has. Phase 0 measured why
    // the radius matters: 12-cell blobs die where 24-cell blobs live.
    match sim.init {
        SimInit::Noise { amplitude } => {
            let mut a = amplitude;
            if ui
                .add(egui::Slider::new(&mut a, 0.0..=1.0).text(t!("sim_panel.amplitude").as_ref()))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimInitAmplitude, a.into());
                *state.reseed = true;
            }
        }
        SimInit::Blob { radius } | SimInit::Ring { radius } => {
            let mut r = radius;
            if ui
                .add(egui::Slider::new(&mut r, 1..=256).text(t!("sim_panel.radius").as_ref()))
                .on_hover_text(t!("sim_panel.radius_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimInitRadius, r.into());
                *state.reseed = true;
            }
        }
        SimInit::Blobs { count, radius } => {
            let mut r = radius;
            if ui
                .add(egui::Slider::new(&mut r, 1..=256).text(t!("sim_panel.radius").as_ref()))
                .on_hover_text(t!("sim_panel.radius_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimInitRadius, r.into());
                *state.reseed = true;
            }
            let mut c = count;
            if ui
                .add(egui::Slider::new(&mut c, 1..=64).text(t!("sim_panel.count").as_ref()))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimInitCount, c.into());
                *state.reseed = true;
            }
        }
        SimInit::Line | SimInit::Center => {}
    }

    ui.horizontal(|ui| {
        ui.label(t!("sim_panel.boundary").as_ref());
        egui::ComboBox::from_id_salt("sim_boundary")
            .selected_text(sim.boundary.name())
            .show_ui(ui, |ui| {
                for n in SimBoundary::NAMES {
                    if ui.selectable_label(sim.boundary.name() == *n, *n).clicked() {
                        let _ = config_manager
                            .update_param(ConfigPath::SimBoundary, (*n).to_string().into());
                        *state.reseed = true;
                    }
                }
            })
            .response
            .on_hover_text(t!("sim_panel.boundary_tip"));
    });

    ui.separator();

    // ---- Stepping ----
    let mut steps = sim.steps;
    if ui
        .add(
            egui::Slider::new(&mut steps, 1..=100_000)
                .text(t!("sim_panel.steps").as_ref())
                .logarithmic(true),
        )
        .on_hover_text(t!("sim_panel.steps_tip"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::SimSteps, steps.into());
    }
    let mut spf = sim.steps_per_frame;
    if ui
        .add(egui::Slider::new(&mut spf, 1..=256).text(t!("sim_panel.steps_per_frame").as_ref()))
        .on_hover_text(t!("sim_panel.steps_per_frame_tip"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::SimStepsPerFrame, spf.into());
    }
    let mut dt = sim.dt;
    if ui
        .add(
            egui::Slider::new(&mut dt, 0.001..=model.max_dt)
                .text(t!("sim_panel.dt").as_ref()),
        )
        .on_hover_text(t!("sim_panel.dt_tip"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::SimDt, dt.into());
    }

    ui.separator();

    // ---- Colouring ----
    let coloring = crate::sim::coloring_or_default(&sim.coloring);
    ui.horizontal(|ui| {
        ui.label(t!("sim_panel.coloring").as_ref());
        egui::ComboBox::from_id_salt("sim_coloring")
            .selected_text(coloring.display_name)
            .show_ui(ui, |ui| {
                for c in COLORINGS {
                    if ui
                        .selectable_label(c.name == coloring.name, c.display_name)
                        .on_hover_text(c.description)
                        .clicked()
                        && c.name != coloring.name
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::SimColoring, c.name.to_string().into());
                    }
                }
            });
    });
    for p in coloring.parameters {
        let mut v = sim.coloring_param(p.name, p.default);
        if param_control(ui, &mut v, p, "sim_coloring") {
            let _ = config_manager.update_param(
                ConfigPath::SimColoringParam { param: p.name.to_string() },
                v.into(),
            );
        }
    }
}
