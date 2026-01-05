use crate::app::render_mode::TransitionResult;
use crate::app::App;
use crate::config::FractalConfig;

impl App {
    /// Load config via ConfigManager and sync app state
    /// Creates snapshot-based undo entry and triggers GPU update
    pub fn load_config_with_undo(&mut self, config: FractalConfig, description: String) -> Result<(), String> {
        // Load via ConfigManager (creates before/after snapshots for undo)
        self.config_manager
            .load_config(config, description)
            .map_err(|e| format!("{}", e))?;

        // Sync all app state from ConfigManager (triggers GPU update)
        let active_config = self.config_manager.active_config().clone();
        self.import_config(active_config);

        Ok(())
    }

    pub fn export_config(&self) -> FractalConfig {
        // ConfigManager is the single source of truth - just return its config
        self.config_manager.active_config().clone()
    }

    /// Import configuration from FractalConfig
    pub fn import_config(&mut self, config: FractalConfig) {
        // Sync working copy for renderer (only field not in ConfigManager)
        self.flame = config.flame.clone();

        // Handle palette library updates
        if let Some(ref palette) = config.palette {
            // Add or update palette in library (prevents duplicates)
            let _palette_idx = self.palette_library.add_or_update(palette.clone());

            // Sync palette editor with the palette
            self.egui_layer.update_palette_editor(palette.clone());
        } else {
            // No custom palette in config, sync with library palette
            if let Some(palette) = self.palette_library.get(config.palette_index) {
                self.egui_layer.update_palette_editor(palette.clone());
            }
        }

        // Use the comprehensive load_config function to ensure all GPU state is synchronized
        // (including tone mapping, palette, transforms, params, etc.)
        if let Some(ref mut renderer) = self.flame_renderer {
            let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                label: Some("Config Import Encoder"),
            });

            if let Some(palette) = config.palette.as_ref().or_else(|| self.palette_library.get(config.palette_index)) {
                renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in);
            }

            self.gpu.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    /// Undo to previous state
    pub fn undo(&mut self) {
        if let Ok(_update_type) = self.config_manager.undo() {
            // Sync App working copy and GPU state from ConfigManager
            let config = self.config_manager.config();
            self.import_config(config.clone());
        }
    }

    /// Redo to next state
    pub fn redo(&mut self) {
        if let Ok(_update_type) = self.config_manager.redo() {
            // Sync App working copy and GPU state from ConfigManager
            let config = self.config_manager.config();
            self.import_config(config.clone());
        }
    }

    pub fn can_undo(&self) -> bool {
        self.config_manager.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.config_manager.can_redo()
    }

    /// Handle exiting animation mode
    ///
    /// Called when animation stops (user stop, pause, or auto-stop from LoopMode::Once).
    /// Creates undo entry if the FSM transition requires it.
    pub fn handle_animation_exit(&mut self) {
        let result = self.render_mode.exit_animation(self.config_manager.active_config());

        if let TransitionResult::CreateUndo { before, after, description } = result {
            if let Err(e) = self.config_manager.load_config_with_explicit_before(
                before,
                after,
                description,
            ) {
                log::error!("Failed to create animation undo snapshot: {}", e);
            }
        }
    }

    /// Export PNG at custom dimensions using unified render API
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_custom_size(&self, transparent: bool, config: FractalConfig, _render_time_ms: f64) {
        use crate::renderer::{render, NoProgress, RenderJob};

        println!("Exporting at custom size: {}×{}", self.export_width, self.export_height);

        // Check if we need CPU-based high-res export (for large resolutions)
        if crate::export::needs_cpu_export(self.export_width, self.export_height) {
            println!("  Using high-res CPU export (resolution exceeds GPU buffer limits)");
            self.export_high_res_cpu(transparent, config);
            return;
        }

        // Use unified render API
        let job = RenderJob::new(&config, self.export_width, self.export_height)
            .with_iterations_per_thread(self.config_manager.system_settings().iterations_per_thread)
            .with_burn_in(self.config_manager.system_settings().burn_in)
            .with_transparent(transparent);

        let result = pollster::block_on(async {
            render(&self.gpu.device, &self.gpu.queue, job, &mut NoProgress).await
        });

        match result {
            Ok(output) => {
                // Build metadata
                let metadata = crate::png_metadata::PngMetadata::from_app_state(
                    output.width,
                    output.height,
                    output.total_iterations,
                    output.render_time_ms,
                    self.config_manager.system_settings().iterations_per_thread,
                    config.speed_factor,
                    &config,
                );

                // Encode PNG
                match crate::renderer::compute_kernel::encode_png_from_rgba(
                    output.width,
                    output.height,
                    output.rgba_data,
                    Some(metadata),
                ) {
                    Ok(png_data) => {
                        // Open file dialog
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG Image", &["png"])
                            .set_file_name("fractal.png")
                            .save_file()
                        {
                            if let Err(e) = std::fs::write(&path, png_data) {
                                eprintln!("Failed to save PNG: {}", e);
                            } else {
                                println!(
                                    "PNG exported to: {} ({}×{}, {:.2}s)",
                                    path.display(),
                                    output.width,
                                    output.height,
                                    output.render_time_ms / 1000.0
                                );
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to encode PNG: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to render: {}", e),
        }
    }

    /// High-resolution CPU export using HighResExporter (same system as CLI)
    #[cfg(not(target_arch = "wasm32"))]
    fn export_high_res_cpu(&self, transparent: bool, config: FractalConfig) {
        use crate::export::{HighResExporter, CliExportProgress};
        use std::time::Instant;

        let export_start = Instant::now();
        let width = self.export_width;
        let height = self.export_height;

        // Create exporter (async, but we block on it)
        let exporter_result = pollster::block_on(HighResExporter::new(&config, width, height, None));

        let mut exporter = match exporter_result {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Failed to create high-res exporter: {}", e);
                return;
            }
        };

        // Run export with CLI-style progress (prints to console)
        let mut progress = CliExportProgress;
        let rgba_result = pollster::block_on(exporter.export(&config, config.max_iterations, transparent, &mut progress));

        let rgba_data = match rgba_result {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to export: {}", e);
                return;
            }
        };

        let total_export_time_ms = export_start.elapsed().as_secs_f64() * 1000.0;

        // Build metadata
        let metadata = crate::png_metadata::PngMetadata::from_app_state(
            width,
            height,
            config.max_iterations,
            total_export_time_ms,
            self.config_manager.system_settings().iterations_per_thread,
            config.speed_factor,
            &config,
        );

        // Encode PNG with metadata
        match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
            Ok(png_data) => {
                // Open file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG Image", &["png"])
                    .set_file_name("fractal.png")
                    .save_file()
                {
                    if let Err(e) = std::fs::write(&path, png_data) {
                        eprintln!("Failed to save PNG: {}", e);
                    } else {
                        println!("PNG exported to: {} ({}×{}, {:.2}s)",
                            path.display(), width, height, total_export_time_ms / 1000.0);
                    }
                }
            }
            Err(e) => eprintln!("Failed to encode PNG: {}", e),
        }
    }
}
