use crate::scene::transforms::{Flame, RenderMode};
use crate::variations::VariationCategory;
use crate::config::{ConfigManager, ConfigPath, UpdateType, AffineParam};
use super::variation_controls::{render_variation_category, render_variation_category_final};



/// Render affine matrix controls (a, b, c, d, e, f)
fn render_affine_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    ui.label("Affine Matrix");

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

    max_update
}

/// Render color controls (color, color speed, blend, opacity)
fn render_color_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Palette position slider (0.0 to 1.0)
    let mut temp_color = transform.color;
    let response_color = ui.add(egui::Slider::new(&mut temp_color, 0.0..=1.0).text("Palette Position"));
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
        ui.horizontal(|ui| {
            ui.label("Color:");
            let mut color_swatch = egui::Color32::from_rgb(
                (actual_color[0] * 255.0) as u8,
                (actual_color[1] * 255.0) as u8,
                (actual_color[2] * 255.0) as u8,
            );
            ui.color_edit_button_srgba(&mut color_swatch);
        });
    }

    let mut temp_speed = transform.color_speed;
    let response_speed = ui.add(egui::Slider::new(&mut temp_speed, -1.0..=1.0).text("Color Speed (Symmetry)"));
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

/// Render the Transforms panel content (transform list, affine, variations)
///
/// This is the panel version without the Window wrapper.
pub fn render_transforms_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &mut Flame,
    add_transform: &mut bool,
    delete_transform: &mut Option<usize>,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    ui.heading(format!("Transforms ({})", flame.transforms.len()));

    // Add/Delete transform buttons
    ui.horizontal(|ui| {
        if ui.button("➕ Add Transform").clicked() {
            *add_transform = true;
        }
        ui.label(format!("({} transforms)", flame.transforms.len()));
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
    let num_transforms = flame.transforms.len();
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Regular transforms
        for (i, transform) in flame.transforms.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                egui::CollapsingHeader::new(format!("Transform {}", i + 1))
                    .default_open(i == 0)
                    .show(ui, |ui| {
                        // Affine Matrix controls
                        let affine_update = render_affine_controls(ui, config_manager, i, transform);
                        max_update = max_update.max(affine_update);

                        // Z offset (only in 3D mode)
                        if matches!(flame.render_mode, RenderMode::ThreeD) {
                            ui.horizontal(|ui| {
                                ui.label("g (Z offset):");
                                let mut temp_g = transform.g;
                                if ui.add(egui::DragValue::new(&mut temp_g).speed(0.01)).changed() {
                                    if let Ok(update_type) = config_manager.update_param(
                                        ConfigPath::TransformAffine { index: i, param: AffineParam::G },
                                        temp_g.into()
                                    ) {
                                        transform.g = config_manager.active_config().flame.transforms[i].g;
                                        max_update = max_update.max(update_type);
                                    }
                                }
                            });
                        }

                        ui.separator();

                        // Weight control
                        ui.label("Weight");
                        let mut temp_weight = transform.weight;
                        let response = ui.add(egui::Slider::new(&mut temp_weight, 0.0..=1024.0).logarithmic(true));
                        if response.changed() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::TransformWeight { index: i },
                                temp_weight.into()
                            ) {
                                transform.weight = config_manager.active_config().flame.transforms[i].weight;
                                max_update = max_update.max(update_type);
                            }
                        }
                        if response.drag_stopped() {
                            let _ = config_manager.force_commit_preview(&ConfigPath::TransformWeight { index: i });
                        }

                        ui.separator();

                        // Color controls
                        let color_update = render_color_controls(ui, config_manager, i, transform);
                        max_update = max_update.max(color_update);

                        // Variation controls by category
                        let var_update = render_variation_category(ui, config_manager, i, VariationCategory::Basic2D, "Basic 2D Variations");
                        max_update = max_update.max(var_update);

                        let var_update = render_variation_category(ui, config_manager, i, VariationCategory::Advanced2D, "Advanced 2D Variations");
                        max_update = max_update.max(var_update);

                        // 3D variation categories (only visible in 3D mode)
                        if matches!(flame.render_mode, RenderMode::ThreeD) {
                            let var_update = render_variation_category(ui, config_manager, i, VariationCategory::Depth3D, "3D Depth Variations");
                            max_update = max_update.max(var_update);

                            let var_update = render_variation_category(ui, config_manager, i, VariationCategory::Rotation3D, "3D Rotation Variations");
                            max_update = max_update.max(var_update);

                            let var_update = render_variation_category(ui, config_manager, i, VariationCategory::Full3D, "Full 3D Variations");
                            max_update = max_update.max(var_update);
                        }

                        ui.separator();

                        // Delete button (only show if more than 1 transform exists)
                        if num_transforms > 1 {
                            if ui.button("🗑 Delete Transform").clicked() {
                                delete_index = Some(i);
                            }
                        }
                    });
            });
        }

        // Final transform (if enabled)
        if let Some(final_xform) = &mut flame.final_transform {
            ui.separator();
            ui.push_id("final_transform", |ui| {
                let style = ui.style_mut();
                style.visuals.collapsing_header_frame = true;

                egui::CollapsingHeader::new("Transform [Final]")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.colored_label(
                            egui::Color32::LIGHT_GRAY,
                            "Final transform is applied once to all points after iteration loop."
                        );
                        ui.separator();

                        // Affine Matrix controls
                        ui.label("Affine Matrix");
                        macro_rules! affine_param {
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
                                            max_update = max_update.max(update_type);
                                        }
                                    }
                                    if response.drag_stopped() {
                                        let _ = config_manager.force_commit_preview(&ConfigPath::FinalTransformAffine { param: AffineParam::$param });
                                    }
                                });
                            };
                        }

                        affine_param!("a:", a, A);
                        affine_param!("b:", b, B);
                        affine_param!("c:", c, C);
                        affine_param!("d:", d, D);
                        affine_param!("e:", e, E);
                        affine_param!("f:", f, F);

                        // Z offset (only in 3D mode)
                        if matches!(flame.render_mode, RenderMode::ThreeD) {
                            affine_param!("g (Z offset):", g, G);
                        }

                        ui.separator();

                        // Color controls
                        ui.label("Color Properties");
                        let mut temp_color = final_xform.color;
                        let response_color = ui.add(egui::Slider::new(&mut temp_color, 0.0..=1.0).text("Color Coordinate"));
                        if response_color.changed() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::FinalTransformColor,
                                temp_color.into()
                            ) {
                                final_xform.color = config_manager.active_config().flame.final_transform.as_ref().unwrap().color;
                                max_update = max_update.max(update_type);
                            }
                        }
                        if response_color.drag_stopped() {
                            let _ = config_manager.force_commit_preview(&ConfigPath::FinalTransformColor);
                        }

                        let mut temp_speed = final_xform.color_speed;
                        let response_speed = ui.add(egui::Slider::new(&mut temp_speed, -1.0..=1.0).text("Color Speed (Symmetry)"));
                        if response_speed.changed() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::FinalTransformColorSpeed,
                                temp_speed.into()
                            ) {
                                final_xform.color_speed = config_manager.active_config().flame.final_transform.as_ref().unwrap().color_speed;
                                max_update = max_update.max(update_type);
                            }
                        }
                        if response_speed.drag_stopped() {
                            let _ = config_manager.force_commit_preview(&ConfigPath::FinalTransformColorSpeed);
                        }

                        // Variation controls by category
                        let var_update = render_variation_category_final(ui, config_manager, VariationCategory::Basic2D, "Basic 2D Variations");
                        max_update = max_update.max(var_update);

                        let var_update = render_variation_category_final(ui, config_manager, VariationCategory::Advanced2D, "Advanced 2D Variations");
                        max_update = max_update.max(var_update);

                        // 3D variation categories (only visible in 3D mode)
                        if matches!(flame.render_mode, RenderMode::ThreeD) {
                            let var_update = render_variation_category_final(ui, config_manager, VariationCategory::Depth3D, "3D Depth Variations");
                            max_update = max_update.max(var_update);

                            let var_update = render_variation_category_final(ui, config_manager, VariationCategory::Rotation3D, "3D Rotation Variations");
                            max_update = max_update.max(var_update);

                            let var_update = render_variation_category_final(ui, config_manager, VariationCategory::Full3D, "Full 3D Variations");
                            max_update = max_update.max(var_update);
                        }
                    });
            });
        }
    });

    // Set delete_transform if a transform was marked for deletion
    if let Some(idx) = delete_index {
        *delete_transform = Some(idx);
    }

    max_update
}

