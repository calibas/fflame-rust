use crate::scene::{presets::PresetLibrary, transforms::Flame};
use crate::config::{ConfigManager, ConfigPath};
use super::formatting::format_iterations;

/// Render settings content (for docking panels)
/// Same as render_settings_window (removed) but without the Window wrapper
pub fn render_settings_content(
    ui: &mut egui::Ui,
    png_export_with_background: &mut bool,
    _png_export_transparent: &mut bool,
    export_width: &mut u32,
    export_height: &mut u32,
    use_custom_export_size: &mut bool,
    preset_library: &PresetLibrary,
    current_preset_index: &mut usize,
    preset_changed: &mut bool,
    flame: &mut Flame,
    flame_renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
    paused: &mut bool,
    config_manager: &mut ConfigManager,
    open_config_dialog: &mut bool,
) {
    // Clone config to avoid borrow conflicts (allows mutation of config_manager in closures)
    let config = config_manager.active_config().clone();

    // Section 1: File & Project
    egui::CollapsingHeader::new("File & Project")
        .default_open(true)
        .show(ui, |ui| {
            // Preset selector
            ui.label("Presets");
            let presets = preset_library.presets();
            let current_preset_name = presets.get(*current_preset_index)
                .map(|p| p.flame.name.as_str())
                .unwrap_or("Unknown");

            egui::ComboBox::from_label("Load Preset")
                .selected_text(current_preset_name)
                .show_ui(ui, |ui| {
                    for (idx, preset) in presets.iter().enumerate() {
                        if ui.selectable_value(current_preset_index, idx, &preset.flame.name).changed() {
                            println!("UI: Loading preset: {} ({})", preset.flame.name, idx);
                            // Load preset via ConfigManager (creates single bidirectional snapshot)
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

            // Config Import/Export
            if ui.button("📁 Config Import/Export").clicked() {
                *open_config_dialog = true;
            }

        });

    ui.separator();

    // Section 2: Rendering Controls
    egui::CollapsingHeader::new("Rendering")
        .default_open(true)
        .show(ui, |ui| {
            // Pause/Reset buttons
            ui.horizontal(|ui| {
                let button_text = if *paused { "▶ Resume" } else { "⏸ Pause" };
                if ui.button(button_text).clicked() {
                    *paused = !*paused;
                }

                if ui.button("🔄 Reset Accumulation").clicked() {
                    config_manager.request_reset();
                }
            });

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
                    let _ = config_manager.update_param(ConfigPath::MaxIterations, new_max_iterations.into());
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
                let _ = config_manager.update_param(
                    ConfigPath::IterationsPerThread,
                    temp_iterations.into()
                );
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
                let _ = config_manager.update_param(
                    ConfigPath::HistogramColorScale,
                    temp_histogram.into()
                );
            }

            if response.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::HistogramColorScale);
            }

            // Low-density smoothing
            let mut temp_smoothing = config.low_density_smoothing;
            let response = ui.add(egui::Slider::new(&mut temp_smoothing, 0.0..=1.0)
                .text("Low-Density Smoothing"))
                .on_hover_text(
                    "Reduces noise in sparse areas by slowing accumulation.\n\
                    0 = No smoothing (noisy)\n\
                    0.5 = Balanced (default)\n\
                    1.0 = Maximum smoothing (smooth but slower)"
                );

            if response.changed() {
                let _ = config_manager.update_param(
                    ConfigPath::LowDensitySmoothing,
                    temp_smoothing.into()
                );
            }

            if response.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::LowDensitySmoothing);
            }

            // Density compression
            let mut temp_compression = config.density_compression_strength;
            let response = ui.add(egui::Slider::new(&mut temp_compression, 0.0..=100.0)
                .text("Density Compression"))
                .on_hover_text(
                    "Slows accumulation in bright areas to reveal detail.\n\
                    0 = Disabled (default)\n\
                    25 = Gentle (20% rate in bright areas)\n\
                    50 = Moderate (2% rate)\n\
                    100 = Strong (1% rate)"
                );

            if response.changed() {
                let _ = config_manager.update_param(
                    ConfigPath::DensityCompressionStrength,
                    temp_compression.into()
                );
            }

            if response.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::DensityCompressionStrength);
            }

            // Per-pixel iteration limit
            let mut temp_limit = config.target_iterations_per_pixel;
            let response = ui.add(egui::Slider::new(&mut temp_limit, 0..=1_000_000)
                .logarithmic(true)
                .text("Target Iterations Per Pixel"))
                .on_hover_text(
                    "Stop accumulating pixel after N hits (0 = disabled).\n\
                    Prevents over-sampling dense areas.\n\
                    Low values (5-100) for quick previews.\n\
                    High values (100K-1M) for quality."
                );

            if response.changed() {
                let _ = config_manager.update_param(
                    ConfigPath::TargetIterationsPerPixel,
                    temp_limit.into()
                );
            }

            if response.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::TargetIterationsPerPixel);
            }

            // Dynamic blend mode
            let mut temp_dynamic = config.use_dynamic_blend;
            if ui.checkbox(&mut temp_dynamic, "Use Dynamic Blend")
                .on_hover_text(
                    "Exponential convergence (old behavior).\n\
                    When enabled, blend rate adapts over time.\n\
                    When disabled, uses fixed blend rate below."
                )
                .changed()
            {
                let _ = config_manager.update_param(
                    ConfigPath::UseDynamicBlend,
                    temp_dynamic.into()
                );
            }

            // Fixed blend rate (only enabled when dynamic blend is disabled)
            let mut temp_blend = config.blend_factor;
            let response = ui.add_enabled(
                !config.use_dynamic_blend,
                egui::Slider::new(&mut temp_blend, 0.01..=1.0)
                    .text("Fixed Blend Rate")
            ).on_hover_text(
                "Controls how quickly new samples blend with history.\n\
                Only active when Dynamic Blend is disabled.\n\n\
                0.01 = Very slow/smooth\n\
                0.1 = Balanced (default)\n\
                1.0 = Fast/flickery"
            );

            if response.changed() {
                let _ = config_manager.update_param(
                    ConfigPath::BlendFactor,
                    temp_blend.into()
                );
            }

            if response.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::BlendFactor);
            }

            ui.separator();

            // VSync and frame rate settings
            let mut vsync = config.vsync_enabled;
            if ui.checkbox(&mut vsync, "Enable VSync").changed() {
                let _ = config_manager.update_param(
                    ConfigPath::VsyncEnabled,
                    vsync.into()
                );
            }

            // Only show target FPS when VSync is disabled
            if !config.vsync_enabled {
                ui.horizontal(|ui| {
                    ui.label("Target FPS:");
                    let mut target_fps = config.target_fps as f32;
                    if ui.add(egui::Slider::new(&mut target_fps, 10.0..=1000.0).suffix(" FPS")).changed() {
                        let _ = config_manager.update_param(
                            ConfigPath::TargetFps,
                            target_fps.into()
                        );
                    }
                });
            }
        });

    ui.separator();

    // Section 3: Export
    egui::CollapsingHeader::new("Export")
        .default_open(false)
        .show(ui, |ui| {
            ui.label("PNG Export");

            if ui.button("Export with Background").clicked() {
                *png_export_with_background = true;
            }
            // TODO: Transparent export currently broken, hiding for now
            // ui.checkbox(png_export_transparent, "Export Transparent");

            ui.separator();
            ui.checkbox(use_custom_export_size, "Use Custom Export Size");

            ui.add_enabled_ui(*use_custom_export_size, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width:");
                    ui.add(egui::DragValue::new(export_width).range(64..=8192).speed(10));
                });
                ui.horizontal(|ui| {
                    ui.label("Height:");
                    ui.add(egui::DragValue::new(export_height).range(64..=8192).speed(10));
                });
                ui.label(format!("Export resolution: {}×{}", export_width, export_height));
            });

            if !*use_custom_export_size {
                ui.label("Export will use current viewport size");
            }
        });

    ui.separator();

    // Section 4: Preferences
    egui::CollapsingHeader::new("Preferences")
        .default_open(false)
        .show(ui, |ui| {
            // Language selector moved to menu bar (globe icon 🌐)
            ui.label("Language: Use the 🌐 globe icon in the menu bar");

            ui.separator();

            // Advanced settings
            let mut temp_deterministic = config.deterministic_rng;
            if ui.checkbox(&mut temp_deterministic, "Deterministic RNG").on_hover_text(
                "Use fixed random seed for reproducible rendering.\n\
                Enable for testing/comparison, disable for varied output."
            ).changed() {
                let _ = config_manager.update_param(
                    ConfigPath::DeterministicRng,
                    temp_deterministic.into()
                );
            }
        });
}
