use crate::scene::{presets::PresetLibrary, transforms::Flame};
use crate::config::{ConfigManager, ConfigPath, UpdateType};
use super::formatting::format_iterations;

/// Render the Settings window with all control panels
///
/// Note: Config change tracking is now handled by ConfigManager.get_pending_actions()
/// This function no longer needs *_changed parameters for config updates
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
    flame_renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
    paused: &mut bool,
    pause_changed: &mut bool,
    config_manager: &mut ConfigManager,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    egui::Window::new("Settings")
        .open(show_settings)
        .show(ctx, |ui| {
            // Clone config to avoid borrow conflicts (allows mutation of config_manager in closures)
            let config = config_manager.active_config().clone();
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
                                    println!("UI: Loading preset: {} ({})", preset.flame.name, idx);
                                    // Load preset via ConfigManager (creates two undo points)
                                    if let Err(e) = config_manager.load_config(
                                        preset.clone(),
                                        format!("Load Preset: {}", preset.flame.name),
                                    ) {
                                        log::error!("Failed to load preset: {}", e);
                                    } else {
                                        // Update flame reference from config
                                        *flame = config_manager.active_config().flame.clone();
                                        *preset_changed = true;
                                    }
                                }
                            }
                        });

                    ui.separator();

                    // 3D Rendering Controls
                    ui.label("Render Mode");
                    ui.horizontal(|ui| {
                        let was_2d = matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::TwoD);
                        if ui.selectable_label(was_2d, "2D").clicked() {
                            if let Err(e) = config_manager.update_param(
                                ConfigPath::RenderMode,
                                crate::scene::transforms::RenderMode::TwoD.into(),
                                false,
                            ) {
                                log::error!("Failed to update render mode: {}", e);
                            }
                        }
                        if ui.selectable_label(!was_2d, "3D").clicked() {
                            if let Err(e) = config_manager.update_param(
                                ConfigPath::RenderMode,
                                crate::scene::transforms::RenderMode::ThreeD.into(),
                                false,
                            ) {
                                log::error!("Failed to update render mode: {}", e);
                            }
                        }
                    });

                    // Show projection controls only in 3D mode
                    if matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
                        ui.label("Projection");
                        ui.horizontal(|ui| {
                            let is_ortho = matches!(config.flame.projection, crate::scene::transforms::ProjectionType::Orthographic);
                            if ui.selectable_label(is_ortho, "Orthographic").clicked() {
                                if let Err(e) = config_manager.update_param(
                                    ConfigPath::ProjectionType,
                                    crate::scene::transforms::ProjectionType::Orthographic.into(),
                                    false,
                                ) {
                                    log::error!("Failed to update projection type: {}", e);
                                }
                            }
                            if ui.selectable_label(!is_ortho, "Perspective").clicked() {
                                if let Err(e) = config_manager.update_param(
                                    ConfigPath::ProjectionType,
                                    crate::scene::transforms::ProjectionType::Perspective { strength: 2.0 }.into(),
                                    false,
                                ) {
                                    log::error!("Failed to update projection type: {}", e);
                                }
                            }
                        });

                        // Perspective strength slider
                        if let crate::scene::transforms::ProjectionType::Perspective { mut strength } = config.flame.projection {
                            let response = ui.add(egui::Slider::new(&mut strength, 0.5..=10.0).text("Perspective Strength"));
                            if response.changed() {
                                if let Err(e) = config_manager.update_param(
                                    ConfigPath::ProjectionType,
                                    crate::scene::transforms::ProjectionType::Perspective { strength }.into(),
                                    response.dragged(),
                                ) {
                                    log::error!("Failed to update perspective strength: {}", e);
                                }
                            }
                            if response.drag_stopped() && config_manager.is_in_preview_mode() {
                                if let Err(e) = config_manager.force_commit_preview(&ConfigPath::ProjectionType) {
                                    log::error!("Failed to commit perspective strength preview: {}", e);
                                }
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
                            config_manager.request_reset();
                        }
                    }

                    ui.separator();

                    // Max iterations control
                    if let Some(renderer) = &flame_renderer {
                        ui.label("Max Iterations");

                        // Show progress
                        let current = renderer.total_iterations();
                        let max = config.max_iterations;
                        if current >= max {
                            ui.label("✅ Max iterations reached");
                        } else {
                            let progress = current as f64 / max as f64;
                            ui.label(format!("Progress: {} / {} ({:.1}%)",
                                format_iterations(current),
                                format_iterations(max),
                                progress * 100.0
                            ));
                        }

                        // Max iterations slider (30M to 1T with logarithmic scale)
                        let mut log_value = (config.max_iterations as f64).log10();
                        if ui.add(egui::Slider::new(&mut log_value, 7.47713..=12.0)
                            .text("Max Iterations")
                            .custom_formatter(|n, _| format!("{}", format_iterations(10f64.powf(n) as u64))))
                            .changed()
                        {
                            let new_max_iterations = 10f64.powf(log_value) as u64;
                            if let Ok(update) = config_manager.update_param(ConfigPath::MaxIterations, new_max_iterations.into(), false) {
                                max_update = max_update.max(update);
                            }
                        }
                    }

                    ui.separator();

                    // Render settings - Iterations per thread
                    let mut temp_iterations = config.iterations_per_thread;
                    let response = ui.add(egui::Slider::new(&mut temp_iterations, 64..=4096)
                        .text("Iterations per Thread"))
                        .on_hover_text(
                            "GPU workgroup performance tuning.\n\
                            Higher values = fewer dispatches, better GPU utilization.\n\
                            Lower values = more frequent updates, smoother animation."
                        );

                    if response.changed() {
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::IterationsPerThread,
                            temp_iterations.into(),
                            response.dragged()  // Lazy undo
                        ) {
                            max_update = max_update.max(update_type);
                        }
                    }

                    if response.drag_stopped() {
                        let _ = config_manager.force_commit_preview(&ConfigPath::IterationsPerThread);
                    }

                    // Histogram color scale
                    let mut temp_histogram = config.histogram_color_scale;
                    let response = ui.add(egui::Slider::new(&mut temp_histogram, 1.0..=100.0)
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
                        );

                    if response.changed() {
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::HistogramColorScale,
                            temp_histogram.into(),
                            response.dragged()  // Lazy undo
                        ) {
                            max_update = max_update.max(update_type);
                        }
                    }

                    if response.drag_stopped() {
                        let _ = config_manager.force_commit_preview(&ConfigPath::HistogramColorScale);
                    }

                    // Low-density smoothing
                    let mut temp_smoothing = config.low_density_smoothing;
                    let response = ui.add(egui::Slider::new(&mut temp_smoothing, 0.0..=1.0)
                        .text("Low-Density Smoothing"))
                        .on_hover_text(
                            "Reduces noise in low-density (sparse) areas by limiting single-hit weight.\n\
                            Higher values create smoother low-density regions but slower convergence.\n\n\
                            0.0: No smoothing (accurate but noisy single hits)\n\
                            0.5: Moderate smoothing (balanced) - recommended default\n\
                            1.0: Maximum smoothing (very smooth but slow to converge)"
                        );

                    if response.changed() {
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::LowDensitySmoothing,
                            temp_smoothing.into(),
                            response.dragged()  // Lazy undo
                        ) {
                            max_update = max_update.max(update_type);
                        }
                    }

                    if response.drag_stopped() {
                        let _ = config_manager.force_commit_preview(&ConfigPath::LowDensitySmoothing);
                    }

                    // Dynamic blend toggle
                    let mut temp_use_dynamic = config.use_dynamic_blend;
                    if ui.checkbox(&mut temp_use_dynamic, "Use Dynamic Blend (Exponential Convergence)")
                        .on_hover_text(
                            "Enabled: Classic exponential convergence - blend rate decreases over time.\n\
                            Image converges to stable result, prevents overbrighten over time.\n\
                            Formula: blend_factor = samples_this_frame / samples_accumulated\n\n\
                            Disabled: Fixed blend rate - uses slider value constantly.\n\
                            Useful for testing density compression effects.\n\
                            Warning: May overbrighten if rendering for long periods."
                        )
                        .changed()
                    {
                        let _ = config_manager.update_param(
                            ConfigPath::UseDynamicBlend,
                            temp_use_dynamic.into(),
                            false  // Immediate
                        );
                    }

                    // Blend factor (accumulation rate) - only when dynamic blend is OFF
                    ui.add_enabled_ui(!config.use_dynamic_blend, |ui| {
                        let mut temp_blend = config.blend_factor;
                        let response = ui.add(egui::Slider::new(&mut temp_blend, 0.01..=1.0)
                            .logarithmic(true)
                            .text("Fixed Blend Rate"))
                            .on_hover_text(
                                "Only used when Dynamic Blend is disabled.\n\
                                Controls constant blend rate per frame.\n\n\
                                0.01: Very smooth (1% per frame)\n\
                                0.1: Balanced (10% per frame) - default\n\
                                0.5: Fast (50% per frame)\n\
                                1.0: Instant (100% per frame)"
                            );

                        if response.changed() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::BlendFactor,
                                temp_blend.into(),
                                response.dragged()  // Lazy undo
                            ) {
                                max_update = max_update.max(update_type);
                            }
                        }

                        if response.drag_stopped() {
                            let _ = config_manager.force_commit_preview(&ConfigPath::BlendFactor);
                        }
                    });

                    // Density compression strength
                    let mut temp_compression = config.density_compression_strength;
                    let response = ui.add(egui::Slider::new(&mut temp_compression, 0.0..=100.0)
                        .text("Density Compression"))
                        .on_hover_text(
                            "Slows accumulation in bright areas to reveal core detail.\n\
                            Works with Blend Rate to control per-pixel accumulation speed.\n\n\
                            0: No compression - uniform accumulation (default)\n\
                            25: Gentle - bright areas accumulate at ~20% rate\n\
                            50: Moderate - bright areas accumulate at ~2% rate\n\
                            100: Strong - bright areas accumulate at ~1% rate\n\n\
                            Higher Blend Rate makes compression effect more visible.\n\
                            Use this to prevent bright cores from washing out detail."
                        );

                    if response.changed() {
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::DensityCompressionStrength,
                            temp_compression.into(),
                            response.dragged()  // Lazy undo
                        ) {
                            max_update = max_update.max(update_type);
                        }
                    }

                    if response.drag_stopped() {
                        let _ = config_manager.force_commit_preview(&ConfigPath::DensityCompressionStrength);
                    }

                    // Per-pixel iteration limit
                    // Use logarithmic slider for better control across large ranges
                    let mut log_value = if config.target_iterations_per_pixel == 0 {
                        0.0  // 0 maps to log(0) special case
                    } else {
                        (config.target_iterations_per_pixel as f64).log10()
                    };

                    let response = ui.add(egui::Slider::new(&mut log_value, 0.0..=6.0)
                        .custom_formatter(|n, _| {
                            if n < 0.5 {
                                "Disabled".to_string()
                            } else {
                                format!("{}", format_iterations(10f64.powf(n) as u64))
                            }
                        })
                        .text("Per-Pixel Iteration Limit"))
                        .on_hover_text(
                            "Stop accumulating a pixel after it receives N iterations.\n\
                            Prevents over-sampling dense areas while sparse areas catch up.\n\n\
                            Disabled: All pixels accumulate forever (default)\n\
                            1,000: Very aggressive - stops early\n\
                            10,000: Moderate - good for quick previews\n\
                            100,000: Conservative - allows good convergence\n\
                            1,000,000: Very conservative - high quality\n\n\
                            Works independently of blend_factor and density compression."
                        );

                    if response.changed() {
                        let new_value = if log_value < 0.5 {
                            0  // Disabled
                        } else {
                            10f64.powf(log_value) as u32
                        };
                        if let Ok(update_type) = config_manager.update_param(
                            ConfigPath::TargetIterationsPerPixel,
                            new_value.into(),
                            response.dragged()  // Lazy undo
                        ) {
                            max_update = max_update.max(update_type);
                        }
                    }

                    if response.drag_stopped() {
                        let _ = config_manager.force_commit_preview(&ConfigPath::TargetIterationsPerPixel);
                    }

                    // Speed multiplier for frame rate (1x = 60 FPS, 2x = 120 FPS, etc.)
                    ui.horizontal(|ui| {
                        ui.label("Speed:");
                        if ui.selectable_label(config.speed_multiplier == 1, "1x").clicked() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::SpeedMultiplier,
                                1u32.into(),
                                false  // Immediate capture
                            ) {
                                max_update = max_update.max(update_type);
                            }
                        }
                        if ui.selectable_label(config.speed_multiplier == 2, "2x").clicked() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::SpeedMultiplier,
                                2u32.into(),
                                false
                            ) {
                                max_update = max_update.max(update_type);
                            }
                        }
                        if ui.selectable_label(config.speed_multiplier == 4, "4x").clicked() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::SpeedMultiplier,
                                4u32.into(),
                                false
                            ) {
                                max_update = max_update.max(update_type);
                            }
                        }
                        if ui.selectable_label(config.speed_multiplier == 8, "8x").clicked() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::SpeedMultiplier,
                                8u32.into(),
                                false
                            ) {
                                max_update = max_update.max(update_type);
                            }
                        }
                        if ui.selectable_label(config.speed_multiplier == 16, "16x").clicked() {
                            if let Ok(update_type) = config_manager.update_param(
                                ConfigPath::SpeedMultiplier,
                                16u32.into(),
                                false
                            ) {
                                max_update = max_update.max(update_type);
                            }
                        }
                    });
                    ui.label(format!("Target FPS: {}", 60 * config.speed_multiplier)).on_hover_text(
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
                    let mut temp_deterministic = config.deterministic_rng;
                    if ui.checkbox(&mut temp_deterministic, "Deterministic RNG").on_hover_text(
                        "Use fixed random seed for reproducible rendering.\n\
                        Enable for testing/comparison, disable for varied output."
                    ).changed() {
                        let _ = config_manager.update_param(
                            ConfigPath::DeterministicRng,
                            temp_deterministic.into(),
                            false  // Immediate
                        );
                    }
                });
        });

    max_update
}
