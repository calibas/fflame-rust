use crate::scene::{presets::PresetLibrary, transforms::Flame};
use super::formatting::format_iterations;

/// Render the Settings window with all control panels
#[allow(clippy::too_many_arguments)]
pub fn render_settings_window(
    ctx: &egui::Context,
    show_settings: &mut bool,
    show_config_window: &mut bool,
    _show_palette_editor: &mut bool,
    can_undo: bool,
    can_redo: bool,
    undo_requested: &mut bool,
    redo_requested: &mut bool,
    png_export_with_background: &mut bool,
    png_export_transparent: &mut bool,
    preset_library: &PresetLibrary,
    current_preset_index: &mut usize,
    preset_changed: &mut bool,
    flame: &mut Flame,
    render_mode_changed: &mut bool,
    projection_changed: &mut bool,
    flame_changed: &mut bool,
    flame_renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
    paused: &mut bool,
    pause_changed: &mut bool,
    reset_requested: &mut bool,
    max_iterations: &mut Option<u64>,
    iterations_per_thread: &mut u32,
    iterations_changed: &mut bool,
    deterministic_rng: &mut bool,
    speed_multiplier: &mut u32,
    histogram_color_scale: &mut f32,
    histogram_color_scale_changed: &mut bool,
    low_density_smoothing: &mut f32,
    low_density_smoothing_changed: &mut bool,
) {
    egui::Window::new("Settings")
        .open(show_settings)
        .show(ctx, |ui| {
            // Section 1: File & Project
            egui::CollapsingHeader::new("File & Project")
                .default_open(true)
                .show(ui, |ui| {
                    // Config import/export button
                    if ui.button("⚙ Import/Export Config").clicked() {
                        *show_config_window = !*show_config_window;
                    }

                    // Undo/Redo buttons
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(can_undo, |ui| {
                            if ui.button("⮪ Undo (Ctrl+Z)").clicked() {
                                *undo_requested = true;
                            }
                        });
                        ui.add_enabled_ui(can_redo, |ui| {
                            if ui.button("⮬ Redo (Ctrl+Y)").clicked() {
                                *redo_requested = true;
                            }
                        });
                    });

                    ui.separator();

                    // PNG export buttons
                    ui.label("Export Image");
                    ui.horizontal(|ui| {
                        if ui.button("💾 PNG (with BG)").clicked() {
                            *png_export_with_background = true;
                        }
                        if ui.button("💾 PNG (transparent)").clicked() {
                            *png_export_transparent = true;
                        }
                    });
                });

            // Section 2: Preset & Rendering
            egui::CollapsingHeader::new("Preset & Rendering")
                .default_open(true)
                .show(ui, |ui| {
                    // Preset selector
                    let presets = preset_library.presets();
                    let current_preset_name = presets.get(*current_preset_index)
                        .map(|p| p.flame.name.as_str())
                        .unwrap_or("Unknown");

                    egui::ComboBox::from_label("Preset")
                        .selected_text(current_preset_name)
                        .show_ui(ui, |ui| {
                            for (idx, preset) in presets.iter().enumerate() {
                                if ui.selectable_value(current_preset_index, idx, &preset.flame.name).changed() {
                                    println!("UI: Preset changed to {} ({})", preset.flame.name, idx);
                                    *preset_changed = true;
                                }
                            }
                        });

                    ui.separator();

                    // 3D Rendering Controls
                    ui.label("Render Mode");
                    ui.horizontal(|ui| {
                        let was_2d = matches!(flame.render_mode, crate::scene::transforms::RenderMode::TwoD);
                        if ui.selectable_label(was_2d, "2D").clicked() {
                            flame.render_mode = crate::scene::transforms::RenderMode::TwoD;
                            *render_mode_changed = true;
                            *flame_changed = true;
                        }
                        if ui.selectable_label(!was_2d, "3D").clicked() {
                            flame.render_mode = crate::scene::transforms::RenderMode::ThreeD;
                            *render_mode_changed = true;
                            *flame_changed = true;
                        }
                    });

                    // Show projection controls only in 3D mode
                    if matches!(flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
                        ui.label("Projection");
                        ui.horizontal(|ui| {
                            let is_ortho = matches!(flame.projection, crate::scene::transforms::ProjectionType::Orthographic);
                            if ui.selectable_label(is_ortho, "Orthographic").clicked() {
                                flame.projection = crate::scene::transforms::ProjectionType::Orthographic;
                                *projection_changed = true;
                                *flame_changed = true;
                            }
                            if ui.selectable_label(!is_ortho, "Perspective").clicked() {
                                flame.projection = crate::scene::transforms::ProjectionType::Perspective { strength: 2.0 };
                                *projection_changed = true;
                                *flame_changed = true;
                            }
                        });

                        // Perspective strength slider
                        if let crate::scene::transforms::ProjectionType::Perspective { strength } = &mut flame.projection {
                            if ui.add(egui::Slider::new(strength, 0.5..=10.0).text("Perspective Strength")).changed() {
                                *projection_changed = true;
                                *flame_changed = true;
                            }
                        }
                    }

                    ui.separator();

                    // Pause/Resume button
                    if flame_renderer.is_some() {
                        let button_text = if *paused { "▶ Resume" } else { "⏸ Pause" };
                        if ui.button(button_text).clicked() {
                            *paused = !*paused;
                            *pause_changed = true;
                        }

                        if ui.button("🔄 Reset Accumulation").clicked() {
                            *reset_requested = true;
                        }
                    }

                    ui.separator();

                    // Max iterations control
                    if let Some(renderer) = &flame_renderer {
                        ui.label("Max Iterations");

                        let mut max_enabled = max_iterations.is_some();
                        if ui.checkbox(&mut max_enabled, "Enable max iterations").changed() {
                            if max_enabled {
                                *max_iterations = Some(1_000_000_000);
                            } else {
                                *max_iterations = None;
                            }
                        }

                        if let Some(max) = max_iterations {
                            // Use a logarithmic slider for better control across large ranges
                            let mut log_value = (*max as f64).log10();
                            if ui.add(egui::Slider::new(&mut log_value, 3.0..=12.0)
                                .custom_formatter(|n, _| format!("{}", format_iterations(10f64.powf(n) as u64)))
                            ).changed() {
                                *max = 10f64.powf(log_value) as u64;
                            }

                            // Show progress if enabled
                            let current = renderer.total_iterations();
                            if current >= *max {
                                ui.label("✅ Max iterations reached");
                            } else {
                                let progress = current as f64 / *max as f64;
                                ui.label(format!("Progress: {} / {} ({:.1}%)",
                                    format_iterations(current),
                                    format_iterations(*max),
                                    progress * 100.0
                                ));
                            }
                        }
                    }

                    ui.separator();

                    // Render settings
                    if ui.add(egui::Slider::new(iterations_per_thread, 64..=4096).text("Iterations per Thread")).changed() {
                        *iterations_changed = true;
                    }

                    // Histogram color scale
                    if ui.add(egui::Slider::new(histogram_color_scale, 1.0..=100.0)
                        .logarithmic(true)
                        .text("Histogram Color Scale"))
                        .on_hover_text(
                            "Controls color precision vs overflow protection in histogram accumulation.\n\
                            Lower values prevent artifacts in zoomed-out scenes.\n\
                            Higher values give better color accuracy but overflow sooner.\n\n\
                            1-5: Maximum overflow protection (65535+ hits), very low precision\n\
                            10: Balanced (6553 hits, 10 color levels) - recommended default\n\
                            50: Higher precision (1310 hits, 50 color levels)\n\
                            100: Maximum precision (655 hits, 100 color levels) - classic"
                        )
                        .changed()
                    {
                        *histogram_color_scale_changed = true;
                    }

                    // Low-density smoothing
                    if ui.add(egui::Slider::new(low_density_smoothing, 0.0..=1.0)
                        .text("Low-Density Smoothing"))
                        .on_hover_text(
                            "Reduces noise in low-density (sparse) areas by limiting single-hit weight.\n\
                            Higher values create smoother low-density regions but slower convergence.\n\n\
                            0.0: No smoothing (accurate but noisy single hits)\n\
                            0.5: Moderate smoothing (balanced) - recommended default\n\
                            1.0: Maximum smoothing (very smooth but slow to converge)"
                        )
                        .changed()
                    {
                        *low_density_smoothing_changed = true;
                    }

                    // Speed multiplier for frame rate (1x = 60 FPS, 2x = 120 FPS, etc.)
                    ui.horizontal(|ui| {
                        ui.label("Speed:");
                        if ui.selectable_label(*speed_multiplier == 1, "1x").clicked() { *speed_multiplier = 1; }
                        if ui.selectable_label(*speed_multiplier == 2, "2x").clicked() { *speed_multiplier = 2; }
                        if ui.selectable_label(*speed_multiplier == 4, "4x").clicked() { *speed_multiplier = 4; }
                        if ui.selectable_label(*speed_multiplier == 8, "8x").clicked() { *speed_multiplier = 8; }
                        if ui.selectable_label(*speed_multiplier == 16, "16x").clicked() { *speed_multiplier = 16; }
                    });
                    ui.label(format!("Target FPS: {}", 60 * *speed_multiplier)).on_hover_text(
                        "Speed multiplier increases frame rate for smoother progressive rendering.\n\
                        Higher speeds improve quality consistency across different iterations_per_thread settings.\n\
                        1x = 60 FPS (default, vsync)\n\
                        2x = 120 FPS\n\
                        4x = 240 FPS\n\
                        8x = 480 FPS\n\
                        16x = 960 FPS"
                    );
                });

            // Section 3: Advanced
            egui::CollapsingHeader::new("Advanced")
                .default_open(false)
                .show(ui, |ui| {
                    if ui.checkbox(deterministic_rng, "Deterministic RNG").on_hover_text(
                        "Use fixed random seed for reproducible rendering.\n\
                        Enable for testing/comparison, disable for varied output."
                    ).changed() {
                        *iterations_changed = true; // Trigger renderer update
                    }
                });
        });
}
