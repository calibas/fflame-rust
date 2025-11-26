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
                renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, self.config_manager.system_settings().iterations_per_thread);
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

    /// Export PNG at custom dimensions (creates temporary renderer)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_custom_size(&self, transparent: bool, config: FractalConfig, _render_time_ms: f64) {
        use crate::renderer::compute_kernel::FlameRenderer;
        use std::time::Instant;

        println!("Exporting at custom size: {}×{}", self.export_width, self.export_height);
        println!("  iterations_per_thread: {}", self.config_manager.system_settings().iterations_per_thread);
        println!("  max_iterations: {}", config.max_iterations);
        println!("  density_scale: {}", config.density_scale);
        println!("  histogram_color_scale: {}", config.histogram_color_scale);
        println!("  brightness: {}", config.brightness);
        println!("  exposure: {}", config.exposure);
        println!("  use_dynamic_blend: {}", config.use_dynamic_blend);
        println!("  blend_factor: {}", config.blend_factor);

        // Create temporary renderer at export dimensions
        let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
        let mut temp_renderer = FlameRenderer::new(
            &self.gpu.device,
            &self.gpu.queue,
            surface_format,
            self.export_width,
            self.export_height,
            &config.flame,
        );

        // Load config into temp renderer
        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Custom Export Encoder"),
        });

        // Get palette
        let palette = config.palette.as_ref()
            .or_else(|| self.palette_library.get(config.palette_index))
            .expect("No palette found");

        temp_renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, self.config_manager.system_settings().iterations_per_thread);
        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        // Render frames until we reach max_iterations
        let render_start = Instant::now();
        let mut total_rendered = 0u64;
        let target = config.max_iterations;

        const NUM_WORKGROUPS: u32 = 128;
        const THREADS_PER_WORKGROUP: u64 = 64;
        const BATCH_SIZE: u32 = 4; // Match viewport's accumulation_batch_size

        let mut accumulation_count = 0;
        let mut batch_frame_count = 0;

        while total_rendered < target {
            let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                label: Some("Export Render Frame"),
            });

            // Clear histogram only on first frame of batch (match viewport behavior)
            let clear_histogram = batch_frame_count == 0;

            // Use FULL iterations_per_thread like viewport does (not divided by speed_multiplier)
            temp_renderer.compute_pass(
                &mut encoder,
                &self.gpu.queue,
                NUM_WORKGROUPS,
                self.config_manager.system_settings().iterations_per_thread, // CHANGED: Use full value like viewport
                config.zoom,
                config.pan_x,
                config.pan_y,
                config.rotation,
                config.camera_rotation_x,
                config.camera_rotation_y,
                config.camera_z,
                config.speed_factor,
                clear_histogram, // CHANGED: Conditional clear like viewport
            );

            let samples_this_frame = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * self.config_manager.system_settings().iterations_per_thread as u64;
            total_rendered += samples_this_frame;
            batch_frame_count += 1;

            // Accumulate only when batch is complete (match viewport behavior)
            let should_accumulate = batch_frame_count >= BATCH_SIZE;
            if should_accumulate {
                accumulation_count += 1;

                // Pass total samples in batch like viewport does (multiply by batch size)
                let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
                temp_renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);

                batch_frame_count = 0; // Reset batch counter
            }

            self.gpu.queue.submit(std::iter::once(encoder.finish()));

            if total_rendered >= target {
                // Final accumulation if we have partial batch
                if batch_frame_count > 0 {
                    let mut final_accum_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("Export Final Batch Accumulation"),
                    });
                    let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                    temp_renderer.accumulate_pass(&mut final_accum_encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
                    self.gpu.queue.submit(std::iter::once(final_accum_encoder.finish()));
                    accumulation_count += 1;
                }
                break;
            }
        }

        let export_render_time = render_start.elapsed().as_secs_f64() * 1000.0;

        println!("Export stats: {} accumulation passes, {} total iterations",
            accumulation_count, total_rendered);

        // Set transparent mode if requested (before tonemap pass)
        if transparent {
            temp_renderer.set_transparent_mode(&self.gpu.queue, true, &config, self.config_manager.system_settings().iterations_per_thread);
        }

        // Final tonemap pass
        let mut final_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Export Final Tonemap"),
        });
        temp_renderer.tonemap_pass(&mut final_encoder);
        self.gpu.queue.submit(std::iter::once(final_encoder.finish()));

        // Read pixels
        let pixels_future = temp_renderer.read_fractal_pixels(&self.gpu.device, &self.gpu.queue, transparent, config.background_color);

        match pollster::block_on(pixels_future) {
            Ok((width, height, rgba_data)) => {
                // Build metadata
                let metadata = crate::png_metadata::PngMetadata::from_app_state(
                    width,
                    height,
                    total_rendered,
                    export_render_time,
                    self.config_manager.system_settings().iterations_per_thread,
                    config.speed_factor,
                    &config,
                );

                // Encode PNG
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
                                    path.display(), width, height, export_render_time / 1000.0);
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to encode PNG: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to capture pixels: {}", e),
        }
    }
}
