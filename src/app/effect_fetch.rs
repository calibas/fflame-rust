//! Fetching effects a flame references but this build does not have, and
//! the effect catalog.
//!
//! One rule set with the variation fetch (`variation_fetch`; docs/projects/
//! curated-content.md, step 6): each download an [`api::job`]
//! (`downloads::Downloads`), registered first and cached only once the
//! registry accepted it, a built-in's or a custom effect's name never
//! replaced.
//!
//! Two differences worth naming. A missing *variation* is visible from
//! the config alone, so `missing_variations_in` scans the flame; a missing
//! *effect* depends on what the registry holds right now, so it is
//! recorded where the answer is already known -- `EffectChain` looks each
//! one up while compiling and finds nothing -- and this drains that set.
//! And this does **not** pause rendering: a flame with a missing
//! variation renders something wrong, a flame with a missing effect
//! renders the un-effected image, a reasonable thing to look at while the
//! fetch runs.

use crate::api::job;
use crate::app::App;

impl App {
    pub(super) fn handle_effect_fetches(&mut self) {
        self.poll_effect_fetches();

        let missing = crate::effects::take_missing_effects();
        if missing.is_empty() {
            return;
        }
        let fresh: Vec<String> = missing.into_iter().filter(|n| !self.effect_fetch_attempted.contains(n)).collect();
        self.fetch_effects(fresh);
    }

    /// Re-fetch downloaded effects the catalog says are stale: the path
    /// that installs, as the variations' update is (`register_from_api`
    /// replaces a download, the cache write overwrites).
    pub(super) fn handle_effect_updates(&mut self, ui_response: &crate::ui::UiResponse) {
        if ui_response.effect_update_requested.is_empty() {
            return;
        }
        // Only a download can be updated: a built-in cannot be replaced,
        // and one of the user's is theirs.
        let names: Vec<String> = {
            let registry = crate::effects::global_effect_registry();
            ui_response
                .effect_update_requested
                .iter()
                .filter(|n| registry.get(n).is_none_or(|i| i.provenance.is_cached_download()))
                .cloned()
                .collect()
        };
        self.fetch_effects(names);
    }

    /// The Effects panel's Clear Cache: the cached downloads and the
    /// catalog, as the variations' does; a flame using one fetches it
    /// again.
    pub(super) fn handle_clear_effect_cache(&mut self, ui_response: &crate::ui::UiResponse) {
        // The Effects panel's Reset: a temporal effect's history starts
        // over at the next frame.
        if ui_response.reset_effect_history {
            self.effect_timeline.reset();
        }
        if !ui_response.clear_effect_cache_requested {
            return;
        }
        let _ = crate::storage::effect_catalog::clear();
        self.effect_catalog = None;
        self.effect_catalog_started = false;
        self.effect_fetch_attempted.clear();
        match crate::storage::effect_cache::clear_all() {
            Ok(count) => {
                crate::effects::global_effect_registry_mut().clear_api();
                self.egui_layer.show_api_notification(&t!("api.effect_cache_cleared", count = count), false);
            }
            Err(e) => {
                log::error!("Failed to clear the effect cache: {e}");
                self.egui_layer.show_api_notification(&t!("api.effect_cache_clear_failed", error = e.to_string()), true);
            }
        }
    }

    /// Fetch effects in the background (effects are publicly readable:
    /// a guest's too), each folded in by `poll_effect_fetches`. A name is
    /// tried once a session: the compile step records a still-missing
    /// name every frame, and without the guard a server that does not
    /// have it would be asked sixty times a second.
    fn fetch_effects(&mut self, names: Vec<String>) {
        for name in names {
            self.effect_fetch_attempted.insert(name.clone());
            log::info!("Fetching effect `{name}` from the API");
            let api = crate::api::ApiState::new(crate::api::API_BASE_URL);
            let window = self.window.clone();
            let wanted = name.clone();
            let download = job::spawn(async move { api.fetch_effect(&wanted).await }, move || window.request_redraw());
            self.effect_downloads.push(name, download);
        }
    }

    /// Refresh the effect catalog once per session, in the background.
    ///
    /// Same terms as the variation catalog: not gated on any fetch, not
    /// pausing the render, and a failure is an info-level log. The
    /// catalog is panel metadata, not something a frame depends on.
    pub(super) fn refresh_effect_catalog(&mut self) {
        if !self.effect_catalog_started {
            self.effect_catalog_started = true;
            let api = crate::api::ApiState::new(crate::api::API_BASE_URL);
            let window = self.window.clone();
            self.effect_catalog_job = Some(job::spawn(async move { api.list_effects().await }, move || window.request_redraw()));
            return;
        }
        let Some(result) = self.effect_catalog_job.as_ref().and_then(job::Job::take) else { return };
        self.effect_catalog_job = None;
        match result {
            Ok(items) => {
                let fetchable = items.iter().filter(|i| i.downloadable).count();
                log::info!("Effect catalog: {} entries, {fetchable} fetchable", items.len());
                let catalog = crate::storage::effect_catalog::CachedEffectCatalog { items, version: None };
                if let Err(e) = crate::storage::effect_catalog::save(&catalog) {
                    log::warn!("Could not cache the effect catalog: {e}");
                }
                self.effect_catalog = Some(catalog);
            }
            Err(e) => {
                // Offline is a normal state, not an error condition.
                log::info!(
                    "Effect catalog unavailable ({e}) — showing what is installed{}",
                    if self.effect_catalog.is_some() { " plus the last cached listing" } else { "" }
                );
            }
        }
    }

    fn poll_effect_fetches(&mut self) {
        let mut registered = 0usize;
        for (name, result) in self.effect_downloads.take_done() {
            let dl = match result {
                Ok(dl) => dl,
                Err(e) => {
                    log::warn!("Could not fetch effect `{name}`: {e}");
                    crate::ui::inventory::download_failed(crate::inventory::Kind::Effect, &name, &e.to_string());
                    continue;
                }
            };
            // Register FIRST, cache second: the refusal rules live in
            // `register_from_api`, and caching a payload this build
            // rejects would only mean rejecting it at every start.
            match crate::effects::global_effect_registry_mut().register_from_api(&dl, crate::provenance::Provenance::Api { version: dl.version }) {
                Ok(()) => {
                    if let Err(e) = crate::storage::effect_cache::save(&dl) {
                        log::warn!("Could not cache effect `{name}`: {e}");
                    }
                    registered += 1;
                }
                Err(e) => {
                    log::error!("Refusing effect `{name}`: {e}");
                    crate::ui::inventory::download_failed(crate::inventory::Kind::Effect, &name, &e);
                    self.egui_layer.show_api_notification(&format!("Cannot use effect `{name}`: {e}"), true);
                }
            }
        }
        if registered > 0 {
            // Nothing to invalidate: `compile_effects` runs every frame and
            // only skips names already in its map, so the next frame picks
            // the effect up now that the registry has it.
            log::info!("Registered {registered} effect(s) from the API");
        }
    }
}
