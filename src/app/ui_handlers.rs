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
        self.handle_attachment_edit(ui_response);
        self.handle_new_flame(ui_response);
        self.handle_random_flame(ui_response);
        self.handle_generated_flame(ui_response);
        self.handle_palette_operations(ui_response);
        self.handle_file_operations(ui_response);
        self.handle_subflame_load(ui_response);
        self.handle_undo_redo(ui_response);
        self.handle_panel_requests(ui_response);
        self.handle_preset_selection(ui_response);
        // NOTE: Export requests handled in mod.rs due to complexity
        self.handle_animation_requests(ui_response);
        self.handle_animation_seek(ui_response);
        self.handle_path_filters(ui_response);
        self.handle_save_online(ui_response);
        self.handle_save_animation_online(ui_response);
        self.handle_load_api_animation(ui_response);
        self.handle_url_load();
        self.handle_variation_fetches();
        self.handle_clear_variation_cache(ui_response);
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
                    .set_parent(self.window.as_ref())
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
                    if let Err(e) = self.load_config_with_undo(config, "history.action.import_config".to_string(), None) {
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
            // Pull length + xaos from the active editing target so an
            // "Add Transform" while editing a subflame inserts at the
            // subflame's end, not the parent's.
            let (insert_index, xaos_before) = {
                let target_flame = self.active_target_flame();
                (target_flame.transforms.len(), target_flame.xaos.clone())
            };

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
            // Same routing: read from the active editing target.
            let (target_count, xaos_before, deleted_transform) = {
                let target_flame = self.active_target_flame();
                if idx >= target_flame.transforms.len() {
                    return;
                }
                (
                    target_flame.transforms.len(),
                    target_flame.xaos.clone(),
                    target_flame.transforms[idx].clone(),
                )
            };

            if target_count > 1 {
                // Create specialized snapshot for efficient undo/redo
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

                    // Animation tracks targeting the deleted transform
                    // are rebound automatically by the structural_changed
                    // hook in gpu_updates — no per-site call needed.
                }
            }
        }

        // Handle clone transform
        if let Some(idx) = ui_response.clone_transform {
            // Source the cloned transform + xaos from the active
            // editing target, same as add/delete.
            let cloned_transform_with_xaos = {
                let target_flame = self.active_target_flame();
                if idx < target_flame.transforms.len() {
                    Some((target_flame.transforms[idx].clone(), target_flame.xaos.clone()))
                } else {
                    None
                }
            };

            if let Some((cloned_transform, xaos_before)) = cloned_transform_with_xaos {
                let insert_idx = idx + 1;
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

        // ---------- LINKED POOL ----------
        if ui_response.add_linked_transform {
            self.apply_pool_full_snapshot("Add Linked Transform", |flame| {
                let mut t = Transform::default();
                t.set_variation("linear", 1.0);
                flame.linked_transforms.push(t);
            });
        }
        if let Some(idx) = ui_response.clone_linked_transform {
            self.apply_pool_full_snapshot(&format!("Clone Linked {}", idx + 1), |flame| {
                if let Some(src) = flame.linked_transforms.get(idx).cloned() {
                    let insert_at = idx + 1;
                    flame.linked_transforms.insert(insert_at.min(flame.linked_transforms.len()), src);
                }
            });
        }
        if let Some(idx) = ui_response.delete_linked_transform {
            self.apply_pool_full_snapshot(&format!("Delete Linked {}", idx + 1), |flame| {
                if idx < flame.linked_transforms.len() {
                    flame.linked_transforms.remove(idx);
                    // Clean up dangling indices in every normal's linked attachment list.
                    for normal in flame.transforms.iter_mut() {
                        normal.linked_attachments.retain(|&a| a != idx);
                        for a in normal.linked_attachments.iter_mut() {
                            if *a > idx { *a -= 1; }
                        }
                    }
                }
            });
        }

        // ---------- FINAL POOL ----------
        if ui_response.add_final_transform {
            self.apply_pool_full_snapshot("Add Final Transform", |flame| {
                let mut t = Transform::default();
                t.set_variation("linear", 1.0);
                let new_idx = flame.final_transforms.len();
                flame.final_transforms.push(t);
                // Auto-attach to every normal transform (per UI spec).
                for normal in flame.transforms.iter_mut() {
                    normal.final_attachments.push(new_idx);
                }
            });
        }
        if let Some(idx) = ui_response.clone_final_transform {
            self.apply_pool_full_snapshot(&format!("Clone Final {}", idx + 1), |flame| {
                if let Some(src) = flame.final_transforms.get(idx).cloned() {
                    let insert_at = (idx + 1).min(flame.final_transforms.len());
                    flame.final_transforms.insert(insert_at, src);
                    // Shift any attachment indices >= insert_at forward.
                    for normal in flame.transforms.iter_mut() {
                        for a in normal.final_attachments.iter_mut() {
                            if *a >= insert_at { *a += 1; }
                        }
                    }
                }
            });
        }
        if let Some(idx) = ui_response.delete_final_transform {
            self.apply_pool_full_snapshot(&format!("Delete Final {}", idx + 1), |flame| {
                if idx < flame.final_transforms.len() {
                    flame.final_transforms.remove(idx);
                    for normal in flame.transforms.iter_mut() {
                        normal.final_attachments.retain(|&a| a != idx);
                        for a in normal.final_attachments.iter_mut() {
                            if *a > idx { *a -= 1; }
                        }
                    }
                }
            });
        }
    }

    /// Apply a per-normal attachment edit (Linked/Final toggle or reorder).
    /// Wraps the mutation in a full-config snapshot so undo/redo Just Works.
    pub(super) fn handle_attachment_edit(&mut self, ui_response: &UiResponse) {
        use crate::ui::response::{AttachmentEdit, AttachmentKind, AttachmentOp};
        let Some(edit) = ui_response.attachment_edit.clone() else { return; };
        let AttachmentEdit { normal_index, kind, op } = edit;

        let description = match (&kind, &op) {
            (AttachmentKind::Linked, AttachmentOp::Toggle(_)) => "Toggle Linked Attachment",
            (AttachmentKind::Linked, AttachmentOp::MoveUp(_)) => "Reorder Linked Attachment",
            (AttachmentKind::Linked, AttachmentOp::MoveDown(_)) => "Reorder Linked Attachment",
            (AttachmentKind::Final, AttachmentOp::Toggle(_)) => "Toggle Final Attachment",
            (AttachmentKind::Final, AttachmentOp::MoveUp(_)) => "Reorder Final Attachment",
            (AttachmentKind::Final, AttachmentOp::MoveDown(_)) => "Reorder Final Attachment",
        };

        self.apply_pool_full_snapshot(description, |flame| {
            let Some(normal) = flame.transforms.get_mut(normal_index) else { return; };
            let list = match kind {
                AttachmentKind::Linked => &mut normal.linked_attachments,
                AttachmentKind::Final => &mut normal.final_attachments,
            };
            match op {
                AttachmentOp::Toggle(pool_idx) => {
                    if let Some(pos) = list.iter().position(|&a| a == pool_idx) {
                        list.remove(pos);
                    } else {
                        list.push(pool_idx);
                    }
                }
                AttachmentOp::MoveUp(pool_idx) => {
                    if let Some(pos) = list.iter().position(|&a| a == pool_idx) {
                        if pos > 0 { list.swap(pos, pos - 1); }
                    }
                }
                AttachmentOp::MoveDown(pool_idx) => {
                    if let Some(pos) = list.iter().position(|&a| a == pool_idx) {
                        if pos + 1 < list.len() { list.swap(pos, pos + 1); }
                    }
                }
            }
        });
    }

    /// Apply a structural change to a Linked/Final pool by mutating the flame
    /// in `mutate`, then commit a full-config snapshot for clean undo/redo.
    /// Read-only reference to the flame the user is currently
    /// editing. With un-swap, `active_config().flame` is always the
    /// main; this routes to `flame.subflames[i]` when the editing
    /// target is a subflame. Falls back to the main flame on
    /// out-of-bounds (defensive — UI shouldn't have set such a
    /// target).
    pub(super) fn active_target_flame(&self) -> &crate::scene::transforms::Flame {
        use crate::config::manager::EditingTarget;
        let cfg = self.config_manager.active_config();
        match self.config_manager.editing_target() {
            EditingTarget::Subflame { index } if index < cfg.flame.subflames.len() => {
                &cfg.flame.subflames[index]
            }
            _ => &cfg.flame,
        }
    }

    fn apply_pool_full_snapshot<F: FnOnce(&mut crate::scene::transforms::Flame)>(
        &mut self,
        description: &str,
        mutate: F,
    ) {
        use crate::config::manager::EditingTarget;

        let before = self.config_manager.active_config().clone();
        let mut after = before.clone();
        // Route the mutation to the active editing target so
        // "Add Linked Transform" etc. land on the right flame when
        // the user is editing a subflame.
        let target = self.config_manager.editing_target();
        let target_flame: &mut crate::scene::transforms::Flame = match target {
            EditingTarget::Subflame { index } if index < after.flame.subflames.len() => {
                &mut after.flame.subflames[index]
            }
            _ => &mut after.flame,
        };
        mutate(target_flame);
        let change = crate::config::ConfigChange::full_config_snapshot(
            before,
            after,
            description.to_string(),
        );
        if let Err(e) = self.config_manager.apply_structural_change(change) {
            eprintln!("Failed to apply {description}: {e}");
        } else {
            self.flame = self.config_manager.active_config().flame.clone();
        }
    }

    /// Handle new flame creation (reset to default)
    fn handle_new_flame(&mut self, ui_response: &UiResponse) {
        if ui_response.new_flame_requested {
            // Create a proper default config with one identity transform + linear variation
            let new_config = crate::resources::create_default_preset();

            // Load the default config with undo support
            // (passing None clears api_state — this is a new local flame)
            if let Err(e) = self.load_config_with_undo(new_config, "history.action.new_flame".to_string(), None) {
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
            // (passing None clears api_state — this is a new local flame)
            if let Err(e) = self.load_config_with_undo(new_config, "history.action.random_flame".to_string(), None) {
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
            if let Err(e) = self.load_config_with_undo(new_config, "history.action.random_flame".to_string(), None) {
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
                    .set_parent(self.window.as_ref())
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
                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation, config.palette_squeeze, config.palette_squeeze_mode, config.palette_squeeze_falloff, config.palette_log_strength, config.palette_reverse);
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
                    .set_parent(self.window.as_ref())
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
                                        renderer.update_palette(&self.gpu.device, &self.gpu.queue, &palette, config.palette_rotation, config.palette_squeeze, config.palette_squeeze_mode, config.palette_squeeze_falloff, config.palette_log_strength, config.palette_reverse);
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

    /// Handle "Load from file" requests from the Subflames panel.
    /// Replaces the targeted subflame's flame data with one loaded from
    /// a `.fflame` file. Only the flame is taken — view, tone, color,
    /// effects, and any nested subflames in the loaded file are dropped.
    fn handle_subflame_load(&mut self, ui_response: &UiResponse) {
        // Trigger side: button was clicked this frame.
        if let Some(index) = ui_response.load_subflame_into {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(self.window.as_ref())
                    .add_filter("Fractal Flame Config", &["fflame"])
                    .pick_file()
                {
                    match FractalConfig::load_from_file(&path) {
                        Ok(config) => {
                            let mut flame = config.flame;
                            // Strip nested subflames — the project keeps
                            // nesting out of scope, and the user asked
                            // for "only the flame data."
                            flame.subflames.clear();
                            if let Err(e) = self.config_manager.replace_subflame(index, flame) {
                                log::error!(
                                    "Failed to replace subflame {}: {}", index, e
                                );
                            } else {
                                log::info!(
                                    "Replaced subflame {} with flame from {}",
                                    index, path.display(),
                                );
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to load .fflame file: {}", e);
                        }
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                // Stash the target index for async pickup, then trigger
                // the browser file picker. Pickup runs below each frame.
                self.pending_subflame_load_index = Some(index);
                let ctx = self.egui_layer.ctx.clone();
                super::trigger_browser_file_picker(
                    ".fflame", ctx, "pending_subflame_load_raw",
                );
            }
        }

        // Async pickup side (WASM only): the browser file picker
        // writes the file contents into egui temp data under
        // `pending_subflame_load_raw`. Pair it with the stashed index.
        #[cfg(target_arch = "wasm32")]
        if let Some(json) = self.egui_layer.ctx.data_mut(|data| {
            data.remove_temp::<String>(egui::Id::new("pending_subflame_load_raw"))
        }) {
            if let Some(index) = self.pending_subflame_load_index.take() {
                match serde_json::from_str::<FractalConfig>(&json) {
                    Ok(config) => {
                        let mut flame = config.flame;
                        flame.subflames.clear();
                        if let Err(e) = self.config_manager.replace_subflame(index, flame) {
                            log::error!("Failed to replace subflame {}: {}", index, e);
                        } else {
                            log::info!("Replaced subflame {} from loaded file", index);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to parse .fflame JSON: {}", e);
                    }
                }
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
                    .set_parent(self.window.as_ref())
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
                                if let Err(e) = self.load_config_with_undo(config, "history.action.load_config".to_string(), None) {
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
                                if let Err(e) = self.load_config_with_undo(first_config, "history.action.load_config".to_string(), None) {
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
                    .set_parent(self.window.as_ref())
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
                                        if let Err(e) = self.load_config_with_undo(config, "history.action.import_apophysis".to_string(), None) {
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
                                        if let Err(e) = self.load_config_with_undo(first_config, "history.action.import_apophysis".to_string(), None) {
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

        // Handle Apophysis .flame export
        if ui_response.apophysis_export_file_requested {
            let config = self.export_config();
            let xml = crate::apophysis_xml::write_flame_xml(&config);
            let default_name = format!(
                "{}.flame",
                if config.flame.name.is_empty() {
                    "fractal".to_string()
                } else {
                    config.flame.name.to_lowercase().replace(' ', "_")
                }
            );

            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(self.window.as_ref())
                    .add_filter("Apophysis Flame", &["flame"])
                    .set_file_name(&default_name)
                    .save_file()
                {
                    if let Err(e) = std::fs::write(&path, &xml) {
                        eprintln!("Failed to write Apophysis flame: {}", e);
                    } else {
                        println!("Exported Apophysis flame to: {}", path.display());
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                if let Err(e) = trigger_browser_download(xml.as_bytes(), &default_name, "application/xml") {
                    log::error!("Failed to trigger Apophysis download: {}", e);
                }
            }
        }

        // Handle file browser open request (loads into FractalBrowser Files tab)
        if ui_response.file_browser_open_requested {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: synchronous file dialog
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(self.window.as_ref())
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
                    .set_parent(self.window.as_ref())
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
                    .set_parent(self.window.as_ref())
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
                    .set_parent(self.window.as_ref())
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
            // Show loading notification for API-loaded flames
            if ui_response.loaded_api_flame_id.is_some() {
                let name = config.flame.name.clone();
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.loading_flame", name = name),
                    false,
                );
            }

            let api_metadata = ui_response.loaded_api_flame_id.as_ref().map(|flame_id| {
                crate::app::ApiContentState {
                    flame_id: Some(flame_id.clone()),
                    flame_is_public: ui_response.loaded_api_flame_is_public,
                    flame_user_id: ui_response.loaded_api_flame_user_id.clone(),
                    animation_count: ui_response.loaded_api_flame_animation_count,
                    flame_animations: ui_response.loaded_api_flame_animations.clone(),
                    animation_id: None,
                }
            });
            if let Err(e) = self.load_config_with_undo(config.clone(), "history.action.load_preset".to_string(), api_metadata) {
                log::error!("Failed to load preset: {}", e);
            } else {
                log::info!("Preset loaded successfully");

                // Show success notification for API-loaded flames
                if ui_response.loaded_api_flame_id.is_some() {
                    let name = config.flame.name.clone();
                    self.egui_layer.show_api_notification(
                        &rust_i18n::t!("api.load_flame_success", name = name),
                        false,
                    );
                }
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
                        if let Err(e) = self.load_config_with_undo(config, "history.action.load_config".to_string(), None) {
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
                            if let Err(e) = self.load_config_with_undo(config, "history.action.import_apophysis".to_string(), None) {
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
                    Ok(mut animation) => {
                        // If animation has embedded config, load it first
                        if let Some(ref config) = animation.base_config {
                            log::info!("Animation '{}' has embedded config, loading it", animation.name);
                            let description = format!("history.action.load_animation_config|name={}", animation.name);
                            if let Err(e) = self.load_config_with_undo(config.clone(), description, None) {
                                log::error!("Failed to load animation's embedded config: {}", e);
                            }
                        }
                        let generators = animation.generators.clone();
                        let duration = animation.duration;
                        self.animation_controller.load(animation);
                        // Bind AFTER controller.load so we're binding
                        // the new animation that's now in the
                        // controller. The embedded config (if any)
                        // was loaded above via load_config_with_undo,
                        // so active_config carries the new IDs the
                        // tracks will resolve against.
                        if let Some(anim) = self.animation_controller.animation.as_mut() {
                            anim.bind_to_config(self.config_manager.active_config());
                        }
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

            for (flame_target, path_str, json_value) in frame_values {
                if let Some(path) = crate::config::ConfigPath::from_string_key(&path_str) {
                    if let Some(config_value) = crate::config::json_to_config_value(&json_value, &path) {
                        if let Err(e) = self
                            .config_manager
                            .update_param_silent_on(flame_target, path, config_value)
                        {
                            log::warn!(
                                "Animation scrub: failed to update {:?}/{}: {}",
                                flame_target, path_str, e
                            );
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

    /// Poll for completed health check results and trigger periodic checks.
    pub(super) fn poll_health_check(&mut self) {

        // 1. Poll async result
        if self.health_check_in_progress {
            let outcome = self.health_check_result.lock().ok().and_then(|mut slot| slot.take());
            if let Some(outcome) = outcome {
                self.health_check_in_progress = false;
                self.last_health_check = Some(web_time::Instant::now());
                self.process_health_check_outcome(outcome);
            }
        }

        // 2. Trigger periodic check (every 30 seconds)
        if !self.health_check_in_progress {
            let settings = self.config_manager.system_settings();
            let has_auth = settings.is_signed_in();

            let should_check = settings.online_mode
                && has_auth
                && self.last_health_check
                    .map(|t| t.elapsed().as_secs() >= 30)
                    .unwrap_or(true);

            if should_check {
                self.trigger_health_check();
            }
        }
    }

    fn process_health_check_outcome(&mut self, outcome: crate::api::HealthCheckOutcome) {
        use crate::api::{ApiConnectivity, HealthCheckOutcome};

        let prev = self.api_connectivity;

        match outcome {
            HealthCheckOutcome::Authenticated { email, user_id } => {
                self.api_connectivity = ApiConnectivity::Online;
                log::debug!("Health check OK: authenticated (email: {:?})", email);

                self.current_user_id = Some(user_id);

                // On WASM, store email from cookie-based session (no token needed)
                #[cfg(target_arch = "wasm32")]
                if email.is_some() {
                    let settings = self.config_manager.system_settings_mut();
                    settings.auth_email = email;
                    // Don't persist — in-memory only for WASM cookie auth
                }
            }
            HealthCheckOutcome::TokenExpired => {
                self.api_connectivity = ApiConnectivity::Online;
                log::info!("Health check: token expired, clearing auth");

                self.current_user_id = None;

                // Clear auth from SystemSettings
                {
                    let settings = self.config_manager.system_settings_mut();
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        settings.auth_token = None;
                    }
                    settings.auth_email = None;
                    // On WASM, don't persist — cookie is already expired server-side
                    #[cfg(not(target_arch = "wasm32"))]
                    let _ = settings.save();
                }

                // Clear cloud state in UI
                self.egui_layer.clear_cloud_state();

                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("auth.session_expired"),
                    true,
                );
            }
            HealthCheckOutcome::ServerError(ref msg) => {
                self.api_connectivity = ApiConnectivity::Online;
                log::warn!("Health check server error: {}", msg);
            }
            HealthCheckOutcome::NetworkError(ref msg) => {
                self.api_connectivity = ApiConnectivity::Unreachable;
                log::warn!("Health check network error: {}", msg);
            }
        }

        // Transition notifications
        if prev == ApiConnectivity::Online && self.api_connectivity == ApiConnectivity::Unreachable {
            self.egui_layer.show_api_notification(
                &rust_i18n::t!("auth.connection_lost"),
                true,
            );
        }
        if prev == ApiConnectivity::Unreachable && self.api_connectivity == ApiConnectivity::Online {
            self.egui_layer.show_api_notification(
                &rust_i18n::t!("auth.connection_restored"),
                false,
            );
        }
    }

    fn trigger_health_check(&mut self) {
        let settings = self.config_manager.system_settings();
        let base_url = crate::api::API_BASE_URL.to_string();

        // get_auth_token returns empty string on WASM (cookies handle auth)
        let token = match settings.get_auth_token() {
            Some(t) => t,
            // On WASM startup, auth_email may not be set yet — use empty token
            #[cfg(target_arch = "wasm32")]
            None => String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            None => return,
        };

        self.health_check_in_progress = true;
        let result_slot = self.health_check_result.clone();
        let window = self.window.clone();

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let outcome = crate::api::check_api_health(&base_url, &token).await;
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(outcome);
                }
                window.request_redraw();
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let outcome = pollster::block_on(
                    crate::api::check_api_health(&base_url, &token)
                );
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(outcome);
                }
                window.request_redraw();
            });
        }
    }

    /// Trigger a health check if conditions are met (has token/cookies, online mode, not in flight).
    /// Called once at startup — on WASM, always tries (cookies may already be valid).
    pub(super) fn maybe_trigger_health_check(&mut self) {
        // On WASM, always try at startup — cookies may hold a valid session
        // even before auth_email is set (the health check discovers the session)
        #[cfg(target_arch = "wasm32")]
        let has_auth = true;
        #[cfg(not(target_arch = "wasm32"))]
        let has_auth = self.config_manager.system_settings().is_signed_in();

        if !self.health_check_in_progress
            && self.config_manager.system_settings().online_mode
            && has_auth
        {
            self.trigger_health_check();
        }
    }

    /// Render a thumbnail for the given config and encode as PNG bytes.
    ///
    /// Must be called on the main thread (needs GPU device/queue access).
    /// Returns None if rendering or encoding fails.
    /// Desktop only — on WASM, thumbnail rendering is done inside spawn_local.
    #[cfg(not(target_arch = "wasm32"))]
    fn render_thumbnail_jpg(&self, config: &FractalConfig) -> Option<Vec<u8>> {
        let image = pollster::block_on(
            crate::renderer::thumbnail::render_thumbnail_async(
                &self.gpu.device,
                &self.gpu.queue,
                config,
            )
        );
        match encode_rgba_to_jpeg(image.width(), image.height(), &image.into_raw()) {
            Ok(data) => {
                log::info!("Thumbnail rendered: {} JPEG bytes", data.len());
                Some(data)
            }
            Err(e) => {
                log::error!("Failed to encode thumbnail JPEG: {}", e);
                None
            }
        }
    }

    /// Handle API save/update — process async results and new requests
    fn handle_save_online(&mut self, ui_response: &UiResponse) {
        use crate::ui::ApiSaveAction;

        // Check for completed async flame save
        if self.api_save_in_progress {
            // Extract result from mutex first, then act on it (avoids borrow conflict)
            let save_result = self.api_save_result.lock().ok().and_then(|mut r| r.take());
            if let Some(save_result) = save_result {
                self.api_save_in_progress = false;
                match save_result {
                    Ok(flame_id) => {
                        log::info!("Flame saved/updated online: {}", flame_id);
                        let was_new = self.api_pending_is_new;
                        self.api_pending_is_new = false;
                        self.api_state.flame_id = Some(flame_id.clone());
                        self.api_state.flame_is_public = self.api_pending_visibility.take();
                        // The flame is now owned by the current user
                        self.api_state.flame_user_id = self.current_user_id.clone();
                        // SaveNew creates a fresh flame with no animations attached;
                        // Update preserves the existing animations list
                        if was_new {
                            self.api_state.animation_count = 0;
                            self.api_state.flame_animations.clear();
                            self.api_state.animation_id = None;
                        }
                        let name = self.config_manager.active_config().flame.name.clone();
                        self.egui_layer.show_api_notification(
                            &rust_i18n::t!("api.save_success", name = name),
                            false,
                        );
                        // Auto-refresh the Online tab
                        self.egui_layer.request_online_refresh();

                        // If pending animation save was requested, trigger it now
                        if let Some(anim_visibility) = self.pending_animation_save.take() {
                            self.trigger_animation_save(&flame_id, anim_visibility);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to save flame online: {}", e);
                        self.pending_animation_save = None;
                        self.api_pending_visibility = None;
                        self.egui_layer.show_api_notification(
                            &rust_i18n::t!("api.save_error", error = e),
                            true,
                        );
                    }
                }
            }
        }

        // Handle new save/update request
        let action = &ui_response.api_save_action;
        let should_act = !matches!(action, ApiSaveAction::None) && !self.api_save_in_progress;
        if should_act {
            // Clear any stale result from a previous attempt (e.g. "Not signed in" error
            // that was written while api_save_in_progress was false and never consumed)
            if let Ok(mut slot) = self.api_save_result.lock() {
                slot.take();
            }

            let config = self.export_config();
            let result_slot = self.api_save_result.clone();
            self.api_save_in_progress = true;

            // Read credentials from SystemSettings
            let settings = self.config_manager.system_settings();
            let token = match settings.get_auth_token() {
                Some(t) => t,
                None => {
                    self.api_save_in_progress = false;
                    self.egui_layer.show_api_notification(
                        &rust_i18n::t!("api.save_error", error = "Not signed in"),
                        true,
                    );
                    return;
                }
            };

            match action {
                ApiSaveAction::SaveNew { name, upload_thumbnail, make_public } => {
                    let name = name.clone();
                    let upload_thumbnail = *upload_thumbnail;
                    let make_public = *make_public;

                    let visibility = if make_public {
                        Some(crate::api::types::ApiVisibility::Public)
                    } else {
                        None
                    };

                    self.api_pending_visibility = Some(make_public);
                    self.api_pending_is_new = true;

                    let window = self.window.clone();

                    #[cfg(target_arch = "wasm32")]
                    {
                        // On WASM, render thumbnail inside spawn_local (same thread,
                        // device/queue are Arc-wrapped so clone is cheap)
                        let device = self.gpu.device.clone();
                        let queue = self.gpu.queue.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let thumbnail_jpg = if upload_thumbnail {
                                log::info!("Rendering thumbnail for upload...");
                                render_thumbnail_jpg_async(&device, &queue, &config).await
                            } else {
                                None
                            };
                            let save_result = save_flame_online(&token, config, &name, visibility, thumbnail_jpg).await;
                            if let Ok(mut slot) = result_slot.lock() {
                                *slot = Some(save_result);
                            }
                            window.request_redraw();
                        });
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        // On desktop, render thumbnail synchronously before spawning thread
                        let thumbnail_jpg = if upload_thumbnail {
                            log::info!("Rendering thumbnail for upload...");
                            self.render_thumbnail_jpg(&config)
                        } else {
                            None
                        };
                        std::thread::spawn(move || {
                            let result = pollster::block_on(save_flame_online(&token, config, &name, visibility, thumbnail_jpg));
                            if let Ok(mut slot) = result_slot.lock() {
                                *slot = Some(result);
                            }
                            window.request_redraw();
                        });
                    }
                }
                ApiSaveAction::Update { name, upload_thumbnail, make_public } => {
                    let name = name.clone();
                    let upload_thumbnail = *upload_thumbnail;
                    let make_public = *make_public;

                    let visibility = if make_public {
                        Some(crate::api::types::ApiVisibility::Public)
                    } else {
                        None
                    };

                    self.api_pending_visibility = Some(make_public);
                    self.api_pending_is_new = false;

                    if let Some(flame_id) = self.api_state.flame_id.clone() {
                        let window = self.window.clone();

                        #[cfg(target_arch = "wasm32")]
                        {
                            let device = self.gpu.device.clone();
                            let queue = self.gpu.queue.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                let thumbnail_jpg = if upload_thumbnail {
                                    log::info!("Rendering thumbnail for upload...");
                                    render_thumbnail_jpg_async(&device, &queue, &config).await
                                } else {
                                    None
                                };
                                let save_result = update_flame_online(&token, config, &flame_id, &name, visibility, thumbnail_jpg).await;
                                if let Ok(mut slot) = result_slot.lock() {
                                    *slot = Some(save_result);
                                }
                                window.request_redraw();
                            });
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let thumbnail_jpg = if upload_thumbnail {
                                log::info!("Rendering thumbnail for upload...");
                                self.render_thumbnail_jpg(&config)
                            } else {
                                None
                            };
                            std::thread::spawn(move || {
                                let result = pollster::block_on(update_flame_online(&token, config, &flame_id, &name, visibility, thumbnail_jpg));
                                if let Ok(mut slot) = result_slot.lock() {
                                    *slot = Some(result);
                                }
                                window.request_redraw();
                            });
                        }
                    } else {
                        log::error!("Update requested but no api_flame_id");
                        self.api_save_in_progress = false;
                    }
                }
                ApiSaveAction::None => unreachable!(),
            }
        }
    }

    /// Handle animation API save/update — process async results and new requests
    fn handle_save_animation_online(&mut self, ui_response: &UiResponse) {
        use crate::ui::ApiAnimationSaveAction;

        // Check for completed async animation save
        if self.api_animation_save_in_progress {
            if let Ok(mut result) = self.api_animation_save_result.lock() {
                if let Some(save_result) = result.take() {
                    self.api_animation_save_in_progress = false;
                    match save_result {
                        Ok(animation_id) => {
                            log::info!("Animation saved/updated online: {}", animation_id);
                            self.api_state.animation_id = Some(animation_id);
                            let name = self.animation_controller.animation
                                .as_ref()
                                .map(|a| a.name.clone())
                                .unwrap_or_default();
                            self.egui_layer.show_api_notification(
                                &rust_i18n::t!("api.animation_save_success", name = name),
                                false,
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to save animation online: {}", e);
                            self.egui_layer.show_api_notification(
                                &rust_i18n::t!("api.animation_save_error", error = e),
                                true,
                            );
                        }
                    }
                }
            }
        }

        // Handle new animation save/update request (from Animation Panel buttons)
        let action = &ui_response.api_animation_save_action;
        let should_act = !matches!(action, ApiAnimationSaveAction::None) && !self.api_animation_save_in_progress;
        if should_act {
            match action {
                ApiAnimationSaveAction::SaveNew { make_public } => {
                    let visibility = if *make_public {
                        Some(crate::api::types::ApiVisibility::Public)
                    } else {
                        Some(crate::api::types::ApiVisibility::Private)
                    };
                    if let Some(flame_id) = self.api_state.flame_id.clone() {
                        self.trigger_animation_save(&flame_id, visibility);
                    } else {
                        log::error!("Animation save requested but no api_flame_id");
                    }
                }
                ApiAnimationSaveAction::Update { make_public } => {
                    let visibility = if *make_public {
                        Some(crate::api::types::ApiVisibility::Public)
                    } else {
                        Some(crate::api::types::ApiVisibility::Private)
                    };
                    if let Some(animation_id) = self.api_state.animation_id.clone() {
                        self.trigger_animation_update(&animation_id, visibility);
                    } else {
                        log::error!("Animation update requested but no api_animation_id");
                    }
                }
                ApiAnimationSaveAction::None => unreachable!(),
            }
        }
    }

    /// Trigger saving the current animation to the API as a new entry.
    fn trigger_animation_save(&mut self, flame_id: &str, visibility: Option<crate::api::types::ApiVisibility>) {
        let animation = match self.animation_controller.animation.as_ref() {
            Some(a) => a.clone(),
            None => return,
        };

        let settings = self.config_manager.system_settings();
        let base_url = crate::api::API_BASE_URL.to_string();
        let token = match settings.get_auth_token() {
            Some(t) => t,
            None => {
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.animation_save_error", error = "Not signed in"),
                    true,
                );
                return;
            }
        };

        let result_slot = self.api_animation_save_result.clone();
        let window = self.window.clone();
        self.api_animation_save_in_progress = true;

        // Clear stale result
        if let Ok(mut slot) = result_slot.lock() {
            slot.take();
        }

        let flame_id = flame_id.to_string();
        let name = animation.name.clone();

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = save_animation_online(&base_url, &token, animation, &name, &flame_id, visibility).await;
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(result);
                }
                window.request_redraw();
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let result = pollster::block_on(save_animation_online(&base_url, &token, animation, &name, &flame_id, visibility));
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(result);
                }
                window.request_redraw();
            });
        }
    }

    /// Trigger updating an existing animation on the API.
    fn trigger_animation_update(&mut self, animation_id: &str, visibility: Option<crate::api::types::ApiVisibility>) {
        let animation = match self.animation_controller.animation.as_ref() {
            Some(a) => a.clone(),
            None => return,
        };
        let flame_id = self.api_state.flame_id.clone();

        let settings = self.config_manager.system_settings();
        let base_url = crate::api::API_BASE_URL.to_string();
        let token = match settings.get_auth_token() {
            Some(t) => t,
            None => {
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.animation_save_error", error = "Not signed in"),
                    true,
                );
                return;
            }
        };

        let result_slot = self.api_animation_save_result.clone();
        let window = self.window.clone();
        self.api_animation_save_in_progress = true;

        // Clear stale result
        if let Ok(mut slot) = result_slot.lock() {
            slot.take();
        }

        let animation_id = animation_id.to_string();
        let name = animation.name.clone();

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = update_animation_online(&base_url, &token, animation, &animation_id, &name, flame_id, visibility).await;
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(result);
                }
                window.request_redraw();
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let result = pollster::block_on(update_animation_online(&base_url, &token, animation, &animation_id, &name, flame_id, visibility));
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(result);
                }
                window.request_redraw();
            });
        }
    }

    /// Trigger loading a flame or animation by ID (cross-platform).
    /// Reuses the URL deep-link loading mechanism for the result plumbing.
    pub(super) fn trigger_url_load(&mut self, flame_id: Option<String>, animation_id: Option<String>) {
        // Show "loading" notification
        if animation_id.is_some() {
            self.egui_layer.show_api_notification(
                &rust_i18n::t!("api.loading_animation"),
                false,
            );
        } else if flame_id.is_some() {
            self.egui_layer.show_api_notification(
                &rust_i18n::t!("api.loading_flame_generic"),
                false,
            );
        }

        let base_url = crate::api::API_BASE_URL.to_string();
        let result_slot = self.url_load_result.clone();
        let window = self.window.clone();
        self.url_load_in_progress = true;
        self.url_load_started = Some(web_time::Instant::now());

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let result = load_from_url(&base_url, flame_id, animation_id).await;
            if let Ok(mut slot) = result_slot.lock() {
                *slot = Some(result);
            }
            // Wake the event loop so handle_url_load runs immediately
            window.request_redraw();
        });

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result = pollster::block_on(load_from_url(&base_url, flame_id, animation_id));
            if let Ok(mut slot) = result_slot.lock() {
                *slot = Some(result);
            }
            window.request_redraw();
        });
    }

    /// Poll for completed URL deep-link load and apply the result.
    /// Handle request to load an animation from the API (triggered from the
    /// animations list at the bottom of the Animation panel).
    fn handle_load_api_animation(&mut self, ui_response: &UiResponse) {
        if let Some(ref animation_id) = ui_response.load_api_animation_id {
            if self.url_load_in_progress {
                log::warn!("Animation load requested but URL load already in progress");
                return;
            }
            log::info!("Loading animation from API: {}", animation_id);
            // Reuse the URL load mechanism — passing None for flame_id means
            // the animation's base_config will provide the flame
            self.trigger_url_load(None, Some(animation_id.clone()));
        }
    }

    fn handle_url_load(&mut self) {
        use super::UrlLoadedData;

        if !self.url_load_in_progress {
            return;
        }

        // Timeout: if the load has been in progress for over 30 seconds, give up
        if let Some(started) = self.url_load_started {
            if started.elapsed() > std::time::Duration::from_secs(30) {
                log::error!("URL load timed out after 30 seconds");
                self.url_load_in_progress = false;
                self.url_load_started = None;
                self.paused = false;
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.url_load_error", error = "Request timed out"),
                    true,
                );
                return;
            }
        }

        let loaded = self.url_load_result.lock().ok().and_then(|mut r| r.take());
        let Some(result) = loaded else { return };

        self.url_load_in_progress = false;
        self.url_load_started = None;
        self.paused = false;

        match result {
            Ok(UrlLoadedData::Flame { config, flame_id, is_public, user_id, animation_count, animations }) => {
                let name = config.flame.name.clone();
                log::info!("URL load: flame '{}' ({})", name, flame_id);
                let api_metadata = Some(crate::app::ApiContentState {
                    flame_id: Some(flame_id.clone()),
                    flame_is_public: is_public,
                    flame_user_id: Some(user_id),
                    animation_id: None,
                    animation_count,
                    flame_animations: animations,
                });
                if let Err(e) = self.load_config_with_undo(config, format!("Load flame from URL: {}", name), api_metadata) {
                    log::error!("Failed to load flame from URL: {}", e);
                    self.egui_layer.show_api_notification(
                        &rust_i18n::t!("api.url_load_error", error = e),
                        true,
                    );
                    return;
                }
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.url_load_flame_success", name = name),
                    false,
                );
            }
            Ok(UrlLoadedData::Animation { animation, animation_id, flame_id, flame_meta }) => {
                let anim_name = animation.name.clone();
                log::info!("URL load: animation '{}' ({})", anim_name, animation_id);

                // Switch to the animation-focused workspace so the user lands on a layout
                // with the Animation panel visible. In compact mode, just open the panel
                // since compact is single-panel.
                let compact = self.config_manager.system_settings().compact_mode.unwrap_or(false);
                if compact {
                    let ctx = &self.egui_layer.ctx;
                    self.workspace.open_compact_panel(crate::ui::workspace::PanelType::Animation, ctx);
                } else if !matches!(self.workspace.current_layout, crate::ui::workspace::WorkspaceLayout::Animation) {
                    self.workspace.apply_layout(crate::ui::workspace::WorkspaceLayout::Animation);
                }

                // Load the flame config from the animation's base_config (populated from embedded flame)
                if let Some(config) = animation.base_config.clone() {
                    let flame_name = config.flame.name.clone();
                    let api_metadata = flame_meta.as_ref().map(|meta| crate::app::ApiContentState {
                        flame_id: flame_id.clone(),
                        flame_is_public: meta.is_public,
                        flame_user_id: Some(meta.user_id.clone()),
                        animation_id: Some(animation_id.clone()),
                        animation_count: meta.animation_count,
                        flame_animations: meta.animations.clone(),
                    });
                    if let Err(e) = self.load_config_with_undo(config, format!("Load flame for animation: {}", flame_name), api_metadata) {
                        log::error!("Failed to load flame for animation: {}", e);
                    }
                }

                // Load the animation
                let generators = animation.generators.clone();
                let duration = animation.duration;
                self.animation_controller.load(animation);
                // Bind AFTER controller.load — see file-load path above
                // for the reasoning.
                if let Some(anim) = self.animation_controller.animation.as_mut() {
                    anim.bind_to_config(self.config_manager.active_config());
                }
                self.egui_layer.signal_panel_state.restore_generators(
                    generators, &mut self.signal_manager, duration,
                );

                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.url_load_animation_success", name = anim_name),
                    false,
                );
            }
            Err(e) => {
                log::error!("Failed to load from URL: {}", e);
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.url_load_error", error = e),
                    true,
                );
            }
        }
    }
}

/// Render a thumbnail and encode as PNG bytes (async helper for WASM).
#[cfg(target_arch = "wasm32")]
async fn render_thumbnail_jpg_async(
    device: &egui_wgpu::wgpu::Device,
    queue: &egui_wgpu::wgpu::Queue,
    config: &crate::config::FractalConfig,
) -> Option<Vec<u8>> {
    let image = crate::renderer::thumbnail::render_thumbnail_async(device, queue, config).await;
    match encode_rgba_to_jpeg(image.width(), image.height(), &image.into_raw()) {
        Ok(data) => {
            log::info!("Thumbnail rendered: {} JPEG bytes", data.len());
            Some(data)
        }
        Err(e) => {
            log::error!("Failed to encode thumbnail JPEG: {}", e);
            None
        }
    }
}

/// Save a flame to the API as new (async helper).
async fn save_flame_online(
    token: &str,
    config: crate::config::FractalConfig,
    name: &str,
    visibility: Option<crate::api::types::ApiVisibility>,
    thumbnail_jpg: Option<Vec<u8>>,
) -> Result<String, String> {
    let mut api = crate::api::ApiState::new("");
    api.set_token(token);
    api.save_flame(&config, Some(name), visibility, thumbnail_jpg.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Update an existing flame on the API (async helper).
async fn update_flame_online(
    token: &str,
    config: crate::config::FractalConfig,
    flame_id: &str,
    name: &str,
    visibility: Option<crate::api::types::ApiVisibility>,
    thumbnail_jpg: Option<Vec<u8>>,
) -> Result<String, String> {
    let mut api = crate::api::ApiState::new("");
    api.set_token(token);
    api.update_flame(flame_id, &config, Some(name), visibility, thumbnail_jpg.as_deref())
        .await
        .map(|resp| resp.id)
        .map_err(|e| e.to_string())
}

/// Delete a flame from the API (async helper).
async fn delete_flame_online(
    base_url: &str,
    token: &str,
    flame_id: &str,
) -> Result<(), String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    api.delete_flame(flame_id)
        .await
        .map_err(|e| e.to_string())
}

/// Save an animation to the API as new (async helper).
async fn save_animation_online(
    base_url: &str,
    token: &str,
    animation: crate::animation::Animation,
    name: &str,
    flame_id: &str,
    visibility: Option<crate::api::types::ApiVisibility>,
) -> Result<String, String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    api.save_animation(&animation, Some(name), Some(flame_id), visibility)
        .await
        .map_err(|e| e.to_string())
}

/// Update an existing animation on the API (async helper).
async fn update_animation_online(
    base_url: &str,
    token: &str,
    animation: crate::animation::Animation,
    animation_id: &str,
    name: &str,
    flame_id: Option<String>,
    visibility: Option<crate::api::types::ApiVisibility>,
) -> Result<String, String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    api.update_animation(animation_id, &animation, Some(name), flame_id.as_deref(), visibility)
        .await
        .map(|resp| resp.id)
        .map_err(|e| e.to_string())
}

/// Load a flame and/or animation from the API by URL params (async helper).
/// Prioritizes `animation_id` over `flame_id` if both are present.
async fn load_from_url(
    base_url: &str,
    flame_id: Option<String>,
    animation_id: Option<String>,
) -> Result<super::UrlLoadedData, String> {
    // WASM uses cookie auth — empty token is fine (require_token returns Ok(""))
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token("");

    if let Some(animation_id) = animation_id {
        // Load animation with embedded flame config (single request)
        let (animation, _flame_config, resp_flame_id) = api.load_animation_full(&animation_id).await
            .map_err(|e| format!("Failed to load animation: {}", e))?;

        // Use flame_id from URL param, or fall back to the one from the animation response
        let effective_flame_id = flame_id.or(resp_flame_id);

        // Fetch full flame metadata (animations list, owner, etc.) if we have a flame_id
        let flame_meta = if let Some(ref fid) = effective_flame_id {
            api.load_flame_with_visibility(fid).await.ok()
        } else {
            None
        };

        Ok(super::UrlLoadedData::Animation {
            animation,
            animation_id,
            flame_id: effective_flame_id,
            flame_meta,
        })
    } else if let Some(flame_id) = flame_id {
        let result = api.load_flame_with_visibility(&flame_id).await
            .map_err(|e| format!("Failed to load flame: {}", e))?;
        Ok(super::UrlLoadedData::Flame {
            config: result.config,
            flame_id,
            is_public: result.is_public,
            user_id: result.user_id,
            animation_count: result.animation_count,
            animations: result.animations,
        })
    } else {
        Err("No flame or animation ID provided".to_string())
    }
}

/// Encode RGBA pixel data to JPEG bytes (quality 90).
/// Used for API thumbnail uploads — separate from the PNG export pipeline.
fn encode_rgba_to_jpeg(width: u32, height: u32, rgba_data: &[u8]) -> Result<Vec<u8>, String> {
    use image::codecs::jpeg::JpegEncoder;
    use std::io::Cursor;

    // JPEG doesn't support alpha — strip A channel
    let rgb_data: Vec<u8> = rgba_data
        .chunks_exact(4)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();

    let mut buf = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, 90);
    encoder
        .encode(&rgb_data, width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG encode failed: {}", e))?;
    Ok(buf.into_inner())
}
