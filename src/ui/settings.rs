use crate::config::{ConfigManager, ConfigPath};
use super::formatting::format_iterations;
use rust_i18n::t;

/// Render settings content (for docking panels)
pub fn render_settings_content(
    ui: &mut egui::Ui,
    flame_renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
    paused: &mut bool,
    config_manager: &mut ConfigManager,
) {
    // Clone config to avoid borrow conflicts (allows mutation of config_manager in closures)
    let config = config_manager.active_config().clone();

    // Section: Rendering Controls
    ui.label(t!("settings.iterations"));
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
        let max_iter_response = ui.add(egui::Slider::new(&mut log_value, 7.47713..=12.0)
            .text(t!("settings.max_iterations"))
            .custom_formatter(|n, _| format!("{}", format_iterations(10f64.powf(n) as u64)))
            // The slider's VALUE is log10(iterations); typed text is an
            // iteration count ("30M", "500000000"). Without this parser,
            // typed input was read as the LOG — "10000000" clamped to
            // the slider max of 12 and every VKB submit became 1T.
            .custom_parser(|s| {
                super::formatting::parse_iterations(s).map(|v| v.log10())
            }));
        super::vkb_sync_opts(ui, &max_iter_response, &format!("{}", config.max_iterations), "integer");
        if max_iter_response
            .on_hover_text(t!("settings.tooltip_max_iterations"))
            .changed()
        {
            let new_max_iterations = 10f64.powf(log_value) as u64;
            let _ = config_manager.update_param(ConfigPath::MaxIterations, new_max_iterations.into());
        }
    }

    // Render settings - Iterations per thread
    let mut temp_iterations = config_manager.system_settings().iterations_per_thread;
    let response = ui.add(super::VkbSlider::new(&mut temp_iterations, 1..=10000)
        .text(t!("settings.iterations_per_thread")))
        .on_hover_text(t!("settings.tooltip_iterations_per_thread"));

    if response.changed() {
        let _ = config_manager.update_system_setting(
            crate::config::ConfigPath::SystemIterationsPerThread,
            temp_iterations.into()
        );
    }

    // Advanced settings (collapsed by default)
    egui::CollapsingHeader::new(t!("settings.advanced"))
        .default_open(false)
        .show(ui, |ui| {
            // Burn-in iterations
            let mut temp_burn_in = config_manager.system_settings().burn_in;
            let response = ui.add(super::VkbSlider::new(&mut temp_burn_in, 0..=4096)
                .text(t!("settings.burn_in")))
                .on_hover_text(t!("settings.tooltip_burn_in"));

            if response.changed() {
                let _ = config_manager.update_system_setting(
                    crate::config::ConfigPath::SystemBurnIn,
                    temp_burn_in.into()
                );
            }

            // Deep-zoom reference orbit cache (desktop only: the wasm
            // build has no orbit store).
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut temp_cache_mb = config_manager.system_settings().orbit_cache_mb;
                let response = ui
                    .add(
                        super::VkbSlider::new(&mut temp_cache_mb, 64..=8192)
                            .text(t!("settings.orbit_cache_mb")),
                    )
                    .on_hover_text(t!("settings.tooltip_orbit_cache_mb"));
                if response.changed() {
                    let _ = config_manager.update_system_setting(
                        crate::config::ConfigPath::SystemOrbitCacheMb,
                        temp_cache_mb.into(),
                    );
                }
                ui.horizontal(|ui| {
                    let used_mb =
                        crate::escape::orbit_store::bytes_in_use() as f64 / (1024.0 * 1024.0);
                    ui.label(
                        egui::RichText::new(t!(
                            "settings.orbit_cache_in_use",
                            used = format!("{used_mb:.0}")
                        ))
                        .small()
                        .weak(),
                    );
                    if ui
                        .small_button(t!("settings.orbit_cache_clear").as_ref())
                        .on_hover_text(t!("settings.tooltip_orbit_cache_clear"))
                        .clicked()
                    {
                        crate::escape::orbit_store::clear();
                    }
                });
            }

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
        super::VkbSlider::new(&mut temp_blend, 0.001..=1.0)
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
                if ui.add(super::VkbSlider::new(&mut target_fps, 10.0..=1000.0).suffix(" FPS")).changed() {
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

    ui.separator();

    // Reset to Defaults button
    if ui.button(t!("settings.reset_to_defaults").as_ref()).clicked() {
        super::reset_rendering_to_defaults(config_manager);
        *paused = false; // Resume rendering after reset
    }
}
