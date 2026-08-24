//! Rendering a script's declared parameters.
//!
//! Shared, because the Scripts panel is no longer the only surface that
//! runs scripts: the Palette Editor offers the palette-flagged ones, and
//! anything else that grows a "run a script" section will want the same
//! controls rather than its own near-copy.

use std::collections::HashMap;

use egui;

use crate::script::{ParamDecl, ParamValue, ScriptMeta};

/// Draw a control per declared parameter, writing edits into `values`.
///
/// Values are keyed by parameter name and left untouched where the user
/// has not set one, so a script keeps its own defaults.
pub fn render(ui: &mut egui::Ui, meta: &ScriptMeta, values: &mut HashMap<String, ParamValue>) {

        if meta.params.is_empty() {
            ui.label(egui::RichText::new("This script has no settings.").weak());
            return;
        }

        for decl in &meta.params {
            let key = decl.key().to_string();
            match decl {
                ParamDecl::Color { key: _, label, default } => {
                    let mut v = match values.get(&key) {
                        Some(ParamValue::Color(v)) => *v,
                        _ => *default,
                    };
                    ui.horizontal(|ui| {
                        // The same picker the Palette Editor, Solid panel
                        // and background colour already use.
                        if ui.color_edit_button_rgb(&mut v).changed() {
                            values.insert(key.clone(), ParamValue::Color(v));
                        }
                        ui.label(label.clone());
                    });
                }
                ParamDecl::Float { label, default, min, max, .. } => {
                    let mut v = match values.get(&key) {
                        Some(ParamValue::Float(v)) => *v,
                        _ => *default,
                    };
                    if ui
                        .add(super::VkbSlider::new(&mut v, *min..=*max).text(label.clone()))
                        .changed()
                    {
                        values.insert(key, ParamValue::Float(v));
                    }
                }
                ParamDecl::Int { label, default, min, max, .. } => {
                    let mut v = match values.get(&key) {
                        Some(ParamValue::Int(v)) => *v,
                        _ => *default,
                    };
                    if ui
                        .add(super::VkbSlider::new(&mut v, *min..=*max).text(label.clone()))
                        .changed()
                    {
                        values.insert(key, ParamValue::Int(v));
                    }
                }
                ParamDecl::Bool { label, default, .. } => {
                    let mut v = match values.get(&key) {
                        Some(ParamValue::Bool(v)) => *v,
                        _ => *default,
                    };
                    if ui.checkbox(&mut v, label.clone()).changed() {
                        values.insert(key, ParamValue::Bool(v));
                    }
                }
                ParamDecl::Text { label, default, max_len, .. } => {
                    let mut v = match values.get(&key) {
                        Some(ParamValue::Text(v)) => v.clone(),
                        _ => default.clone(),
                    };
                    ui.horizontal(|ui| {
                        ui.label(label.clone());
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut v)
                                // Keyed by parameter, not by position: the
                                // param list is rebuilt from the script every
                                // frame, and positional ids let the caret and
                                // selection bleed between params (and between
                                // scripts that happen to lay out alike).
                                .id_salt(&key)
                                .char_limit(*max_len)
                                .desired_width(f32::INFINITY),
                        );
                        crate::ui::vkb_sync(ui, &r, &v);
                        if r.changed() {
                            values.insert(key.clone(), ParamValue::Text(v.clone()));
                        }
                    });
                }
                ParamDecl::Choice { label, options, default, .. } => {
                    let mut idx = match values.get(&key) {
                        Some(ParamValue::Choice(i)) => *i,
                        _ => *default,
                    };
                    let shown = options.get(idx).cloned().unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(label.clone());
                        egui::ComboBox::from_id_salt(format!("script_choice_{key}"))
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (i, opt) in options.iter().enumerate() {
                                    if ui.selectable_label(i == idx, opt).clicked() {
                                        idx = i;
                                        values.insert(key.clone(), ParamValue::Choice(i));
                                    }
                                }
                            });
                    });
                }
            }
        }
}
