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
use crate::config::sim::{
    SimBoundary, SimDownscale, SimGrid, SimInit, SimMatteChannel, SimMatteEdge, SimUpscale,
    SimWarp, SimWarpFilter,
};
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
                    // And the model's time step. A preset is a whole
                    // recipe -- parameters, steps, initial field,
                    // colouring -- and a dt the user had dragged
                    // somewhere else would show a different picture
                    // from the one the preset is named for. Lenia is
                    // the sharp case: it runs at 0.1 and dies at 1.
                    if !model.has(crate::sim::ModelFeature::NoTimeStep) {
                        changes.push((ConfigPath::SimDt, model.default_dt.into()));
                    }
                    // A preset that names an initial field brings it:
                    // FitzHugh-Nagumo's constants give spirals from a
                    // cut wavefront and a flat field from noise, so
                    // applying only the numbers would ship a picture of
                    // nothing.
                    if let Some(init) = pre.init {
                        changes.push((
                            ConfigPath::SimInitKind,
                            init.kind_name().to_string().into(),
                        ));
                    }
                    // And the colouring it is meant to be seen
                    // through, with all of that colouring's
                    // parameters. Which colouring a model wants is a
                    // property of its state layout, not a taste the
                    // user should have to acquire: a sandpile's
                    // heights want 1/3, the snowfake's crystal is
                    // channel .z, DLA reads as a cluster only through
                    // arrival order.
                    if let Some(c) = pre.coloring {
                        changes.push((ConfigPath::SimColoring, c.to_string().into()));
                        for (k, v) in pre.coloring_params {
                            changes.push((
                                ConfigPath::SimColoringParam { param: (*k).to_string() },
                                (*v).into(),
                            ));
                        }
                    }
                    // And what the model calls empty space. A growth
                    // model without this is illegible: `age` cannot
                    // tell a cell that never grew from one that grew
                    // long ago, so the two ends of the palette are
                    // "background" and "oldest" at once. Set either
                    // way, so choosing a preset that wants no matte
                    // clears one the last preset set.
                    let matte = pre.matte.unwrap_or_default();
                    changes.push((
                        ConfigPath::SimMatteChannel,
                        matte.channel.name().to_string().into(),
                    ));
                    changes.push((ConfigPath::SimMatteCutoff, matte.cutoff.into()));
                    changes.push((ConfigPath::SimMatteSoftness, matte.softness.into()));
                    changes.push((ConfigPath::SimMatteInvert, matte.invert.into()));
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
        // No sizes to offer: these shapes are defined by the grid.
        SimInit::Line | SimInit::Center | SimInit::BrokenWave => {}
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
    // From 0, which is the no-cap sentinel: an integer slider sets
    // `smallest_positive` to 1, and egui's logarithmic sliders take a
    // zero bound, so the leftmost stop is 0 and the next is 1.
    if ui
        .add(
            egui::Slider::new(&mut steps, 0..=100_000)
                .text(t!("sim_panel.steps").as_ref())
                .logarithmic(true),
        )
        .on_hover_text(t!("sim_panel.steps_tip"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::SimSteps, steps.into());
    }
    if sim.steps == 0 {
        // An export runs `steps` from the seed, so at 0 it is the
        // seed. Saying it here beats finding out from a blank PNG.
        ui.label(egui::RichText::new(t!("sim_panel.steps_uncapped")).small().weak())
            .on_hover_text(t!("sim_panel.steps_uncapped_tip"));
    }
    let mut spf = sim.steps_per_frame;
    if ui
        .add(egui::Slider::new(&mut spf, 1..=256).text(t!("sim_panel.steps_per_frame").as_ref()))
        .on_hover_text(t!("sim_panel.steps_per_frame_tip"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::SimStepsPerFrame, spf.into());
    }
    // An automaton advances by a generation, not by dt: showing the
    // slider would be a control that does nothing.
    let mut dt = sim.dt;
    if !model.has(crate::sim::ModelFeature::NoTimeStep) {
        // The range is the model's STATIC ceiling, never the
        // parameter-dependent stability cap. A range that moved with
        // the other sliders moved this slider's handle -- and, once
        // egui clamped the value into it, the stored dt too -- so
        // dragging Mobility appeared to edit the time step.
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
        // What the solver will actually use. Capping silently would
        // leave the panel claiming a step the run does not take.
        let effective = model.max_dt_for(&sim.model_params);
        if sim.dt > effective * 1.001 {
            ui.label(
                egui::RichText::new(t!(
                    "sim_panel.dt_capped",
                    dt = format!("{effective:.4}")
                ))
                .small()
                .weak(),
            )
            .on_hover_text(t!("sim_panel.dt_capped_tip"));
        }
    }

    ui.separator();

    // ---- Warp ----
    // Per-step rates about the centre. The ranges are narrow on
    // purpose: a step is a fraction of a frame, and a percent of zoom
    // a step is already a fast pull.
    ui.collapsing(t!("sim_panel.warp").as_ref(), |ui| {
        ui.label(egui::RichText::new(t!("sim_panel.warp_tip")).small().weak());
        let w = sim.warp;
        let mut zoom = w.zoom;
        if ui
            .add(
                egui::Slider::new(&mut zoom, 0.98..=1.02)
                    .text(t!("sim_panel.warp_zoom").as_ref())
                    .fixed_decimals(4),
            )
            .on_hover_text(t!("sim_panel.warp_zoom_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::SimWarpZoom, zoom.into());
        }
        let mut rot = w.rotation;
        if ui
            .add(
                egui::Slider::new(&mut rot, -0.05..=0.05)
                    .text(t!("sim_panel.warp_rotation").as_ref())
                    .fixed_decimals(4),
            )
            .on_hover_text(t!("sim_panel.warp_rotation_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::SimWarpRotation, rot.into());
        }
        let mut px = w.pan_x;
        if ui
            .add(egui::Slider::new(&mut px, -2.0..=2.0).text(t!("sim_panel.warp_pan_x").as_ref()))
            .on_hover_text(t!("sim_panel.warp_pan_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::SimWarpPanX, px.into());
        }
        let mut py = w.pan_y;
        if ui
            .add(egui::Slider::new(&mut py, -2.0..=2.0).text(t!("sim_panel.warp_pan_y").as_ref()))
            .on_hover_text(t!("sim_panel.warp_pan_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::SimWarpPanY, py.into());
        }
        let mut flow = w.flow;
        if ui
            .add(
                egui::Slider::new(&mut flow, -0.05..=0.05)
                    .text(t!("sim_panel.warp_flow").as_ref())
                    .fixed_decimals(4),
            )
            .on_hover_text(t!("sim_panel.warp_flow_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::SimWarpFlow, flow.into());
        }
        ui.horizontal(|ui| {
            ui.label(t!("sim_panel.warp_filter").as_ref());
            egui::ComboBox::from_id_salt("sim_warp_filter")
                .selected_text(w.filter.name())
                .show_ui(ui, |ui| {
                    for n in SimWarpFilter::NAMES {
                        if ui.selectable_label(w.filter.name() == *n, *n).clicked() {
                            let _ = config_manager
                                .update_param(ConfigPath::SimWarpFilter, (*n).to_string().into());
                        }
                    }
                })
                .response
                .on_hover_text(t!("sim_panel.warp_filter_tip"));
            if !w.is_identity() && ui.small_button(t!("sim_panel.warp_reset").as_ref()).clicked() {
                let id = SimWarp::default();
                let changes = vec![
                    (ConfigPath::SimWarpZoom, id.zoom.into()),
                    (ConfigPath::SimWarpRotation, id.rotation.into()),
                    (ConfigPath::SimWarpPanX, id.pan_x.into()),
                    (ConfigPath::SimWarpPanY, id.pan_y.into()),
                    (ConfigPath::SimWarpFlow, id.flow.into()),
                ];
                let _ = config_manager.update_batch(changes, "Reset simulation warp".to_string());
            }
        });
    });

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

    // ---- Matte ----
    // Which cells are figure and which are background. Under
    // Colouring because that is what it is: the field is untouched.
    let m = sim.matte;
    ui.collapsing(t!("sim_panel.matte").as_ref(), |ui| {
        ui.label(egui::RichText::new(t!("sim_panel.matte_tip")).small().weak());
        ui.horizontal(|ui| {
            ui.label(t!("sim_panel.matte_channel").as_ref());
            egui::ComboBox::from_id_salt("sim_matte_channel")
                .selected_text(m.channel.name())
                .show_ui(ui, |ui| {
                    for n in SimMatteChannel::NAMES {
                        if ui.selectable_label(m.channel.name() == *n, *n).clicked() {
                            let _ = config_manager
                                .update_param(ConfigPath::SimMatteChannel, (*n).to_string().into());
                        }
                    }
                })
                .response
                .on_hover_text(t!("sim_panel.matte_channel_tip"));
        });
        // The rest only means anything once a channel is chosen.
        ui.add_enabled_ui(!m.is_off(), |ui| {
            let mut cut = m.cutoff;
            if ui
                .add(
                    egui::Slider::new(&mut cut, 0.0..=4.0)
                        .text(t!("sim_panel.matte_cutoff").as_ref()),
                )
                .on_hover_text(t!("sim_panel.matte_cutoff_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimMatteCutoff, cut.into());
            }
            let mut soft = m.softness;
            if ui
                .add(
                    egui::Slider::new(&mut soft, 0.0..=2.0)
                        .text(t!("sim_panel.matte_softness").as_ref()),
                )
                .on_hover_text(t!("sim_panel.matte_softness_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimMatteSoftness, soft.into());
            }
            let mut inv = m.invert;
            if ui
                .checkbox(&mut inv, t!("sim_panel.matte_invert").as_ref())
                .on_hover_text(t!("sim_panel.matte_invert_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::SimMatteInvert, inv.into());
            }
            ui.horizontal(|ui| {
                ui.label(t!("sim_panel.matte_edge").as_ref());
                egui::ComboBox::from_id_salt("sim_matte_edge")
                    .selected_text(m.edge.name())
                    .show_ui(ui, |ui| {
                        for n in SimMatteEdge::NAMES {
                            if ui.selectable_label(m.edge.name() == *n, *n).clicked() {
                                let _ = config_manager
                                    .update_param(ConfigPath::SimMatteEdge, (*n).to_string().into());
                            }
                        }
                    })
                    .response
                    .on_hover_text(t!("sim_panel.matte_edge_tip"));
            });
        });
    });
}
