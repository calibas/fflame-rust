use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::palette::{ColorMode, PathMapStyle, PathCaptureMode, PathTrackingMode, PaletteLibrary};
use crate::config::{ConfigManager, ConfigPath, LazyUndoUi, UpdateType};
use rust_i18n::t;

/// Render curve editor UI with ConfigManager integration
fn render_curve_editor(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
) -> UpdateType {
    ui.label(t!("tonemap.curve_editor"));
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
            if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, modified_curve.into()) {
                max_update = max_update.max(update);
            }
        }
    }

    // Clear drag on mouse release
    if !mouse_down && dragging_point.is_some() {
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

            // Create modified curve with new point
            let mut modified_curve = config_manager.active_config().tonemap_curve.clone();
            modified_curve.add_point(crate::scene::tonemap::CurvePoint::new(x, y));

            // Update via ConfigManager (not lazy - immediate capture for discrete action)
            if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, modified_curve.into()) {
                max_update = max_update.max(update);
            }
        }
    }

    // Show instructions
    ui.label(t!("tonemap.curve_instructions"));

    // List control points
    ui.label(t!("tonemap.curve_control_points", count = curve.points.len()));

    max_update
}

/// Render the Colors panel content (tone mapping, color mode, palette)
///
/// This is the panel version without the Window wrapper.
pub fn render_colors_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    palette_library: &PaletteLibrary,
    open_palette_editor: &mut bool,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Section 1: Tone Mapping
    egui::CollapsingHeader::new(t!("tonemap.title"))
        .default_open(true)
        .show(ui, |ui| {
            ui.label(t!("tonemap.mode"));
            let current_tonemap_mode = config_manager.active_config().tonemap_mode;
            ui.horizontal(|ui| {
                if ui.selectable_label(matches!(current_tonemap_mode, ToneMapMode::Linear), t!("tonemap.mode_linear")).clicked() {
                    if let Ok(update) = config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::Linear.into()) {
                        max_update = max_update.max(update);
                    }
                }
                if ui.selectable_label(matches!(current_tonemap_mode, ToneMapMode::Logarithmic), t!("tonemap.mode_log")).clicked() {
                    if let Ok(update) = config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::Logarithmic.into()) {
                        max_update = max_update.max(update);
                    }
                }
                if ui.selectable_label(matches!(current_tonemap_mode, ToneMapMode::DensityVisualization), t!("tonemap.mode_density")).clicked() {
                    if let Ok(update) = config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::DensityVisualization.into()) {
                        max_update = max_update.max(update);
                    }
                }
            });

            ui.separator();

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Exposure, 0.01..=10.0, t!("tonemap.exposure").as_ref(), Some(t!("tonemap.tooltip_exposure").as_ref())) {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Gamma, -1.0..=10.0, t!("tonemap.gamma").as_ref(), Some(t!("tonemap.tooltip_gamma").as_ref())) {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::GammaThreshold, 0.0..=1000.0, t!("tonemap.gamma_threshold").as_ref(), Some(t!("tonemap.tooltip_gamma_threshold").as_ref())) {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Brightness, 0.001..=100.0, t!("tonemap.brightness").as_ref(), Some(t!("tonemap.tooltip_brightness").as_ref())) {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Vibrancy, 0.0..=30.0, t!("tonemap.vibrancy").as_ref(), Some(t!("tonemap.tooltip_vibrancy").as_ref())) {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::Saturation, 0.0..=3.0, t!("tonemap.saturation").as_ref(), Some(t!("tonemap.tooltip_saturation").as_ref())) {
                max_update = max_update.max(result.update_type);
            }

            if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::HueShift, -360.0..=360.0, t!("tonemap.hue_shift").as_ref(), Some(t!("tonemap.tooltip_hue_shift").as_ref())) {
                max_update = max_update.max(result.update_type);
            }

            ui.separator();
            egui::CollapsingHeader::new(t!("tonemap.alpha_blending"))
                .default_open(false)
                .show(ui, |ui| {
                    ui.label(t!("tonemap.alpha_blending_desc"));

                    if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::AlphaBlendLow, 0.0..=1.0, t!("tonemap.alpha_blend_low").as_ref(), Some(t!("tonemap.tooltip_alpha_blend_low").as_ref())) {
                        max_update = max_update.max(result.update_type);
                    }

                    if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::AlphaBlendHigh, 0.0..=1.0, t!("tonemap.alpha_blend_high").as_ref(), Some(t!("tonemap.tooltip_alpha_blend_high").as_ref())) {
                        max_update = max_update.max(result.update_type);
                    }

                    if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::DensityScale, 0.01..=10.0, t!("tonemap.density_scale").as_ref(), Some(t!("tonemap.tooltip_density_scale").as_ref())) {
                        max_update = max_update.max(result.update_type);
                    }
                });
        });

    // Section 2: Tone Curve
    egui::CollapsingHeader::new(t!("tonemap.curve"))
        .default_open(false)
        .show(ui, |ui| {
            let mut temp_use_curve = config_manager.active_config().use_curve;
            if ui.checkbox(&mut temp_use_curve, t!("tonemap.enable_curve")).changed() {
                if let Ok(update) = config_manager.update_param(ConfigPath::UseCurve, temp_use_curve.into()) {
                    max_update = max_update.max(update);
                }
            }

            let current_use_curve = config_manager.active_config().use_curve;
            ui.add_enabled_ui(current_use_curve, |ui| {
                ui.label(t!("tonemap.curve_presets"));
                ui.horizontal(|ui| {
                    if ui.button(t!("tonemap.curve_linear")).clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::linear().into()) {
                            max_update = max_update.max(update);
                        }
                    }
                    if ui.button(t!("tonemap.curve_s_curve")).clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::s_curve().into()) {
                            max_update = max_update.max(update);
                        }
                    }
                    if ui.button(t!("tonemap.curve_brighten_shadows")).clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::brighten_shadows().into()) {
                            max_update = max_update.max(update);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(t!("tonemap.curve_darken_highlights")).clicked() {
                        if let Ok(update) = config_manager.update_param(ConfigPath::TonemapCurve, ToneCurve::darken_highlights().into()) {
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
    egui::CollapsingHeader::new(t!("tonemap.color_appearance"))
        .default_open(true)
        .show(ui, |ui| {
            let current_mode = config_manager.active_config().color_mode;
            let selected_text = match current_mode {
                ColorMode::Palette => t!("tonemap.color_mode_palette"),
                ColorMode::Speed => t!("tonemap.color_mode_speed"),
                ColorMode::PathMap => t!("tonemap.color_mode_pathmap"),
            };

            let mut temp_color_mode = current_mode;
            egui::ComboBox::from_label(t!("tonemap.color_mode"))
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut temp_color_mode, ColorMode::Palette, t!("tonemap.color_mode_palette"))
                        .on_hover_text(t!("tonemap.color_mode_palette_tooltip"))
                        .changed() 
                    {
                        if let Ok(update) = config_manager.update_param(ConfigPath::ColorMode, temp_color_mode.into()) {
                            max_update = max_update.max(update);
                        }
                    }
                    if ui.selectable_value(&mut temp_color_mode, ColorMode::Speed, t!("tonemap.color_mode_speed"))
                        .on_hover_text(t!("tonemap.color_mode_speed_tooltip"))
                        .changed() 
                    {
                        if let Ok(update) = config_manager.update_param(ConfigPath::ColorMode, temp_color_mode.into()) {
                            max_update = max_update.max(update);
                        }
                    }
                    if ui.selectable_value(&mut temp_color_mode, ColorMode::PathMap, t!("tonemap.color_mode_pathmap"))
                        .on_hover_text(t!("tonemap.color_mode_pathmap_tooltip"))
                        .changed()
                    {
                        if let Ok(update) = config_manager.update_param(ConfigPath::ColorMode, temp_color_mode.into()) {
                            max_update = max_update.max(update);
                        }
                    }
                });

            let current_color_mode = config_manager.active_config().color_mode;
            if matches!(current_color_mode, ColorMode::Palette | ColorMode::Speed) {
                // Clone palette to avoid borrow checker issues with closures
                let current_palette = config_manager.active_config().palette.clone();
                let current_palette_name = current_palette.name.clone();

                // Build list of palettes from enabled packs
                let mut available_palettes: Vec<&crate::scene::palette::Palette> = Vec::new();
                for pack_idx in 0..palette_library.pack_count() {
                    if palette_library.is_pack_enabled(pack_idx) {
                        if let Some(pack) = palette_library.get_pack(pack_idx) {
                            for palette in &pack.palettes {
                                available_palettes.push(palette);
                            }
                        }
                    }
                }

                egui::ComboBox::from_id_salt("palette_selector")
                    .selected_text(&current_palette_name)
                    .show_ui(ui, |ui| {
                        ui.label(t!("tonemap.palette"));

                        // Always show current palette first (may not be in any pack)
                        let current_in_packs = available_palettes.iter().any(|p| p.name == current_palette_name);
                        if !current_in_packs {
                            if ui.selectable_label(true, &current_palette_name).clicked() {
                                // Already selected, nothing to do
                            }
                            ui.separator();
                        }

                        // Show palettes from enabled packs
                        for palette in available_palettes.iter() {
                            let is_selected = current_palette_name == palette.name;
                            if ui.selectable_label(is_selected, &palette.name).clicked() {
                                // Set palette directly - create an editable copy
                                let mut palette_copy = (*palette).clone();
                                palette_copy.built_in = false;

                                if let Ok(update) = config_manager.update_param(
                                    ConfigPath::Palette,
                                    palette_copy.into()
                                ) {
                                    max_update = max_update.max(update);
                                }
                            }
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.button(t!("tonemap.edit_palette")).clicked() {
                        *open_palette_editor = true;
                    }

                    if ui.button(t!("tonemap.clone_palette")).clicked() {
                        // Clone current palette with new name
                        let mut cloned_palette = current_palette.clone();
                        let base_name = &current_palette.name;
                        let mut new_name = format!("{} (Copy)", base_name);
                        let mut counter = 2;

                        // Ensure unique name among available palettes
                        while available_palettes.iter().any(|p| p.name == new_name)
                            || current_palette_name == new_name {
                            new_name = format!("{} (Copy {})", base_name, counter);
                            counter += 1;
                        }

                        cloned_palette.name = new_name;
                        cloned_palette.built_in = false;

                        if let Ok(update) = config_manager.update_param(
                            ConfigPath::Palette,
                            cloned_palette.into()
                        ) {
                            max_update = max_update.max(update);
                        }
                    }
                });

                if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::PaletteRotation, 0.0..=1.0, t!("tonemap.palette_rotation").as_ref(), Some(t!("tonemap.tooltip_palette_rotation").as_ref())) {
                    max_update = max_update.max(result.update_type);
                }

                // Palette squeeze slider (0.1 to 16.0)
                if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::PaletteSqueeze, 0.1..=16.0, t!("tonemap.palette_squeeze").as_ref(), Some(t!("tonemap.tooltip_palette_squeeze").as_ref())) {
                    max_update = max_update.max(result.update_type);
                }

                // Palette size slider (256-4096, step by power of 2)
                ui.horizontal(|ui| {
                    ui.label(t!("tonemap.palette_size").as_ref());
                    let current_size = config_manager.active_config().palette_size;
                    let sizes = [256u32, 512, 1024, 2048, 4096];
                    egui::ComboBox::from_id_salt("palette_size_combo")
                        .selected_text(format!("{}", current_size))
                        .show_ui(ui, |ui| {
                            for &size in &sizes {
                                if ui.selectable_label(current_size == size, format!("{}", size)).clicked() {
                                    if let Err(e) = config_manager.update_param(ConfigPath::PaletteSize, (size as f32).into()) {
                                        log::error!("Failed to update palette size: {}", e);
                                    } else {
                                        max_update = max_update.max(UpdateType::ColorOnly);
                                    }
                                }
                            }
                        });
                }).response.on_hover_text(t!("tonemap.tooltip_palette_size").as_ref());
            }

            if matches!(current_color_mode, ColorMode::Speed) {
                if let Ok(result) = ui.lazy_slider(config_manager, ConfigPath::SpeedFactor, 0.0..=1.0, t!("tonemap.speed_blend_factor").as_ref(), Some(t!("tonemap.tooltip_speed_factor").as_ref())) {
                    max_update = max_update.max(result.update_type);
                }
            }

            if matches!(current_color_mode, ColorMode::PathMap) {
                let current_style = config_manager.active_config().path_map_style;
                let style_text = match current_style {
                    PathMapStyle::Prefix => t!("tonemap.path_prefix"),
                    PathMapStyle::Suffix => t!("tonemap.path_suffix"),
                    PathMapStyle::PrefixDistinct => t!("tonemap.path_prefix_distinct"),
                    PathMapStyle::SuffixDistinct => t!("tonemap.path_suffix_distinct"),
                    PathMapStyle::Depth => t!("tonemap.path_depth"),
                    PathMapStyle::OriginRadial => t!("tonemap.path_origin_radial"),
                    PathMapStyle::OriginHorizontal => t!("tonemap.path_origin_horizontal"),
                    PathMapStyle::OriginVertical => t!("tonemap.path_origin_vertical"),
                };

                let mut temp_style = current_style;
                egui::ComboBox::from_label(t!("tonemap.path_style"))
                    .selected_text(style_text)
                    .show_ui(ui, |ui| {
                        // Hash-based styles
                        ui.label(t!("tonemap.path_hash_based"));
                        if ui.selectable_value(&mut temp_style, PathMapStyle::Prefix, t!("tonemap.path_prefix"))
                            .on_hover_text(t!("tonemap.tooltip_path_prefix"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_style, PathMapStyle::Suffix, t!("tonemap.path_suffix"))
                            .on_hover_text(t!("tonemap.tooltip_path_suffix"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_style, PathMapStyle::PrefixDistinct, t!("tonemap.path_prefix_distinct"))
                            .on_hover_text(t!("tonemap.tooltip_path_prefix_distinct"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_style, PathMapStyle::SuffixDistinct, t!("tonemap.path_suffix_distinct"))
                            .on_hover_text(t!("tonemap.tooltip_path_suffix_distinct"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }

                        ui.separator();
                        ui.label(t!("tonemap.path_palette_gradient"));
                        if ui.selectable_value(&mut temp_style, PathMapStyle::Depth, t!("tonemap.path_depth"))
                            .on_hover_text(t!("tonemap.tooltip_path_depth"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_style, PathMapStyle::OriginRadial, t!("tonemap.path_origin_radial"))
                            .on_hover_text(t!("tonemap.tooltip_path_origin_radial"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_style, PathMapStyle::OriginHorizontal, t!("tonemap.path_origin_horizontal"))
                            .on_hover_text(t!("tonemap.tooltip_path_origin_horizontal"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_style, PathMapStyle::OriginVertical, t!("tonemap.path_origin_vertical"))
                            .on_hover_text(t!("tonemap.tooltip_path_origin_vertical"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathMapStyle, temp_style.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                    });

                // Path Capture Mode dropdown
                let current_capture = config_manager.active_config().path_capture_mode;
                let capture_text = match current_capture {
                    PathCaptureMode::FirstHit => t!("tonemap.capture_first_hit"),
                    PathCaptureMode::FirstAfterBurnIn => t!("tonemap.capture_first_after_burnin"),
                    PathCaptureMode::LastHit => t!("tonemap.capture_deepest_hit"),
                };

                let mut temp_capture = current_capture;
                egui::ComboBox::from_label(t!("tonemap.capture_mode"))
                    .selected_text(capture_text)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut temp_capture, PathCaptureMode::FirstHit, t!("tonemap.capture_first_hit"))
                            .on_hover_text(t!("tonemap.tooltip_capture_first_hit"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathCaptureMode, temp_capture.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_capture, PathCaptureMode::FirstAfterBurnIn, t!("tonemap.capture_first_after_burnin"))
                            .on_hover_text(t!("tonemap.tooltip_capture_first_after_burnin"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathCaptureMode, temp_capture.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_capture, PathCaptureMode::LastHit, t!("tonemap.capture_deepest_hit"))
                            .on_hover_text(t!("tonemap.tooltip_capture_deepest_hit"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathCaptureMode, temp_capture.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                    });

                // Path Tracking Mode dropdown
                let current_tracking = config_manager.active_config().path_tracking_mode;
                let tracking_text = match current_tracking {
                    PathTrackingMode::First => t!("tonemap.tracking_first_32"),
                    PathTrackingMode::Recent => t!("tonemap.tracking_recent_32"),
                };

                let mut temp_tracking = current_tracking;
                egui::ComboBox::from_label(t!("tonemap.tracking_mode"))
                    .selected_text(tracking_text)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut temp_tracking, PathTrackingMode::First, t!("tonemap.tracking_first_32"))
                            .on_hover_text(t!("tonemap.tooltip_tracking_first_32"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathTrackingMode, temp_tracking.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                        if ui.selectable_value(&mut temp_tracking, PathTrackingMode::Recent, t!("tonemap.tracking_recent_32"))
                            .on_hover_text(t!("tonemap.tooltip_tracking_recent_32"))
                            .changed()
                        {
                            if let Ok(update) = config_manager.update_param(ConfigPath::PathTrackingMode, temp_tracking.into()) {
                                max_update = max_update.max(update);
                            }
                        }
                    });
            }

            ui.separator();

            let current_bg = config_manager.active_config().background_color;
            let mut bg_array = [current_bg[0], current_bg[1], current_bg[2]];
            if ui.color_edit_button_rgb(&mut bg_array).changed() {
                if let Ok(update) = config_manager.update_param(
                    ConfigPath::BackgroundColor,
                    bg_array.into()
                ) {
                    max_update = max_update.max(update);
                }
            }
            ui.label(t!("tonemap.background_color"));
        });

    max_update
}
