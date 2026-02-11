use crate::scene::transforms::{Flame, RenderMode};
use crate::variations::{VariationCategory, global_registry};
use crate::config::{ConfigManager, ConfigPath, UpdateType, AffineParam};
use super::variation_params::render_variation_params;
use egui::Color32;
use rust_i18n::t;

/// Get a distinct color for each transform index (matches Triangle Editor)
fn get_transform_color(index: usize) -> Color32 {
    let colors = [
        Color32::from_rgb(255, 100, 100), // Red
        Color32::from_rgb(100, 255, 100), // Green
        Color32::from_rgb(100, 100, 255), // Blue
        Color32::from_rgb(255, 255, 100), // Yellow
        Color32::from_rgb(255, 100, 255), // Magenta
        Color32::from_rgb(100, 255, 255), // Cyan
        Color32::from_rgb(255, 150, 100), // Orange
        Color32::from_rgb(150, 100, 255), // Purple
    ];
    colors[index % colors.len()]
}

/// Render weight control (always visible)
fn render_weight_control(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    let mut temp_weight = transform.weight;
    let response = ui.add(egui::Slider::new(&mut temp_weight, 0.0..=1024.0)
        .logarithmic(true)
        .text(t!("transform.weight")))
      .on_hover_text(t!("tooltips.transform_weight"));
    if response.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformWeight { index },
            temp_weight.into()
        ) {
            transform.weight = config_manager.active_config().flame.transforms[index].weight;
            max_update = max_update.max(update_type);
        }
    }
    if response.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformWeight { index });
    }

    max_update
}

/// Render color controls (palette position + preview)
fn render_color_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    ui.horizontal(|ui| {
        // Palette position slider (0.0 to 1.0)
        let mut temp_color = transform.color;
        let response_color = ui.add(
            egui::Slider::new(&mut temp_color, 0.0..=1.0)
                .text(t!("transform.color"))
                
        ).on_hover_text(t!("tooltips.transform_color"));
        if response_color.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformColor { index },
                temp_color.into()
            ) {
                transform.color = config_manager.active_config().flame.transforms[index].color;
                max_update = max_update.max(update_type);
            }
        }
        if response_color.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index });
        }

        // Show color preview at current palette position
        let palette = &config_manager.active_config().palette;
        let actual_color = palette.sample_color(transform.color);
        let color_swatch = egui::Color32::from_rgb(
            (actual_color[0] * 255.0) as u8,
            (actual_color[1] * 255.0) as u8,
            (actual_color[2] * 255.0) as u8,
        );
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(20.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color_swatch);
    });

    max_update
}

/// Render affine matrix controls (in Advanced section)
fn render_affine_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
    render_mode: RenderMode,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Row 1: a, b
    ui.horizontal(|ui| {
        ui.label(t!("transform.affine_a")).on_hover_text(t!("tooltips.affine_a"));
        let mut temp_a = transform.a;
        let response_a = ui.add(egui::DragValue::new(&mut temp_a).speed(0.01)).on_hover_text(t!("tooltips.affine_a"));
        if response_a.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformAffine { index, param: AffineParam::A },
                temp_a.into()
            ) {
                transform.a = config_manager.active_config().flame.transforms[index].a;
                max_update = max_update.max(update_type);
            }
        }
        if response_a.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformAffine { index, param: AffineParam::A });
        }

        ui.label(t!("transform.affine_b")).on_hover_text(t!("tooltips.affine_b"));
        let mut temp_b = transform.b;
        let response_b = ui.add(egui::DragValue::new(&mut temp_b).speed(0.01)).on_hover_text(t!("tooltips.affine_b"));
        if response_b.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformAffine { index, param: AffineParam::B },
                temp_b.into()
            ) {
                transform.b = config_manager.active_config().flame.transforms[index].b;
                max_update = max_update.max(update_type);
            }
        }
        if response_b.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformAffine { index, param: AffineParam::B });
        }
    });

    // Row 2: c, d
    ui.horizontal(|ui| {
        ui.label(t!("transform.affine_c")).on_hover_text(t!("tooltips.affine_c"));
        let mut temp_c = transform.c;
        let response_c = ui.add(egui::DragValue::new(&mut temp_c).speed(0.01)).on_hover_text(t!("tooltips.affine_c"));
        if response_c.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformAffine { index, param: AffineParam::C },
                temp_c.into()
            ) {
                transform.c = config_manager.active_config().flame.transforms[index].c;
                max_update = max_update.max(update_type);
            }
        }
        if response_c.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformAffine { index, param: AffineParam::C });
        }

        ui.label(t!("transform.affine_d")).on_hover_text(t!("tooltips.affine_d"));
        let mut temp_d = transform.d;
        let response_d = ui.add(egui::DragValue::new(&mut temp_d).speed(0.01)).on_hover_text(t!("tooltips.affine_d"));
        if response_d.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformAffine { index, param: AffineParam::D },
                temp_d.into()
            ) {
                transform.d = config_manager.active_config().flame.transforms[index].d;
                max_update = max_update.max(update_type);
            }
        }
        if response_d.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformAffine { index, param: AffineParam::D });
        }
    });

    // Row 3: e, f
    ui.horizontal(|ui| {
        ui.label(t!("transform.affine_e")).on_hover_text(t!("tooltips.affine_e"));
        let mut temp_e = transform.e;
        let response_e = ui.add(egui::DragValue::new(&mut temp_e).speed(0.01)).on_hover_text(t!("tooltips.affine_e"));
        if response_e.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformAffine { index, param: AffineParam::E },
                temp_e.into()
            ) {
                transform.e = config_manager.active_config().flame.transforms[index].e;
                max_update = max_update.max(update_type);
            }
        }
        if response_e.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformAffine { index, param: AffineParam::E });
        }

        ui.label(t!("transform.affine_f")).on_hover_text(t!("tooltips.affine_f"));
        let mut temp_f = transform.f;
        let response_f = ui.add(egui::DragValue::new(&mut temp_f).speed(0.01)).on_hover_text(t!("tooltips.affine_f"));
        if response_f.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformAffine { index, param: AffineParam::F },
                temp_f.into()
            ) {
                transform.f = config_manager.active_config().flame.transforms[index].f;
                max_update = max_update.max(update_type);
            }
        }
        if response_f.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformAffine { index, param: AffineParam::F });
        }
    });

    // Z offset (only in 3D mode)
    if matches!(render_mode, RenderMode::ThreeD) {
        ui.horizontal(|ui| {
            ui.label(t!("transform.affine_g")).on_hover_text(t!("tooltips.affine_g"));
            let mut temp_g = transform.g;
            let response_g = ui.add(egui::DragValue::new(&mut temp_g).speed(0.01)).on_hover_text(t!("tooltips.affine_g"));
            if response_g.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformAffine { index, param: AffineParam::G },
                    temp_g.into()
                ) {
                    transform.g = config_manager.active_config().flame.transforms[index].g;
                    max_update = max_update.max(update_type);
                }
            }
            if response_g.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformAffine { index, param: AffineParam::G });
            }
        });
    }

    max_update
}

/// Render post-affine controls: enable checkbox + matrix (in Advanced section)
fn render_post_affine_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
    render_mode: RenderMode,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Enable checkbox
    let mut temp_enabled = transform.post_affine_enabled;
    if ui.checkbox(&mut temp_enabled, t!("transform.post_affine_enabled"))
        .on_hover_text(t!("tooltips.post_affine_enabled"))
        .changed()
    {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformPostAffineEnabled { index },
            temp_enabled.into()
        ) {
            transform.post_affine_enabled = config_manager.active_config().flame.transforms[index].post_affine_enabled;
            max_update = max_update.max(update_type);
        }
    }

    // Show matrix controls only when enabled
    if transform.post_affine_enabled {
        ui.label(t!("transform.post_affine_matrix"));

        // Row 1: a, b
        ui.horizontal(|ui| {
            ui.label(t!("transform.affine_a")).on_hover_text(t!("tooltips.affine_a"));
            let mut temp_a = transform.post_a;
            let response_a = ui.add(egui::DragValue::new(&mut temp_a).speed(0.01)).on_hover_text(t!("tooltips.affine_a"));
            if response_a.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformPostAffine { index, param: AffineParam::A },
                    temp_a.into()
                ) {
                    transform.post_a = config_manager.active_config().flame.transforms[index].post_a;
                    max_update = max_update.max(update_type);
                }
            }
            if response_a.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformPostAffine { index, param: AffineParam::A });
            }

            ui.label(t!("transform.affine_b")).on_hover_text(t!("tooltips.affine_b"));
            let mut temp_b = transform.post_b;
            let response_b = ui.add(egui::DragValue::new(&mut temp_b).speed(0.01)).on_hover_text(t!("tooltips.affine_b"));
            if response_b.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformPostAffine { index, param: AffineParam::B },
                    temp_b.into()
                ) {
                    transform.post_b = config_manager.active_config().flame.transforms[index].post_b;
                    max_update = max_update.max(update_type);
                }
            }
            if response_b.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformPostAffine { index, param: AffineParam::B });
            }
        });

        // Row 2: c, d
        ui.horizontal(|ui| {
            ui.label(t!("transform.affine_c")).on_hover_text(t!("tooltips.affine_c"));
            let mut temp_c = transform.post_c;
            let response_c = ui.add(egui::DragValue::new(&mut temp_c).speed(0.01)).on_hover_text(t!("tooltips.affine_c"));
            if response_c.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformPostAffine { index, param: AffineParam::C },
                    temp_c.into()
                ) {
                    transform.post_c = config_manager.active_config().flame.transforms[index].post_c;
                    max_update = max_update.max(update_type);
                }
            }
            if response_c.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformPostAffine { index, param: AffineParam::C });
            }

            ui.label(t!("transform.affine_d")).on_hover_text(t!("tooltips.affine_d"));
            let mut temp_d = transform.post_d;
            let response_d = ui.add(egui::DragValue::new(&mut temp_d).speed(0.01)).on_hover_text(t!("tooltips.affine_d"));
            if response_d.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformPostAffine { index, param: AffineParam::D },
                    temp_d.into()
                ) {
                    transform.post_d = config_manager.active_config().flame.transforms[index].post_d;
                    max_update = max_update.max(update_type);
                }
            }
            if response_d.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformPostAffine { index, param: AffineParam::D });
            }
        });

        // Row 3: e, f
        ui.horizontal(|ui| {
            ui.label(t!("transform.affine_e")).on_hover_text(t!("tooltips.affine_e"));
            let mut temp_e = transform.post_e;
            let response_e = ui.add(egui::DragValue::new(&mut temp_e).speed(0.01)).on_hover_text(t!("tooltips.affine_e"));
            if response_e.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformPostAffine { index, param: AffineParam::E },
                    temp_e.into()
                ) {
                    transform.post_e = config_manager.active_config().flame.transforms[index].post_e;
                    max_update = max_update.max(update_type);
                }
            }
            if response_e.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformPostAffine { index, param: AffineParam::E });
            }

            ui.label(t!("transform.affine_f")).on_hover_text(t!("tooltips.affine_f"));
            let mut temp_f = transform.post_f;
            let response_f = ui.add(egui::DragValue::new(&mut temp_f).speed(0.01)).on_hover_text(t!("tooltips.affine_f"));
            if response_f.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformPostAffine { index, param: AffineParam::F },
                    temp_f.into()
                ) {
                    transform.post_f = config_manager.active_config().flame.transforms[index].post_f;
                    max_update = max_update.max(update_type);
                }
            }
            if response_f.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformPostAffine { index, param: AffineParam::F });
            }
        });

        // Z offset (only in 3D mode)
        if matches!(render_mode, RenderMode::ThreeD) {
            ui.horizontal(|ui| {
                ui.label(t!("transform.affine_g")).on_hover_text(t!("tooltips.affine_g"));
                let mut temp_g = transform.post_g;
                let response_g = ui.add(egui::DragValue::new(&mut temp_g).speed(0.01)).on_hover_text(t!("tooltips.affine_g"));
                if response_g.changed() {
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::TransformPostAffine { index, param: AffineParam::G },
                        temp_g.into()
                    ) {
                        transform.post_g = config_manager.active_config().flame.transforms[index].post_g;
                        max_update = max_update.max(update_type);
                    }
                }
                if response_g.drag_stopped() {
                    let _ = config_manager.force_commit_preview(&ConfigPath::TransformPostAffine { index, param: AffineParam::G });
                }
            });
        }
    }

    max_update
}

/// Render advanced settings (color speed, opacity, solo toggle)
fn render_advanced_settings(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
    solo_transform: Option<usize>,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Solo toggle - only this transform gets weight, all others = 0
    let is_solo = solo_transform == Some(index);
    let mut solo_checked = is_solo;

    if ui.checkbox(&mut solo_checked, t!("transform.solo"))
        .on_hover_text(t!("tooltips.transform_solo"))
        .changed()
    {
        // -1 = None (no solo), 0+ = Some(index)
        let new_value = if solo_checked { index as i32 } else { -1 };
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::SoloTransform,
            new_value.into()
        ) {
            max_update = max_update.max(update_type);
        }
    }

    // Show "(muted)" indicator if another transform is solo'd
    if solo_transform.is_some() && !is_solo {
        ui.label(egui::RichText::new("(muted)").weak().italics());
    }

    ui.add_space(4.0);

    // Color Speed (Symmetry)
    let mut temp_speed = transform.color_speed;
    let response_speed = ui.add(egui::Slider::new(&mut temp_speed, -1.0..=1.0).text(t!("transform.color_speed")))
        .on_hover_text(t!("tooltips.color_speed"));
    if response_speed.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformColorSpeed { index },
            temp_speed.into()
        ) {
            transform.color_speed = config_manager.active_config().flame.transforms[index].color_speed;
            max_update = max_update.max(update_type);
        }
    }
    if response_speed.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformColorSpeed { index });
    }

    // Opacity slider
    let mut temp_opacity = transform.opacity;
    let response_opacity = ui.add(egui::Slider::new(&mut temp_opacity, 0.0..=1.0).text(t!("transform.opacity")))
        .on_hover_text(t!("tooltips.opacity"));
    if response_opacity.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformOpacity { index },
            temp_opacity.into()
        ) {
            transform.opacity = config_manager.active_config().flame.transforms[index].opacity;
            max_update = max_update.max(update_type);
        }
    }
    if response_opacity.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformOpacity { index });
    }

    max_update
}

/// Render a single enabled variation with weight slider and delete button
fn render_enabled_variation(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    transform_index: usize,
    variation_name: &str,
    current_weight: f32,
) -> (UpdateType, bool) {
    let mut max_update = UpdateType::None;
    let mut delete_requested = false;

    let registry = global_registry();
    let var_info = registry.get(variation_name);
    let display_name = var_info
        .map(|v| v.display_name.as_str())
        .unwrap_or(variation_name);

    ui.horizontal(|ui| {
        // Weight slider
        let mut value = current_weight;
        let response = ui.add(
            egui::Slider::new(&mut value, -5.0..=5.0)
                .text(display_name)
                .drag_value_speed(0.1)
                .clamping(egui::SliderClamping::Never)
        );

        if response.changed() {
            value = value.clamp(f32::MIN, f32::MAX);
            let path = ConfigPath::TransformVariation {
                index: transform_index,
                variation: variation_name.to_string(),
            };
            if let Ok(update_type) = config_manager.update_param(path, value.into()) {
                max_update = max_update.max(update_type);
            }
        }

        if response.drag_stopped() {
            let path = ConfigPath::TransformVariation {
                index: transform_index,
                variation: variation_name.to_string(),
            };
            let _ = config_manager.force_commit_preview(&path);
        }
        
        // Delete button
        if ui.small_button(t!("variations.remove")).on_hover_text(t!("tooltips.remove_variation")).clicked() {
            delete_requested = true;
        }
    });

    // Show parameters if variation has them
    if let Some(var_info) = var_info {
        if !var_info.parameters.is_empty() {
            egui::CollapsingHeader::new(t!("variations.parameters", name = display_name))
                .id_salt(format!("params_{}_{}", transform_index, variation_name))
                .default_open(false)
                .show(ui, |ui| {
                    let param_update = render_variation_params(
                        ui,
                        config_manager,
                        transform_index,
                        variation_name,
                        &var_info.parameters,
                    );
                    max_update = max_update.max(param_update);
                });
        }
    }

    (max_update, delete_requested)
}

/// Render the variations section for a transform
fn render_variations_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    transform_index: usize,
    transform: &crate::scene::transforms::Transform,
    render_mode: RenderMode,
    add_variation_popup_id: egui::Id,
) -> (UpdateType, Option<String>, Option<String>) {
    let mut max_update = UpdateType::None;
    let mut variation_to_delete: Option<String> = None;
    let mut variation_to_add: Option<String> = None;

    ui.label(t!("transform.variations"));

    // Get enabled variations sorted by name for consistent display
    let mut enabled: Vec<(String, f32)> = transform.variations.iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    enabled.sort_by(|a, b| a.0.cmp(&b.0));

    if enabled.is_empty() {
        ui.label(egui::RichText::new(t!("transform.no_variations")).italics().weak());
    } else {
        for (name, weight) in &enabled {
            let (update, delete) = render_enabled_variation(
                ui,
                config_manager,
                transform_index,
                name,
                *weight,
            );
            max_update = max_update.max(update);
            if delete {
                variation_to_delete = Some(name.clone());
            }
        }
    }

    ui.add_space(4.0);

    // Add Variation button
    let add_btn = ui.button(t!("variations.add"));
    if add_btn.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(add_variation_popup_id));
    }

    // Variation picker popup
    egui::popup_below_widget(ui, add_variation_popup_id, &add_btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(250.0);
        ui.set_max_height(300.0);

        // Search filter
        let search_id = ui.id().with("search");
        let mut search_text = ui.data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
        ui.horizontal(|ui| {
            ui.label(t!("variations.search"));
            ui.text_edit_singleline(&mut search_text);
        });
        ui.data_mut(|d| d.insert_temp(search_id, search_text.clone()));

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let registry = global_registry();
            let search_lower = search_text.to_lowercase();

            // Collect categories to show based on render mode
            let categories: Vec<VariationCategory> = if matches!(render_mode, RenderMode::ThreeD) {
                vec![
                    VariationCategory::Basic2D,
                    VariationCategory::Advanced2D,
                    VariationCategory::Depth3D,
                    VariationCategory::Rotation3D,
                    VariationCategory::Full3D,
                ]
            } else {
                vec![
                    VariationCategory::Basic2D,
                    VariationCategory::Advanced2D,
                ]
            };

            for category in categories {
                let variations = registry.by_category(category);
                let filtered: Vec<_> = variations
                    .iter()
                    .filter(|v| {
                        // Filter by search and exclude already-enabled variations
                        let matches_search = search_text.is_empty()
                            || v.name.to_lowercase().contains(&search_lower)
                            || v.display_name.to_lowercase().contains(&search_lower);
                        let not_enabled = !transform.variations.contains_key(&v.name);
                        matches_search && not_enabled
                    })
                    .collect();

                if !filtered.is_empty() {
                    ui.label(egui::RichText::new(format!("{:?}", category)).strong());
                    for var_info in filtered {
                        if ui.selectable_label(false, &var_info.display_name).clicked() {
                            variation_to_add = Some(var_info.name.clone());
                            ui.memory_mut(|mem| mem.close_popup(add_variation_popup_id));
                        }
                    }
                    ui.add_space(4.0);
                }
            }
        });
    });

    (max_update, variation_to_delete, variation_to_add)
}

/// Render the Transforms panel content (transform list, affine, variations)
///
/// This is the panel version without the Window wrapper.
pub fn render_transforms_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &mut Flame,
    add_transform: &mut bool,
    delete_transform: &mut Option<usize>,
    clone_transform: &mut Option<usize>,
    open_triangle_editor: &mut bool,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    ui.heading(format!("Transforms ({})", flame.transforms.len()));

    // Add transform button
    ui.horizontal(|ui| {
        if ui.button(t!("transform.add")).clicked() {
            *add_transform = true;
        }
    });

    ui.separator();

    // Final Transform checkbox
    ui.horizontal(|ui| {
        let mut has_final = flame.final_transform.is_some();
        if ui.checkbox(&mut has_final, t!("transform.enable_final")).on_hover_text(t!("tooltips.final_transform_help")).changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::FinalTransformEnabled,
                has_final.into()
            ) {
                flame.final_transform = config_manager.active_config().flame.final_transform.clone();
                max_update = max_update.max(update_type);
            }
        }
        // if ui.button("❓").clicked() {}
    });

    ui.separator();

    let mut delete_index = None;
    let mut clone_index = None;
    let num_transforms = flame.transforms.len();
    let render_mode = flame.render_mode;

    // Regular transforms
    for (i, transform) in flame.transforms.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            // Custom header with bold text and colored circle
            let transform_color = get_transform_color(i);

            // Use a horizontal layout to place circle after header
            let id = ui.make_persistent_id(format!("transform_header_{}", i));
            let default_open = true;
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

            let header_response = ui.horizontal(|ui| {
                // Toggle button (arrow)
                let _icon_response = state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);

                // Bold header text
                let header_text = egui::RichText::new(format!("Transform {}", i + 1))
                    .strong()
                    .size(14.0);
                let text_response = ui.label(header_text);

                // Colored circle indicator
                let (circle_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().circle_filled(circle_rect.center(), 5.0, transform_color);

                // Make entire row clickable
                text_response
            });

            // Toggle on click anywhere in header row
            if header_response.inner.clicked() {
                state.toggle(ui);
            }

            state.show_body_indented(&header_response.response, ui, |ui| {
                    // Add padding around transform body
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 0,
                            right: 5,
                            top: 5,
                            bottom: 5,
                        })
                        .show(ui, |ui| {

                    // === ALWAYS VISIBLE ===

                    // Edit Triangle button
                    ui.horizontal(|ui| {
                        if ui.button(t!("transform.edit_triangle")).on_hover_text(t!("tooltips.transform_edit_triangle")).clicked() {
                            // Update the shared selection state that Triangle Editor also reads
                            ui.ctx().data_mut(|d| {
                                d.insert_persisted(egui::Id::new("triangle_editor_selected_transform"), Some(i));
                            });
                            // Open Triangle Editor panel if not already open
                            *open_triangle_editor = true;
                        }

                        // Clone button
                        if ui.button(t!("transform.clone")).on_hover_text(t!("tooltips.transform_clone")).clicked() {
                            clone_index = Some(i);
                        }

                        // Delete button (only if more than 1 transform)
                        if num_transforms > 1 {
                            if ui.button(t!("transform.delete")).on_hover_text(t!("tooltips.transform_delete")).clicked() {
                                delete_index = Some(i);
                            }
                        }
                    });

                    // Weight control
                    let weight_update = render_weight_control(ui, config_manager, i, transform);
                    max_update = max_update.max(weight_update);

                    // Color controls (palette position + preview)
                    let color_update = render_color_controls(ui, config_manager, i, transform);
                    max_update = max_update.max(color_update);

                    // === ADVANCED SECTION (collapsed) ===
                    egui::CollapsingHeader::new(t!("transform.advanced"))
                        .id_salt(format!("advanced_{}", i))
                        .default_open(false)
                        .show(ui, |ui| {
                            // Affine Matrix
                            ui.label(t!("transform.affine_matrix"));
                            let affine_update = render_affine_controls(ui, config_manager, i, transform, render_mode);
                            max_update = max_update.max(affine_update);

                            ui.add_space(4.0);

                            // Post-Affine
                            let post_affine_update = render_post_affine_controls(ui, config_manager, i, transform, render_mode);
                            max_update = max_update.max(post_affine_update);

                            ui.add_space(4.0);

                            // Color Speed, Opacity, and Solo toggle
                            let solo_transform = config_manager.active_config().flame.solo_transform;
                            let advanced_update = render_advanced_settings(ui, config_manager, i, transform, solo_transform);
                            max_update = max_update.max(advanced_update);
                        });

                    ui.add_space(4.0);

                    // === VARIATIONS SECTION (with border) ===
                    let popup_id = ui.id().with("add_var_popup");
                    let mut var_update = UpdateType::None;
                    let mut var_to_delete = None;
                    let mut var_to_add = None;

                    egui::Frame::new()
                        .fill(ui.visuals().extreme_bg_color)
                        .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                        .corner_radius(4.0)
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            let (update, to_delete, to_add) = render_variations_section(
                                ui,
                                config_manager,
                                i,
                                transform,
                                render_mode,
                                popup_id,
                            );
                            var_update = update;
                            var_to_delete = to_delete;
                            var_to_add = to_add;
                        });

                    max_update = max_update.max(var_update);

                    // Handle variation deletion
                    if let Some(var_name) = var_to_delete {
                        let path = ConfigPath::TransformVariation {
                            index: i,
                            variation: var_name,
                        };
                        // Use NaN as sentinel value to signal removal
                        if let Ok(update_type) = config_manager.update_param(path, f32::NAN.into()) {
                            max_update = max_update.max(update_type);
                        }
                    }

                    // Handle variation addition
                    if let Some(var_name) = var_to_add {
                        let path = ConfigPath::TransformVariation {
                            index: i,
                            variation: var_name,
                        };
                        // Add with weight 1.0 (not 0.0 since that would remove it immediately)
                        if let Ok(update_type) = config_manager.update_param(path, 1.0f32.into()) {
                            max_update = max_update.max(update_type);
                        }
                    }

                }); // end Frame
            }); // end show_body_indented
        });
    }

    // Final transform (if enabled)
    if let Some(final_xform) = &mut flame.final_transform {
        ui.separator();
        render_final_transform(ui, config_manager, final_xform, render_mode, &mut max_update, open_triangle_editor);
    }

    // Set delete_transform if a transform was marked for deletion
    if let Some(idx) = delete_index {
        *delete_transform = Some(idx);
    }

    // Set clone_transform if a transform was marked for cloning
    if let Some(idx) = clone_index {
        *clone_transform = Some(idx);
    }

    max_update
}

/// Render the final transform section
fn render_final_transform(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    final_xform: &mut crate::scene::transforms::Transform,
    render_mode: RenderMode,
    max_update: &mut UpdateType,
    open_triangle_editor: &mut bool,
) {
    ui.push_id("final_transform", |ui| {
        let style = ui.style_mut();
        style.visuals.collapsing_header_frame = true;

        // Bold header text
        let header_text = egui::RichText::new(t!("transform.final"))
            .strong()
            .size(14.0);

        egui::CollapsingHeader::new(header_text)
            .default_open(true)
            .show(ui, |ui| {

                // Note: Weight, Color, Color Speed, and Opacity are NOT used for final transforms.
                // The final transform only applies affine + variations to position after color is computed.

                ui.add_space(3.0);

                // Edit Triangle button
                if ui.button(t!("transform.edit_triangle")).on_hover_text(t!("tooltips.transform_edit_triangle")).clicked() {
                    // Set selection to None (final transform) in the shared state
                    ui.ctx().data_mut(|d| {
                        d.insert_persisted(egui::Id::new("triangle_editor_selected_transform"), None::<usize>);
                    });
                    // Open Triangle Editor panel if not already open
                    *open_triangle_editor = true;
                }

                ui.add_space(3.0);

                // Advanced section for final transform
                egui::CollapsingHeader::new(t!("transform.advanced"))
                    .id_salt("advanced_final")
                    .default_open(false)
                    .show(ui, |ui| {
                        // Affine Matrix
                        ui.label(t!("transform.affine_matrix"));
                        macro_rules! affine_param_final {
                            ($label:expr, $tooltip:expr, $field:ident, $param:ident) => {
                                ui.horizontal(|ui| {
                                    ui.label($label).on_hover_text($tooltip);
                                    let mut temp = final_xform.$field;
                                    let response = ui.add(egui::DragValue::new(&mut temp).speed(0.01)).on_hover_text($tooltip);
                                    if response.changed() {
                                        if let Ok(update_type) = config_manager.update_param(
                                            ConfigPath::FinalTransformAffine { param: AffineParam::$param },
                                            temp.into()
                                        ) {
                                            final_xform.$field = config_manager.active_config().flame.final_transform.as_ref().unwrap().$field;
                                            *max_update = (*max_update).max(update_type);
                                        }
                                    }
                                    if response.drag_stopped() {
                                        let _ = config_manager.force_commit_preview(&ConfigPath::FinalTransformAffine { param: AffineParam::$param });
                                    }
                                });
                            };
                        }

                        affine_param_final!(t!("transform.affine_a"), t!("tooltips.affine_a"), a, A);
                        affine_param_final!(t!("transform.affine_b"), t!("tooltips.affine_b"), b, B);
                        affine_param_final!(t!("transform.affine_c"), t!("tooltips.affine_c"), c, C);
                        affine_param_final!(t!("transform.affine_d"), t!("tooltips.affine_d"), d, D);
                        affine_param_final!(t!("transform.affine_e"), t!("tooltips.affine_e"), e, E);
                        affine_param_final!(t!("transform.affine_f"), t!("tooltips.affine_f"), f, F);

                        if matches!(render_mode, RenderMode::ThreeD) {
                            affine_param_final!(t!("transform.affine_g"), t!("tooltips.affine_g"), g, G);
                        }

                        ui.add_space(4.0);

                        // Post-Affine for final transform
                        let mut temp_post_enabled = final_xform.post_affine_enabled;
                        if ui.checkbox(&mut temp_post_enabled, t!("transform.post_affine_enabled"))
                            .on_hover_text(t!("tooltips.post_affine_enabled"))
                            .changed()
                        {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::FinalTransformPostAffineEnabled,
                                temp_post_enabled.into()
                            ) {
                                final_xform.post_affine_enabled = config_manager.active_config().flame.final_transform.as_ref().unwrap().post_affine_enabled;
                                *max_update = (*max_update).max(update_type);
                            }
                        }

                        if final_xform.post_affine_enabled {
                            ui.label(t!("transform.post_affine_matrix"));

                            macro_rules! post_affine_param_final {
                                ($label:expr, $tooltip:expr, $field:ident, $param:ident) => {
                                    ui.horizontal(|ui| {
                                        ui.label($label).on_hover_text($tooltip);
                                        let mut temp = final_xform.$field;
                                        let response = ui.add(egui::DragValue::new(&mut temp).speed(0.01)).on_hover_text($tooltip);
                                        if response.changed() {
                                            if let Ok(update_type) = config_manager.update_param(
                                                ConfigPath::FinalTransformPostAffine { param: AffineParam::$param },
                                                temp.into()
                                            ) {
                                                final_xform.$field = config_manager.active_config().flame.final_transform.as_ref().unwrap().$field;
                                                *max_update = (*max_update).max(update_type);
                                            }
                                        }
                                        if response.drag_stopped() {
                                            let _ = config_manager.force_commit_preview(&ConfigPath::FinalTransformPostAffine { param: AffineParam::$param });
                                        }
                                    });
                                };
                            }

                            post_affine_param_final!(t!("transform.affine_a"), t!("tooltips.affine_a"), post_a, A);
                            post_affine_param_final!(t!("transform.affine_b"), t!("tooltips.affine_b"), post_b, B);
                            post_affine_param_final!(t!("transform.affine_c"), t!("tooltips.affine_c"), post_c, C);
                            post_affine_param_final!(t!("transform.affine_d"), t!("tooltips.affine_d"), post_d, D);
                            post_affine_param_final!(t!("transform.affine_e"), t!("tooltips.affine_e"), post_e, E);
                            post_affine_param_final!(t!("transform.affine_f"), t!("tooltips.affine_f"), post_f, F);

                            if matches!(render_mode, RenderMode::ThreeD) {
                                post_affine_param_final!(t!("transform.affine_g"), t!("tooltips.affine_g"), post_g, G);
                            }
                        }

                        // Note: Color, Color Speed, Opacity, and Weight are NOT used by the final transform.
                        // The final transform only applies affine + variations to position after color is computed.
                    });

                // Variations section for final transform
                egui::Frame::new()
                    .fill(ui.visuals().extreme_bg_color)
                    .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                    .corner_radius(4.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.label(t!("transform.variations"));

                        let mut enabled: Vec<(String, f32)> = final_xform.variations.iter()
                            .map(|(k, v)| (k.clone(), *v))
                            .collect();
                        enabled.sort_by(|a, b| a.0.cmp(&b.0));

                        let mut var_to_delete: Option<String> = None;

                        if enabled.is_empty() {
                            ui.label(egui::RichText::new(t!("transform.no_variations")).italics().weak());
                        } else {
                            for (name, weight) in &enabled {
                                let registry = global_registry();
                                let var_info = registry.get(&name);
                                let display_name = var_info
                                    .map(|v| v.display_name.as_str())
                                    .unwrap_or(&name);

                                ui.horizontal(|ui| {
                                    let mut value = *weight;
                                    let response = ui.add(
                                        egui::Slider::new(&mut value, -5.0..=5.0)
                                            .text(display_name)
                                            .drag_value_speed(0.1)
                                            .clamping(egui::SliderClamping::Never)
                                    );

                                    if response.changed() {
                                        let path = ConfigPath::FinalTransformVariation {
                                            variation: name.clone(),
                                        };
                                        if let Ok(update_type) = config_manager.update_param(path, value.into()) {
                                            *max_update = (*max_update).max(update_type);
                                        }
                                    }
                                    if response.drag_stopped() {
                                        let path = ConfigPath::FinalTransformVariation {
                                            variation: name.clone(),
                                        };
                                        let _ = config_manager.force_commit_preview(&path);
                                    }
                                    if ui.small_button("🗑").on_hover_text(t!("tooltips.remove_variation")).clicked() {
                                        var_to_delete = Some(name.clone());
                                    }
                                });

                                // Parameters
                                if let Some(var_info) = var_info {
                                    if !var_info.parameters.is_empty() {
                                        egui::CollapsingHeader::new(t!("variations.parameters", name = display_name))
                                            .id_salt(format!("final_params_{}", name))
                                            .default_open(false)
                                            .show(ui, |ui| {
                                                let param_update = super::variation_params::render_variation_params_final(
                                                    ui,
                                                    config_manager,
                                                    &name,
                                                    &var_info.parameters,
                                                );
                                                *max_update = (*max_update).max(param_update);
                                            });
                                    }
                                }
                            }
                        }

                        // Handle deletion
                        if let Some(var_name) = var_to_delete {
                            let path = ConfigPath::FinalTransformVariation {
                                variation: var_name,
                            };
                            // Use NaN as sentinel value to signal removal
                            if let Ok(update_type) = config_manager.update_param(path, f32::NAN.into()) {
                                *max_update = (*max_update).max(update_type);
                            }
                        }

                        // Add Variation button for final transform
                        let popup_id = ui.id().with("add_var_popup_final");
                        let add_btn = ui.button(t!("variations.add"));
                        if add_btn.clicked() {
                            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                        }

                        egui::popup_below_widget(ui, popup_id, &add_btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                            ui.set_min_width(250.0);
                            ui.set_max_height(300.0);

                            let search_id = ui.id().with("search_final");
                            let mut search_text = ui.data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
                            ui.horizontal(|ui| {
                                ui.label(t!("variations.search"));
                                ui.text_edit_singleline(&mut search_text);
                            });
                            ui.data_mut(|d| d.insert_temp(search_id, search_text.clone()));

                            ui.separator();

                            egui::ScrollArea::vertical().show(ui, |ui| {
                                let registry = global_registry();
                                let search_lower = search_text.to_lowercase();

                                let categories: Vec<VariationCategory> = if matches!(render_mode, RenderMode::ThreeD) {
                                    vec![
                                        VariationCategory::Basic2D,
                                        VariationCategory::Advanced2D,
                                        VariationCategory::Depth3D,
                                        VariationCategory::Rotation3D,
                                        VariationCategory::Full3D,
                                    ]
                                } else {
                                    vec![
                                        VariationCategory::Basic2D,
                                        VariationCategory::Advanced2D,
                                    ]
                                };

                                for category in categories {
                                    let variations = registry.by_category(category);
                                    let filtered: Vec<_> = variations
                                        .iter()
                                        .filter(|v| {
                                            let matches_search = search_text.is_empty()
                                                || v.name.to_lowercase().contains(&search_lower)
                                                || v.display_name.to_lowercase().contains(&search_lower);
                                            let not_enabled = !final_xform.variations.contains_key(&v.name);
                                            matches_search && not_enabled
                                        })
                                        .collect();

                                    if !filtered.is_empty() {
                                        ui.label(egui::RichText::new(format!("{:?}", category)).strong());
                                        for var_info in filtered {
                                            if ui.selectable_label(false, &var_info.display_name).clicked() {
                                                let path = ConfigPath::FinalTransformVariation {
                                                    variation: var_info.name.clone(),
                                                };
                                                if let Ok(update_type) = config_manager.update_param(path, 1.0f32.into()) {
                                                    *max_update = (*max_update).max(update_type);
                                                }
                                                ui.memory_mut(|mem| mem.close_popup(popup_id));
                                            }
                                        }
                                        ui.add_space(4.0);
                                    }
                                }
                            });
                        });
                    });

            });
    });
}
