use crate::scene::palette::{ColorStop, Palette};

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

/// Render the Palette Editor window
#[allow(clippy::too_many_arguments)]
pub fn render_palette_editor_window(
    ctx: &egui::Context,
    show_palette_editor: &mut bool,
    palette_editor: &mut PaletteEditor,
    config_manager: &mut crate::config::ConfigManager,
    custom_palette: &mut Option<Palette>,
    palette_changed: &mut bool,
    palette_export_json: &mut Option<Palette>,
    palette_save_file: &mut Option<Palette>,
    palette_import_json: &mut Option<String>,
    palette_load_file: &mut bool,
) {
    if !*show_palette_editor {
        return;
    }

    egui::Window::new("Palette Editor")
        .open(show_palette_editor)
        .default_width(600.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Palette Name:");
                let name_response = ui.text_edit_singleline(&mut palette_editor.current_palette.name);

                // Create undo entry when user finishes editing name
                if name_response.lost_focus() {
                    let palette_box = Box::new(palette_editor.current_palette.clone());
                    if let Ok(_) = config_manager.update_param(
                        crate::config::ConfigPath::Palette(palette_box.clone()),
                        crate::config::ConfigValue::Palette((*palette_box).clone()),
                        false // immediate mode - discrete action
                    ) {
                        *palette_changed = true;
                    }
                }
            });
            ui.separator();

            // Gradient preview
            ui.label("Gradient Preview:");
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
                    let color = palette_editor.current_palette.sample_color(t);
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
                if palette_editor.current_palette.locked {
                    ui.label("Mode: 🔒 Fixed 256-Color");
                } else {
                    ui.label("Mode: 🎨 Free Gradient");
                }

                // Toggle button
                let button_text = if palette_editor.current_palette.locked {
                    "Switch to Free Mode"
                } else {
                    "Switch to Fixed 256-Color Mode"
                };

                if ui.button(button_text).clicked() {
                    if palette_editor.current_palette.locked {
                        // Free mode: just unlock (no data loss, no warning needed)
                        palette_editor.current_palette.convert_to_free();
                        *palette_changed = true;
                        // Auto-apply the palette when converting to free mode
                        *custom_palette = Some(palette_editor.current_palette.clone());
                    } else {
                        // Fixed mode: show warning dialog
                        palette_editor.show_fixed_mode_warning = true;
                    }
                }
            });
            ui.separator();

            // Color stops list
            ui.label("Color Stops:");

            let mut stop_to_remove = None;
            let mut palette_updated = false;
            let mut force_commit = false;

            egui::ScrollArea::vertical()
                .max_height(250.0)
                .show(ui, |ui| {
                    let stops_len = palette_editor.current_palette.stops.len();
                    for i in 0..stops_len {
                        ui.horizontal(|ui| {
                            let stop = &mut palette_editor.current_palette.stops[i];

                            ui.label(format!("Stop {}:", i));

                            // Position slider - locked in fixed mode
                            let mut pos_int = (stop.position * 255.0) as i32;
                            let slider_response = ui.add_enabled(
                                !palette_editor.current_palette.locked,
                                egui::Slider::new(&mut pos_int, 0..=255).text("Position")
                            );
                            if slider_response.changed() {
                                stop.position = pos_int as f32 / 255.0;
                                palette_updated = true;
                            }

                            // Force commit on drag end to create final undo entry
                            if slider_response.drag_stopped() {
                                force_commit = true;
                            }

                            // Color picker
                            let mut color = [stop.color[0], stop.color[1], stop.color[2]];
                            let color_response = ui.color_edit_button_rgb(&mut color);
                            if color_response.changed() {
                                stop.color = color;
                                palette_updated = true;
                            }

                            // Force commit when color picker closes (loses focus)
                            if color_response.lost_focus() {
                                force_commit = true;
                            }

                            // Remove button (disabled in fixed mode, keep at least 2 stops)
                            if stops_len > 2 && !palette_editor.current_palette.locked && ui.button("🗑").clicked() {
                                stop_to_remove = Some(i);
                            }
                        });
                    }
                });

            // Handle ConfigManager updates after all mutable borrows are done
            if palette_updated {
                // Live update via ConfigManager (lazy mode for smooth editing)
                let palette_box = Box::new(palette_editor.current_palette.clone());
                if let Ok(_) = config_manager.update_param(
                    crate::config::ConfigPath::Palette(palette_box.clone()),
                    crate::config::ConfigValue::Palette((*palette_box).clone()),
                    true // lazy mode - preview during drag
                ) {
                    *palette_changed = true;
                }
            }

            if force_commit {
                let palette_box = Box::new(palette_editor.current_palette.clone());
                let _ = config_manager.force_commit_preview(&crate::config::ConfigPath::Palette(palette_box));
            }

            // Remove stop if requested
            if let Some(idx) = stop_to_remove {
                palette_editor.current_palette.stops.remove(idx);

                // Immediate undo entry for delete operation
                let palette_box = Box::new(palette_editor.current_palette.clone());
                if let Ok(_) = config_manager.update_param(
                    crate::config::ConfigPath::Palette(palette_box.clone()),
                    crate::config::ConfigValue::Palette((*palette_box).clone()),
                    false // immediate mode - discrete action
                ) {
                    *palette_changed = true;
                }
            }

            ui.separator();

            // Add stop button (disabled in fixed mode)
            if ui.add_enabled(
                !palette_editor.current_palette.locked,
                egui::Button::new("➕ Add Color Stop")
            ).clicked() {
                let new_position = if palette_editor.current_palette.stops.is_empty() {
                    0.5
                } else {
                    // Find a gap to insert
                    palette_editor.current_palette.stops.last().unwrap().position
                };
                palette_editor.current_palette.stops.push(ColorStop {
                    position: new_position,
                    color: [1.0, 1.0, 1.0],
                });
                // Sort by position
                palette_editor.current_palette.stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());

                // Immediate undo entry for add operation
                let palette_box = Box::new(palette_editor.current_palette.clone());
                if let Ok(_) = config_manager.update_param(
                    crate::config::ConfigPath::Palette(palette_box.clone()),
                    crate::config::ConfigValue::Palette((*palette_box).clone()),
                    false // immediate mode - discrete action
                ) {
                    *palette_changed = true;
                }
            }

            ui.separator();

            // Import/Export section
            ui.collapsing("Import/Export Palette", |ui| {
                ui.horizontal(|ui| {
                    if ui.button("📋 Export to Clipboard").clicked() {
                        *palette_export_json = Some(palette_editor.current_palette.clone());
                    }

                    if ui.button("💾 Save as .palette").clicked() {
                        *palette_save_file = Some(palette_editor.current_palette.clone());
                    }
                });

                ui.separator();
                ui.label("Import from JSON:");

                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        ui.text_edit_multiline(&mut palette_editor.json_buffer);
                    });

                ui.horizontal(|ui| {
                    if ui.button("✅ Import").clicked() && !palette_editor.json_buffer.is_empty() {
                        *palette_import_json = Some(palette_editor.json_buffer.clone());
                    }

                    if ui.button("🗑 Clear").clicked() {
                        palette_editor.json_buffer.clear();
                    }

                    if ui.button("📂 Load .palette").clicked() {
                        *palette_load_file = true;
                    }
                });
            });

            ui.separator();

            // Note: Apply button removed - all changes now applied live via ConfigManager
            ui.label("💡 All changes are applied instantly and tracked in undo history.");
        });

    // Fixed mode warning dialog
    if palette_editor.show_fixed_mode_warning {
        egui::Window::new("⚠️ Convert to Fixed 256-Color Mode?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("This will resample your gradient to exactly 256 color stops.");
                ui.label("Color positions will be locked at fixed intervals (0/255, 1/255, etc.).");
                ui.label("");
                ui.label("You can switch back to Free Mode later without data loss.");

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("✅ Convert to Fixed Mode").clicked() {
                        palette_editor.current_palette.convert_to_fixed();
                        *palette_changed = true;
                        // Auto-apply the palette when converting to fixed mode
                        *custom_palette = Some(palette_editor.current_palette.clone());
                        palette_editor.show_fixed_mode_warning = false;
                    }

                    if ui.button("❌ Cancel").clicked() {
                        palette_editor.show_fixed_mode_warning = false;
                    }
                });
            });
    }
}
