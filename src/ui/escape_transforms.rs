//! The escape panel's **Transformations** (the KIFS plan, C4): flame
//! variations applied to the sample point in the fractal's coordinates
//! (Plane, UF's transformations), to each orbit value the colouring
//! accumulates (Orbit, UF's trap position), or to `z` after every step
//! (Iteration). They render through the lens's maps flame
//! (`escape::lens`); this is only their editor.
//!
//! A structural edit -- add, remove, move, a stage or a variation -- writes
//! the whole list (`ConfigPath::EscapeTransforms`) as one undo step; an
//! amount or a parameter writes its own path, which animation can target.

use crate::config::escape::{EscapeTransform, TransformStage};
use crate::config::{ConfigManager, ConfigPath, ConfigValue};
use crate::ui::variation_params::{render_variation_params, ParamTarget};
use crate::variations::{VariationCategory, VariationParameter};

use egui_dock::egui;
use rust_i18n::t;

/// One transformation's parameters, as `render_variation_params` edits
/// them: its map, its paths.
struct TransformTarget {
    index: usize,
}

impl ParamTarget for TransformTarget {
    fn stored(&self, config_manager: &ConfigManager, _var_name: &str, param: &VariationParameter) -> Option<f32> {
        let t = config_manager.active_config().escape.transforms.get(self.index)?;
        Some(t.params.get(&param.name).copied().unwrap_or(param.default_value))
    }

    fn path(&self, _var_name: &str, param_name: &str) -> ConfigPath {
        ConfigPath::EscapeTransformParam { index: self.index, param: param_name.to_string() }
    }
}

fn stage_label(stage: TransformStage) -> String {
    match stage {
        TransformStage::Plane => t!("escape_transforms.stage_plane").to_string(),
        TransformStage::Orbit => t!("escape_transforms.stage_orbit").to_string(),
        TransformStage::Iteration => t!("escape_transforms.stage_iteration").to_string(),
    }
}

fn stage_tip(stage: TransformStage) -> String {
    match stage {
        TransformStage::Plane => t!("escape_transforms.stage_plane_tip").to_string(),
        TransformStage::Orbit => t!("escape_transforms.stage_orbit_tip").to_string(),
        TransformStage::Iteration => t!("escape_transforms.stage_iteration_tip").to_string(),
    }
}

/// The list written whole, plus -- when a variation of the user's is
/// now used -- its source carried in the flame, one undo step.
fn write_list(config_manager: &mut ConfigManager, list: &[EscapeTransform], description: &str) {
    let mut changes = vec![(ConfigPath::EscapeTransforms, ConfigValue::String(serde_json::to_string(list).unwrap_or_default()))];
    #[cfg(feature = "engine-escape")]
    if let Some(carry) = crate::variations::custom::carry_change(config_manager.active_config(), list.iter().map(|t| t.variation.as_str())) {
        changes.push(carry);
    }
    let _ = config_manager.update_batch(changes, description.to_string());
}

/// A new entry: the variation with every parameter at its default, so
/// an undo of a first edit never writes a 0 the user did not see (the
/// lens's `lens_choice` for the same reason).
fn entry(stage: TransformStage, variation: &str, amount: f32) -> EscapeTransform {
    let mut params = std::collections::BTreeMap::new();
    if let Some(info) = crate::variations::global_registry().get(variation) {
        for p in &info.parameters {
            params.insert(p.name.clone(), p.default_value);
        }
    }
    EscapeTransform { stage, variation: variation.to_string(), amount, params }
}

/// The section: each transformation's stage, variation, amount and
/// parameters, with move and remove, and Add.
pub(super) fn show_transforms_section(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    let list = config_manager.active_config().escape.transforms.clone();
    egui::CollapsingHeader::new(t!("escape_transforms.title"))
        .id_salt("escape_transforms")
        .default_open(!list.is_empty())
        .show(ui, |ui| {
            ui.label(egui::RichText::new(t!("escape_transforms.tip")).small().weak());
            let mut edit: Option<(Vec<EscapeTransform>, &'static str)> = None;
            for (i, t) in list.iter().enumerate() {
                ui.push_id(("escape_transform", i), |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // The stage.
                            egui::ComboBox::from_id_salt("stage").selected_text(stage_label(t.stage)).show_ui(ui, |ui| {
                                for stage in TransformStage::ALL {
                                    if ui.selectable_label(t.stage == stage, stage_label(stage)).on_hover_text(stage_tip(stage)).clicked() && t.stage != stage {
                                        let mut l = list.clone();
                                        l[i].stage = stage;
                                        edit = Some((l, "history.action.escape_transform_stage"));
                                    }
                                }
                            });
                            // The variation.
                            if let Some(name) = variation_picker(ui, i, &t.variation) {
                                let mut l = list.clone();
                                l[i] = entry(t.stage, &name, t.amount);
                                edit = Some((l, "history.action.escape_transform_variation"));
                            }
                            if ui.small_button("\u{2191}").on_hover_text(t!("escape_transforms.up")).clicked() && i > 0 {
                                let mut l = list.clone();
                                l.swap(i, i - 1);
                                edit = Some((l, "history.action.escape_transform_move"));
                            }
                            if ui.small_button("\u{2193}").on_hover_text(t!("escape_transforms.down")).clicked() && i + 1 < list.len() {
                                let mut l = list.clone();
                                l.swap(i, i + 1);
                                edit = Some((l, "history.action.escape_transform_move"));
                            }
                            if ui.small_button("\u{2715}").on_hover_text(t!("escape_transforms.remove")).clicked() {
                                let mut l = list.clone();
                                l.remove(i);
                                edit = Some((l, "history.action.escape_transform_remove"));
                            }
                        });
                        let mut a = t.amount;
                        let lim = crate::config::escape::LENS_AMOUNT_LIMIT;
                        if ui
                            .add(egui::Slider::new(&mut a, -lim..=lim).text(t!("escape_transforms.amount")))
                            .on_hover_text(t!("escape_transforms.amount_tip"))
                            .changed()
                        {
                            let _ = config_manager.update_param(ConfigPath::EscapeTransformAmount { index: i }, a.into());
                        }
                        let registry = crate::variations::global_registry();
                        match registry.get(&t.variation) {
                            Some(info) if !info.parameters.is_empty() => {
                                let params = info.parameters.clone();
                                drop(registry);
                                render_variation_params(ui, config_manager, &TransformTarget { index: i }, &t.variation, &params);
                            }
                            Some(_) => {}
                            None => {
                                ui.colored_label(egui::Color32::from_rgb(240, 120, 120), t!("escape_transforms.unknown", name = t.variation));
                            }
                        }
                    });
                });
            }
            if let Some((l, what)) = edit {
                write_list(config_manager, &l, what);
                return;
            }
            if ui.button(t!("escape_transforms.add")).on_hover_text(t!("escape_transforms.add_tip")).clicked() {
                let mut l = list.clone();
                // The identity until a variation is picked.
                l.push(entry(TransformStage::Plane, "linear", 1.0));
                write_list(config_manager, &l, "history.action.escape_transform_add");
            }
            // What the stages cost: Plane and Iteration keep the direct path.
            if config_manager.active_config().escape.transforms_absolute() {
                ui.label(egui::RichText::new(t!("escape_transforms.direct_only")).small().weak());
            }
        });
}

/// A variation picker for entry `i`: a button showing the current one and
/// a popup with a filter and every variation with a 2D body -- a popup,
/// not a combo, so a click in the filter does not close it (the lens
/// picker's reason). The chosen name, when one is chosen.
fn variation_picker(ui: &mut egui::Ui, i: usize, current: &str) -> Option<String> {
    let registry = crate::variations::global_registry();
    let label = registry.get(current).map(|v| v.display_name.clone()).unwrap_or_else(|| current.to_string());
    let popup_id = egui::Id::new(("escape_transform_popup", i));
    let filter_id = popup_id.with("filter");
    let btn = ui.add(egui::Button::new(format!("{label}  \u{25be}")).min_size(egui::vec2(160.0, 0.0)));
    if btn.clicked() {
        egui::Popup::toggle_id(ui.ctx(), popup_id);
    }
    let mut pick = None;
    egui::Popup::from_response(&btn)
        .id(popup_id)
        .open_memory(None)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(240.0);
            let mut filter = ui.data_mut(|d| d.get_temp::<String>(filter_id).unwrap_or_default());
            let edit = ui.add(egui::TextEdit::singleline(&mut filter).hint_text(t!("escape_transforms.filter")).desired_width(220.0));
            super::vkb_sync(ui, &edit, &filter);
            if edit.changed() {
                ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
            }
            let needle = filter.trim().to_lowercase();
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for name in registry.names() {
                    let Some(info) = registry.get(name) else { continue };
                    if info.category == VariationCategory::Only3D {
                        continue;
                    }
                    if !needle.is_empty() && !name.to_lowercase().contains(&needle) && !info.display_name.to_lowercase().contains(&needle) {
                        continue;
                    }
                    if ui.selectable_label(name == current, &info.display_name).clicked() {
                        pick = Some(name.clone());
                        egui::Popup::close_id(ui.ctx(), popup_id);
                    }
                }
            });
        });
    pick
}
