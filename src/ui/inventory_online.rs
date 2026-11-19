//! The Catalog's **Online** place (docs/archive/projects/catalog.md, *Phase 3*):
//! what the server holds for a family -- its **curated** items, or the
//! account's own (docs/projects/curated-content.md: anyone else's public
//! item is opened by its id or link, *Open by ID or link*) -- asked for
//! page by page, searched by the server, and drawn as the Catalog's rows
//! ([`crate::inventory::online`]); fractals as a grid of the server's
//! thumbnails (decision 2), animations as rows.
//!
//! Every request is an [`crate::api::job`]: a guest's goes unsigned, a
//! signed-in one carries the token, and a 401 to it signs the session
//! out. A list is kept per (kind, scope, search, sort), so going back to
//! an earlier search shows what it found; Refresh forgets them all.

use super::inventory::{CloudAccount, InventoryContext, InventoryResponse};
use crate::api::job::{self, Job};
use crate::api::{Browse, Scope};
use crate::inventory::online::{self, Item};
use crate::inventory::{Entry, EntryKey, Family, Group, Kind};
use std::collections::HashMap;
use std::sync::Arc;

/// A page's items and the cursor of the next, `None` at the end.
type PageResult = (Vec<Item>, Option<String>);

/// Question ids *Open by ID* asks under (apart from the app's).
const OPEN_QUESTION_BASE: u64 = 1 << 40;
static OPEN_QUESTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How long the search waits after the last key before asking.
const SEARCH_SETTLE_SECONDS: f64 = 0.4;
/// Thumbnails downloading at once.
const THUMBNAIL_JOBS: usize = 4;
/// A grid cell's thumbnail side.
pub(super) const CELL: f32 = 120.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ListKey {
    kind: Kind,
    scope: Scope,
    q: String,
    sort: &'static str,
    /// A fractal's render mode (`2d`, `3d`, `escape`, `simulation`).
    mode: Option<&'static str>,
}

/// The fractals' render-mode filter: the server's word, and the label.
const MODES: &[(Option<&str>, &str)] = &[
    (None, "browser.search_all"),
    (Some("2d"), "browser.search_2d"),
    (Some("3d"), "browser.search_3d"),
    (Some("escape"), "mode.escape"),
    (Some("simulation"), "mode.simulation"),
];

/// A fractal's kind as a line: its mode, and for a flame its transform
/// count and variations (an escape-time or simulation config has neither:
/// the formula or the model is the fractal, and "0T" would read as a
/// broken flame).
pub fn fractal_line(flame: &crate::api::types::FlameListItem) -> String {
    use crate::api::types::ApiRenderMode as M;
    let mode = t!(match flame.render_mode {
        M::TwoD => "fractal_gallery.render_mode_2d",
        M::ThreeD => "fractal_gallery.render_mode_3d",
        M::Escape => "fractal_gallery.render_mode_escape",
        M::Simulation => "fractal_gallery.render_mode_simulation",
        M::Unknown(_) => "fractal_gallery.render_mode_newer",
    })
    .to_string();
    match flame.render_mode {
        M::Escape | M::Simulation | M::Unknown(_) => mode,
        M::TwoD | M::ThreeD => {
            let names: Vec<String> = {
                let registry = crate::variations::global_registry();
                flame
                    .variation_names
                    .iter()
                    .map(|n| match registry.get(n) {
                        Some(v) => v.display_name.clone(),
                        None if crate::scripted::is_key(n) => t!("browser.rhai_variation").to_string(),
                        None => n.clone(),
                    })
                    .collect()
            };
            let variations = if names.len() <= 3 { names.join(", ") } else { format!("{}, +{}", names[..2].join(", "), names.len() - 2) };
            format!("{mode} | {}T | {variations}", flame.transform_count)
        }
    }
}

#[derive(Default)]
struct List {
    rows: Vec<(Entry, Item)>,
    next: Option<String>,
    /// The first page was asked for.
    started: bool,
    job: Option<Job<PageResult>>,
    error: Option<String>,
}

impl List {
    fn loading(&self) -> bool {
        self.job.is_some()
    }

    /// More pages to ask for.
    fn more(&self) -> bool {
        !self.started || (self.next.is_some() && self.error.is_none())
    }
}

/// What *Open by ID or link* found: what it does with it.
enum Opened {
    Flame(String, crate::api::FlameLoadResult),
    Animation(String),
    Palette(crate::api::types::PaletteItem),
    /// A script: its id, and whether it is curated (opened without
    /// asking); otherwise its code, to ask about.
    Script { id: String, title: String, trusted: bool, code: crate::curated::trust::Code },
    /// A built-in variation or effect: nothing to fetch, it is on this
    /// device (or downloads by name).
    Builtin(String, String),
}

/// What the info pane asks of the panel.
pub enum InfoAction {
    /// Show the app's own entry of an online item, on this device.
    ShowLocal(EntryKey),
}

/// The kinds a family lists online: a fractal, a palette, or the Rhai
/// kinds the scripts resource stores.
pub fn kinds(family: Family) -> Vec<Kind> {
    match family {
        Family::Palettes => vec![Kind::Palette],
        Family::Fractals => vec![Kind::Fractal, Kind::Animation],
        _ => crate::storage::library::ALL_PUBLISHED.iter().filter_map(|k| Kind::of_file(*k)).filter(|k| k.family() == family).collect(),
    }
}

pub struct OnlinePlace {
    pub scope: Scope,
    /// The fractals' render-mode filter.
    mode: Option<&'static str>,
    /// Each kind's sort, by the server's word.
    sorts: HashMap<Kind, &'static str>,
    /// The search as typed, and when it last changed; `settled` is what
    /// the lists ask for.
    typed: String,
    typed_at: f64,
    settled: String,
    lists: HashMap<ListKey, List>,
    /// The local list the rows were matched against (what the app holds).
    local: Option<Arc<Vec<Entry>>>,
    thumbs: Thumbnails,
    flame_job: Option<(String, Job<crate::api::FlameLoadResult>)>,
    delete_job: Option<(ListKey, String, String, Job<()>)>,
    /// A rename or a visibility of yours on its way: the list, the id,
    /// what it changes.
    patch_job: Option<(ListKey, String, crate::api::types::ItemPatch, Job<serde_json::Value>)>,
    /// A rename of yours being typed: the id, the name.
    renaming: Option<(String, String)>,
    /// Asked to delete, awaiting the confirmation: the list, id, name.
    confirm_delete: Option<(ListKey, String, String)>,
    status: Option<(String, bool)>,
    /// *Open by ID or link*: what is typed, and the request on its way.
    open_text: String,
    open_job: Option<Job<Opened>>,
    /// A script *Open by ID* found that nobody reviewed: the question
    /// asked about it, and its id.
    open_question: Option<(u64, String)>,
}

impl Default for OnlinePlace {
    fn default() -> Self {
        Self::new()
    }
}

impl OnlinePlace {
    pub fn new() -> Self {
        OnlinePlace {
            scope: Scope::Curated,
            mode: None,
            sorts: HashMap::new(),
            typed: String::new(),
            typed_at: 0.0,
            settled: String::new(),
            lists: HashMap::new(),
            local: None,
            thumbs: Thumbnails::default(),
            flame_job: None,
            delete_job: None,
            patch_job: None,
            renaming: None,
            confirm_delete: None,
            status: None,
            open_text: String::new(),
            open_job: None,
            open_question: None,
        }
    }

    /// Forget every list (Refresh, or the account changed).
    pub fn forget(&mut self) {
        self.lists.clear();
        self.status = None;
    }

    /// Signed out: your own lists go.
    pub fn signed_out(&mut self) {
        self.lists.retain(|k, _| k.scope != Scope::Mine);
        if self.scope == Scope::Mine {
            self.scope = Scope::Curated;
        }
    }

    fn sort(&self, kind: Kind) -> &'static str {
        self.sorts.get(&kind).copied().unwrap_or_else(|| online::sorts(kind)[0].0)
    }

    fn key(&self, kind: Kind) -> ListKey {
        let mode = if kind == Kind::Fractal { self.mode } else { None };
        ListKey { kind, scope: self.scope, q: self.settled.trim().to_string(), sort: self.sort(kind), mode }
    }

    /// An online entry and its item, by key, from any list kept.
    pub fn find(&self, key: &EntryKey) -> Option<(&Entry, &Item)> {
        self.lists.values().flat_map(|l| l.rows.iter()).find(|(e, _)| &e.key == key).map(|(e, i)| (e, i))
    }

    /// Once a frame, before drawing: the search settles, pages that came
    /// back are added, and rows are matched again when the local list
    /// changed. `query` is the panel's search box.
    pub fn update(&mut self, ctx: &egui::Context, query: &str, local: &Arc<Vec<Entry>>, out: &mut InventoryResponse) {
        let now = ctx.input(|i| i.time);
        if query != self.typed {
            self.typed = query.to_string();
            self.typed_at = now;
        }
        if self.typed != self.settled {
            if now - self.typed_at >= SEARCH_SETTLE_SECONDS {
                self.settled = self.typed.clone();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(SEARCH_SETTLE_SECONDS));
            }
        }
        let rematch = !self.local.as_ref().is_some_and(|l| Arc::ptr_eq(l, local));
        self.local = Some(local.clone());
        for (key, list) in &mut self.lists {
            let group = group_of(key.kind);
            if let Some(result) = list.job.as_ref().and_then(Job::take) {
                list.job = None;
                list.started = true;
                match result {
                    Ok((items, next)) => {
                        for item in items {
                            if list.rows.iter().any(|(_, i)| i.id() == item.id()) {
                                continue;
                            }
                            if let Some(e) = online::entry(&item, group.clone(), local) {
                                list.rows.push((e, item));
                            }
                        }
                        list.next = next;
                    }
                    Err(e) => list.error = Some(e.to_string()),
                }
            }
            if rematch {
                for (e, item) in &mut list.rows {
                    if let Some(fresh) = online::entry(item, group.clone(), local) {
                        *e = fresh;
                    }
                }
            }
        }
        if let Some((id, job)) = &self.flame_job {
            if let Some(result) = job.take() {
                match result {
                    Ok(loaded) => out.load_api_flame = Some((id.clone(), loaded)),
                    Err(e) => self.status = Some((t!("catalog.load_failed", error = e.to_string()).to_string(), true)),
                }
                self.flame_job = None;
            }
        }
        if let Some((key, id, patch, job)) = &self.patch_job {
            if let Some(result) = job.take() {
                match result {
                    Ok(_) => {
                        // The row says it at once; the server's list says
                        // it on the next Refresh.
                        if let Some(list) = self.lists.get_mut(key) {
                            for (e, item) in list.rows.iter_mut().filter(|(_, i)| i.id() == id) {
                                if let Some(name) = &patch.name {
                                    e.title = name.clone();
                                    match item {
                                        Item::Flame(f) => f.name = name.clone(),
                                        Item::Palette(p) => p.name = name.clone(),
                                        Item::Script(s) => s.display_name = name.clone(),
                                        Item::Animation(a) => a.name = name.clone(),
                                    }
                                }
                                if let (Some(v), Some(info)) = (&patch.visibility, e.online.as_mut()) {
                                    info.visibility = serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string));
                                }
                            }
                        }
                        self.status = Some((t!("catalog.changed").to_string(), false));
                    }
                    Err(e) => self.status = Some((t!("catalog.change_failed", error = e.to_string()).to_string(), true)),
                }
                self.patch_job = None;
            }
        }
        if let Some(result) = self.open_job.as_ref().and_then(Job::take) {
            self.open_job = None;
            match result {
                Ok(Opened::Flame(id, loaded)) => out.load_api_flame = Some((id, loaded)),
                Ok(Opened::Animation(id)) => out.load_api_animation = Some(id),
                Ok(Opened::Palette(p)) => {
                    self.status = Some((t!("catalog.opened_palette", name = p.name.clone()).to_string(), false));
                    out.apply_palette = Some(crate::api::sync::palette_from_item(&p));
                }
                Ok(Opened::Script { id, title, trusted, code }) => {
                    if trusted || crate::curated::trust::is_trusted(&code.hash) {
                        out.script_cloud_request = Some(crate::app::script_cloud::ScriptCloudRequest::Adopt { id });
                    } else {
                        // Somebody's code nobody reviewed: ask before it
                        // joins your files (curated::trust).
                        let qid = OPEN_QUESTION_BASE + OPEN_QUESTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        crate::curated::trust::ask(crate::curated::trust::Question { id: qid, what: title, owner: None, code: vec![code] });
                        self.open_question = Some((qid, id));
                    }
                }
                Ok(Opened::Builtin(kind, name)) => {
                    self.status = Some((t!("catalog.opened_builtin", kind = kind, name = name).to_string(), false));
                }
                Err(crate::resources::FetchError::NotFound(message)) => self.status = Some((message, true)),
                Err(e) => self.status = Some((e.to_string(), true)),
            }
        }
        if let Some((qid, id)) = &self.open_question {
            if let Some(answer) = crate::curated::trust::take_answer(*qid) {
                if answer != crate::curated::trust::Answer::No {
                    out.script_cloud_request = Some(crate::app::script_cloud::ScriptCloudRequest::Adopt { id: id.clone() });
                }
                self.open_question = None;
            }
        }
        if let Some((key, id, name, job)) = &self.delete_job {
            if let Some(result) = job.take() {
                match result {
                    Ok(()) => {
                        if let Some(list) = self.lists.get_mut(key) {
                            list.rows.retain(|(_, i)| i.id() != id);
                        }
                        self.status = Some((t!("catalog.deleted", name = name.clone()).to_string(), false));
                    }
                    Err(e) => self.status = Some((t!("catalog.delete_failed", error = e.to_string()).to_string(), true)),
                }
                self.delete_job = None;
            }
        }
    }

    /// Ask for a list's next page, unless one is on its way.
    fn ask(&mut self, key: &ListKey, account: &CloudAccount, ctx: &egui::Context) {
        let list = self.lists.entry(key.clone()).or_default();
        if list.loading() || !list.more() {
            return;
        }
        let api = account.api();
        let words = Browse {
            q: key.q.clone(),
            sort: Some(key.sort.to_string()),
            kind: online::script_wire(key.kind).map(str::to_string),
            cursor: list.next.clone(),
            per_page: 50,
            filters: key.mode.map(|m| vec![("render_mode".to_string(), m.to_string())]).unwrap_or_default(),
        };
        let (kind, scope) = (key.kind, key.scope);
        let wake = ctx.clone();
        list.error = None;
        list.job = Some(
            job::spawn(
                async move {
                    match kind {
                        Kind::Fractal => api.browse_flames(scope, &words).await.map(|p| (p.items.into_inner().into_iter().map(Item::Flame).collect(), p.next)),
                        Kind::Palette => api.browse_palettes(scope, &words).await.map(|p| (p.items.into_inner().into_iter().map(Item::Palette).collect(), p.next)),
                        Kind::Animation => api.browse_animations(scope, &words).await.map(|p| (p.items.into_inner().into_iter().map(Item::Animation).collect(), p.next)),
                        _ => api.browse_scripts(scope, &words).await.map(|p| (p.items.into_inner().into_iter().map(Item::Script).collect(), p.next)),
                    }
                },
                move || wake.request_repaint(),
            )
            .authed(account.signed_in),
        );
    }

    // ------------------------------------------------------------- the top

    /// The scope, the sort and Refresh, beside the panel's search; then
    /// *Open by ID or link*.
    pub fn render_top(&mut self, ui: &mut egui::Ui, family: Family, account: &CloudAccount) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.scope, Scope::Curated, t!("catalog.scope_curated")).on_hover_text(t!("catalog.scope_curated_tip"));
            let mine = ui.add_enabled(account.signed_in, egui::Button::selectable(self.scope == Scope::Mine, t!("catalog.scope_mine")));
            let mine = if account.signed_in { mine } else { mine.on_disabled_hover_text(t!("catalog.sign_in_for_mine")) };
            if mine.clicked() {
                self.scope = Scope::Mine;
            }
            if let Some(kind) = kinds(family).first().copied() {
                let sorts = online::sorts(kind);
                let current = self.sort(kind);
                let label = sorts.iter().find(|(w, _)| *w == current).map_or(current.to_string(), |(_, k)| t!(*k).to_string());
                egui::ComboBox::from_id_salt(("catalog_sort", family)).selected_text(label).show_ui(ui, |ui| {
                    for (word, key) in sorts {
                        if ui.selectable_label(current == *word, t!(*key)).clicked() {
                            for k in kinds(family) {
                                if online::sorts(k).iter().any(|(w, _)| w == word) {
                                    self.sorts.insert(k, word);
                                }
                            }
                        }
                    }
                });
            }
            if ui.button(t!("catalog.refresh")).clicked() {
                self.forget();
            }
        });
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.open_text)
                    .hint_text(t!("catalog.open_hint"))
                    .desired_width((ui.available_width() - 80.0).max(120.0)),
            )
            .on_hover_text(t!("catalog.open_tip"));
            super::vkb_sync(ui, &r, &mut self.open_text);
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let busy = self.open_job.is_some();
            if (ui.add_enabled(!busy && !self.open_text.trim().is_empty(), egui::Button::new(t!("catalog.open"))).clicked() || enter) && !busy {
                self.open(account, ui.ctx());
            }
            if busy {
                ui.spinner();
            }
        });
        if family == Family::Fractals {
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in MODES {
                    if ui.selectable_label(self.mode == *mode, t!(*label)).clicked() {
                        self.mode = *mode;
                    }
                }
            });
        }
        if let Some((text, is_error)) = &self.status {
            if *is_error {
                ui.colored_label(ui.visuals().error_fg_color, text);
            } else {
                ui.weak(text);
            }
        }
    }

    // ------------------------------------------------------------ the list

    /// The family's online lists, a group a kind (only `kind`, when the
    /// chips chose one): rows, or a fractal grid; the next page asked for
    /// as the end of a list comes into view. A click selects (a palette
    /// applies, a fractal loads, as on this device).
    /// Gives whether a narrow panel's selected row asked for its info.
    pub fn render_list(&mut self, ui: &mut egui::Ui, family: Family, only: Option<Kind>, selected: &mut Option<EntryKey>, narrow: bool, cx: &mut InventoryContext) -> bool {
        if self.scope == Scope::Mine && !cx.account.signed_in {
            ui.weak(t!("catalog.sign_in_for_mine"));
            return false;
        }
        let mut details = false;
        // Narrow: the selection's way to its info, above the lists.
        if narrow {
            if let Some(title) = selected.as_ref().and_then(|k| self.find(k)).map(|(e, _)| e.title.clone()) {
                ui.horizontal(|ui| {
                    ui.strong(title);
                    if ui.small_button("\u{203a}").on_hover_text(t!("catalog.details")).clicked() {
                        details = true;
                    }
                });
                ui.separator();
            }
        }
        let shown: Vec<Kind> = kinds(family).into_iter().filter(|k| only.is_none_or(|o| o == *k)).collect();
        let several = shown.len() > 1;
        let ctx = ui.ctx().clone();
        for kind in shown {
            let key = self.key(kind);
            if !self.lists.get(&key).is_some_and(|l| l.started || l.loading()) {
                self.ask(&key, &cx.account, &ctx);
            }
            if several {
                let n = self.lists.get(&key).map_or(0, |l| l.rows.len());
                ui.strong(format!("{} ({n}{})", t!(kind.label_key()), if self.lists.get(&key).is_some_and(List::more) { "+" } else { "" }));
            }
            let end_visible = if kind == Kind::Fractal {
                self.render_grid(ui, &key, selected, cx)
            } else {
                self.render_rows(ui, &key, selected, cx)
            };
            let list = self.lists.get(&key);
            if list.is_some_and(List::loading) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.weak(t!("catalog.loading"));
                });
            } else if let Some(err) = list.and_then(|l| l.error.clone()) {
                ui.colored_label(ui.visuals().error_fg_color, err);
                if ui.button(t!("catalog.retry")).clicked() {
                    self.ask(&key, &cx.account, &ctx);
                }
            } else if list.is_some_and(|l| l.started && l.rows.is_empty()) {
                ui.weak(t!("catalog.nothing_online"));
            } else if end_visible && list.is_some_and(List::more) {
                self.ask(&key, &cx.account, &ctx);
            }
            ui.add_space(6.0);
        }
        self.render_confirm_delete(&ctx, &cx.account);
        details
    }

    /// A list's rows; whether its last row is on screen.
    fn render_rows(&mut self, ui: &mut egui::Ui, key: &ListKey, selected: &mut Option<EntryKey>, cx: &mut InventoryContext) -> bool {
        let Some(list) = self.lists.get(key) else { return false };
        let mut clicked = None;
        let mut end_visible = list.rows.is_empty();
        for (i, (e, item)) in list.rows.iter().enumerate() {
            let r = if let Item::Palette(p) = item {
                palette_row(ui, e, p, selected.as_ref() == Some(&e.key))
            } else {
                let mut label = super::inventory::row_label(e);
                if let Some(info) = &e.online {
                    if !info.owner.is_empty() {
                        label = format!("{label}  · {}", info.owner);
                    }
                }
                ui.selectable_label(selected.as_ref() == Some(&e.key), label)
            };
            if r.clicked() {
                clicked = Some((e.key.clone(), item.clone()));
            }
            if i + 1 == list.rows.len() {
                end_visible = ui.is_rect_visible(r.rect);
            }
        }
        if let Some((k, item)) = clicked {
            *selected = Some(k);
            if let Item::Palette(p) = &item {
                super::inventory::apply_palette(cx.config_manager, crate::api::sync::palette_from_item(p));
            }
        }
        end_visible
    }

    /// Fractals as a grid of the server's thumbnails; whether its last cell
    /// is on screen. A click loads one.
    fn render_grid(&mut self, ui: &mut egui::Ui, key: &ListKey, selected: &mut Option<EntryKey>, cx: &mut InventoryContext) -> bool {
        let Some(list) = self.lists.get(key) else { return false };
        let rows: Vec<(Entry, Option<String>, String)> = list
            .rows
            .iter()
            .map(|(e, _)| (e.clone(), e.online.as_ref().and_then(|o| o.thumbnail_url.clone()), e.online.as_ref().map_or(String::new(), |o| o.id.clone())))
            .collect();
        let mut end_visible = rows.is_empty();
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (i, (e, thumb, id)) in rows.iter().enumerate() {
                let texture = thumb.as_ref().and_then(|u| self.thumbs.get(ui.ctx(), u, &cx.account));
                let is_selected = selected.as_ref() == Some(&e.key);
                let r = grid_cell(ui, e, texture.as_ref(), is_selected, thumb.is_some());
                if r.clicked() {
                    clicked = Some((e.key.clone(), id.clone()));
                }
                if i + 1 == rows.len() {
                    end_visible = ui.is_rect_visible(r.rect);
                }
            }
        });
        if let Some((k, id)) = clicked {
            *selected = Some(k);
            self.load_flame(&id, &cx.account, ui.ctx());
        }
        end_visible
    }

    /// *Open by ID or link*: read what was typed, find it on the server,
    /// and use it as a click on its row would -- a fractal or an animation
    /// loads, a palette applies, a script is opened (adopted). An id alone
    /// is looked for as each resource in turn.
    fn open(&mut self, account: &CloudAccount, ctx: &egui::Context) {
        let Some(target) = online::parse_open(&self.open_text) else {
            self.status = Some((t!("catalog.open_unreadable").to_string(), true));
            return;
        };
        let api = account.api();
        let wake = ctx.clone();
        self.status = None;
        let job = job::spawn(
            async move {
                use crate::inventory::online::OpenTarget;
                let not_found = || crate::resources::FetchError::NotFound(t!("catalog.open_not_found").to_string());
                let (resource, id) = match target {
                    OpenTarget::Id { resource, id } => (resource.map(str::to_string), id),
                    OpenTarget::Name { kind, name } => {
                        let r = api.resolve_curated(&kind, &name).await.map_err(|_| not_found())?;
                        match (r.resource.as_str(), r.id) {
                            ("variation" | "effect", _) => return Ok(Opened::Builtin(r.kind, r.name.unwrap_or(name))),
                            (res, Some(id)) => (Some(format!("{res}s")), id),
                            _ => return Err(not_found()),
                        }
                    }
                };
                let order: Vec<String> = match resource {
                    Some(r) => vec![r],
                    None => ["flames", "animations", "palettes", "scripts"].iter().map(|s| s.to_string()).collect(),
                };
                for r in order {
                    let found = match r.as_str() {
                        "flames" => api.load_flame_with_visibility(&id).await.ok().map(|l| Opened::Flame(id.clone(), l)),
                        "animations" => api.exists("animations", &id).await.ok().map(|_| Opened::Animation(id.clone())),
                        "palettes" => api.get_palette(&id).await.ok().map(Opened::Palette),
                        "scripts" => match api.load_script(&id).await {
                            Ok(s) => {
                                let title = if s.display_name.is_empty() { s.name.clone() } else { s.display_name.clone() };
                                let mut code = crate::curated::trust::code_of_source(&s.source);
                                code.title = title.clone();
                                Some(Opened::Script { id: id.clone(), title, trusted: s.curated.is_some(), code })
                            }
                            Err(_) => None,
                        },
                        _ => None,
                    };
                    if let Some(found) = found {
                        return Ok(found);
                    }
                }
                Err(not_found())
            },
            move || wake.request_repaint(),
        )
        .authed(account.signed_in);
        self.open_job = Some(job);
    }

    fn load_flame(&mut self, id: &str, account: &CloudAccount, ctx: &egui::Context) {
        let api = account.api();
        let flame_id = id.to_string();
        let wake = ctx.clone();
        let job = job::spawn(async move { api.load_flame_with_visibility(&flame_id).await }, move || wake.request_repaint()).authed(account.signed_in);
        self.status = None;
        self.flame_job = Some((id.to_string(), job));
    }

    // ------------------------------------------------------------ the info

    /// What an online entry is, and what can be done with it.
    pub fn render_info(&mut self, ui: &mut egui::Ui, key: &EntryKey, cx: &mut InventoryContext, out: &mut InventoryResponse) -> Option<InfoAction> {
        let (e, item) = self.find(key).map(|(e, i)| (e.clone(), i.clone()))?;
        let info = e.online.clone()?;
        ui.heading(&e.title);
        let mut line = t!(e.kind.label_key()).to_string();
        if !info.owner.is_empty() {
            line = format!("{line} · {}", t!("catalog.by_owner", owner = info.owner.clone()));
        }
        ui.label(egui::RichText::new(line).weak());
        if let Item::Flame(f) = &item {
            ui.label(egui::RichText::new(fractal_line(f)).weak());
        }
        match &info.curated {
            Some(c) => {
                let mut line = match &c.name {
                    Some(name) => t!("catalog.curated_as", name = name.clone()).to_string(),
                    None => t!("catalog.curated").to_string(),
                };
                if !c.current {
                    line = format!("{line} · {}", t!("catalog.curated_older"));
                }
                ui.label(line).on_hover_text(t!("catalog.curated_tip"));
            }
            None => {
                ui.label(t!("catalog.from_online"));
            }
        }
        if !info.updated.is_empty() {
            let mut when = t!("catalog.updated", date = info.updated.chars().take(10).collect::<String>()).to_string();
            if let Some(v) = &info.visibility {
                when = format!("{when} · {}", visibility_label(v));
            }
            ui.label(egui::RichText::new(when).weak());
        }
        if !e.authors.is_empty() {
            ui.label(t!("catalog.by", names = e.authors.join(", ")));
        }
        ui.separator();
        match &item {
            Item::Flame(_) => {
                if let Some(url) = &info.thumbnail_url {
                    match self.thumbs.get(ui.ctx(), url, &cx.account) {
                        Some(t) => {
                            let w = ui.available_width().clamp(120.0, 320.0);
                            ui.add(egui::Image::new(&t).fit_to_exact_size(egui::vec2(w, w)));
                        }
                        None => {
                            ui.spinner();
                        }
                    }
                    ui.add_space(4.0);
                }
            }
            Item::Palette(p) => {
                let palette = crate::api::sync::palette_from_item(p);
                let w = ui.available_width().clamp(120.0, 360.0);
                let tex = super::palette_library::palette_preview(ui.ctx(), &p.content_hash, &palette, [360, 24]);
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(w, 24.0)));
                ui.add_space(4.0);
            }
            Item::Script(_) | Item::Animation(_) => {}
        }
        if e.description.is_empty() {
            ui.weak(t!("catalog.no_description"));
        } else {
            super::markdown::show(ui, &e.description);
        }
        ui.separator();
        let mut action = None;
        let list_key = self.lists.iter().find(|(_, l)| l.rows.iter().any(|(r, _)| r.key == e.key)).map(|(k, _)| k.clone());
        let mine = list_key.as_ref().is_some_and(|k| k.scope == Scope::Mine);
        ui.horizontal_wrapped(|ui| match &item {
            Item::Palette(p) => {
                if ui.button(t!("catalog.use_palette")).clicked() {
                    super::inventory::apply_palette(cx.config_manager, crate::api::sync::palette_from_item(p));
                }
                if let Some(held) = &info.held {
                    if ui.button(t!("catalog.show_local")).on_hover_text(t!("catalog.held_palette")).clicked() {
                        action = Some(InfoAction::ShowLocal(held.clone()));
                    }
                }
            }
            Item::Flame(_) => {
                let loading = self.flame_job.as_ref().is_some_and(|(id, _)| *id == info.id);
                if ui.add_enabled(!loading, egui::Button::new(t!("catalog.use_fractal"))).clicked() {
                    self.load_flame(&info.id, &cx.account, ui.ctx());
                }
                if loading {
                    ui.spinner();
                }
            }
            Item::Animation(_) => {
                if ui.button(t!("catalog.use_animation")).on_hover_text(t!("catalog.use_animation_tip")).clicked() {
                    out.load_api_animation = Some(info.id.clone());
                }
            }
            Item::Script(_) => match &info.held {
                Some(held) => {
                    ui.weak(t!("catalog.held_script"));
                    if ui.button(t!("catalog.show_local")).clicked() {
                        action = Some(InfoAction::ShowLocal(held.clone()));
                    }
                }
                None => {
                    let busy = cx.script_cloud.busy;
                    if ui.add_enabled(!busy, egui::Button::new(t!("catalog.open_script"))).on_hover_text(t!("catalog.open_script_tip")).clicked() {
                        out.script_cloud_request = Some(crate::app::script_cloud::ScriptCloudRequest::Adopt { id: info.id.clone() });
                    }
                    if busy {
                        ui.spinner();
                    }
                }
            },
        });
        if matches!(item, Item::Script(_)) && info.held.is_none() {
            ui.weak(t!("catalog.open_script_tip"));
        }
        if let Some(status) = cx.script_cloud.status.as_deref().filter(|_| matches!(item, Item::Script(_))) {
            ui.weak(status);
        }
        if mine && matches!(item, Item::Flame(_) | Item::Palette(_)) {
            ui.separator();
            let resource = if matches!(item, Item::Flame(_)) { "flames" } else { "palettes" };
            let busy = self.patch_job.is_some() || self.delete_job.as_ref().is_some_and(|(_, id, _, _)| *id == info.id);
            let mut patch: Option<crate::api::types::ItemPatch> = None;
            ui.horizontal_wrapped(|ui| {
                if let Some((id, name)) = self.renaming.as_mut().filter(|(id, _)| *id == info.id) {
                    let _ = id;
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(160.0));
                    super::vkb_sync(ui, &r, name);
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button(t!("manage.rename")).clicked() || enter {
                        let name = name.trim().to_string();
                        if !name.is_empty() {
                            patch = Some(crate::api::types::ItemPatch { name: Some(name), visibility: None });
                        }
                        self.renaming = None;
                    } else if ui.button(t!("manage.cancel")).clicked() {
                        self.renaming = None;
                    }
                    return;
                }
                if ui.add_enabled(!busy, egui::Button::new(t!("manage.rename_dots"))).clicked() {
                    self.renaming = Some((info.id.clone(), e.title.clone()));
                }
                let public = info.visibility.as_deref() == Some("public");
                let (label, to) = if public {
                    (t!("manage.make_private"), crate::api::types::ApiVisibility::Private)
                } else {
                    (t!("manage.make_public"), crate::api::types::ApiVisibility::Public)
                };
                if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                    patch = Some(crate::api::types::ItemPatch { name: None, visibility: Some(to) });
                }
                if ui.add_enabled(!busy, egui::Button::new(t!("catalog.delete"))).clicked() {
                    if let Some(k) = list_key.clone() {
                        self.confirm_delete = Some((k, info.id.clone(), e.title.clone()));
                    }
                }
                if busy {
                    ui.spinner();
                }
            });
            if let (Some(patch), Some(key)) = (patch, list_key) {
                let api = cx.account.api();
                let (id, body) = (info.id.clone(), patch.clone());
                let wake = ui.ctx().clone();
                let job = job::spawn(async move { api.patch_item(resource, &id, &body).await }, move || wake.request_repaint()).authed(cx.account.signed_in);
                self.patch_job = Some((key, info.id.clone(), patch, job));
            }
        }
        action
    }

    /// The delete's confirmation, and the request once confirmed.
    fn render_confirm_delete(&mut self, ctx: &egui::Context, account: &CloudAccount) {
        let Some((key, id, name)) = self.confirm_delete.clone() else { return };
        let mut open = true;
        let mut decided = None;
        egui::Window::new(t!("catalog.delete_title")).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label(t!("catalog.delete_confirm", name = name.clone()));
            ui.horizontal(|ui| {
                if ui.button(t!("catalog.delete_yes")).clicked() {
                    decided = Some(true);
                }
                if ui.button(t!("catalog.delete_no")).clicked() {
                    decided = Some(false);
                }
            });
        });
        if !open || decided.is_some() {
            self.confirm_delete = None;
        }
        if decided == Some(true) {
            let api = account.api();
            let (kind, item_id) = (key.kind, id.clone());
            let wake = ctx.clone();
            let job = job::spawn(
                async move {
                    match kind {
                        Kind::Fractal => api.delete_flame(&item_id).await,
                        _ => api.delete_palette(&item_id).await,
                    }
                },
                move || wake.request_repaint(),
            )
            .authed(account.signed_in);
            self.delete_job = Some((key, id, name, job));
        }
    }
}

fn group_of(kind: Kind) -> Group {
    Group { order: 0, label: t!(kind.label_key()).to_string() }
}

fn visibility_label(v: &str) -> String {
    match v {
        "public" => t!("catalog.visibility_public").to_string(),
        "unlisted" => t!("catalog.visibility_unlisted").to_string(),
        "private" => t!("catalog.visibility_private").to_string(),
        other => other.to_string(),
    }
}

/// An online palette: its name and owner, and its gradient at the right.
fn palette_row(ui: &mut egui::Ui, e: &Entry, p: &crate::api::types::PaletteItem, selected: bool) -> egui::Response {
    let palette = crate::api::sync::palette_from_item(p);
    let mut title = e.title.clone();
    if let Some(owner) = e.online.as_ref().map(|o| o.owner.as_str()).filter(|o| !o.is_empty()) {
        title = format!("{title}  · {owner}");
    }
    super::inventory::palette_row(ui, &p.content_hash, &title, &palette, selected)
}

/// A fractal's cell: its thumbnail (a placeholder until it is in) and its
/// name under it.
pub(super) fn grid_cell(ui: &mut egui::Ui, e: &Entry, texture: Option<&egui::TextureHandle>, selected: bool, has_thumb: bool) -> egui::Response {
    grid_cell_lazy(ui, e, selected, || (texture.cloned(), has_thumb))
}

/// A cell whose picture is asked for only once it is laid out and on
/// screen (`picture` gives the texture, if it is in, and whether one is
/// coming): its own rect decides, not a guess from the cursor -- in a
/// wrapped row the cursor is not where the cell lands, and a cell
/// scrolled half out of view was taken for one off screen.
pub(super) fn grid_cell_lazy(ui: &mut egui::Ui, e: &Entry, selected: bool, picture: impl FnOnce() -> (Option<egui::TextureHandle>, bool)) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::vec2(CELL, CELL + 18.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let (texture, has_thumb) = picture();
        let texture = texture.as_ref();
        let visuals = ui.visuals();
        let img = egui::Rect::from_min_size(rect.min, egui::vec2(CELL, CELL));
        match texture {
            Some(t) => {
                egui::Image::new(t).fit_to_exact_size(img.size()).paint_at(ui, img);
            }
            None => {
                ui.painter().rect_filled(img, 3.0, visuals.extreme_bg_color);
                if has_thumb {
                    ui.painter().text(img.center(), egui::Align2::CENTER_CENTER, "…", egui::FontId::proportional(18.0), visuals.weak_text_color());
                }
            }
        }
        if selected {
            ui.painter().rect_stroke(img, 3.0, visuals.selection.stroke, egui::StrokeKind::Inside);
        } else if r.hovered() {
            ui.painter().rect_stroke(img, 3.0, visuals.widgets.hovered.bg_stroke, egui::StrokeKind::Inside);
        }
        let name_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, img.max.y + 2.0), rect.max);
        let galley = ui.painter().layout_no_wrap(e.title.clone(), egui::FontId::proportional(12.0), visuals.text_color());
        ui.painter().with_clip_rect(name_rect).galley(name_rect.left_top(), galley, visuals.text_color());
    }
    let tip = match e.online.as_ref().map(|o| o.owner.as_str()).filter(|o| !o.is_empty()) {
        Some(owner) => format!("{}\n{}", e.title, t!("catalog.by_owner", owner = owner)),
        None => e.title.clone(),
    };
    r.on_hover_text(tip)
}

// ------------------------------------------------------------- thumbnails

enum Thumb {
    Loading(Job<egui::ColorImage>),
    Ready(egui::TextureHandle),
    Failed,
}

/// The server's thumbnails, by URL (it changes when the image does, so a
/// URL's image never goes stale): downloaded a few at a time, decoded off
/// the frame, and kept in the shared thumbnail cache (on disk on desktop,
/// within its limit).
#[derive(Default)]
struct Thumbnails {
    map: HashMap<String, Thumb>,
}

impl Thumbnails {
    /// The thumbnail's texture, once it is in; asks for it the first time.
    fn get(&mut self, ctx: &egui::Context, url: &str, account: &CloudAccount) -> Option<egui::TextureHandle> {
        if let Some(Thumb::Loading(job)) = self.map.get(url) {
            if let Some(result) = job.take() {
                let t = match result {
                    Ok(image) => Thumb::Ready(ctx.load_texture(format!("online_thumb_{url}"), image, egui::TextureOptions::LINEAR)),
                    Err(e) => {
                        log::info!("thumbnail {url}: {e}");
                        Thumb::Failed
                    }
                };
                self.map.insert(url.to_string(), t);
            }
        }
        match self.map.get(url) {
            Some(Thumb::Ready(t)) => return Some(t.clone()),
            Some(_) => return None,
            None => {}
        }
        if self.map.values().filter(|t| matches!(t, Thumb::Loading(_))).count() >= THUMBNAIL_JOBS {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return None;
        }
        let api = account.api();
        let u = url.to_string();
        let wake = ctx.clone();
        let job = job::spawn(
            async move {
                // The app's one thumbnail cache (its size limit, its
                // eviction), keyed by the URL's hash.
                let key = cache_key(&u);
                let cached = crate::storage::thumbnail_cache::shared_thumbnail_cache().load(&key);
                let image = match cached {
                    Some(image) => image,
                    None => {
                        let bytes = api.fetch_thumbnail(&u).await?;
                        let image = image::load_from_memory(&bytes).map_err(|e| crate::resources::FetchError::Parse(e.to_string()))?.to_rgba8();
                        if let Err(e) = crate::storage::thumbnail_cache::shared_thumbnail_cache().save(&key, &image) {
                            log::info!("thumbnail {u} not cached: {e}");
                        }
                        image
                    }
                };
                Ok(egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw()))
            },
            move || wake.request_repaint(),
        );
        self.map.insert(url.to_string(), Thumb::Loading(job));
        None
    }
}

/// A thumbnail URL's key in the shared thumbnail cache: 16 hex digits,
/// the cache's own form (FNV-1a, as a config's key is; a URL and a config
/// do not meet in a 64-bit space).
fn cache_key(url: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in url.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn script(id: &str) -> Item {
        Item::Script(
            serde_json::from_value(serde_json::json!({
                "id": id, "name": id, "display_name": format!("Script {id}"), "kind": "coloring",
                "owner": {"id": "u1", "display_name": "ana"}, "updated_at": "2026-11-16T10:00:00Z"
            }))
            .unwrap(),
        )
    }

    fn frame(ctx: &egui::Context, time: f64) {
        let _ = ctx.run_ui(egui::RawInput { time: Some(time), ..Default::default() }, |_| {});
    }

    /// Update until the list's page is in.
    fn settle(place: &mut OnlinePlace, ctx: &egui::Context, local: &Arc<Vec<Entry>>, key: &ListKey) {
        for _ in 0..500 {
            place.update(ctx, "", local, &mut InventoryResponse::default());
            if place.lists.get(key).is_some_and(|l| l.started) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("the page never came");
    }

    /// A page's items become rows once each (pages may repeat an item:
    /// clients de-duplicate by id), its cursor is kept for the next page,
    /// and when the device's list changes the rows say what it now holds
    /// (an adopted script's file).
    #[test]
    fn pages_add_rows_once_and_follow_what_the_app_holds() {
        let ctx = egui::Context::default();
        frame(&ctx, 0.0);
        let mut place = OnlinePlace::new();
        let key = ListKey { kind: Kind::Coloring, scope: Scope::Public, q: String::new(), sort: "recent", mode: None };
        let page = (vec![script("s1"), script("s1"), script("s2")], Some("c2".to_string()));
        place.lists.insert(key.clone(), List { job: Some(job::spawn(async move { Ok(page) }, || {})), ..Default::default() });
        let none = Arc::new(Vec::new());
        settle(&mut place, &ctx, &none, &key);
        let list = &place.lists[&key];
        assert_eq!(list.rows.len(), 2, "s1 once");
        assert_eq!(list.next.as_deref(), Some("c2"));
        assert!(list.more());
        assert_eq!(place.find(&EntryKey::Online(Kind::Coloring, "s2".into())).map(|(e, _)| e.title.as_str()), Some("Script s2"));

        let mut adopted = place.lists[&key].rows[0].0.clone();
        adopted.key = EntryKey::File { store: crate::storage::library::Store::Formulas, file: "s1.rhai".into(), shipped: false };
        adopted.online = None;
        adopted.link = Some(crate::storage::links::Link { cloud_id: Some("s1".into()), ..Default::default() });
        let local = Arc::new(vec![adopted.clone()]);
        place.update(&ctx, "", &local, &mut InventoryResponse::default());
        let held = place.find(&EntryKey::Online(Kind::Coloring, "s1".into())).and_then(|(e, _)| e.online.as_ref()?.held.clone());
        assert_eq!(held, Some(adopted.key), "the row knows the file it was opened into");
    }

    /// The search is asked for once the typing stops, not at every key.
    #[test]
    fn the_search_settles_before_it_is_asked() {
        let ctx = egui::Context::default();
        let local = Arc::new(Vec::new());
        let mut place = OnlinePlace::new();
        frame(&ctx, 10.0);
        place.update(&ctx, "fire", &local, &mut InventoryResponse::default());
        assert_eq!(place.key(Kind::Fractal).q, "", "not yet");
        frame(&ctx, 10.1);
        place.update(&ctx, "fire", &local, &mut InventoryResponse::default());
        assert_eq!(place.key(Kind::Fractal).q, "", "still typing, as far as it knows");
        frame(&ctx, 10.5);
        place.update(&ctx, "fire", &local, &mut InventoryResponse::default());
        assert_eq!(place.key(Kind::Fractal).q, "fire");
    }

    fn flame(render_mode: crate::api::types::ApiRenderMode, transform_count: i32, variations: &[&str]) -> crate::api::types::FlameListItem {
        serde_json::from_value(serde_json::json!({
            "id": "id", "name": "Reef", "render_mode": render_mode, "transform_count": transform_count,
            "variation_names": variations, "has_3d": false, "created_at": "", "updated_at": ""
        }))
        .unwrap()
    }

    /// A simulation or an escape-time config has no transforms and no
    /// variations, legitimately: its line says what it is, not "0T".
    #[test]
    fn a_simulation_line_has_no_transform_columns() {
        use crate::api::types::ApiRenderMode as M;
        for mode in [M::Simulation, M::Escape] {
            let line = fractal_line(&flame(mode, 0, &[]));
            assert!(!line.contains("0T") && !line.contains('|'), "{line}");
        }
    }

    /// A flame's line keeps its transform count and its variations, by
    /// their display names, the rest counted.
    #[test]
    fn a_flame_line_keeps_its_transform_count_and_variations() {
        use crate::api::types::ApiRenderMode as M;
        assert!(fractal_line(&flame(M::TwoD, 3, &["linear", "swirl"])).ends_with("| 3T | Linear, Swirl"));
        assert!(fractal_line(&flame(M::ThreeD, 2, &["a", "b", "c", "d"])).ends_with("| 2T | a, b, +2"));
    }

    /// The mode filter asks the server by its word, for fractals only;
    /// simulations are searchable (the server's render_mode knows them
    /// since its migration 102).
    #[test]
    fn the_mode_filter_is_the_servers_word() {
        let mut place = OnlinePlace::new();
        place.mode = Some("simulation");
        assert_eq!(place.key(Kind::Fractal).mode, Some("simulation"));
        assert_eq!(place.key(Kind::Palette).mode, None, "palettes have no mode");
        let words: Vec<_> = MODES.iter().filter_map(|(m, _)| *m).collect();
        assert_eq!(words, ["2d", "3d", "escape", "simulation"]);
    }

    /// Each family lists online what the server stores of its kinds.
    #[test]
    fn families_list_their_kinds_online() {
        assert_eq!(kinds(Family::Palettes), vec![Kind::Palette]);
        assert_eq!(kinds(Family::Fractals), vec![Kind::Fractal, Kind::Animation], "animations are a kind of the Fractals family");
        assert_eq!(kinds(Family::Simulation), vec![Kind::SimModel, Kind::SimColoring]);
        assert!(kinds(Family::Escape).contains(&Kind::Formula) && kinds(Family::Escape).contains(&Kind::Coloring));
        assert_eq!(kinds(Family::Variations), vec![Kind::Variation], "people's Rhai variations, after the server's catalog");
    }
}
