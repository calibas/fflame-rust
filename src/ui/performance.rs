use crate::util::PerformanceMetrics;
use super::formatting::format_iterations;
use rust_i18n::t;

/// Render the Performance panel content (used by both window and dockable panel)
pub fn render_performance_content(
    ui: &mut egui::Ui,
    metrics: &PerformanceMetrics,
    window_size: winit::dpi::PhysicalSize<u32>,
    flame_renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
            // Version info
            let version_info = crate::version::get_version_info();
            ui.heading(t!("performance.heading"));
            ui.label(t!("performance.version", version = version_info.full_version()));
            ui.label(t!("performance.build",
                hash = &version_info.git_hash,
                branch = &version_info.git_branch
            ));
            ui.label(t!("performance.built", time = &version_info.build_time));
            ui.label(t!("performance.profile", profile = &version_info.profile));

            ui.separator();

            // Frame statistics
            ui.label(t!("performance.fps", value = format!("{:.1}", metrics.fps())));
            ui.label(t!("performance.frame_time", value = format!("{:.2}", metrics.frame_time_ms())));

            let (min, max) = metrics.frame_time_range();
            ui.label(t!("performance.frame_time_range",
                min = format!("{:.2}", min),
                max = format!("{:.2}", max)
            ));

            ui.separator();

            // Component timings (collapsible)
            egui::CollapsingHeader::new(t!("performance.component_timings"))
                .default_open(false)
                .show(ui, |ui| {
                    ui.label(t!("performance.compute_time", value = format!("{:.2}", metrics.compute_time_ms)));
                    ui.label(t!("performance.accumulate_time", value = format!("{:.2}", metrics.accumulate_time_ms)));
                    ui.label(t!("performance.tonemap_time", value = format!("{:.2}", metrics.tonemap_time_ms)));
                    ui.label(t!("performance.ui_time", value = format!("{:.2}", metrics.ui_time_ms)));
                    ui.label(t!("performance.submit_time", value = format!("{:.2}", metrics.submit_time_ms)));
                    ui.label(t!("performance.present_time", value = format!("{:.2}", metrics.present_time_ms)));

                    let total_measured = metrics.compute_time_ms + metrics.accumulate_time_ms +
                        metrics.tonemap_time_ms + metrics.ui_time_ms + metrics.submit_time_ms + metrics.present_time_ms;
                    ui.label(t!("performance.total_measured", value = format!("{:.2}", total_measured)));
                    ui.label(t!("performance.render_function", value = format!("{:.2}", metrics.render_time_ms)));

                    let overhead = metrics.frame_time_ms() - metrics.render_time_ms;
                    ui.label(t!("performance.overhead", value = format!("{:.2}", overhead)));
                });

            ui.separator();

            // Frame and iteration counts
            ui.label(t!("performance.total_frames", count = metrics.frame_count()));
            ui.label(t!("performance.resolution",
                width = window_size.width,
                height = window_size.height
            ));

            if let Some(renderer) = flame_renderer {
                ui.label(t!("performance.frames_accumulated", count = renderer.samples_accumulated()));
                ui.label(t!("performance.total_iterations", count = format_iterations(renderer.total_iterations())));
            }
    });
}
