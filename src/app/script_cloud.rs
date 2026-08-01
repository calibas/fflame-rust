//! The Scripts panel's half of the online library.
//!
//! The panel raises a [`ScriptCloudRequest`]; App performs it in the
//! background and folds the answer back into [`ScriptCloudState`],
//! which the panel renders. Same shape as the variation fetch: the UI
//! never awaits, and a slow or absent network costs a frame nothing.
//!
//! # Being offline is not an error
//!
//! Every failure here is a status line, never a modal and never a
//! blocked panel. A user with no network still has their local scripts,
//! which is the whole point of the store being real storage rather than
//! a cache.

use std::sync::{Arc, Mutex};

use crate::api::types::{ApiVisibility, ScriptConflict, ScriptListItem, ScriptResponse};
use crate::app::App;
use crate::script::store;

/// Something the panel wants done against the online library.
#[derive(Debug, Clone)]
pub enum ScriptCloudRequest {
    /// Refresh the caller's own scripts.
    ListMine,
    /// Public search. An empty query lists whatever the server offers.
    Search(String),
    /// Publish a local script that has no cloud id yet.
    Publish { stem: String, source: String, visibility: ApiVisibility },
    /// Push an edit to a script that does have one, optimistically.
    Update { stem: String, cloud_id: String, source: String, version: u32 },
    /// Fetch a script's source and store it locally under a new stem,
    /// marked as somebody else's.
    Adopt { id: String },
    /// Overwrite an existing local script with the server's copy, in
    /// place.
    ///
    /// Distinct from [`Self::Adopt`] and it has to be: resolving a
    /// conflict on your OWN script by adopting would save a second copy
    /// under a freed stem AND mark it as somebody else's work. The
    /// script is still yours; only its content is being replaced.
    Refetch { stem: String, id: String },
    /// Remove a script from the server. The local copy is untouched —
    /// unpublishing and deleting are different intentions.
    Unpublish { stem: String, cloud_id: String },
}

/// What the panel renders.
#[derive(Default)]
pub struct ScriptCloudState {
    pub mine: Vec<ScriptListItem>,
    pub browse: Vec<ScriptListItem>,
    /// The query `browse` answers, so the panel can say what it is
    /// showing rather than implying the results are everything.
    pub browse_query: String,
    /// A request is in flight. The panel disables its buttons rather
    /// than letting a second click race the first.
    pub busy: bool,
    pub status: Option<String>,
    /// Set when the status is a failure, so it can be coloured without
    /// the panel parsing the message.
    pub status_is_error: bool,
    /// Somebody else wrote first. Held until the user chooses, because
    /// the choice — discard mine, or keep editing — is theirs and a
    /// dismissed banner would silently mean "keep editing".
    pub conflict: Option<(String, ScriptConflict)>,
    /// Whether `ListMine` has ever succeeded, so the panel can tell
    /// "nothing published" from "not looked yet".
    pub mine_loaded: bool,
    /// Bumped whenever the LOCAL script store changed behind the
    /// panel's back — an adoption, or a refetch.
    ///
    /// A counter rather than a flag because the panel only holds
    /// `&ScriptCloudState` and so cannot clear one; it remembers the
    /// last value it acted on and re-scans when they differ.
    pub library_generation: u64,
}

impl ScriptCloudState {
    fn ok(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
        self.status_is_error = false;
    }
    fn fail(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
        self.status_is_error = true;
    }
    /// A success that also changed the local store.
    fn adopted(&mut self, msg: impl Into<String>) {
        self.ok(msg);
        self.library_generation += 1;
    }
}

/// What a background request produced.
pub(super) enum ScriptCloudResult {
    Mine(Result<Vec<ScriptListItem>, String>),
    Browse(String, Result<Vec<ScriptListItem>, String>),
    /// A publish or update. Carries the stem so the link can be written
    /// against the right script.
    Saved(String, Result<ScriptResponse, ScriptCloudError>),
    Adopted(Result<ScriptResponse, String>),
    /// `(stem, response)` — the stem is the local script to overwrite.
    Refetched(String, Result<ScriptResponse, String>),
    Unpublished(String, Result<(), String>),
}

/// An update can fail in a way the user can act on.
pub(super) enum ScriptCloudError {
    /// Somebody wrote first.
    Conflict(ScriptConflict),
    Other(String),
}

pub(super) type ScriptCloudSlot = Arc<Mutex<Vec<ScriptCloudResult>>>;

impl App {
    /// Start whatever the panel asked for.
    pub(super) fn handle_script_cloud_request(&mut self, req: Option<ScriptCloudRequest>) {
        let Some(req) = req else { return };
        if self.script_cloud.busy {
            // A second click while one is in flight would race the
            // first and, for an update, could publish against a version
            // that is about to change.
            return;
        }
        self.script_cloud.busy = true;
        self.script_cloud.status = None;

        let slot = self.script_cloud_results.clone();
        let window = self.window.clone();
        let base = crate::api::API_BASE_URL.to_string();
        // WASM authenticates by cookie, so `get_auth_token` yields an
        // empty string there rather than None.
        let token = match self.config_manager.system_settings().get_auth_token() {
            Some(t) => t,
            None => {
                self.script_cloud.busy = false;
                self.script_cloud.fail("Sign in to use the online script library.");
                return;
            }
        };

        let job = async move {
            let mut api = crate::api::ApiState::new(&base);
            api.set_token(&token);
            run_request(&api, req).await
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

    /// Fold any finished request back into the panel's state.
    pub(super) fn poll_script_cloud(&mut self) {
        let drained: Vec<ScriptCloudResult> = match self.script_cloud_results.lock() {
            Ok(mut s) => std::mem::take(&mut *s),
            Err(_) => return,
        };
        if drained.is_empty() {
            return;
        }
        self.script_cloud.busy = false;

        for result in drained {
            match result {
                ScriptCloudResult::Mine(Ok(items)) => {
                    self.script_cloud.mine = items;
                    self.script_cloud.mine_loaded = true;
                }
                ScriptCloudResult::Mine(Err(e)) => {
                    self.script_cloud.fail(format!("Could not list your online scripts: {e}"))
                }
                ScriptCloudResult::Browse(q, Ok(items)) => {
                    self.script_cloud.browse_query = q;
                    self.script_cloud.browse = items;
                }
                ScriptCloudResult::Browse(_, Err(e)) => {
                    self.script_cloud.fail(format!("Search failed: {e}"))
                }
                ScriptCloudResult::Saved(stem, Ok(resp)) => {
                    // Record the id and version, or the next update has
                    // nothing to send and optimistic concurrency has
                    // nothing to compare.
                    let link = store::ScriptLink {
                        cloud_id: Some(resp.id.clone()),
                        version: Some(resp.version),
                        owner: Some(format!("{}/{}", resp.owner_display_name, resp.name)),
                        // Publishing your own script does not make it
                        // somebody else's — preserve whatever was there.
                        from_others: store::link_of(&stem).is_some_and(|l| l.from_others),
                    };
                    if let Err(e) = store::set_link(&stem, link) {
                        log::warn!("Could not record the cloud link for `{stem}`: {e}");
                    }
                    self.script_cloud.conflict = None;
                    self.script_cloud.ok(format!("Published `{stem}` (v{})", resp.version));
                    self.script_cloud.mine_loaded = false;
                }
                ScriptCloudResult::Saved(stem, Err(ScriptCloudError::Conflict(c))) => {
                    self.script_cloud.conflict = Some((stem, c));
                    self.script_cloud
                        .fail("Somebody else saved this script after you loaded it.");
                }
                ScriptCloudResult::Saved(_, Err(ScriptCloudError::Other(e))) => {
                    self.script_cloud.fail(format!("Publish failed: {e}"))
                }
                ScriptCloudResult::Adopted(Ok(resp)) => match adopt(&resp) {
                    Ok(stem) => self.script_cloud.adopted(format!(
                        "Saved `{stem}` from {} — it runs with the restrictions \
                         downloaded scripts get",
                        resp.owner_display_name
                    )),
                    Err(e) => self.script_cloud.fail(e),
                },
                ScriptCloudResult::Adopted(Err(e)) => {
                    self.script_cloud.fail(format!("Could not open that script: {e}"))
                }
                ScriptCloudResult::Refetched(stem, Ok(resp)) => {
                    // In place, and provenance preserved rather than
                    // set: replacing your own script's content with the
                    // server's does not make it somebody else's.
                    let was_theirs = store::link_of(&stem).is_some_and(|l| l.from_others);
                    match store::save(&stem, &resp.source) {
                        Ok(saved) => {
                            let _ = store::set_link(
                                &saved,
                                store::ScriptLink {
                                    cloud_id: Some(resp.id.clone()),
                                    version: Some(resp.version),
                                    owner: Some(format!(
                                        "{}/{}",
                                        resp.owner_display_name, resp.name
                                    )),
                                    from_others: was_theirs,
                                },
                            );
                            self.script_cloud.conflict = None;
                            self.script_cloud.library_generation += 1;
                            self.script_cloud
                                .ok(format!("`{saved}` replaced with version {}", resp.version));
                        }
                        Err(e) => self.script_cloud.fail(e),
                    }
                }
                ScriptCloudResult::Refetched(_, Err(e)) => {
                    self.script_cloud.fail(format!("Could not load the server's version: {e}"))
                }
                ScriptCloudResult::Unpublished(stem, Ok(())) => {
                    // Keep the local copy; drop only the link, so the
                    // script stops claiming to be a server script.
                    if let Err(e) = store::set_link(&stem, store::ScriptLink::default()) {
                        log::warn!("Could not clear the cloud link for `{stem}`: {e}");
                    }
                    self.script_cloud.ok(format!("`{stem}` is no longer published"));
                    self.script_cloud.mine_loaded = false;
                }
                ScriptCloudResult::Unpublished(_, Err(e)) => {
                    self.script_cloud.fail(format!("Could not unpublish: {e}"))
                }
            }
        }
    }
}

/// Store a fetched script locally, marked as somebody else's.
///
/// Named from the server's `name`, but through `free_stem`: a published
/// script may carry a name that collides with a shipped stem on an older
/// client even though the server rejects those, and a local collision
/// must not silently overwrite the user's own work either.
fn adopt(resp: &ScriptResponse) -> Result<String, String> {
    let desired = store::free_stem(&resp.name);
    let stem = store::save(&desired, &resp.source)?;
    store::set_link(
        &stem,
        store::ScriptLink {
            cloud_id: Some(resp.id.clone()),
            version: Some(resp.version),
            owner: Some(format!("{}/{}", resp.owner_display_name, resp.name)),
            // The whole point: this survives the save, so the script
            // keeps running under the cross-call restriction.
            from_others: true,
        },
    )?;
    Ok(stem)
}

async fn run_request(
    api: &crate::api::ApiState,
    req: ScriptCloudRequest,
) -> ScriptCloudResult {
    match req {
        ScriptCloudRequest::ListMine => {
            ScriptCloudResult::Mine(api.list_my_scripts(1, 100).await.map_err(|e| e.to_string()))
        }
        ScriptCloudRequest::Search(q) => {
            let r = api.search_public_scripts(&q, 1, 50).await.map_err(|e| e.to_string());
            ScriptCloudResult::Browse(q, r)
        }
        ScriptCloudRequest::Publish { stem, source, visibility } => {
            let r = api
                .create_script(&stem, &source, Some(visibility))
                .await
                .map_err(|e| ScriptCloudError::Other(e.to_string()));
            ScriptCloudResult::Saved(stem, r)
        }
        ScriptCloudRequest::Update { stem, cloud_id, source, version } => {
            let r = api
                .update_script(&cloud_id, &stem, &source, None, version)
                .await
                .map_err(|e| match ScriptConflict::from_error(&e) {
                    Some(c) => ScriptCloudError::Conflict(c),
                    None => ScriptCloudError::Other(e.to_string()),
                });
            ScriptCloudResult::Saved(stem, r)
        }
        ScriptCloudRequest::Adopt { id } => {
            ScriptCloudResult::Adopted(api.load_script(&id).await.map_err(|e| e.to_string()))
        }
        ScriptCloudRequest::Refetch { stem, id } => {
            let r = api.load_script(&id).await.map_err(|e| e.to_string());
            ScriptCloudResult::Refetched(stem, r)
        }
        ScriptCloudRequest::Unpublish { stem, cloud_id } => {
            let r = api.delete_script(&cloud_id).await.map_err(|e| e.to_string());
            ScriptCloudResult::Unpublished(stem, r)
        }
    }
}
