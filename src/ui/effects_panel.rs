//! Effects Panel UI
//!
//! UI for managing post-processing effects (color and density effects).

use egui::Ui;

use crate::config::{ConfigChange, ConfigManager, ConfigPath, UpdateType};
use crate::effects::{global_effect_registry, EffectCategory, EffectInstance};

/// Render the Effects panel
pub fn render_effects_panel(ui: &mut Ui, config_manager: &mut ConfigManager) -> UpdateType {
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

                    // Effect name
                    ui.label(egui::RichText::new(&effect.effect_type).strong());

                    // Spacer
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Remove button
                        if ui.small_button("✕").clicked() {
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

                                let slider = egui::Slider::new(
                                    &mut value,
                                    param_def.min_value.unwrap_or(0.0)
                                        ..=param_def.max_value.unwrap_or(1.0),
                                )
                                .text(info.translated_param_name(&param_def.name));

                                if ui.add(slider).changed() {
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
            max_update = max_update.max(UpdateType::ToneMappingOnly);
        }

        // Add effect button with dropdown
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Add effect:");

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
                // Show buttons for each available effect
                for info in available {
                    let translated = info.translated_name();
                    if ui.button(&translated).clicked() {
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
            }
        });
    });

    ui.add_space(16.0);

    // Density Effects section
    let density_header = egui::CollapsingHeader::new("Density Effects").default_open(false);

    density_header.show(ui, |ui| {
        ui.label(
            egui::RichText::new("Applied before tone mapping (access to density data)")
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

                        ui.label(egui::RichText::new(&effect.effect_type).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
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

                                    let slider = egui::Slider::new(
                                        &mut value,
                                        param_def.min_value.unwrap_or(0.0)
                                            ..=param_def.max_value.unwrap_or(1.0),
                                    )
                                    .text(info.translated_param_name(&param_def.name));

                                    if ui.add(slider).changed() {
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
                max_update = max_update.max(UpdateType::ToneMappingOnly);
            }

            // Add density effect buttons
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Add effect:");
                for info in density_effects_available {
                    let translated = info.translated_name();
                    if ui.button(&translated).clicked() {
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
        }
    });

    max_update
}
