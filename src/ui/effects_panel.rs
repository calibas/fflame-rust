//! Effects Panel UI
//!
//! UI for managing post-processing effects (color and density effects).

use egui::Ui;

use crate::animation::AnimationController;
use crate::config::{ConfigChange, ConfigManager, ConfigPath, UpdateType};
use crate::effects::{global_effect_registry, EffectCategory, EffectInstance};

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
        registry.get(name).map(|e| {
            (
                e.provenance.is_builtin(),
                e.provenance.version().unwrap_or(0),
            )
        })
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
                    ui.label(egui::RichText::new(&effect.effect_type).strong());

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
                    let registry = global_effect_registry();
                    if let Some(info) = registry.get(&effect.effect_type) {
                        ui.indent(format!("effect_params_{}", idx), |ui| {
                            for param_def in &info.parameters {
                                let current_value = effect.get_param(&param_def.name);
                                let mut value = current_value;

                                let slider = super::VkbSlider::new(
                                    &mut value,
                                    param_def.min_value.unwrap_or(0.0)
                                        ..=param_def.max_value.unwrap_or(1.0),
                                )
                                .text(&param_def.display_name);

                                let mut response = ui.add(slider);
                                if let Some(desc) = &param_def.description {
                                    response = response.on_hover_text(desc);
                                }
                                if response.changed() {
                                    if let Err(e) = config_manager.update_param(
                                        ConfigPath::ColorEffectParam {
                                            index: idx,
                                            param: param_def.name.clone(),
                                        },
                                        value.into(),
                                    ) {
                                        log::error!("Failed to update effect param: {}", e);
                                    }
                                    max_update = max_update.max(UpdateType::ToneMappingOnly);
                                }
                            }
                        });
                    }
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
                format!("Remove Effect: {}", color_effects[idx].effect_type),
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
                let effect_name = color_effects[from_idx].effect_type.clone();
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
                "Note: skipped on very large exports (above ~119 MP, e.g. larger than \
                 ~10,900×10,900) to stay within GPU memory.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
        ui.add_space(4.0);

        // Check if any density effects are registered
        let registry = global_effect_registry();
        let density_effects_available: Vec<_> = registry
            .all()
            .filter(|info| info.category == EffectCategory::Density)
            .collect();

        if density_effects_available.is_empty() {
            ui.label(
                egui::RichText::new("No density effects registered yet")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        } else {
            let density_effects = config_manager.active_config().density_effects.clone();

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

                        ui.label(egui::RichText::new(&effect.effect_type).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").clicked() {
                                effect_to_remove = Some(idx);
                            }
                        });
                    });

                    if effect.enabled {
                        if let Some(info) = registry.get(&effect.effect_type) {
                            ui.indent(format!("density_effect_params_{}", idx), |ui| {
                                for param_def in &info.parameters {
                                    let current_value = effect.get_param(&param_def.name);
                                    let mut value = current_value;

                                    let slider = super::VkbSlider::new(
                                        &mut value,
                                        param_def.min_value.unwrap_or(0.0)
                                            ..=param_def.max_value.unwrap_or(1.0),
                                    )
                                    .text(&param_def.display_name);

                                    let mut response = ui.add(slider);
                                    if let Some(desc) = &param_def.description {
                                        response = response.on_hover_text(desc);
                                    }
                                    if response.changed() {
                                        if let Err(e) = config_manager.update_param(
                                            ConfigPath::DensityEffectParam {
                                                index: idx,
                                                param: param_def.name.clone(),
                                            },
                                            value.into(),
                                        ) {
                                            log::error!("Failed to update effect param: {}", e);
                                        }
                                        max_update = max_update.max(UpdateType::ToneMappingOnly);
                                    }
                                }
                            });
                        }
                    }
                });
                ui.add_space(4.0);
            }

            if let Some(idx) = effect_to_remove {
                let effect = density_effects[idx].clone();
                let change = ConfigChange::delete_density_effect_snapshot(
                    idx,
                    effect,
                    format!("Remove Effect: {}", density_effects[idx].effect_type),
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
                    let effect_name = density_effects[from_idx].effect_type.clone();
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
                        for info in density_effects_available {
                            let translated = info.translated_name();
                            if ui.selectable_label(false, &translated).clicked() {
                                let effect = EffectInstance::new(&info.name);
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
