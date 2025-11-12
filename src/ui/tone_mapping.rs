use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::palette::{ColorMode, PaletteLibrary};
use crate::config::{ConfigManager, ConfigPath, LazyUndoUi, UpdateType};

/// Render curve editor UI with ConfigManager integration
fn render_curve_editor(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
) -> UpdateType {
    ui.label("Curve Editor");
    let mut max_update = UpdateType::None;

    // Clone curve to avoid borrow conflicts
    let curve = config_manager.active_config().tonemap_curve.clone();

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

            // Create modified curve
            let mut modified_curve = config_manager.active_config().tonemap_curve.clone();
            modified_curve.move_point(idx, new_x, new_y);

            // Update via ConfigManager with lazy mode (throttled)
            if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, modified_curve.into(), true) {
                max_update = max_update.max(update);
            }
        }
    }

    // Clear drag on mouse release and force commit preview
    if !mouse_down && dragging_point.is_some() {
        dragging_point = None;

        // Exit preview mode immediately on drag end
        if config_manager.is_in_preview_mode() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TonemapCurve);
        }
    }

    // Persist drag state
    ui.ctx().data_mut(|d| {
        d.insert_persisted(egui::Id::new("curve_editor_dragging_point"), dragging_point);
    });

    // Add point on double-click
    if response.double_clicked() {
        if let Some(click_pos) = response.interact_pointer_pos() {
            let (x, y) = from_screen(click_pos);

            // Create modified curve with new point
            let mut modified_curve = config_manager.active_config().tonemap_curve.clone();
            modified_curve.add_point(crate::scene::tonemap::CurvePoint::new(x, y));

            // Update via ConfigManager (not lazy - immediate capture for discrete action)
            if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, modified_curve.into(), false) {
                max_update = max_update.max(update);
            }
        }
    }

    // Show instructions
    ui.label("Double-click to add points, drag to move, Ctrl+click to remove");

    // List control points
    ui.label(format!("{} control points", curve.points.len()));

    max_update
}

/// Render the Colors panel content (tone mapping, color mode, palette)
///
/// This is the panel version without the Window wrapper.
pub fn render_colors_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    palette_library: &PaletteLibrary,
    custom_palette: &mut Option<crate::scene::palette::Palette>,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Section 1: Tone Mapping
    egui::CollapsingHeader::new("Tone Mapping")
        .default_open(true)
        .show(ui, |ui| {
            ui.label("Tone Map Mode");
            let current_tonemap_mode = config_manager.active_config().tonemap_mode;
            ui.horizontal(|ui| {
                if ui.selectable_label(matches!(current_tonemap_mode, ToneMapMode::Linear), "Linear").clicked() {
                    if let Ok(update) = config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::Linear.into(), false) {
                        max_update = max_update.max(update);
                    }
                }
                if ui.selectable_label(matches!(current_tonemap_mode, ToneMapMode::Logarithmic), "Logarithmic").clicked() {
                    if let Ok(update) = config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::Logarithmic.into(), false) {
                        max_update = max_update.max(update);
                    }
                }
                if ui.selectable_label(matches!(current_tonemap_mode, ToneMapMode::DensityVisualization), "Density").clicked() {
                    if let Ok(update) = config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::DensityVisualization.into(), false) {
                        max_update = max_update.max(update);
                    }
                }
            });

            ui.separator();

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Exposure, 0.01..=10.0, "Exposure") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Gamma, -1.0..=10.0, "Gamma") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::GammaThreshold, 0.0..=1000.0, "Gamma Threshold") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Brightness, 0.001..=100.0, "Brightness") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Vibrancy, 0.0..=30.0, "Vibrancy") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Saturation, 0.0..=3.0, "Saturation") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::HueShift, -360.0..=360.0, "Hue Shift") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::ValueScale, 0.0..=3.0, "Value Scale") {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::DensityScale, 0.01..=10.0, "Density Scale") {
                max_update = max_update.max(result.update_type);
            }
        });

    // Section 2: Tone Curve
    egui::CollapsingHeader::new("Tone Curve")
        .default_open(false)
        .show(ui, |ui| {
            let mut temp_use_curve = config_manager.active_config().use_curve;
            if ui.checkbox(&mut temp_use_curve, "Enable Tone Curve").changed() {
                if let Ok(update) = config_manager.update_param(ConfigPath::UseCurve, temp_use_curve.into(), false) {
                    max_update = max_update.max(update);
                }
            }

            let current_use_curve = config_manager.active_config().use_curve;
            ui.add_enabled_ui(current_use_curve, |ui| {
                ui.label("Presets");
                ui.horizontal(|ui| {
                    if ui.button("Linear").clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::linear().into(), false) {
                            max_update = max_update.max(update);
                        }
                    }
                    if ui.button("S-Curve").clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::s_curve().into(), false) {
                            max_update = max_update.max(update);
                        }
                    }
                    if ui.button("Brighten Shadows").clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::brighten_shadows().into(), false) {
                            max_update = max_update.max(update);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Darken Highlights").clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::darken_highlights().into(), false) {
                            max_update = max_update.max(update);
                        }
                    }
                });

                ui.separator();

                let curve_update = render_curve_editor(ui, config_manager);
                max_update = max_update.max(curve_update);
            });
        });

    // Section 3: Color & Appearance
    egui::CollapsingHeader::new("Color & Appearance")
        .default_open(true)
        .show(ui, |ui| {
            let current_mode = config_manager.active_config().color_mode;
            let selected_text = match current_mode {
                ColorMode::Palette => "Palette",
                ColorMode::Speed => "Speed",
            };

            let mut temp_color_mode = current_mode;
            egui::ComboBox::from_label("Color Mode")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut temp_color_mode, ColorMode::Palette, "Palette").changed() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::ColorMode, temp_color_mode.into(), false) {
                            max_update = max_update.max(update);
                        }
                    }
                    if ui.selectable_value(&mut temp_color_mode, ColorMode::Speed, "Speed").changed() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::ColorMode, temp_color_mode.into(), false) {
                            max_update = max_update.max(update);
                        }
                    }
                });

            let current_color_mode = config_manager.active_config().color_mode;
            if matches!(current_color_mode, ColorMode::Palette | ColorMode::Speed) {
                let palettes = palette_library.palettes();

                let current_palette = config_manager.active_config().palette.clone();
                let current_palette_name = current_palette
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "(None)".to_string());

                egui::ComboBox::from_id_salt(format!("palette_selector_{}", palettes.len()))
                    .selected_text(&current_palette_name)
                    .show_ui(ui, |ui| {
                        ui.label("Palette");

                        for palette in palettes.iter() {
                            let is_selected = current_palette.as_ref().map(|p| &p.name) == Some(&palette.name);
                            if ui.selectable_label(is_selected, &palette.name).clicked() {
                                let mut palette_copy = palette.clone();

                                if palette.built_in {
                                    let base_name = &palette.name;
                                    let mut new_name = format!("{} (Custom)", base_name);
                                    let mut counter = 2;

                                    while palette_library.palettes().iter().any(|p| p.name == new_name)
                                        || (current_palette.as_ref().map(|p| &p.name) == Some(&new_name)) {
                                        new_name = format!("{} (Custom {})", base_name, counter);
                                        counter += 1;
                                    }

                                    palette_copy.name = new_name;
                                }

                                palette_copy.built_in = false;

                                if let Ok(update) = config_manager.update_param(
                                    ConfigPath::Palette,
                                    palette_copy.clone().into(),
                                    false
                                ) {
                                    max_update = max_update.max(update);
                                }

                                *custom_palette = Some(palette_copy);
                            }
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.button("🎨 Edit Palette").clicked() {
                        // Palette Editor is now accessible via Windows menu
                        // *show_palette_editor = !*show_palette_editor;

                        if current_palette.is_none() {
                            let palette_index = config_manager.active_config().palette_index;
                            if let Some(lib_pal) = palette_library.get(palette_index) {
                                let mut pal = lib_pal.clone();

                                if lib_pal.built_in {
                                    let base_name = &lib_pal.name;
                                    let mut new_name = format!("{} (Custom)", base_name);
                                    let mut counter = 2;

                                    while palette_library.palettes().iter().any(|p| p.name == new_name) {
                                        new_name = format!("{} (Custom {})", base_name, counter);
                                        counter += 1;
                                    }

                                    pal.name = new_name;
                                }

                                pal.built_in = false;

                                let _ = config_manager.update_param(
                                    ConfigPath::Palette,
                                    pal.into(),
                                    false
                                );
                            }
                        }
                    }

                    if ui.button("📋 Clone").clicked() {
                        if let Some(pal) = &current_palette {
                            let mut cloned_palette = pal.clone();
                            let base_name = &pal.name;
                            let mut new_name = format!("{} (Copy)", base_name);
                            let mut counter = 2;

                            while palette_library.palettes().iter().any(|p| p.name == new_name)
                                || (current_palette.as_ref().map(|p| &p.name) == Some(&new_name)) {
                                new_name = format!("{} (Copy {})", base_name, counter);
                                counter += 1;
                            }

                            cloned_palette.name = new_name;
                            cloned_palette.built_in = false;

                            if let Ok(update) = config_manager.update_param(
                                ConfigPath::Palette,
                                cloned_palette.clone().into(),
                                false
                            ) {
                                max_update = max_update.max(update);
                            }

                            *custom_palette = Some(cloned_palette);
                        }
                    }
                });

                if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::PaletteRotation, 0.0..=1.0, "Palette Rotation") {
                    max_update = max_update.max(result.update_type);
                }
            }

            if matches!(current_color_mode, ColorMode::Speed) {
                if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::SpeedFactor, 0.0..=1.0, "Speed Blend Factor") {
                    max_update = max_update.max(result.update_type);
                }
            }

            ui.separator();

            let current_bg = config_manager.active_config().background_color;
            let mut bg_array = [current_bg[0], current_bg[1], current_bg[2]];
            if ui.color_edit_button_rgb(&mut bg_array).changed() {
                if let Ok(update) = config_manager.update_param(
                    ConfigPath::BackgroundColor,
                    [bg_array[0], bg_array[1], bg_array[2]].into(),
                    false
                ) {
                    max_update = max_update.max(update);
                }
            }
            ui.label("Background Color");
        });

    max_update
}
