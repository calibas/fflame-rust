//! The Catalog panel (docs/archive/projects/catalog.md): every resource the app
//! knows, from [`crate::inventory`], to browse, read about and use.
//!
//! One view for every catalog (docs/projects/one-catalog-view.md): the
//! top chooses the catalog (fractals, palettes, variations, ...), its
//! filters (Category, Source, Owner, Type), the view (grid or list, the
//! cells' size), a search and the grouping; the list is the device's
//! entries and the server's pages together (`ui::inventory_view`); the
//! info pane says what the entry under the pointer is -- or, the pointer
//! elsewhere, the selected one -- who wrote it, where it came from, and
//! offers its **Use**. A click selects and loads or applies (one undo
//! step), except a variation or an effect, which the pane adds (D3).
//!
//! The selection is the panel's. Whether Use acts in the current mode is
//! `visibility::catalog_use`'s answer; whether a colouring suits the
//! current formula is the escape engine's own rule.
//!
//! The list is read once a frame, before any panel draws
//! ([`InventoryPanel::refresh`]), and [`shared`] with the pickers
//! (`ui::picker`), so every list of a kind is the same list. What the
//! server offers and the app lacks is listed too, its Use a **Download**
//! ([`request_download`], drained into the app's fetches once a frame).
//!
//! What the server holds -- curated, or your own -- comes page by page
//! (`ui::inventory_online`); Source chooses online, this device, or both.

pub use super::inventory_view::Source;
use super::inventory_view::{categories, has_grid, types, Filters, GroupBy, Owner, View};
use crate::config::{ConfigManager, ConfigPath, FractalConfig};
use crate::inventory::{self, Entry, EntryKey, Family, Kind};
use crate::provenance::Origin;
use crate::scene::palette::PaletteLibrary;
use crate::storage::library::Store;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// The list as last read, for the pickers.
static SHARED: RwLock<Option<Arc<Vec<Entry>>>> = RwLock::new(None);

/// Downloads asked for since the frame's last drain, and every name asked
/// for this session (its row says it is coming rather than offering it
/// again).
static DOWNLOADS: Mutex<(Vec<(Kind, String)>, Vec<(Kind, String)>)> = Mutex::new((Vec::new(), Vec::new()));

/// Curated online scripts asked for (L3): those not yet handed to the
/// script cloud, and every id asked for this session.
static ADOPTS: Mutex<(Vec<String>, Vec<String>)> = Mutex::new((Vec::new(), Vec::new()));

/// The next curated script to adopt, when the script cloud is free: one
/// at a time, as it takes one request at a time.
pub fn take_adopt(busy: bool) -> Option<String> {
    if busy {
        return None;
    }
    let mut a = ADOPTS.lock().ok()?;
    (!a.0.is_empty()).then(|| a.0.remove(0))
}

/// The curated online scripts the pickers offer (L3): asked for once a
/// session, while the pickers offer downloads; until the answer, none;
/// offline, none.
#[derive(Default)]
struct CuratedScripts {
    job: Option<crate::api::job::Job<Vec<crate::api::types::ScriptListItem>>>,
    asked: bool,
    items: Vec<crate::api::types::ScriptListItem>,
}

impl CuratedScripts {
    fn poll(&mut self) {
        if !self.asked && picker_downloads() && !cfg!(test) {
            self.asked = true;
            let api = crate::api::ApiState::new(crate::api::API_BASE_URL);
            let kinds: Vec<&'static str> = crate::storage::library::ALL_PUBLISHED
                .iter()
                .filter(|k| k.can_be_plugin())
                .filter_map(|k| k.wire())
                .collect();
            self.job = Some(crate::api::job::spawn(
                async move {
                    let mut all = Vec::new();
                    for kind in kinds {
                        let words = crate::api::Browse { kind: Some(kind.to_string()), per_page: 100, ..Default::default() };
                        all.extend(api.browse_scripts(crate::api::Scope::Curated, &words).await?.items.into_inner());
                    }
                    Ok(all)
                },
                || {},
            ));
        }
        if let Some(result) = self.job.as_ref().and_then(crate::api::job::Job::take) {
            self.job = None;
            match result {
                Ok(items) => self.items = items,
                Err(e) => log::info!("Curated scripts not fetched: {e}"),
            }
        }
    }
}

/// Downloads that failed this session, with the reason: their rows say so
/// and offer Download again.
static FAILED: Mutex<Vec<((Kind, String), String)>> = Mutex::new(Vec::new());

/// The app's fetch could not install `name` (`app::variation_fetch`,
/// `app::effect_fetch`): it is asked for no more, and its row says why.
pub fn download_failed(kind: Kind, name: &str, why: &str) {
    let item = (kind, name.to_string());
    if let Ok(mut d) = DOWNLOADS.lock() {
        d.1.retain(|i| *i != item);
    }
    if let Ok(mut f) = FAILED.lock() {
        f.retain(|(i, _)| *i != item);
        f.push((item, why.to_string()));
    }
}

/// Why the last download of the entry failed, if it did.
pub fn download_failure(e: &Entry) -> Option<String> {
    let EntryKey::Registered(kind, name) = &e.key else { return None };
    FAILED.lock().ok()?.iter().find(|((k, n), _)| k == kind && n == name).map(|(_, why)| why.clone())
}

/// A panel asked to show the Catalog at a catalog and source, with an
/// entry selected.
static OPEN: Mutex<Option<(Option<Family>, Source, Option<EntryKey>)>> = Mutex::new(None);

/// Show the Catalog at `family`, Source `source` (opening its panel): the
/// Catalog menu's rows, and what a panel whose online section moved into
/// the Catalog offers instead.
pub fn request_open(family: Family, source: Source) {
    if let Ok(mut o) = OPEN.lock() {
        *o = Some((Some(family), source, None));
    }
}

/// Show the Catalog at Source `source`, on the catalog it was last on
/// (the Catalog menu's Online...).
pub fn request_open_place(source: Source) {
    if let Ok(mut o) = OPEN.lock() {
        *o = Some((None, source, None));
    }
}

/// Show an entry in the Catalog, selected (the Rhai Editor's Show in
/// Catalog).
pub fn request_open_select(family: Family, key: EntryKey) {
    if let Ok(mut o) = OPEN.lock() {
        *o = Some((Some(family), Source::All, Some(key)));
    }
}

/// Whether the pickers offer what can be downloaded (L3; the user's
/// setting, `SystemSettings::picker_downloads`): set once a frame.
static PICKER_DOWNLOADS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Set whether the pickers offer downloads.
pub fn set_picker_downloads(on: bool) {
    PICKER_DOWNLOADS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the pickers offer downloads.
pub fn picker_downloads() -> bool {
    PICKER_DOWNLOADS.load(std::sync::atomic::Ordering::Relaxed)
}

/// The open asked for since the last call.
pub fn take_open() -> Option<(Option<Family>, Source, Option<EntryKey>)> {
    OPEN.lock().ok()?.take()
}

/// The Catalog menu's rows: a family each, then Online... (the desktop
/// bar's menu and the compact menu's submenu draw the same rows).
pub fn catalog_menu_rows(ui: &mut egui::Ui) {
    for family in Family::ALL {
        if ui.button(t!(family.label_key())).clicked() {
            request_open(family, Source::All);
            ui.close();
        }
    }
    ui.separator();
    if ui.button(t!("menu.catalog_online")).on_hover_text(t!("catalog.place_online_tip")).clicked() {
        request_open_place(Source::Online);
        ui.close();
    }
}

/// A line saying where something moved, and a button that shows it there.
pub fn moved_here(ui: &mut egui::Ui, what: &str, family: Family, source: Source) {
    ui.horizontal_wrapped(|ui| {
        ui.weak(what);
        if ui.button(t!("catalog.open_in_catalog")).clicked() {
            request_open(family, source);
        }
    });
}

/// The list every picker reads: the Catalog's, as last refreshed.
pub fn shared() -> Arc<Vec<Entry>> {
    SHARED.read().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// Ask for an entry the app lacks (`!installed`) to be downloaded.
pub fn request_download(e: &Entry) {
    if let EntryKey::Online(_, id) = &e.key {
        if let Ok(mut a) = ADOPTS.lock() {
            if !a.1.contains(id) {
                a.0.push(id.clone());
                a.1.push(id.clone());
            }
        }
        return;
    }
    let EntryKey::Registered(kind, name) = &e.key else { return };
    if let Ok(mut f) = FAILED.lock() {
        f.retain(|((k, n), _)| !(k == kind && n == name));
    }
    if let Ok(mut d) = DOWNLOADS.lock() {
        let item = (*kind, name.clone());
        if !d.1.contains(&item) {
            d.0.push(item.clone());
            d.1.push(item);
        }
    }
}

/// Whether a download of the entry was asked for this session.
pub fn download_requested(e: &Entry) -> bool {
    if let EntryKey::Online(_, id) = &e.key {
        return ADOPTS.lock().is_ok_and(|a| a.1.contains(id));
    }
    let EntryKey::Registered(kind, name) = &e.key else { return false };
    DOWNLOADS.lock().is_ok_and(|d| d.1.iter().any(|(k, n)| k == kind && n == name))
}

/// The downloads asked for since the last call, for the app to fetch.
pub fn take_downloads() -> Vec<(Kind, String)> {
    DOWNLOADS.lock().map(|mut d| std::mem::take(&mut d.0)).unwrap_or_default()
}

/// Below this width the list and the info are two screens (phase 5).
const NARROW: f32 = 560.0;

/// The online account as a request needs it: whether the session is
/// signed in, and its token (desktop; the web's cookie needs none).
#[derive(Debug, Clone, Default)]
pub struct CloudAccount {
    pub signed_in: bool,
    pub token: Option<String>,
}

impl CloudAccount {
    /// An API client for one request.
    pub fn api(&self) -> crate::api::ApiState {
        let mut api = crate::api::ApiState::new(crate::api::API_BASE_URL);
        if let Some(t) = self.token.as_deref().filter(|t| !t.is_empty()) {
            api.set_token(t);
        }
        api
    }
}

/// What the panel asks of the app this frame.
#[derive(Default)]
pub struct InventoryResponse {
    /// A fractal to load (`App::load_config_with_undo`).
    pub load_preset: Option<FractalConfig>,
    pub open_palette_editor: bool,
    /// A file to open in the Rhai Editor: its store, its name, and
    /// whether it is a shipped one.
    pub open_file: Option<(Store, String, bool)>,
    /// An online fractal loaded, by its id, to load as the Fractal
    /// Browser's Online tab did (one undo step; Save Online updates it).
    pub load_api_flame: Option<(String, crate::api::FlameLoadResult)>,
    /// An online script to open (adopt into your files).
    pub script_cloud_request: Option<crate::app::script_cloud::ScriptCloudRequest>,
    /// An online animation to load, by its id (as the Animation panel's
    /// list loads one).
    pub load_api_animation: Option<String>,
    /// A palette an online request found, to apply.
    pub apply_palette: Option<crate::scene::palette::Palette>,
    /// What running a script produced (a flame, a batch, an animation).
    pub run: super::script_run::RunResult,
    /// Clear Cache: the downloaded variations, or effects.
    pub clear_variation_cache: bool,
    pub clear_effect_cache: bool,
    /// Refresh: the app asks the server for its variation and effect
    /// catalogs again.
    pub refresh_catalogs: bool,
}

/// What the panel needs from the app to draw.
pub struct InventoryContext<'a> {
    pub config_manager: &'a mut ConfigManager,
    /// The palette library the app shows: its packs, as enabled and loaded.
    pub palette_library: &'a mut PaletteLibrary,
    /// The online account, for the Online place's requests.
    pub account: CloudAccount,
    /// The script cloud: busy (a request waits for it), its last word,
    /// a conflict to resolve.
    pub script_cloud: &'a crate::app::script_cloud::ScriptCloudState,
}

pub struct InventoryPanel {
    entries: Arc<Vec<Entry>>,
    revision: Option<u64>,
    family: Family,
    /// Category, Source, Owner, Type (docs/projects/one-catalog-view.md).
    filters: Filters,
    /// Each catalog's view: grid or list, the cells' size, the grouping.
    views: HashMap<Family, View>,
    query: String,
    selected: Option<EntryKey>,
    /// The entry under the pointer when the list was last drawn: the info
    /// pane shows it (the pane is drawn before the list), else the
    /// selection.
    hovered: Option<EntryKey>,
    /// Only the downloads the server has a newer version of.
    updates_only: bool,
    /// Narrow (a phone, a narrow dock): the list and the info are two
    /// screens, and this is the info's (phase 5; D7).
    details: bool,
    /// Whether the panel was narrow when last drawn.
    narrow: bool,
    online: super::inventory_online::OnlinePlace,
    manage: super::inventory_manage::Manage,
    /// A CPU script's run controls, for the script selected.
    run: super::script_run::ScriptRun,
    curated: CuratedScripts,
}

impl Default for InventoryPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// A row as the list draws it: its entry and, online, its item.
struct Drawn {
    entry: Entry,
    item: Option<crate::inventory::online::Item>,
}

impl InventoryPanel {
    pub fn new() -> Self {
        InventoryPanel {
            entries: Arc::new(Vec::new()),
            revision: None,
            family: Family::Fractals,
            filters: Filters::default(),
            views: HashMap::new(),
            query: String::new(),
            selected: None,
            hovered: None,
            updates_only: false,
            details: false,
            narrow: false,
            online: super::inventory_online::OnlinePlace::new(),
            manage: super::inventory_manage::Manage::default(),
            run: super::script_run::ScriptRun::default(),
            curated: CuratedScripts::default(),
        }
    }

    /// Show a catalog (the Colors panel's and the Palette Editor's palette
    /// library buttons, File > From Preset Library). Its own Category and
    /// Type start at All.
    pub fn open_on(&mut self, family: Family) {
        if self.family != family {
            self.family = family;
            self.filters.category = None;
            self.filters.kind = None;
            self.hovered = None;
        }
    }

    /// The catalog's view.
    fn view(&self) -> View {
        self.views.get(&self.family).copied().unwrap_or_else(|| View::default_for(self.family))
    }

    fn view_mut(&mut self) -> &mut View {
        let family = self.family;
        self.views.entry(family).or_insert_with(|| View::default_for(family))
    }

    fn entry(&self, key: &EntryKey) -> Option<&Entry> {
        if matches!(key, EntryKey::Online(..)) {
            return self.online.find(key).map(|(e, _)| e);
        }
        self.entries.iter().find(|e| &e.key == key)
    }

    /// Show `family` (when given) at `source`, with `select` selected.
    pub fn open_at(&mut self, family: Option<Family>, source: Source, select: Option<EntryKey>) {
        if let Some(family) = family {
            self.open_on(family);
        }
        self.filters.source = source;
        if select.is_some() {
            self.selected = select;
        }
    }

    /// Your own online lists changed (a fractal saved online): ask again.
    pub fn forget_online(&mut self) {
        self.online.forget();
    }

    /// Refresh, everything at once: the online lists and the curated
    /// scripts asked for again, the files on disk read again, and (by the
    /// app) the variation and effect catalogs fetched again.
    fn refresh_all(&mut self, out: &mut InventoryResponse) {
        self.online.forget();
        if self.curated.job.is_none() {
            self.curated.asked = false;
        }
        self.revision = None;
        out.refresh_catalogs = true;
    }

    /// The session signed out: your own online lists go.
    pub fn signed_out(&mut self) {
        self.online.signed_out();
    }

    /// Read the sources again when what they hold has changed, and share
    /// the list with the pickers. Once a frame, before any panel draws.
    pub fn refresh(&mut self, library: &PaletteLibrary, files: u64, online: inventory::Online) {
        // The curated online scripts the pickers offer (L3): asked for once
        // a session, in the background; until they come, none.
        self.curated.poll();
        let scripts = self.curated.items.clone();
        let online = inventory::Online { scripts: &scripts, ..online };
        let rev = inventory::revision(library, files, online);
        if self.revision != Some(rev) {
            self.entries = Arc::new(inventory::scan(library, online));
            self.revision = Some(rev);
            // What was asked for and is now in, current, is asked for no
            // more (a later update may ask again).
            if let Ok(mut d) = DOWNLOADS.lock() {
                let entries = &self.entries;
                d.1.retain(|(k, n)| {
                    !entries.iter().any(|e| e.key == EntryKey::Registered(*k, n.clone()) && e.installed && e.update.is_none())
                });
            }
            if let Ok(mut s) = SHARED.write() {
                *s = Some(self.entries.clone());
            }
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext) -> InventoryResponse {
        let mut out = InventoryResponse::default();
        self.online.update(ui.ctx(), &self.query, &self.entries, &mut out);
        if let Some(palette) = out.apply_palette.take() {
            apply_palette(cx.config_manager, palette);
        }
        self.narrow = ui.available_width() < NARROW;
        self.render_top(ui, cx, &mut out);
        ui.separator();
        let mut hovered = None;
        if !self.narrow {
            self.details = false;
            let shown = self.hovered.clone().or_else(|| self.selected.clone());
            egui::Panel::right("catalog_info").resizable(true).default_size(280.0).min_size(200.0).show_inside(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("catalog_info_scroll").show(ui, |ui| self.render_info(ui, shown, cx, &mut out));
            });
            egui::CentralPanel::default().show_inside(ui, |ui| self.render_list(ui, cx, &mut out, &mut hovered));
        } else if self.details && self.selected.is_some() {
            // Narrow (a phone, a narrow dock): the info is a screen of its
            // own, Back to the list.
            if ui.button(t!("catalog.back")).clicked() {
                self.details = false;
            }
            ui.separator();
            let shown = self.selected.clone();
            egui::ScrollArea::vertical().id_salt("catalog_info_scroll").show(ui, |ui| self.render_info(ui, shown, cx, &mut out));
        } else {
            self.details = false;
            self.render_list(ui, cx, &mut out, &mut hovered);
        }
        // The pane follows the pointer: a change is drawn at once.
        if hovered != self.hovered {
            self.hovered = hovered;
            ui.ctx().request_repaint();
        }
        self.online.render_confirm_delete(ui.ctx(), &cx.account);
        out
    }

    // ------------------------------------------------------------------ top

    /// The catalog and Refresh; the filters; the view, search, size and
    /// grouping; then *Open by ID or link* (online), the downloads bar and
    /// the file actions, where they apply.
    fn render_top(&mut self, ui: &mut egui::Ui, cx: &InventoryContext, out: &mut InventoryResponse) {
        let family = self.family;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("catalog_family").selected_text(t!(family.label_key())).show_ui(ui, |ui| {
                for f in Family::ALL {
                    if ui.selectable_label(f == family, t!(f.label_key())).clicked() {
                        self.open_on(f);
                    }
                }
            });
            // Refresh in the corner: everything at once (`refresh_all`).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("\u{27f3}").on_hover_text(t!("catalog.refresh_tip")).clicked() {
                    self.refresh_all(out);
                }
                // Narrow: no pane, no hover; the selection's info is a
                // screen of its own (D7).
                if self.narrow && self.selected.is_some() && ui.button(t!("catalog.details")).clicked() {
                    self.details = true;
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            let categories = categories(family);
            if !categories.is_empty() {
                ui.label(t!("catalog.filter_category"));
                let current = self.filters.category.and_then(|c| categories.iter().find(|g| g.order == c)).map_or_else(|| t!("catalog.filter_all").to_string(), |g| g.label.clone());
                egui::ComboBox::from_id_salt(("catalog_category", family)).selected_text(current).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filters.category, None, t!("catalog.filter_all"));
                    for g in &categories {
                        ui.selectable_value(&mut self.filters.category, Some(g.order), &g.label);
                    }
                });
            }
            ui.label(t!("catalog.filter_source"));
            egui::ComboBox::from_id_salt("catalog_source").selected_text(t!(self.filters.source.label_key())).show_ui(ui, |ui| {
                for s in Source::ALL {
                    ui.selectable_value(&mut self.filters.source, s, t!(s.label_key()));
                }
            });
            ui.label(t!("catalog.filter_owner"));
            egui::ComboBox::from_id_salt("catalog_owner").selected_text(t!(self.filters.owner.label_key())).show_ui(ui, |ui| {
                for o in Owner::ALL {
                    ui.selectable_value(&mut self.filters.owner, o, t!(o.label_key()));
                }
            });
            let types = types(family);
            if !types.is_empty() {
                ui.label(t!("catalog.filter_type"));
                let current = self.filters.kind.map_or_else(|| t!("catalog.filter_all").to_string(), |k| t!(k.label_key()).to_string());
                egui::ComboBox::from_id_salt(("catalog_type", family)).selected_text(current).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filters.kind, None, t!("catalog.filter_all"));
                    for k in types {
                        ui.selectable_value(&mut self.filters.kind, Some(k), t!(k.label_key()));
                    }
                });
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut view = self.view();
            if has_grid(family) {
                ui.selectable_value(&mut view.grid, true, t!("catalog.view_grid")).on_hover_text(t!("catalog.view_grid_tip"));
                ui.selectable_value(&mut view.grid, false, t!("catalog.view_list"));
            }
            let r = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text(t!("catalog.search")).desired_width(160.0));
            super::vkb_sync(ui, &r, &mut self.query);
            if !self.query.is_empty() && ui.small_button("×").on_hover_text(t!("catalog.clear_search")).clicked() {
                self.query.clear();
            }
            if view.grid && has_grid(family) {
                ui.label(t!("catalog.size"));
                ui.add(super::VkbSlider::new(&mut view.size, super::inventory_view::SIZES).show_value(false));
            }
            ui.label(t!("catalog.group_by"));
            egui::ComboBox::from_id_salt(("catalog_group_by", family)).selected_text(t!(view.group.label_key())).show_ui(ui, |ui| {
                for g in GroupBy::offered(family) {
                    ui.selectable_value(&mut view.group, *g, t!(g.label_key()));
                }
            });
            if view != self.view() {
                *self.view_mut() = view;
            }
        });
        if self.filters.source != Source::Device {
            self.online.render_open(ui, &cx.account);
        }
        if matches!(family, Family::Variations | Family::Effects) {
            self.render_downloads_bar(ui, out);
        }
        if let Some(super::inventory_manage::Act::Select(key)) = self.manage.render_top(ui, family, out) {
            self.selected = Some(key);
        }
    }

    /// Variations and effects on this device: the downloads the server
    /// has a newer version of (a chip, and Update all), and Clear Cache
    /// (the Variations and Effects panels' catalog sections, moved here).
    fn render_downloads_bar(&mut self, ui: &mut egui::Ui, out: &mut InventoryResponse) {
        let family = self.family;
        let stale: Vec<Entry> = self.entries.iter().filter(|e| e.kind.family() == family && e.update.is_some()).cloned().collect();
        let downloads = self.entries.iter().any(|e| e.kind.family() == family && e.origin == Origin::Api && e.installed);
        if stale.is_empty() {
            self.updates_only = false;
        }
        if stale.is_empty() && !downloads {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if !stale.is_empty() {
                let label = t!("catalog.updates", count = stale.len());
                if ui.selectable_label(self.updates_only, label).on_hover_text(t!("catalog.updates_tip")).clicked() {
                    self.updates_only = !self.updates_only;
                }
                if ui.button(t!("catalog.update_all")).clicked() {
                    for e in &stale {
                        request_download(e);
                    }
                }
            }
            if downloads && ui.button(t!("catalog.clear_cache")).on_hover_text(t!("catalog.clear_cache_tip")).clicked() {
                match family {
                    Family::Variations => out.clear_variation_cache = true,
                    _ => out.clear_effect_cache = true,
                }
            }
        });
    }

    // ----------------------------------------------------------------- list

    /// The one list: the device's entries and the server's pages, as the
    /// filters admit them, grouped, as cells or rows. Hovering a row shows
    /// it in the pane; a click selects it and loads or applies it
    /// (a variation or an effect only selects: its pane adds it, D3).
    fn render_list(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext, out: &mut InventoryResponse, hovered: &mut Option<EntryKey>) {
        let family = self.family;
        let view = self.view();
        let ctx = ui.ctx().clone();
        let lists = super::inventory_view::online_lists(family, &self.filters, cx.account.signed_in);
        let keys = self.online.keys(&lists);
        self.online.start(&keys, &cx.account, &ctx);
        // The online rows, owned for the frame (the thumbnails are asked
        // of the same lists while the rows are drawn).
        let online: Vec<(Entry, crate::inventory::online::Item, crate::api::Scope)> =
            self.online.rows(&keys).into_iter().map(|r| (r.entry.clone(), r.item.clone(), r.scope)).collect();
        let online_rows: Vec<super::inventory_view::OnlineRow> = online.iter().map(|(entry, item, scope)| super::inventory_view::OnlineRow { entry, item, scope: *scope }).collect();
        let entries = self.entries.clone();
        let mut rows = super::inventory_view::rows(family, &entries, &online_rows, &self.filters, &self.query);
        if self.updates_only {
            rows.retain(|r| r.entry.update.is_some());
        }
        let mut clicked: Option<Drawn> = None;
        let mut end_visible = false;
        egui::ScrollArea::vertical().id_salt("catalog_list").auto_shrink([false, false]).show(ui, |ui| {
            if family == Family::Palettes && view.group == GroupBy::Pack {
                end_visible = self.render_palettes(ui, cx, &rows, &mut clicked, hovered);
                return;
            }
            if rows.is_empty() {
                return;
            }
            let groups = super::inventory_view::group(family, rows.clone(), view.group);
            // A script's preview runs it with the library's palettes; no
            // other kind reads them.
            let palettes: Vec<crate::scene::palette::Palette> = if family == Family::Scripts { cx.palette_library.iter().cloned().collect() } else { Vec::new() };
            let last = groups.len() - 1;
            for (gi, (g, members)) in groups.iter().enumerate() {
                let mut body = |ui: &mut egui::Ui| {
                    let n = members.len();
                    let mut draw = |ui: &mut egui::Ui, i: usize, r: &super::inventory_view::Row| {
                        let e = r.entry;
                        let selected = self.selected.as_ref() == Some(&e.key);
                        let label = online_label(e);
                        let resp = if family == Family::Palettes {
                            palette_entry_row(ui, e, r.item, &label, selected, cx.palette_library)
                        } else if view.grid {
                            let online = &mut self.online;
                            let account = &cx.account;
                            super::inventory_online::grid_cell_sized(ui, e, &label, selected, true, view.size, || picture(online, e, r.item, &ctx, account, &palettes))
                        } else {
                            text_row(ui, &label, selected)
                        };
                        if resp.hovered() {
                            *hovered = Some(e.key.clone());
                        }
                        if resp.clicked() {
                            clicked = Some(Drawn { entry: e.clone(), item: r.item.cloned() });
                        }
                        if gi == last && i + 1 == n {
                            end_visible = ui.is_rect_visible(resp.rect);
                        }
                    };
                    if view.grid && family != Family::Palettes {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                            for (i, r) in members.iter().enumerate() {
                                draw(ui, i, r);
                            }
                        });
                    } else {
                        for (i, r) in members.iter().enumerate() {
                            draw(ui, i, r);
                        }
                    }
                };
                if g.label.is_empty() {
                    body(ui);
                    continue;
                }
                let id = ui.make_persistent_id(("catalog_group", family, view.group as u8, g.order, g.label.as_str()));
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                    .show_header(ui, |ui| {
                        ui.strong(format!("{} ({})", g.label, members.len()));
                    })
                    .body(body);
            }
        });
        // Where the server's lists stand, under the list.
        let state = self.online.state(&keys);
        if state.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(t!("catalog.loading"));
            });
        } else if let Some(err) = &state.error {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, err);
                if ui.button(t!("catalog.retry")).clicked() {
                    self.online.retry(&keys, &cx.account, &ctx);
                }
            });
        } else if rows.is_empty() {
            ui.weak(t!("catalog.nothing_matches"));
        } else if (end_visible || rows.is_empty()) && state.more {
            self.online.more(&keys, &cx.account, &ctx);
        }
        if let Some(d) = clicked {
            self.click(&d, &ctx, cx, out);
        }
    }

    /// A click: select the entry, and load or apply it where a click does
    /// (D3: not a variation or an effect, which the pane adds; not a
    /// script, which the pane runs or opens; not what is not installed).
    fn click(&mut self, d: &Drawn, ctx: &egui::Context, cx: &mut InventoryContext, out: &mut InventoryResponse) {
        use crate::inventory::online::Item;
        let e = &d.entry;
        self.selected = Some(e.key.clone());
        if !e.installed {
            return;
        }
        let id = e.online.as_ref().map(|o| o.id.clone()).unwrap_or_default();
        match &d.item {
            Some(Item::Flame(_)) => self.online.load_flame(&id, &cx.account, ctx),
            Some(Item::Palette(p)) => apply_palette(cx.config_manager, crate::api::sync::palette_from_item(p)),
            Some(Item::Animation(_)) => out.load_api_animation = Some(id),
            Some(Item::Script(_)) => {}
            None => {
                if applies_on_click(e) && usable(e, cx.config_manager.active_config()).is_ok() {
                    apply(e, ctx, cx, out);
                }
            }
        }
    }

    /// Palettes grouped by pack (D4): each pack's checkbox and load state
    /// (`palette_library::pack_section`), its palettes the filters admit;
    /// online palettes the device does not hold in a section after.
    /// Gives whether the last online row is on screen.
    fn render_palettes(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext, rows: &[super::inventory_view::Row], clicked: &mut Option<Drawn>, hovered: &mut Option<EntryKey>) -> bool {
        let filtering = !self.query.trim().is_empty() || self.filters != Filters::default();
        let admitted: std::collections::HashSet<&EntryKey> = rows.iter().filter(|r| r.item.is_none()).map(|r| &r.entry.key).collect();
        for pack in 0..cx.palette_library.pack_count() {
            // This pack's entries, in its order (the scan keeps it), with
            // their index in the pack, those the filters admit.
            let members: Vec<(usize, Entry)> = self
                .entries
                .iter()
                .filter(|e| e.kind == Kind::Palette && e.group.order == pack as u32)
                .enumerate()
                .filter(|(_, e)| admitted.contains(&e.key))
                .map(|(j, e)| (j, e.clone()))
                .collect();
            if filtering && members.is_empty() {
                continue;
            }
            let selected = self.selected.clone();
            super::palette_library::pack_section(ui, cx.palette_library, pack, filtering, |ui, lib| {
                let Some(p) = lib.get_pack(pack) else { return };
                for (j, e) in &members {
                    let (Some(palette), EntryKey::Palette(hash)) = (p.palettes.get(*j), &e.key) else { continue };
                    let is_selected = selected.as_ref() == Some(&e.key);
                    let row = palette_row(ui, hash, &row_label(e), palette, is_selected);
                    if row.hovered() {
                        *hovered = Some(e.key.clone());
                    }
                    if row.clicked() {
                        *clicked = Some(Drawn { entry: e.clone(), item: None });
                    }
                }
            });
        }
        if cx.palette_library.pack_count() == 0 {
            ui.label(t!("palette_library.no_packs"));
            ui.label(t!("palette_library.no_packs_hint"));
        }
        let online: Vec<&super::inventory_view::Row> = rows.iter().filter(|r| r.item.is_some()).collect();
        let mut end_visible = false;
        if !online.is_empty() {
            ui.separator();
            ui.strong(t!("catalog.source_online"));
            for (i, r) in online.iter().enumerate() {
                let selected = self.selected.as_ref() == Some(&r.entry.key);
                let resp = palette_entry_row(ui, r.entry, r.item, &online_label(r.entry), selected, cx.palette_library);
                if resp.hovered() {
                    *hovered = Some(r.entry.key.clone());
                }
                if resp.clicked() {
                    *clicked = Some(Drawn { entry: r.entry.clone(), item: r.item.cloned() });
                }
                if i + 1 == online.len() {
                    end_visible = ui.is_rect_visible(resp.rect);
                }
            }
        }
        end_visible
    }

    /// Desktop: make one preview a frame of what the lists asked for.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one_preview<F>(&mut self, ctx: &egui::Context, render_fn: F) -> bool
    where
        F: Fn(&FractalConfig) -> image::RgbaImage,
    {
        super::inventory_preview::with(|p| p.generate_one(ctx, render_fn))
    }

    /// WASM: start the next preview the lists asked for.
    #[cfg(target_arch = "wasm32")]
    pub fn start_next_preview(&mut self, device: &egui_wgpu::wgpu::Device, queue: &egui_wgpu::wgpu::Queue, ctx: &egui::Context) {
        super::inventory_preview::with(|p| p.start_next(device, queue, ctx));
    }

    // ----------------------------------------------------------------- info

    /// What `key` is: the entry under the pointer, else the selection.
    fn render_info(&mut self, ui: &mut egui::Ui, key: Option<EntryKey>, cx: &mut InventoryContext, out: &mut InventoryResponse) {
        let Some(key) = key else {
            ui.weak(t!("catalog.hover_hint"));
            return;
        };
        if let EntryKey::Online(..) = key {
            match self.online.render_info(ui, &key, cx, out) {
                Some(super::inventory_online::InfoAction::ShowLocal(local)) => {
                    if let Some(e) = self.entries.iter().find(|e| e.key == local) {
                        let family = e.kind.family();
                        self.open_on(family);
                    }
                    self.filters.source = Source::All;
                    self.selected = Some(local);
                }
                None if self.online.find(&key).is_none() => {
                    ui.weak(t!("catalog.hover_hint"));
                }
                None => {}
            }
            return;
        }
        let Some(e) = self.entry(&key).cloned() else {
            ui.weak(t!("catalog.hover_hint"));
            return;
        };
        let palettes: Vec<crate::scene::palette::Palette> = if e.kind.family() == Family::Scripts { cx.palette_library.iter().cloned().collect() } else { Vec::new() };
        let preview = super::inventory_preview::want(&e, ui.ctx(), &palettes);
        entry_summary(ui, &e, Some(cx.palette_library), Some(&preview));
        // A preset: what the Fractals list's expanded rows said.
        if let EntryKey::Preset(name) = &e.key {
            if let Some(config) = inventory::preset(name) {
                ui.label(super::fractal_gallery::list_summary(&config));
                for line in super::fractal_gallery::list_details(&config) {
                    ui.weak(line);
                }
            }
        }
        if let Some(v) = &e.update {
            ui.label(egui::RichText::new(t!("catalog.update_available", version = v.clone())).strong());
        }
        if let Some(why) = download_failure(&e) {
            ui.colored_label(ui.visuals().error_fg_color, t!("catalog.download_failed", error = why));
        }
        ui.separator();
        // A CPU script: running it is its Use (decision 2 of the survey).
        let script = matches!(&e.key, EntryKey::File { store: Store::Scripts, .. }) && matches!(e.kind, Kind::Generator | Kind::Modifier | Kind::ScriptLibrary);
        if let (true, EntryKey::File { file, .. }) = (script, &e.key) {
            let id = file.strip_suffix(".rhai").unwrap_or(file).to_string();
            let current = cx.config_manager.active_config().clone();
            let palettes: Vec<crate::scene::palette::Palette> = cx.palette_library.iter().cloned().collect();
            let generation = super::script_run::files_generation(cx.script_cloud);
            out.run = self.run.render_stored(ui, &id, &current, &palettes, generation);
            ui.separator();
        }
        ui.horizontal_wrapped(|ui| {
            if !e.installed {
                download_button(ui, &e);
                return;
            }
            if script {
                return;
            }
            if e.update.is_some() {
                if download_requested(&e) {
                    ui.add_enabled(false, egui::Button::new(t!("catalog.updating")));
                } else if ui.button(t!("catalog.update")).clicked() {
                    request_download(&e);
                }
            }
            // A variation: the transform it is added to, the Triangle
            // Editor's selection (one list, Normal, Linked and Final).
            if e.kind == Kind::Variation {
                let flame = cx.config_manager.active_config().flame.clone();
                super::transforms::transform_selector(ui, "catalog_transform", &flame);
            }
            match usable(&e, cx.config_manager.active_config()) {
                Ok(()) => {
                    if ui.button(t!(use_label_key(&e))).clicked() {
                        self.selected = Some(e.key.clone());
                        apply(&e, ui.ctx(), cx, out);
                    }
                }
                Err(reason) => {
                    ui.add_enabled(false, egui::Button::new(t!(use_label_key(&e)))).on_disabled_hover_text(reason);
                }
            }
            match &e.key {
                EntryKey::Palette(_) => {
                    if ui.button(t!("catalog.open_palette_editor")).clicked() {
                        out.open_palette_editor = true;
                    }
                }
                // A shipped file, to read: yours are edited below.
                EntryKey::File { store, file, shipped: true } => {
                    if ui.button(t!("catalog.open_in_editor")).clicked() {
                        super::inventory_manage::edit(*store, file, true, out);
                    }
                }
                _ => {}
            }
        });
        // Your own: edit, rename, duplicate, delete, publishing; a shipped
        // file or a built-in: a copy of yours.
        if let Some(super::inventory_manage::Act::Select(key)) = self.manage.render_actions(ui, &e, cx, out) {
            self.selected = Some(key);
        }
    }
}

/// Whether a click on a device entry loads or applies it: what replaces
/// something (a fractal, a palette, a formula, a model, a colouring), not
/// what adds (a variation, an effect: D3) nor a script or a file to open.
fn applies_on_click(e: &Entry) -> bool {
    !matches!(e.kind, Kind::Variation | Kind::Effect | Kind::Generator | Kind::Modifier | Kind::ScriptLibrary | Kind::Track | Kind::FormulaFile | Kind::Unreadable)
        && !matches!(e.key, EntryKey::File { store: Store::Scripts, .. })
}

/// A row's label: its marks, and an online row's owner.
fn online_label(e: &Entry) -> String {
    let label = row_label(e);
    match e.online.as_ref().map(|o| o.owner.as_str()).filter(|o| !o.is_empty()) {
        Some(owner) => format!("{label}  · {owner}"),
        None => label,
    }
}

/// A cell's picture: an online fractal's thumbnail, else the entry's
/// preview; and whether one is coming.
fn picture(
    online: &mut super::inventory_online::OnlinePlace,
    e: &Entry,
    item: Option<&crate::inventory::online::Item>,
    ctx: &egui::Context,
    account: &CloudAccount,
    palettes: &[crate::scene::palette::Palette],
) -> (Option<egui::TextureHandle>, bool) {
    if let Some(url) = e.online.as_ref().and_then(|o| o.thumbnail_url.as_deref()) {
        return (online.thumbnail(ctx, url, account), true);
    }
    if item.is_some() || !super::inventory_preview::has_previews(e.kind.family()) {
        return (None, false);
    }
    match super::inventory_preview::want(e, ctx, palettes) {
        super::inventory_preview::Shown::Ready(t) => (t.last().cloned(), true),
        super::inventory_preview::Shown::Pending => (None, true),
        super::inventory_preview::Shown::None => (None, false),
    }
}

/// A list row: its label across the list's width, highlighted when
/// selected or under the pointer.
fn text_row(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let h = ui.spacing().interact_size.y + 2.0;
    let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        if selected {
            ui.painter().rect_filled(rect, 2.0, visuals.selection.bg_fill);
        } else if r.hovered() {
            ui.painter().rect_filled(rect, 2.0, visuals.widgets.hovered.bg_fill);
        }
        let colour = if selected { visuals.selection.stroke.color } else { visuals.text_color() };
        let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::default(), colour);
        ui.painter().with_clip_rect(rect).galley(rect.left_center() + egui::vec2(4.0, -galley.size().y / 2.0), galley, colour);
    }
    r
}

/// A palette's row in the list: the device's (its gradient looked up in
/// the library only until made), or an online one's (its item's colours).
fn palette_entry_row(ui: &mut egui::Ui, e: &Entry, item: Option<&crate::inventory::online::Item>, label: &str, selected: bool, library: &PaletteLibrary) -> egui::Response {
    match (item, &e.key) {
        (Some(crate::inventory::online::Item::Palette(p)), _) => palette_row_lazy(ui, &p.content_hash, label, selected, || Some(crate::api::sync::palette_from_item(p))),
        (_, EntryKey::Palette(hash)) => palette_row_lazy(ui, hash, label, selected, || inventory::palette(library, hash)),
        _ => text_row(ui, label, selected),
    }
}

/// A palette row: its name, and its gradient at the right; the texture is
/// made only once the row is on screen.
pub(super) fn palette_row(ui: &mut egui::Ui, hash: &str, title: &str, palette: &crate::scene::palette::Palette, selected: bool) -> egui::Response {
    palette_row_lazy(ui, hash, title, selected, || Some(palette.clone()))
}

/// [`palette_row`], the palette found only when its gradient is not made
/// yet (a picker's rows hold an entry, not the palette).
pub(super) fn palette_row_lazy(ui: &mut egui::Ui, hash: &str, title: &str, selected: bool, palette: impl FnOnce() -> Option<crate::scene::palette::Palette>) -> egui::Response {
    let h = 20.0;
    let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h + 4.0), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return r;
    }
    let visuals = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, 2.0, visuals.selection.bg_fill);
    } else if r.hovered() {
        ui.painter().rect_filled(rect, 2.0, visuals.widgets.hovered.bg_fill);
    }
    let preview_w = (rect.width() * 0.5).clamp(60.0, 200.0);
    let name_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x - preview_w - 8.0, rect.max.y));
    let text_color = if selected { visuals.selection.stroke.color } else { visuals.text_color() };
    let galley = ui.painter().layout_no_wrap(title.to_string(), egui::FontId::default(), text_color);
    ui.painter().with_clip_rect(name_rect).galley(name_rect.left_center() + egui::vec2(4.0, -galley.size().y / 2.0), galley, text_color);
    let img = egui::Rect::from_min_size(egui::pos2(rect.max.x - preview_w, rect.min.y + 2.0), egui::vec2(preview_w, h));
    let size = [200, 20];
    let tex = super::palette_library::palette_preview_made(ui.ctx(), hash, size).or_else(|| palette().map(|p| super::palette_library::palette_preview(ui.ctx(), hash, &p, size)));
    if let Some(tex) = tex {
        egui::Image::new(&tex).fit_to_exact_size(img.size()).paint_at(ui, img);
    }
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r
}

/// What an entry is, as the info pane and the picker's show it: its title,
/// kind and group, where it came from, version, authors, a palette's
/// gradient (given the library) and its description.
pub(crate) fn entry_summary(ui: &mut egui::Ui, e: &Entry, palettes: Option<&PaletteLibrary>, preview: Option<&super::inventory_preview::Shown>) {
    ui.heading(&e.title);
    ui.label(egui::RichText::new(format!("{} · {}", t!(e.kind.label_key()), e.group.label)).weak());
    ui.label(origin_text(e));
    if let Some(v) = &e.version {
        ui.label(egui::RichText::new(t!("catalog.version", version = v)).weak());
    }
    if !e.authors.is_empty() {
        ui.label(t!("catalog.by", names = e.authors.join(", ")));
    }
    ui.separator();
    if let (EntryKey::Palette(hash), Some(lib)) = (&e.key, palettes) {
        if let Some(p) = inventory::palette(lib, hash) {
            let w = ui.available_width().clamp(120.0, 360.0);
            let tex = super::palette_library::palette_preview(ui.ctx(), hash, &p, [360, 24]);
            ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(w, 24.0)));
            ui.add_space(4.0);
        }
    }
    if let Some(p) = preview {
        super::inventory_preview::show(ui, p);
        ui.add_space(4.0);
    }
    if e.description.is_empty() {
        ui.weak(t!("catalog.no_description"));
    } else {
        super::markdown::show(ui, &e.description);
    }
}

/// A row's text: the title, the origin's mark, a plug-in's; a download
/// arrow for what is not installed.
pub(crate) fn row_label(e: &Entry) -> String {
    let available = if !e.installed {
        "  \u{2193}"
    } else if e.update.is_some() {
        "  \u{2191}"
    } else {
        ""
    };
    format!("{}{}{available}", e.title, crate::storage::library::marks(e.origin))
}

/// Download, for an entry the app lacks: greyed once asked for.
pub(crate) fn download_button(ui: &mut egui::Ui, e: &Entry) {
    if download_requested(e) {
        ui.add_enabled(false, egui::Button::new(t!("catalog.downloading")));
    } else if ui.button(t!("catalog.download")).on_hover_text(t!("catalog.download_tip")).clicked() {
        request_download(e);
    }
}

/// Where an entry came from, as a sentence.
fn origin_text(e: &Entry) -> String {
    let base = match (e.origin, &e.key) {
        (Origin::Builtin, EntryKey::File { .. }) => t!("catalog.from_shipped_file"),
        (Origin::Builtin, _) => t!("catalog.from_builtin"),
        (Origin::Local, _) => t!("catalog.from_yours"),
        (Origin::Api, EntryKey::File { .. }) => match e.link.as_ref().and_then(|l| l.owner.clone()) {
            Some(owner) => t!("catalog.from_adopted_by", owner = owner),
            None => t!("catalog.from_adopted"),
        },
        (Origin::Api, _) => t!("catalog.from_download"),
    };
    base.to_string()
}

/// The Use button's label for an entry.
fn use_label_key(e: &Entry) -> &'static str {
    match e.kind {
        Kind::Palette => "catalog.use_palette",
        Kind::Fractal => "catalog.use_fractal",
        Kind::Variation => "catalog.use_variation",
        Kind::Effect => "catalog.use_effect",
        Kind::Generator | Kind::Modifier | Kind::ScriptLibrary => "catalog.use_script",
        _ => "catalog.use",
    }
}

/// Files a list asked for that are not compiled yet: compiled before the
/// next frame's panels draw ([`compile_requested`]), never inside a draw.
/// A panel may hold a registry's read guard while it draws a picker (the
/// Transforms panel does), and registering what a file makes takes that
/// registry's write lock: compiling inside the draw hung the app.
static TO_COMPILE: Mutex<Vec<(String, bool, String)>> = Mutex::new(Vec::new());

/// The file, shipped and version of a Rhai file of the formula store of a
/// kind a list can use; `None` for anything else: a built-in, a CPU
/// script, a UF or Fractint file.
fn list_file(e: &Entry) -> Option<(String, bool, String)> {
    let EntryKey::File { store: Store::Formulas, file, shipped } = &e.key else { return None };
    if !crate::storage::formula_store::is_rhai(file) {
        return None;
    }
    let lib_kind = crate::storage::library::Kind::from_wire(crate::inventory::online::script_wire(e.kind)?)?;
    if !lib_kind.can_be_plugin() {
        return None;
    }
    Some((file.clone(), *shipped, e.version.clone().unwrap_or_default()))
}

/// A Rhai file of the formula store compiled, for the lists and Use
/// (docs/projects/one-list.md, L1): a shipped example or a file of the
/// user's, of a kind a list can use, compiled once per version
/// (`library::compile_file`). Not compiled yet, it is asked for (compiled
/// before the next frame: [`TO_COMPILE`]) and says so, `Err`
/// ([`compile_pending`] tells that apart). `None` for anything else.
pub(crate) fn compiled(e: &Entry) -> Option<Result<crate::storage::library::Plugin, String>> {
    let (file, shipped, version) = list_file(e)?;
    if let Some(done) = crate::storage::library::compiled_cached(&file, shipped, &version) {
        return Some(done);
    }
    if let Ok(mut asked) = TO_COMPILE.lock() {
        let item = (file, shipped, version);
        if !asked.contains(&item) {
            asked.push(item);
        }
    }
    Some(Err(t!("catalog.compiling").to_string()))
}

/// Whether the entry is a file a list uses that is not compiled yet (and
/// now asked for).
pub(crate) fn compile_pending(e: &Entry) -> bool {
    list_file(e).is_some_and(|(file, shipped, version)| crate::storage::library::compiled_cached(&file, shipped, &version).is_none()) && compiled(e).is_some()
}

/// Compile the files the lists asked for since the last call; whether
/// there were any (the caller repaints, so the lists show them). Called
/// once a frame before any panel draws, holding no registry's lock.
pub fn compile_requested() -> bool {
    let asked = TO_COMPILE.lock().map(|mut a| std::mem::take(&mut *a)).unwrap_or_default();
    for (file, shipped, version) in &asked {
        let _ = crate::storage::library::compile_file(file, *shipped, version, || match shipped {
            true => crate::storage::formula_store::EXAMPLES.iter().find(|(n, _)| n == file).map(|(_, s)| s.to_string()),
            false => crate::storage::manage::load(Store::Formulas, file),
        });
    }
    !asked.is_empty()
}

/// A file's compiled record, when it compiles.
pub(crate) fn plugin_of(e: &Entry) -> Option<crate::storage::library::Plugin> {
    compiled(e)?.ok()
}

/// The registry name an entry applies by: its own, or its file's key.
pub(crate) fn applied_name(e: &Entry) -> Option<String> {
    match &e.key {
        EntryKey::Registered(_, n) => Some(n.clone()),
        EntryKey::File { .. } => plugin_of(e).map(|p| p.key),
        _ => None,
    }
}

/// Whether a picker may offer an entry (docs/projects/one-list.md): a
/// built-in or a download, installed, or a file that compiles; or, not
/// installed, what can be downloaded (L3: the picker shows those only when
/// the user's setting offers them). A file a list cannot use -- a UF or
/// Fractint file, one that does not compile -- is not offered (L2).
pub(crate) fn pickable(e: &Entry) -> bool {
    !e.installed || picked_now(e)
}

/// [`pickable`], installed: what can be picked without a download.
pub(crate) fn picked_now(e: &Entry) -> bool {
    e.installed
        && match &e.key {
            EntryKey::Registered(..) => true,
            // Not compiled yet: offered this frame, compiled before the next.
            EntryKey::File { .. } => plugin_of(e).is_some() || compile_pending(e),
            _ => false,
        }
}

/// Whether Use can apply the entry to this config, or why not.
pub(crate) fn usable(e: &Entry, config: &FractalConfig) -> Result<(), String> {
    if !e.installed {
        return Err(t!("catalog.not_installed").to_string());
    }
    let only_3d = e.kind == Kind::Variation
        && applied_name(e).is_some_and(|n| {
            crate::variations::global_registry().get(&n).is_some_and(|v| v.category == crate::variations::VariationCategory::Only3D)
        });
    if let super::visibility::Vis::Grey(reason) = super::visibility::catalog_use(e.kind, only_3d, config.render_mode) {
        return Err(t!(reason).to_string());
    }
    // A file is used as what it compiles to; one that does not compile,
    // or is not a kind a list uses (a UF file, a track), from the editor;
    // a CPU script from the Scripts panel.
    if let EntryKey::File { store, .. } = &e.key {
        if *store == Store::Formulas {
            match compiled(e) {
                Some(Ok(_)) => {}
                Some(Err(why)) => return Err(t!("catalog.does_not_compile", error = why).to_string()),
                None => return Err(t!("catalog.open_to_use").to_string()),
            }
        }
    }
    let esc = &config.escape;
    let name = applied_name(e).unwrap_or_default();
    match e.kind {
        Kind::Coloring if crate::escape::fields::get_field(&esc.formula).is_some() || crate::escape::ifs::get_ifs(&esc.formula).is_some() => {
            Err(t!("catalog.needs_escape_formula").to_string())
        }
        Kind::Coloring => {
            let formula = crate::escape::get_formula(&esc.formula);
            match crate::escape::find_coloring(&name) {
                Some(c) if !crate::escape::coloring_suits_formula(formula, c) => {
                    Err(t!("catalog.coloring_does_not_suit", formula = formula.display_name).to_string())
                }
                _ => Ok(()),
            }
        }
        Kind::FieldColoring if crate::escape::fields::get_field(&esc.formula).is_none() => Err(t!("catalog.needs_field").to_string()),
        Kind::SolidColoring => match crate::escape::ifs::get_ifs(&esc.formula) {
            None => Err(t!("catalog.needs_solid").to_string()),
            Some(d) => match crate::escape::ifs::IFS_COLORINGS.iter().find(|c| c.name == name) {
                Some(c) if !crate::escape::ifs::coloring_applies(d, c) => Err(t!("catalog.coloring_does_not_suit", formula = d.display_name).to_string()),
                _ => Ok(()),
            },
        },
        Kind::Variation if editing_flame_is_empty(config) => Err(t!("catalog.needs_transform").to_string()),
        Kind::Track | Kind::FormulaFile => Err(t!("catalog.open_to_use").to_string()),
        _ => Ok(()),
    }
}

fn editing_flame_is_empty(config: &FractalConfig) -> bool {
    let f = &config.flame;
    f.transforms.is_empty() && f.linked_transforms.is_empty() && f.final_transforms.is_empty()
}

pub(super) fn apply_palette(config_manager: &mut ConfigManager, palette: crate::scene::palette::Palette) {
    // An editable copy: editing it changes only the config's palette,
    // until it is saved to the library.
    let mut copy = palette;
    copy.built_in = false;
    let _ = config_manager.update_param(ConfigPath::Palette, copy.into());
}

/// Use: what each kind's own picker does when it is chosen there.
fn apply(e: &Entry, ctx: &egui::Context, cx: &mut InventoryContext, out: &mut InventoryResponse) {
    let cm = &mut *cx.config_manager;
    if let EntryKey::File { store: Store::Scripts, file, shipped } = &e.key {
        out.open_file = Some((Store::Scripts, file.clone(), *shipped));
        return;
    }
    match &e.key {
        EntryKey::Palette(hash) => {
            if let Some(p) = inventory::palette(cx.palette_library, hash) {
                apply_palette(cm, p);
            }
        }
        EntryKey::Preset(name) => out.load_preset = inventory::preset(name),
        _ => use_entry(e, ctx, cm),
    }
}

/// Use, for a registered resource or a plug-in file: what each kind's own
/// picker does with a choice (the Catalog's Use, and every picker's pick,
/// `ui::picker`).
pub(crate) fn use_entry(e: &Entry, ctx: &egui::Context, cm: &mut ConfigManager) {
    use_entry_with(e, plugin_of(e), ctx, cm);
}

/// [`use_entry`] with the entry's plug-in record given (a preview's, of a
/// file that is not a plug-in): it applies by the record's key.
pub(crate) fn use_entry_with(e: &Entry, plugin: Option<crate::storage::library::Plugin>, ctx: &egui::Context, cm: &mut ConfigManager) {
    let name = plugin.as_ref().map(|p| p.key.clone()).or_else(|| applied_name(e)).unwrap_or_default();
    match (&e.key, e.kind) {
        (_, Kind::Variation) => {
            let flame = cm.active_config().flame.clone();
            if let Some(xref) = super::transforms::editing_transform(ctx, &flame) {
                let _ = super::transforms::add_variation(cm, xref, &name);
            }
        }
        (_, Kind::Effect) => {
            super::effects_panel::add_effect(cm, &name);
        }
        (_, Kind::Formula | Kind::Field | Kind::Solid) if plugin.is_some() => {
            super::escape_panel::choose_plugin(cm, plugin.as_ref().expect("checked"), false);
        }
        (_, Kind::Coloring | Kind::FieldColoring) if plugin.is_some() => {
            super::escape_panel::choose_plugin(cm, plugin.as_ref().expect("checked"), true);
        }
        (_, Kind::Formula) => {
            if let Some(f) = crate::escape::FORMULAS.iter().find(|f| f.name == name) {
                super::escape_panel::choose_formula(cm, f);
            }
        }
        (_, Kind::Field) => {
            if let Some(f) = crate::escape::fields::get_field(&name) {
                super::escape_panel::choose_field(cm, f);
            }
        }
        (_, Kind::Solid) => {
            if let Some(d) = crate::escape::ifs::get_ifs(&name) {
                super::escape_panel::choose_solid(cm, d);
            }
        }
        (_, Kind::Coloring) => {
            let esc = cm.active_config().escape.clone();
            if let Some(c) = crate::escape::find_coloring(&name) {
                super::escape_panel::pick_coloring(cm, crate::escape::get_formula(&esc.formula), &esc, c);
            }
        }
        (_, Kind::FieldColoring | Kind::SolidColoring) => super::escape_panel::choose_named_coloring(cm, &name),
        (_, Kind::SimModel) => match &plugin {
            Some(p) => super::sim_panel::choose_first_layer_plugin_model(cm, p),
            None => {
                if let Some(m) = crate::sim::MODELS.iter().find(|m| m.name == name) {
                    super::sim_panel::choose_first_layer_model(cm, m);
                }
            }
        },
        (_, Kind::SimColoring) => super::sim_panel::choose_base_coloring(cm, &name, plugin.as_ref()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A list's filter never compiles: the Transforms panel draws its Add
    /// Variation picker holding the variation registry's read guard, and
    /// compiling a variation file there took the write lock -- the app
    /// hung. Asked with the guard held, a file not compiled yet is offered
    /// and queued, and the call returns.
    #[test]
    fn a_pickers_filter_never_compiles_under_a_registry_guard() {
        let entries = inventory::scan(&crate::scene::palette::global_palette_library().read().unwrap().clone(), inventory::Online::default());
        let files: Vec<&Entry> = entries.iter().filter(|e| e.kind == Kind::Variation && matches!(e.key, EntryKey::File { shipped: true, .. })).collect();
        assert!(!files.is_empty(), "the shipped variation examples");
        let guard = crate::variations::global_registry();
        for e in &files {
            assert!(pickable(e), "{}: offered, compiled or not yet", e.title);
            let _ = applied_name(e);
        }
        drop(guard);
    }

    /// A failed download is asked for no more and says why; asking again
    /// clears the failure. (Before, its row said *Downloading…* until
    /// the app restarted.)
    #[test]
    fn a_failed_download_can_be_asked_again() {
        let mut e = crate::ui::picker::row(Kind::Variation, "it_failing_download", "Failing", inventory::Group { order: 1000, label: String::new() });
        e.installed = false;
        request_download(&e);
        assert!(download_requested(&e));
        assert!(take_downloads().iter().any(|(_, n)| n == "it_failing_download"));
        download_failed(Kind::Variation, "it_failing_download", "HTTP 500");
        assert!(!download_requested(&e), "no longer coming");
        assert_eq!(download_failure(&e).as_deref(), Some("HTTP 500"));
        request_download(&e);
        assert!(download_requested(&e) && download_failure(&e).is_none(), "asked again");
        assert!(row_label(&e).ends_with("  \u{2193}"), "the download mark");
        e.installed = true;
        e.update = Some("v4".into());
        assert!(row_label(&e).ends_with("  \u{2191}"), "an update's mark");
    }

    /// Add to Transform adds to the transform the selector (the Triangle
    /// Editor's selection) names -- a final one too -- not the first
    /// (reported: it always added to the first transform).
    #[test]
    fn add_to_transform_adds_to_the_selected_transform() {
        use crate::config::TransformRef;
        let mut config = crate::scene::presets::global_preset_library().presets()[0].clone();
        config.render_mode = crate::scene::transforms::RenderMode::TwoD;
        config.flame.final_transforms = vec![crate::scene::transforms::Transform::new()];
        let mut cm = ConfigManager::new(config);
        let ctx = egui::Context::default();
        ctx.data_mut(|d| d.insert_persisted(egui::Id::new("triangle_editor_selected_transform"), TransformRef::Final(0)));
        let julia = crate::ui::picker::row(Kind::Variation, "julia", "Julia", inventory::Group { order: 0, label: String::new() });
        use_entry(&julia, &ctx, &mut cm);
        let flame = &cm.active_config().flame;
        assert!(flame.final_transforms[0].variations.contains_key("julia"), "added to Final 1");
        assert!(!flame.transforms[0].variations.contains_key("julia"), "not to the first");
        assert_eq!(super::super::transforms::transform_label(TransformRef::Final(0)), "Final 1");
    }

    /// Use's rules: a variation needs a flame mode (and 3D for a 3D-only
    /// one), an escape colouring an escape-time formula it can draw; a
    /// palette always applies.
    #[test]
    fn use_says_why_it_cannot_apply() {
        let entry = |kind: Kind, name: &str| Entry {
            kind,
            key: EntryKey::Registered(kind, name.into()),
            title: name.into(),
            description: String::new(),
            authors: Vec::new(),
            version: None,
            origin: Origin::Builtin,
            group: inventory::Group { order: 0, label: String::new() },
            link: None,
            installed: true,
            category: None,
            update: None,
            online: None,
        };
        // A flame with transforms to add to.
        let mut config = crate::scene::presets::global_preset_library().presets()[0].clone();
        config.render_mode = crate::scene::transforms::RenderMode::TwoD;
        assert!(!config.flame.transforms.is_empty());
        assert!(usable(&entry(Kind::Variation, "julia"), &config).is_ok());
        config.flame.transforms.clear();
        config.flame.linked_transforms.clear();
        config.flame.final_transforms.clear();
        assert!(usable(&entry(Kind::Variation, "julia"), &config).is_err(), "no transform to add to");
        config.render_mode = crate::scene::transforms::RenderMode::Escape;
        assert!(usable(&entry(Kind::Variation, "julia"), &config).is_err(), "a variation in Escape mode");
        assert!(usable(&entry(Kind::Formula, "mandelbrot"), &config).is_ok(), "a formula enters Escape");
        config.escape.formula = crate::escape::fields::FIELDS[0].name.to_string();
        assert!(usable(&entry(Kind::Coloring, "smooth"), &config).is_err(), "an escape colouring over a field");
        assert!(usable(&entry(Kind::FieldColoring, crate::escape::fields::FIELD_COLORINGS[0].name), &config).is_ok());
        assert!(usable(&entry(Kind::SolidColoring, crate::escape::ifs::IFS_COLORINGS[0].name), &config).is_err(), "no solid");
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod phone_tests {
    use super::*;

    /// Every piece of text a frame drew, and where.
    fn placed(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Pos2)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos)),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for s in shapes {
            walk(&s.shape, &mut out);
        }
        out
    }

    /// Narrow (a phone, D7): the list fills the panel; with a selection,
    /// Details in the top row opens its info as a screen of its own, Back
    /// returns to the list.
    #[test]
    fn a_narrow_catalog_is_two_screens() {
        let mut library = crate::scene::palette::global_palette_library().read().unwrap().clone();
        let mut cm = ConfigManager::new(FractalConfig::default());
        let cloud = crate::app::script_cloud::ScriptCloudState::default();
        let mut panel = InventoryPanel::new();
        // Its own list, not the shared one the pickers' tests fill.
        panel.entries = Arc::new(inventory::scan(&library, inventory::Online::default()));
        panel.revision = Some(inventory::revision(&library, 0, inventory::Online::default()));
        panel.open_on(Family::Variations);
        // No server in a test.
        panel.filters.source = Source::Device;
        let linear = panel.entries.iter().find(|e| e.key == EntryKey::Registered(Kind::Variation, "linear".into())).unwrap().key.clone();
        panel.selected = Some(linear);
        let ctx = egui::Context::default();
        let mut frame = |panel: &mut InventoryPanel, click: Option<egui::Pos2>| {
            let mut drawn = Vec::new();
            for step in 0..3 {
                let mut input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 800.0))),
                    ..Default::default()
                };
                if let (Some(p), 1) = (click, step) {
                    input.events.push(egui::Event::PointerMoved(p));
                    input.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
                    input.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
                }
                let out = ctx.run_ui(input, |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        let mut cx = InventoryContext { config_manager: &mut cm, palette_library: &mut library, account: CloudAccount::default(), script_cloud: &cloud };
                        let _ = panel.render(ui, &mut cx);
                    });
                });
                drawn = placed(&out.shapes);
            }
            drawn
        };
        let list = frame(&mut panel, None);
        assert!(panel.narrow, "400 points is narrow");
        let details = list.iter().find(|(t, _)| t == &t!("catalog.details").to_string()).map(|(_, p)| *p).expect("Details");
        assert!(!list.iter().any(|(t, _)| t == "\u{203a}"), "no › on a row");
        assert!(!list.iter().any(|(t, _)| t == &t!("catalog.back").to_string()), "the list alone");

        let info = frame(&mut panel, Some(details + egui::vec2(3.0, 6.0)));
        assert!(panel.details, "Details opens the info");
        let back = info.iter().find(|(t, _)| t == &t!("catalog.back").to_string()).map(|(_, p)| *p).expect("Back");
        assert!(info.iter().any(|(t, _)| t == "Linear"), "the info names it: {info:?}");

        frame(&mut panel, Some(back + egui::vec2(4.0, 6.0)));
        assert!(!panel.details, "Back returns to the list");
    }

    /// The grid (decision 1): a family with previews drawn as cells, each
    /// on screen asking for its preview, those off screen not.
    #[test]
    fn the_grid_asks_for_the_previews_on_screen() {
        let mut library = crate::scene::palette::global_palette_library().read().unwrap().clone();
        let mut cm = ConfigManager::new(FractalConfig::default());
        let cloud = crate::app::script_cloud::ScriptCloudState::default();
        let mut panel = InventoryPanel::new();
        panel.entries = Arc::new(inventory::scan(&library, inventory::Online::default()));
        panel.open_on(Family::Variations);
        // No server in a test.
        panel.filters.source = Source::Device;
        panel.views.insert(Family::Variations, View { grid: true, ..View::default_for(Family::Variations) });
        let ctx = egui::Context::default();
        let mut drawn = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))), ..Default::default() };
            let out = ctx.run_ui(input, |ui| {
                super::super::inventory_preview::begin_frame(ui.ctx());
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    let mut cx = InventoryContext { config_manager: &mut cm, palette_library: &mut library, account: CloudAccount::default(), script_cloud: &cloud };
                    let _ = panel.render(ui, &mut cx);
                });
            });
            drawn = placed(&out.shapes);
        }
        assert!(drawn.iter().any(|(t, _)| t == "Linear"), "a cell names its entry");
        // Asked for (made, cached or waiting): the cells on screen alone.
        let asked = super::super::inventory_preview::with(|p| p.asked());
        let variations = panel.entries.iter().filter(|e| e.kind == Kind::Variation).count();
        assert!(asked > 0 && asked < variations / 4, "the cells on screen ask, not all {variations}: {asked}");
    }

    /// The Fractals catalog (one-catalog-view.md): the presets as cells
    /// under the panel's one search (the presets' gallery and its own
    /// search are gone); the pointer on a cell shows it in the pane; a
    /// click loads it.
    #[test]
    fn the_fractals_catalog_is_one_list_that_hovers_and_loads() {
        let mut library = crate::scene::palette::global_palette_library().read().unwrap().clone();
        let mut cm = ConfigManager::new(FractalConfig::default());
        let cloud = crate::app::script_cloud::ScriptCloudState::default();
        let mut panel = InventoryPanel::new();
        panel.entries = Arc::new(inventory::scan(&library, inventory::Online::default()));
        panel.filters.source = Source::Device;
        let ctx = egui::Context::default();
        let mut frame = |panel: &mut InventoryPanel, pointer: Option<egui::Pos2>, click: bool| {
            let mut drawn = Vec::new();
            let mut loaded = false;
            for step in 0..3 {
                let mut input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))),
                    ..Default::default()
                };
                if let Some(p) = pointer {
                    input.events.push(egui::Event::PointerMoved(p));
                    if click && step == 1 {
                        input.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
                        input.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
                    }
                }
                let out = ctx.run_ui(input, |ui| {
                    super::super::inventory_preview::begin_frame(ui.ctx());
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        let mut cx = InventoryContext { config_manager: &mut cm, palette_library: &mut library, account: CloudAccount::default(), script_cloud: &cloud };
                        loaded |= panel.render(ui, &mut cx).load_preset.is_some();
                    });
                });
                drawn = placed(&out.shapes);
            }
            (drawn, loaded)
        };
        let (list, _) = frame(&mut panel, None, false);
        assert!(panel.view().grid, "the Fractals catalog starts as a grid");
        let search = t!("catalog.search").to_string();
        assert_eq!(list.iter().filter(|(t, _)| *t == search).count(), 1, "one search box");
        assert!(!list.iter().any(|(t, _)| *t == t!("fractal_gallery.search").to_string()), "not the gallery's");
        let first = panel.entries.iter().find(|e| matches!(e.key, EntryKey::Preset(_))).unwrap().clone();
        let cell = list.iter().find(|(t, _)| *t == first.title).map(|(_, p)| *p).expect("a preset's cell");

        let (hovered, loaded) = frame(&mut panel, Some(cell + egui::vec2(4.0, 4.0)), false);
        assert!(!loaded, "pointing loads nothing");
        assert_eq!(panel.hovered.as_ref(), Some(&first.key));
        assert!(hovered.iter().filter(|(t, _)| *t == first.title).count() >= 2, "the pane names the cell pointed at");

        let (_, loaded) = frame(&mut panel, Some(cell + egui::vec2(4.0, 4.0)), true);
        assert!(loaded, "a click loads it");
        assert_eq!(panel.selected.as_ref(), Some(&first.key), "and selects it");
    }
}
