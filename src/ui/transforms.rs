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
                        egui::CollapsingHeader::new(format!("Transform {}", i))
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
                                if ui.add(egui::Slider::new(&mut transform.weight, 0.0..=2.0)).changed() {
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
                                ui.label("2D Variations");

                                let variation_names_2d = [
                                    "Linear", "Sinusoidal", "Spherical", "Swirl",
                                    "Horseshoe", "Polar", "Handkerchief", "Heart",
                                    "Disc", "Spiral", "Hyperbolic", "Diamond",
                                    "Ex", "Julia", "Bent", "Waves"
                                ];

                                for (idx, name) in variation_names_2d.iter().enumerate() {
                                    if ui.add(egui::Slider::new(&mut transform.variations[idx], 0.0..=2.0).text(*name)).changed() {
                                        *flame_changed = true;
                                    }
                                }

                                // Show 3D variations only in 3D mode
                                if matches!(flame.render_mode, RenderMode::ThreeD) {
                                    ui.separator();
                                    ui.label("3D Variations");

                                    let variation_names_3d = [
                                        "Zcone", "Flatten", "Hemisphere",
                                        "PreRotateX", "PreRotateY", "PostRotateX", "PostRotateY",
                                        "ZScale"
                                    ];

                                    for (i, name) in variation_names_3d.iter().enumerate() {
                                        let idx = 16 + i; // 3D variations start at index 16
                                        if ui.add(egui::Slider::new(&mut transform.variations[idx], 0.0..=2.0).text(*name)).changed() {
                                            *flame_changed = true;
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
