use crate::app::render_mode::TransitionResult;
use crate::app::{App, ApiContentState};
use crate::config::FractalConfig;

impl App {
    /// Load config via ConfigManager and sync app state.
    /// Creates snapshot-based undo entry and triggers GPU update.
    ///
    /// Local loads (file, preset, random, new): pass `None` to clear API state.
    /// API loads (browser, URL deep-link): pass `Some(api_state)` with the
    /// flame_id, animations, ownership, etc. from the API response.
    pub fn load_config_with_undo(
        &mut self,
        config: FractalConfig,
        description: String,
        api_metadata: Option<ApiContentState>,
    ) -> Result<(), String> {
        // Snapshot the current api_state as the "after" of the previous snapshot
        // (captures any changes since the last load, e.g. saving online)
        self.update_current_api_state_snapshot();

        // Capture api_state BEFORE replacing — this is the "before" of the new snapshot
        let before_api_state = self.api_state.clone();

        // Replace api_state: API loads set their metadata, local loads clear it.
        self.api_state = api_metadata.unwrap_or_default();

        // Log effects being loaded (diagnostic for API-loaded configs)
        let color_count = config.color_effects.iter().filter(|e| e.enabled).count();
        let density_count = config.density_effects.iter().filter(|e| e.enabled).count();
        if color_count > 0 || density_count > 0 {
            let color_names: Vec<&str> = config.color_effects.iter()
                .filter(|e| e.enabled)
                .map(|e| e.effect_type.as_str())
                .collect();
            let density_names: Vec<&str> = config.density_effects.iter()
                .filter(|e| e.enabled)
                .map(|e| e.effect_type.as_str())
                .collect();
            log::info!("Loading config with {} color effects {:?}, {} density effects {:?}",
                color_count, color_names, density_count, density_names);
        }

        // Load via ConfigManager (creates before/after snapshots for undo)
        self.config_manager
            .load_config(config, description)
            .map_err(|e| format!("{}", e))?;

        // Record api_state snapshot at the new history position.
        let snapshot_idx = self.config_manager.position().saturating_sub(1);
        self.api_state_history.retain(|&k, _| k < snapshot_idx);
        self.api_state_history.insert(snapshot_idx, (before_api_state, self.api_state.clone()));
        self.current_api_snapshot_idx = Some(snapshot_idx);

        // Sync all app state from ConfigManager (triggers GPU update)
        let active_config = self.config_manager.active_config().clone();
        self.import_config(active_config);

        // Re-bind animation tracks against the just-loaded config.
        // The new config carries fresh session-local IDs (assigned by
        // ConfigManager::load_config); any previously-bound tracks
        // were referencing the *previous* config's IDs, which no
        // longer exist. A fresh bind walks each track's `target` /
        // `flame_target` and resolves it against the new config.
        //
        // Covers the deferred-load case in particular: the panel
        // viewer stashes an animation's embedded base_config into
        // `selected_preset_config` and loads the animation immediately
        // (with stale bind). Later, `handle_preset_selection` applies
        // the stashed config via this same `load_config_with_undo`
        // path — at which point the hook below re-binds.
        if let Some(animation) = self.animation_controller.animation.as_mut() {
            animation.bind_to_config(self.config_manager.active_config());
        }

        // Enable overwrite mode for immediate rendering (same as restore_base_config).
        // Without this, the first frames after loading use normal blend mode (e.g. 10%),
        // making the image very dim and effects invisible until accumulation builds up.
        self.use_overwrite_next_frame = true;

        // Check for unknown variations and fetch them from the API.
        // This pauses rendering until fetches complete (or fail/timeout).
        let missing = crate::variations::missing_variations_in(
            &self.config_manager.active_config().flame
        );
        if !missing.is_empty() {
            self.trigger_variation_fetches(missing);
        }

        Ok(())
    }

    pub fn export_config(&self) -> FractalConfig {
        // Use the *logical* config (un-swap any active subflame edit) so
        // what we serialize matches the user's mental model. When editing
        // the main flame, this is just active_config().clone(); when
        // editing a subflame, it puts the active subflame back at its
        // original index so saved JSON has the parent as `flame` and the
        // subflames as `flame.subflames`.
        self.config_manager.logical_config()
    }

    /// Import configuration from FractalConfig
    pub fn import_config(&mut self, config: FractalConfig) {
        // Sync working copy for renderer (only field not in ConfigManager)
        self.flame = config.flame.clone();

        // Sync palette editor with the config palette
        self.egui_layer.update_palette_editor(config.palette.clone());

        // Use the comprehensive load_config function to ensure all GPU state is synchronized
        // (including tone mapping, palette, transforms, params, etc.)
        if let Some(ref mut renderer) = self.flame_renderer {
            let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                label: Some("Config Import Encoder"),
            });

            renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &config, &config.palette, self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in);

            self.gpu.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    /// Undo to previous state
    pub fn undo(&mut self) {
        // Before navigating, capture any api_state changes into the current snapshot
        self.update_current_api_state_snapshot();

        let pos_before = self.config_manager.position();
        if let Ok(_update_type) = self.config_manager.undo() {
            // Sync App working copy and GPU state from ConfigManager
            let config = self.config_manager.config();
            self.import_config(config.clone());
            self.restore_api_state_for_undo(pos_before);
        }
    }

    /// Redo to next state
    pub fn redo(&mut self) {
        self.update_current_api_state_snapshot();

        let pos_before = self.config_manager.position();
        if let Ok(_update_type) = self.config_manager.redo() {
            let config = self.config_manager.config();
            self.import_config(config.clone());
            self.restore_api_state_for_redo(pos_before);
        }
    }

    /// Update the current snapshot's "after" api_state with the current value.
    /// Called before any navigation so changes (e.g. saving online) are preserved.
    pub(super) fn update_current_api_state_snapshot(&mut self) {
        if let Some(idx) = self.current_api_snapshot_idx {
            if let Some((_, after)) = self.api_state_history.get_mut(&idx) {
                *after = self.api_state.clone();
            }
        }
    }

    /// After undo: if we crossed a FullConfig snapshot, restore the "before" api_state.
    fn restore_api_state_for_undo(&mut self, pos_before: usize) {
        // Undo moves position back by 1. If position `pos_before - 1` has a snapshot,
        // we were at a FullConfig snapshot and just crossed it backwards → restore before.
        let crossed_idx = pos_before.saturating_sub(1);
        if let Some((before, _)) = self.api_state_history.get(&crossed_idx) {
            self.api_state = before.clone();
            // We've moved past this snapshot; find the previous snapshot we're now "in"
            self.current_api_snapshot_idx = self.api_state_history.keys()
                .filter(|&&k| k < crossed_idx)
                .max()
                .copied();
        }
    }

    /// After redo: if we crossed a FullConfig snapshot forward, restore the "after" api_state.
    fn restore_api_state_for_redo(&mut self, pos_before: usize) {
        // Redo moves position forward by 1. If position `pos_before` has a snapshot,
        // we just crossed it forward → restore after.
        let crossed_idx = pos_before;
        if let Some((_, after)) = self.api_state_history.get(&crossed_idx) {
            self.api_state = after.clone();
            self.current_api_snapshot_idx = Some(crossed_idx);
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
    /// For high-res exports, spawns a background thread with progress updates.
    /// For normal GPU exports, runs synchronously (fast enough).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_custom_size(&mut self, transparent: bool, config: FractalConfig, _render_time_ms: f64) {
        use crate::renderer::{render, NoProgress, RenderJob};

        // Check if already exporting
        if self.png_export_progress.lock().unwrap().is_exporting {
            log::warn!("PNG export already in progress");
            return;
        }

        println!("Exporting at custom size: {}×{}", self.export_width, self.export_height);

        // Route based on the device's *actual* storage-buffer-binding
        // size, not the WebGPU spec floor. The app's device requests
        // adapter limits at startup (see gpu/device.rs), so on a
        // typical desktop reporting 2 GB+ bindings, 8K renders
        // (1 GB histogram) take the fast direct-histogram path
        // through FlameRenderer instead of falling back to the
        // CPU-histogram path. The fallback only fires when the
        // histogram truly exceeds what one storage buffer binding
        // can hold (e.g. 12K+ on a 2 GB device, 4K+ on WASM's
        // typical 128 MB limit).
        let max_binding = self.gpu.device.limits().max_storage_buffer_binding_size as u64;
        let hist_size = crate::export::histogram_size_bytes(self.export_width, self.export_height);
        if hist_size > max_binding {
            println!(
                "  Routing through HighResExporter for {}x{} (histogram {} MB > device binding {} MB)",
                self.export_width, self.export_height,
                hist_size / (1024 * 1024),
                max_binding / (1024 * 1024)
            );
            self.export_high_res_cpu_background(transparent, config);
            return;
        }

        // Regular GPU export - runs synchronously (fast enough, uses main thread GPU)
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

    /// High-resolution CPU export in background thread with progress updates
    #[cfg(not(target_arch = "wasm32"))]
    fn export_high_res_cpu_background(&mut self, transparent: bool, config: FractalConfig) {
        use crate::export::HighResExporter;
        use crate::ui::PngExportProgress;
        use std::sync::{Arc, Mutex};

        let width = self.export_width;
        let height = self.export_height;
        let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
        let speed_factor = config.speed_factor;
        let max_iterations = config.max_iterations;

        // Set initial progress state
        {
            let mut p = self.png_export_progress.lock().unwrap();
            p.is_exporting = true;
            p.current_iterations = 0;
            p.total_iterations = max_iterations;
            p.status = "Starting export...".to_string();
        }

        // Clone the Arc for the background thread
        let progress_arc = Arc::clone(&self.png_export_progress);

        // Spawn background thread
        std::thread::spawn(move || {
            use crate::export::ExportProgress as HighResProgress;
            use std::time::Instant;

            let export_start = Instant::now();

            // Progress callback that updates UI state
            struct UiProgress {
                progress: Arc<Mutex<PngExportProgress>>,
                total: u64,
            }

            impl HighResProgress for UiProgress {
                fn on_dispatch(&mut self, current: u64, total: u64) {
                    // Estimate iterations based on dispatch progress
                    let iterations = (current as f64 / total as f64 * self.total as f64) as u64;
                    if let Ok(mut p) = self.progress.lock() {
                        p.current_iterations = iterations;
                        p.status = format!("Rendering... ({}/{})", current, total);
                    }
                }

                fn on_accumulating(&mut self, _samples: u64) {
                    if let Ok(mut p) = self.progress.lock() {
                        p.status = "Accumulating samples...".to_string();
                    }
                }

                fn on_tonemapping(&mut self) {
                    if let Ok(mut p) = self.progress.lock() {
                        p.status = "Tonemapping...".to_string();
                    }
                }

                fn on_complete(&mut self) {
                    if let Ok(mut p) = self.progress.lock() {
                        p.status = "Encoding PNG...".to_string();
                    }
                }
            }

            // Create exporter (creates its own GPU context)
            let exporter_result = pollster::block_on(HighResExporter::new(&config, width, height, None));

            let mut exporter = match exporter_result {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Failed to create high-res exporter: {}", e);
                    if let Ok(mut p) = progress_arc.lock() {
                        p.is_exporting = false;
                        p.status = format!("Error: {}", e);
                    }
                    return;
                }
            };

            // Run export with UI progress
            let mut progress = UiProgress {
                progress: Arc::clone(&progress_arc),
                total: max_iterations,
            };
            let rgba_result = pollster::block_on(exporter.export(&config, max_iterations, transparent, &mut progress));

            let rgba_data = match rgba_result {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to export: {}", e);
                    if let Ok(mut p) = progress_arc.lock() {
                        p.is_exporting = false;
                        p.status = format!("Error: {}", e);
                    }
                    return;
                }
            };

            let total_export_time_ms = export_start.elapsed().as_secs_f64() * 1000.0;

            // Build metadata
            let metadata = crate::png_metadata::PngMetadata::from_app_state(
                width,
                height,
                max_iterations,
                total_export_time_ms,
                iterations_per_thread,
                speed_factor,
                &config,
            );

            // Encode PNG with metadata
            match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                Ok(png_data) => {
                    // Update status
                    if let Ok(mut p) = progress_arc.lock() {
                        p.status = "Saving file...".to_string();
                    }

                    // Open file dialog (note: this blocks until user responds)
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PNG Image", &["png"])
                        .set_file_name("fractal.png")
                        .save_file()
                    {
                        if let Err(e) = std::fs::write(&path, png_data) {
                            eprintln!("Failed to save PNG: {}", e);
                            if let Ok(mut p) = progress_arc.lock() {
                                p.status = format!("Error saving: {}", e);
                            }
                        } else {
                            println!("PNG exported to: {} ({}×{}, {:.2}s)",
                                path.display(), width, height, total_export_time_ms / 1000.0);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to encode PNG: {}", e);
                    if let Ok(mut p) = progress_arc.lock() {
                        p.status = format!("Error encoding: {}", e);
                    }
                }
            }

            // Mark export complete
            if let Ok(mut p) = progress_arc.lock() {
                p.is_exporting = false;
                p.current_iterations = max_iterations;
                p.status = "Complete".to_string();
            }
        });
    }
}
