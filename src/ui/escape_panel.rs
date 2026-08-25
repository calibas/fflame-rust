//! Escape Fractal panel — the whole editing surface for escape-time
//! mode (the flame-only panels stay hidden rather than learning a
//! second vocabulary; see docs/projects/escape-time-fractals.md §3).
//!
//! Every control writes through `config_manager.update_param` with an
//! `Escape*` ConfigPath, so undo/redo, coalescing, animation tracks and
//! script `config.set` all work unmodified. Formula/coloring parameter
//! sliders are generated from the registry defs, the way variation
//! params generate theirs.

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

    ui.separator();

    // ---- Formula ----
    let formula = crate::escape::get_formula(&esc.formula);
    ui.horizontal(|ui| {
        ui.label(t!("escape_panel.formula"));
        egui::ComboBox::from_id_salt("escape_formula")
            .selected_text(formula.display_name)
            .show_ui(ui, |ui| {
                for f in crate::escape::FORMULAS {
                    if ui
                        .selectable_label(f.name == formula.name, f.display_name)
                        .clicked()
                        && f.name != formula.name
                    {
                        let _ = config_manager.update_param(
                            ConfigPath::EscapeFormula,
                            ConfigValue::String(f.name.to_string()),
                        );
                    }
                }
            });
    });

    // Formula parameters, straight from the def (slider bounds and
    // tooltips included). Values read def defaults when unset — the
    // same value the shader's packer uses.
    for p in formula.parameters {
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

    // ---- Julia toggle ----
    let mut julia = esc.julia;
    if ui
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
            .add(egui::DragValue::new(&mut iter).speed(4).range(1..=1_000_000))
            .on_hover_text(t!("escape_panel.tooltip_max_iter"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeMaxIter, ConfigValue::UInt(iter));
        }
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
    let new_z = (z + factor.log2()).clamp(-8.0, 45.0);
    let _ = config_manager.update_param(ConfigPath::EscapeZoomLog2, (new_z as f32).into());
}
