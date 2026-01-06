use crate::config::{ConfigManager, ConfigPath};
use super::formatting::format_iterations;
use rust_i18n::t;

/// Render settings content (for docking panels)
/// Same as render_settings_window (removed) but without the Window wrapper
pub fn render_settings_content(
    ui: &mut egui::Ui,
    png_export_with_background: &mut bool,
    png_export_transparent: &mut bool,
    export_width: &mut u32,
    export_height: &mut u32,
    use_custom_export_size: &mut bool,
    flame_renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
    paused: &mut bool,
    config_manager: &mut ConfigManager,
    open_config_dialog: &mut bool,
    open_preset_library: &mut bool,
) {
    // Clone config to avoid borrow conflicts (allows mutation of config_manager in closures)
    let config = config_manager.active_config().clone();

    // Section 1: File & Project
    egui::CollapsingHeader::new(t!("settings.file_project"))
        .default_open(true)
        .show(ui, |ui| {
            // Preset Library button
            if ui.button(format!("📚 {}", t!("settings.open_preset_library"))).clicked() {
                *open_preset_library = true;
            }

            ui.separator();

            // Config Import/Export
            if ui.button(t!("settings.config_import_export")).clicked() {
                *open_config_dialog = true;
            }

        });

    ui.separator();

    // Section 2: Rendering Controls
    egui::CollapsingHeader::new(t!("settings.rendering"))
        .default_open(true)
        .show(ui, |ui| {
            // Pause/Reset buttons
            ui.horizontal(|ui| {
                let button_text = if *paused { t!("settings.resume") } else { t!("settings.pause") };
                if ui.button(button_text.as_ref()).clicked() {
                    *paused = !*paused;
                }

                if ui.button(t!("settings.reset_accumulation").as_ref()).clicked() {
                    config_manager.request_reset();
                }
            });

            ui.separator();

            // Max iterations control
            if let Some(renderer) = &flame_renderer {
                ui.label(t!("settings.max_iterations"));

                // Show progress
                let current = renderer.total_iterations();
                let max = config.max_iterations;
                if current >= max {
                    ui.label(t!("settings.max_iterations_reached"));
                } else {
                    let progress = current as f64 / max as f64;
                    ui.label(t!("settings.progress",
                        current = format_iterations(current),
                        max = format_iterations(max),
                        percent = format!("{:.1}", progress * 100.0)
                    ));
                }

                // Max iterations slider (30M to 1T with logarithmic scale)
                let mut log_value = (config.max_iterations as f64).log10();
                if ui.add(egui::Slider::new(&mut log_value, 7.47713..=12.0)
                    .text(t!("settings.max_iterations"))
                    .custom_formatter(|n, _| format!("{}", format_iterations(10f64.powf(n) as u64))))
                    .on_hover_text(t!("settings.tooltip_max_iterations"))
                    .changed()
                {
                    let new_max_iterations = 10f64.powf(log_value) as u64;
                    let _ = config_manager.update_param(ConfigPath::MaxIterations, new_max_iterations.into());
                }
            }

            ui.separator();

            // Render settings - Iterations per thread
            let mut temp_iterations = config_manager.system_settings().iterations_per_thread;
            let response = ui.add(egui::Slider::new(&mut temp_iterations, 1..=4096)
                .text(t!("settings.iterations_per_thread")))
                .on_hover_text(t!("settings.tooltip_iterations_per_thread"));

            if response.changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemIterationsPerThread,
                    temp_iterations.into()
                );
            }

            // Burn-in iterations
            let mut temp_burn_in = config_manager.system_settings().burn_in;
            let response = ui.add(egui::Slider::new(&mut temp_burn_in, 0..=4096)
                .text(t!("settings.burn_in")))
                .on_hover_text(t!("settings.tooltip_burn_in"));

            if response.changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemBurnIn,
                    temp_burn_in.into()
                );
            }

            // Histogram color scale
            let mut temp_histogram = config.histogram_color_scale;
            let response = ui.add(egui::Slider::new(&mut temp_histogram, 1.0..=100.0)
                .logarithmic(true)
                .text(t!("settings.histogram_color_scale")))
                .on_hover_text(t!("settings.tooltip_histogram_color_scale"));

            if response.changed() {
                let _ = config_manager.update_param(
                    ConfigPath::HistogramColorScale,
                    temp_histogram.into()
                );
            }

            if response.drag_stopped() {
                let _ = config_manager.force_commit_preview(&ConfigPath::HistogramColorScale);
            }

            // Per-pixel iteration limit
            let mut temp_limit = config.target_iterations_per_pixel;
            let response = ui.add(egui::Slider::new(&mut temp_limit, 0..=1_000_000)
                .logarithmic(true)
                .text(t!("settings.target_iterations_per_pixel")))
                .on_hover_text(t!("settings.tooltip_target_iterations_per_pixel"));

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
            if ui.checkbox(&mut temp_dynamic, t!("settings.use_dynamic_blend").as_ref())
                .on_hover_text(t!("settings.tooltip_use_dynamic_blend"))
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
                egui::Slider::new(&mut temp_blend, 0.001..=1.0)
                    .logarithmic(true)
                    .text(t!("settings.fixed_blend_rate"))
            ).on_hover_text(t!("settings.tooltip_fixed_blend_rate"));

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

            // VSync and frame rate settings (Desktop only - WASM always uses VSync)
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut vsync = config_manager.system_settings().vsync_enabled;
                if ui.checkbox(&mut vsync, t!("settings.enable_vsync").as_ref())
                    .on_hover_text(t!("settings.tooltip_vsync"))
                    .changed()
                {
                    let _ = config_manager.update_system_setting(
                        crate::config::ConfigPath::SystemVsyncEnabled,
                        vsync.into()
                    );
                }

                // Only show target FPS when VSync is disabled
                if !config_manager.system_settings().vsync_enabled {
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.target_fps"));
                        let mut target_fps = config_manager.system_settings().target_fps;
                        if ui.add(egui::Slider::new(&mut target_fps, 10.0..=1000.0).suffix(" FPS")).changed() {
                            let _ = config_manager.update_system_setting(
                                crate::config::ConfigPath::SystemTargetFps,
                                target_fps.into()
                            );
                        }
                    });
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                ui.label(t!("settings.vsync_wasm"));
                ui.add_enabled(false, egui::Checkbox::new(&mut true, t!("settings.enable_vsync").as_ref()))
                    .on_disabled_hover_text(t!("settings.tooltip_vsync_wasm"));
            }
        });

    ui.separator();

    // Section 3: Export
    egui::CollapsingHeader::new(t!("settings.export_section"))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(t!("settings.png_export"));

            if ui.button(t!("settings.export_with_background").as_ref()).clicked() {
                *png_export_with_background = true;
            }

            if ui.button(t!("settings.export_transparent").as_ref()).clicked() {
                *png_export_transparent = true;
            }

            ui.separator();
            ui.checkbox(use_custom_export_size, t!("settings.use_custom_size").as_ref());

            ui.add_enabled_ui(*use_custom_export_size, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t!("settings.width"));
                    if ui.add(egui::DragValue::new(export_width).range(64..=8192).speed(10)).changed() {
                        let _ = config_manager.update_system_setting(
                            crate::config::ConfigPath::SystemExportWidth,
                            (*export_width).into()
                        );
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t!("settings.height"));
                    if ui.add(egui::DragValue::new(export_height).range(64..=8192).speed(10)).changed() {
                        let _ = config_manager.update_system_setting(
                            crate::config::ConfigPath::SystemExportHeight,
                            (*export_height).into()
                        );
                    }
                });
                ui.label(t!("settings.export_resolution", width = export_width, height = export_height));
            });

            if !*use_custom_export_size {
                ui.label(t!("settings.export_viewport_size"));
            }
        });

    ui.separator();

    // Section 4: Preferences
    egui::CollapsingHeader::new(t!("settings.preferences"))
        .default_open(false)
        .show(ui, |ui| {
            // Language selector moved to menu bar (globe icon 🌐)
            ui.label(t!("settings.language_hint"));

            ui.separator();

            // Advanced settings
            let mut temp_deterministic = config.deterministic_rng;
            if ui.checkbox(&mut temp_deterministic, t!("settings.deterministic_rng").as_ref())
                .on_hover_text(t!("settings.tooltip_deterministic_rng"))
                .changed()
            {
                let _ = config_manager.update_param(
                    ConfigPath::DeterministicRng,
                    temp_deterministic.into()
                );
            }
        });
}
