use crate::scene::palette::{ColorStop, Palette};
use rust_i18n::t;

pub struct PaletteEditor {
    pub current_palette: Palette,
    pub json_buffer: String,
    pub show_fixed_mode_warning: bool,
}

impl PaletteEditor {
    pub fn new() -> Self {
        Self {
            current_palette: Palette::fire(),
            json_buffer: String::new(),
            show_fixed_mode_warning: false,
        }
    }
}

/// Core implementation of palette editor (shared by window and panel)
fn render_palette_editor_core_impl(
    ui: &mut egui::Ui,
    palette_editor: &mut PaletteEditor,
    config_manager: &mut crate::config::ConfigManager,
    palette: &mut Palette,
    _custom_palette: &mut Option<Palette>,
    palette_export_json: &mut Option<Palette>,
    palette_save_file: &mut Option<Palette>,
    palette_import_json: &mut Option<String>,
    palette_load_file: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label(t!("palette_editor.palette_name"));
        let name_response = ui.text_edit_singleline(&mut palette.name);

        if name_response.lost_focus() {
            let _ = config_manager.update_param(
                crate::config::ConfigPath::Palette,
                palette.clone().into(),
            );
        }
    });
    ui.separator();

            // Gradient preview
            ui.label(t!("palette_editor.gradient_preview"));
            let preview_height = 40.0;
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), preview_height),
                egui::Sense::click()
            );

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // Draw gradient preview
                let steps = rect.width() as usize;
                for i in 0..steps {
                    let t = i as f32 / steps as f32;
                    let color = palette.sample_color(t);
                    let x = rect.left() + i as f32;
                    let color_u8 = egui::Color32::from_rgb(
                        (color[0] * 255.0) as u8,
                        (color[1] * 255.0) as u8,
                        (color[2] * 255.0) as u8,
                    );
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x, rect.top()),
                            egui::vec2(1.0, preview_height)
                        ),
                        0.0,
                        color_u8
                    );
                }
            }

            ui.separator();

            // Mode indicator and toggle
            ui.horizontal(|ui| {
                if palette.locked {
                    ui.label(t!("palette_editor.mode_fixed"));
                } else {
                    ui.label(t!("palette_editor.mode_free"));
                }

                // Toggle button
                let button_text = if palette.locked {
                    t!("palette_editor.switch_to_free")
                } else {
                    t!("palette_editor.switch_to_fixed")
                };

                if ui.button(button_text.as_ref()).clicked() {
                    if palette.locked {
                        // Free mode: just unlock (no data loss, no warning needed)
                        palette.convert_to_free();
                        let _ = config_manager.update_param(
                            crate::config::ConfigPath::Palette,
                            palette.clone().into()
                        );
                    } else {
                        // Fixed mode: show warning dialog
                        palette_editor.show_fixed_mode_warning = true;
                    }
                }
            });
            ui.separator();

            // Color stops list
            ui.label(t!("palette_editor.color_stops"));

            let mut stop_to_remove = None;
            let mut palette_updated = false;
            let mut force_commit = false;

            egui::ScrollArea::vertical()
                .max_height(250.0)
                .show(ui, |ui| {
                    let stops_len = palette.stops.len();
                    for i in 0..stops_len {
                        ui.horizontal(|ui| {
                            let stop = &mut palette.stops[i];

                            ui.label(t!("palette_editor.stop_label", index = i));

                            // Position slider - locked in fixed mode
                            let mut pos_int = (stop.position * 255.0) as i32;
                            let slider_response = ui.add_enabled(
                                !palette.locked,
                                egui::Slider::new(&mut pos_int, 0..=255).text(t!("palette_editor.position"))
                            ).on_hover_text(t!("palette_editor.tooltip_position"));
                            if slider_response.changed() {
                                stop.position = pos_int as f32 / 255.0;
                                palette_updated = true;
                            }

                            // Force commit on drag end to create final undo entry
                            if slider_response.drag_stopped() {
                                force_commit = true;
                            }

                            // Color picker - no preview mode (immediate commits)
                            let mut color = [stop.color[0], stop.color[1], stop.color[2]];
                            let color_response = ui.color_edit_button_rgb(&mut color)
                                .on_hover_text(t!("palette_editor.tooltip_color"));
                            if color_response.changed() {
                                stop.color = color;
                                // Immediate commit (no preview mode) since we can't detect popup close
                                let _ = config_manager.update_param(
                                    crate::config::ConfigPath::Palette,
                                    palette.clone().into()
                                );
                            }

                            // Remove button (disabled in fixed mode, keep at least 2 stops)
                            if stops_len > 2 && !palette.locked && ui.button("🗑")
                                .on_hover_text(t!("palette_editor.tooltip_remove"))
                                .clicked()
                            {
                                stop_to_remove = Some(i);
                            }
                        });
                    }
                });

            // Handle ConfigManager updates after all mutable borrows are done
            if palette_updated {
                // Update library (live updates during drag)

                // Enter preview mode for live rendering
                let _ = config_manager.update_param(
                    crate::config::ConfigPath::Palette,
                    palette.clone().into()
                );
            }

            // Remove stop if requested
            if let Some(idx) = stop_to_remove {
                palette.stops.remove(idx);

                // Update library and create undo point
                let _ = config_manager.update_param(
                    crate::config::ConfigPath::Palette,
                    palette.clone().into()
                );
            }

            ui.separator();

            // Add stop button (disabled in fixed mode)
            if ui.add_enabled(
                !palette.locked,
                egui::Button::new(t!("palette_editor.add_color_stop"))
            ).clicked() {
                let new_position = if palette.stops.is_empty() {
                    0.5
                } else {
                    // Find a gap to insert
                    palette.stops.last().unwrap().position
                };
                palette.stops.push(ColorStop {
                    position: new_position,
                    color: [1.0, 1.0, 1.0],
                });
                // Sort by position
                palette.stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());

                // Update library and create undo point
                let _ = config_manager.update_param(
                    crate::config::ConfigPath::Palette,
                    palette.clone().into()
                );
            }

            ui.separator();

            // Import/Export section
            ui.collapsing(t!("palette_editor.import_export"), |ui| {
                ui.horizontal(|ui| {
                    if ui.button(t!("palette_editor.export_clipboard")).clicked() {
                        *palette_export_json = Some(palette.clone());
                    }

                    if ui.button(t!("palette_editor.save_as_palette")).clicked() {
                        *palette_save_file = Some(palette.clone());
                    }
                });

                ui.separator();
                ui.label(t!("palette_editor.import_from_json"));

                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        ui.text_edit_multiline(&mut palette_editor.json_buffer);
                    });

                ui.horizontal(|ui| {
                    if ui.button(t!("palette_editor.import")).clicked() && !palette_editor.json_buffer.is_empty() {
                        *palette_import_json = Some(palette_editor.json_buffer.clone());
                    }

                    if ui.button(t!("palette_editor.clear")).clicked() {
                        palette_editor.json_buffer.clear();
                    }

                    if ui.button(t!("palette_editor.load_palette")).clicked() {
                        *palette_load_file = true;
                    }
                });
            });

            ui.separator();

}

pub fn render_fixed_mode_warning(
    ctx: &egui::Context,
    palette_editor: &mut PaletteEditor,
    config_manager: &mut crate::config::ConfigManager,
) {
    // Fixed mode warning dialog
    if palette_editor.show_fixed_mode_warning {
        egui::Window::new(t!("palette_editor.warning_title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(t!("palette_editor.warning_resample"));
                ui.label(t!("palette_editor.warning_locked"));
                ui.label("");
                ui.label(t!("palette_editor.warning_revert"));

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button(t!("palette_editor.convert_confirm")).clicked() {
                        // Get current palette and convert it
                        if let Some(mut palette) = config_manager.active_config().palette.clone() {
                            palette.convert_to_fixed();
                            let _ = config_manager.update_param(
                                crate::config::ConfigPath::Palette,
                                palette.into()
                            );
                        }
                        palette_editor.show_fixed_mode_warning = false;
                    }

                    if ui.button(t!("palette_editor.cancel")).clicked() {
                        palette_editor.show_fixed_mode_warning = false;
                    }
                });
            });
    }
}

/// Render the Palette Editor panel content (palette editing)
///
/// This is the panel version without the Window wrapper.
pub fn render_palette_editor_content(
    ui: &mut egui::Ui,
    palette_editor: &mut PaletteEditor,
    config_manager: &mut crate::config::ConfigManager,
    custom_palette: &mut Option<Palette>,
    palette_export_json: &mut Option<Palette>,
    palette_save_file: &mut Option<Palette>,
    palette_import_json: &mut Option<String>,
    palette_load_file: &mut bool,
) {
    // Always read from config.palette (single source of truth)
    let Some(config_palette) = &config_manager.active_config().palette else {
        ui.label(t!("palette_editor.no_palette"));
        ui.label(t!("palette_editor.no_palette_hint"));
        return;
    };

    // Work on a mutable copy for this frame
    let mut palette = config_palette.clone();

    render_palette_editor_core_impl(
        ui,
        palette_editor,
        config_manager,
        &mut palette,
        custom_palette,
        palette_export_json,
        palette_save_file,
        palette_import_json,
        palette_load_file,
    );

    // Note: The warning dialog can't be shown in panel mode (needs Window)
    // It will only appear when using the floating window mode
}
