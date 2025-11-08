use crate::scene::transforms::{Flame, RenderMode};
use crate::variations::VariationCategory;
use crate::config::{ConfigManager, ConfigPath, UpdateType, AffineParam};
use super::variation_controls::render_variation_category;
use std::collections::HashMap;

/// Color edit mode for transform color UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorEditMode {
    /// Edit palette position directly (0-1 slider)
    PalettePosition,
    /// Pick RGB color, find closest palette position
    ColorPicker,
}

impl Default for ColorEditMode {
    fn default() -> Self {
        Self::PalettePosition
    }
}

/// UI state for transform color editing (per-transform)
/// This is UI-only state and is not serialized
static mut COLOR_EDIT_MODES: Option<HashMap<usize, ColorEditMode>> = None;

fn get_color_edit_mode(transform_index: usize) -> ColorEditMode {
    unsafe {
        COLOR_EDIT_MODES
            .get_or_insert_with(HashMap::new)
            .get(&transform_index)
            .copied()
            .unwrap_or_default()
    }
}

fn set_color_edit_mode(transform_index: usize, mode: ColorEditMode) {
    unsafe {
        COLOR_EDIT_MODES
            .get_or_insert_with(HashMap::new)
            .insert(transform_index, mode);
    }
}

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

/// Render color controls (color, color speed, blend, opacity)
fn render_color_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Mode toggle button
    let current_mode = get_color_edit_mode(index);
    let mode_label = match current_mode {
        ColorEditMode::PalettePosition => "📍 Palette Position",
        ColorEditMode::ColorPicker => "🎨 Color Picker",
    };

    ui.horizontal(|ui| {
        if ui.button(mode_label).clicked() {
            let new_mode = match current_mode {
                ColorEditMode::PalettePosition => ColorEditMode::ColorPicker,
                ColorEditMode::ColorPicker => ColorEditMode::PalettePosition,
            };
            set_color_edit_mode(index, new_mode);
        }
        ui.label("(click to toggle)");
    });

    // Render UI based on mode (both store RGB directly now!)
    match current_mode {
        ColorEditMode::PalettePosition => {
            // Palette Position mode: Sample palette → store RGB
            if let Some(palette) = &config_manager.active_config().palette {
                // Find current palette position from RGB
                let current_pos = palette.find_position(transform.color);
                let mut temp_pos = current_pos;

                let response = ui.add(egui::Slider::new(&mut temp_pos, 0.0..=1.0).text("Palette Position"));

                if response.changed() {
                    // Sample palette to get RGB
                    let new_rgb = palette.sample_color(temp_pos);

                    // Update all RGB components via batch
                    let changes = vec![
                        (ConfigPath::TransformColor { index, component: crate::config::ColorComponent::R }, new_rgb[0].into()),
                        (ConfigPath::TransformColor { index, component: crate::config::ColorComponent::G }, new_rgb[1].into()),
                        (ConfigPath::TransformColor { index, component: crate::config::ColorComponent::B }, new_rgb[2].into()),
                    ];

                    if let Ok(update_type) = config_manager.update_batch(
                        changes,
                        format!("Transform {} Color", index + 1),
                        response.dragged()
                    ) {
                        transform.color = config_manager.active_config().flame.transforms[index].color;
                        max_update = max_update.max(update_type);
                    }
                }

                if response.drag_stopped() && config_manager.is_in_preview_mode() {
                    let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: crate::config::ColorComponent::R });
                }
            } else {
                ui.label("No palette loaded");
            }
        }
        ColorEditMode::ColorPicker => {
            // Color Picker mode: Pick RGB directly → store RGB
            ui.label("Pick a color:");

            // R slider
            let mut temp_r = transform.color[0];
            ui.horizontal(|ui| {
                ui.label("R:");
                let response_r = ui.add(egui::Slider::new(&mut temp_r, 0.0..=1.0).fixed_decimals(3));
                if response_r.changed() {
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::TransformColor { index, component: crate::config::ColorComponent::R },
                        temp_r.into(),
                        response_r.dragged()
                    ) {
                        transform.color[0] = config_manager.active_config().flame.transforms[index].color[0];
                        max_update = max_update.max(update_type);
                    }
                }
                if response_r.drag_stopped() && config_manager.is_in_preview_mode() {
                    let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: crate::config::ColorComponent::R });
                }
            });

            // G slider
            let mut temp_g = transform.color[1];
            ui.horizontal(|ui| {
                ui.label("G:");
                let response_g = ui.add(egui::Slider::new(&mut temp_g, 0.0..=1.0).fixed_decimals(3));
                if response_g.changed() {
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::TransformColor { index, component: crate::config::ColorComponent::G },
                        temp_g.into(),
                        response_g.dragged()
                    ) {
                        transform.color[1] = config_manager.active_config().flame.transforms[index].color[1];
                        max_update = max_update.max(update_type);
                    }
                }
                if response_g.drag_stopped() && config_manager.is_in_preview_mode() {
                    let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: crate::config::ColorComponent::G });
                }
            });

            // B slider
            let mut temp_b = transform.color[2];
            ui.horizontal(|ui| {
                ui.label("B:");
                let response_b = ui.add(egui::Slider::new(&mut temp_b, 0.0..=1.0).fixed_decimals(3));
                if response_b.changed() {
                    if let Ok(update_type) = config_manager.update_param(
                        ConfigPath::TransformColor { index, component: crate::config::ColorComponent::B },
                        temp_b.into(),
                        response_b.dragged()
                    ) {
                        transform.color[2] = config_manager.active_config().flame.transforms[index].color[2];
                        max_update = max_update.max(update_type);
                    }
                }
                if response_b.drag_stopped() && config_manager.is_in_preview_mode() {
                    let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: crate::config::ColorComponent::B });
                }
            });

            // Color preview button (for quick visual reference)
            let mut temp_rgb = transform.color;
            let response_picker = egui::color_picker::color_edit_button_rgb(ui, &mut temp_rgb);

            // The color picker popup is open while being interacted with
            // Treat all changes as preview mode until the popup closes (detected via changed() stopping)
            if response_picker.changed() && temp_rgb != transform.color {
                let changes = vec![
                    (ConfigPath::TransformColor { index, component: crate::config::ColorComponent::R }, temp_rgb[0].into()),
                    (ConfigPath::TransformColor { index, component: crate::config::ColorComponent::G }, temp_rgb[1].into()),
                    (ConfigPath::TransformColor { index, component: crate::config::ColorComponent::B }, temp_rgb[2].into()),
                ];

                // Use lazy=true for preview mode (color picker is always "dragging" when popup is open)
                if let Ok(update_type) = config_manager.update_batch(
                    changes,
                    format!("Transform {} Color", index + 1),
                    true  // Preview mode while color picker popup is open
                ) {
                    transform.color = config_manager.active_config().flame.transforms[index].color;
                    max_update = max_update.max(update_type);
                }
            }
            // When interaction ends (popup closes), commit the preview
            // Note: We check on_hover_text_at_pointer() or has_focus() to detect popup state,
            // but simpler approach: always commit if we're in preview mode and the response didn't change this frame
            else if !response_picker.changed() && config_manager.is_in_preview_mode() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index, component: crate::config::ColorComponent::R });
            }
        }
    }

    // Show color swatch (both modes)
    ui.horizontal(|ui| {
        ui.label("Color:");
        let mut color_swatch = egui::Color32::from_rgb(
            (transform.color[0] * 255.0) as u8,
            (transform.color[1] * 255.0) as u8,
            (transform.color[2] * 255.0) as u8,
        );
        ui.color_edit_button_srgba(&mut color_swatch);
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

    // Color Blend slider (how much transform color is applied)
    let mut temp_blend = transform.color_blend;
    let response_blend = ui.add(egui::Slider::new(&mut temp_blend, 0.0..=1.0).text("Color Blend"));
    if response_blend.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformColorBlend { index },
            temp_blend.into(),
            response_blend.dragged()  // Lazy undo
        ) {
            transform.color_blend = config_manager.active_config().flame.transforms[index].color_blend;
            max_update = max_update.max(update_type);
        }
    }
    if response_blend.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformColorBlend { index });
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
