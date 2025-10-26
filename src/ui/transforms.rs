use crate::scene::transforms::{Flame, RenderMode};

/// Render the Transforms window with transform editing controls
pub fn render_transforms_window(
    ctx: &egui::Context,
    show_transforms: &mut bool,
    flame: &mut Flame,
    flame_changed: &mut bool,
    add_transform: &mut bool,
    delete_transform: &mut Option<usize>,
) {
    egui::Window::new("Transforms")
        .open(show_transforms)
        .show(ctx, |ui| {
            ui.heading(format!("Transforms ({})", flame.transforms.len()));

            // Add/Delete transform buttons
            ui.horizontal(|ui| {
                if ui.button("➕ Add Transform").clicked() {
                    *add_transform = true;
                    *flame_changed = true;
                }
                ui.label(format!("({} transforms)", flame.transforms.len()));
            });

            ui.separator();

            let mut delete_index = None;
            let num_transforms = flame.transforms.len();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, transform) in flame.transforms.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        egui::CollapsingHeader::new(format!("Transform {}", i + 1))
                            .default_open(i == 0)
                            .show(ui, |ui| {
                                ui.label("Affine Matrix");

                                ui.horizontal(|ui| {
                                    ui.label("a:");
                                    if ui.add(egui::DragValue::new(&mut transform.a).speed(0.01)).changed() {
                                        *flame_changed = true;
                                    }
                                    ui.label("b:");
                                    if ui.add(egui::DragValue::new(&mut transform.b).speed(0.01)).changed() {
                                        *flame_changed = true;
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label("c:");
                                    if ui.add(egui::DragValue::new(&mut transform.c).speed(0.01)).changed() {
                                        *flame_changed = true;
                                    }
                                    ui.label("d:");
                                    if ui.add(egui::DragValue::new(&mut transform.d).speed(0.01)).changed() {
                                        *flame_changed = true;
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label("e:");
                                    if ui.add(egui::DragValue::new(&mut transform.e).speed(0.01)).changed() {
                                        *flame_changed = true;
                                    }
                                    ui.label("f:");
                                    if ui.add(egui::DragValue::new(&mut transform.f).speed(0.01)).changed() {
                                        *flame_changed = true;
                                    }
                                });

                                // Z offset (only show in 3D mode)
                                if matches!(flame.render_mode, RenderMode::ThreeD) {
                                    ui.horizontal(|ui| {
                                        ui.label("g (Z offset):");
                                        if ui.add(egui::DragValue::new(&mut transform.g).speed(0.01)).changed() {
                                            *flame_changed = true;
                                        }
                                    });
                                }

                                ui.separator();
                                ui.label("Weight");
                                if ui.add(egui::Slider::new(&mut transform.weight, 0.0..=1024.0).logarithmic(true)).changed() {
                                    *flame_changed = true;
                                }

                                ui.separator();
                                ui.label("Color");
                                if ui.horizontal(|ui| {
                                    ui.label("R:");
                                    let r_changed = ui.add(egui::Slider::new(&mut transform.color[0], 0.0..=1.0)).changed();
                                    ui.label("G:");
                                    let g_changed = ui.add(egui::Slider::new(&mut transform.color[1], 0.0..=1.0)).changed();
                                    ui.label("B:");
                                    let b_changed = ui.add(egui::Slider::new(&mut transform.color[2], 0.0..=1.0)).changed();
                                    r_changed || g_changed || b_changed
                                }).inner {
                                    *flame_changed = true;
                                }

                                if ui.add(egui::Slider::new(&mut transform.color_speed, 0.0..=1.0).text("Color Speed")).changed() {
                                    *flame_changed = true;
                                }

                                ui.separator();
                                ui.label("Basic 2D Variations");

                                // Get basic 2D variations from registry
                                let basic_2d = crate::variations::global_registry().by_category(crate::variations::VariationCategory::Basic2D);
                                for var_info in basic_2d {
                                    let mut value = transform.get_variation(&var_info.name);
                                    if ui.add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name)).changed() {
                                        transform.set_variation(&var_info.name, value);
                                        *flame_changed = true;
                                    }

                                    // Show parameters if variation is active and has parameters
                                    if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
                                        ui.indent(format!("params_{}", var_info.name), |ui| {
                                            for param in &var_info.parameters {
                                                let mut param_value = transform.get_variation_param_or_default(
                                                    &var_info.name,
                                                    &param.name,
                                                    &crate::variations::global_registry(),
                                                );

                                                let param_changed = match param.param_type {
                                                    crate::variations::ParamType::Float => {
                                                        let min = param.min_value.unwrap_or(-10.0);
                                                        let max = param.max_value.unwrap_or(10.0);
                                                        ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                            .text(&param.display_name)
                                                            .step_by(0.01))
                                                            .changed()
                                                    }
                                                    crate::variations::ParamType::Integer => {
                                                        let min = param.min_value.unwrap_or(1.0) as i32;
                                                        let max = param.max_value.unwrap_or(10.0) as i32;
                                                        let mut int_val = param_value as i32;
                                                        let changed = ui.add(egui::Slider::new(&mut int_val, min..=max)
                                                            .text(&param.display_name))
                                                            .changed();
                                                        param_value = int_val as f32;
                                                        changed
                                                    }
                                                    crate::variations::ParamType::Angle => {
                                                        let min = param.min_value.unwrap_or(0.0);
                                                        let max = param.max_value.unwrap_or(360.0);
                                                        ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                            .text(&param.display_name)
                                                            .suffix("°"))
                                                            .changed()
                                                    }
                                                };

                                                if param_changed {
                                                    transform.set_variation_param(&var_info.name, &param.name, param_value);
                                                    *flame_changed = true;
                                                }
                                            }
                                        });
                                    }
                                }

                                ui.separator();
                                ui.label("Advanced 2D Variations");

                                // Get advanced 2D variations from registry
                                let advanced_2d = crate::variations::global_registry().by_category(crate::variations::VariationCategory::Advanced2D);
                                for var_info in advanced_2d {
                                    let mut value = transform.get_variation(&var_info.name);
                                    if ui.add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name)).changed() {
                                        transform.set_variation(&var_info.name, value);
                                        *flame_changed = true;
                                    }

                                    // Show parameters if variation is active and has parameters
                                    if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
                                        ui.indent(format!("params_{}", var_info.name), |ui| {
                                            for param in &var_info.parameters {
                                                let mut param_value = transform.get_variation_param_or_default(
                                                    &var_info.name,
                                                    &param.name,
                                                    &crate::variations::global_registry(),
                                                );

                                                let param_changed = match param.param_type {
                                                    crate::variations::ParamType::Float => {
                                                        let min = param.min_value.unwrap_or(-10.0);
                                                        let max = param.max_value.unwrap_or(10.0);
                                                        ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                            .text(&param.display_name)
                                                            .step_by(0.01))
                                                            .changed()
                                                    }
                                                    crate::variations::ParamType::Integer => {
                                                        let min = param.min_value.unwrap_or(1.0) as i32;
                                                        let max = param.max_value.unwrap_or(10.0) as i32;
                                                        let mut int_val = param_value as i32;
                                                        let changed = ui.add(egui::Slider::new(&mut int_val, min..=max)
                                                            .text(&param.display_name))
                                                            .changed();
                                                        param_value = int_val as f32;
                                                        changed
                                                    }
                                                    crate::variations::ParamType::Angle => {
                                                        let min = param.min_value.unwrap_or(0.0);
                                                        let max = param.max_value.unwrap_or(360.0);
                                                        ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                            .text(&param.display_name)
                                                            .suffix("°"))
                                                            .changed()
                                                    }
                                                };

                                                if param_changed {
                                                    transform.set_variation_param(&var_info.name, &param.name, param_value);
                                                    *flame_changed = true;
                                                }
                                            }
                                        });
                                    }
                                }

                                // Show 3D variations only in 3D mode
                                if matches!(flame.render_mode, RenderMode::ThreeD) {
                                    ui.separator();
                                    ui.label("3D Depth Variations");

                                    // Get 3D depth variations from registry
                                    let depth_3d = crate::variations::global_registry().by_category(crate::variations::VariationCategory::Depth3D);
                                    for var_info in depth_3d {
                                        let mut value = transform.get_variation(&var_info.name);
                                        if ui.add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name)).changed() {
                                            transform.set_variation(&var_info.name, value);
                                            *flame_changed = true;
                                        }

                                        // Show parameters if variation is active and has parameters
                                        if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
                                            ui.indent(format!("params_{}", var_info.name), |ui| {
                                                for param in &var_info.parameters {
                                                    let mut param_value = transform.get_variation_param_or_default(
                                                        &var_info.name,
                                                        &param.name,
                                                        &crate::variations::global_registry(),
                                                    );

                                                    let param_changed = match param.param_type {
                                                        crate::variations::ParamType::Float => {
                                                            let min = param.min_value.unwrap_or(-10.0);
                                                            let max = param.max_value.unwrap_or(10.0);
                                                            ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                                .text(&param.display_name)
                                                                .step_by(0.01))
                                                                .changed()
                                                        }
                                                        crate::variations::ParamType::Integer => {
                                                            let min = param.min_value.unwrap_or(1.0) as i32;
                                                            let max = param.max_value.unwrap_or(10.0) as i32;
                                                            let mut int_val = param_value as i32;
                                                            let changed = ui.add(egui::Slider::new(&mut int_val, min..=max)
                                                                .text(&param.display_name))
                                                                .changed();
                                                            param_value = int_val as f32;
                                                            changed
                                                        }
                                                        crate::variations::ParamType::Angle => {
                                                            let min = param.min_value.unwrap_or(0.0);
                                                            let max = param.max_value.unwrap_or(360.0);
                                                            ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                                .text(&param.display_name)
                                                                .suffix("°"))
                                                                .changed()
                                                        }
                                                    };

                                                    if param_changed {
                                                        transform.set_variation_param(&var_info.name, &param.name, param_value);
                                                        *flame_changed = true;
                                                    }
                                                }
                                            });
                                        }
                                    }

                                    ui.separator();
                                    ui.label("3D Rotation Variations");

                                    // Get 3D rotation variations from registry
                                    let rotation_3d = crate::variations::global_registry().by_category(crate::variations::VariationCategory::Rotation3D);
                                    for var_info in rotation_3d {
                                        let mut value = transform.get_variation(&var_info.name);
                                        if ui.add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name)).changed() {
                                            transform.set_variation(&var_info.name, value);
                                            *flame_changed = true;
                                        }

                                        // Show parameters if variation is active and has parameters
                                        if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
                                            ui.indent(format!("params_{}", var_info.name), |ui| {
                                                for param in &var_info.parameters {
                                                    let mut param_value = transform.get_variation_param_or_default(
                                                        &var_info.name,
                                                        &param.name,
                                                        &crate::variations::global_registry(),
                                                    );

                                                    let param_changed = match param.param_type {
                                                        crate::variations::ParamType::Float => {
                                                            let min = param.min_value.unwrap_or(-10.0);
                                                            let max = param.max_value.unwrap_or(10.0);
                                                            ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                                .text(&param.display_name)
                                                                .step_by(0.01))
                                                                .changed()
                                                        }
                                                        crate::variations::ParamType::Integer => {
                                                            let min = param.min_value.unwrap_or(1.0) as i32;
                                                            let max = param.max_value.unwrap_or(10.0) as i32;
                                                            let mut int_val = param_value as i32;
                                                            let changed = ui.add(egui::Slider::new(&mut int_val, min..=max)
                                                                .text(&param.display_name))
                                                                .changed();
                                                            param_value = int_val as f32;
                                                            changed
                                                        }
                                                        crate::variations::ParamType::Angle => {
                                                            let min = param.min_value.unwrap_or(0.0);
                                                            let max = param.max_value.unwrap_or(360.0);
                                                            ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                                .text(&param.display_name)
                                                                .suffix("°"))
                                                                .changed()
                                                        }
                                                    };

                                                    if param_changed {
                                                        transform.set_variation_param(&var_info.name, &param.name, param_value);
                                                        *flame_changed = true;
                                                    }
                                                }
                                            });
                                        }
                                    }

                                    ui.separator();
                                    ui.label("Full 3D Variations");

                                    // Get full 3D variations from registry
                                    let full_3d = crate::variations::global_registry().by_category(crate::variations::VariationCategory::Full3D);
                                    for var_info in full_3d {
                                        let mut value = transform.get_variation(&var_info.name);
                                        if ui.add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name)).changed() {
                                            transform.set_variation(&var_info.name, value);
                                            *flame_changed = true;
                                        }

                                        // Show parameters if variation is active and has parameters
                                        if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
                                            ui.indent(format!("params_{}", var_info.name), |ui| {
                                                for param in &var_info.parameters {
                                                    let mut param_value = transform.get_variation_param_or_default(
                                                        &var_info.name,
                                                        &param.name,
                                                        &crate::variations::global_registry(),
                                                    );

                                                    let param_changed = match param.param_type {
                                                        crate::variations::ParamType::Float => {
                                                            let min = param.min_value.unwrap_or(-10.0);
                                                            let max = param.max_value.unwrap_or(10.0);
                                                            ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                                .text(&param.display_name)
                                                                .step_by(0.01))
                                                                .changed()
                                                        }
                                                        crate::variations::ParamType::Integer => {
                                                            let min = param.min_value.unwrap_or(1.0) as i32;
                                                            let max = param.max_value.unwrap_or(10.0) as i32;
                                                            let mut int_val = param_value as i32;
                                                            let changed = ui.add(egui::Slider::new(&mut int_val, min..=max)
                                                                .text(&param.display_name))
                                                                .changed();
                                                            param_value = int_val as f32;
                                                            changed
                                                        }
                                                        crate::variations::ParamType::Angle => {
                                                            let min = param.min_value.unwrap_or(0.0);
                                                            let max = param.max_value.unwrap_or(360.0);
                                                            ui.add(egui::Slider::new(&mut param_value, min..=max)
                                                                .text(&param.display_name)
                                                                .suffix("°"))
                                                                .changed()
                                                        }
                                                    };

                                                    if param_changed {
                                                        transform.set_variation_param(&var_info.name, &param.name, param_value);
                                                        *flame_changed = true;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }

                                ui.separator();

                                // Delete button (only show if more than 1 transform exists)
                                if num_transforms > 1 {
                                    if ui.button("🗑 Delete Transform").clicked() {
                                        delete_index = Some(i);
                                        *flame_changed = true;
                                    }
                                }
                            });
                    });
                }
            });

            // Set delete_transform if a transform was marked for deletion
            if let Some(idx) = delete_index {
                *delete_transform = Some(idx);
            }
        });
}
