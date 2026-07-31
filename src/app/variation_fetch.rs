//! Async fetching of API-managed variations when a flame references
//! variations not yet in the registry.

use crate::app::App;
use crate::api::types::VariationDownload;
use crate::ui::UiResponse;

const VARIATION_FETCH_TIMEOUT_SECS: u64 = 30;

impl App {
    /// Fire off async fetches for the given variation names.
    /// Pauses rendering until all complete (or timeout).
    pub(super) fn trigger_variation_fetches(&mut self, names: Vec<String>) {
        if names.is_empty() {
            return;
        }
        log::info!("Fetching {} unknown variations from API: {:?}", names.len(), names);

        self.egui_layer.show_api_notification(
            &rust_i18n::t!("api.loading_variations", names = names.join(", ")),
            false,
        );

        // New batch: anything still in flight from a previous one is now
        // a straggler and must not be counted against this batch.
        self.variation_fetch_epoch = self.variation_fetch_epoch.wrapping_add(1);
        let epoch = self.variation_fetch_epoch;
        if let Ok(mut slot) = self.variation_fetch_results.lock() {
            slot.clear();
        }

        self.variation_fetch_in_progress = true;
        self.variation_fetch_started = Some(web_time::Instant::now());
        self.variation_fetch_pending_count = names.len();
        self.variation_fetch_names = names.clone();
        self.paused = true;

        let base_url = crate::api::API_BASE_URL.to_string();
        let result_slot = self.variation_fetch_results.clone();

        for name in names {
            let url_name = name.clone();
            let slot = result_slot.clone();
            let base = base_url.clone();
            let window = self.window.clone();

            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let api = crate::api::ApiState::new(&base);
                let result = api.fetch_variation(&url_name).await
                    .map_err(|e| e.to_string());
                if let Ok(mut s) = slot.lock() {
                    s.push((epoch, url_name, result));
                }
                window.request_redraw();
            });

            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                let api = crate::api::ApiState::new(&base);
                let result = pollster::block_on(api.fetch_variation(&url_name))
                    .map_err(|e| e.to_string());
                if let Ok(mut s) = slot.lock() {
                    s.push((epoch, url_name, result));
                }
                window.request_redraw();
            });
        }
    }

    /// Poll for completed variation fetches, register successes, and finalize when done.
    /// Called every frame while a fetch is in progress.
    pub(super) fn handle_variation_fetches(&mut self) {
        if !self.variation_fetch_in_progress {
            return;
        }

        // Timeout watchdog
        if let Some(started) = self.variation_fetch_started {
            if started.elapsed() > std::time::Duration::from_secs(VARIATION_FETCH_TIMEOUT_SECS) {
                log::error!("Variation fetch timed out after {}s", VARIATION_FETCH_TIMEOUT_SECS);
                // Tell the user. `finalize` used to take a `had_failures`
                // flag and ignore it, so a timeout resumed rendering in
                // silence and the flame just drew wrong.
                let names = std::mem::take(&mut self.variation_fetch_names).join(", ");
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.variation_fetch_timeout", names = names),
                    true,
                );
                self.finalize_variation_fetches();
                return;
            }
        }

        // Drain any completed fetches
        let drained: Vec<(u64, String, Result<VariationDownload, String>)> = {
            let mut slot = match self.variation_fetch_results.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            std::mem::take(&mut *slot)
        };
        // Drop stragglers from an abandoned batch. Counting them here is
        // what finalized the current batch early.
        let epoch = self.variation_fetch_epoch;
        let mut stale = 0usize;
        let new_results: Vec<(String, Result<VariationDownload, String>)> = drained
            .into_iter()
            .filter_map(|(e, name, result)| {
                if e == epoch {
                    Some((name, result))
                } else {
                    stale += 1;
                    None
                }
            })
            .collect();
        if stale > 0 {
            log::debug!("Discarded {stale} variation fetch result(s) from an earlier batch");
        }

        if new_results.is_empty() {
            return;
        }

        let mut succeeded = 0;
        let mut failed = Vec::new();
        for (name, result) in new_results {
            self.variation_fetch_pending_count = self.variation_fetch_pending_count.saturating_sub(1);
            match result {
                Ok(download) => {
                    // Persist to cache, then register
                    if let Err(e) = crate::storage::variation_cache::save(&download) {
                        log::warn!("Failed to cache variation '{}': {}", download.name, e);
                    }
                    crate::variations::global_registry_mut().register_from_api(&download);
                    succeeded += 1;
                }
                Err(e) => {
                    log::warn!("Failed to fetch variation '{}': {}", name, e);
                    failed.push(name);
                }
            }
        }

        if succeeded > 0 {
            // Variations were added — the renderer needs to rebuild the shader
            // when it next renders. Mark for shader rebuild via flame_renderer.
            // (The next render will detect the new variations and rebuild.)
            log::info!("Registered {} new variations from API", succeeded);
        }

        if self.variation_fetch_pending_count == 0 {
            self.finalize_variation_fetches();
            if !failed.is_empty() {
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.variation_fetch_failed", names = failed.join(", ")),
                    true,
                );
            }
        }
    }

    /// Handle the user clicking "Clear Variation Cache" in the Variations panel.
    pub(super) fn handle_clear_variation_cache(&mut self, ui_response: &UiResponse) {
        if !ui_response.clear_variation_cache_requested {
            return;
        }
        match crate::storage::variation_cache::clear_all() {
            Ok(count) => {
                crate::variations::global_registry_mut().clear_api();
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.variation_cache_cleared", count = count),
                    false,
                );
            }
            Err(e) => {
                log::error!("Failed to clear variation cache: {}", e);
                self.egui_layer.show_api_notification(
                    &rust_i18n::t!("api.variation_cache_clear_failed", error = e),
                    true,
                );
            }
        }
    }

    /// Reset fetch state and rebuild the shader.
    ///
    /// Deliberately says nothing to the user: the two call sites need
    /// different messages (timeout vs. partial failure), so each shows
    /// its own. The old signature took a `had_failures` flag and ignored
    /// it, which is why a timeout was silent.
    fn finalize_variation_fetches(&mut self) {
        self.variation_fetch_in_progress = false;
        self.variation_fetch_started = None;
        self.variation_fetch_pending_count = 0;
        self.variation_fetch_names.clear();
        self.paused = false;
        // Trigger a render reset so the new shader (with the fetched variations)
        // takes effect immediately.
        if let Some(ref mut renderer) = self.flame_renderer {
            // Force a shader rebuild by re-loading the current config
            let config = self.config_manager.active_config().clone();
            let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                label: Some("Variation fetch reload encoder"),
            });
            renderer.load_config(
                &self.gpu.device,
                &mut encoder,
                &self.gpu.queue,
                &config,
                &config.palette,
                self.config_manager.system_settings().iterations_per_thread,
                self.config_manager.system_settings().burn_in,
            );
            self.gpu.queue.submit(std::iter::once(encoder.finish()));
        }
    }
}
