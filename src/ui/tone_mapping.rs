use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::palette::{ColorMode, PaletteLibrary};
use crate::ui::{LazyUndoHelper, LazyUndoUi};

/// Render the Tone Mapping window with all tone mapping and color controls
#[allow(clippy::too_many_arguments)]
pub fn render_tone_mapping_window(
    ctx: &egui::Context,
    show_tone_mapping: &mut bool,
    show_palette_editor: &mut bool,
    tonemap_mode: &mut ToneMapMode,
    tonemap_mode_changed: &mut bool,
    tonemap_curve: &mut ToneCurve,
    tonemap_curve_changed: &mut bool,
    use_curve: &mut bool,
    use_curve_changed: &mut bool,
    exposure: &mut f32,
    exposure_changed: &mut bool,
    gamma: &mut f32,
    gamma_changed: &mut bool,
    density_scale: &mut f32,
    density_changed: &mut bool,
    color_mode: &mut ColorMode,
    color_mode_changed: &mut bool,
    palette_library: &PaletteLibrary,
    current_palette_index: &mut usize,
    palette_changed: &mut bool,
    palette_editor_palette: &mut crate::scene::palette::Palette,
    palette_editor_has_changes: &mut bool,
    speed_factor: &mut f32,
    background_color: &mut [f32; 3],
    background_color_changed: &mut bool,
    lazy_undo_sliders: &mut LazyUndoHelper,
    lazy_undo_curve: &mut LazyUndoHelper,
) {
    egui::Window::new("Tone Mapping & Colors")
        .open(show_tone_mapping)
        .show(ctx, |ui| {
            // Section 1: Tone Mapping
            egui::CollapsingHeader::new("Tone Mapping")
                .default_open(true)
                .show(ui, |ui| {
                    ui.label("Tone Map Mode");
                    ui.horizontal(|ui| {
                        if ui.selectable_label(matches!(*tonemap_mode, ToneMapMode::Linear), "Linear").clicked() {
                            *tonemap_mode = ToneMapMode::Linear;
                            *tonemap_mode_changed = true;
                        }
                        if ui.selectable_label(matches!(*tonemap_mode, ToneMapMode::Logarithmic), "Logarithmic").clicked() {
                            *tonemap_mode = ToneMapMode::Logarithmic;
                            *tonemap_mode_changed = true;
                        }
                        if ui.selectable_label(matches!(*tonemap_mode, ToneMapMode::DensityVisualization), "Density").clicked() {
                            *tonemap_mode = ToneMapMode::DensityVisualization;
                            *tonemap_mode_changed = true;
                        }
                    });

                    ui.separator();

                    // Use lazy undo for all sliders to throttle undo captures
                    let result = ui.lazy_slider(lazy_undo_sliders, exposure, 0.1..=5.0, "Exposure");
                    if result.should_capture {
                        *exposure_changed = true;
                    }

                    let result = ui.lazy_slider(lazy_undo_sliders, gamma, 1.0..=3.0, "Gamma");
                    if result.should_capture {
                        *gamma_changed = true;
                    }

                    let result = ui.lazy_slider(lazy_undo_sliders, density_scale, 0.01..=10.0, "Density Scale");
                    if result.should_capture {
                        *density_changed = true;
                    }
                });

            // Section 2: Tone Curve
            egui::CollapsingHeader::new("Tone Curve")
                .default_open(false)
                .show(ui, |ui| {
                    // Enable/disable curve
                    if ui.checkbox(use_curve, "Enable Tone Curve").changed() {
                        *use_curve_changed = true;
                    }

                    ui.add_enabled_ui(*use_curve, |ui| {
                        // Preset curves
                        ui.label("Presets");
                        ui.horizontal(|ui| {
                            if ui.button("Linear").clicked() {
                                *tonemap_curve = ToneCurve::linear();
                                *tonemap_curve_changed = true;
                            }
                            if ui.button("S-Curve").clicked() {
                                *tonemap_curve = ToneCurve::s_curve();
                                *tonemap_curve_changed = true;
                            }
                            if ui.button("Brighten Shadows").clicked() {
                                *tonemap_curve = ToneCurve::brighten_shadows();
                                *tonemap_curve_changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Darken Highlights").clicked() {
                                *tonemap_curve = ToneCurve::darken_highlights();
                                *tonemap_curve_changed = true;
                            }
                        });

                        ui.separator();

                        // Curve editor
                        render_curve_editor(ui, tonemap_curve, tonemap_curve_changed, lazy_undo_curve);
                    });
                });

            // Section 3: Color & Appearance
            egui::CollapsingHeader::new("Color & Appearance")
                .default_open(true)
                .show(ui, |ui| {
                    let current_mode = *color_mode;
                    let selected_text = match current_mode {
                        ColorMode::Transform => "Transform Colors",
                        ColorMode::Palette => "Palette",
                        ColorMode::Speed => "Speed",
                    };

                    egui::ComboBox::from_label("Color Mode")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(color_mode, ColorMode::Transform, "Transform Colors").changed() {
                                *color_mode_changed = true;
                            }
                            if ui.selectable_value(color_mode, ColorMode::Palette, "Palette").changed() {
                                *color_mode_changed = true;
                            }
                            if ui.selectable_value(color_mode, ColorMode::Speed, "Speed").changed() {
                                *color_mode_changed = true;
                            }
                        });

                    // Show palette selector for Palette and Speed modes
                    if matches!(*color_mode, ColorMode::Palette | ColorMode::Speed) {
                        let palettes = palette_library.palettes();
                        let current_palette_name = palettes.get(*current_palette_index)
                            .map(|p| p.name.as_str())
                            .unwrap_or("Unknown");

                        // Use ID with palette count to force refresh when library changes
                        egui::ComboBox::from_id_source(format!("palette_selector_{}", palettes.len()))
                            .selected_text(current_palette_name)
                            .show_ui(ui, |ui| {
                                ui.label("Palette");
                                for (idx, palette) in palettes.iter().enumerate() {
                                    if ui.selectable_value(current_palette_index, idx, &palette.name).changed() {
                                        *palette_changed = true;
                                        // Update palette editor when selection changes
                                        if let Some(pal) = palette_library.get(*current_palette_index) {
                                            let mut edited_palette = pal.clone();

                                            // Generate unique name for built-in palettes even when switching
                                            if pal.built_in {
                                                let base_name = &pal.name;
                                                let mut new_name = format!("{} (Custom)", base_name);
                                                let mut counter = 2;

                                                while palette_library.palettes().iter().any(|p| p.name == new_name) {
                                                    new_name = format!("{} (Custom {})", base_name, counter);
                                                    counter += 1;
                                                }

                                                edited_palette.name = new_name;
                                                edited_palette.built_in = false;
                                            }

                                            *palette_editor_palette = edited_palette;
                                            *palette_editor_has_changes = true; // New palette needs to be applied
                                        }
                                    }
                                }
                            });

                        ui.horizontal(|ui| {
                            // Edit Palette button - creates copy of built-ins, edits custom palettes in-place
                            if ui.button("🎨 Edit Palette").clicked() {
                                *show_palette_editor = !*show_palette_editor;
                                // Load current palette into editor
                                if let Some(pal) = palette_library.get(*current_palette_index) {
                                    let mut edited_palette = pal.clone();

                                    // Always generate unique name for built-in palettes to prevent editing originals
                                    if pal.built_in {
                                        let base_name = &pal.name;
                                        let mut new_name = format!("{} (Custom)", base_name);
                                        let mut counter = 2;

                                        // Keep incrementing until we find a unique name
                                        while palette_library.palettes().iter().any(|p| p.name == new_name) {
                                            new_name = format!("{} (Custom {})", base_name, counter);
                                            counter += 1;
                                        }

                                        edited_palette.name = new_name;
                                        edited_palette.built_in = false; // Custom palettes are not built-in
                                    }
                                    // For custom palettes, keep the same name (will update in place)

                                    *palette_editor_palette = edited_palette;
                                    *palette_editor_has_changes = true; // New palette needs to be applied
                                }
                            }

                            // Clone button - always creates a copy with unique name
                            if ui.button("📋 Clone").clicked() {
                                *show_palette_editor = !*show_palette_editor;
                                if let Some(pal) = palette_library.get(*current_palette_index) {
                                    let mut edited_palette = pal.clone();
                                    edited_palette.built_in = false; // Clones are never built-in

                                    let base_name = &pal.name;
                                    let mut new_name = format!("{} (Copy)", base_name);
                                    let mut counter = 2;

                                    // Keep incrementing until we find a unique name
                                    while palette_library.palettes().iter().any(|p| p.name == new_name) {
                                        new_name = format!("{} (Copy {})", base_name, counter);
                                        counter += 1;
                                    }

                                    edited_palette.name = new_name;
                                    *palette_editor_palette = edited_palette;
                                    *palette_editor_has_changes = true; // New palette needs to be applied
                                }
                            }
                        });
                    }

                    // Show speed factor slider in Speed mode
                    if matches!(*color_mode, ColorMode::Speed) {
                        let result = ui.lazy_slider(lazy_undo_sliders, speed_factor, 0.0..=1.0, "Speed Blend Factor");
                        if result.should_capture {
                            *color_mode_changed = true;
                        }
                    }

                    ui.separator();

                    // Background color picker
                    ui.label("Background Color");
                    if ui.color_edit_button_rgb(background_color).changed() {
                        *background_color_changed = true;
                    }
                });
        });
}

/// Render curve editor UI
fn render_curve_editor(
    ui: &mut egui::Ui,
    curve: &mut ToneCurve,
    curve_changed: &mut bool,
    lazy_undo: &mut LazyUndoHelper,
) {
    ui.label("Curve Editor");

    // Plot the curve
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), 200.0),
        egui::Sense::click_and_drag(),
    );

    let rect = response.rect;
    let to_screen = |x: f32, y: f32| -> egui::Pos2 {
        egui::pos2(
            rect.left() + x * rect.width(),
            rect.bottom() - y * rect.height(),
        )
    };

    let from_screen = |pos: egui::Pos2| -> (f32, f32) {
        (
            ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
            ((rect.bottom() - pos.y) / rect.height()).clamp(0.0, 1.0),
        )
    };

    // Draw background
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(20));

    // Draw grid
    for i in 0..=4 {
        let t = i as f32 / 4.0;
        painter.line_segment(
            [to_screen(t, 0.0), to_screen(t, 1.0)],
            egui::Stroke::new(1.0, egui::Color32::from_gray(40)),
        );
        painter.line_segment(
            [to_screen(0.0, t), to_screen(1.0, t)],
            egui::Stroke::new(1.0, egui::Color32::from_gray(40)),
        );
    }

    // Draw diagonal reference line (y = x)
    painter.line_segment(
        [to_screen(0.0, 0.0), to_screen(1.0, 1.0)],
        egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
    );

    // Draw smooth curve (use more samples for cubic interpolation)
    let num_samples = 200;
    let mut points: Vec<egui::Pos2> = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let x = i as f32 / (num_samples - 1) as f32;
        let y = curve.evaluate(x);
        points.push(to_screen(x, y));
    }
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255)),
    ));

    // Persistent drag state (tracks which point is being dragged across frames)
    let mut dragging_point = ui.ctx().data_mut(|d| {
        d.get_persisted::<Option<usize>>(egui::Id::new("curve_editor_dragging_point"))
            .unwrap_or(None)
    });

    // Check if mouse button is down globally
    let mouse_down = ui.ctx().input(|i| i.pointer.primary_down());

    // Draw control points
    for (i, point) in curve.points.iter().enumerate() {
        let screen_pos = to_screen(point.x, point.y);
        let point_radius = 6.0;

        // Check if mouse is over this point (check both hover and interact positions for fast drags)
        let is_hovered = if let Some(hover_pos) = response.hover_pos() {
            hover_pos.distance(screen_pos) < point_radius * 2.0
        } else {
            false
        };

        // Also check interact position for fast drags where hover might lag
        let is_clicked = if let Some(interact_pos) = response.interact_pointer_pos() {
            interact_pos.distance(screen_pos) < point_radius * 2.0
        } else {
            false
        };

        let point_color = if is_hovered || is_clicked || dragging_point == Some(i) {
            egui::Color32::from_rgb(255, 200, 100)
        } else if i == 0 || i == curve.points.len() - 1 {
            egui::Color32::from_rgb(200, 100, 100) // Endpoints in red
        } else {
            egui::Color32::from_rgb(255, 255, 100)
        };

        painter.circle_filled(screen_pos, point_radius, point_color);
        painter.circle_stroke(screen_pos, point_radius, egui::Stroke::new(1.0, egui::Color32::WHITE));

        // Start dragging if clicking on a point (check both hover and interact for fast drags)
        if dragging_point.is_none() && response.dragged() && (is_hovered || is_clicked) {
            dragging_point = Some(i);
        }
    }

    // Update dragged point (use global pointer position)
    if let Some(idx) = dragging_point {
        if let Some(drag_pos) = ui.ctx().pointer_latest_pos() {
            let (new_x, new_y) = from_screen(drag_pos);
            curve.move_point(idx, new_x, new_y);

            // Use lazy undo to throttle captures during drag
            if lazy_undo.should_capture_for_drag_state(true) {
                *curve_changed = true;
            }
        }
    }

    // Clear drag on mouse release
    if !mouse_down && dragging_point.is_some() {
        // Capture undo on drag end
        if lazy_undo.should_capture_for_drag_state(false) {
            *curve_changed = true;
        }
        dragging_point = None;
    }

    // Persist drag state
    ui.ctx().data_mut(|d| {
        d.insert_persisted(egui::Id::new("curve_editor_dragging_point"), dragging_point);
    });

    // Add point on double-click
    if response.double_clicked() {
        if let Some(click_pos) = response.interact_pointer_pos() {
            let (x, y) = from_screen(click_pos);
            curve.add_point(crate::scene::tonemap::CurvePoint::new(x, y));
            *curve_changed = true;
        }
    }

    // Show instructions
    ui.label("Double-click to add points, drag to move, Ctrl+click to remove");

    // List control points
    ui.label(format!("{} control points", curve.points.len()));
}
