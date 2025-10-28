use crate::scene::transforms::{Flame, RenderMode};
use crate::variations::VariationCategory;
use super::variation_controls::render_variation_category;

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
                                // Affine Matrix controls
                                render_affine_controls(ui, transform, flame_changed);

                                // Z offset (only in 3D mode)
                                if matches!(flame.render_mode, RenderMode::ThreeD) {
                                    ui.horizontal(|ui| {
                                        ui.label("g (Z offset):");
                                        if ui.add(egui::DragValue::new(&mut transform.g).speed(0.01)).changed() {
                                            *flame_changed = true;
                                        }
                                    });
                                }

                                ui.separator();

                                // Weight control
                                ui.label("Weight");
                                if ui.add(egui::Slider::new(&mut transform.weight, 0.0..=1024.0).logarithmic(true)).changed() {
                                    *flame_changed = true;
                                }

                                ui.separator();

                                // Color controls
                                render_color_controls(ui, transform, flame_changed);

                                // Variation controls by category
                                render_variation_category(ui, transform, VariationCategory::Basic2D, "Basic 2D Variations", flame_changed);
                                render_variation_category(ui, transform, VariationCategory::Advanced2D, "Advanced 2D Variations", flame_changed);

                                // 3D variation categories (only visible in 3D mode)
                                if matches!(flame.render_mode, RenderMode::ThreeD) {
                                    render_variation_category(ui, transform, VariationCategory::Depth3D, "3D Depth Variations", flame_changed);
                                    render_variation_category(ui, transform, VariationCategory::Rotation3D, "3D Rotation Variations", flame_changed);
                                    render_variation_category(ui, transform, VariationCategory::Full3D, "Full 3D Variations", flame_changed);
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

/// Render affine matrix controls (a, b, c, d, e, f)
fn render_affine_controls(ui: &mut egui::Ui, transform: &mut crate::scene::transforms::Transform, flame_changed: &mut bool) {
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
}

/// Render color controls (RGB color and color speed)
fn render_color_controls(ui: &mut egui::Ui, transform: &mut crate::scene::transforms::Transform, flame_changed: &mut bool) {
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
}
