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
            // Try to find if this palette already exists in library by name
            let mut found_index = None;
            for (i, lib_palette) in self.palette_library.iter().enumerate() {
                if lib_palette.name == palette.name {
                    found_index = Some(i);
                    break;
                }
            }

            if found_index.is_some() {
                // Palette exists, no need to update (library is stable)
            } else {
                // New palette, add to library
                self.palette_library.add(palette.clone());
            }

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
            let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Config Import Encoder"),
            });

            if let Some(palette) = config.palette.as_ref().or_else(|| self.palette_library.get(config.palette_index)) {
                renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, palette, config.iterations_per_thread);
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
}
