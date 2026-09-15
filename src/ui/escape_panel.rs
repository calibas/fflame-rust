//! Escape Fractal panel — the whole editing surface for escape-time
//! mode (the flame-only panels stay hidden rather than learning a
//! second vocabulary; see docs/projects/escape-time-fractals.md §3).
//!
//! Every control writes through `config_manager.update_param` with an
//! `Escape*` ConfigPath, so undo/redo, coalescing, animation tracks and
//! script `config.set` all work unmodified. Formula/coloring parameter
//! sliders are generated from the registry defs, the way variation
//! params generate theirs.

use crate::config::escape::{
    ContrastMode, DownsampleMode, ShadingBlend, ShadingField, ShadingTexture,
};
use crate::config::{ConfigManager, ConfigPath, ConfigValue};
use crate::scene::transforms::RenderMode;
use rust_i18n::t;

/// Render the Escape Fractal panel.
pub fn render_escape_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    workspace_request: &mut Option<super::workspace::WorkspaceLayout>,
) {
    let config = config_manager.active_config().clone();
    let esc = config.escape.clone();

    // ---- A way in, not a toggle ----
    //
    // The Mode menu owns switching (ui-render-modes plan, section
    // 3.1), so this button only ENTERS. It used to toggle, and leaving
    // was hardcoded to 3D -- so turning Escape on from 2D and off
    // again left you somewhere you had never been.
    let active = config.render_mode == RenderMode::Escape;
    if !active {
        if ui
            .add(egui::Button::new(t!("escape_panel.toggle_on").as_ref()))
            .on_hover_text(t!("escape_panel.toggle_tip"))
            .clicked()
        {
            if let Err(e) =
                super::render_mode::switch_render_mode(config_manager, RenderMode::Escape)
            {
                log::error!("Failed to switch render mode: {e}");
            } else {
                // Entering: bring the workspace with it.
                workspace_request.replace(super::workspace::WorkspaceLayout::EscapeTime);
            }
        }
        ui.separator();
        ui.label(t!("escape_panel.not_active_hint"));
        return;
    }

    // Whether the picture on screen is finished. An escape render
    // arrives in chunks, and a screenshot of an unsettled frame has
    // been reported as a render bug more than once -- the panel should
    // say which it is rather than leaving the user to guess from the
    // noise level.
    match crate::escape::renderer::render_progress() {
        Some((done, want)) if want > 0 => {
            let pct = (done as f32 / want as f32 * 100.0).clamp(0.0, 100.0);
            ui.label(
                egui::RichText::new(t!(
                    "escape_panel.rendering",
                    percent = format!("{pct:.0}")
                ))
                .small()
                .weak(),
            );
        }
        _ => {
            ui.label(
                egui::RichText::new(t!("escape_panel.settled"))
                    .small()
                    .weak(),
            )
            .on_hover_text(t!("escape_panel.settled_tip"));
        }
    }

    ui.separator();

    // ---- Formula ----
    // Mode B (field) formulas share the dropdown as a second group;
    // which registry resolves the name routes everything downstream.
    let field = crate::escape::fields::get_field(&esc.formula);
    let ifs_def = crate::escape::ifs::get_ifs(&esc.formula);
    let formula = crate::escape::get_formula(&esc.formula);
    let selected_label = match (ifs_def, field) {
        (Some(d), _) => d.display_name,
        (None, Some(f)) => f.display_name,
        (None, None) => formula.display_name,
    };
    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.formula"));
        egui::ComboBox::from_id_salt("escape_formula")
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                for f in crate::escape::FORMULAS {
                    if ui
                        .selectable_label(
                            field.is_none() && f.name == formula.name,
                            f.display_name,
                        )
                        .clicked()
                        && esc.formula != f.name
                    {
                        // Land on the formula's own starting point
                        // rather than inheriting the last one's view.
                        // A centre and zoom chosen for the Mandelbrot
                        // mean nothing over Origami, and the coloring
                        // may not even be able to draw it.
                        match crate::escape::formula_default_preset(f) {
                            Some(p) => {
                                let _ = apply_preset(config_manager, f, p);
                            }
                            None => {
                                let _ = config_manager.update_param(
                                    ConfigPath::EscapeFormula,
                                    ConfigValue::String(f.name.to_string()),
                                );
                            }
                        }
                    }
                }
                ui.separator();
                for f in crate::escape::fields::FIELDS {
                    if ui
                        .selectable_label(
                            field.is_some_and(|sel| sel.name == f.name),
                            f.display_name,
                        )
                        .clicked()
                        && esc.formula != f.name
                    {
                        // Same reasoning as mode A: a field's natural
                        // view and TERM COUNT are its own.
                        match crate::escape::field_default_preset(f) {
                            Some(p) => {
                                let _ = apply_field_preset(config_manager, f, p);
                            }
                            None => {
                                let _ = config_manager.update_param(
                                    ConfigPath::EscapeFormula,
                                    ConfigValue::String(f.name.to_string()),
                                );
                            }
                        }
                    }
                }
                // Mode D: distance functions. `ifs_flame` reads the
                // LOADED FLAME rather than a formula of its own, so
                // there is no default view to land on — the flame's
                // own extent decides where to stand, and the panel
                // offers a Frame button for it below.
                ui.separator();
                for d in crate::escape::ifs::IFS_DEFS {
                    if ui
                        .selectable_label(
                            ifs_def.is_some_and(|sel| sel.name == d.name),
                            d.display_name,
                        )
                        .clicked()
                        && esc.formula != d.name
                    {
                        let _ = config_manager.update_batch(
                            vec![
                                (
                                    ConfigPath::EscapeFormula,
                                    ConfigValue::String(d.name.to_string()),
                                ),
                                (
                                    ConfigPath::EscapeColoring,
                                    ConfigValue::String(d.default_coloring.to_string()),
                                ),
                            ],
                            "history.param.escape_formula".to_string(),
                        );
                    }
                }
            });
    });

    // ---- Mode D: does the loaded flame qualify? ----
    //
    // The criterion answers *why not* rather than *whether* (the
    // analysis returns every reason, not the first), so the panel can
    // list them: a flame with two non-affine transforms should say so
    // once, not make the user fix one to discover the next.
    if let Some(d) = ifs_def {
        if d.needs_flame {
            show_ifs_criterion(ui, config_manager, d.solid);
        }
        if d.solid {
            show_solid_camera(ui, config_manager, &esc);
        }
    }

    // ---- Presets ----
    //
    // The formula picks the mathematics; the preset picks a place to
    // stand in it. Kept as a separate row because re-applying one is
    // a normal thing to want after wandering off, not only something
    // that happens on a formula switch.
    {
        let presets: &[crate::escape::EscapePreset] = match (ifs_def, field) {
            (Some(d), _) => d.presets,
            (None, Some(f)) => f.presets,
            (None, None) => crate::escape::get_formula(&esc.formula).presets,
        };
        if !presets.is_empty() {
            ui.horizontal(|ui| {
                ui.label(t!("escape_panel.preset"));
                egui::ComboBox::from_id_salt("escape_preset")
                    .selected_text(t!("escape_panel.preset_pick"))
                    .show_ui(ui, |ui| {
                        for p in presets {
                            if ui.selectable_label(false, p.name).clicked() {
                                match field {
                                    Some(f) => {
                                        let _ = apply_field_preset(config_manager, f, p);
                                    }
                                    None => {
                                        let _ = apply_preset(
                                            config_manager,
                                            crate::escape::get_formula(&esc.formula),
                                            p,
                                        );
                                    }
                                }
                            }
                        }
                    });
            })
            .response
            .on_hover_text(t!("escape_panel.preset_tip"));
        }
    }

    // How deep this formula goes, said out loud. 17 of the 23
    // formulas stop resolving around zoom 14 -- the direct path's f32
    // pixel mapping runs out -- and nothing in the panel used to say
    // so, which leaves the user zooming into a flat wash with no way
    // to tell a limitation from a bug.
    match crate::escape::EscapeRenderer::usable_depth(&esc) {
        crate::escape::UsableDepth::Perturbed => {
            ui.label(
                egui::RichText::new(t!("escape_panel.depth_deep"))
                    .small()
                    .weak(),
            )
            .on_hover_text(t!("escape_panel.depth_deep_tip"));
        }
        crate::escape::UsableDepth::Direct(limit) => {
            let past = esc.zoom_log2 > limit;
            let text = egui::RichText::new(t!(
                "escape_panel.depth_direct",
                zoom = format!("{:.1}", limit * std::f64::consts::LOG10_2)
            ))
            .small();
            ui.label(if past { text.color(egui::Color32::from_rgb(220, 160, 60)) } else { text.weak() })
                .on_hover_text(t!("escape_panel.depth_direct_tip"));
            if past {
                ui.label(
                    egui::RichText::new(t!("escape_panel.depth_exceeded"))
                        .small()
                        .color(egui::Color32::from_rgb(220, 160, 60)),
                );
            }
        }
    }

    // Formula parameters, straight from the def (slider bounds and
    // tooltips included). Values read def defaults when unset — the
    // same value the shader's packer uses.
    let formula_params = match (ifs_def, field) {
        (Some(d), _) => d.parameters,
        (None, Some(f)) => f.parameters,
        (None, None) => formula.parameters,
    };
    for p in formula_params {
        let mut v = esc.formula_params.get(p.name).copied().unwrap_or(p.default);
        if param_control(ui, &mut v, p, "formula") {
            let _ = config_manager.update_param(
                ConfigPath::EscapeFormulaParam { param: p.name.to_string() },
                v.into(),
            );
        }
    }

    show_lens_section(ui, config_manager);

    // ---- Julia toggle ----
    //
    // Mode A only (fields have no Julia plane), and only where the map
    // HAS a parameter: for Origami, Newton, Collatz and Lattes the
    // pixel is the starting point rather than a parameter, so both
    // planes render the same image and the control is inert. See
    // FormulaFeature::DynamicalOnly.
    let julia_meaningful = field.is_none()
        && ifs_def.is_none()
        && crate::escape::formula_julia_is_meaningful(
            crate::escape::get_formula(&esc.formula),
        );
    let mut julia = esc.julia;
    if julia_meaningful
        && ui
            .checkbox(&mut julia, t!("escape_panel.julia").as_ref())
            .on_hover_text(t!("escape_panel.tooltip_julia"))
            .changed()
    {
        let _ = config_manager.update_param(ConfigPath::EscapeJulia, julia.into());
    }
    if esc.julia && julia_meaningful {
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.julia_seed"));
            let mut re = esc.julia_re;
            if ui
                .add(egui::DragValue::new(&mut re).speed(0.002).prefix("re: "))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::EscapeJuliaRe, re.into());
            }
            let mut im = esc.julia_im;
            if ui
                .add(egui::DragValue::new(&mut im).speed(0.002).prefix("im: "))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::EscapeJuliaIm, im.into());
            }
        });
    }

    ui.separator();

    // ---- View: center (exact decimal strings), zoom exponent, rotation ----
    ui.label(t!("escape_panel.view_heading"));
    for (label, value, path) in [
        ("re", &esc.center_re, ConfigPath::EscapeCenterRe),
        ("im", &esc.center_im, ConfigPath::EscapeCenterIm),
    ] {
        ui.horizontal(|ui| {
            ui.label(format!("{}:", label));
            let mut text = value.clone();
            // The center is an exact decimal string (deep-zoom ready);
            // an unparseable intermediate state falls back to the
            // default center at render time and corrects as you type.
            if ui.text_edit_singleline(&mut text).changed() {
                let _ = config_manager.update_param(path.clone(), ConfigValue::String(text));
            }
        });
    }

    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.zoom_log10"));
        // Shown in base 10 — the unit every other deep-zoom tool
        // reports — while the engine keeps base 2 (see
        // EscapeConfig::zoom_log10 for why the STORED value must).
        let mut z10 = esc.zoom_log10();
        // Same drag feel as before: 0.02 octaves per step, in decades.
        let resp = ui
            .add(
                egui::DragValue::new(&mut z10)
                    .speed(0.02 * std::f64::consts::LOG10_2)
                    .max_decimals(4),
            )
            .on_hover_text(t!("escape_panel.tooltip_zoom_log10"));
        if resp.changed() {
            let z2 = crate::config::escape::EscapeConfig::log10_to_log2(z10);
            let _ = config_manager.update_param(ConfigPath::EscapeZoomLog2, (z2 as f32).into());
        }
        ui.label(egui::RichText::new(magnification_label(esc.zoom_log10())).weak());
    });

    // Newton navigation: locate the minibrot governing the current
    // view and recenter on its nucleus exactly (arbitrary-precision
    // digits). One batch -> one undo point. Eligible formulas match
    // the nucleus references: z^p + c at integer powers, parameter
    // plane. The search runs on a background thread (six-figure
    // periods take seconds) and the result lands on a later frame.
    // The reference-orbit controls are ENGINE INTERNALS: they
    // decide how the deep-zoom machinery finds and reuses a
    // reference, and none of them change what the fractal looks
    // like. Collapsed by default so the panel opens on the
    // controls that do.
    egui::CollapsingHeader::new(t!("escape_panel.reference_section"))
        .id_salt("escape_reference_section")
        .default_open(false)
        .show(ui, |ui| {
        // ---- Reference period (deep dives) ----
        // f3's reference.period: the governing nucleus's period. Verified
        // before use; 0 = none. Detect runs the ball method at the
        // center's intrinsic depth on a background thread (minutes at
        // large periods).
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.reference_period"));
            let mut period = esc.reference_period.unwrap_or(0);
            if ui
                .add(egui::DragValue::new(&mut period).speed(10).range(0..=100_000_000))
                .on_hover_text(t!("escape_panel.tooltip_reference_period"))
                .changed()
            {
                let _ = config_manager
                    .update_param(ConfigPath::EscapeReferencePeriod, ConfigValue::UInt(period));
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let slot = period_search_slot();
                let running = matches!(*slot.lock().unwrap(), PeriodSearch::Running);
                if running {
                    ui.spinner();
                    ui.label(t!("escape_panel.searching_period"));
                    ui.ctx().request_repaint();
                } else if ui
                    .button(t!("escape_panel.detect_period").as_ref())
                    .on_hover_text(t!("escape_panel.tooltip_detect_period"))
                    .clicked()
                {
                    let re = esc.center_re.clone();
                    let im = esc.center_im.clone();
                    let power = match esc.formula.as_str() {
                        "multibrot" => esc
                            .formula_params
                            .get("power")
                            .map(|p| p.round() as u32)
                            .unwrap_or(3),
                        _ => 2,
                    };
                    *slot.lock().unwrap() = PeriodSearch::Running;
                    let out = slot.clone();
                    // The view's depth decides which period is USEFUL: the
                    // smallest closing period is the wrong answer at depth,
                    // because a shallow atom's wrap is not exact there.
                    let zoom = esc.zoom_log2;
                    std::thread::spawn(move || {
                        let found = crate::escape::nucleus::detect_period_for_zoom(
                            &re, &im, power, 8_000_000, zoom,
                        );
                        *out.lock().unwrap() = PeriodSearch::Done(found);
                    });
                }
                let done = {
                    let mut s = slot.lock().unwrap();
                    if let PeriodSearch::Done(found) = &*s {
                        let f = *found;
                        *s = PeriodSearch::Idle;
                        Some(f)
                    } else {
                        None
                    }
                };
                if let Some(found) = done {
                    let note = period_note_slot();
                    match found {
                        Some((p, oct)) => {
                            let _ = config_manager.update_param(
                                ConfigPath::EscapeReferencePeriod,
                                ConfigValue::UInt(p),
                            );
                            // -oct - 16 inverts closure_limit_for_zoom: the
                            // deepest view this wrap stays exact for.
                            *note.lock().unwrap() = Some(
                                t!(
                                    "escape_panel.period_found",
                                    period = p,
                                    octave = -oct,
                                    zoom = format!(
                                        "{:.1}",
                                        (-oct - 16).max(0) as f64 * std::f64::consts::LOG10_2
                                    )
                                )
                                .to_string(),
                            );
                        }
                        None => {
                            log::warn!(
                                "period detection: nothing within 8,000,000 wraps at zoom {:.0}",
                                esc.zoom_log2
                            );
                            *note.lock().unwrap() =
                                Some(t!("escape_panel.period_none").to_string());
                        }
                    }
                }
                if let Some(msg) = period_note_slot().lock().unwrap().as_ref() {
                    ui.label(egui::RichText::new(msg).small().weak());
                }
            }
        });

        // What the renderer is ACTUALLY using. Progressive detection
        // finds its own periods while zooming (and retires them as the
        // view deepens), so this can differ from the field above - and
        // when the field is empty it is the only way to see that a
        // periodic reference is in play at all.
        if let Some(live) = crate::escape::reference::live_reference_period() {
            ui.horizontal(|ui| {
                ui.label(t!("escape_panel.detected_period", period = live))
                    .on_hover_text(t!("escape_panel.tooltip_detected_period"));
                if esc.reference_period.unwrap_or(0) != live
                    && ui
                        .small_button(t!("escape_panel.use_detected_period").as_ref())
                        .clicked()
                {
                    let _ = config_manager.update_param(
                        ConfigPath::EscapeReferencePeriod,
                        ConfigValue::UInt(live),
                    );
                }
            });
        }

        let nav_power: Option<u32> = if esc.julia {
            None
        } else {
            match esc.formula.as_str() {
                "mandelbrot" => Some(2),
                "multibrot" => {
                    let p = esc.formula_params.get("power").copied().unwrap_or(3.0);
                    let r = p.round();
                    if (p - r).abs() < 1e-6 && (2.0..=12.0).contains(&r) {
                        Some(r as u32)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        if let Some(power) = nav_power {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let pending = minibrot_search_slot();
                let in_flight = {
                    let s = pending.lock().unwrap();
                    matches!(*s, MinibrotSearch::Running)
                };
                if in_flight {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(t!("escape_panel.searching_minibrot"));
                    });
                    ui.ctx().request_repaint();
                } else if ui
                    .button(t!("escape_panel.center_minibrot").as_ref())
                    .on_hover_text(t!("escape_panel.tooltip_center_minibrot"))
                    .clicked()
                {
                    let re = esc.center_re.clone();
                    let im = esc.center_im.clone();
                    let zoom = esc.zoom_log2;
                    *pending.lock().unwrap() = MinibrotSearch::Running;
                    let slot = pending.clone();
                    std::thread::spawn(move || {
                        let hit = crate::escape::nucleus::locate_minibrot(
                            &re, &im, zoom, 100_000, power, 100_000,
                        );
                        *slot.lock().unwrap() = MinibrotSearch::Done(hit);
                    });
                }
                // Poll: a finished search applies on this frame.
                let done = {
                    let mut s = pending.lock().unwrap();
                    if matches!(*s, MinibrotSearch::Done(_)) {
                        std::mem::replace(&mut *s, MinibrotSearch::Idle)
                    } else {
                        MinibrotSearch::Idle
                    }
                };
                if let MinibrotSearch::Done(result) = done {
                    match result {
                        Some(hit) => {
                            log::info!(
                                "Minibrot found: period {} at ({}, {})",
                                hit.period,
                                hit.re,
                                hit.im
                            );
                            let _ = config_manager.update_batch(
                                vec![
                                    (ConfigPath::EscapeCenterRe, ConfigValue::String(hit.re)),
                                    (ConfigPath::EscapeCenterIm, ConfigValue::String(hit.im)),
                                ],
                                "history.action.center_minibrot".to_string(),
                            );
                        }
                        None => log::info!("No minibrot found governing this view"),
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                // No threads in the browser build: synchronous with a
                // modest budget.
                if ui
                    .button(t!("escape_panel.center_minibrot").as_ref())
                    .on_hover_text(t!("escape_panel.tooltip_center_minibrot"))
                    .clicked()
                {
                    if let Some(hit) = crate::escape::nucleus::locate_minibrot(
                        &esc.center_re,
                        &esc.center_im,
                        esc.zoom_log2,
                        20_000,
                        power,
                        5_000,
                    ) {
                        let _ = config_manager.update_batch(
                            vec![
                                (ConfigPath::EscapeCenterRe, ConfigValue::String(hit.re)),
                                (ConfigPath::EscapeCenterIm, ConfigValue::String(hit.im)),
                            ],
                            "history.action.center_minibrot".to_string(),
                        );
                    }
                }
            }
        }

        });

    egui::CollapsingHeader::new(t!("escape_panel.diag_section"))
        .default_open(false)
        .show(ui, |ui| {
            // A latency attribution readout, not a health check: when
            // an edit past the perturbation threshold feels slow,
            // this says which stage the time went to.
            let d = crate::escape::diag::snapshot();
            ui.label(t!("escape_panel.diag_path", path = d.path))
                .on_hover_text(t!("escape_panel.tooltip_diag_path"));
            ui.label(t!(
                "escape_panel.diag_settle",
                ms = format!("{:.0}", d.settle_ms),
                frames = d.settle_frames
            ))
            .on_hover_text(t!("escape_panel.tooltip_diag_settle"));
            if d.inflight_frames > 0 {
                ui.label(t!("escape_panel.diag_inflight", frames = d.inflight_frames));
            }
            ui.label(t!("escape_panel.diag_restarts", count = d.restarts))
                .on_hover_text(t!("escape_panel.tooltip_diag_restarts"));
            ui.label(t!(
                "escape_panel.diag_render_cpu",
                ms = format!("{:.2}", d.render_cpu_ms)
            ))
            .on_hover_text(t!("escape_panel.tooltip_diag_render_cpu"));
            if !d.path.is_empty() && d.path != "direct" {
                ui.separator();
                ui.label(t!(
                    "escape_panel.diag_orbit",
                    len = d.orbit_len,
                    source = d.orbit_source.label(),
                    ms = format!("{:.0}", d.orbit_ms)
                ))
                .on_hover_text(t!("escape_panel.tooltip_diag_orbit"));
                ui.label(t!(
                    "escape_panel.diag_orbit_churn",
                    rebuilds = d.orbit_rebuilds,
                    relocations = d.orbit_relocations,
                    waits = d.orbit_wait_frames
                ))
                .on_hover_text(t!("escape_panel.tooltip_diag_orbit_churn"));
                ui.label(t!(
                    "escape_panel.diag_stale",
                    frames = d.orbit_stale_serves
                ))
                .on_hover_text(t!("escape_panel.tooltip_diag_stale"));
                ui.label(t!("escape_panel.diag_upload", kb = d.upload_bytes / 1024));
                if d.bla_active {
                    ui.label(t!(
                        "escape_panel.diag_bla",
                        kb = d.bla_bytes / 1024,
                        ms = format!("{:.1}", d.bla_build_ms)
                    ));
                } else {
                    ui.label(t!("escape_panel.diag_bla_off"));
                }
                ui.label(t!("escape_panel.diag_chunk", iters = d.last_chunk_iters));
            }
        });

    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.rotation"));
        let mut deg = esc.rotation.to_degrees();
        if ui
            .add(egui::DragValue::new(&mut deg).speed(0.5).suffix("°"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeRotation, deg.to_radians().into());
        }
    });

    ui.separator();

    // ---- Iteration ----
    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.max_iter"));
        let mut iter = esc.max_iter;
        if ui
            .add(egui::DragValue::new(&mut iter).speed(4).range(1..=100_000_000))
            .on_hover_text(t!("escape_panel.tooltip_max_iter"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeMaxIter, ConfigValue::UInt(iter));
        }
    });
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
                    if ui.selectable_label(n == current, label_for(n)).clicked() && n != current
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::EscapeSupersample, ConfigValue::UInt(n));
                    }
                }
            })
            .response
            .on_hover_text(t!("escape_panel.tooltip_supersample"));
    });
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
                    for m in [
                        DownsampleMode::Box,
                        DownsampleMode::Perceptual,
                        DownsampleMode::Vivid,
                    ] {
                        if ui.selectable_label(cur == m, label_for(m).as_ref()).clicked()
                            && cur != m
                        {
                            let _ = config_manager.update_param(
                                ConfigPath::EscapeDownsample,
                                ConfigValue::String(m.as_str().to_string()),
                            );
                        }
                    }
                })
                .response
                .on_hover_text(t!("escape_panel.tooltip_downsample"));
        });
    }
    // ---- Auto contrast ----
    // Sits above relief because it changes what relief slopes: both
    // read the coloring's value field, and this one decides how much
    // of it the palette actually spans.
    egui::CollapsingHeader::new(t!("escape_panel.contrast"))
        .default_open(!esc.contrast.mode.is_off())
        .show(ui, |ui| {
            let ct = esc.contrast.clone();
            ui.horizontal(|ui| {
                ui.label(t!("escape_panel.contrast_mode"));
                let cur = ct.mode;
                let name = |m: ContrastMode| match m {
                    ContrastMode::Off => t!("escape_panel.contrast_off"),
                    ContrastMode::AutoRange => t!("escape_panel.contrast_auto_range"),
                    ContrastMode::Flatten => t!("escape_panel.contrast_flatten"),
                };
                egui::ComboBox::from_id_salt("escape_contrast_mode")
                    .selected_text(name(cur))
                    .show_ui(ui, |ui| {
                        for m in [ContrastMode::Off, ContrastMode::AutoRange, ContrastMode::Flatten]
                        {
                            if ui.selectable_label(m == cur, name(m)).clicked() && m != cur {
                                let _ = config_manager.update_param(
                                    ConfigPath::EscapeContrastMode,
                                    ConfigValue::String(
                                        crate::config::escape::contrast_mode_to_str(m).to_string(),
                                    ),
                                );
                            }
                        }
                    })
                    .response
                    .on_hover_text(t!("escape_panel.tooltip_contrast"));
            });
            ui.add_enabled_ui(!ct.mode.is_off(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.contrast_strength"));
                    let mut v = ct.strength;
                    if ui
                        .add(egui::Slider::new(&mut v, 0.0..=1.0))
                        .on_hover_text(t!("escape_panel.tooltip_contrast_strength"))
                        .changed()
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::EscapeContrastStrength, v.into());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.contrast_turns"));
                    let mut v = ct.turns;
                    if ui
                        .add(
                            egui::Slider::new(&mut v, 0.05..=64.0)
                                .logarithmic(true),
                        )
                        .on_hover_text(t!("escape_panel.tooltip_contrast_turns"))
                        .changed()
                    {
                        let _ =
                            config_manager.update_param(ConfigPath::EscapeContrastTurns, v.into());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.contrast_clip"));
                    let mut v = ct.clip;
                    if ui
                        .add(egui::Slider::new(&mut v, 0.0..=0.25).fixed_decimals(3))
                        .on_hover_text(t!("escape_panel.tooltip_contrast_clip"))
                        .changed()
                    {
                        let _ = config_manager.update_param(ConfigPath::EscapeContrastClip, v.into());
                    }
                });
            });
        });

    // ---- Relief shading ----
    // A LAYER, not a coloring: it runs after the palette lookup, so it
    // composes with whatever is above it. Collapsed by default because
    // it is off by default and its ten controls would otherwise crowd
    // the panel.
    egui::CollapsingHeader::new(t!("escape_panel.shading"))
        .default_open(esc.shading.enabled)
        .show(ui, |ui| {
            let sh = esc.shading.clone();
            let mut enabled = sh.enabled;
            if ui
                .checkbox(&mut enabled, t!("escape_panel.shading_enabled"))
                .on_hover_text(t!("escape_panel.tooltip_shading"))
                .changed()
            {
                let _ = config_manager
                    .update_param(ConfigPath::EscapeShadingEnabled, enabled.into());
            }
            ui.add_enabled_ui(sh.enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.shading_light"));
                    let mut a = sh.light_angle;
                    if ui
                        .add(egui::DragValue::new(&mut a).speed(1.0).range(0.0..=360.0).suffix("°"))
                        .on_hover_text(t!("escape_panel.tooltip_shading_light"))
                        .changed()
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::EscapeShadingLightAngle, a.into());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.shading_height"));
                    let mut h = sh.height;
                    // Logarithmic: the useful value depends on how many
                    // palette turns the coloring spends across a view,
                    // which differs by three orders of magnitude
                    // between (say) a scaled escape count and a bounded
                    // coloring. A linear slider would be unusable.
                    if ui
                        .add(
                            egui::Slider::new(&mut h, 0.01..=1000.0)
                                .logarithmic(true)
                                .clamping(egui::SliderClamping::Never),
                        )
                        .on_hover_text(t!("escape_panel.tooltip_shading_height"))
                        .changed()
                    {
                        let _ =
                            config_manager.update_param(ConfigPath::EscapeShadingHeight, h.into());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.shading_softness"));
                    let mut sf = sh.softness;
                    if ui
                        // Continuous: this is a Gaussian WIDTH, not a
                        // stencil radius, so fractions mean something.
                        .add(egui::Slider::new(&mut sf, 0.0..=16.0))
                        .on_hover_text(t!("escape_panel.tooltip_shading_softness"))
                        .changed()
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::EscapeShadingSoftness, sf.into());
                    }
                });
                // ---- Surface texture ----
                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.shading_texture"));
                    let cur = sh.texture_kind;
                    egui::ComboBox::from_id_salt("escape_shading_texture")
                        .selected_text(match cur {
                            ShadingTexture::None => t!("escape_panel.texture_none"),
                            ShadingTexture::Grain => t!("escape_panel.texture_grain"),
                            ShadingTexture::Paper => t!("escape_panel.texture_paper"),
                        })
                        .show_ui(ui, |ui| {
                            for k in [
                                ShadingTexture::None,
                                ShadingTexture::Grain,
                                ShadingTexture::Paper,
                            ] {
                                let label = match k {
                                    ShadingTexture::None => t!("escape_panel.texture_none"),
                                    ShadingTexture::Grain => t!("escape_panel.texture_grain"),
                                    ShadingTexture::Paper => t!("escape_panel.texture_paper"),
                                };
                                if ui.selectable_label(cur == k, label.as_ref()).clicked()
                                    && cur != k
                                {
                                    let _ = config_manager.update_param(
                                        ConfigPath::EscapeShadingTextureKind,
                                        ConfigValue::String(k.as_str().to_string()),
                                    );
                                }
                            }
                        });
                })
                .response
                .on_hover_text(t!("escape_panel.tooltip_shading_texture"));
                if sh.texture_kind != ShadingTexture::None {
                    ui.horizontal(|ui| {
                        ui.label(t!("escape_panel.texture_strength"));
                        let mut v = sh.texture_strength;
                        if ui
                            .add(egui::Slider::new(&mut v, 0.0..=4.0))
                            .on_hover_text(t!("escape_panel.tooltip_texture_strength"))
                            .changed()
                        {
                            let _ = config_manager.update_param(
                                ConfigPath::EscapeShadingTextureStrength,
                                v.into(),
                            );
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(t!("escape_panel.texture_scale"));
                        let mut v = sh.texture_scale;
                        if ui
                            .add(egui::Slider::new(&mut v, 0.25..=64.0).logarithmic(true))
                            .on_hover_text(t!("escape_panel.tooltip_texture_scale"))
                            .changed()
                        {
                            let _ = config_manager
                                .update_param(ConfigPath::EscapeShadingTextureScale, v.into());
                        }
                    });
                }

                ui.horizontal(|ui| {
                    ui.label(t!("escape_panel.shading_field"));
                    let cur = sh.field;
                    egui::ComboBox::from_id_salt("escape_shading_field")
                        .selected_text(match cur {
                            ShadingField::Smooth => t!("escape_panel.shading_field_smooth"),
                            ShadingField::Banded => t!("escape_panel.shading_field_banded"),
                        })
                        .show_ui(ui, |ui| {
                            for f in [ShadingField::Smooth, ShadingField::Banded] {
                                let label = match f {
                                    ShadingField::Smooth => {
                                        t!("escape_panel.shading_field_smooth")
                                    }
                                    ShadingField::Banded => {
                                        t!("escape_panel.shading_field_banded")
                                    }
                                };
                                if ui.selectable_label(f == cur, label).clicked() && f != cur {
                                    let _ = config_manager.update_param(
                                        ConfigPath::EscapeShadingField,
                                        ConfigValue::String(
                                            crate::config::escape::shading_field_to_str(f)
                                                .to_string(),
                                        ),
                                    );
                                }
                            }
                        })
                        .response
                        .on_hover_text(t!("escape_panel.tooltip_shading_field"));
                });

                ui.separator();
                shading_side(
                    ui,
                    config_manager,
                    t!("escape_panel.shading_shadows").to_string(),
                    "shadow",
                    sh.shadow_color,
                    sh.shadow_strength,
                    sh.shadow_blend,
                    ConfigPath::EscapeShadingShadowColor,
                    ConfigPath::EscapeShadingShadowStrength,
                    ConfigPath::EscapeShadingShadowBlend,
                );
                shading_side(
                    ui,
                    config_manager,
                    t!("escape_panel.shading_highlights").to_string(),
                    "highlight",
                    sh.highlight_color,
                    sh.highlight_strength,
                    sh.highlight_blend,
                    ConfigPath::EscapeShadingHighlightColor,
                    ConfigPath::EscapeShadingHighlightStrength,
                    ConfigPath::EscapeShadingHighlightBlend,
                );
            });
        });

    // ---- Iteration controls, shown only where the shader reads them ----
    //
    // `bailout` and the biomorph axis both live inside the escape
    // test, which the assembler compiles in only for an ESCAPING
    // formula; damping is spliced into the step of any mode-A
    // formula. A field shader has none of the three -- no escape
    // test, no bailout, and a fixed-count accumulation with no step
    // to damp -- so all three sat in the panel doing nothing.
    // Mode D reads none of them either: its walk has no escape test,
    // no bailout and no step to damp — the depth and the beam are its
    // own parameters, drawn from the def above.
    let controls = match (ifs_def, field) {
        (Some(_), _) | (None, Some(_)) => crate::escape::FIELD_ITERATION_CONTROLS,
        (None, None) => {
            crate::escape::iteration_controls(crate::escape::get_formula(&esc.formula))
        }
    };

    if controls.bailout {
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.bailout"));
            let mut bail = esc.bailout;
            if ui
                .add(egui::DragValue::new(&mut bail).speed(0.1).range(0.001..=1.0e12))
                .on_hover_text(t!("escape_panel.tooltip_bailout"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::EscapeBailout, bail.into());
            }
        });
    }

    // Mann-iteration damping (complex α; 1+0i = plain iteration)
    if controls.damping {
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.damping"))
                .on_hover_text(t!("escape_panel.tooltip_damping"));
            let mut dre = esc.damping_re;
            if ui
                .add(egui::DragValue::new(&mut dre).speed(0.005).prefix("re: "))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::EscapeDampingRe, dre.into());
            }
            let mut dim = esc.damping_im;
            if ui
                .add(egui::DragValue::new(&mut dim).speed(0.005).prefix("im: "))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::EscapeDampingIm, dim.into());
            }
        });

    }

    // Biomorph classification axis
    if controls.biomorph {
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.biomorph"));
            let current = crate::config::escape::biomorph_to_str(esc.biomorph);
            egui::ComboBox::from_id_salt("escape_biomorph")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for name in ["off", "re", "im"] {
                        if ui.selectable_label(current == name, name).clicked() && current != name {
                            let _ = config_manager.update_param(
                                ConfigPath::EscapeBiomorph,
                                ConfigValue::String(name.to_string()),
                            );
                        }
                    }
                });
        });
    }

    ui.separator();

    // ---- Coloring ----
    show_coloring_section(ui, config_manager, &esc, field, ifs_def);
}

/// A starting `scale` for a coloring, from the iteration cap.
///
/// The scale/offset pair is the panel's least guessable control: it
/// maps an escape value onto the palette, and the useful range depends
/// on how long pixels actually take to escape -- which the user cannot
/// see. Picking the Ducks showcase's 1.86/11.6 took a numpy probe.
///
/// This is a STARTING POINT, not a measurement. The honest version
/// reads the rendered value distribution back off the GPU; until that
/// exists, this puts a few palette cycles across the range typical of
/// a view at this iteration cap, which is what the shipped configs
/// use. Smooth and escape-count colorings scale with the COUNT (most
/// pixels escape in a few hundred iterations however high the cap is),
/// while the orbit-trap and average families produce O(1) values and
/// want a scale near 1.
/// Slider for one escape parameter, logarithmic when its range spans
/// enough decades that a linear one is useless.
///
/// An iteration-scaled `scale` runs from 1e-6 to 1: linearly, the
/// entire useful deep-zoom range lives in the leftmost thousandth of
/// the track. The threshold is high enough (1e5) that every existing
/// range keeps the linear feel it had.
fn param_slider<'a>(
    v: &'a mut f32,
    p: &'static crate::escape::EscapeParamDef,
) -> egui::Slider<'a> {
    let decades = p.min > 0.0 && p.max / p.min >= 1e5;
    egui::Slider::new(v, p.min..=p.max)
        .text(p.display_name)
        .logarithmic(decades)
}

/// One escape parameter's control: a DROPDOWN when the definition
/// names discrete choices, the slider otherwise. Returns whether the
/// value changed.
///
/// The stored value is identical either way — the choice index, in the
/// same f32 the shader reads — so this changes presentation only. A
/// slider over "0: Burning Ship, 1: Perpendicular Mandelbrot, ... 5:
/// Perpendicular Celtic" let you land on 2.4, which is variant 2 with
/// no indication that it was not something in its own right, and hid
/// the roster behind the tooltip.
///
/// `salt` separates the formula and coloring sections: both can define
/// a parameter called `variant`, and two combo boxes sharing an egui
/// id would share their popup state.
fn param_control(
    ui: &mut egui::Ui,
    v: &mut f32,
    p: &'static crate::escape::EscapeParamDef,
    salt: &str,
) -> bool {
    if p.choices.is_empty() {
        return ui.add(param_slider(v, p)).on_hover_text(p.tooltip).changed();
    }
    // Round rather than truncate, and clamp: a config written before
    // the choice list existed (or hand-edited) can hold anything, and
    // it must land on a real entry rather than panicking the index.
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

fn suggested_coloring_scale(coloring: &str, max_iter: u32) -> f32 {
    match coloring {
        "smooth" | "escape_count" | "period" => {
            // A few cycles over a typical escape range. The floor is
            // the slider's own minimum, not a round number above it:
            // it used to stop at 0.005, so at the 100k iterations a
            // deep view needs, the suggestion came back 60x coarser
            // than the formula asked for -- which read as "the
            // minimum scale is much too large".
            (8.0 / (max_iter as f32).max(1.0)).clamp(0.000001, 0.5)
        }
        "distance_estimate" | "triangle_inequality" | "root_basin" => 1.0,
        // Orbit traps and the averaging family already live at O(1).
        _ => 1.0,
    }
}

/// The solid camera (D8), shown only when the loaded formula is a
/// solid one.
///
/// D2: the 3D controls follow the CONFIG, not the render mode. Escape
/// mode is not three-dimensional — one formula in it is — so gating on
/// the mode would show these over a Mandelbrot and hide them over the
/// thing they steer.
fn show_solid_camera(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    esc: &crate::config::escape::EscapeConfig,
) {
    ui.separator();
    ui.label(egui::RichText::new(t!("escape_panel.camera")).strong());

    // The target is decimal STRINGS: a deep zoom is an approach to a
    // point, so the target is the quantity that needs digits while the
    // distance shrinks around it. An f32 here would cap 3D at a zoom
    // the plane passed long ago.
    let axes: [(&str, ConfigPath, &String); 3] = [
        ("X", ConfigPath::EscapeCamTargetX, &esc.cam_target_x),
        ("Y", ConfigPath::EscapeCamTargetY, &esc.cam_target_y),
        ("Z", ConfigPath::EscapeCamTargetZ, &esc.cam_target_z),
    ];
    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.camera_target"));
        for (name, path, value) in axes {
            let mut text = value.clone();
            ui.label(name);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .desired_width(78.0)
                    .hint_text(t!("escape_panel.camera_target_auto")),
            );
            if resp.changed() {
                let _ = config_manager
                    .update_param(path, ConfigValue::String(text.trim().to_string()));
            }
        }
    });
    ui.label(
        egui::RichText::new(t!("escape_panel.camera_target_tip")).small().weak(),
    );

    let mut angle = |ui: &mut egui::Ui,
                     label: String,
                     path: ConfigPath,
                     value: f32,
                     range: std::ops::RangeInclusive<f32>,
                     tip: String| {
        ui.horizontal(|ui| {
            ui.label(label);
            let mut deg = value.to_degrees();
            if ui
                .add(egui::Slider::new(&mut deg, *range.start()..=*range.end()).suffix("°"))
                .on_hover_text(tip)
                .changed()
            {
                let _ = config_manager.update_param(path, deg.to_radians().into());
            }
        });
    };

    angle(
        ui,
        t!("escape_panel.camera_pitch").to_string(),
        ConfigPath::EscapeCamPitch,
        esc.cam_pitch,
        -90.0..=90.0,
        t!("escape_panel.camera_pitch_tip").to_string(),
    );
    angle(
        ui,
        t!("escape_panel.camera_yaw").to_string(),
        ConfigPath::EscapeCamYaw,
        esc.cam_yaw,
        -180.0..=180.0,
        t!("escape_panel.camera_yaw_tip").to_string(),
    );
    angle(
        ui,
        t!("escape_panel.camera_bank").to_string(),
        ConfigPath::EscapeCamBank,
        esc.cam_bank,
        -180.0..=180.0,
        t!("escape_panel.camera_bank_tip").to_string(),
    );
    angle(
        ui,
        t!("escape_panel.camera_fov").to_string(),
        ConfigPath::EscapeCamFov,
        esc.cam_fov,
        3.0..=170.0,
        t!("escape_panel.camera_fov_tip").to_string(),
    );
}

/// Mode D's criterion (the plan's §2.4), shown under the formula row.
///
/// The analysis returns EVERY reason a flame fails rather than the
/// first, so this lists them: a flame with two non-affine transforms
/// should say so once, not make the user fix one to discover the next.
///
/// It also says, when the flame does qualify, that the render draws
/// the SET and not the measure — weights, colour speed and density do
/// not apply here, and two flames differing only in weights render
/// identically. That is D6, and the panel is where it stops being a
/// surprise.
/// Choosing a lens: the name, and every one of its parameters at its
/// registry default, as ONE undo step.
///
/// Seeding rather than leaving them absent, for two reasons that both
/// bite otherwise. An absent parameter reads back as 0.0 from the
/// config manager -- a def's default is a registry concern, which is
/// why `LensTarget` resolves it at the panel -- so an undo of the
/// first edit would write that 0.0 as if it were the old value and
/// leave the lens in a state the user never saw. And `lens_params` is
/// keyed by parameter NAME alone, so a `c1` left behind by the
/// previous lens would otherwise be inherited by the next one that
/// happens to have a `c1`.
///
/// This is the same shape as `apply_preset` below, which enumerates a
/// def's parameters into one batch for the same reason.
fn lens_choice(name: &str) -> Vec<(ConfigPath, ConfigValue)> {
    let mut changes = vec![(ConfigPath::EscapeLens, ConfigValue::String(name.to_string()))];
    if let Some(info) = crate::variations::global_registry().get(name) {
        for p in &info.parameters {
            changes.push((
                ConfigPath::EscapeLensParam { param: p.name.clone() },
                p.default_value.into(),
            ));
        }
    }
    changes
}

/// The **camera lens**: a variation applied to the screen offset.
///
/// A lens warps the view rather than the fractal, which is the
/// opposite direction from a flame's final transform, so a variation
/// need not be invertible to be one and every shipped variation is
/// offered. Most are not good lenses -- measured, 285 of 647 are
/// smooth and in frame at their defaults -- but "good" here depends on
/// parameters the user can edit, so the picker curates nothing and
/// says so in its tooltip instead.
///
/// The parameters are rendered by `render_variation_params`, the same
/// code the transforms panel uses, through a `ParamTarget` that reads
/// the escape config's map instead of a transform. That is the whole
/// reason the trait exists: a second parameter renderer here would
/// have to reproduce the ParamType zoo, the undo coalescing and the
/// "a quantising widget must not rewrite the value merely by being
/// drawn" rule, and would drift from the original.
fn show_lens_section(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    use crate::ui::variation_params::{render_variation_params, LensTarget};

    let registry = crate::variations::global_registry();
    let current = config_manager.active_config().escape.lens.clone();
    let amount = config_manager.active_config().escape.lens_amount;

    let label = if current.is_empty() {
        t!("escape_panel.lens_none").to_string()
    } else {
        registry
            .get(&current)
            .map(|i| i.display_name.clone())
            .unwrap_or_else(|| current.clone())
    };

    egui::CollapsingHeader::new(t!("escape_panel.lens"))
        .id_salt("escape_lens")
        .default_open(!current.is_empty())
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(t!("escape_panel.lens_tip"))
                    .small()
                    .weak(),
            );

            let mut pick: Option<String> = None;
            let filter_id = egui::Id::new("escape_lens_filter");
            egui::ComboBox::from_id_salt("escape_lens_pick")
                .selected_text(label)
                .width(220.0)
                .show_ui(ui, |ui| {
                    // 647 entries is a scroll, not a list. The filter
                    // lives in egui memory rather than the config: it
                    // is a way of finding a lens, not part of one, and
                    // putting it in the config would make typing here
                    // an undo step.
                    let mut filter =
                        ui.data_mut(|d| d.get_temp::<String>(filter_id).unwrap_or_default());
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut filter)
                                .hint_text(t!("escape_panel.lens_filter"))
                                .desired_width(200.0),
                        )
                        .changed()
                    {
                        ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
                    }
                    let needle = filter.trim().to_lowercase();

                    // The measured set by default, everything on
                    // request. The classification describes a
                    // variation's DEFAULT parameters and those are
                    // editable, so it is a starting point rather than
                    // a verdict -- but a picker whose entries mostly
                    // do nothing is worse than a short one.
                    let all_id = egui::Id::new("escape_lens_show_all");
                    let mut show_all = ui.data_mut(|d| d.get_temp::<bool>(all_id).unwrap_or(false));
                    if ui
                        .checkbox(&mut show_all, t!("escape_panel.lens_show_all"))
                        .on_hover_text(t!("escape_panel.lens_show_all_tip"))
                        .changed()
                    {
                        ui.data_mut(|d| d.insert_temp(all_id, show_all));
                    }

                    if ui
                        .selectable_label(current.is_empty(), t!("escape_panel.lens_none"))
                        .clicked()
                    {
                        pick = Some(String::new());
                    }
                    ui.separator();

                    // The recommended lenses first in their own
                    // order, then everything else usable in the
                    // registry's order -- the same order the
                    // variations browser uses, so a lens is found
                    // where a variation is found.
                    let menu: Vec<String> = if show_all {
                        registry.names().to_vec()
                    } else {
                        crate::escape::lens::lens_menu(&registry)
                    };
                    let head = if show_all {
                        0
                    } else {
                        crate::escape::lens::LENS_RECOMMENDED
                            .iter()
                            .filter(|n| registry.get(n).is_some())
                            .count()
                    };
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (i, name) in menu.iter().enumerate() {
                            let Some(info) = registry.get(name) else { continue };
                            if !needle.is_empty()
                                && !name.to_lowercase().contains(&needle)
                                && !info.display_name.to_lowercase().contains(&needle)
                            {
                                continue;
                            }
                            // A rule under the recommended ones, so
                            // the head reads as a shortlist rather
                            // than as an arbitrary reordering.
                            if i == head && head > 0 && needle.is_empty() {
                                ui.separator();
                            }
                            if ui
                                .selectable_label(&current == name, &info.display_name)
                                .clicked()
                            {
                                pick = Some(name.clone());
                            }
                        }
                    });
                });
            if let Some(name) = pick {
                let _ = config_manager
                    .update_batch(lens_choice(&name), "history.param.escape_lens".to_string());
            }

            if current.is_empty() {
                return;
            }

            let mut a = amount;
            let lim = crate::escape::lens::LENS_AMOUNT_LIMIT;
            if ui
                .add(
                    egui::Slider::new(&mut a, -lim..=lim)
                        .text(t!("escape_panel.lens_amount")),
                )
                .on_hover_text(t!("escape_panel.lens_amount_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::EscapeLensAmount, a.into());
            }

            if let Some(info) = registry.get(&current) {
                if !info.parameters.is_empty() {
                    render_variation_params(
                        ui,
                        config_manager,
                        &LensTarget,
                        &current,
                        &info.parameters,
                    );
                }
            }
        });
}

/// `solid` picks WHICH criterion: a solid formula walks the 3D IFS,
/// and qualifying in three dimensions is a different question from
/// qualifying in two. Reported against the plane regardless, this
/// said "`quaternion_julia` is not affine" over a flame the solid
/// walk was rendering perfectly well -- the variation is a kernel in
/// `Space::Solid` and has no planar reading at all. The renderer had
/// its own version of the same mistake (`pack_flame` required the
/// planar analysis to pass first, plan §8.11 step 2) and was fixed
/// there; this is the panel's half.
/// What the criterion says about a flame, in the dimension the
/// formula walks it in. Extracted from the panel so the CHOICE of
/// analysis is testable: reading the plane's verdict over a solid
/// formula is the bug this exists to pin.
pub(crate) enum Verdict {
    Ok {
        maps: usize,
        roots: usize,
        lo: f64,
        hi: f64,
        has_final: bool,
        centre: [f64; 3],
        radius: f64,
    },
    No(Vec<String>),
}

pub(crate) fn ifs_verdict(flame: &crate::scene::transforms::Flame, solid: bool) -> Verdict {
    let registry = crate::variations::global_registry();
    macro_rules! verdict {
        ($ifs:expr, $centre:expr) => {{
            match $ifs {
                Ok(ifs) => Verdict::Ok {
                    maps: ifs.maps.len(),
                    roots: ifs.maps.iter().filter(|m| !m.forward.is_affine()).count(),
                    lo: ifs.maps.iter().map(|m| m.sigma_min).fold(f64::INFINITY, f64::min),
                    hi: ifs.maps.iter().map(|m| m.sigma_max).fold(0.0f64, f64::max),
                    has_final: ifs.final_map.is_some(),
                    centre: $centre(&ifs.ball.centre),
                    radius: ifs.ball.radius,
                },
                Err(why) => Verdict::No(why.iter().map(|d| d.to_string()).collect()),
            }
        }};
    }
    if solid {
        verdict!(crate::scene::ifs_analysis::analyse_3d(flame, &registry), |c: &[f64; 3]| *c)
    } else {
        verdict!(crate::scene::ifs_analysis::analyse_2d(flame, &registry), |c: &[f64; 2]| [c[0], c[1], 0.0])
    }
}

fn show_ifs_criterion(ui: &mut egui::Ui, config_manager: &mut ConfigManager, solid: bool) {
    // Analyse first and drop the borrow, so the Frame button below can
    // write through the same manager.
    let verdict = ifs_verdict(&config_manager.active_config().flame, solid);

    match verdict {
        Verdict::Ok { maps, roots, lo, hi, has_final, centre, radius } => {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(120, 190, 120),
                    if roots == 0 {
                        t!(
                            "escape_panel.ifs_qualifies",
                            count = maps.to_string(),
                            lo = format!("{lo:.3}"),
                            hi = format!("{hi:.3}")
                        )
                    } else {
                        t!(
                            "escape_panel.ifs_qualifies_roots",
                            count = maps.to_string(),
                            roots = roots.to_string()
                        )
                    },
                );
                if has_final {
                    ui.label(
                        egui::RichText::new(t!("escape_panel.ifs_has_final")).small().weak(),
                    );
                }
            });
            ui.label(
                egui::RichText::new(t!("escape_panel.ifs_set_not_measure")).small().weak(),
            );
            if roots > 0 {
                // Plan 8.8 J2: a root map's set is the FILLED one, and
                // the flame draws its boundary. Said here, where the
                // difference stops being a surprise.
                ui.label(egui::RichText::new(t!("escape_panel.ifs_roots_filled")).small().weak());
            }
            if radius > 0.0
                && ui
                    .button(t!("escape_panel.ifs_frame"))
                    .on_hover_text(t!("escape_panel.ifs_frame_tip"))
                    .clicked()
            {
                // A solid's view is a CAMERA -- a target it orbits and
                // a zoom that sets the distance -- and the distance is
                // already `FRAME_DISTANCE · radius / 2^zoom`, so
                // framing it is the target and a zoom of nothing. The
                // planar centre this used to write is a quantity the
                // solid camera does not read, so the button did
                // nothing at all over a solid.
                let updates = if solid {
                    vec![
                        (ConfigPath::EscapeCamTargetX, ConfigValue::String(format!("{:?}", centre[0]))),
                        (ConfigPath::EscapeCamTargetY, ConfigValue::String(format!("{:?}", centre[1]))),
                        (ConfigPath::EscapeCamTargetZ, ConfigValue::String(format!("{:?}", centre[2]))),
                        (ConfigPath::EscapeZoomLog2, 0.0f32.into()),
                    ]
                } else {
                    // The home view spans 4 units vertically, so a span
                    // of 2.4 radii leaves the attractor a comfortable
                    // margin.
                    let span = (radius * 2.4).max(1e-12);
                    vec![
                        (
                            ConfigPath::EscapeCenterRe,
                            ConfigValue::String(format!("{centre:?}", centre = centre[0])),
                        ),
                        (
                            ConfigPath::EscapeCenterIm,
                            ConfigValue::String(format!("{centre:?}", centre = centre[1])),
                        ),
                        (ConfigPath::EscapeZoomLog2, ((4.0f64 / span).log2() as f32).into()),
                    ]
                };
                let _ = config_manager
                    .update_batch(updates, "history.param.escape_center".to_string());
            }
        }
        Verdict::No(reasons) => {
            ui.colored_label(
                egui::Color32::from_rgb(220, 170, 90),
                if solid {
                    t!("escape_panel.ifs_rejected_solid")
                } else {
                    t!("escape_panel.ifs_rejected")
                },
            );
            for r in &reasons {
                ui.label(egui::RichText::new(format!("  \u{2022} {r}")).small().weak());
            }
            ui.label(
                egui::RichText::new(if solid {
                    t!("escape_panel.ifs_rejected_solid_tip")
                } else {
                    t!("escape_panel.ifs_rejected_tip")
                })
                .small()
                .weak(),
            );
        }
    }
}

/// Coloring dropdown + params. `field` = Some routes to the mode-B
/// coloring registry (with the def's fallback resolution — the
/// stored name usually still says "smooth" right after a switch).
fn show_coloring_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    esc: &crate::config::escape::EscapeConfig,
    field: Option<&'static crate::escape::fields::FieldDef>,
    ifs_def: Option<&'static crate::escape::ifs::IfsDef>,
) {
    if let Some(d) = ifs_def {
        let coloring = crate::escape::ifs::get_ifs_coloring(&esc.coloring, d);
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.coloring"));
            egui::ComboBox::from_id_salt("escape_coloring")
                .selected_text(coloring.display_name)
                .show_ui(ui, |ui| {
                    for c in crate::escape::ifs::IFS_COLORINGS {
                        if ui
                            .selectable_label(c.name == coloring.name, c.display_name)
                            .clicked()
                            && c.name != coloring.name
                        {
                            let _ = config_manager.update_param(
                                ConfigPath::EscapeColoring,
                                ConfigValue::String(c.name.to_string()),
                            );
                        }
                    }
                });
        });
        for p in coloring.parameters {
            let mut v = esc.coloring_params.get(p.name).copied().unwrap_or(p.default);
            if param_control(ui, &mut v, p, "coloring") {
                let _ = config_manager.update_param(
                    ConfigPath::EscapeColoringParam { param: p.name.to_string() },
                    v.into(),
                );
            }
        }
        return;
    }
    if let Some(f) = field {
        let coloring = crate::escape::fields::get_field_coloring(&esc.coloring, f);
        ui.horizontal(|ui| {
            ui.label(t!("escape_panel.coloring"));
            egui::ComboBox::from_id_salt("escape_coloring")
                .selected_text(coloring.display_name)
                .show_ui(ui, |ui| {
                    for c in crate::escape::fields::FIELD_COLORINGS {
                        if ui
                            .selectable_label(c.name == coloring.name, c.display_name)
                            .clicked()
                            && c.name != coloring.name
                        {
                            let _ = config_manager.update_param(
                                ConfigPath::EscapeColoring,
                                ConfigValue::String(c.name.to_string()),
                            );
                        }
                    }
                });
        });
    // The scale/offset pair is the hardest control here to guess at;
        // offer a starting point rather than leaving the user to probe.
            if coloring.parameters.iter().any(|p| p.name == "scale") {
            let suggested = suggested_coloring_scale(coloring.name, esc.max_iter);
            let current = esc.coloring_params.get("scale").copied();
            ui.horizontal(|ui| {
                if ui
                    .button(t!("escape_panel.auto_scale"))
                    .on_hover_text(t!("escape_panel.auto_scale_tip", value = format!("{suggested:.6}")))
                    .clicked()
                {
                    let _ = config_manager.update_param(
                        ConfigPath::EscapeColoringParam { param: "scale".to_string() },
                        suggested.into(),
                    );
                }
                if current.is_some_and(|c| (c - suggested).abs() > suggested * 0.5) {
                    ui.label(
                        egui::RichText::new(t!(
                            "escape_panel.auto_scale_hint",
                            value = format!("{suggested:.6}")
                        ))
                        .small()
                        .weak(),
                    );
                }
            });
        }
        for p in coloring.parameters {
            let mut v = esc.coloring_params.get(p.name).copied().unwrap_or(p.default);
            if param_control(ui, &mut v, p, "coloring") {
                let _ = config_manager.update_param(
                    ConfigPath::EscapeColoringParam { param: p.name.to_string() },
                    v.into(),
                );
            }
        }
        return;
    }
    let coloring = crate::escape::get_coloring(&esc.coloring);
    let formula_def = crate::escape::get_formula(&esc.formula);
    // Only the colorings that can actually draw THIS formula. An
    // escape-time coloring over a non-escaping map renders black, not
    // badly -- see `coloring_suits_formula`. Offering the whole list
    // and letting the user find the blank ones is most of what makes
    // this panel hard to approach.
    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.coloring"));
        egui::ComboBox::from_id_salt("escape_coloring")
            .selected_text(coloring.display_name)
            .show_ui(ui, |ui| {
                for c in crate::escape::COLORINGS {
                    if !crate::escape::coloring_suits_formula(formula_def, c) {
                        continue;
                    }
                    if ui
                        .selectable_label(c.name == coloring.name, c.display_name)
                        .clicked()
                        && c.name != coloring.name
                    {
                        let _ = config_manager.update_param(
                            ConfigPath::EscapeColoring,
                            ConfigValue::String(c.name.to_string()),
                        );
                    }
                }
            });
    });
    // The saved coloring may predate the formula it is paired with (an
    // older config, or a formula switch). Say so and offer the fix,
    // rather than leaving a black picture to be puzzled over.
    if !crate::escape::coloring_suits_formula(formula_def, coloring) {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(220, 170, 90),
                t!(
                    "escape_panel.coloring_mismatch",
                    coloring = coloring.display_name,
                    formula = formula_def.display_name
                ),
            );
            if let Some(fix) = crate::escape::COLORINGS
                .iter()
                .find(|c| crate::escape::coloring_suits_formula(formula_def, c))
            {
                if ui
                    .small_button(t!("escape_panel.coloring_use", name = fix.display_name))
                    .clicked()
                {
                    let _ = config_manager.update_param(
                        ConfigPath::EscapeColoring,
                        ConfigValue::String(fix.name.to_string()),
                    );
                }
            }
        });
    }
    // A derivative-based coloring with no derivative to read renders
    // flat on purpose (a confident wrong image is worse than a visibly
    // missing one). Flat is honest but silent, so say why — otherwise
    // the only signal is a blank picture.
    if coloring.has_feature(crate::escape::ColoringFeature::NeedsDerivative) {
        if let Some(gap) = crate::escape::EscapeRenderer::derivative_gap(esc) {
            let msg = match gap {
                crate::escape::DerivativeGap::Formula => t!(
                    "escape_panel.no_derivative_formula",
                    formula = crate::escape::get_formula(&esc.formula).display_name,
                    coloring = coloring.display_name
                ),
                crate::escape::DerivativeGap::Perturbed => {
                    t!("escape_panel.no_derivative_perturbed", coloring = coloring.display_name)
                }
            };
            ui.colored_label(egui::Color32::from_rgb(220, 170, 90), msg);
        }
    }

    // The scale/offset pair is the hardest control here to guess at;
    // offer a starting point rather than leaving the user to probe.
    if coloring.parameters.iter().any(|p| p.name == "scale") {
        let suggested = suggested_coloring_scale(coloring.name, esc.max_iter);
        let current = esc.coloring_params.get("scale").copied();
        ui.horizontal(|ui| {
            if ui
                .button(t!("escape_panel.auto_scale"))
                .on_hover_text(t!("escape_panel.auto_scale_tip", value = format!("{suggested:.6}")))
                .clicked()
            {
                let _ = config_manager.update_param(
                    ConfigPath::EscapeColoringParam { param: "scale".to_string() },
                    suggested.into(),
                );
            }
            if current.is_some_and(|c| (c - suggested).abs() > suggested * 0.5) {
                ui.label(
                    egui::RichText::new(t!(
                        "escape_panel.auto_scale_hint",
                        value = format!("{suggested:.6}")
                    ))
                    .small()
                    .weak(),
                );
            }
        });
    }
    for p in coloring.parameters {
        let mut v = esc.coloring_params.get(p.name).copied().unwrap_or(p.default);
        if param_control(ui, &mut v, p, "coloring") {
            let _ = config_manager.update_param(
                ConfigPath::EscapeColoringParam { param: p.name.to_string() },
                v.into(),
            );
        }
    }
}

/// Switch render mode, defaulting the tonemap to Linear on the way
/// INTO escape mode (plan: escape's home tonemap is Linear).
///
/// Exposure and gamma come along: flame presets carry
/// LOGARITHMIC-calibrated values (e.g. the startup flame's exposure
/// 0.016 / gamma 0.35), and Linear mode computes
/// `rgb × exposure` then `pow(…, 1/gamma)` — under those values the
/// escape image renders at ~1e-5 brightness, i.e. an all-black
/// viewport (found the hard way during bring-up). So when the tonemap
/// is at the flame's Logarithmic mode, entering escape batches
/// Linear + the config-default exposure/gamma as ONE undo point —
/// leaving escape and pressing Ctrl+Z restores the flame's look
/// exactly. A deliberately non-Logarithmic tonemap is left alone.
/// Per-mode tonemap state is the real fix, noted in the plan.
/// Apply one preset as a SINGLE undo step.
///
/// Everything travels together deliberately: view, iteration budget,
/// coloring and both parameter sets. Applying them as separate
/// updates would leave the render passing through states nobody asked
/// for (a deep view under the wrong coloring, say) and would litter
/// the history with a dozen entries for one click.
///
/// Parameters not named by the preset are RESET to the definition's
/// defaults rather than left behind — a leftover value from the
/// previous formula is exactly the kind of invisible state that makes
/// a preset fail to reproduce its own picture.
pub fn apply_preset(
    config_manager: &mut ConfigManager,
    formula: &'static crate::escape::FormulaDef,
    preset: &crate::escape::EscapePreset,
) -> Result<(), crate::config::manager::ConfigError> {
    let coloring = crate::escape::get_coloring(preset.coloring);
    let mut changes: Vec<(ConfigPath, ConfigValue)> = vec![
        (ConfigPath::EscapeFormula, ConfigValue::String(formula.name.to_string())),
        (ConfigPath::EscapeColoring, ConfigValue::String(preset.coloring.to_string())),
        (ConfigPath::EscapeCenterRe, ConfigValue::String(preset.center_re.to_string())),
        (ConfigPath::EscapeCenterIm, ConfigValue::String(preset.center_im.to_string())),
        (ConfigPath::EscapeZoomLog2, (preset.zoom_log2 as f32).into()),
        (ConfigPath::EscapeMaxIter, ConfigValue::UInt(preset.max_iter)),
        (ConfigPath::EscapeJulia, preset.julia.is_some().into()),
    ];
    if let Some((re, im)) = preset.julia {
        changes.push((ConfigPath::EscapeJuliaRe, re.into()));
        changes.push((ConfigPath::EscapeJuliaIm, im.into()));
    }
    if let Some(b) = preset.bailout {
        changes.push((ConfigPath::EscapeBailout, b.into()));
    }
    for p in formula.parameters {
        let v = preset
            .formula_params
            .iter()
            .find(|(k, _)| *k == p.name)
            .map(|(_, v)| *v)
            .unwrap_or(p.default);
        changes.push((
            ConfigPath::EscapeFormulaParam { param: p.name.to_string() },
            v.into(),
        ));
    }
    for p in coloring.parameters {
        let v = preset
            .coloring_params
            .iter()
            .find(|(k, _)| *k == p.name)
            .map(|(_, v)| *v)
            .unwrap_or(p.default);
        changes.push((
            ConfigPath::EscapeColoringParam { param: p.name.to_string() },
            v.into(),
        ));
    }
    config_manager
        .update_batch(changes, "history.action.escape_preset".to_string())
        .map(|_| ())
}

/// [`apply_preset`] for a mode-B field.
///
/// Separate because a field's parameters and colorings come from the
/// field registry, not the formula one — the same shape of work, over
/// a different pair of definitions.
pub fn apply_field_preset(
    config_manager: &mut ConfigManager,
    field: &'static crate::escape::fields::FieldDef,
    preset: &crate::escape::EscapePreset,
) -> Result<(), crate::config::manager::ConfigError> {
    let coloring = crate::escape::fields::get_field_coloring(preset.coloring, field);
    let mut changes: Vec<(ConfigPath, ConfigValue)> = vec![
        (ConfigPath::EscapeFormula, ConfigValue::String(field.name.to_string())),
        (ConfigPath::EscapeColoring, ConfigValue::String(coloring.name.to_string())),
        (ConfigPath::EscapeCenterRe, ConfigValue::String(preset.center_re.to_string())),
        (ConfigPath::EscapeCenterIm, ConfigValue::String(preset.center_im.to_string())),
        (ConfigPath::EscapeZoomLog2, (preset.zoom_log2 as f32).into()),
        (ConfigPath::EscapeMaxIter, ConfigValue::UInt(preset.max_iter)),
    ];
    for p in field.parameters {
        let v = preset
            .formula_params
            .iter()
            .find(|(k, _)| *k == p.name)
            .map(|(_, v)| *v)
            .unwrap_or(p.default);
        changes.push((
            ConfigPath::EscapeFormulaParam { param: p.name.to_string() },
            v.into(),
        ));
    }
    for p in coloring.parameters {
        let v = preset
            .coloring_params
            .iter()
            .find(|(k, _)| *k == p.name)
            .map(|(_, v)| *v)
            .unwrap_or(p.default);
        changes.push((
            ConfigPath::EscapeColoringParam { param: p.name.to_string() },
            v.into(),
        ));
    }
    config_manager
        .update_batch(changes, "history.action.escape_preset".to_string())
        .map(|_| ())
}

/// Magnification as a readable factor, formatted FROM THE LOG.
///
/// Never from `zoom_factor()`: 2^zoom overflows f64 past zoom_log2
/// 1024 and a deep dive runs past 9000, so the plain factor would
/// read "inf" exactly where the number matters most.
pub(crate) fn magnification_label(log10: f64) -> String {
    if !log10.is_finite() {
        return String::new();
    }
    if log10.abs() >= 5.0 {
        let e = log10.floor();
        // 10^frac stays in [1, 10) — no overflow at any depth.
        let m = 10f64.powf(log10 - e);
        format!("×{m:.2}e{}", e as i64)
    } else {
        let v = 10f64.powf(log10);
        if v >= 100.0 {
            format!("×{v:.0}")
        } else {
            format!("×{v:.3}")
        }
    }
}

/// Zoom the escape view by a plain factor (keyboard +/- keys): adds
/// log2(factor) to the exponent, clamped to the same travel range the
/// wheel uses.
pub(crate) fn escape_zoom_by_factor(config_manager: &mut ConfigManager, factor: f64) {
    let z = config_manager.active_config().escape.zoom_log2;
    // Same ceiling as the wheel (panel_viewer): the phase-1 clamp of
    // 300 would collapse a deep session's zoom on one keypress.
    let new_z = (z + factor.log2()).clamp(-8.0, 100_000_000.0);
    let _ = config_manager.update_param(ConfigPath::EscapeZoomLog2, (new_z as f32).into());
}

/// Background minibrot-search state (desktop). Module-static because
/// the panel is stateless between frames; one search at a time.
#[cfg(not(target_arch = "wasm32"))]
enum PeriodSearch {
    Idle,
    Running,
    /// (period, closure octave) — the octave is what makes the result
    /// explainable: it says how deep the wrap stays exact.
    Done(Option<(u32, i64)>),
}

#[cfg(not(target_arch = "wasm32"))]
fn period_search_slot() -> std::sync::Arc<std::sync::Mutex<PeriodSearch>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static SLOT: OnceLock<Arc<Mutex<PeriodSearch>>> = OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(PeriodSearch::Idle)))
        .clone()
}

/// What the last detection concluded, shown under the field. A period
/// that cannot wrap at the current depth is a real answer and the user
/// needs to see it — silently writing it into the field is what made a
/// z9316 view adopt a period-71,100 atom that serves only to ~z597.
#[cfg(not(target_arch = "wasm32"))]
fn period_note_slot() -> std::sync::Arc<std::sync::Mutex<Option<String>>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static SLOT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

#[cfg(not(target_arch = "wasm32"))]
enum MinibrotSearch {
    Idle,
    Running,
    Done(Option<crate::escape::nucleus::Nucleus>),
}

#[cfg(not(target_arch = "wasm32"))]
fn minibrot_search_slot() -> std::sync::Arc<std::sync::Mutex<MinibrotSearch>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static SLOT: OnceLock<Arc<Mutex<MinibrotSearch>>> = OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(MinibrotSearch::Idle)))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_dock::egui;

    /// Lay the panel out for real, for every formula.
    ///
    /// A panel that compiles can still panic at layout (a duplicate
    /// widget id, a slider whose range is empty because a def's min
    /// equals its max) or quietly render a translation KEY where a
    /// label belongs. Neither shows up in a build, and neither shows
    /// up in the visual suite, which renders fractals and not panels.
    fn lay_out(config: crate::config::FractalConfig) -> Vec<String> {
        let ctx = egui::Context::default();
        let mut manager = ConfigManager::new(config);
        let mut labels = Vec::new();
        // Two frames: the first populates egui's memory, the second
        // takes the "widget already exists" path where id collisions
        // surface.
        for _ in 0..2 {
            let out = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_escape_content(ui, &mut manager, &mut None);
                });
            });
            labels = out
                .textures_delta
                .set
                .iter()
                .map(|(id, _)| format!("{id:?}"))
                .collect();
        }
        labels
    }

    #[test]
    fn the_panel_lays_out_for_every_formula() {
        for f in crate::escape::FORMULAS {
            let mut config = crate::config::FractalConfig::default();
            config.render_mode = RenderMode::Escape;
            config.escape.formula = f.name.to_string();
            let _ = lay_out(config);
        }
    }

    /// Mode D lays out for a flame that qualifies and for one that
    /// does not — the criterion path runs the analysis and formats
    /// every reason, which is a lot of panel code that only executes
    /// when a flame fails.
    #[test]
    fn the_panel_lays_out_for_every_distance_function() {
        use std::collections::HashMap;
        let half = |tx: f32, ty: f32, var: &str| {
            let mut t = crate::scene::transforms::Transform::default();
            t.a = 0.5;
            t.d = 0.5;
            t.e = tx;
            t.f = ty;
            t.variations = HashMap::from([(var.to_string(), 1.0)]);
            t.variation_order = vec![var.to_string()];
            t
        };
        for d in crate::escape::ifs::IFS_DEFS {
            for (label, var) in [("qualifying", "linear"), ("rejected", "spherical")] {
                for c in crate::escape::ifs::IFS_COLORINGS {
                    let mut config = crate::config::FractalConfig::default();
                    config.render_mode = RenderMode::Escape;
                    config.escape.formula = d.name.to_string();
                    config.escape.coloring = c.name.to_string();
                    config.flame.transforms =
                        vec![half(0.0, 0.0, "linear"), half(0.5, 0.0, var)];
                    config.flame.final_transforms.clear();
                    config.flame.xaos = None;
                    let _ = lay_out(config);
                    let _ = label;
                }
            }
        }
    }

    /// Every mode-D parameter and coloring must have somewhere to be
    /// drawn. A def whose params the panel never reaches is a control
    /// the user cannot touch, and the shader reads its default
    /// silently — which looks like the parameter doing nothing.
    #[test]
    fn every_mode_d_parameter_is_reachable_from_the_panel() {
        for d in crate::escape::ifs::IFS_DEFS {
            assert!(!d.parameters.is_empty(), "{} has no parameters to draw", d.name);
            assert!(
                crate::escape::ifs::IFS_COLORINGS
                    .iter()
                    .any(|c| c.name == d.default_coloring),
                "{} defaults to a coloring outside the registry",
                d.name
            );
        }
        for c in crate::escape::ifs::IFS_COLORINGS {
            assert!(!c.parameters.is_empty(), "{} has no parameters to draw", c.name);
        }
    }

    #[test]
    fn the_panel_lays_out_for_every_coloring() {
        for c in crate::escape::COLORINGS {
            let mut config = crate::config::FractalConfig::default();
            config.render_mode = RenderMode::Escape;
            config.escape.coloring = c.name.to_string();
            let _ = lay_out(config);
        }
    }

    /// The "no derivative" hint must match what the SHADER decides.
    ///
    /// The panel is claiming to explain a flat render, so the claim
    /// has to be checked against the thing that actually causes it:
    /// the `HAS_DERIVATIVE` constant the assembler emits. Comparing
    /// against a restatement of the rule would pass even if both
    /// sides were wrong together, so this assembles the real shader
    /// for every formula and reads the constant back out.
    #[test]
    fn the_derivative_hint_matches_what_the_shader_compiles() {
        use crate::escape::{assembler, colorings, DerivativeGap, EscapeRenderer};
        for f in crate::escape::FORMULAS {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = f.name.to_string();
            esc.coloring = "distance_estimate".to_string();

            let src = assembler::assemble(f, &colorings::DISTANCE_ESTIMATE, false);
            let shader_has = src.contains("const HAS_DERIVATIVE: bool = true;");
            let panel_says = EscapeRenderer::derivative_gap(&esc).is_none();
            assert_eq!(
                shader_has, panel_says,
                "{}: the shader compiles HAS_DERIVATIVE={shader_has} but the panel \
                 would tell the user {}",
                f.name,
                if panel_says { "it has one" } else { "it has none" }
            );
        }
    }

    /// ...and the perturbed rungs outrank the formula.
    ///
    /// A Mandelbrot dive is the case that matters: the formula defines
    /// a derivative, so the hint must appear only once the view is
    /// deep enough to leave the direct path — and it must name the
    /// deep path as the reason rather than blaming the formula.
    #[test]
    fn the_derivative_hint_follows_the_deep_path() {
        use crate::escape::{DerivativeGap, EscapeRenderer};
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "mandelbrot".to_string();
        esc.coloring = "distance_estimate".to_string();

        esc.zoom_log2 = 4.0;
        assert_eq!(
            EscapeRenderer::derivative_gap(&esc),
            None,
            "shallow Mandelbrot renders direct and has its derivative"
        );

        esc.zoom_log2 = 30.0;
        assert_eq!(
            EscapeRenderer::derivative_gap(&esc),
            Some(DerivativeGap::Perturbed),
            "a deep dive loses the derivative to the perturbed rungs, and the \
             hint must say so rather than blaming the formula"
        );

        // Damping takes the same view OFF the perturbed path, so the
        // derivative comes back. If the hint were keyed on zoom alone
        // it would keep warning here.
        esc.damping_re = 0.5;
        assert_eq!(
            EscapeRenderer::derivative_gap(&esc),
            None,
            "damping renders direct at any depth, so the derivative is available"
        );
    }

    /// The depth hint must say the right thing for each tier.
    ///
    /// This is the panel's answer to "why has zooming stopped
    /// helping", so a formula that perturbs must not be labelled as
    /// stopping at 2^14, and vice versa.
    #[test]
    fn the_depth_hint_matches_the_engine() {
        use crate::escape::{EscapeRenderer, UsableDepth};
        for (formula, deep) in [
            ("mandelbrot", true),
            ("multibrot", true),
            ("burning_ship", true),
            ("tricorn", true),
            ("phoenix", true),
            ("manowar", true),
            // c*z*(1-z): perturbs since the Lambda tier shipped, so
            // the panel must stop telling users it stops at 2^14.
            ("lambda", true),
            ("feather", true),
            // The big-float families: Kaliset, Ducks (plain log),
            // Newton (polynomial functions) and Nova all perturb now.
            ("kaliset", true),
            ("ducks", true),
            ("newton", true),
            ("nova", true),
            ("tetration", false),
            ("lattes", false),
        ] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = formula.to_string();
            let depth = EscapeRenderer::usable_depth(&esc);
            assert_eq!(
                matches!(depth, UsableDepth::Perturbed),
                deep,
                "{formula}: the panel would tell the user the wrong depth ({depth:?})"
            );
        }
    }

    /// Damping and biomorph take a config OFF the perturbed path, so
    /// the hint has to follow them rather than the formula name.
    #[test]
    fn the_depth_hint_follows_the_settings_that_disable_perturbation() {
        use crate::escape::{EscapeRenderer, UsableDepth};
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "mandelbrot".to_string();
        assert!(matches!(EscapeRenderer::usable_depth(&esc), UsableDepth::Perturbed));
        esc.biomorph = crate::config::escape::BiomorphMode::Re;
        assert!(
            matches!(EscapeRenderer::usable_depth(&esc), UsableDepth::Direct(_)),
            "biomorph disables perturbation, so the hint must stop promising depth"
        );
    }

    /// The suggested scale has to be usable, not just present.
    #[test]
    fn the_suggested_scale_is_in_the_slider_range() {
        for c in crate::escape::COLORINGS {
            let Some(p) = c.parameters.iter().find(|p| p.name == "scale") else {
                continue;
            };
            for max_iter in [64u32, 256, 4000, 60_000] {
                let v = suggested_coloring_scale(c.name, max_iter);
                assert!(
                    v >= p.min && v <= p.max,
                    "{}: suggested scale {v} is outside the slider's {}..={}",
                    c.name,
                    p.min,
                    p.max
                );
            }
        }
    }
}

/// One side of the relief layer — shadows or highlights. They carry
/// exactly the same three controls, and writing them twice is how the
/// two drift apart.
#[allow(clippy::too_many_arguments)]
fn shading_side(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    title: String,
    id: &str,
    color: [f32; 3],
    strength: f32,
    blend: ShadingBlend,
    color_path: ConfigPath,
    strength_path: ConfigPath,
    blend_path: ConfigPath,
) {
    ui.horizontal(|ui| {
        ui.label(title);
        let mut rgb = color;
        if ui.color_edit_button_rgb(&mut rgb).changed() {
            let _ = config_manager.update_param(color_path, ConfigValue::ColorRgb(rgb));
        }
        let mut s = strength;
        if ui
            .add(egui::Slider::new(&mut s, 0.0..=4.0).show_value(true))
            .on_hover_text(t!("escape_panel.tooltip_shading_strength"))
            .changed()
        {
            let _ = config_manager.update_param(strength_path, s.into());
        }
        egui::ComboBox::from_id_salt(format!("escape_shading_blend_{id}"))
            .width(90.0)
            .selected_text(blend_label(blend))
            .show_ui(ui, |ui| {
                for b in ShadingBlend::all() {
                    if ui.selectable_label(b == blend, blend_label(b)).clicked() && b != blend {
                        let _ = config_manager.update_param(
                            blend_path.clone(),
                            ConfigValue::String(
                                crate::config::escape::shading_blend_to_str(b).to_string(),
                            ),
                        );
                    }
                }
            })
            .response
            .on_hover_text(t!("escape_panel.tooltip_shading_blend"));
    });
}

fn blend_label(b: ShadingBlend) -> String {
    match b {
        ShadingBlend::Multiply => t!("escape_panel.blend_multiply").to_string(),
        ShadingBlend::Screen => t!("escape_panel.blend_screen").to_string(),
        ShadingBlend::Overlay => t!("escape_panel.blend_overlay").to_string(),
        ShadingBlend::Mix => t!("escape_panel.blend_mix").to_string(),
    }
}

#[cfg(test)]
mod zoom_display_tests {
    use super::magnification_label;
    use crate::config::escape::EscapeConfig;

    /// The base-10 display must survive depths where the plain
    /// magnification does not.
    ///
    /// `zoom_factor()` is 2^zoom_log2, which is +inf past 1024 — and
    /// real dives run past 9000. Formatting from the LOG is what
    /// keeps the readout meaningful exactly where the user needs it.
    #[test]
    fn magnification_reads_at_any_depth() {
        let mut cfg = EscapeConfig::default();
        for (z2, want) in [
            (426.5725f64, "×2.58e128"),
            (9316.7, "×4.04e2804"),
            (100_000.0, "×9.99e30102"),
        ] {
            cfg.zoom_log2 = z2;
            assert!(
                cfg.zoom_factor().is_infinite() || z2 < 1024.0,
                "test premise: the plain factor overflows up here"
            );
            assert_eq!(magnification_label(cfg.zoom_log10()), want, "at zoom_log2 {z2}");
        }
        // Shallow and zoomed-OUT views stay readable too.
        cfg.zoom_log2 = 0.0;
        assert_eq!(magnification_label(cfg.zoom_log10()), "×1.000");
        cfg.zoom_log2 = -8.0;
        assert_eq!(magnification_label(cfg.zoom_log10()), "×0.004");
        // Never a panic or "NaN" in the panel.
        assert_eq!(magnification_label(f64::NAN), "");
        assert_eq!(magnification_label(f64::INFINITY), "");
    }

    /// The display conversion must round-trip: an edit that does not
    /// move the drag value must not move the zoom. The UI writes
    /// through an f32 config path, so f32 is the bar.
    #[test]
    fn log10_round_trips_through_the_display() {
        let mut cfg = EscapeConfig::default();
        for z2 in [0.0f64, -8.0, 21.5, 426.5725, 9316.7, 100_000.0] {
            cfg.zoom_log2 = z2;
            let back = EscapeConfig::log10_to_log2(cfg.zoom_log10());
            assert!(
                (back - z2).abs() <= z2.abs() * 1e-12 + 1e-12,
                "zoom {z2} came back as {back}"
            );
            // ...and through the f32 path the panel actually writes.
            assert_eq!(back as f32, z2 as f32, "f32 config path moved zoom {z2}");
        }
    }
}

#[cfg(test)]
mod criterion_tests {
    /// The criterion the panel reads must be the one the FORMULA
    /// walks: a solid formula's is the 3D analysis.
    ///
    /// Reported from use: a flame of one inverse-mode
    /// `quaternion_julia` transform rendered perfectly well under
    /// `ifs_flame_3d` while the panel said the variation "is not
    /// affine" -- which was the plane's verdict, and the plane has no
    /// reading of that variation at all. `pack_flame` had had the
    /// same mistake (plan 8.11 step 2) and was fixed there; this is
    /// the panel's half, and the two now answer alike.
    /// Picking a lens seeds every parameter at its registry default,
    /// in one undo step with the name.
    #[test]
    fn choosing_a_lens_seeds_its_parameters() {
        use crate::config::{ConfigPath, ConfigValue};
        let registry = crate::variations::global_registry();
        let changes = super::lens_choice("curl");
        assert_eq!(changes[0].0, ConfigPath::EscapeLens);
        let info = registry.get("curl").expect("curl");
        assert_eq!(changes.len(), 1 + info.parameters.len(), "not every parameter seeded");
        for p in &info.parameters {
            let want = ConfigPath::EscapeLensParam { param: p.name.clone() };
            let got = changes.iter().find(|(path, _)| *path == want).expect("seeded");
            assert_eq!(got.1, ConfigValue::Float(p.default_value), "{} seeded wrong", p.name);
        }
        // Clearing the lens carries no parameters.
        assert_eq!(super::lens_choice("").len(), 1);
    }
    #[test]
    fn a_solid_formula_reads_the_solid_criterion() {
        use crate::scene::transforms::{Flame, Transform};
        let mut t = Transform::default();
        t.a = 1.0;
        t.d = 1.0;
        t.variations = std::collections::HashMap::from([("quaternion_julia".to_string(), 1.0)]);
        t.variation_order = vec!["quaternion_julia".to_string()];
        for (k, v) in [("cx", 0.3f32), ("cy", 0.0), ("cz", 0.0), ("cw", -0.6), ("power", 2.0), ("inverse", 1.0)] {
            t.set_variation_param("quaternion_julia", k, v);
        }
        let mut flame = Flame::default();
        flame.transforms = vec![t];
        flame.final_transforms.clear();
        flame.xaos = None;

        // The solid criterion accepts it, and says it is nonlinear.
        match super::ifs_verdict(&flame, true) {
            super::Verdict::Ok { maps, roots, radius, .. } => {
                assert_eq!((maps, roots), (1, 1));
                assert!(radius > 0.0 && radius.is_finite());
            }
            super::Verdict::No(why) => panic!("the solid criterion rejected it: {why:?}"),
        }
        // The plane's does not, which is correct and is what the panel
        // used to say over a solid.
        assert!(
            matches!(super::ifs_verdict(&flame, false), super::Verdict::No(_)),
            "the plane has no reading of quaternion_julia"
        );

        // And the panel agrees with the renderer: what `pack_for`
        // builds for this formula has the solid the walk needs.
        let mut cfg = crate::config::FractalConfig::default();
        cfg.flame = flame;
        cfg.escape.formula = "ifs_flame_3d".to_string();
        let def = crate::escape::ifs::get_ifs(&cfg.escape.formula).expect("a mode-D def");
        assert!(def.solid && def.needs_flame);
        let registry = crate::variations::global_registry();
        let packed = crate::escape::ifs::pack_for(def, &cfg, &registry).expect("packs");
        let (ifs3, rows) = packed.solid.as_ref().expect("a solid reading");
        assert_eq!((ifs3.maps.len(), rows.len()), (1, 1));
    }
}
