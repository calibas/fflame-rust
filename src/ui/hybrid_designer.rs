//! The Hybrid Designer: Kalles Fraktaler-style hybrid formulas
//! (`escape::lang::hybrid`; plan, phase 3, step 5).
//!
//! Groups of lines of operators, each group run its repeat count of
//! passes, looping from the group marked as the loop's start. The
//! hybrid in use is the config's custom formula (dialect `Hybrid`, its
//! JSON as the source), so it saves, undoes and renders anywhere as a
//! custom formula does. A structural edit is a new source: it compiles
//! and applies at once, one undo step. A multiplier is the program's
//! parameter, written as any formula parameter is, so dragging one
//! recompiles nothing.

use crate::config::escape::{CustomDialect, CustomSource};
use crate::config::manager::custom_source_json;
use crate::config::{ConfigManager, ConfigPath, ConfigValue};
use crate::escape::custom;
use crate::escape::lang::hybrid::{self, Combine, Group, Hybrid, Line, Operator};
use crate::escape::lang::EntryKind;

pub struct HybridDesignerPanel {
    /// What the panel shows: the config's hybrid when it uses one, else
    /// the one being put together.
    hybrid: Hybrid,
    preset: usize,
    status: Option<String>,
}

impl Default for HybridDesignerPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// The config's hybrid, when its formula is one.
fn in_use(cm: &ConfigManager) -> Option<Hybrid> {
    let escape = &cm.config().escape;
    let src = escape.custom_formula.as_ref()?;
    if src.dialect != CustomDialect::Hybrid || !escape.formula.starts_with(custom::PREFIX) {
        return None;
    }
    Hybrid::from_json(&src.source).ok()
}

impl HybridDesignerPanel {
    pub fn new() -> Self {
        let presets = hybrid::presets();
        HybridDesignerPanel { hybrid: presets[0].1.clone(), preset: 0, status: None }
    }

    /// Put `h` in the config as the formula: one undo step, with each
    /// multiplier's parameter set to the hybrid's value (a structural
    /// edit renames operators, and a stale value must not follow a
    /// name to another operator).
    fn apply(&mut self, cm: &mut ConfigManager, h: Hybrid) {
        let src = CustomSource { dialect: CustomDialect::Hybrid, entry: "Hybrid".into(), source: h.to_json() };
        let name = custom::name_of(EntryKind::Formula, &src);
        if let Err(d) = custom::compile_formula(&src, &name) {
            let first = d.first().map(|x| x.message.clone()).unwrap_or_default();
            self.status = Some(t!("hybrid_designer.did_not_compile", error = first).to_string());
            return;
        }
        let first_use = in_use(cm).is_none();
        let mut changes = vec![
            (ConfigPath::EscapeCustomFormula, ConfigValue::String(custom_source_json(&Some(src)))),
            (ConfigPath::EscapeFormula, ConfigValue::String(name)),
        ];
        for (g, group) in h.groups.iter().enumerate() {
            for (l, line) in group.lines.iter().enumerate() {
                for (second, op) in std::iter::once((false, &line.one)).chain(line.two.as_ref().map(|o| (true, o))) {
                    let p = Hybrid::param_name(g, l, second);
                    changes.push((ConfigPath::EscapeFormulaParam { param: format!("{p}.re") }, (op.mul[0] as f32).into()));
                    changes.push((ConfigPath::EscapeFormulaParam { param: format!("{p}.im") }, (op.mul[1] as f32).into()));
                }
            }
        }
        if first_use {
            // The parameter plane's usual frame for a fresh formula.
            changes.push((ConfigPath::EscapeCenterRe, ConfigValue::String("-0.5".into())));
            changes.push((ConfigPath::EscapeCenterIm, ConfigValue::String("0".into())));
            changes.push((ConfigPath::EscapeZoomLog2, 0.0f32.into()));
        }
        match cm.update_batch(changes, "history.action.use_hybrid".to_string()) {
            Ok(_) => {
                self.hybrid = h;
                self.status = None;
            }
            Err(e) => self.status = Some(format!("{e:?}")),
        }
    }

    /// The hybrid with each multiplier as the config holds it now, so a
    /// structural edit keeps the values dragged since the last one.
    fn with_live_multipliers(&self, cm: &ConfigManager) -> Hybrid {
        let params = &cm.config().escape.formula_params;
        let mut h = self.hybrid.clone();
        for (g, group) in h.groups.iter_mut().enumerate() {
            for (l, line) in group.lines.iter_mut().enumerate() {
                let take = |op: &mut Operator, second: bool| {
                    let p = Hybrid::param_name(g, l, second);
                    if let Some(v) = params.get(&format!("{p}.re")) {
                        op.mul[0] = f64::from(*v);
                    }
                    if let Some(v) = params.get(&format!("{p}.im")) {
                        op.mul[1] = f64::from(*v);
                    }
                };
                take(&mut line.one, false);
                if let Some(two) = line.two.as_mut() {
                    take(two, true);
                }
            }
        }
        h
    }

    pub fn render(&mut self, ui: &mut egui::Ui, cm: &mut ConfigManager) {
        let live = in_use(cm);
        if let Some(h) = &live {
            if *h != self.hybrid {
                self.hybrid = h.clone();
            }
        }
        let presets = hybrid::presets();

        // ---- Presets ----
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("hybrid_designer.preset"));
            egui::ComboBox::from_id_salt("hybrid_designer_preset")
                .selected_text(presets.get(self.preset).map_or("", |p| p.0))
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in presets.iter().enumerate() {
                        ui.selectable_value(&mut self.preset, i, *name);
                    }
                });
            if ui.button(t!("hybrid_designer.use_preset")).on_hover_text(t!("hybrid_designer.use_preset_tip")).clicked() {
                if let Some((_, h)) = presets.get(self.preset) {
                    self.apply(cm, h.clone());
                }
            }
        });
        if live.is_none() {
            ui.horizontal_wrapped(|ui| {
                ui.label(t!("hybrid_designer.not_in_use"));
                if ui.button(t!("hybrid_designer.use")).clicked() {
                    let h = self.hybrid.clone();
                    self.apply(cm, h);
                }
            });
        }
        if let Some(s) = &self.status {
            ui.colored_label(ui.visuals().warn_fg_color, s);
        }
        ui.separator();

        // ---- Groups ----
        // Edits gather into `next`; a structural one applies at the end.
        let mut next = if live.is_some() { self.with_live_multipliers(cm) } else { self.hybrid.clone() };
        let mut structural = false;
        let mut multipliers: Vec<(String, f32)> = Vec::new();
        let n_groups = next.groups.len();
        let mut remove_group: Option<usize> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for g in 0..n_groups {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(t!("hybrid_designer.group", n = g + 1));
                        ui.label(t!("hybrid_designer.repeats"));
                        let mut r = next.groups[g].repeats;
                        // A recompile a value: the drag commits on release.
                        let (_, changed) = super::slow_controls::on_release(ui, egui::Id::new(("hybrid_repeats", g)), &mut r, |ui, v| {
                            ui.add(egui::DragValue::new(v).range(1..=hybrid::MAX_REPEATS).speed(0.05))
                        });
                        if changed && r != next.groups[g].repeats {
                            next.groups[g].repeats = r;
                            structural = true;
                        }
                        if ui
                            .radio(next.loop_start == g, t!("hybrid_designer.loop_start"))
                            .on_hover_text(t!("hybrid_designer.loop_start_tip"))
                            .clicked()
                            && next.loop_start != g
                        {
                            next.loop_start = g;
                            structural = true;
                        }
                        if n_groups > 1 && ui.small_button("🗑").on_hover_text(t!("hybrid_designer.remove_group")).clicked() {
                            remove_group = Some(g);
                        }
                    });
                    let n_lines = next.groups[g].lines.len();
                    let mut remove_line: Option<usize> = None;
                    for l in 0..n_lines {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(t!("hybrid_designer.line", n = l + 1));
                            let line = &mut next.groups[g].lines[l];
                            structural |= operator_ui(ui, &mut line.one, (g, l, false), &mut multipliers);
                            match line.two.as_mut() {
                                Some(two) => {
                                    let before = line.combine;
                                    egui::ComboBox::from_id_salt(("hybrid_combine", g, l))
                                        .selected_text(line.combine.symbol())
                                        .width(36.0)
                                        .show_ui(ui, |ui| {
                                            for c in Combine::ALL {
                                                ui.selectable_value(&mut line.combine, c, c.symbol());
                                            }
                                        });
                                    structural |= line.combine != before;
                                    structural |= operator_ui(ui, two, (g, l, true), &mut multipliers);
                                    if ui.small_button("−").on_hover_text(t!("hybrid_designer.remove_operator")).clicked() {
                                        line.two = None;
                                        structural = true;
                                    }
                                }
                                None => {
                                    if ui.small_button("+").on_hover_text(t!("hybrid_designer.add_operator")).clicked() {
                                        line.two = Some(Operator { power: 1, mul: [0.0, 0.0], ..Operator::default() });
                                        structural = true;
                                    }
                                }
                            }
                            if n_lines > 1 && ui.small_button("🗑").on_hover_text(t!("hybrid_designer.remove_line")).clicked() {
                                remove_line = Some(l);
                            }
                        });
                    }
                    if let Some(l) = remove_line {
                        next.groups[g].lines.remove(l);
                        structural = true;
                    }
                    if n_lines < hybrid::MAX_LINES && ui.small_button(t!("hybrid_designer.add_line")).clicked() {
                        next.groups[g].lines.push(Line { one: Operator { power: 1, ..Operator::default() }, two: None, combine: Combine::Add });
                        structural = true;
                    }
                });
            }
            if let Some(g) = remove_group {
                next.groups.remove(g);
                if next.loop_start >= next.groups.len() || next.loop_start > g {
                    next.loop_start = next.loop_start.saturating_sub(1);
                }
                structural = true;
            }
            if next.groups.len() < hybrid::MAX_GROUPS && ui.button(t!("hybrid_designer.add_group")).clicked() {
                next.groups.push(Group { repeats: 1, lines: vec![Line { one: Operator::default(), two: None, combine: Combine::Add }] });
                structural = true;
            }
            ui.add_space(4.0);
            let cycle = next.cycle();
            ui.weak(t!("hybrid_designer.cycle", prefix = cycle.prefix, period = cycle.period));
            ui.weak(t!("hybrid_designer.formula_text"));
            ui.weak(formula_line(&next));
        });

        if structural {
            if live.is_some() {
                self.apply(cm, next);
            } else {
                self.hybrid = next;
            }
        } else if !multipliers.is_empty() {
            if live.is_some() {
                for (param, v) in multipliers {
                    let _ = cm.update_param(ConfigPath::EscapeFormulaParam { param }, v.into());
                }
            } else {
                self.hybrid = next;
            }
        }
    }
}

/// An operator's controls: the folds and negations, the power, the
/// multiplier. Returns whether its structure changed; a multiplier
/// change is pushed to `multipliers` (and written into `op`).
fn operator_ui(ui: &mut egui::Ui, op: &mut Operator, (g, l, second): (usize, usize, bool), multipliers: &mut Vec<(String, f32)>) -> bool {
    let mut changed = false;
    changed |= ui.toggle_value(&mut op.abs_x, "|x|").on_hover_text(t!("hybrid_designer.abs_x_tip")).changed();
    changed |= ui.toggle_value(&mut op.abs_y, "|y|").on_hover_text(t!("hybrid_designer.abs_y_tip")).changed();
    changed |= ui.toggle_value(&mut op.neg_x, "−x").on_hover_text(t!("hybrid_designer.neg_x_tip")).changed();
    changed |= ui.toggle_value(&mut op.neg_y, "−y").on_hover_text(t!("hybrid_designer.neg_y_tip")).changed();
    ui.label("^");
    let mut p = op.power;
    let (response, committed) = super::slow_controls::on_release(ui, egui::Id::new(("hybrid_power", g, l, second)), &mut p, |ui, v| {
        ui.add(egui::DragValue::new(v).range(1..=hybrid::MAX_POWER).speed(0.05))
    });
    response.on_hover_text(t!("hybrid_designer.power_tip"));
    if committed && p != op.power {
        op.power = p;
        changed = true;
    }
    ui.label("×");
    let name = Hybrid::param_name(g, l, second);
    for (k, part) in ["re", "im"].iter().enumerate() {
        let mut v = op.mul[k];
        if ui.add(egui::DragValue::new(&mut v).speed(0.005).max_decimals(4)).on_hover_text(t!("hybrid_designer.multiplier_tip")).changed() {
            op.mul[k] = v;
            multipliers.push((format!("{name}.{part}"), v as f32));
        }
    }
    changed
}

/// The hybrid as one line of arithmetic, for the reader.
fn formula_line(h: &Hybrid) -> String {
    let op_text = |o: &Operator| {
        let part = |n: &str, abs: bool, neg: bool| format!("{}{}", if neg { "−" } else { "" }, if abs { format!("|{n}|") } else { n.to_string() });
        let base = if o.abs_x || o.abs_y || o.neg_x || o.neg_y {
            format!("({} + {}i)", part("x", o.abs_x, o.neg_x), part("y", o.abs_y, o.neg_y))
        } else {
            "z".to_string()
        };
        let pow = if o.power == 1 { base } else { format!("{base}^{}", o.power) };
        if o.mul == [1.0, 0.0] {
            pow
        } else {
            format!("({}, {})·{pow}", o.mul[0], o.mul[1])
        }
    };
    h.groups
        .iter()
        .enumerate()
        .map(|(g, group)| {
            let lines: Vec<String> = group
                .lines
                .iter()
                .map(|line| match &line.two {
                    None => op_text(&line.one),
                    Some(t) => format!("{} {} {}", op_text(&line.one), line.combine.symbol(), op_text(t)),
                })
                .collect();
            format!("{}{}×[z = {}; + c]", if g == h.loop_start { "⟳ " } else { "" }, group.repeats, lines.join("; z = "))
        })
        .collect::<Vec<_>>()
        .join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, panel: &mut HybridDesignerPanel, cm: &mut ConfigManager) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 600.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            panel.render(ui, cm);
        });
    }

    /// The panel draws with and without a hybrid in use; using one puts
    /// it in the config as the formula, one undo step, with each
    /// multiplier's parameter; a structural edit keeps a dragged
    /// multiplier.
    #[test]
    fn a_hybrid_goes_into_the_config_and_back_out() {
        let ctx = egui::Context::default();
        let mut cm = ConfigManager::new(crate::config::FractalConfig::default());
        let mut panel = HybridDesignerPanel::new();
        frame(&ctx, &mut panel, &mut cm);
        assert!(in_use(&cm).is_none());

        let (_, alternating) = hybrid::presets().into_iter().find(|(n, _)| *n == "Mandelbrot / Burning Ship").unwrap();
        panel.apply(&mut cm, alternating.clone());
        assert_eq!(in_use(&cm), Some(alternating.clone()), "{:?}", panel.status);
        let escape = &cm.config().escape;
        assert!(custom::formula(&escape.formula).is_some(), "compiled and registered");
        let a = |cm: &ConfigManager, p: &str| f32::try_from(cm.get_value(&ConfigPath::EscapeFormulaParam { param: p.into() }).unwrap()).unwrap();
        assert_eq!(a(&cm, "a2_1_1.re"), 1.0);
        frame(&ctx, &mut panel, &mut cm);

        // A dragged multiplier, then a structural edit: the value stays.
        cm.update_param(ConfigPath::EscapeFormulaParam { param: "a2_1_1.re".into() }, 0.5f32.into()).unwrap();
        let mut more = panel.with_live_multipliers(&cm);
        assert_eq!(more.groups[1].lines[0].one.mul, [0.5, 0.0]);
        more.groups[1].repeats = 3;
        panel.apply(&mut cm, more);
        let now = in_use(&cm).unwrap();
        assert_eq!((now.groups[1].repeats, now.groups[1].lines[0].one.mul), (3, [0.5, 0.0]));
        frame(&ctx, &mut panel, &mut cm);

        // Undone, the hybrid before the edit.
        cm.undo().unwrap();
        assert_eq!(in_use(&cm).unwrap().groups[1].repeats, 1);
    }
}
