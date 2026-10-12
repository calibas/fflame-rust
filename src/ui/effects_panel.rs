//! Effects Panel UI
//!
//! UI for managing post-processing effects (color and density effects).

use egui::Ui;

use crate::animation::AnimationController;
use crate::config::{ConfigChange, ConfigManager, ConfigPath, UpdateType};
use crate::effects::{global_effect_registry, EffectCategory, EffectInstance, EffectParameter, EffectRegistry};
use crate::variations::{ParamType, VariationParameter};

use super::variation_params::{render_variation_params, ParamTarget};

/// The name to show for an effect in the list and in undo entries: the
/// registry's label (curated and translated for a built-in), or the raw
/// key for an effect this build does not have. Rows used to show the
/// key itself -- `sobel_edges` for "Edge Glow".
fn effect_label(registry: &EffectRegistry, effect_type: &str) -> String {
    registry
        .get(effect_type)
        .map(|info| info.translated_name())
        .unwrap_or_else(|| effect_type.to_string())
}

/// Which list an effect sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectList {
    Color,
    Density,
}

/// One effect's parameters as a [`ParamTarget`], so they are drawn by
/// the variation parameter renderer: an Integer gets an integer slider
/// (Blend Mode 0-12), a Boolean a checkbox, an Angle degrees -- every
/// parameter used to be a float slider -- and a value is written only
/// when it actually changed, never merely by being drawn.
struct EffectTarget {
    list: EffectList,
    index: usize,
}

impl ParamTarget for EffectTarget {
    fn stored(
        &self,
        config_manager: &ConfigManager,
        _var_name: &str,
        param: &VariationParameter,
    ) -> Option<f32> {
        let config = config_manager.active_config();
        let effects = match self.list {
            EffectList::Color => &config.color_effects,
            EffectList::Density => &config.density_effects,
        };
        let effect = effects.get(self.index)?;
        Some(effect.params.get(&param.name).copied().unwrap_or(param.default_value))
    }

    fn path(&self, _var_name: &str, param_name: &str) -> ConfigPath {
        let (index, param) = (self.index, param_name.to_string());
        match self.list {
            EffectList::Color => ConfigPath::ColorEffectParam { index, param },
            EffectList::Density => ConfigPath::DensityEffectParam { index, param },
        }
    }
}

/// An effect parameter in the shape the parameter renderer draws. The
/// two structs carry the same fields. A Float the effect left unranged
/// keeps this panel's old 0..1 slider rather than the variations' -10..10.
fn widget_param(p: &EffectParameter) -> VariationParameter {
    let (min_value, max_value) = match p.param_type {
        ParamType::Float => (p.min_value.or(Some(0.0)), p.max_value.or(Some(1.0))),
        _ => (p.min_value, p.max_value),
    };
    VariationParameter {
        name: p.name.clone(),
        display_name: p.display_name.clone(),
        param_type: p.param_type.clone(),
        default_value: p.default_value,
        min_value,
        max_value,
        description: p.description.clone(),
    }
}

/// Draw the parameters of the effect at `index` in `list`.
fn render_effect_params(
    ui: &mut Ui,
    config_manager: &mut ConfigManager,
    list: EffectList,
    index: usize,
    effect_type: &str,
) -> UpdateType {
    // Copied out so the registry's lock is not held while the widgets
    // write through the config manager.
    let Some(params) = global_effect_registry()
        .get(effect_type)
        .map(|info| info.parameters.iter().map(widget_param).collect::<Vec<_>>())
    else {
        return UpdateType::None;
    };
    let salt = match list {
        EffectList::Color => "effect_params",
        EffectList::Density => "density_effect_params",
    };
    ui.indent(format!("{salt}_{index}"), |ui| {
        render_variation_params(ui, config_manager, &EffectTarget { list, index }, effect_type, &params)
    })
    .inner
}

/// Render reorder buttons (up/down) for an effect
/// Returns the action to take: None, MoveUp, or MoveDown
fn render_reorder_buttons(ui: &mut Ui, index: usize, total: usize) -> Option<ReorderAction> {
    let mut action = None;

    // Up button (disabled if first)
    let up_enabled = index > 0;
    if ui.add_enabled(up_enabled, egui::Button::new("^").small()).clicked() {
        action = Some(ReorderAction::MoveUp);
    }

    // Down button (disabled if last)
    let down_enabled = index < total.saturating_sub(1);
    if ui.add_enabled(down_enabled, egui::Button::new("v").small()).clicked() {
        action = Some(ReorderAction::MoveDown);
    }

    action
}

#[derive(Debug, Clone, Copy)]
enum ReorderAction {
    MoveUp,
    MoveDown,
}

/// Render the Effects panel
/// What the online library holds versus what is installed.
///
/// Mirrors the Variations panel's section, sharing its state machine
/// through `storage::catalog`. Silent when there is no catalog: an app
/// that renders perfectly well offline should not grow an error panel
/// because a metadata endpoint was unreachable.
fn render_effect_catalog(
    ui: &mut Ui,
    catalog: Option<&crate::storage::effect_catalog::CachedEffectCatalog>,
) {
    use crate::storage::catalog::summarize;

    let Some(catalog) = catalog else { return };
    if catalog.items.is_empty() {
        return;
    }

    let registry = global_effect_registry();
    let summary = summarize(&catalog.items, |name| {
        registry.get(name).map(|e| e.provenance.clone())
    });
    let installed = catalog
        .items
        .iter()
        .filter(|i| registry.contains(&i.name))
        .count();
    drop(registry);

    ui.add_space(8.0);
    egui::CollapsingHeader::new(format!(
        "Online library — {} catalogued, {} here",
        catalog.items.len(),
        installed
    ))
    .default_open(false)
    .show(ui, |ui| {
        if !summary.updatable.is_empty() {
            ui.label(
                egui::RichText::new(format!(
                    "{} downloaded effect(s) have a newer version",
                    summary.updatable.len()
                ))
                .strong(),
            );
            for (item, have, avail) in &summary.updatable {
                ui.weak(format!(
                    "{} — have v{have}, v{avail} available",
                    item.display_name
                ));
            }
            ui.add_space(4.0);
        }

        if !summary.available.is_empty() {
            // Fetched on demand when a flame uses one, so this is a
            // listing rather than a set of buttons — a download-all
            // control would pull shader code nobody has a use for.
            ui.weak("Fetched automatically when a flame uses one:");
            for item in summary.available.iter().take(40) {
                ui.weak(&item.display_name);
                if let Some(d) = item.description_plain.as_ref().filter(|d| !d.is_empty()) {
                    ui.indent(("eff_cat_desc", &item.name), |ui| {
                        ui.weak(d);
                    });
                }
            }
            if summary.available.len() > 40 {
                ui.weak(format!("… and {} more", summary.available.len() - 40));
            }
        }

        if summary.builtin_only_elsewhere > 0 {
            ui.add_space(4.0);
            ui.weak(format!(
                "{} not available to download yet",
                summary.builtin_only_elsewhere
            ))
            .on_hover_text(
                "The server lists these but is not serving their shader code yet, so there \
                 is nothing to fetch. They are shown so the catalog is not silently short.",
            );
        } else if summary.available.is_empty() && summary.updatable.is_empty() {
            ui.weak("Everything in the catalog is already here.");
        }
    });
}

pub fn render_effects_panel(
    ui: &mut Ui,
    config_manager: &mut ConfigManager,
    // Kept for API stability with the panel viewer; the old
    // on_*_removed / on_*_reordered animation-track patching that
    // used this is now handled by the structural_changed rebind hook
    // in `app::gpu_updates`.
    _animation_controller: &mut AnimationController,
    catalog: Option<&crate::storage::effect_catalog::CachedEffectCatalog>,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    ui.heading("Post-Processing Effects");
    ui.add_space(8.0);

    // Color Effects section
    let header = egui::CollapsingHeader::new("Color Effects").default_open(true);

    header.show(ui, |ui| {
        ui.label(
            egui::RichText::new("Applied after tone mapping")
                .small()
                .color(egui::Color32::GRAY),
        );
        ui.add_space(4.0);

        let color_effects = config_manager.active_config().color_effects.clone();
        let labels: Vec<String> = {
            let registry = global_effect_registry();
            color_effects.iter().map(|e| effect_label(&registry, &e.effect_type)).collect()
        };

        // Show existing effects
        let mut effect_to_remove: Option<usize> = None;
        let mut effect_to_move: Option<(usize, ReorderAction)> = None;
        let num_color_effects = color_effects.len();

        for (idx, effect) in color_effects.iter().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Enable/disable checkbox
                    let mut enabled = effect.enabled;
                    if ui.checkbox(&mut enabled, "").changed() {
                        if let Err(e) = config_manager.update_param(
                            ConfigPath::ColorEffectEnabled { index: idx },
                            enabled.into(),
                        ) {
                            log::error!("Failed to update effect enabled: {}", e);
                        }
                        max_update = max_update.max(UpdateType::ToneMappingOnly);
                    }

                    // Reorder buttons
                    if let Some(action) = render_reorder_buttons(ui, idx, num_color_effects) {
                        effect_to_move = Some((idx, action));
                    }

                    // Effect name
                    ui.label(egui::RichText::new(&labels[idx]).strong());

                    // Spacer
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Remove button
                        if ui.small_button("X").clicked() {
                            effect_to_remove = Some(idx);
                        }
                    });
                });

                // Effect parameters (only show if enabled)
                if effect.enabled {
                    let update =
                        render_effect_params(ui, config_manager, EffectList::Color, idx, &effect.effect_type);
                    max_update = max_update.max(update);
                }
            });
            ui.add_space(4.0);
        }

        // Handle removal after iteration
        if let Some(idx) = effect_to_remove {
            let effect = color_effects[idx].clone();
            let change = ConfigChange::delete_color_effect_snapshot(
                idx,
                effect,
                format!("Remove Effect: {}", labels[idx]),
            );
            if let Err(e) = config_manager.apply_structural_change(change) {
                log::error!("Failed to remove effect: {}", e);
            }
            // Animation tracks rebound automatically via the
            // structural_changed hook in gpu_updates.
            max_update = max_update.max(UpdateType::ToneMappingOnly);
        }

        // Handle reordering after iteration
        if let Some((idx, action)) = effect_to_move {
            let (from_idx, to_idx) = match action {
                ReorderAction::MoveUp => (idx, idx.saturating_sub(1)),
                ReorderAction::MoveDown => (idx, (idx + 1).min(num_color_effects.saturating_sub(1))),
            };
            if from_idx != to_idx {
                let effect_name = labels[from_idx].clone();
                let change = ConfigChange::move_color_effect_snapshot(
                    from_idx,
                    to_idx,
                    format!("Move Effect: {}", effect_name),
                );
                if let Err(e) = config_manager.apply_structural_change(change) {
                    log::error!("Failed to move effect: {}", e);
                }
                // Tracks rebound automatically via structural_changed hook.
                max_update = max_update.max(UpdateType::ToneMappingOnly);
            }
        }

        // Add effect dropdown
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            // Get available color effects
            let registry = global_effect_registry();
            let available: Vec<_> = registry
                .all()
                .filter(|info| info.category == EffectCategory::Color)
                .collect();

            if available.is_empty() {
                ui.label(
                    egui::RichText::new("No effects available")
                        .italics()
                        .color(egui::Color32::GRAY),
                );
            } else {
                // Dropdown for adding effects
                egui::ComboBox::from_label("Add effect")
                    .selected_text("Select...")
                    .show_ui(ui, |ui| {
                        for info in available {
                            let translated = info.translated_name();
                            if ui.selectable_label(false, &translated).clicked() {
                                let effect = EffectInstance::new(&info.name);
                                let index = config_manager.active_config().color_effects.len();
                                let change = ConfigChange::add_color_effect_snapshot(
                                    index,
                                    effect,
                                    format!("Add Effect: {}", translated),
                                );
                                if let Err(e) = config_manager.apply_structural_change(change) {
                                    log::error!("Failed to add effect: {}", e);
                                }
                                max_update = max_update.max(UpdateType::ToneMappingOnly);
                            }
                        }
                    });
            }
        });
    });

    ui.add_space(16.0);

    // Density Effects section
    let density_header = egui::CollapsingHeader::new("Density Effects").default_open(false);

    density_header.show(ui, |ui| {
        ui.label(
            egui::RichText::new("Applied before tone mapping (access to density data). ")
                .small()
                .color(egui::Color32::GRAY),
        );
        ui.label(
            egui::RichText::new(
                "Note: skipped on very large exports (above ~83 MP, e.g. larger than \
                 ~9,100×9,100) to stay within GPU memory.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
        ui.add_space(4.0);

        // Check if any density effects are registered. Names only, so the
        // registry's lock is released before the widgets below write
        // through the config manager.
        let density_effects = config_manager.active_config().density_effects.clone();
        let (density_effects_available, labels): (Vec<(String, String)>, Vec<String>) = {
            let registry = global_effect_registry();
            let available = registry
                .all()
                .filter(|info| info.category == EffectCategory::Density)
                .map(|info| (info.name.clone(), info.translated_name()))
                .collect();
            let labels = density_effects.iter().map(|e| effect_label(&registry, &e.effect_type)).collect();
            (available, labels)
        };

        if density_effects_available.is_empty() {
            ui.label(
                egui::RichText::new("No density effects registered yet")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        } else {
            // Show existing density effects
            let mut effect_to_remove: Option<usize> = None;
            let mut effect_to_move: Option<(usize, ReorderAction)> = None;
            let num_density_effects = density_effects.len();

            for (idx, effect) in density_effects.iter().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let mut enabled = effect.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            if let Err(e) = config_manager.update_param(
                                ConfigPath::DensityEffectEnabled { index: idx },
                                enabled.into(),
                            ) {
                                log::error!("Failed to update effect enabled: {}", e);
                            }
                            max_update = max_update.max(UpdateType::ToneMappingOnly);
                        }

                        // Reorder buttons
                        if let Some(action) = render_reorder_buttons(ui, idx, num_density_effects) {
                            effect_to_move = Some((idx, action));
                        }

                        ui.label(egui::RichText::new(&labels[idx]).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").clicked() {
                                effect_to_remove = Some(idx);
                            }
                        });
                    });

                    if effect.enabled {
                        let update = render_effect_params(
                            ui,
                            config_manager,
                            EffectList::Density,
                            idx,
                            &effect.effect_type,
                        );
                        max_update = max_update.max(update);
                    }
                });
                ui.add_space(4.0);
            }

            if let Some(idx) = effect_to_remove {
                let effect = density_effects[idx].clone();
                let change = ConfigChange::delete_density_effect_snapshot(
                    idx,
                    effect,
                    format!("Remove Effect: {}", labels[idx]),
                );
                if let Err(e) = config_manager.apply_structural_change(change) {
                    log::error!("Failed to remove effect: {}", e);
                }
                // Tracks rebound automatically via structural_changed hook.
                max_update = max_update.max(UpdateType::ToneMappingOnly);
            }

            // Handle reordering after iteration
            if let Some((idx, action)) = effect_to_move {
                let (from_idx, to_idx) = match action {
                    ReorderAction::MoveUp => (idx, idx.saturating_sub(1)),
                    ReorderAction::MoveDown => (idx, (idx + 1).min(num_density_effects.saturating_sub(1))),
                };
                if from_idx != to_idx {
                    let effect_name = labels[from_idx].clone();
                    let change = ConfigChange::move_density_effect_snapshot(
                        from_idx,
                        to_idx,
                        format!("Move Effect: {}", effect_name),
                    );
                    if let Err(e) = config_manager.apply_structural_change(change) {
                        log::error!("Failed to move effect: {}", e);
                    }
                    // Tracks rebound automatically via structural_changed hook.
                    max_update = max_update.max(UpdateType::ToneMappingOnly);
                }
            }

            // Add density effect dropdown
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Add effect")
                    .selected_text("Select...")
                    .show_ui(ui, |ui| {
                        for (name, translated) in &density_effects_available {
                            if ui.selectable_label(false, translated).clicked() {
                                let effect = EffectInstance::new(name);
                                let index = config_manager.active_config().density_effects.len();
                                let change = ConfigChange::add_density_effect_snapshot(
                                    index,
                                    effect,
                                    format!("Add Effect: {}", translated),
                                );
                                if let Err(e) = config_manager.apply_structural_change(change) {
                                    log::error!("Failed to add effect: {}", e);
                                }
                                max_update = max_update.max(UpdateType::ToneMappingOnly);
                            }
                        }
                    });
            });
        }
    });

    render_effect_catalog(ui, catalog);

    max_update
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FractalConfig;

    #[test]
    fn rows_show_the_effects_name_not_its_key() {
        let registry = global_effect_registry();
        let label = effect_label(&registry, "sobel_edges");
        assert_ne!(label, "sobel_edges");
        assert_eq!(label, registry.get("sobel_edges").unwrap().translated_name());
        // An effect this build lacks still has a row.
        assert_eq!(effect_label(&registry, "no_such_effect"), "no_such_effect");
    }

    /// Integer, Boolean and Angle parameters keep their type on the way
    /// to the widget renderer: every one used to be a float slider.
    #[test]
    fn effect_parameters_keep_their_types() {
        let registry = global_effect_registry();
        let vignette = registry.get("vignette").expect("vignette");
        let blend = vignette.get_param("blend_mode").expect("blend_mode");
        let w = widget_param(blend);
        assert!(matches!(w.param_type, ParamType::Integer));
        assert_eq!((w.min_value, w.max_value), (Some(0.0), Some(12.0)));

        let typed: Vec<_> = registry
            .all()
            .flat_map(|info| info.parameters.iter())
            .map(|p| widget_param(p).param_type)
            .collect();
        assert!(typed.iter().any(|t| matches!(t, ParamType::Boolean)));
        assert!(typed.iter().any(|t| matches!(t, ParamType::Angle)));
    }

    /// Drawing an effect's parameters must not edit it: values off a
    /// slider's grid, or an integer, stay as stored, with no undo entry.
    #[test]
    fn drawing_effect_parameters_keeps_their_values() {
        let mut effect = EffectInstance::new("vignette");
        effect.params.insert("intensity".to_string(), 0.006);
        effect.params.insert("blend_mode".to_string(), 3.0);
        let mut config = FractalConfig::default();
        config.color_effects = vec![effect.clone()];
        config.density_effects = vec![];
        let mut manager = ConfigManager::new(config);

        for _ in 0..3 {
            egui::__run_test_ui(|ui| {
                render_effect_params(ui, &mut manager, EffectList::Color, 0, "vignette");
            });
        }
        assert_eq!(manager.active_config().color_effects[0].params, effect.params);
        assert!(!manager.can_undo());
    }

    #[test]
    fn an_effect_target_writes_its_own_list() {
        let color = EffectTarget { list: EffectList::Color, index: 2 };
        assert_eq!(
            color.path("x", "radius"),
            ConfigPath::ColorEffectParam { index: 2, param: "radius".to_string() }
        );
        let density = EffectTarget { list: EffectList::Density, index: 0 };
        assert_eq!(
            density.path("x", "strength"),
            ConfigPath::DensityEffectParam { index: 0, param: "strength".to_string() }
        );
    }
}
