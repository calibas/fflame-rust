//! Fetching API-managed variations a flame references but the registry
//! does not hold, and the variation catalog.
//!
//! One rule set with the effect fetch (`effect_fetch`; docs/projects/
//! curated-content.md, step 6): each download an [`api::job`]
//! (`downloads::Downloads`), registered first and cached only once the
//! registry accepted it, a built-in's or a custom variation's name never
//! replaced. A missing variation pauses rendering (the flame would draw
//! wrong meanwhile); a missing effect does not (the picture is merely
//! un-effected).

use crate::api::job;
use crate::app::App;
use crate::ui::UiResponse;

const VARIATION_FETCH_TIMEOUT_SECS: u64 = 30;

impl App {
    /// Fetch these variation names; rendering pauses until all are in (or
    /// the batch times out). A new batch replaces one in flight.
    pub(super) fn trigger_variation_fetches(&mut self, names: Vec<String>) {
        if names.is_empty() {
            return;
        }
        log::info!("Fetching {} unknown variations from API: {:?}", names.len(), names);
        self.egui_layer.show_api_notification(&t!("api.loading_variations", names = names.join(", ")), false);
        self.variation_downloads.new_batch();
        self.paused = true;
        for name in names {
            let api = crate::api::ApiState::new(crate::api::API_BASE_URL);
            let window = self.window.clone();
            let wanted = name.clone();
            let download = job::spawn(async move { api.fetch_variation(&wanted).await }, move || window.request_redraw());
            self.variation_downloads.push(name, download);
        }
    }

    /// Split missing variations into "we can fetch this" and "you are
    /// missing a plugin", and act differently on each.
    ///
    /// From the config the two look identical — both are a name the
    /// registry does not know. They need opposite responses: one is a
    /// download away, the other never arrives, and telling somebody to
    /// wait for a fetch that cannot succeed is worse than telling them
    /// nothing. A flame is shared as names, so the second case is what
    /// opening somebody's flame that used their own plugin looks like.
    pub(super) fn report_or_fetch_missing(&mut self, missing: Vec<String>) {
        use crate::variations::MissingReason;

        let mut fetchable = Vec::new();
        let mut unresolvable = Vec::new();
        // A Rhai variation is carried, never fetched: one missing is a
        // carried source that did not compile (or a flame without it).
        let (keys, missing): (Vec<String>, Vec<String>) = missing.into_iter().partition(|n| crate::scripted::is_key(n));
        for key in keys {
            let name = crate::variations::global_registry().label(&key);
            let message = match crate::variations::custom::error_of(&key) {
                Some(error) => t!("api.variation_did_not_compile", name = name, error = error),
                None => t!("api.variation_not_carried", name = name),
            };
            self.egui_layer.show_api_notification(&message, true);
        }
        for name in missing {
            match crate::variations::classify_missing(&name, self.variation_catalog.as_ref()) {
                // Unknown means no catalog has been fetched. Being
                // offline is not evidence that a name is unlisted, so
                // this tries the fetch and lets it fail honestly.
                MissingReason::Downloadable | MissingReason::Unknown => fetchable.push(name),
                MissingReason::KnownButNotFetchable | MissingReason::NotInCatalog => unresolvable.push(name),
            }
        }

        if !unresolvable.is_empty() {
            self.egui_layer.show_api_notification(&t!("api.variations_need_a_plugin", names = unresolvable.join(", ")), true);
        }
        if !fetchable.is_empty() {
            // A guest's too: variations are publicly readable.
            self.trigger_variation_fetches(fetchable);
        }
    }

    /// Once a frame: register what came back; when the batch is done (or
    /// timed out), rebuild the shader and render again.
    pub(super) fn handle_variation_fetches(&mut self) {
        if !self.variation_downloads.busy() {
            return;
        }
        if self.variation_downloads.started.is_some_and(|t| t.elapsed() > std::time::Duration::from_secs(VARIATION_FETCH_TIMEOUT_SECS)) {
            log::error!("Variation fetch timed out after {}s", VARIATION_FETCH_TIMEOUT_SECS);
            let names = self.variation_downloads.names();
            for name in &names {
                crate::ui::inventory::download_failed(crate::inventory::Kind::Variation, name, &t!("catalog.download_timed_out"));
            }
            self.variation_downloads.abandon();
            self.egui_layer.show_api_notification(&t!("api.variation_fetch_timeout", names = names.join(", ")), true);
            self.finalize_variation_fetches();
            return;
        }

        let done = self.variation_downloads.take_done();
        if done.is_empty() {
            return;
        }
        let mut failed = Vec::new();
        for (name, result) in done {
            let outcome = result.map_err(|e| e.to_string()).and_then(|download| {
                // Register FIRST, cache second: a payload the registry
                // refuses would only be refused again at every start.
                crate::variations::global_registry_mut()
                    .register_from_api(&download, crate::provenance::Provenance::Api { version: download.version })?;
                if let Err(e) = crate::storage::variation_cache::save(&download) {
                    log::warn!("Failed to cache variation '{}': {}", download.name, e);
                }
                Ok(())
            });
            if let Err(e) = outcome {
                log::warn!("Variation '{name}' not installed: {e}");
                crate::ui::inventory::download_failed(crate::inventory::Kind::Variation, &name, &e);
                failed.push(name);
            }
        }
        if !self.variation_downloads.busy() {
            self.finalize_variation_fetches();
            if !failed.is_empty() {
                self.egui_layer.show_api_notification(&t!("api.variation_fetch_failed", names = failed.join(", ")), true);
            }
        }
    }

    /// Refresh the variation catalog once per session, in the
    /// background.
    ///
    /// Deliberately not gated on a fetch in flight and not pausing the
    /// render: the catalog is metadata for the panel, not something a
    /// frame depends on. A failure is a logged note and nothing else —
    /// the cached copy (or an empty listing) is a correct answer, and an
    /// app that renders fractals offline should not surface a modal
    /// because a catalog endpoint was unreachable.
    pub(super) fn refresh_variation_catalog(&mut self) {
        if !self.catalog_fetch_started {
            self.catalog_fetch_started = true;
            let api = crate::api::ApiState::new(crate::api::API_BASE_URL);
            let window = self.window.clone();
            self.catalog_job = Some(job::spawn(async move { api.list_variations().await }, move || window.request_redraw()));
            return;
        }
        let Some(result) = self.catalog_job.as_ref().and_then(job::Job::take) else { return };
        self.catalog_job = None;
        match result {
            Ok(items) => {
                log::info!("Variation catalog: {} entries", items.len());
                let catalog = crate::storage::variation_catalog::CachedCatalog { items, version: None };
                if let Err(e) = crate::storage::variation_catalog::save(&catalog) {
                    log::warn!("Could not cache the variation catalog: {e}");
                }
                self.variation_catalog = Some(catalog);
            }
            Err(e) => {
                // Offline is a normal state, not an error condition.
                log::info!(
                    "Variation catalog unavailable ({e}) — showing what is installed{}",
                    if self.variation_catalog.is_some() { " plus the last cached listing" } else { "" }
                );
            }
        }
    }

    /// Re-fetch downloaded variations the catalog says are stale.
    ///
    /// Deliberately the same path as a first install: `register_from_api`
    /// replaces a non-core entry outright and `variation_cache::save`
    /// overwrites, so "update" is "install again" and there is no second
    /// code path to keep in step.
    pub(super) fn handle_variation_updates(&mut self, ui_response: &UiResponse) {
        if ui_response.variation_update_requested.is_empty() {
            return;
        }
        let registry = crate::variations::global_registry();
        let names: Vec<String> = ui_response
            .variation_update_requested
            .iter()
            // Only a downloaded copy can be updated. A built-in cannot
            // be replaced, and a local plugin is the user's own file —
            // an "update" would overwrite their work.
            .filter(|n| registry.get(n).is_none_or(|v| v.provenance.is_cached_download()))
            .cloned()
            .collect();
        drop(registry);
        self.trigger_variation_fetches(names);
    }

    /// Handle the user clicking "Clear Variation Cache" in the Variations panel.
    pub(super) fn handle_clear_variation_cache(&mut self, ui_response: &UiResponse) {
        if !ui_response.clear_variation_cache_requested {
            return;
        }
        let _ = crate::storage::variation_catalog::clear();
        self.variation_catalog = None;
        self.catalog_fetch_started = false; // re-fetch on the next frame
        match crate::storage::variation_cache::clear_all() {
            Ok(count) => {
                crate::variations::global_registry_mut().clear_api();
                self.egui_layer.show_api_notification(&t!("api.variation_cache_cleared", count = count), false);
            }
            Err(e) => {
                log::error!("Failed to clear variation cache: {}", e);
                self.egui_layer.show_api_notification(&t!("api.variation_cache_clear_failed", error = e), true);
            }
        }
    }

    /// Resume rendering and rebuild the shader. Says nothing to the user:
    /// the two call sites (timeout, partial failure) each show their own.
    fn finalize_variation_fetches(&mut self) {
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
