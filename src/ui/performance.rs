use crate::util::PerformanceMetrics;
use super::formatting::format_iterations;

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
            ui.heading("Fractal Flame Renderer");
            ui.label(format!("Version: {}", version_info.full_version()));
            ui.label(format!("Build: {} ({})",
                version_info.git_hash,
                version_info.git_branch
            ));
            ui.label(format!("Built: {}", version_info.build_time));
            ui.label(format!("Profile: {}", version_info.profile));

            ui.separator();

            // Frame statistics
            ui.label(format!("FPS: {:.1}", metrics.fps()));
            ui.label(format!("Frame Time: {:.2} ms", metrics.frame_time_ms()));

            let (min, max) = metrics.frame_time_range();
            ui.label(format!("Frame Time Range: {:.2} - {:.2} ms", min, max));

            ui.separator();

            // Component timings (collapsible)
            egui::CollapsingHeader::new("Component Timings")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label(format!("  Compute: {:.2} ms", metrics.compute_time_ms));
                    ui.label(format!("  Accumulate: {:.2} ms", metrics.accumulate_time_ms));
                    ui.label(format!("  Tonemap: {:.2} ms", metrics.tonemap_time_ms));
                    ui.label(format!("  UI: {:.2} ms", metrics.ui_time_ms));
                    ui.label(format!("  Submit: {:.2} ms", metrics.submit_time_ms));
                    ui.label(format!("  Present: {:.2} ms", metrics.present_time_ms));

                    let total_measured = metrics.compute_time_ms + metrics.accumulate_time_ms +
                        metrics.tonemap_time_ms + metrics.ui_time_ms + metrics.submit_time_ms + metrics.present_time_ms;
                    ui.label(format!("  Total Measured: {:.2} ms", total_measured));
                    ui.label(format!("  Render Function: {:.2} ms", metrics.render_time_ms));

                    let overhead = metrics.frame_time_ms() - metrics.render_time_ms;
                    ui.label(format!("  Overhead (event loop): {:.2} ms", overhead));
                });

            ui.separator();

            // Frame and iteration counts
            ui.label(format!("Total Frames: {}", metrics.frame_count()));
            ui.label(format!("Resolution: {}x{}", window_size.width, window_size.height));

            if let Some(renderer) = flame_renderer {
                ui.label(format!("Frames Accumulated: {}", renderer.samples_accumulated()));
                ui.label(format!("Total Iterations: {}", format_iterations(renderer.total_iterations())));
            }
    });
}
