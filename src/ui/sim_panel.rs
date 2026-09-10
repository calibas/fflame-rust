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
    SimBoundary, SimConfig, SimDownscale, SimGrid, SimInit, SimUpscale, SimWarp,
    SimWarpFilter,
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
    /// The step count the timeline has committed the grid to, when the
    /// timeline is driving. `Some` greys the transport -- a picture
    /// that depended on both the playhead and how long Run had been
    /// held would depend on how long the user looked at it -- and puts
    /// the target in the readout so a catch-up is visibly progress
    /// rather than a hang.
    pub timeline_target: Option<u32>,
    /// The timeline is asking for a step count BELOW the grid, under
    /// motion that would restart the run on every frame, so it is
    /// being held (`sim::timeline_target_applies`). The readout says
    /// why, since otherwise a held picture looks like a broken one.
    pub timeline_holding: bool,
    /// The timeline owns the grid: greys the transport. True whenever
    /// `timeline_target` is `Some`, and ALSO while a backward target is
    /// being held, when there is no target and the grid must still not
    /// be moved by a button.
    pub timeline_driven: bool,
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

    // ---- A way in, not a toggle ----
    // Same as the escape panel: the Mode menu owns switching, so this
    // only enters.
    if !active {
        if ui
            .add(egui::Button::new(t!("sim_panel.toggle_on").as_ref()))
            .on_hover_text(t!("sim_panel.toggle_tip"))
            .clicked()
        {
            if let Err(e) =
                super::render_mode::switch_render_mode(config_manager, RenderMode::Simulation)
            {
                log::error!("Failed to switch render mode: {e}");
            } else {
                *state.reseed = true;
                workspace_request.replace(super::workspace::WorkspaceLayout::Simulation);
            }
        }
        ui.separator();
        ui.label(t!("sim_panel.inactive_hint"));
        return;
    }

    ui.separator();

    // ---- Transport ----
    // The section escape has no analogue for. A simulation's picture is
    // "the state at step N", so the counter is not decoration: it is
    // what makes a still identifiable.
    // While the timeline drives the step count the transport is
    // inert, so it is greyed rather than left looking live.
    let driven = state.timeline_driven;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!driven, |ui| {
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
        if driven {
            ui.label("⏱").on_hover_text(t!("sim_panel.timeline_owns_steps").as_ref());
        }
    });
    // Three readouts, because "step 340" alone cannot tell a run that
    // has arrived from one still walking toward a target from one that
    // is deliberately holding.
    let readout = match state.timeline_target {
        Some(target) if target < state.step_index => t!(
            "sim_panel.step_readout_restarting",
            target = target.to_string(),
            width = state.grid.0.to_string(),
            height = state.grid.1.to_string()
        ),
        Some(target) if target != state.step_index => t!(
            "sim_panel.step_readout_seeking",
            step = state.step_index.to_string(),
            target = target.to_string(),
            width = state.grid.0.to_string(),
            height = state.grid.1.to_string()
        ),
        _ => t!(
            "sim_panel.step_readout",
            step = state.step_index.to_string(),
            width = state.grid.0.to_string(),
            height = state.grid.1.to_string()
        ),
    };
    ui.label(readout.as_ref());
    if state.timeline_holding {
        ui.label(
            egui::RichText::new(t!("sim_panel.timeline_holding").as_ref())
                .small()
                .italics(),
        );
    }

    ui.separator();

    // Everything below the transport scrolls; the transport itself
    // does not. This is the panel's ONE scroll area -- egui_dock's was
    // turned off for this tab in `panel_viewer::scroll_bars`, and a
    // second one here would be a scrollbar inside a scrollbar with two
    // competing drag targets.
    //
    // `id_salt` because this `Ui`'s id derives from the dock tab, so a
    // second scroll area added to this panel later would collide.
    egui::ScrollArea::vertical()
        .id_salt("sim_panel_body")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::CollapsingHeader::new(t!("sim_panel.section_model").as_ref())
                .default_open(true)
                .show(ui, |ui| {
                // ---- Model: one list whose entry 0 IS the model ----
                render_model_section(ui, config_manager, &config, &sim, state.reseed);

                ui.separator();
            });

            egui::CollapsingHeader::new(t!("sim_panel.section_settings").as_ref())
                .default_open(false)
                .show(ui, |ui| {
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
                    ui.label(t!("sim_panel.fit").as_ref());
                    egui::ComboBox::from_id_salt("sim_fit")
                        .selected_text(sim.fit.name())
                        .show_ui(ui, |ui| {
                            for n in crate::config::sim::SimFit::NAMES {
                                if ui.selectable_label(sim.fit.name() == *n, *n).clicked() {
                                    let _ = config_manager
                                        .update_param(ConfigPath::SimFit, (*n).to_string().into());
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
                        // Ten million: the coupled-Brusselator and Rossler papers
                        // settle over 10^5 to 10^6 steps at their dt.
                        egui::Slider::new(&mut steps, 0..=10_000_000)
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
                    .add(
                        egui::Slider::new(&mut spf, 1..=2048)
                            .text(t!("sim_panel.steps_per_frame").as_ref())
                            .logarithmic(true),
                    )
                    .on_hover_text(t!("sim_panel.steps_per_frame_tip"))
                    .changed()
                {
                    let _ = config_manager.update_param(ConfigPath::SimStepsPerFrame, spf.into());
                }
                // An automaton advances by a generation, not by dt: showing the
                // slider would be a control that does nothing.
                let mut dt = sim.dt;
                if sim.any_layer_has_a_time_step() {
                    // The range is the model's STATIC ceiling, never the
                    // parameter-dependent stability cap. A range that moved with
                    // the other sliders moved this slider's handle -- and, once
                    // egui clamped the value into it, the stored dt too -- so
                    // dragging Mobility appeared to edit the time step.
                    if ui
                        .add(
                            egui::Slider::new(&mut dt, 0.001..=sim.max_dt_ceiling())
                                .text(t!("sim_panel.dt").as_ref()),
                        )
                        .on_hover_text(t!("sim_panel.dt_tip"))
                        .changed()
                    {
                        let _ = config_manager.update_param(ConfigPath::SimDt, dt.into());
                    }
                    // What the solver will actually use. Capping silently would
                    // leave the panel claiming a step the run does not take.
                    let effective = sim.effective_max_dt();
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
            });

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
                    ui.label(t!("sim_panel.warp_mode").as_ref());
                    egui::ComboBox::from_id_salt("sim_warp_mode")
                        .selected_text(w.mode.name())
                        .show_ui(ui, |ui| {
                            for n in crate::config::sim::SimWarpMode::NAMES {
                                if ui.selectable_label(w.mode.name() == *n, *n).clicked() {
                                    let _ = config_manager
                                        .update_param(ConfigPath::SimWarpMode, (*n).to_string().into());
                                }
                            }
                        })
                        .response
                        .on_hover_text(t!("sim_panel.warp_mode_tip"));
                    if w.mode == crate::config::sim::SimWarpMode::Continuous {
                        ui.horizontal(|ui| {
                            ui.label(t!("sim_panel.warp_layers").as_ref());
                            for (bit, name) in ["x", "y", "z", "w"].iter().enumerate() {
                                let mut on = w.layers & (1 << bit) != 0;
                                if ui.checkbox(&mut on, *name).on_hover_text(t!("sim_panel.warp_layers_tip")).changed() {
                                    let mask = if on { w.layers | (1 << bit) } else { w.layers & !(1 << bit) };
                                    let _ = config_manager
                                        .update_param(ConfigPath::SimWarpLayers, (mask as i32).into());
                                }
                            }
                        });
                    }
                    if w.mode == crate::config::sim::SimWarpMode::Octaves {
                        let mut cull = w.cull;
                        if ui
                            .checkbox(&mut cull, t!("sim_panel.warp_cull").as_ref())
                            .on_hover_text(t!("sim_panel.warp_cull_tip"))
                            .changed()
                        {
                            let _ = config_manager.update_param(ConfigPath::SimWarpCull, cull.into());
                        }
                    }
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

            egui::CollapsingHeader::new(t!("sim_panel.section_coloring").as_ref())
                .default_open(true)
                .show(ui, |ui| {
                // ---- Colouring: one list whose entry 0 IS the colouring ----
                render_coloring_section(ui, config_manager, &config, &sim);
            });

    });
}

/// Where a colouring's controls write: the flat `coloring` /
/// `coloring_params` / `matte` fields, or `color_layers[k]`.
///
/// The twin of `LayerSlot`. Note the asymmetry it hides: a layer's
/// colouring NAME and the discrete parts of its matte are snapshot
/// edits, while the flat ones are `ConfigPath` writes -- the
/// animatable fields (parameters, cutoff, softness, opacity) are paths
/// on both sides.
#[derive(Clone, Copy, PartialEq)]
enum ColorSlot {
    Flat,
    At(usize),
}

impl ColorSlot {
    fn param_path(self, param: &str) -> ConfigPath {
        match self {
            ColorSlot::Flat => ConfigPath::SimColoringParam { param: param.to_string() },
            ColorSlot::At(index) => ConfigPath::SimColorLayerParam { index, param: param.to_string() },
        }
    }

    fn cutoff_path(self) -> ConfigPath {
        match self {
            ColorSlot::Flat => ConfigPath::SimMatteCutoff,
            ColorSlot::At(index) => ConfigPath::SimColorLayerMatteCutoff { index },
        }
    }

    fn softness_path(self) -> ConfigPath {
        match self {
            ColorSlot::Flat => ConfigPath::SimMatteSoftness,
            ColorSlot::At(index) => ConfigPath::SimColorLayerMatteSoftness { index },
        }
    }

    fn salt(self) -> String {
        match self {
            ColorSlot::Flat => "sim_coloring".to_string(),
            ColorSlot::At(k) => format!("sim_color_layer_{k}"),
        }
    }
}

/// The Colouring section: one list whose entry 0 is the colouring.
///
/// "Colouring" and "Colouring layers" used to be separate sections
/// with the same relationship Model and Layers had -- the single one
/// ignored once a stack existed, the stack empty until it did. Entry 0
/// reads the flat fields until a second is added, which promotes it
/// into `color_layers[0]`; removing back to one demotes it.
///
/// Unlike the model side this promotion switches which SHADER is
/// assembled (`assemble_color` against `assemble_color_stack`). A
/// one-layer Normal stack at opacity 1 is the single colouring's
/// picture bit for bit, which is what makes it safe, and
/// `a_single_normal_colour_layer_is_the_single_colouring` is the test
/// that says so.
fn render_coloring_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    config: &crate::config::FractalConfig,
    sim: &SimConfig,
) {
    use crate::config::sim::{SimBlend, SimColorLayer, SimMatteChannel, SimMatteEdge, MAX_COLOR_LAYERS};
    let structural = |config_manager: &mut ConfigManager, edit: &dyn Fn(&mut SimConfig)| {
        let mut after = config.clone();
        edit(&mut after.sim);
        let _ = config_manager.load_config(after, "history.action.sim_color_layers".to_string());
    };
    let layered = !sim.color_layers.is_empty();
    let count = if layered { sim.color_layers.len() } else { 1 };
    let n_sim_layers = sim.layer_count();

    // Top of the stack first, as a layer panel reads. With one entry
    // the order is moot.
    let order: Vec<usize> = if layered { (0..count).rev().collect() } else { vec![0] };
    for k in order {
        let slot = if layered { ColorSlot::At(k) } else { ColorSlot::Flat };
        let entry = sim.color_layers.get(k);
        let coloring = crate::sim::coloring_or_default(
            entry.map(|l| l.coloring.as_str()).unwrap_or(sim.coloring.as_str()),
        );
        let matte = entry.map(|l| l.matte).unwrap_or(sim.matte);
        let mut action: Option<Box<dyn Fn(&mut SimConfig)>> = None;

        ui.horizontal(|ui| {
            // With one entry this is the Colouring row it has always
            // been; with several it is entry k of a stack.
            let label = if count > 1 {
                t!("sim_panel.color_layer_label", n = k.to_string())
            } else {
                t!("sim_panel.coloring")
            };
            ui.label(label.as_ref());
            if let Some(l) = entry {
                let mut on = l.enabled;
                if ui.checkbox(&mut on, "").on_hover_text(t!("sim_panel.layer_enabled_tip")).changed() {
                    action = Some(Box::new(move |s: &mut SimConfig| {
                        if let Some(l) = s.color_layers.get_mut(k) {
                            l.enabled = on;
                        }
                    }));
                }
            }
            egui::ComboBox::from_id_salt(format!("{}_pick", slot.salt()))
                .selected_text(coloring.display_name)
                .show_ui(ui, |ui| {
                    for c in COLORINGS {
                        if ui
                            .selectable_label(c.name == coloring.name, c.display_name)
                            .on_hover_text(c.description)
                            .clicked()
                            && c.name != coloring.name
                        {
                            match slot {
                                ColorSlot::Flat => {
                                    let _ = config_manager.update_param(
                                        ConfigPath::SimColoring,
                                        c.name.to_string().into(),
                                    );
                                }
                                ColorSlot::At(i) => {
                                    let name = c.name.to_string();
                                    action = Some(Box::new(move |s: &mut SimConfig| {
                                        if let Some(l) = s.color_layers.get_mut(i) {
                                            l.coloring = name.clone();
                                            // Parameters are keyed by
                                            // name for whichever
                                            // colouring is current, so
                                            // keeping them would feed
                                            // one colouring's numbers
                                            // to another.
                                            l.coloring_params.clear();
                                        }
                                    }));
                                }
                            }
                        }
                    }
                });
            if layered {
                if n_sim_layers > 1 {
                    let source = entry.map(|l| l.source).unwrap_or(0);
                    egui::ComboBox::from_id_salt(format!("{}_source", slot.salt()))
                        .selected_text(t!("sim_panel.layer_label", n = source.to_string()).as_ref())
                        .show_ui(ui, |ui| {
                            for l in 0..n_sim_layers {
                                if ui
                                    .selectable_label(l == source, t!("sim_panel.layer_label", n = l.to_string()).as_ref())
                                    .clicked()
                                {
                                    action = Some(Box::new(move |s: &mut SimConfig| {
                                        if let Some(cl) = s.color_layers.get_mut(k) {
                                            cl.source = l;
                                        }
                                    }));
                                }
                            }
                        })
                        .response
                        .on_hover_text(t!("sim_panel.color_layer_source_tip"));
                    let mut gather = entry.map(|l| l.gather).unwrap_or(false);
                    if ui
                        .checkbox(&mut gather, t!("sim_panel.color_layer_gather").as_ref())
                        .on_hover_text(t!("sim_panel.color_layer_gather_tip"))
                        .changed()
                    {
                        action = Some(Box::new(move |s: &mut SimConfig| {
                            if let Some(cl) = s.color_layers.get_mut(k) {
                                cl.gather = gather;
                            }
                        }));
                    }
                }
                if k + 1 < count && ui.small_button("▲").on_hover_text(t!("sim_panel.color_layer_up_tip")).clicked() {
                    action = Some(Box::new(move |s: &mut SimConfig| s.color_layers.swap(k, k + 1)));
                }
                if k > 0 && ui.small_button("▼").on_hover_text(t!("sim_panel.color_layer_down_tip")).clicked() {
                    action = Some(Box::new(move |s: &mut SimConfig| s.color_layers.swap(k, k - 1)));
                }
                if ui.small_button(t!("sim_panel.remove").as_ref()).clicked() {
                    action = Some(Box::new(move |s: &mut SimConfig| {
                        s.color_layers.remove(k);
                        // Back to one colouring: fold it into the flat
                        // fields, the exact inverse of promotion.
                        s.demote_single_color_layer();
                    }));
                }
            }
        });
        if let Some(edit) = action {
            structural(config_manager, &*edit);
            return;
        }

        let mut body = |ui: &mut egui::Ui, config_manager: &mut ConfigManager| {
            let mut action: Option<Box<dyn Fn(&mut SimConfig)>> = None;
            if let Some(l) = entry {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(format!("{}_blend", slot.salt()))
                        .selected_text(l.blend.name())
                        .show_ui(ui, |ui| {
                            for name in SimBlend::NAMES {
                                if ui.selectable_label(l.blend.name() == *name, *name).clicked() {
                                    if let Some(b) = SimBlend::from_name(name) {
                                        action = Some(Box::new(move |s: &mut SimConfig| {
                                            if let Some(cl) = s.color_layers.get_mut(k) {
                                                cl.blend = b;
                                            }
                                        }));
                                    }
                                }
                            }
                        })
                        .response
                        .on_hover_text(t!("sim_panel.color_layer_blend_tip"));
                    let mut opacity = l.opacity;
                    if ui
                        .add(egui::Slider::new(&mut opacity, 0.0..=1.0).text(t!("sim_panel.color_layer_opacity").as_ref()))
                        .changed()
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::SimColorLayerOpacity { index: k }, opacity.into());
                    }
                });
            }
            let params = entry.map(|l| &l.coloring_params).unwrap_or(&sim.coloring_params);
            for p in coloring.parameters {
                let mut v = params.get(p.name).copied().unwrap_or(p.default);
                if param_control(ui, &mut v, p, &slot.salt()) {
                    let _ = config_manager.update_param(slot.param_path(p.name), v.into());
                }
            }
            // The matte: channel, invert and edge are snapshot edits on
            // a layer and paths on the flat fields; cutoff and softness
            // animate on both.
            // Collapsed by default, as it always has been: most
            // colourings never need a matte, and the four controls
            // under it are disabled until a channel is chosen.
            egui::CollapsingHeader::new(t!("sim_panel.matte").as_ref())
                .id_salt(format!("{}_matte_section", slot.salt()))
                .show(ui, |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(format!("{}_matte", slot.salt()))
                    .selected_text(matte.channel.name())
                    .show_ui(ui, |ui| {
                        for name in SimMatteChannel::NAMES {
                            if ui.selectable_label(matte.channel.name() == *name, *name).clicked() {
                                if let Some(c) = SimMatteChannel::from_name(name) {
                                    match slot {
                                        ColorSlot::Flat => {
                                            let _ = config_manager.update_param(
                                                ConfigPath::SimMatteChannel,
                                                c.name().to_string().into(),
                                            );
                                        }
                                        ColorSlot::At(i) => {
                                            action = Some(Box::new(move |s: &mut SimConfig| {
                                                if let Some(l) = s.color_layers.get_mut(i) {
                                                    l.matte.channel = c;
                                                }
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .response
                    .on_hover_text(t!("sim_panel.matte_channel_tip"));
            });
            // The rest only means anything once a channel is chosen.
            ui.add_enabled_ui(!matte.is_off(), |ui| {
                let mut cut = matte.cutoff;
                if ui
                    .add(egui::Slider::new(&mut cut, 0.0..=4.0).text(t!("sim_panel.matte_cutoff").as_ref()))
                    .on_hover_text(t!("sim_panel.matte_cutoff_tip"))
                    .changed()
                {
                    let _ = config_manager.update_param(slot.cutoff_path(), cut.into());
                }
                let mut soft = matte.softness;
                if ui
                    .add(egui::Slider::new(&mut soft, 0.0..=2.0).text(t!("sim_panel.matte_softness").as_ref()))
                    .on_hover_text(t!("sim_panel.matte_softness_tip"))
                    .changed()
                {
                    let _ = config_manager.update_param(slot.softness_path(), soft.into());
                }
                let mut inv = matte.invert;
                if ui
                    .checkbox(&mut inv, t!("sim_panel.matte_invert").as_ref())
                    .on_hover_text(t!("sim_panel.matte_invert_tip"))
                    .changed()
                {
                    match slot {
                        ColorSlot::Flat => {
                            let _ = config_manager.update_param(ConfigPath::SimMatteInvert, inv.into());
                        }
                        ColorSlot::At(i) => {
                            action = Some(Box::new(move |s: &mut SimConfig| {
                                if let Some(l) = s.color_layers.get_mut(i) {
                                    l.matte.invert = inv;
                                }
                            }));
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(t!("sim_panel.matte_edge").as_ref());
                    egui::ComboBox::from_id_salt(format!("{}_matte_edge", slot.salt()))
                        .selected_text(matte.edge.name())
                        .show_ui(ui, |ui| {
                            for name in SimMatteEdge::NAMES {
                                if ui.selectable_label(matte.edge.name() == *name, *name).clicked() {
                                    if let Some(e) = SimMatteEdge::from_name(name) {
                                        match slot {
                                            ColorSlot::Flat => {
                                                let _ = config_manager.update_param(
                                                    ConfigPath::SimMatteEdge,
                                                    e.name().to_string().into(),
                                                );
                                            }
                                            ColorSlot::At(i) => {
                                                action = Some(Box::new(move |s: &mut SimConfig| {
                                                    if let Some(l) = s.color_layers.get_mut(i) {
                                                        l.matte.edge = e;
                                                    }
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        })
                        .response
                        .on_hover_text(t!("sim_panel.matte_edge_tip"));
                });
            });
                });
            action
        };
        let pending = if count > 1 {
            ui.indent(format!("{}_body", slot.salt()), |ui| body(ui, config_manager)).inner
        } else {
            body(ui, config_manager)
        };
        if let Some(edit) = pending {
            structural(config_manager, &*edit);
            return;
        }
    }

    if count < MAX_COLOR_LAYERS
        && ui
            .button(t!("sim_panel.add_color_layer").as_ref())
            .on_hover_text(t!("sim_panel.add_color_layer_first_tip"))
            .clicked()
    {
        structural(config_manager, &|s: &mut SimConfig| {
            // Make entry 0 explicit, then add one over it.
            s.promote_coloring_to_layers();
            s.color_layers.push(SimColorLayer::default());
        });
    }
    if layered
        && ui
            .small_button(t!("sim_panel.color_layers_flatten").as_ref())
            .on_hover_text(t!("sim_panel.color_layers_flatten_tip"))
            .clicked()
    {
        // Keep the bottom entry, drop the rest.
        structural(config_manager, &|s: &mut SimConfig| {
            s.color_layers.truncate(1);
            s.demote_single_color_layer();
        });
    }
}

/// The Layers and Couplings lists. Per-field edits go through
/// ConfigPath; adding or removing a layer or a coupling is a
/// structural edit and goes through a full-config snapshot, which
/// undoes as one step.
/// Where a layer's controls write.
///
/// `SimConfig` keeps a flat `model` / `model_params` pair AND a
/// `layers` list, and `layer_model_name(0)` already falls back from
/// one to the other -- so layer 0 has always *been* the model, the
/// panel just did not say so. This names the two homes for one row of
/// controls, which is what lets the merged list draw entry 0 the same
/// way whichever holds it.
#[derive(Clone, Copy, PartialEq)]
enum LayerSlot {
    /// No `layers` list: layer 0 is the flat `model` fields.
    Flat,
    /// `layers[i]`.
    At(usize),
}

impl LayerSlot {
    fn model_path(self) -> ConfigPath {
        match self {
            LayerSlot::Flat => ConfigPath::SimModel,
            LayerSlot::At(layer) => ConfigPath::SimLayerModel { layer },
        }
    }

    fn param_path(self, param: &str) -> ConfigPath {
        match self {
            LayerSlot::Flat => ConfigPath::SimModelParam { param: param.to_string() },
            LayerSlot::At(layer) => ConfigPath::SimLayerParam { layer, param: param.to_string() },
        }
    }

    fn salt(self) -> String {
        match self {
            LayerSlot::Flat => "sim_model".to_string(),
            LayerSlot::At(i) => format!("sim_layer_{i}"),
        }
    }
}

/// Everything a model preset sets, aimed at one slot.
///
/// A preset is a whole recipe, not a parameter set: its measured step
/// count, its per-frame count, the model's dt, the initial field, the
/// colouring it is meant to be seen through and what it calls empty
/// space. Only the model parameters are per-layer; the rest are the
/// simulation's, which is why applying layer 1's preset restyles the
/// whole picture. That is accepted -- a preset that set only numbers
/// would show the pattern half-formed.
fn preset_changes(
    model: &'static crate::sim::ModelDef,
    pre: &'static crate::sim::SimPreset,
    slot: LayerSlot,
) -> Vec<(ConfigPath, crate::config::delta::ConfigValue)> {
    let mut changes: Vec<(ConfigPath, crate::config::delta::ConfigValue)> = pre
        .params
        .iter()
        .map(|(k, v)| (slot.param_path(k), (*v).into()))
        .collect();
    changes.push((ConfigPath::SimSteps, pre.steps.into()));
    // A per-frame count that reaches the preset's picture in about two
    // hundred frames: at the default of 4 a 200,000-step run at dt
    // 0.001 was fourteen minutes of the uniform fixed point.
    if pre.steps > 0 {
        changes.push((ConfigPath::SimStepsPerFrame, (pre.steps / 200).clamp(4, 2048).into()));
    }
    // Lenia is the sharp case: it runs at 0.1 and dies at 1.
    if !model.has(crate::sim::ModelFeature::NoTimeStep) {
        changes.push((ConfigPath::SimDt, model.default_dt.into()));
    }
    // FitzHugh-Nagumo's constants give spirals from a cut wavefront and
    // a flat field from noise, so applying only the numbers would ship
    // a picture of nothing.
    if let Some(init) = pre.init {
        changes.push((ConfigPath::SimInitKind, init.kind_name().to_string().into()));
    }
    // Which colouring a model wants is a property of its state layout,
    // not a taste the user should have to acquire.
    if let Some(c) = pre.coloring {
        changes.push((ConfigPath::SimColoring, c.to_string().into()));
        for (k, v) in pre.coloring_params {
            changes.push((ConfigPath::SimColoringParam { param: (*k).to_string() }, (*v).into()));
        }
    }
    // And what the model calls empty space. Set either way, so a preset
    // that wants no matte clears one the last preset set.
    let matte = pre.matte.unwrap_or_default();
    changes.push((ConfigPath::SimMatteChannel, matte.channel.name().to_string().into()));
    changes.push((ConfigPath::SimMatteCutoff, matte.cutoff.into()));
    changes.push((ConfigPath::SimMatteSoftness, matte.softness.into()));
    changes.push((ConfigPath::SimMatteInvert, matte.invert.into()));
    // And the warp, for the presets whose subject is an inflating
    // space. Set either way, as the matte is.
    let warp = pre.warp.unwrap_or_default();
    changes.push((ConfigPath::SimWarpZoom, warp.zoom.into()));
    changes.push((ConfigPath::SimWarpRotation, warp.rotation.into()));
    changes.push((ConfigPath::SimWarpPanX, warp.pan_x.into()));
    changes.push((ConfigPath::SimWarpPanY, warp.pan_y.into()));
    changes.push((ConfigPath::SimWarpFlow, warp.flow.into()));
    changes.push((ConfigPath::SimWarpFilter, warp.filter.name().to_string().into()));
    changes.push((ConfigPath::SimWarpMode, warp.mode.name().to_string().into()));
    changes.push((ConfigPath::SimWarpCull, warp.cull.into()));
    changes.push((ConfigPath::SimWarpLayers, (warp.layers as i32).into()));
    changes
}

/// The Model section: one list whose entry 0 is the primary model.
///
/// "Model" and "Layers" used to be separate sections that each did
/// nothing in the other's case -- Model was ignored once layers
/// existed, Layers was empty until they did. They are one list now.
/// Entry 0 reads the flat fields until a second layer is added, which
/// promotes it into `layers[0]`; removing back to one demotes it. The
/// seam is invisible, undoes as one step, and
/// `promotion_then_demotion_round_trips_the_config` is what keeps it
/// exact.
fn render_model_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    config: &crate::config::FractalConfig,
    sim: &SimConfig,
    reseed: &mut bool,
) {
    use crate::config::sim::{
        SimCoupling, SimCouplingForm, SimLayer, MAX_COUPLINGS, MAX_LAYERS,
    };
    let structural = |config_manager: &mut ConfigManager, edit: &dyn Fn(&mut SimConfig)| {
        let mut after = config.clone();
        edit(&mut after.sim);
        let _ = config_manager.load_config(after, "history.action.sim_layers".to_string());
    };
    let layered = !sim.layers.is_empty();
    let count = sim.layer_count();

    // What makes the flame's transforms the layers' per-layer warps.
    // It lived inside the old Layers header and was dropped when that
    // header was merged away; `the_panel_writes_every_sim_path` is the
    // test that found it.
    let mut use_transforms = sim.use_transforms;
    if ui
        .checkbox(&mut use_transforms, t!("sim_panel.use_transforms").as_ref())
        .on_hover_text(t!("sim_panel.use_transforms_tip"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::SimUseTransforms, use_transforms.into());
    }
    ui.horizontal(|ui| {
        ui.label(t!("sim_panel.layered_presets").as_ref());
        egui::ComboBox::from_id_salt("sim_layered_preset")
            .selected_text(t!("sim_panel.layered_preset_pick").as_ref())
            .show_ui(ui, |ui| {
                for p in crate::sim::LAYERED_PRESETS {
                    if ui.selectable_label(false, p.display_name).on_hover_text(p.description).clicked() {
                        structural(config_manager, &|s: &mut SimConfig| p.apply(s));
                    }
                }
            });
    });

    for i in 0..count {
        let slot = if layered { LayerSlot::At(i) } else { LayerSlot::Flat };
        let model = crate::sim::model_or_default(sim.layer_model_name(i));
        let mut remove = false;
        ui.horizontal(|ui| {
            // With one layer this is the Model row it has always been;
            // with several it is entry i of a list.
            let label = if count > 1 {
                t!("sim_panel.layer_label", n = i.to_string())
            } else {
                t!("sim_panel.model")
            };
            ui.label(label.as_ref());
            egui::ComboBox::from_id_salt(format!("{}_model", slot.salt()))
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
                                .update_param(slot.model_path(), m.name.to_string().into());
                        }
                    }
                });
            if count > 1 {
                let mut on = sim.layer_enabled(i);
                if ui
                    .checkbox(&mut on, t!("sim_panel.layer_enabled").as_ref())
                    .on_hover_text(t!("sim_panel.layer_enabled_tip"))
                    .changed()
                {
                    let _ = config_manager
                        .update_param(ConfigPath::SimLayerEnabled { layer: i }, on.into());
                }
                if ui.small_button(t!("sim_panel.remove").as_ref()).clicked() {
                    remove = true;
                }
            }
        });
        if remove {
            structural(config_manager, &|s: &mut SimConfig| {
                s.layers.remove(i);
                // Couplings that named the layer go; the rest renumber
                // past it.
                s.couplings.retain(|c| c.from != i && c.to != i);
                for c in &mut s.couplings {
                    if c.from > i {
                        c.from -= 1;
                    }
                    if c.to > i {
                        c.to -= 1;
                    }
                }
                // Back to one system: fold the layer into the flat
                // fields, the exact inverse of promotion.
                s.demote_single_layer();
            });
            return;
        }
        let mut body = |ui: &mut egui::Ui, config_manager: &mut ConfigManager| {
            if !model.presets.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!("sim_panel.presets").as_ref());
                    for pre in model.presets {
                        if ui.small_button(pre.display_name).clicked() {
                            let _ = config_manager.update_batch(
                                preset_changes(model, pre, slot),
                                "history.action.sim_preset".to_string(),
                            );
                            *reseed = true;
                        }
                    }
                });
            }
            let params = sim.layer_model_params(i);
            for p in model.parameters.iter() {
                let mut v = params.get(p.name).copied().unwrap_or(p.default);
                if param_control(ui, &mut v, p, &slot.salt()) {
                    let _ = config_manager.update_param(slot.param_path(p.name), v.into());
                }
            }
        };
        if count > 1 {
            ui.indent(format!("{}_body", slot.salt()), |ui| body(ui, config_manager));
        } else {
            body(ui, config_manager);
        }
    }

    if count < MAX_LAYERS
        && ui
            .button(t!("sim_panel.add_layer").as_ref())
            .on_hover_text(t!("sim_panel.add_layer_tip"))
            .clicked()
    {
        let model = sim.model.clone();
        let last = sim.layers.last().map(|l| l.model.clone());
        structural(config_manager, &|s: &mut SimConfig| {
            // Make entry 0 explicit, then add one after it. The new
            // layer takes the same model but the registry's default
            // parameters -- a copy of layer 0 would step identically
            // and look like nothing had happened.
            s.promote_model_to_layers();
            let next = last.clone().unwrap_or_else(|| model.clone());
            s.layers.push(SimLayer {
                model: next,
                model_params: Default::default(),
                enabled: true,
            });
        });
    }

    // ---- Couplings: only meaningful between two layers or more ----
    if sim.layers.len() < 2 {
        return;
    }
    egui::CollapsingHeader::new(t!("sim_panel.couplings").as_ref())
        .default_open(!sim.couplings.is_empty())
        .show(ui, |ui| {
            ui.label(egui::RichText::new(t!("sim_panel.couplings_tip")).small().weak());
            let n = sim.layers.len();
            for (i, c) in sim.couplings.iter().enumerate() {
                let mut remove = false;
                ui.horizontal(|ui| {
                    let layer_combo = |ui: &mut egui::Ui, salt: String, value: usize, path: ConfigPath, config_manager: &mut ConfigManager| {
                        egui::ComboBox::from_id_salt(salt)
                            .selected_text(t!("sim_panel.layer_label", n = value.to_string()).as_ref())
                            .show_ui(ui, |ui| {
                                for l in 0..n {
                                    if ui
                                        .selectable_label(l == value, t!("sim_panel.layer_label", n = l.to_string()).as_ref())
                                        .clicked()
                                    {
                                        let _ = config_manager.update_param(path.clone(), (l as i32).into());
                                    }
                                }
                            });
                    };
                    layer_combo(ui, format!("sim_coupling_from_{i}"), c.from, ConfigPath::SimCouplingFrom { index: i }, config_manager);
                    ui.label("→");
                    layer_combo(ui, format!("sim_coupling_to_{i}"), c.to, ConfigPath::SimCouplingTo { index: i }, config_manager);
                    egui::ComboBox::from_id_salt(format!("sim_coupling_form_{i}"))
                        .selected_text(c.form.name())
                        .show_ui(ui, |ui| {
                            for name in SimCouplingForm::NAMES {
                                if ui.selectable_label(c.form.name() == *name, *name).clicked() {
                                    let _ = config_manager.update_param(
                                        ConfigPath::SimCouplingForm { index: i },
                                        (*name).to_string().into(),
                                    );
                                }
                            }
                        })
                        .response
                        .on_hover_text(t!("sim_panel.coupling_form_tip"));
                    if ui.small_button(t!("sim_panel.remove").as_ref()).clicked() {
                        remove = true;
                    }
                });
                if remove {
                    structural(config_manager, &|s: &mut SimConfig| {
                        s.couplings.remove(i);
                    });
                    return;
                }
                ui.horizontal(|ui| {
                    let mut strength = c.strength;
                    if ui
                        .add(
                            egui::Slider::new(&mut strength, -2.0..=2.0)
                                .text(t!("sim_panel.coupling_strength").as_ref())
                                .fixed_decimals(3),
                        )
                        .on_hover_text(t!("sim_panel.coupling_strength_tip"))
                        .changed()
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::SimCouplingStrength { index: i }, strength.into());
                    }
                    for (bit, name) in ["x", "y", "z", "w"].iter().enumerate() {
                        let mut on = c.channels & (1 << bit) != 0;
                        if ui.checkbox(&mut on, *name).on_hover_text(t!("sim_panel.coupling_channels_tip")).changed() {
                            let mask = if on { c.channels | (1 << bit) } else { c.channels & !(1 << bit) };
                            let _ = config_manager
                                .update_param(ConfigPath::SimCouplingChannels { index: i }, (mask as i32).into());
                        }
                    }
                });
            }
            if sim.couplings.len() < MAX_COUPLINGS
                && ui.button(t!("sim_panel.add_coupling").as_ref()).clicked()
            {
                structural(config_manager, &|s: &mut SimConfig| {
                    s.couplings.push(SimCoupling::default());
                });
            }
        });
}

#[cfg(test)]
mod locale_tests {
    /// Every `sim_panel.*` key this file asks for must exist in the
    /// English table.
    ///
    /// `t!` returns the KEY when a translation is missing, so a typo
    /// or a forgotten locale entry ships as a label reading
    /// "sim_panel.timeline_holding" and nothing fails. This is also
    /// the file where an edited translation once did not rebuild the
    /// crate at all (`build.rs` now watches `locales/`), so the cheap
    /// check is worth having.
    #[test]
    fn every_key_this_panel_asks_for_exists() {
        let source = include_str!("sim_panel.rs");
        let mut missing: Vec<String> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (i, _) in source.match_indices("t!(\"sim_panel.") {
            let rest = &source[i + 4..];
            let Some(end) = rest.find('"') else { continue };
            let key = &rest[..end];
            if !seen.insert(key.to_string()) {
                continue;
            }
            // The test's own source contains the prefix; skip the
            // literal used in this scan.
            if key == "sim_panel." {
                continue;
            }
            let got = rust_i18n::t!(key, locale = "en");
            if got == key {
                missing.push(key.to_string());
            }
        }
        assert!(
            missing.is_empty(),
            "these sim_panel keys are missing from locales/en.yml: {missing:#?}"
        );
        assert!(seen.len() > 20, "the scan found almost nothing: {}", seen.len());
    }
}
