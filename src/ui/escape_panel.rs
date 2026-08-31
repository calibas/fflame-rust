//! Escape Fractal panel — the whole editing surface for escape-time
//! mode (the flame-only panels stay hidden rather than learning a
//! second vocabulary; see docs/projects/escape-time-fractals.md §3).
//!
//! Every control writes through `config_manager.update_param` with an
//! `Escape*` ConfigPath, so undo/redo, coalescing, animation tracks and
//! script `config.set` all work unmodified. Formula/coloring parameter
//! sliders are generated from the registry defs, the way variation
//! params generate theirs.

use crate::config::escape::{ShadingBlend, ShadingField};
use crate::config::{ConfigManager, ConfigPath, ConfigValue};
use crate::scene::transforms::RenderMode;
use rust_i18n::t;

/// Render the Escape Fractal panel.
pub fn render_escape_content(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    let config = config_manager.active_config().clone();
    let esc = config.escape.clone();

    // ---- Mode switch (same row the View panel shows, plus Escape,
    // so the user can leave escape mode without hunting for View) ----
    ui.label(t!("view.render_mode"));
    ui.horizontal(|ui| {
        let mode = config.render_mode;
        for (m, label) in [
            (RenderMode::TwoD, t!("view.mode_2d")),
            (RenderMode::ThreeD, t!("view.mode_3d")),
            (RenderMode::Escape, t!("view.mode_escape")),
        ] {
            if ui.selectable_label(mode == m, label.as_ref()).clicked() && mode != m {
                if let Err(e) = switch_render_mode(config_manager, m) {
                    log::error!("Failed to switch render mode: {e}");
                }
            }
        }
    });

    if config.render_mode != RenderMode::Escape {
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
    let formula = crate::escape::get_formula(&esc.formula);
    let selected_label = match field {
        Some(f) => f.display_name,
        None => formula.display_name,
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
                        let _ = config_manager.update_param(
                            ConfigPath::EscapeFormula,
                            ConfigValue::String(f.name.to_string()),
                        );
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
                        let _ = config_manager.update_param(
                            ConfigPath::EscapeFormula,
                            ConfigValue::String(f.name.to_string()),
                        );
                    }
                }
            });
    });

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
                zoom = format!("{limit:.0}")
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
    let formula_params = match field {
        Some(f) => f.parameters,
        None => formula.parameters,
    };
    for p in formula_params {
        let mut v = esc.formula_params.get(p.name).copied().unwrap_or(p.default);
        let resp = ui
            .add(egui::Slider::new(&mut v, p.min..=p.max).text(p.display_name))
            .on_hover_text(p.tooltip);
        if resp.changed() {
            let _ = config_manager.update_param(
                ConfigPath::EscapeFormulaParam { param: p.name.to_string() },
                v.into(),
            );
        }
    }

    // ---- Julia toggle ---- (mode A only: fields have no Julia plane)
    let mut julia = esc.julia;
    if field.is_none()
        && ui
            .checkbox(&mut julia, t!("escape_panel.julia").as_ref())
            .on_hover_text(t!("escape_panel.tooltip_julia"))
            .changed()
    {
        let _ = config_manager.update_param(ConfigPath::EscapeJulia, julia.into());
    }
    if esc.julia {
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
        ui.label(t!("escape_panel.zoom_log2"));
        let mut z = esc.zoom_log2 as f32;
        if ui
            .add(egui::DragValue::new(&mut z).speed(0.02))
            .on_hover_text(t!("escape_panel.tooltip_zoom_log2"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeZoomLog2, z.into());
        }
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
                                    zoom = (-oct - 16).max(0)
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
        let current = esc.supersample.clamp(1, 3);
        egui::ComboBox::from_id_salt("escape_supersample")
            .selected_text(match current {
                1 => t!("escape_panel.supersample_off").to_string(),
                n => format!("{n}\u{00d7}"),
            })
            .show_ui(ui, |ui| {
                for n in 1u32..=3 {
                    let label = match n {
                        1 => t!("escape_panel.supersample_off").to_string(),
                        n => format!("{n}\u{00d7}"),
                    };
                    if ui.selectable_label(n == current, label).clicked() && n != current {
                        let _ = config_manager
                            .update_param(ConfigPath::EscapeSupersample, ConfigValue::UInt(n));
                    }
                }
            })
            .response
            .on_hover_text(t!("escape_panel.tooltip_supersample"));
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
                        .add(egui::Slider::new(&mut sf, 0.0..=8.0).step_by(1.0))
                        .on_hover_text(t!("escape_panel.tooltip_shading_softness"))
                        .changed()
                    {
                        let _ = config_manager
                            .update_param(ConfigPath::EscapeShadingSoftness, sf.into());
                    }
                });
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

    // Mann-iteration damping (complex α; 1+0i = plain iteration)
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

    // Biomorph classification axis
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

    ui.separator();

    // ---- Coloring ----
    show_coloring_section(ui, config_manager, &esc, field);
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
fn suggested_coloring_scale(coloring: &str, max_iter: u32) -> f32 {
    match coloring {
        "smooth" | "escape_count" | "period" => {
            // A few cycles over a typical escape range, floored so a
            // huge cap does not flatten the image to one band.
            (8.0 / (max_iter as f32).max(1.0)).clamp(0.005, 0.5)
        }
        "distance_estimate" | "triangle_inequality" | "root_basin" => 1.0,
        // Orbit traps and the averaging family already live at O(1).
        _ => 1.0,
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
) {
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
                    .on_hover_text(t!("escape_panel.auto_scale_tip", value = format!("{suggested:.4}")))
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
                            value = format!("{suggested:.4}")
                        ))
                        .small()
                        .weak(),
                    );
                }
            });
        }
        for p in coloring.parameters {
            let mut v = esc.coloring_params.get(p.name).copied().unwrap_or(p.default);
            let resp = ui
                .add(egui::Slider::new(&mut v, p.min..=p.max).text(p.display_name))
                .on_hover_text(p.tooltip);
            if resp.changed() {
                let _ = config_manager.update_param(
                    ConfigPath::EscapeColoringParam { param: p.name.to_string() },
                    v.into(),
                );
            }
        }
        return;
    }
    let coloring = crate::escape::get_coloring(&esc.coloring);
    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.coloring"));
        egui::ComboBox::from_id_salt("escape_coloring")
            .selected_text(coloring.display_name)
            .show_ui(ui, |ui| {
                for c in crate::escape::COLORINGS {
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
                .on_hover_text(t!("escape_panel.auto_scale_tip", value = format!("{suggested:.4}")))
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
                        value = format!("{suggested:.4}")
                    ))
                    .small()
                    .weak(),
                );
            }
        });
    }
    for p in coloring.parameters {
        let mut v = esc.coloring_params.get(p.name).copied().unwrap_or(p.default);
        let resp = ui
            .add(egui::Slider::new(&mut v, p.min..=p.max).text(p.display_name))
            .on_hover_text(p.tooltip);
        if resp.changed() {
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
pub fn switch_render_mode(
    config_manager: &mut ConfigManager,
    mode: RenderMode,
) -> Result<(), crate::config::manager::ConfigError> {
    let config = config_manager.active_config();
    let entering_escape = mode == RenderMode::Escape && config.render_mode != RenderMode::Escape;
    let default_tonemap = entering_escape
        && config.tonemap_mode == crate::scene::tonemap::ToneMapMode::Logarithmic;
    if default_tonemap {
        config_manager.update_batch(
            vec![
                (ConfigPath::RenderMode, mode.into()),
                (
                    ConfigPath::TonemapMode,
                    crate::scene::tonemap::ToneMapMode::Linear.into(),
                ),
                (
                    ConfigPath::Exposure,
                    crate::config::defaults::DEFAULT_EXPOSURE.into(),
                ),
                (
                    ConfigPath::Gamma,
                    crate::config::defaults::DEFAULT_GAMMA.into(),
                ),
            ],
            "history.param.render_mode".to_string(),
        )
        .map(|_| ())
    } else {
        config_manager.update_param(ConfigPath::RenderMode, mode.into()).map(|_| ())
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
                    render_escape_content(ui, &mut manager);
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
            ("kaliset", false),
            ("newton", false),
            ("tetration", false),
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
            .add(egui::Slider::new(&mut s, 0.0..=1.0).show_value(true))
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
