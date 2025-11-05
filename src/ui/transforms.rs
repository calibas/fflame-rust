use crate::scene::transforms::{Flame, RenderMode};
use crate::variations::VariationCategory;
use crate::config::{ConfigManager, ConfigPath, UpdateType, AffineParam};
use super::variation_controls::render_variation_category;

/// Render the Transforms window with transform editing controls
///
/// Note: Config change tracking is now handled by ConfigManager.get_pending_actions()
pub fn render_transforms_window(
    ctx: &egui::Context,
    show_transforms: &mut bool,
    config_manager: &mut ConfigManager,
    flame: &mut Flame,
    add_transform: &mut bool,
    delete_transform: &mut Option<usize>,
) -> UpdateType {
    let mut max_update = UpdateType::None;
    egui::Window::new("Transforms")
        .open(show_transforms)
        .show(ctx, |ui| {
            ui.heading(format!("Transforms ({})", flame.transforms.len()));

            // Add/Delete transform buttons
            ui.horizontal(|ui| {
                if ui.button("➕ Add Transform").clicked() {
                    *add_transform = true;
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
                                                temp_g.into(),
                                                true  // Lazy undo
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
                                        temp_weight.into(),
                                        response.dragged()  // Lazy undo
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
            });

            // Set delete_transform if a transform was marked for deletion
            if let Some(idx) = delete_index {
                *delete_transform = Some(idx);
            }
        });

    max_update
}

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
                temp_a.into(),
                response_a.dragged()  // Lazy undo
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
                temp_b.into(),
                response_b.dragged()  // Lazy undo
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
                temp_c.into(),
                response_c.dragged()  // Lazy undo
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
                temp_d.into(),
                response_d.dragged()  // Lazy undo
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
                temp_e.into(),
                response_e.dragged()  // Lazy undo
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
                temp_f.into(),
                response_f.dragged()  // Lazy undo
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

/// Render color controls (RGB color and color speed)
fn render_color_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    use crate::config::ColorComponent;
    let mut max_update = UpdateType::None;

    ui.label("Color");
    ui.horizontal(|ui| {
        ui.label("R:");
        let mut temp_r = transform.color[0];
        let response_r = ui.add(egui::Slider::new(&mut temp_r, 0.0..=1.0));
        if response_r.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformColor { index, component: ColorComponent::R },
                temp_r.into(),
                response_r.dragged()  // Lazy undo
            ) {
                transform.color[0] = config_manager.active_config().flame.transforms[index].color[0];
                max_update = max_update.max(update_type);
            }
        }
        if response_r.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: ColorComponent::R });
        }

        ui.label("G:");
        let mut temp_g = transform.color[1];
        let response_g = ui.add(egui::Slider::new(&mut temp_g, 0.0..=1.0));
        if response_g.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformColor { index, component: ColorComponent::G },
                temp_g.into(),
                response_g.dragged()  // Lazy undo
            ) {
                transform.color[1] = config_manager.active_config().flame.transforms[index].color[1];
                max_update = max_update.max(update_type);
            }
        }
        if response_g.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: ColorComponent::G });
        }

        ui.label("B:");
        let mut temp_b = transform.color[2];
        let response_b = ui.add(egui::Slider::new(&mut temp_b, 0.0..=1.0));
        if response_b.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformColor { index, component: ColorComponent::B },
                temp_b.into(),
                response_b.dragged()  // Lazy undo
            ) {
                transform.color[2] = config_manager.active_config().flame.transforms[index].color[2];
                max_update = max_update.max(update_type);
            }
        }
        if response_b.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: ColorComponent::B });
        }
    });

    let mut temp_speed = transform.color_speed;
    let response_speed = ui.add(egui::Slider::new(&mut temp_speed, -1.0..=1.0).text("Color Speed (Symmetry)"));
    if response_speed.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformColorSpeed { index },
            temp_speed.into(),
            response_speed.dragged()  // Lazy undo
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
            temp_opacity.into(),
            response_opacity.dragged()  // Lazy undo
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
