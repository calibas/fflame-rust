//! UI Response Handlers
//!
//! Processes UI responses from egui and updates application state accordingly.
//! Extracted from mod.rs to improve maintainability.

use crate::config::FractalConfig;
use crate::scene::transforms::Transform;
use crate::ui::UiResponse;


#[cfg(target_arch = "wasm32")]
use super::trigger_browser_download;

use super::App;

impl App {
    /// Process all UI responses and update application state
    ///
    /// This is the main dispatcher that handles all UI-triggered actions.
    /// Called from render() after UI has been rendered.
    pub(super) fn handle_ui_responses(&mut self, ui_response: &UiResponse) {
        self.handle_config_operations(ui_response);
        self.handle_transform_operations(ui_response);
        self.handle_random_flame(ui_response);
        self.handle_palette_operations(ui_response);
        self.handle_file_operations(ui_response);
        self.handle_undo_redo(ui_response);
        self.handle_panel_requests(ui_response);
        self.handle_preset_selection(ui_response);
        // NOTE: Export requests handled in mod.rs due to complexity
        self.handle_animation_requests(ui_response);
        self.handle_animation_seek(ui_response);
        self.handle_path_filters(ui_response);
    }

    /// Handle config export, save, and import operations
    fn handle_config_operations(&mut self, ui_response: &UiResponse) {
        // Handle config export to clipboard
        if ui_response.config_export_requested.is_some() {
            let config = self.export_config();
            if let Ok(json) = config.to_json() {
                self.egui_layer.ctx.copy_text(json);
            }
        }

        // Handle config save to file
        if ui_response.config_save_file_requested {
            let config = self.export_config();

            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .set_file_name("fractal.fflame")
                    .save_file()
                {
                    if let Err(e) = config.save_to_file(&path) {
                        eprintln!("Failed to save config: {}", e);
                    } else {
                        println!("Config saved to: {}", path.display());
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                if let Ok(json) = config.to_json() {
                    let filename = format!("{}.fflame", config.flame.name.to_lowercase().replace(' ', "_"));
                    if let Err(e) = trigger_browser_download(json.as_bytes(), &filename, "application/json") {
                        log::error!("Failed to trigger download: {}", e);
                    }
                }
            }
        }

        // Handle config import
        if let Some(ref json) = ui_response.config_import_requested {
            match FractalConfig::from_json(json) {
                Ok(config) => {
                    if let Err(e) = self.load_config_with_undo(config, "Import Config".to_string()) {
                        eprintln!("Failed to import config: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to import config: {}", e);
                }
            }
        }
    }

    /// Handle add, delete, and clone transform operations
    fn handle_transform_operations(&mut self, ui_response: &UiResponse) {
        // Handle add transform
        if ui_response.add_transform {
            let insert_index = self.config_manager.active_config().flame.transforms.len();

            // Create a new default transform with identity affine and linear variation
            let mut new_transform = Transform::default();
            // Linear variation with weight 1.0
            new_transform.set_variation("linear", 1.0);
            new_transform.color = 0.5;  // Mid-palette position
            new_transform.color_speed = 0.5;

            // Create specialized snapshot for efficient undo/redo
            let change = crate::config::ConfigChange::add_transform_snapshot(
                insert_index,
                new_transform,
                "Add Transform".to_string(),
            );

            if let Err(e) = self.config_manager.apply_structural_change(change) {
                eprintln!("Failed to add transform: {}", e);
            } else {
                // Update app state from config
                self.flame = self.config_manager.active_config().flame.clone();
            }
        }

        // Handle delete transform
        if let Some(idx) = ui_response.delete_transform {
            let config = self.config_manager.active_config();

            if config.flame.transforms.len() > 1 && idx < config.flame.transforms.len() {
                // Get the transform before deleting
                let deleted_transform = config.flame.transforms[idx].clone();

                // Create specialized snapshot for efficient undo/redo
                let change = crate::config::ConfigChange::delete_transform_snapshot(
                    idx,
                    deleted_transform,
                    format!("Delete Transform {}", idx + 1),
                );

                if let Err(e) = self.config_manager.apply_structural_change(change) {
                    eprintln!("Failed to delete transform: {}", e);
                } else {
                    // Update app state from config
                    self.flame = self.config_manager.active_config().flame.clone();
                }
            }
        }

        // Handle clone transform
        if let Some(idx) = ui_response.clone_transform {
            let config = self.config_manager.active_config();

            if idx < config.flame.transforms.len() {
                // Clone the transform
                let cloned_transform = config.flame.transforms[idx].clone();
                // Insert after the original transform
                let insert_idx = idx + 1;

                // Create specialized snapshot for efficient undo/redo
                let change = crate::config::ConfigChange::add_transform_snapshot(
                    insert_idx,
                    cloned_transform,
                    format!("Clone Transform {}", idx + 1),
                );

                if let Err(e) = self.config_manager.apply_structural_change(change) {
                    eprintln!("Failed to clone transform: {}", e);
                } else {
                    // Update app state from config
                    self.flame = self.config_manager.active_config().flame.clone();
                }
            }
        }
    }

    /// Handle random flame generation
    fn handle_random_flame(&mut self, ui_response: &UiResponse) {
        if ui_response.random_flame_requested {
            let random_flame = crate::scene::randomize::generate_random_flame();

            // Create a new config with the random flame
            let mut new_config = FractalConfig::default();
            new_config.flame = random_flame;

            // Use a random palette from the library
            if self.palette_library.len() > 0 {
                let palette_idx = rand::random::<usize>() % self.palette_library.len();
                if let Some(palette) = self.palette_library.get(palette_idx) {
                    new_config.palette = Some(palette.clone());
                }
            }

            // Load the random config with undo support
            if let Err(e) = self.load_config_with_undo(new_config, "Random Flame".to_string()) {
                eprintln!("Failed to load random flame: {}", e);
            }
        }
    }

    /// Handle custom palette from editor or library
    fn handle_palette_operations(&mut self, ui_response: &UiResponse) {
        // Handle custom palette from editor or library
        if let Some(ref custom_pal) = ui_response.custom_palette {
            // Add or update palette in library (prevents duplicates)
            let _palette_index = self.palette_library.add_or_update(custom_pal.clone());

            // Apply the palette to the config via ConfigManager
            if let Ok(update) = self.config_manager.update_param(
                crate::config::ConfigPath::Palette,
                crate::config::ConfigValue::Palette(custom_pal.clone()),
            ) {
                // Update renderer if needed (ColorOnly or IterationReset)
                if matches!(update, crate::config::UpdateType::ColorOnly | crate::config::UpdateType::IterationReset) {
                    let config = self.config_manager.active_config();
                    if let Some(ref mut renderer) = self.flame_renderer {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &custom_pal, config.palette_rotation);
                    }
                }
            }
        }

        // Handle palette export to clipboard
        if let Some(ref palette) = ui_response.palette_export_json {
            if let Ok(json) = serde_json::to_string_pretty(palette) {
                self.egui_layer.ctx.copy_text(json);
            }
        }

        // Handle palette save to file
        if let Some(ref palette) = ui_response.palette_save_file {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Palette", &["palette"])
                    .set_file_name("palette.palette")
                    .save_file()
                {
                    if let Ok(json) = serde_json::to_string_pretty(&palette) {
                        if let Err(e) = std::fs::write(&path, json) {
                            eprintln!("Failed to save palette: {}", e);
                        } else {
                            println!("Palette saved to: {}", path.display());
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog
                if let Ok(json) = serde_json::to_string_pretty(&palette) {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(file_handle) = rfd::AsyncFileDialog::new()
                            .add_filter("Palette", &["palette"])
                            .set_file_name("palette.palette")
                            .save_file()
                            .await
                        {
                            let _ = file_handle.write(json.as_bytes()).await;
                        }
                    });
                }
            }
        }

        // Handle palette import from JSON
        if let Some(ref json) = ui_response.palette_import_json {
            match serde_json::from_str::<crate::scene::palette::Palette>(json) {
                Ok(mut palette) => {
                    // Check if palette with same name exists (case-insensitive)
                    let existing_idx = self.palette_library.iter()
                        .position(|p| p.name.to_lowercase() == palette.name.to_lowercase());

                    if existing_idx.is_some() {
                        // Generate unique name with (Copy) or (Copy N) suffix
                        let base_name = palette.name.clone();
                        let mut counter = 1;
                        let mut new_name = format!("{} (Copy)", base_name);

                        while self.palette_library.iter().any(|p| p.name.to_lowercase() == new_name.to_lowercase()) {
                            counter += 1;
                            new_name = format!("{} (Copy {})", base_name, counter);
                        }

                        palette.name = new_name;
                        palette.built_in = false; // Mark as custom
                    } else {
                        palette.built_in = false; // Mark as custom
                    }

                    // Add to library (now guaranteed to have unique name)
                    let _palette_idx = self.palette_library.add_or_update(palette.clone());

                    // Update palette editor with the new palette
                    self.egui_layer.update_palette_editor(palette.clone());

                    // Set as active palette in config (this is what the UI checks)
                    let _ = self.config_manager.update_param(
                        crate::config::ConfigPath::Palette,
                        crate::config::ConfigValue::Palette(palette.clone())
                    );

                    // Update renderer
                    let config = self.config_manager.active_config();
                    if let Some(ref mut renderer) = self.flame_renderer {
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to import palette: {}", e);
                }
            }
        }

        // Handle palette load from file
        if ui_response.palette_load_file {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Palette", &["palette"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(json) => {
                            match serde_json::from_str::<crate::scene::palette::Palette>(&json) {
                                Ok(palette) => {
                                    // Update palette editor
                                    self.egui_layer.update_palette_editor(palette.clone());

                                    // Add or update in library (prevents duplicates)
                                    let palette_idx = self.palette_library.add_or_update(palette.clone());
                                    // Set to the palette
                                    let _ = self.config_manager.update_param(
                                        crate::config::ConfigPath::PaletteIndex,
                                        (palette_idx as u32).into()
                                    );

                                    // Update renderer
                                    let config = self.config_manager.active_config();
                                    if let Some(ref mut renderer) = self.flame_renderer {
                                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation);
                                    }

                                    println!("Palette loaded from: {}", path.display());
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse palette file: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read palette file: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: async file dialog
                let ctx = self.egui_layer.ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(file_handle) = rfd::AsyncFileDialog::new()
                        .add_filter("Palette", &["palette"])
                        .pick_file()
                        .await
                    {
                        let contents = file_handle.read().await;
                        let json = String::from_utf8_lossy(&contents).to_string();
                        // Copy to clipboard so user can paste it
                        ctx.copy_text(json);
                        log::info!("Palette loaded to clipboard - paste to import");
                    }
                });
            }
        }
    }

    /// Handle file browser and config/Apophysis file loading
    fn handle_file_operations(&mut self, ui_response: &UiResponse) {
        // Handle config load from file
        if ui_response.config_load_file_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .pick_file()
                {
                    match FractalConfig::load_multi_from_file(&path) {
                        Ok(configs) => {
                            if configs.is_empty() {
                                eprintln!("No configurations found in file");
                            } else if configs.len() == 1 {
                                // Single config: load directly
                                let config = configs.into_iter().next().unwrap();
                                if let Err(e) = self.load_config_with_undo(config, "Load Config".to_string()) {
                                    eprintln!("Failed to load config: {}", e);
                                } else {
                                    println!("Config loaded from: {}", path.display());
                                }
                            } else {
                                // Multiple configs: load first one and open File Browser
                                let filename = path.file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("file")
                                    .to_string();
                                println!("Found {} configs in {}, loading first and opening File Browser", configs.len(), filename);

                                // Load all configs into File Browser
                                self.egui_layer.load_configs_into_browser(configs.clone(), &filename);

                                // Load the first config
                                let first_config = configs.into_iter().next().unwrap();
                                if let Err(e) = self.load_config_with_undo(first_config, "Load Config".to_string()) {
                                    eprintln!("Failed to load config: {}", e);
                                }

                                // Open the File Browser panel
                                use crate::ui::workspace::PanelType;
                                self.workspace.open_floating_panel(PanelType::FileBrowser);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to load config: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: native file picker - no extra dialogs
                let ctx = self.egui_layer.ctx.clone();
                super::trigger_browser_file_picker(".fflame", ctx, "pending_config_load_raw");
            }
        }

        // Handle Apophysis .flame import
        if ui_response.apophysis_import_file_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Apophysis Flame", &["flame"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(xml) => {
                            match crate::apophysis_xml::parse_flame_xml(&xml) {
                                Ok(configs) => {
                                    if configs.is_empty() {
                                        eprintln!("No flames found in file");
                                    } else if configs.len() == 1 {
                                        // Single flame: import directly
                                        let config = configs.into_iter().next().unwrap();
                                        if let Err(e) = self.load_config_with_undo(config, "Import Apophysis Flame".to_string()) {
                                            eprintln!("Failed to import flame: {}", e);
                                        } else {
                                            println!("Imported Apophysis flame from: {}", path.display());
                                        }
                                    } else {
                                        // Multiple flames: load first one and open File Browser
                                        let filename = path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("file")
                                            .to_string();
                                        println!("Found {} flames in {}, loading first and opening File Browser", configs.len(), filename);

                                        // Load all configs into File Browser
                                        self.egui_layer.load_configs_into_browser(configs.clone(), &filename);

                                        // Load the first config
                                        let first_config = configs.into_iter().next().unwrap();
                                        if let Err(e) = self.load_config_with_undo(first_config, "Import Apophysis Flame".to_string()) {
                                            eprintln!("Failed to import flame: {}", e);
                                        }

                                        // Open the File Browser panel
                                        use crate::ui::workspace::PanelType;
                                        self.workspace.open_floating_panel(PanelType::FileBrowser);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse Apophysis XML: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read file: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: native file picker - no extra dialogs
                let ctx = self.egui_layer.ctx.clone();
                super::trigger_browser_file_picker(".flame", ctx, "pending_apophysis_import_raw");
            }
        }

        // Handle file browser open request
        if ui_response.file_browser_open_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .pick_file()
                {
                    self.egui_layer.load_file_into_browser(path);
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: use native file picker (no extra dialogs)
                let ctx = self.egui_layer.ctx.clone();
                super::trigger_browser_file_picker(".fflame", ctx, "pending_file_browser_json_raw");
            }
        }
    }

    /// Handle undo/redo from UI buttons
    fn handle_undo_redo(&mut self, ui_response: &UiResponse) {
        if ui_response.undo_requested {
            self.undo();
        }
        if ui_response.redo_requested {
            self.redo();
        }
    }

    /// Handle panel open requests
    fn handle_panel_requests(&mut self, ui_response: &UiResponse) {
        if ui_response.open_palette_editor {
            use crate::ui::workspace::PanelType;
            self.workspace.open_floating_panel(PanelType::PaletteEditor);
        }
        if ui_response.open_config_dialog {
            use crate::ui::workspace::PanelType;
            self.workspace.open_floating_panel(PanelType::ConfigDialog);
        }
        if ui_response.open_triangle_editor {
            use crate::ui::workspace::PanelType;
            self.workspace.activate_panel(PanelType::TriangleEditor);
        }
    }

    /// Handle preset selection from Preset Library panel
    fn handle_preset_selection(&mut self, ui_response: &UiResponse) {
        if let Some(ref config) = ui_response.selected_preset_config {
            if let Err(e) = self.load_config_with_undo(config.clone(), "Load Preset".to_string()) {
                log::error!("Failed to load preset: {}", e);
            } else {
                log::info!("Preset loaded successfully");
            }
        }
    }

    /// Handle animation-related responses (WASM async loads)
    fn handle_animation_requests(&mut self, _ui_response: &UiResponse) {
        // Check for pending config/Apophysis imports from WASM async file dialogs
        #[cfg(target_arch = "wasm32")]
        {
            // Check for pending config load (raw JSON text from native file picker)
            if let Some(json) = self.egui_layer.ctx.data_mut(|data| {
                data.remove_temp::<String>(egui::Id::new("pending_config_load_raw"))
            }) {
                match serde_json::from_str::<FractalConfig>(&json) {
                    Ok(config) => {
                        if let Err(e) = self.load_config_with_undo(config, "Load Config".to_string()) {
                            log::error!("Failed to load config: {}", e);
                        } else {
                            log::info!("Config loaded successfully");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to parse config JSON: {}", e);
                    }
                }
            }

            // Check for pending Apophysis import (raw XML text from native file picker)
            if let Some(xml) = self.egui_layer.ctx.data_mut(|data| {
                data.remove_temp::<String>(egui::Id::new("pending_apophysis_import_raw"))
            }) {
                match crate::apophysis_xml::parse_flame_xml(&xml) {
                    Ok(configs) => {
                        if let Some(config) = configs.into_iter().next() {
                            if let Err(e) = self.load_config_with_undo(config, "Import Apophysis Flame".to_string()) {
                                log::error!("Failed to import Apophysis flame: {}", e);
                            } else {
                                log::info!("Apophysis flame imported successfully");
                            }
                        } else {
                            log::error!("No flames found in Apophysis file");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to parse Apophysis XML: {}", e);
                    }
                }
            }

            // Check for pending animation load (raw JSON text from native file picker)
            if let Some(json) = self.egui_layer.ctx.data_mut(|data| {
                data.remove_temp::<String>(egui::Id::new("pending_animation_load_raw"))
            }) {
                match crate::animation::Animation::from_json(&json) {
                    Ok(animation) => {
                        // If animation has embedded config, load it first
                        if let Some(ref config) = animation.base_config {
                            log::info!("Animation '{}' has embedded config, loading it", animation.name);
                            let description = format!("Load Animation: {}", animation.name);
                            if let Err(e) = self.load_config_with_undo(config.clone(), description) {
                                log::error!("Failed to load animation's embedded config: {}", e);
                            }
                        }
                        self.animation_controller.load(animation);
                        log::info!("Animation loaded successfully");
                    }
                    Err(e) => {
                        log::error!("Failed to parse animation JSON: {}", e);
                    }
                }
            }

            // Check for pending file browser JSON (raw text from native file picker)
            if let Some(json) = self.egui_layer.ctx.data_mut(|data| {
                data.remove_temp::<String>(egui::Id::new("pending_file_browser_json_raw"))
            }) {
                // Load the JSON into the file browser panel
                match FractalConfig::from_json_multi(&json) {
                    Ok(configs) => {
                        if configs.is_empty() {
                            log::error!("File contains no configurations");
                        } else {
                            log::info!("Loaded {} config(s) from file", configs.len());
                            self.egui_layer.load_configs_into_browser(configs, "file");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to parse config JSON: {}", e);
                    }
                }
            }
        }
    }

    /// Handle animation timeline scrubbing
    fn handle_animation_seek(&mut self, ui_response: &UiResponse) {
        if ui_response.animation_seek_changed {
            // Evaluate current frame and apply values
            let frame_values = self.animation_controller.evaluate_frame();

            for (path_str, json_value) in frame_values {
                if let Some(path) = crate::config::ConfigPath::from_string_key(&path_str) {
                    if let Some(config_value) = crate::config::json_to_config_value(&json_value, &path) {
                        if let Err(e) = self.config_manager.update_param_silent(path, config_value) {
                            log::warn!("Animation scrub: failed to update {}: {}", path_str, e);
                        }
                    }
                }
            }

            // Sync flame and trigger refresh
            self.flame = self.config_manager.active_config().flame.clone();
            self.use_overwrite_next_frame = true;
        }
    }

    /// Handle path filter changes from Path Editor panel
    fn handle_path_filters(&mut self, ui_response: &UiResponse) {
        if let Some(ref filters) = ui_response.path_filters_changed {
            if let Some(ref mut renderer) = self.flame_renderer {
                log::info!("Path filters updated: {} filters", filters.len());
                renderer.set_path_filters(filters.clone());
                renderer.update_path_features(&self.gpu.device, &self.gpu.queue, &self.config_manager.active_config().flame);
                self.config_manager.request_reset();
            }
        }
    }
}
