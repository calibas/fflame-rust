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
        // Get the current palette from the library
        let palette = self.palette_library.get(self.current_palette_index).cloned();

        FractalConfig {
            flame: self.flame.clone(),
            zoom: self.zoom,
            pan_x: self.pan_x,
            pan_y: self.pan_y,
            rotation: self.rotation,
            camera_rotation_x: self.camera_rotation_x,
            camera_rotation_y: self.camera_rotation_y,
            density_scale: self.density_scale,
            speed_factor: self.speed_factor,
            max_iterations: self.max_iterations.unwrap_or(1_000_000_000),
            color_mode: self.color_mode,
            palette_index: self.current_palette_index,
            palette,  // Include actual palette data
            background_color: self.background_color,
            tonemap_mode: self.tonemap_mode,
            tonemap_curve: self.tonemap_curve.clone(),
            use_curve: self.use_curve,
            exposure: self.exposure,
            gamma: self.gamma,
            deterministic_rng: self.deterministic_rng,
            histogram_color_scale: self.histogram_color_scale,
            low_density_smoothing: self.low_density_smoothing,
            density_compression_strength: self.density_compression_strength,
            blend_factor: self.blend_factor,
            target_iterations_per_pixel: self.target_iterations_per_pixel,
            iterations_per_thread: self.iterations_per_thread,
            speed_multiplier: self.speed_multiplier,
        }
    }

    /// Import configuration from FractalConfig
    pub fn import_config(&mut self, config: FractalConfig) {
        // Update app-level state
        self.flame = config.flame.clone();
        self.zoom = config.zoom;
        self.pan_x = config.pan_x;
        self.pan_y = config.pan_y;
        self.rotation = config.rotation;
        self.camera_rotation_x = config.camera_rotation_x;
        self.camera_rotation_y = config.camera_rotation_y;
        self.density_scale = config.density_scale;
        self.speed_factor = config.speed_factor;
        self.max_iterations = Some(config.max_iterations);
        self.color_mode = config.color_mode;
        self.current_palette_index = config.palette_index;
        self.background_color = config.background_color;
        self.tonemap_mode = config.tonemap_mode;
        self.tonemap_curve = config.tonemap_curve.clone();
        self.use_curve = config.use_curve;
        self.exposure = config.exposure;
        self.gamma = config.gamma;
        self.deterministic_rng = config.deterministic_rng;
        self.histogram_color_scale = config.histogram_color_scale;
        self.low_density_smoothing = config.low_density_smoothing;
        self.density_compression_strength = config.density_compression_strength;
        self.blend_factor = config.blend_factor;
        self.target_iterations_per_pixel = config.target_iterations_per_pixel;
        self.iterations_per_thread = config.iterations_per_thread;
        self.speed_multiplier = config.speed_multiplier;

        // If config includes a palette, add it to library or update existing
        if let Some(ref palette) = config.palette {
            // Try to find if this palette already exists in library by name
            let mut found_index = None;
            for (i, lib_palette) in self.palette_library.iter().enumerate() {
                if lib_palette.name == palette.name {
                    found_index = Some(i);
                    break;
                }
            }

            if let Some(idx) = found_index {
                // Palette exists, update it
                self.palette_library.update(idx, palette.clone());
                self.current_palette_index = idx;
            } else {
                // New palette, add to library
                self.palette_library.add(palette.clone());
                self.current_palette_index = self.palette_library.len() - 1;
            }

            // Sync palette editor with the updated palette
            self.egui_layer.update_palette_editor(palette.clone());
        } else {
            // No custom palette in config, sync with library palette
            if let Some(palette) = self.palette_library.get(self.current_palette_index) {
                self.egui_layer.update_palette_editor(palette.clone());
            }
        }

        // Use the comprehensive load_config function to ensure all GPU state is synchronized
        // (including tone mapping, palette, transforms, params, etc.)
        if let Some(ref mut renderer) = self.flame_renderer {
            let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Config Import Encoder"),
            });

            if let Some(palette) = self.palette_library.get(self.current_palette_index) {
                renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, self.iterations_per_thread);
            }

            self.gpu.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    /// Undo to previous state
    pub fn undo(&mut self) {
        if let Ok(_update_type) = self.config_manager.undo() {
            // Sync App state from ConfigManager
            let config = self.config_manager.config();
            self.import_config(config.clone());
        }
    }

    /// Redo to next state
    pub fn redo(&mut self) {
        if let Ok(_update_type) = self.config_manager.redo() {
            // Sync App state from ConfigManager
            let config = self.config_manager.config();
            log::debug!("After redo, ConfigManager has: exposure={}, gamma={}, density_scale={}",
                config.exposure, config.gamma, config.density_scale);
            self.import_config(config.clone());
            log::debug!("After import_config, App has: exposure={}, gamma={}, density_scale={}",
                self.exposure, self.gamma, self.density_scale);
        }
    }

    pub fn can_undo(&self) -> bool {
        self.config_manager.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.config_manager.can_redo()
    }
}
