//! Effects Panel UI
//!
//! UI for managing post-processing effects (color and density effects).

use egui::Ui;

use crate::config::{ConfigManager, ConfigPath, UpdateType};
use crate::effects::{global_effect_registry, EffectCategory};

/// Render the Effects panel
pub fn render_effects_panel(
    ui: &mut Ui,
    config_manager: &mut ConfigManager,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    ui.heading("Post-Processing Effects");
    ui.add_space(8.0);

    // Color Effects section
    let header = egui::CollapsingHeader::new("Color Effects")
        .default_open(true);

    header.show(ui, |ui| {
        ui.label(egui::RichText::new("Applied after tone mapping").small().color(egui::Color32::GRAY));
        ui.add_space(4.0);

        let color_effects = config_manager.active_config().color_effects.clone();

        if color_effects.is_empty() {
            ui.label(egui::RichText::new("No color effects added").italics().color(egui::Color32::GRAY));
            ui.add_space(4.0);

            // Show available effects
            ui.label("Available effects:");
            let registry = global_effect_registry();
            for info in registry.all() {
                if info.category == EffectCategory::Color {
                    ui.horizontal(|ui| {
                        ui.label(format!("• {}", info.display_name));
                    });
                }
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Add effects via config file for now").small().italics());
        } else {
            // Show existing effects
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
                                        param_def.min_value.unwrap_or(0.0)..=param_def.max_value.unwrap_or(1.0),
                                    ).text(&param_def.display_name);

                                    if ui.add(slider).changed() {
                                        if let Err(e) = config_manager.update_param(
                                            ConfigPath::ColorEffectParam {
                                                index: idx,
                                                param: param_def.name.clone()
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
        }
    });

    ui.add_space(16.0);

    // Density Effects section (placeholder for future)
    let density_header = egui::CollapsingHeader::new("Density Effects")
        .default_open(false);

    density_header.show(ui, |ui| {
        ui.label(egui::RichText::new("Applied before tone mapping (access to density data)").small().color(egui::Color32::GRAY));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("No density effects registered yet").italics().color(egui::Color32::GRAY));
    });

    max_update
}
