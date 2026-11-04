//! Effects Panel UI
//!
//! UI for managing post-processing effects (color and density effects).

use egui::Ui;
use rust_i18n::t;

use crate::animation::AnimationController;
use crate::config::{ConfigChange, ConfigManager, ConfigPath, UpdateType};
use crate::effects::{global_effect_registry, EffectCategory, EffectInstance, EffectRegistry};

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

/// One effect instance: its list and place, which its parameters'
/// paths name.
#[derive(Clone, Copy)]
struct EffectTarget {
    list: EffectList,
    index: usize,
}

impl EffectTarget {
    /// The value of `param`, its default where unset; `None` once the
    /// instance is gone.
    fn stored(self, config_manager: &ConfigManager, param: &crate::params::ParamDef) -> Option<f32> {
        let config = config_manager.active_config();
        let effects = match self.list {
            EffectList::Color => &config.color_effects,
            EffectList::Density => &config.density_effects,
        };
        let effect = effects.get(self.index)?;
        Some(effect.params.get(&*param.name).copied().unwrap_or(param.slot_default()))
    }

    fn path(self, param_name: &str) -> ConfigPath {
        let (index, param) = (self.index, param_name.to_string());
        match self.list {
            EffectList::Color => ConfigPath::ColorEffectParam { index, param },
            EffectList::Density => ConfigPath::DensityEffectParam { index, param },
        }
    }
}

/// The user's plug-in effects of `category` in an Add list (P3), after a
/// separator: whether one was added. Adding one puts its source in the
/// config first, which carries what its chains run (as the Formula
/// Editor's Use does), then an instance at the end of the chain.
fn plugin_effects(ui: &mut Ui, config_manager: &mut ConfigManager, category: EffectCategory) -> bool {
    let registry = global_effect_registry();
    let plugins: Vec<(crate::storage::library::Plugin, String)> = crate::storage::library::plugins_of(crate::storage::library::Kind::Effect)
        .into_iter()
        .filter_map(|p| {
            let info = registry.get(&p.key)?;
            (info.category == category).then(|| (p.clone(), info.translated_name()))
        })
        .collect();
    drop(registry);
    if plugins.is_empty() {
        return false;
    }
    ui.separator();
    ui.weak(t!("effects_panel.plugins"));
    let mut added = false;
    for (plugin, title) in plugins {
        if !ui.selectable_label(false, &title).on_hover_text(t!("effects_panel.plugin_tip", file = plugin.file)).clicked() {
            continue;
        }
        let config = config_manager.active_config();
        if !config.custom_effects.iter().any(|s| s.entry == plugin.key) {
            let mut list = config.custom_effects.clone();
            list.push(plugin.carried.clone());
            let json = serde_json::to_string(&list).unwrap_or_default();
            if let Err(e) = config_manager.update_batch(vec![(ConfigPath::CustomEffects, crate::config::ConfigValue::String(json))], "history.action.add_plugin_effect".to_string()) {
                log::error!("Failed to carry the effect `{}`: {e}", plugin.key);
                continue;
            }
        }
        let effect = EffectInstance::new(&plugin.key);
        let description = format!("Add Effect: {title}");
        let change = match category {
            EffectCategory::Density => {
                let index = config_manager.active_config().density_effects.len();
                ConfigChange::add_density_effect_snapshot(index, effect, description)
            }
            EffectCategory::Color => {
                let index = config_manager.active_config().color_effects.len();
                ConfigChange::add_color_effect_snapshot(index, effect, description)
            }
        };
        if let Err(e) = config_manager.apply_structural_change(change) {
            log::error!("Failed to add effect: {}", e);
        }
        added = true;
    }
    added
}

/// Edit as Rhai, on a built-in that is its Rhai twin: whether it was
/// clicked. The Formula Editor makes it one of the user's
/// (`FormulaEditorPanel::edit_effect_as_rhai`).
fn edit_as_rhai_button(ui: &mut Ui, effect_type: &str) -> bool {
    let twin = crate::effects::custom::all_twins().any(|(n, _)| *n == effect_type);
    twin && ui.small_button(t!("effects_panel.edit_as_rhai")).on_hover_text(t!("effects_panel.edit_as_rhai_tip")).clicked()
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
        .map(|info| {
            info.parameters
                .iter()
                .map(|p| {
                    // In the current language, for a Rhai effect's labels.
                    let (label, hint) = info.param_label(p);
                    crate::params::ParamDef { label: label.into(), hint: hint.unwrap_or_default().into(), ..p.clone() }
                })
                .collect::<Vec<_>>()
        })
    else {
        return UpdateType::None;
    };
    let salt = match list {
        EffectList::Color => "effect_params",
        EffectList::Density => "density_effect_params",
    };
    let salt = format!("{salt}_{index}");
    ui.indent(&salt, |ui| {
        let target = EffectTarget { list, index };
        // A Rhai effect's conditions and grids.
        #[cfg(feature = "lang")]
        let layout = crate::effects::custom::param_layout(effect_type);
        #[cfg(not(feature = "lang"))]
        let layout: Option<std::sync::Arc<crate::params::layout::ParamLayout>> = None;
        let mut store = super::params::ConfigStore::new(config_manager, move |m, d| target.stored(m, d), move |n| target.path(n));
        super::params::list(ui, &mut store, &super::params::List::new(&params, &salt).layout(layout.as_deref()));
        store.update
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
/// The online library's section; gives the downloaded effects the user
/// asked to update to the catalog's version.
fn render_effect_catalog(
    ui: &mut Ui,
    catalog: Option<&crate::storage::effect_catalog::CachedEffectCatalog>,
) -> Vec<String> {
    use crate::storage::catalog::summarize;

    let mut update = Vec::new();
    let Some(catalog) = catalog else { return update };
    if catalog.items.is_empty() {
        return update;
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
                ui.horizontal(|ui| {
                    ui.weak(format!(
                        "{} — have v{have}, v{avail} available",
                        item.display_name
                    ));
                    // The path that installs also updates.
                    if ui.small_button(t!("effects_panel.update")).clicked() {
                        update.push(item.name.clone());
                    }
                });
            }
            if summary.updatable.len() > 1 && ui.button(t!("effects_panel.update_all", count = summary.updatable.len())).clicked() {
                update.extend(summary.updatable.iter().map(|(i, _, _)| i.name.clone()));
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
    update
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
    formula_editor: &mut Option<super::formula_editor::FormulaEditorPanel>,
    open_formula_editor: &mut bool,
    effect_update_requested: &mut Vec<String>,
    clear_effect_cache_requested: &mut bool,
) -> UpdateType {
    let mut max_update = UpdateType::None;
    // Edit as Rhai: (density, index, the built-in).
    let mut edit_as_rhai: Option<(bool, usize, String)> = None;

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
                        if edit_as_rhai_button(ui, &effect.effect_type) {
                            edit_as_rhai = Some((false, idx, effect.effect_type.clone()));
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
            // A Rhai effect is put to use from the Formula Editor: every
            // version of one is registered under its own key, and would
            // list once each.
            let available: Vec<_> = registry
                .all()
                .filter(|info| info.category == EffectCategory::Color && info.provenance != crate::provenance::Provenance::Custom)
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
                        if plugin_effects(ui, config_manager, EffectCategory::Color) {
                            max_update = max_update.max(UpdateType::ToneMappingOnly);
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
                .filter(|info| info.category == EffectCategory::Density && info.provenance != crate::provenance::Provenance::Custom)
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
                            if edit_as_rhai_button(ui, &effect.effect_type) {
                                edit_as_rhai = Some((true, idx, effect.effect_type.clone()));
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
                        if plugin_effects(ui, config_manager, EffectCategory::Density) {
                            max_update = max_update.max(UpdateType::ToneMappingOnly);
                        }
                    });
            });
        }
    });

    *effect_update_requested = render_effect_catalog(ui, catalog);
    // Downloads only: the built-ins and the user's own are not cache.
    let downloaded = global_effect_registry().all().filter(|i| i.provenance.is_cached_download()).count();
    if downloaded > 0 {
        ui.add_space(4.0);
        if ui.button(t!("effects_panel.clear_cache", count = downloaded)).on_hover_text(t!("effects_panel.clear_cache_tip")).clicked() {
            *clear_effect_cache_requested = true;
        }
    }

    if let Some((density, index, builtin)) = edit_as_rhai {
        formula_editor.get_or_insert_with(super::formula_editor::FormulaEditorPanel::new).edit_effect_as_rhai(config_manager, density, index, &builtin);
        *open_formula_editor = true;
        max_update = max_update.max(UpdateType::ToneMappingOnly);
    }

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

    /// Choice, integer, switch and angle parameters keep their kinds on
    /// the way to the renderer: every one used to be a float slider.
    #[test]
    fn effect_parameters_keep_their_types() {
        use crate::params::ParamKind as K;
        let registry = global_effect_registry();
        let vignette = registry.get("vignette").expect("vignette");
        let blend = vignette.get_param("blend_mode").expect("blend_mode");
        assert!(matches!(blend.options(), Some(o) if o.len() == 13));
        assert_eq!(blend.kind.bounds(), Some((0.0, 12.0)));
        let kaleidoscope = registry.get("kaleidoscope").expect("kaleidoscope");
        assert!(matches!(kaleidoscope.get_param("edge_offset").expect("edge_offset").kind, K::Int { .. }));
        // The shaders loop `octaves` times, truncating it as the slider does.
        for name in ["domain_warp", "simplex_noise"] {
            let octaves = registry.get(name).and_then(|e| e.get_param("octaves")).expect(name);
            assert!(matches!(octaves.kind, K::Int { .. }), "{name}");
        }
        let kinds: Vec<&K> = registry.all().flat_map(|info| info.parameters.iter()).map(|p| &p.kind).collect();
        assert!(kinds.iter().any(|k| matches!(k, K::Bool { .. })));
        assert!(kinds.iter().any(|k| matches!(k, K::Float { angle: true, .. })));
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
            color.path("radius"),
            ConfigPath::ColorEffectParam { index: 2, param: "radius".to_string() }
        );
        let density = EffectTarget { list: EffectList::Density, index: 0 };
        assert_eq!(
            density.path("strength"),
            ConfigPath::DensityEffectParam { index: 0, param: "strength".to_string() }
        );
    }
}
