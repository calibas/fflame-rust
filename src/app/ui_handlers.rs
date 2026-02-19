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
        self.handle_new_flame(ui_response);
        self.handle_random_flame(ui_response);
        self.handle_generated_flame(ui_response);
        self.handle_palette_operations(ui_response);
        self.handle_file_operations(ui_response);
        self.handle_undo_redo(ui_response);
        self.handle_panel_requests(ui_response);
        self.handle_preset_selection(ui_response);
        // NOTE: Export requests handled in mod.rs due to complexity
        self.handle_animation_requests(ui_response);
        self.handle_animation_seek(ui_response);
        self.handle_path_filters(ui_response);
        #[cfg(feature = "api")]
        self.handle_save_online(ui_response);
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
                    if let Err(e) = self.load_config_with_undo(config, "history.action.import_config".to_string()) {
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
            let config = self.config_manager.active_config();
            let insert_index = config.flame.transforms.len();
            let xaos_before = config.flame.xaos.clone();

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
                xaos_before,
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
                let xaos_before = config.flame.xaos.clone();
                let change = crate::config::ConfigChange::delete_transform_snapshot(
                    idx,
                    deleted_transform,
                    xaos_before,
                    format!("Delete Transform {}", idx + 1),
                );

                if let Err(e) = self.config_manager.apply_structural_change(change) {
                    eprintln!("Failed to delete transform: {}", e);
                } else {
                    // Update app state from config
                    self.flame = self.config_manager.active_config().flame.clone();

                    // Update animation track paths to reflect the removed transform
                    if let Some(ref mut animation) = self.animation_controller.animation {
                        let removed_count = animation.on_transform_removed(idx);
                        if removed_count > 0 {
                            log::info!("Removed {} animation tracks targeting deleted transform {}", removed_count, idx + 1);
                        }
                    }
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

                // Create specialized snapshot — clone duplicates the source's xaos relationships
                let xaos_before = config.flame.xaos.clone();
                let change = crate::config::ConfigChange::clone_transform_snapshot(
                    insert_idx,
                    cloned_transform,
                    xaos_before,
                    idx,
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

    /// Handle new flame creation (reset to default)
    fn handle_new_flame(&mut self, ui_response: &UiResponse) {
        if ui_response.new_flame_requested {
            // Create a proper default config with one identity transform + linear variation
            let new_config = crate::resources::create_default_preset();

            // Load the default config with undo support
            if let Err(e) = self.load_config_with_undo(new_config, "history.action.new_flame".to_string()) {
                eprintln!("Failed to create new flame: {}", e);
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
                    new_config.palette = palette.clone();
                }
            }

            // Load the random config with undo support
            if let Err(e) = self.load_config_with_undo(new_config, "history.action.random_flame".to_string()) {
                eprintln!("Failed to load random flame: {}", e);
            }
        }
    }

    /// Handle generated flame from Random Generator panel (single)
    fn handle_generated_flame(&mut self, ui_response: &UiResponse) {
        if let Some(ref flame) = ui_response.generated_flame {
            // Create a new config with the generated flame
            let mut new_config = FractalConfig::default();
            new_config.flame = flame.clone();

            // Use a random palette from the library
            if self.palette_library.len() > 0 {
                let palette_idx = rand::random::<usize>() % self.palette_library.len();
                if let Some(palette) = self.palette_library.get(palette_idx) {
                    new_config.palette = palette.clone();
                }
            }

            // Load the generated config with undo support
            if let Err(e) = self.load_config_with_undo(new_config, "history.action.random_flame".to_string()) {
                eprintln!("Failed to load generated flame: {}", e);
            }
        }

        // Handle generated batch from Random Generator panel
        // Configs are already self-contained with palettes embedded
        if let Some(ref configs) = ui_response.generated_batch {
            if !configs.is_empty() {
                log::info!("Loading {} generated flames into Fractal Browser", configs.len());

                // Load all configs into Fractal Browser (they already have palettes embedded)
                self.egui_layer.load_batch_into_fractal_browser(configs.clone());

                // Open the Fractal Browser panel (auto-switches to Batch tab)
                use crate::ui::workspace::PanelType;
                let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::FractalBrowser, ctx);
            }
        }
    }

    /// Handle palette operations (export, import, save)
    fn handle_palette_operations(&mut self, ui_response: &UiResponse) {
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

        // Handle palette save to library (persists across sessions)
        if let Some(ref palette) = ui_response.palette_save_to_library {
            match self.palette_library.save_to_custom_library(palette.clone()) {
                Ok(()) => {
                    log::info!("Palette '{}' saved to Custom library", palette.name);
                }
                Err(e) => {
                    log::error!("Failed to save palette to library: {}", e);
                }
            }
        }

        // Handle palette delete from library
        if let Some(ref name) = ui_response.palette_delete_from_library {
            match self.palette_library.delete_from_custom_library(name) {
                Ok(true) => {
                    log::info!("Palette '{}' deleted from Custom library", name);
                }
                Ok(false) => {
                    log::warn!("Palette '{}' not found in Custom library", name);
                }
                Err(e) => {
                    log::error!("Failed to delete palette from library: {}", e);
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
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation, config.palette_squeeze);
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
                                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation, config.palette_squeeze);
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
                                if let Err(e) = self.load_config_with_undo(config, "history.action.load_config".to_string()) {
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
                                println!("Found {} configs in {}, loading first and opening Fractal Browser", configs.len(), filename);

                                // Load all configs into Fractal Browser (Files tab)
                                self.egui_layer.load_file_into_fractal_browser(path.clone());

                                // Load the first config
                                let first_config = configs.into_iter().next().unwrap();
                                if let Err(e) = self.load_config_with_undo(first_config, "history.action.load_config".to_string()) {
                                    eprintln!("Failed to load config: {}", e);
                                }

                                // Open the Fractal Browser panel
                                use crate::ui::workspace::PanelType;
                                let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::FractalBrowser, ctx);
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
                                        if let Err(e) = self.load_config_with_undo(config, "history.action.import_apophysis".to_string()) {
                                            eprintln!("Failed to import flame: {}", e);
                                        } else {
                                            println!("Imported Apophysis flame from: {}", path.display());
                                        }
                                    } else {
                                        // Multiple flames: load first one and open Fractal Browser
                                        let filename = path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("file")
                                            .to_string();
                                        println!("Found {} flames in {}, loading first and opening Fractal Browser", configs.len(), filename);

                                        // Load all configs into Fractal Browser (Files tab)
                                        self.egui_layer.load_batch_into_fractal_browser(configs.clone());

                                        // Load the first config
                                        let first_config = configs.into_iter().next().unwrap();
                                        if let Err(e) = self.load_config_with_undo(first_config, "history.action.import_apophysis".to_string()) {
                                            eprintln!("Failed to import flame: {}", e);
                                        }

                                        // Open the Fractal Browser panel
                                        use crate::ui::workspace::PanelType;
                                        let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::FractalBrowser, ctx);
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

        // Handle file browser open request (loads into FractalBrowser Files tab)
        if ui_response.file_browser_open_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .pick_file()
                {
                    self.egui_layer.load_file_into_fractal_browser(path);
                    // Open the Fractal Browser panel (auto-switches to Files tab)
                    use crate::ui::workspace::PanelType;
                    let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::FractalBrowser, ctx);
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: use native file picker (no extra dialogs)
                let ctx = self.egui_layer.ctx.clone();
                super::trigger_browser_file_picker(".fflame", ctx, "pending_file_browser_json_raw");
            }
        }

        // Handle audio file loading
        if ui_response.load_audio_file {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg"])
                    .pick_file()
                {
                    match self.audio_manager.load_file(&path) {
                        Ok(()) => {
                            log::info!("Loaded audio file: {}", path.display());

                            // Also load into AudioPlayer for playback
                            if let Some(audio_data) = self.audio_manager.audio_data() {
                                self.audio_player.load(audio_data.clone());
                                log::info!("Audio loaded into player for playback");
                            }

                            // Auto-analyze after loading
                            self.audio_manager.analyze();
                            log::info!("Audio analysis complete: {} signals available",
                                self.audio_manager.available_signals().len());

                            // Import signals into SignalManager for animation access
                            self.signal_manager.import_from_producer(&self.audio_manager);
                            log::info!("Imported {} signals into SignalManager",
                                self.signal_manager.len());
                        }
                        Err(e) => {
                            log::error!("Failed to load audio file: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: use native file picker (binary variant for audio)
                let ctx = self.egui_layer.ctx.clone();
                super::trigger_browser_file_picker_binary(".mp3,.wav,.flac,.ogg", ctx, "pending_audio_load_bytes");
            }
        }

        // Handle signal file loading (.signal binary format)
        if ui_response.load_signal_file {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Signal Files", &["signal"])
                    .pick_file()
                {
                    match self.signal_manager.load_from_file(&path) {
                        Ok(signal) => {
                            let name = signal.name.clone();
                            log::info!("Loaded signal file: {} ({})", name, path.display());
                            self.egui_layer.signal_panel_state.loaded_signal_files.push(name);
                        }
                        Err(e) => {
                            log::error!("Failed to load signal file: {}", e);
                        }
                    }
                }
            }
        }

        // Handle signal file saving
        if let Some(ref signal_name) = ui_response.save_signal_file {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let default_name = format!("{}.signal", signal_name);
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Signal Files", &["signal"])
                    .set_file_name(&default_name)
                    .save_file()
                {
                    match self.signal_manager.save_to_file(signal_name, &path) {
                        Ok(()) => {
                            log::info!("Saved signal '{}' to {}", signal_name, path.display());
                        }
                        Err(e) => {
                            log::error!("Failed to save signal file: {}", e);
                        }
                    }
                }
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
            let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::PaletteEditor, ctx);
        }
        if ui_response.open_palette_library {
            use crate::ui::workspace::PanelType;
            let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::PaletteLibrary, ctx);
        }
        if ui_response.open_config_dialog {
            use crate::ui::workspace::PanelType;
            let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::ConfigDialog, ctx);
        }
        if ui_response.open_triangle_editor {
            use crate::ui::workspace::PanelType;
            self.workspace.activate_panel(PanelType::TriangleEditor);
        }
        if ui_response.open_preset_library {
            use crate::ui::workspace::PanelType;
            use crate::ui::fractal_browser::BrowserTab;
            self.egui_layer.switch_fractal_browser_tab(BrowserTab::Presets);
            let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::FractalBrowser, ctx);
        }
        if ui_response.open_random_generator {
            use crate::ui::workspace::PanelType;
            let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::RandomGenerator, ctx);
        }
    }

    /// Handle preset selection from Preset Library panel
    fn handle_preset_selection(&mut self, ui_response: &UiResponse) {
        if let Some(ref config) = ui_response.selected_preset_config {
            if let Err(e) = self.load_config_with_undo(config.clone(), "history.action.load_preset".to_string()) {
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
                        if let Err(e) = self.load_config_with_undo(config, "history.action.load_config".to_string()) {
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
                            if let Err(e) = self.load_config_with_undo(config, "history.action.import_apophysis".to_string()) {
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
                            let description = format!("history.action.load_animation_config|name={}", animation.name);
                            if let Err(e) = self.load_config_with_undo(config.clone(), description) {
                                log::error!("Failed to load animation's embedded config: {}", e);
                            }
                        }
                        let generators = animation.generators.clone();
                        let duration = animation.duration;
                        self.animation_controller.load(animation);
                        self.egui_layer.signal_panel_state.restore_generators(
                            generators, &mut self.signal_manager, duration,
                        );
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
                // Load the JSON into the Fractal Browser panel (Files tab)
                self.egui_layer.load_json_into_fractal_browser(&json, "file");
                // Open the Fractal Browser panel
                use crate::ui::workspace::PanelType;
                let ctx = &self.egui_layer.ctx; self.workspace.open_floating_panel(PanelType::FractalBrowser, ctx);
            }

            // Check for pending audio file (binary bytes from native file picker)
            if let Some(bytes) = self.egui_layer.ctx.data_mut(|data| {
                data.remove_temp::<Vec<u8>>(egui::Id::new("pending_audio_load_bytes"))
            }) {
                match self.audio_manager.load_bytes(&bytes) {
                    Ok(()) => {
                        log::info!("Loaded audio file ({} bytes)", bytes.len());

                        // Also load into AudioPlayer for playback
                        if let Some(audio_data) = self.audio_manager.audio_data() {
                            self.audio_player.load(audio_data.clone());
                            log::info!("Audio loaded into player for playback");
                        }

                        // Auto-analyze after loading
                        self.audio_manager.analyze();
                        log::info!("Audio analysis complete: {} signals available",
                            self.audio_manager.available_signals().len());

                        // Import signals into SignalManager for animation access
                        self.signal_manager.import_from_producer(&self.audio_manager);
                        log::info!("Imported {} signals into SignalManager",
                            self.signal_manager.len());
                    }
                    Err(e) => {
                        log::error!("Failed to load audio file: {}", e);
                    }
                }
            }
        }
    }

    /// Handle animation timeline scrubbing
    fn handle_animation_seek(&mut self, ui_response: &UiResponse) {
        if ui_response.animation_seek_changed {
            // Evaluate current frame and apply values
            let frame_values = self.animation_controller.evaluate_frame(Some(&self.signal_manager));

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
            // Note: Overwrite mode is handled by update_overwrite_mode based on recorded actions
            // This ensures tone-mapping-only changes don't incorrectly enable overwrite mode
            self.flame = self.config_manager.active_config().flame.clone();

            // Pause audio during scrubbing to avoid scratching artifacts (desktop)
            // and AudioBufferSourceNode recreation storm (WASM)
            if self.animation_controller.sync_audio && self.audio_player.has_audio() {
                self.audio_player.pause();
            }
        }

        // Only reset accumulation when drag stops or on discrete actions (frame step, click to seek)
        // This provides smooth preview during scrubber drag, then clean rebuild when released
        if ui_response.animation_seek_drag_stopped {
            self.config_manager.request_reset();

            // Seek audio to final position and resume playback after scrub finishes
            if self.animation_controller.sync_audio && self.audio_player.has_audio() {
                self.audio_player.seek(self.animation_controller.current_time);
                if self.animation_controller.is_playing() {
                    let _ = self.audio_player.play();
                }
            }
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

    /// Handle "Save Online" — save current flame to the API
    #[cfg(feature = "api")]
    fn handle_save_online(&mut self, ui_response: &UiResponse) {
        // Check for completed async save
        if self.api_save_in_progress {
            if let Ok(mut result) = self.api_save_result.lock() {
                if let Some(save_result) = result.take() {
                    self.api_save_in_progress = false;
                    match save_result {
                        Ok(flame_id) => {
                            log::info!("Flame saved online: {}", flame_id);
                        }
                        Err(e) => {
                            log::error!("Failed to save flame online: {}", e);
                        }
                    }
                }
            }
        }

        // Handle new save request
        if ui_response.save_online_requested && !self.api_save_in_progress {
            let config = self.export_config();
            let name = config.flame.name.clone();
            let result_slot = self.api_save_result.clone();
            self.api_save_in_progress = true;

            #[cfg(target_arch = "wasm32")]
            {
                wasm_bindgen_futures::spawn_local(async move {
                    let save_result = save_flame_online(config, &name).await;
                    if let Ok(mut slot) = result_slot.lock() {
                        *slot = Some(save_result);
                    }
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                log::warn!("Save Online not yet implemented for desktop");
                self.api_save_in_progress = false;
                if let Ok(mut slot) = self.api_save_result.lock() {
                    *slot = Some(Err("Save Online not yet implemented for desktop".to_string()));
                }
            }
        }
    }
}

/// Save a flame to the API (async helper for WASM).
///
/// Reads the auth token from localStorage and the API base URL,
/// then calls the API to save the flame.
#[cfg(all(feature = "api", target_arch = "wasm32"))]
async fn save_flame_online(
    config: crate::config::FractalConfig,
    name: &str,
) -> Result<String, String> {
    use crate::api::ApiState;

    // Read token from localStorage (set by JS auth wrapper)
    let window = web_sys::window().ok_or("No window")?;
    let storage = window
        .local_storage()
        .map_err(|_| "Failed to access localStorage")?
        .ok_or("No localStorage")?;

    let token = storage
        .get_item("fflame_auth_token")
        .map_err(|_| "Failed to read token")?
        .ok_or("Not signed in — click Sign In first")?;

    // Read API base URL (default to same origin, fall back to localhost)
    let base_url = storage
        .get_item("fflame_api_base_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    // Create a temporary ApiState and save
    let mut api = ApiState::new(&base_url);
    api.set_token(&token);
    api.save_flame(&config, Some(name))
        .await
        .map_err(|e| e.to_string())
}
