use crate::scene::transforms::{Flame, RenderMode};
use crate::variations::{VariationCategory, global_registry};
use crate::config::{ConfigManager, ConfigPath, UpdateType, AffineParam};
use super::variation_params::render_variation_params;
use egui::Color32;

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
    let response = ui.add(egui::Slider::new(&mut temp_weight, 0.0..=1024.0).logarithmic(true).text("Weight"));
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
                .text("Color")
                .show_value(false)
        );
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
        if let Some(palette) = &config_manager.active_config().palette {
            let actual_color = palette.sample_color(transform.color);
            let color_swatch = egui::Color32::from_rgb(
                (actual_color[0] * 255.0) as u8,
                (actual_color[1] * 255.0) as u8,
                (actual_color[2] * 255.0) as u8,
            );
            let (rect, _response) = ui.allocate_exact_size(egui::vec2(20.0, 18.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, color_swatch);
        }
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
        ui.label("a:");
        let mut temp_a = transform.a;
        let response_a = ui.add(egui::DragValue::new(&mut temp_a).speed(0.01));
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

        ui.label("b:");
        let mut temp_b = transform.b;
        let response_b = ui.add(egui::DragValue::new(&mut temp_b).speed(0.01));
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
        ui.label("c:");
        let mut temp_c = transform.c;
        let response_c = ui.add(egui::DragValue::new(&mut temp_c).speed(0.01));
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

        ui.label("d:");
        let mut temp_d = transform.d;
        let response_d = ui.add(egui::DragValue::new(&mut temp_d).speed(0.01));
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
        ui.label("e:");
        let mut temp_e = transform.e;
        let response_e = ui.add(egui::DragValue::new(&mut temp_e).speed(0.01));
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

        ui.label("f:");
        let mut temp_f = transform.f;
        let response_f = ui.add(egui::DragValue::new(&mut temp_f).speed(0.01));
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
            ui.label("g (Z):");
            let mut temp_g = transform.g;
            let response_g = ui.add(egui::DragValue::new(&mut temp_g).speed(0.01));
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

/// Render advanced settings (color speed, opacity)
fn render_advanced_settings(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Color Speed (Symmetry)
    let mut temp_speed = transform.color_speed;
    let response_speed = ui.add(egui::Slider::new(&mut temp_speed, -1.0..=1.0).text("Color Speed"));
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
    let response_opacity = ui.add(egui::Slider::new(&mut temp_opacity, 0.0..=1.0).text("Opacity"));
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
        // Delete button

        // Weight slider
        let mut value = current_weight;
        let response = ui.add(
            egui::Slider::new(&mut value, -10.0..=10.0)
                .text(display_name)
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
        
        if ui.small_button("🗑").on_hover_text("Remove variation").clicked() {
            delete_requested = true;
        }
    });

    // Show parameters if variation has them
    if let Some(var_info) = var_info {
        if !var_info.parameters.is_empty() {
            egui::CollapsingHeader::new(format!("{} Parameters", display_name))
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

    ui.label("Variations");

    // Get enabled variations sorted by name for consistent display
    let mut enabled: Vec<(String, f32)> = transform.variations.iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    enabled.sort_by(|a, b| a.0.cmp(&b.0));

    if enabled.is_empty() {
        ui.label(egui::RichText::new("No variations enabled").italics().weak());
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
    let add_btn = ui.button("➕ Add Variation");
    if add_btn.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(add_variation_popup_id));
    }

    // Variation picker popup
    egui::popup_below_widget(ui, add_variation_popup_id, &add_btn, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(250.0);
        ui.set_max_height(300.0);

        // Search filter
        let search_id = ui.id().with("search");
        let mut search_text = ui.data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
        ui.horizontal(|ui| {
            ui.label("Search:");
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
        if ui.button("➕ Add Transform").clicked() {
            *add_transform = true;
        }
    });

    ui.separator();

    // Final Transform checkbox
    ui.horizontal(|ui| {
        let mut has_final = flame.final_transform.is_some();
        if ui.checkbox(&mut has_final, "Enable Final Transform").changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::FinalTransformEnabled,
                has_final.into()
            ) {
                flame.final_transform = config_manager.active_config().flame.final_transform.clone();
                max_update = max_update.max(update_type);
            }
        }
        if ui.button("❓").on_hover_text("Post-processing transform applied once to every point after iteration loop.\nUsed for framing, positioning, or global effects.").clicked() {}
    });

    ui.separator();

    let mut delete_index = None;
    let mut clone_index = None;
    let num_transforms = flame.transforms.len();
    let render_mode = flame.render_mode;

    egui::ScrollArea::vertical().show(ui, |ui| {
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
                    let icon_response = state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);

                    // Bold header text
                    let header_text = egui::RichText::new(format!("Transform {}", i + 1))
                        .strong()
                        .size(14.0);
                    let text_response = ui.label(header_text);

                    // Colored circle indicator
                    let (circle_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    ui.painter().circle_filled(circle_rect.center(), 5.0, transform_color);

                    // Make entire row clickable
                    icon_response | text_response
                });

                // Toggle on click anywhere in header row
                if header_response.inner.clicked() {
                    state.toggle(ui);
                }

                state.show_body_unindented(ui, |ui| {
                        // === ALWAYS VISIBLE ===

                        // Edit Triangle button
                        ui.horizontal(|ui| {
                            if ui.button("🔺 Edit Triangle").on_hover_text("Select in Triangle Editor").clicked() {
                                // Update the shared selection state that Triangle Editor also reads
                                ui.ctx().data_mut(|d| {
                                    d.insert_persisted(egui::Id::new("triangle_editor_selected_transform"), Some(i));
                                });
                                // Open Triangle Editor panel if not already open
                                *open_triangle_editor = true;
                            }

                            // Clone button
                            if ui.button("📋 Clone").on_hover_text("Create a copy").clicked() {
                                clone_index = Some(i);
                            }

                            // Delete button (only if more than 1 transform)
                            if num_transforms > 1 {
                                if ui.button("🗑").on_hover_text("Delete").clicked() {
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
                        egui::CollapsingHeader::new("Advanced")
                            .id_salt(format!("advanced_{}", i))
                            .default_open(false)
                            .show(ui, |ui| {
                                // Affine Matrix
                                ui.label("Affine Matrix");
                                let affine_update = render_affine_controls(ui, config_manager, i, transform, render_mode);
                                max_update = max_update.max(affine_update);

                                ui.add_space(4.0);

                                // Color Speed and Opacity
                                let advanced_update = render_advanced_settings(ui, config_manager, i, transform);
                                max_update = max_update.max(advanced_update);
                            });

                        ui.add_space(4.0);

                        // === VARIATIONS SECTION (with border) ===
                        let popup_id = ui.id().with("add_var_popup");
                        let mut var_update = UpdateType::None;
                        let mut var_to_delete = None;
                        let mut var_to_add = None;

                        egui::Frame::none()
                            .fill(ui.visuals().extreme_bg_color)
                            .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                            .rounding(4.0)
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

                    });
            });
        }

        // Final transform (if enabled)
        if let Some(final_xform) = &mut flame.final_transform {
            ui.separator();
            render_final_transform(ui, config_manager, final_xform, render_mode, &mut max_update);
        }
    });

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
) {
    ui.push_id("final_transform", |ui| {
        let style = ui.style_mut();
        style.visuals.collapsing_header_frame = true;

        egui::CollapsingHeader::new("Transform [Final]")
            .default_open(false)
            .show(ui, |ui| {
                ui.colored_label(
                    egui::Color32::LIGHT_GRAY,
                    "Applied once to all points after iteration."
                );
                ui.separator();

                // Weight (not really used for final transform, but keep for consistency)
                // Final transforms don't have weight in the traditional sense

                // Color control
                let mut temp_color = final_xform.color;
                let response_color = ui.add(
                    egui::Slider::new(&mut temp_color, 0.0..=1.0).text("Color")
                );
                if response_color.changed() {
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::FinalTransformColor,
                        temp_color.into()
                    ) {
                        final_xform.color = config_manager.active_config().flame.final_transform.as_ref().unwrap().color;
                        *max_update = (*max_update).max(update_type);
                    }
                }
                if response_color.drag_stopped() {
                    let _ = config_manager.force_commit_preview(&ConfigPath::FinalTransformColor);
                }

                // Variations for final transform
                ui.add_space(4.0);
                ui.label("Variations");

                let mut enabled: Vec<(String, f32)> = final_xform.variations.iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect();
                enabled.sort_by(|a, b| a.0.cmp(&b.0));

                let mut var_to_delete: Option<String> = None;

                if enabled.is_empty() {
                    ui.label(egui::RichText::new("No variations enabled").italics().weak());
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
                                egui::Slider::new(&mut value, -10.0..=10.0)
                                    .text(display_name)
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
                            if ui.small_button("🗑").on_hover_text("Remove variation").clicked() {
                                var_to_delete = Some(name.clone());
                            }
                        });

                        // Parameters
                        if let Some(var_info) = var_info {
                            if !var_info.parameters.is_empty() {
                                egui::CollapsingHeader::new(format!("{} Parameters", display_name))
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
                let add_btn = ui.button("➕ Add Variation");
                if add_btn.clicked() {
                    ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                }

                egui::popup_below_widget(ui, popup_id, &add_btn, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(250.0);
                    ui.set_max_height(300.0);

                    let search_id = ui.id().with("search_final");
                    let mut search_text = ui.data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
                    ui.horizontal(|ui| {
                        ui.label("Search:");
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

                // Advanced section for final transform
                egui::CollapsingHeader::new("Advanced")
                    .id_salt("advanced_final")
                    .default_open(false)
                    .show(ui, |ui| {
                        // Affine Matrix
                        ui.label("Affine Matrix");
                        macro_rules! affine_param_final {
                            ($label:expr, $field:ident, $param:ident) => {
                                ui.horizontal(|ui| {
                                    ui.label($label);
                                    let mut temp = final_xform.$field;
                                    let response = ui.add(egui::DragValue::new(&mut temp).speed(0.01));
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

                        affine_param_final!("a:", a, A);
                        affine_param_final!("b:", b, B);
                        affine_param_final!("c:", c, C);
                        affine_param_final!("d:", d, D);
                        affine_param_final!("e:", e, E);
                        affine_param_final!("f:", f, F);

                        if matches!(render_mode, RenderMode::ThreeD) {
                            affine_param_final!("g (Z):", g, G);
                        }

                        ui.add_space(4.0);

                        // Color Speed
                        let mut temp_speed = final_xform.color_speed;
                        let response_speed = ui.add(egui::Slider::new(&mut temp_speed, -1.0..=1.0).text("Color Speed"));
                        if response_speed.changed() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::FinalTransformColorSpeed,
                                temp_speed.into()
                            ) {
                                final_xform.color_speed = config_manager.active_config().flame.final_transform.as_ref().unwrap().color_speed;
                                *max_update = (*max_update).max(update_type);
                            }
                        }
                        if response_speed.drag_stopped() {
                            let _ = config_manager.force_commit_preview(&ConfigPath::FinalTransformColorSpeed);
                        }
                    });
            });
    });
}
