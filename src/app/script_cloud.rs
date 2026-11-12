//! The Scripts panel's and the Formula Editor's half of the online
//! library: one scripts resource for every kind
//! (docs/projects/rhai-scripts-online.md). A request names its local
//! file as a [`Local`]: a CPU script of the script store, or an entry of
//! the formula store, published under its own `INFO.id`
//! ([`crate::api::entries`]).
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

use crate::api::types::{ApiVisibility, ScriptConflict, ScriptCreateRequest, ScriptListItem, ScriptResponse};
use crate::app::App;
use crate::script::store;
use crate::storage::formula_store;
use crate::storage::library::Kind;
use crate::storage::links::Link;

/// The local file a request is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Local {
    /// A CPU script of the script store, by stem.
    Script(String),
    /// An entry of the formula store, by file name.
    Entry(String),
}

impl Local {
    /// Its stem or file name, as a person reads it.
    pub fn name(&self) -> &str {
        match self {
            Local::Script(s) | Local::Entry(s) => s,
        }
    }

    pub fn link(&self) -> Option<Link> {
        match self {
            Local::Script(s) => store::link_of(s),
            Local::Entry(f) => formula_store::link_of(f),
        }
    }

    fn set_link(&self, link: Link) -> Result<(), String> {
        match self {
            Local::Script(s) => store::set_link(s, link),
            Local::Entry(f) => formula_store::set_link(f, link),
        }
    }

    /// Write `source` over it; returns where it was stored. A plug-in
    /// entry's lists offer what was written.
    fn save(&self, source: &str) -> Result<Local, String> {
        match self {
            Local::Script(s) => store::save(s, source).map(Local::Script),
            Local::Entry(f) => {
                let file = formula_store::save(f, source)?;
                if formula_store::link_of(&file).is_some_and(|l| l.plugin) {
                    crate::storage::library::register_plugin(&file, source)?;
                }
                Ok(Local::Entry(file))
            }
        }
    }

    /// The create (or, with `version`, update) body.
    fn request(&self, source: &str, visibility: Option<ApiVisibility>, version: Option<u32>) -> Result<ScriptCreateRequest, String> {
        match self {
            Local::Script(s) => crate::api::sync::script_to_create_request(s, source, visibility, version),
            Local::Entry(f) => crate::api::entries::create_request(f, source, visibility, version),
        }
    }
}

/// Something the panel wants done against the online library.
#[derive(Debug, Clone)]
pub enum ScriptCloudRequest {
    /// Refresh the caller's own scripts, of one kind (its wire name) or
    /// every kind.
    ListMine(Option<String>),
    /// Public search, of one kind or every kind. An empty query lists
    /// whatever the server offers.
    Search(String, Option<String>),
    /// Publish a local script that has no cloud id yet. An entry goes
    /// under its own `INFO.id`, which the caller has written in.
    Publish { local: Local, source: String, visibility: ApiVisibility },
    /// Push an edit to a script that does have one, optimistically.
    Update { local: Local, cloud_id: String, source: String, version: u32 },
    /// Fetch a script's source and store it locally under a new name,
    /// marked as somebody else's: a CPU script in the script store, an
    /// entry in the formula store.
    Adopt { id: String },
    /// Overwrite an existing local script with the server's copy, in
    /// place.
    ///
    /// Distinct from [`Self::Adopt`] and it has to be: resolving a
    /// conflict on your OWN script by adopting would save a second copy
    /// under a freed stem AND mark it as somebody else's work. The
    /// script is still yours; only its content is being replaced.
    Refetch { local: Local, id: String },
    /// Remove a script from the server. The local copy is untouched —
    /// unpublishing and deleting are different intentions.
    Unpublish { local: Local, cloud_id: String },
}

impl ScriptCloudRequest {
    /// Whether it needs a signed-in account. Searching and opening a
    /// public script do not: the server serves them to anyone, so a
    /// guest (`online_account` off) and a signed-out user can browse.
    pub fn needs_account(&self) -> bool {
        match self {
            Self::Search(..) | Self::Adopt { .. } | Self::Refetch { .. } => false,
            Self::ListMine(_) | Self::Publish { .. } | Self::Update { .. } | Self::Unpublish { .. } => true,
        }
    }
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
    pub conflict: Option<(Local, ScriptConflict)>,
    /// Whether `ListMine` has ever succeeded, so the panel can tell
    /// "nothing published" from "not looked yet".
    pub mine_loaded: bool,
    /// Bumped whenever a LOCAL store changed behind a panel's back — an
    /// adoption, or a refetch (the Scripts panel and the Formula Editor
    /// each re-scan).
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
    /// A publish or update. Carries the local file so the link can be
    /// written against the right one.
    Saved(Local, Result<ScriptResponse, ScriptCloudError>),
    Adopted(Result<ScriptResponse, String>),
    /// `(local, response)` — the local file to overwrite.
    Refetched(Local, Result<ScriptResponse, String>),
    Unpublished(Local, Result<(), String>),
}

/// An update can fail in a way the user can act on.
pub(super) enum ScriptCloudError {
    /// Somebody wrote first.
    Conflict(ScriptConflict),
    /// A create under an id the server holds already: the first to
    /// publish an id owns it.
    IdTaken,
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
        // WASM authenticates by cookie, so `get_auth_token` yields an
        // empty string there rather than None. Searching and opening a
        // public script are a guest's too; the rest is your account's.
        let token = self.config_manager.system_settings().get_auth_token();
        if token.is_none() && req.needs_account() {
            let why = if self.config_manager.system_settings().online_account {
                "Sign in to publish scripts and see your own.".to_string()
            } else {
                t!("api.online_account_off").to_string()
            };
            self.script_cloud.fail(why);
            return;
        }
        self.script_cloud.busy = true;
        self.script_cloud.status = None;

        let slot = self.script_cloud_results.clone();
        let window = self.window.clone();
        let base = crate::api::API_BASE_URL.to_string();

        let job = async move {
            let mut api = crate::api::ApiState::new(&base);
            if let Some(t) = token {
                api.set_token(&t);
            }
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
                ScriptCloudResult::Saved(local, Ok(resp)) => {
                    // Record the id and version, or the next update has
                    // nothing to send and optimistic concurrency has
                    // nothing to compare. Publishing your own script
                    // does not make it somebody else's, nor stop it
                    // being a plug-in — preserve whatever was there.
                    let was = local.link().unwrap_or_default();
                    let link = Link { from_others: was.from_others, plugin: was.plugin, ..linked(&resp) };
                    if let Err(e) = local.set_link(link) {
                        log::warn!("Could not record the cloud link for `{}`: {e}", local.name());
                    }
                    self.script_cloud.conflict = None;
                    self.script_cloud.ok(format!("Published `{}` (v{})", local.name(), resp.version));
                    self.script_cloud.mine_loaded = false;
                }
                ScriptCloudResult::Saved(local, Err(ScriptCloudError::Conflict(c))) => {
                    self.script_cloud.conflict = Some((local, c));
                    self.script_cloud
                        .fail("Somebody else saved this script after you loaded it.");
                }
                ScriptCloudResult::Saved(local, Err(ScriptCloudError::IdTaken)) => {
                    self.script_cloud.fail(format!(
                        "`{}` cannot be published: its id is published already (by you from \
                         another file, or by somebody else). A copy of a published script needs \
                         an id of its own: give it a new one and publish that.",
                        local.name()
                    ))
                }
                ScriptCloudResult::Saved(_, Err(ScriptCloudError::Other(e))) => {
                    self.script_cloud.fail(format!("Publish failed: {e}"))
                }
                ScriptCloudResult::Adopted(Ok(resp)) => match adopt(&resp) {
                    Ok(Local::Script(stem)) => self.script_cloud.adopted(format!(
                        "Saved `{stem}` from {} — it runs with the restrictions \
                         downloaded scripts get",
                        resp.owner_display_name
                    )),
                    Ok(Local::Entry(file)) => self.script_cloud.adopted(format!(
                        "Saved `{file}` from {} to the formula library — open it in the \
                         Formula Editor to use it, or to make it a plug-in",
                        resp.owner_display_name
                    )),
                    Err(e) => self.script_cloud.fail(e),
                },
                ScriptCloudResult::Adopted(Err(e)) => {
                    self.script_cloud.fail(format!("Could not open that script: {e}"))
                }
                ScriptCloudResult::Refetched(local, Ok(resp)) => {
                    // In place, and provenance preserved rather than
                    // set: replacing your own script's content with the
                    // server's does not make it somebody else's.
                    let was = local.link().unwrap_or_default();
                    match local.save(&resp.source) {
                        Ok(saved) => {
                            let _ = saved.set_link(Link { from_others: was.from_others, plugin: was.plugin, ..linked(&resp) });
                            self.script_cloud.conflict = None;
                            self.script_cloud.library_generation += 1;
                            self.script_cloud
                                .ok(format!("`{}` replaced with version {}", saved.name(), resp.version));
                        }
                        Err(e) => self.script_cloud.fail(e),
                    }
                }
                ScriptCloudResult::Refetched(_, Err(e)) => {
                    self.script_cloud.fail(format!("Could not load the server's version: {e}"))
                }
                ScriptCloudResult::Unpublished(local, Ok(())) => {
                    // Keep the local copy; drop only the cloud link, so
                    // the script stops claiming to be a server script.
                    // A plug-in stays one.
                    let plugin = local.link().is_some_and(|l| l.plugin);
                    if let Err(e) = local.set_link(Link { plugin, ..Link::default() }) {
                        log::warn!("Could not clear the cloud link for `{}`: {e}", local.name());
                    }
                    self.script_cloud.ok(format!("`{}` is no longer published", local.name()));
                    self.script_cloud.mine_loaded = false;
                }
                ScriptCloudResult::Unpublished(_, Err(e)) => {
                    self.script_cloud.fail(format!("Could not unpublish: {e}"))
                }
            }
        }
    }
}

/// The link a server copy gives a local file: its id, version and
/// `owner/name`.
fn linked(resp: &ScriptResponse) -> Link {
    Link {
        cloud_id: Some(resp.id.clone()),
        version: Some(resp.version),
        owner: Some(format!("{}/{}", resp.owner_display_name, resp.name)),
        ..Default::default()
    }
}

/// Store a fetched script locally, marked as somebody else's: an entry
/// kind in the formula store ([`adopt_entry`]), a CPU one in the script
/// store. A kind this build does not know is refused rather than guessed.
pub(crate) fn adopt(resp: &ScriptResponse) -> Result<Local, String> {
    let kind = resp.kind.as_deref().unwrap_or("generator");
    match Kind::from_wire(kind) {
        Some(k) if k.is_entry() => adopt_entry(resp).map(Local::Entry),
        Some(_) => adopt_script(resp).map(Local::Script),
        None => Err(format!("`{}` is a {kind} script, which this version of the app does not know", resp.name)),
    }
}

/// A fetched entry, saved into the formula store under its own name
/// (`<name>.rhai`, with `-copy` while that name holds another file). Its
/// id is not re-minted: the copy is that script, until an edit is saved
/// (the Formula Editor then gives it an id of its own). The same online
/// script adopted again replaces its copy, keeping that copy's
/// provenance and plug-in flag.
fn adopt_entry(resp: &ScriptResponse) -> Result<String, String> {
    let file = adopt_file(&resp.name, &resp.id);
    let was = formula_store::link_of(&file).filter(|l| l.cloud_id.as_deref() == Some(resp.id.as_str()));
    let saved = Local::Entry(file).save(&resp.source)?;
    let link = match was {
        Some(was) => Link { from_others: was.from_others, plugin: was.plugin, ..linked(resp) },
        None => Link { from_others: true, ..linked(resp) },
    };
    saved.set_link(link)?;
    Ok(saved.name().to_string())
}

/// The formula store's name for an adopted entry: `<name>.rhai`, unless a
/// file of that name is linked to anything else.
fn adopt_file(name: &str, cloud_id: &str) -> String {
    let base = formula_store::file_name_for(&format!("{name}.rhai")).unwrap_or_else(|| "adopted.rhai".into());
    let stem = base.trim_end_matches(".rhai").to_string();
    let taken = |file: &str| formula_store::load(file).is_some() && formula_store::link_of(file).and_then(|l| l.cloud_id).as_deref() != Some(cloud_id);
    std::iter::once(base.clone())
        .chain(std::iter::once(format!("{stem}-copy.rhai")))
        .chain((2..1000).map(|n| format!("{stem}-copy-{n}.rhai")))
        .find(|f| !taken(f))
        .unwrap_or_else(|| format!("{stem}-copy-x.rhai"))
}

/// Store a fetched CPU script locally, marked as somebody else's.
///
/// Named from the server's `name`, but through `adopt_stem`: a published
/// script may carry a name that collides with a shipped stem on an older
/// client even though the server rejects those, and a local collision
/// must not silently overwrite the user's own work either -- the same
/// online script adopted again replaces its copy.
fn adopt_script(resp: &ScriptResponse) -> Result<String, String> {
    let desired = store::adopt_stem(&resp.name, &resp.id);
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
            ..Default::default()
        },
    )?;
    Ok(stem)
}

async fn run_request(
    api: &crate::api::ApiState,
    req: ScriptCloudRequest,
) -> ScriptCloudResult {
    match req {
        ScriptCloudRequest::ListMine(kind) => {
            ScriptCloudResult::Mine(api.list_my_scripts(kind.as_deref(), 1, 100).await.map_err(|e| e.to_string()))
        }
        ScriptCloudRequest::Search(q, kind) => {
            let r = api.search_public_scripts(&q, kind.as_deref(), 1, 50).await.map_err(|e| e.to_string());
            ScriptCloudResult::Browse(q, r)
        }
        ScriptCloudRequest::Publish { local, source, visibility } => {
            let r = match local.request(&source, Some(visibility), None) {
                Ok(req) => api.create_script(&req).await.map_err(|e| match &e {
                    // Not a version conflict (a create has no version):
                    // the id is somebody's already.
                    crate::resources::FetchError::Http { status: 409, .. } => ScriptCloudError::IdTaken,
                    _ => ScriptCloudError::Other(e.to_string()),
                }),
                Err(e) => Err(ScriptCloudError::Other(e)),
            };
            ScriptCloudResult::Saved(local, r)
        }
        ScriptCloudRequest::Update { local, cloud_id, source, version } => {
            let r = match local.request(&source, None, Some(version)) {
                Ok(req) => api.update_script(&cloud_id, &req).await.map_err(|e| match ScriptConflict::from_error(&e) {
                    Some(c) => ScriptCloudError::Conflict(c),
                    None => ScriptCloudError::Other(e.to_string()),
                }),
                Err(e) => Err(ScriptCloudError::Other(e)),
            };
            ScriptCloudResult::Saved(local, r)
        }
        ScriptCloudRequest::Adopt { id } => {
            ScriptCloudResult::Adopted(api.load_script(&id).await.map_err(|e| e.to_string()))
        }
        ScriptCloudRequest::Refetch { local, id } => {
            let r = api.load_script(&id).await.map_err(|e| e.to_string());
            ScriptCloudResult::Refetched(local, r)
        }
        ScriptCloudRequest::Unpublish { local, cloud_id } => {
            let r = api.delete_script(&cloud_id).await.map_err(|e| e.to_string());
            ScriptCloudResult::Unpublished(local, r)
        }
    }
}
