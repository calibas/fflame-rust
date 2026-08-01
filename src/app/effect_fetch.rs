//! Fetching effects a flame references but this build does not have.
//!
//! The same shape as [`super::variation_fetch`], and deliberately so:
//! an unknown effect and an unknown variation are the same problem
//! arriving through different doors.
//!
//! One difference worth naming. A missing *variation* is visible from
//! the config alone, so `missing_variations_in` scans the flame. A
//! missing *effect* depends on what the registry holds right now, so it
//! is recorded where the answer is already known — `EffectChain` looks
//! each one up while compiling and finds nothing. That is why this
//! drains a set rather than scanning.
//!
//! Unlike variations, this does **not** pause rendering. A flame with a
//! missing variation renders something wrong; a flame with a missing
//! effect renders the un-effected image, which is a reasonable thing to
//! look at while the fetch runs.

use crate::api::types::EffectDownload;
use crate::app::App;

impl App {
    pub(super) fn handle_effect_fetches(&mut self) {
        self.poll_effect_fetches();

        let missing = crate::effects::take_missing_effects();
        if missing.is_empty() {
            return;
        }

        let fresh: Vec<String> = missing
            .into_iter()
            .filter(|n| !self.effect_fetch_attempted.contains(n))
            .collect();
        if fresh.is_empty() {
            return;
        }

        // Signed out is not a failure to report: a flame using an effect
        // you do not have is uncommon, and a sign-in prompt triggered by
        // opening someone's flame would be worse than the missing
        // effect.
        let Some(token) = self.config_manager.system_settings().get_auth_token() else {
            for name in &fresh {
                log::info!("Effect `{name}` is unknown and nobody is signed in to fetch it");
                self.effect_fetch_attempted.insert(name.clone());
            }
            return;
        };

        for name in fresh {
            self.effect_fetch_attempted.insert(name.clone());
            log::info!("Fetching unknown effect `{name}` from the API");

            let slot = self.effect_fetch_results.clone();
            let window = self.window.clone();
            let base = crate::api::API_BASE_URL.to_string();
            let token = token.clone();
            let fetch_name = name.clone();

            let job = async move {
                let mut api = crate::api::ApiState::new(&base);
                api.set_token(&token);
                let r = api.fetch_effect(&fetch_name).await.map_err(|e| e.to_string());
                (fetch_name, r)
            };

            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let out = job.await;
                if let Ok(mut s) = slot.lock() {
                    s.push(out);
                }
                window.request_redraw();
            });

            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                let out = pollster::block_on(job);
                if let Ok(mut s) = slot.lock() {
                    s.push(out);
                }
                window.request_redraw();
            });
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
            let slot = self.effect_catalog_result.clone();
            let base = crate::api::API_BASE_URL.to_string();
            let window = self.window.clone();
            let token = self.config_manager.system_settings().get_auth_token();

            let job = async move {
                let mut api = crate::api::ApiState::new(&base);
                if let Some(t) = token {
                    api.set_token(&t);
                }
                api.list_effects().await.map_err(|e| e.to_string())
            };

            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let r = job.await;
                if let Ok(mut s) = slot.lock() { *s = Some(r); }
                window.request_redraw();
            });

            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                let r = pollster::block_on(job);
                if let Ok(mut s) = slot.lock() { *s = Some(r); }
                window.request_redraw();
            });
            return;
        }

        let drained = match self.effect_catalog_result.lock() {
            Ok(mut s) => s.take(),
            Err(_) => return,
        };
        let Some(result) = drained else { return };

        match result {
            Ok(items) => {
                let fetchable = items.iter().filter(|i| i.downloadable).count();
                log::info!(
                    "Effect catalog: {} entries, {fetchable} fetchable",
                    items.len()
                );
                let catalog = crate::storage::effect_catalog::CachedEffectCatalog {
                    items,
                    version: None,
                };
                if let Err(e) = crate::storage::effect_catalog::save(&catalog) {
                    log::warn!("Could not cache the effect catalog: {e}");
                }
                self.effect_catalog = Some(catalog);
            }
            Err(e) => {
                // Offline is a normal state, not an error condition.
                log::info!(
                    "Effect catalog unavailable ({e}) — showing what is installed{}",
                    if self.effect_catalog.is_some() {
                        " plus the last cached listing"
                    } else {
                        ""
                    }
                );
            }
        }
    }

    fn poll_effect_fetches(&mut self) {
        let drained: Vec<(String, Result<EffectDownload, String>)> =
            match self.effect_fetch_results.lock() {
                Ok(mut s) => std::mem::take(&mut *s),
                Err(_) => return,
            };
        if drained.is_empty() {
            return;
        }

        let mut registered = 0usize;
        for (name, result) in drained {
            match result {
                Ok(dl) => {
                    // Register FIRST, cache second. The refusal rules
                    // live in `register_from_api`, so caching a payload
                    // this build rejects would only mean rejecting it
                    // again on every future startup.
                    let outcome = crate::effects::global_effect_registry_mut()
                        .register_from_api(&dl);
                    match outcome {
                        Ok(()) => {
                            if let Err(e) = crate::storage::effect_cache::save(&dl) {
                                log::warn!("Could not cache effect `{name}`: {e}");
                            }
                            registered += 1;
                        }
                        Err(e) => {
                            log::error!("Refusing effect `{name}`: {e}");
                            self.egui_layer.show_api_notification(
                                &format!("Cannot use effect `{name}`: {e}"),
                                true,
                            );
                        }
                    }
                }
                Err(e) => log::warn!("Could not fetch effect `{name}`: {e}"),
            }
        }

        if registered > 0 {
            // Nothing to invalidate. `compile_effects` runs every frame
            // and only skips names already in its map; a failed compile
            // is never inserted, so the next frame picks the effect up
            // now that the registry has it.
            //
            // That same retry is why `effect_fetch_attempted` exists:
            // the compile step re-records a still-missing name on every
            // frame, and without the guard a server that does not have
            // the effect would be asked sixty times a second.
            log::info!("Registered {registered} effect(s) from the API");
        }
    }
}
