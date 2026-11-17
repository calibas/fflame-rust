//! The Catalog panel (docs/projects/catalog.md): every resource the app
//! knows, from [`crate::inventory`], to browse, read about and use.
//!
//! The top chooses a family (palettes, fractals, variations, ...), a
//! search and an origin; the list groups the family's entries (packs,
//! categories, examples and yours); the info pane says what the selected
//! one is, who wrote it, where it came from, and offers its **Use**. A
//! click selects; Use (or a double click) applies -- except a palette,
//! which a click applies (auditioning palettes is clicking through them),
//! and a fractal, which a click loads, as the Fractal Browser's did: one
//! undo step either way.
//!
//! The selection is the panel's, readable by any panel through
//! [`InventoryPanel::selected`]. Whether Use acts in the current mode is
//! `visibility::catalog_use`'s answer; whether a colouring suits the
//! current formula is the escape engine's own rule.
//!
//! The list is read once a frame, before any panel draws
//! ([`InventoryPanel::refresh`]), and [`shared`] with the pickers
//! (`ui::picker`), so every list of a kind is the same list. What the
//! server offers and the app lacks is listed too, its Use a **Download**
//! ([`request_download`], drained into the app's fetches once a frame).
//!
//! Two places, chosen at the top: **On this device**, the list above, and
//! **Online**, what the server holds for the family, public or your own,
//! page by page (`ui::inventory_online`).

use super::fractal_gallery::FractalConfigGallery;
use crate::config::{ConfigManager, ConfigPath, FractalConfig};
use crate::inventory::{self, Entry, EntryKey, Family, Kind};
use crate::provenance::Origin;
use crate::scene::palette::PaletteLibrary;
use crate::scene::presets::PresetCategory;
use crate::storage::library::Store;
use std::sync::{Arc, Mutex, RwLock};

/// The list as last read, for the pickers.
static SHARED: RwLock<Option<Arc<Vec<Entry>>>> = RwLock::new(None);

/// Downloads asked for since the frame's last drain, and every name asked
/// for this session (its row says it is coming rather than offering it
/// again).
static DOWNLOADS: Mutex<(Vec<(Kind, String)>, Vec<(Kind, String)>)> = Mutex::new((Vec::new(), Vec::new()));

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

/// A panel asked to show the Catalog at a family and place, with an
/// entry selected.
static OPEN: Mutex<Option<(Family, Place, Option<EntryKey>)>> = Mutex::new(None);

/// Show the Catalog at `family` in `place` (opening its panel): what a
/// panel whose online section moved into the Catalog offers instead.
pub fn request_open(family: Family, place: Place) {
    if let Ok(mut o) = OPEN.lock() {
        *o = Some((family, place, None));
    }
}

/// Show an entry in the Catalog, selected (the Rhai Editor's Show in
/// Catalog).
pub fn request_open_select(family: Family, key: EntryKey) {
    if let Ok(mut o) = OPEN.lock() {
        *o = Some((family, Place::Local, Some(key)));
    }
}

/// The open asked for since the last call.
pub fn take_open() -> Option<(Family, Place, Option<EntryKey>)> {
    OPEN.lock().ok()?.take()
}

/// A line saying where something moved, and a button that shows it there.
pub fn moved_here(ui: &mut egui::Ui, what: &str, family: Family, place: Place) {
    ui.horizontal_wrapped(|ui| {
        ui.weak(what);
        if ui.button(t!("catalog.open_in_catalog")).clicked() {
            request_open(family, place);
        }
    });
}

/// The list every picker reads: the Catalog's, as last refreshed.
pub fn shared() -> Arc<Vec<Entry>> {
    SHARED.read().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// Ask for an entry the app lacks (`!installed`) to be downloaded.
pub fn request_download(e: &Entry) {
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
    let EntryKey::Registered(kind, name) = &e.key else { return false };
    DOWNLOADS.lock().is_ok_and(|d| d.1.iter().any(|(k, n)| k == kind && n == name))
}

/// The downloads asked for since the last call, for the app to fetch.
pub fn take_downloads() -> Vec<(Kind, String)> {
    DOWNLOADS.lock().map(|mut d| std::mem::take(&mut d.0)).unwrap_or_default()
}

/// Below this width the list and the info are two screens (phase 5).
const NARROW: f32 = 560.0;

/// Which origins the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OriginFilter {
    #[default]
    All,
    Builtin,
    Yours,
    FromOthers,
    Plugins,
}

impl OriginFilter {
    const ALL: [OriginFilter; 5] = [OriginFilter::All, OriginFilter::Builtin, OriginFilter::Yours, OriginFilter::FromOthers, OriginFilter::Plugins];

    fn label_key(self) -> &'static str {
        match self {
            OriginFilter::All => "catalog.origin_all",
            OriginFilter::Builtin => "catalog.origin_builtin",
            OriginFilter::Yours => "catalog.origin_yours",
            OriginFilter::FromOthers => "catalog.origin_from_others",
            OriginFilter::Plugins => "catalog.origin_plugins",
        }
    }

    fn admits(self, e: &Entry) -> bool {
        match self {
            OriginFilter::All => true,
            OriginFilter::Builtin => e.origin == Origin::Builtin,
            OriginFilter::Yours => e.origin == Origin::Local,
            OriginFilter::FromOthers => e.origin == Origin::Api,
            OriginFilter::Plugins => e.plugin,
        }
    }
}

/// Where the list comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    #[default]
    Local,
    Online,
}

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
    /// What running a script produced (a flame, a batch, an animation).
    pub run: super::script_run::RunResult,
    /// Clear Cache: the downloaded variations, or effects.
    pub clear_variation_cache: bool,
    pub clear_effect_cache: bool,
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
    /// Within a family of several kinds; `None` for all of them.
    kind: Option<Kind>,
    origin: OriginFilter,
    query: String,
    selected: Option<EntryKey>,
    /// Only the downloads the server has a newer version of.
    updates_only: bool,
    place: Place,
    /// Narrow (a phone, a narrow dock): the list and the info are two
    /// screens, and this is the info's (phase 5).
    details: bool,
    /// Whether the panel was narrow when last drawn.
    narrow: bool,
    /// The entries' previews (phase 6), and whether the list is a grid of
    /// them (decision 1).
    previews: super::inventory_preview::Previews,
    grid: bool,
    online: super::inventory_online::OnlinePlace,
    manage: super::inventory_manage::Manage,
    /// A CPU script's run controls, for the script selected.
    run: super::script_run::ScriptRun,
    /// The presets, a gallery a category, made when first shown.
    preset_galleries: Option<Vec<FractalConfigGallery>>,
    preset_category: PresetCategory,
}

impl Default for InventoryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl InventoryPanel {
    pub fn new() -> Self {
        InventoryPanel {
            entries: Arc::new(Vec::new()),
            revision: None,
            family: Family::Palettes,
            kind: None,
            origin: OriginFilter::All,
            query: String::new(),
            selected: None,
            updates_only: false,
            place: Place::Local,
            details: false,
            narrow: false,
            previews: super::inventory_preview::Previews::default(),
            grid: false,
            online: super::inventory_online::OnlinePlace::new(),
            manage: super::inventory_manage::Manage::default(),
            run: super::script_run::ScriptRun::default(),
            preset_galleries: None,
            preset_category: PresetCategory::default(),
        }
    }

    /// Show a family (the Colors panel's and the Palette Editor's palette
    /// library buttons, File > From Preset Library).
    pub fn open_on(&mut self, family: Family) {
        if self.family != family {
            self.family = family;
            self.kind = None;
        }
    }

    /// The entry selected, as the list last read it (an online one, as
    /// its page did).
    pub fn selected(&self) -> Option<&Entry> {
        let key = self.selected.as_ref()?;
        if matches!(key, EntryKey::Online(..)) {
            return self.online.find(key).map(|(e, _)| e);
        }
        self.entries.iter().find(|e| &e.key == key)
    }

    /// Show `family` in `place`, with `select` selected.
    pub fn open_at(&mut self, family: Family, place: Place, select: Option<EntryKey>) {
        self.open_on(family);
        self.place = place;
        if select.is_some() {
            self.selected = select;
        }
    }

    /// Your own online lists changed (a fractal saved online): ask again.
    pub fn forget_online(&mut self) {
        self.online.forget();
    }

    /// The session signed out: the Online place forgets your own lists.
    pub fn signed_out(&mut self) {
        self.online.signed_out();
    }

    /// Read the sources again when what they hold has changed, and share
    /// the list with the pickers. Once a frame, before any panel draws.
    pub fn refresh(&mut self, library: &PaletteLibrary, files: u64, online: inventory::Online) {
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

    fn visible(&self, e: &Entry) -> bool {
        let place = match self.place {
            Place::Local => self.origin.admits(e) && (!self.updates_only || e.update.is_some()),
            // Online, of this device's list: what the server's catalog
            // offers, downloaded or not.
            Place::Online => e.origin == Origin::Api || !e.installed,
        };
        e.kind.family() == self.family && self.kind.is_none_or(|k| k == e.kind) && place && e.matches(&self.query)
    }

    pub fn render(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext) -> InventoryResponse {
        let mut out = InventoryResponse::default();
        self.previews.begin_frame(ui.ctx());
        self.online.update(ui.ctx(), &self.query, &self.entries, &mut out);
        self.render_top(ui, cx, &mut out);
        ui.separator();
        self.narrow = ui.available_width() < NARROW;
        if !self.narrow {
            self.details = false;
            egui::Panel::right("catalog_info").resizable(true).default_size(280.0).min_size(200.0).show_inside(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("catalog_info_scroll").show(ui, |ui| self.render_info(ui, cx, &mut out));
            });
            egui::CentralPanel::default().show_inside(ui, |ui| self.render_list(ui, cx, &mut out));
        } else if self.details && self.selected.is_some() {
            // Narrow (a phone, a narrow dock): the info is a screen of its
            // own, Back to the list.
            if ui.button(t!("catalog.back")).clicked() {
                self.details = false;
            }
            ui.separator();
            egui::ScrollArea::vertical().id_salt("catalog_info_scroll").show(ui, |ui| self.render_info(ui, cx, &mut out));
        } else {
            self.details = false;
            self.render_list(ui, cx, &mut out);
        }
        out
    }

    /// The selected row's way to its info, on a narrow panel: a › beside
    /// it. Gives whether it was pressed.
    fn details_button(ui: &mut egui::Ui) -> bool {
        ui.small_button("\u{203a}").on_hover_text(t!("catalog.details")).clicked()
    }

    // ------------------------------------------------------------------ top

    fn render_top(&mut self, ui: &mut egui::Ui, cx: &InventoryContext, out: &mut InventoryResponse) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.place, Place::Local, t!("catalog.place_local"));
            ui.selectable_value(&mut self.place, Place::Online, t!("catalog.place_online")).on_hover_text(t!("catalog.place_online_tip"));
        });
        ui.horizontal_wrapped(|ui| {
            for family in Family::ALL {
                let label = match self.place {
                    Place::Local => {
                        let count = self.entries.iter().filter(|e| e.kind.family() == family).count();
                        format!("{} ({count})", t!(family.label_key()))
                    }
                    Place::Online => t!(family.label_key()).to_string(),
                };
                if ui.selectable_label(self.family == family, label).clicked() {
                    self.open_on(family);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.query).hint_text(t!("catalog.search")).desired_width(180.0));
            if !self.query.is_empty() && ui.small_button("×").on_hover_text(t!("catalog.clear_search")).clicked() {
                self.query.clear();
            }
            if self.place == Place::Local {
                egui::ComboBox::from_id_salt("catalog_origin").selected_text(t!(self.origin.label_key())).show_ui(ui, |ui| {
                    for o in OriginFilter::ALL {
                        ui.selectable_value(&mut self.origin, o, t!(o.label_key()));
                    }
                });
                if super::inventory_preview::has_previews(self.family) {
                    ui.selectable_value(&mut self.grid, false, t!("catalog.view_list"));
                    ui.selectable_value(&mut self.grid, true, t!("catalog.view_grid")).on_hover_text(t!("catalog.view_grid_tip"));
                }
            }
        });
        if self.place == Place::Online {
            self.online.render_top(ui, self.family, &cx.account);
        }
        if self.place == Place::Local && matches!(self.family, Family::Variations | Family::Effects) {
            self.render_downloads_bar(ui, out);
        }
        if self.place == Place::Local {
            if let Some(super::inventory_manage::Act::Select(key)) = self.manage.render_top(ui, self.family, out) {
                self.selected = Some(key);
            }
        }
        // A family of several kinds: a chip each that has entries.
        let mut kinds: Vec<Kind> = match self.place {
            Place::Local => self.entries.iter().filter(|e| e.kind.family() == self.family).map(|e| e.kind).collect(),
            Place::Online => super::inventory_online::kinds(self.family),
        };
        kinds.dedup();
        if kinds.len() > 1 {
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(self.kind.is_none(), t!("catalog.kind_all")).clicked() {
                    self.kind = None;
                }
                for k in kinds {
                    if ui.selectable_label(self.kind == Some(k), t!(k.label_key())).clicked() {
                        self.kind = Some(k);
                    }
                }
            });
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

    fn render_list(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext, out: &mut InventoryResponse) {
        if self.place == Place::Online {
            egui::ScrollArea::vertical().id_salt("catalog_online_list").auto_shrink([false, false]).show(ui, |ui| {
                // Variations and effects: the cached catalog first (it is
                // the server's whole list), then the Rhai ones people
                // published.
                if matches!(self.family, Family::Variations | Family::Effects) {
                    self.render_grouped(ui, cx, out);
                }
                if self.online.render_list(ui, self.family, self.kind, &mut self.selected, self.narrow, cx) {
                    self.details = true;
                }
            });
            return;
        }
        // The presets as galleries, unless a search or a filter asks for
        // rows.
        if self.family == Family::Fractals && self.query.trim().is_empty() && self.origin == OriginFilter::All {
            self.render_presets(ui, out);
            return;
        }
        egui::ScrollArea::vertical().id_salt("catalog_list").auto_shrink([false, false]).show(ui, |ui| {
            if self.family == Family::Palettes {
                self.render_palettes(ui, cx);
            } else {
                self.render_grouped(ui, cx, out);
            }
        });
    }

    /// The entries in groups, each a collapsing heading with its count.
    fn render_grouped(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext, out: &mut InventoryResponse) {
        let shown: Vec<usize> = (0..self.entries.len()).filter(|&i| self.visible(&self.entries[i])).collect();
        if shown.is_empty() {
            ui.weak(t!("catalog.nothing_matches"));
            return;
        }
        let several = matches!(self.family, Family::Escape | Family::Simulation | Family::Scripts) && self.kind.is_none();
        let (mut clicked, mut used) = (None, None);
        let mut details = false;
        let grid = self.grid && self.place == Place::Local && super::inventory_preview::has_previews(self.family);
        let palettes: Vec<crate::scene::palette::Palette> = if grid { cx.palette_library.iter().cloned().collect() } else { Vec::new() };
        let mut start = 0;
        while start < shown.len() {
            let first = &self.entries[shown[start]];
            let mut end = start + 1;
            while end < shown.len() && self.entries[shown[end]].kind == first.kind && self.entries[shown[end]].group == first.group {
                end += 1;
            }
            let kind_label = t!(first.kind.label_key());
            let heading = if several && first.group.label != kind_label {
                format!("{kind_label} · {} ({})", first.group.label, end - start)
            } else {
                format!("{} ({})", first.group.label, end - start)
            };
            let id = ui.make_persistent_id(("catalog_group", first.kind, first.group.order, first.group.label.as_str()));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                .show_header(ui, |ui| {
                    ui.strong(heading);
                })
                .body(|ui| {
                    if grid {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                            for &i in &shown[start..end] {
                                let e = &self.entries[i];
                                let selected = self.selected.as_ref() == Some(&e.key);
                                // Only a cell on screen asks for its picture.
                                let size = egui::vec2(super::inventory_online::CELL, super::inventory_online::CELL + 18.0);
                                let visible = ui.is_rect_visible(egui::Rect::from_min_size(ui.cursor().min, size));
                                let shown = if visible { self.previews.want(e, ui.ctx(), &palettes) } else { super::inventory_preview::Shown::Pending };
                                let (texture, has) = match &shown {
                                    super::inventory_preview::Shown::Ready(t) => (t.last().cloned(), true),
                                    super::inventory_preview::Shown::Pending => (None, true),
                                    super::inventory_preview::Shown::None => (None, false),
                                };
                                let r = super::inventory_online::grid_cell(ui, e, texture.as_ref(), selected, has);
                                if r.clicked() {
                                    clicked = Some(e.key.clone());
                                }
                                if r.double_clicked() {
                                    used = Some(i);
                                }
                            }
                        });
                        return;
                    }
                    for &i in &shown[start..end] {
                        let e = &self.entries[i];
                        let selected = self.selected.as_ref() == Some(&e.key);
                        ui.horizontal(|ui| {
                            let r = ui.selectable_label(selected, row_label(e));
                            let r = if e.description.is_empty() { r } else { r.on_hover_text(crate::markdown::first_paragraph(&e.description)) };
                            if r.clicked() {
                                clicked = Some(e.key.clone());
                            }
                            if r.double_clicked() {
                                used = Some(i);
                            }
                            if selected && self.narrow && Self::details_button(ui) {
                                details = true;
                            }
                        });
                    }
                });
            start = end;
        }
        if let Some(k) = clicked {
            self.selected = Some(k);
        }
        if details {
            self.details = true;
        }
        if let Some(i) = used {
            let e = self.entries[i].clone();
            if !e.installed {
                request_download(&e);
            } else if usable(&e, cx.config_manager.active_config()).is_ok() {
                apply(&e, ui.ctx(), cx, out);
            }
        }
    }

    /// The palettes, pack by pack -- each pack's checkbox and load state,
    /// as the Palette Library had them -- a row each: the name and its
    /// gradient. A click applies it and selects it. Searching, a pack
    /// without a match is left out.
    fn render_palettes(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext) {
        let searching = !self.query.trim().is_empty() || self.origin != OriginFilter::All;
        let mut clicked: Option<(String, crate::scene::palette::Palette)> = None;
        let (narrow, mut details) = (self.narrow, false);
        for pack in 0..cx.palette_library.pack_count() {
            // This pack's entries, in its order (the scan keeps it), with
            // their index in the pack, those the search admits.
            let rows: Vec<(usize, String, String)> = self
                .entries
                .iter()
                .filter(|e| e.kind == Kind::Palette && e.group.order == pack as u32)
                .enumerate()
                .filter(|(_, e)| self.origin.admits(e) && e.matches(&self.query))
                .filter_map(|(j, e)| match &e.key {
                    EntryKey::Palette(h) => Some((j, h.clone(), e.title.clone())),
                    _ => None,
                })
                .collect();
            if searching && rows.is_empty() {
                continue;
            }
            let selected = self.selected.clone();
            super::palette_library::pack_section(ui, cx.palette_library, pack, |ui, lib| {
                let Some(p) = lib.get_pack(pack) else { return };
                for (j, hash, title) in &rows {
                    let Some(palette) = p.palettes.get(*j) else { continue };
                    let is_selected = selected.as_ref().is_some_and(|k| *k == EntryKey::Palette(hash.clone()));
                    let row = if is_selected && narrow {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if Self::details_button(ui) {
                                details = true;
                            }
                            palette_row(ui, hash, title, palette, is_selected)
                        })
                        .inner
                    } else {
                        palette_row(ui, hash, title, palette, is_selected)
                    };
                    if row.clicked() {
                        clicked = Some((hash.clone(), palette.clone()));
                    }
                }
            });
        }
        if cx.palette_library.pack_count() == 0 {
            ui.label(t!("palette_library.no_packs"));
            ui.label(t!("palette_library.no_packs_hint"));
        }
        if cx.account.signed_in {
            // Cloud Palettes became the Online place's own palettes.
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.weak(t!("catalog.your_online_palettes"));
                if ui.button(t!("catalog.show_mine")).clicked() {
                    self.place = Place::Online;
                    self.online.scope = crate::api::Scope::Mine;
                }
            });
        }
        if let Some((hash, palette)) = clicked {
            self.selected = Some(EntryKey::Palette(hash));
            apply_palette(cx.config_manager, palette);
        }
        if details {
            self.details = true;
        }
    }

    // -------------------------------------------------------------- presets

    /// The presets as galleries, a tab a category: a click loads one.
    fn render_presets(&mut self, ui: &mut egui::Ui, out: &mut InventoryResponse) {
        let galleries = self.preset_galleries.get_or_insert_with(preset_galleries);
        if galleries[self.preset_category.index()].len() == 0 {
            if let Some(c) = PresetCategory::ALL.into_iter().find(|c| galleries[c.index()].len() > 0) {
                self.preset_category = c;
            }
        }
        ui.horizontal_wrapped(|ui| {
            for category in PresetCategory::ALL {
                let count = galleries[category.index()].len();
                if count == 0 {
                    continue;
                }
                let label = format!("{} ({count})", t!(category.label_key()));
                if ui.selectable_label(self.preset_category == category, label).clicked() {
                    self.preset_category = category;
                }
            }
        });
        // Narrow: the selection's way to its info, above the gallery.
        if self.narrow {
            if let Some(title) = self.selected.as_ref().and_then(|k| self.entries.iter().find(|e| &e.key == k)).filter(|e| e.kind == Kind::Fractal).map(|e| e.title.clone()) {
                ui.horizontal(|ui| {
                    ui.strong(title);
                    if Self::details_button(ui) {
                        self.details = true;
                    }
                });
            }
        }
        ui.separator();
        let galleries = self.preset_galleries.get_or_insert_with(preset_galleries);
        let response = galleries[self.preset_category.index()].render(ui);
        if let Some(config) = response.selected {
            // Its entry: the first preset of that name (the inventory's
            // key for it).
            if let Some(e) = self.entries.iter().find(|e| e.kind == Kind::Fractal && e.title == config.flame.name) {
                self.selected = Some(e.key.clone());
            }
            out.load_preset = Some(config);
        }
    }

    /// Desktop: make one preset preview for the gallery drawn this frame.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one_preview<F>(&mut self, ctx: &egui::Context, render_fn: F) -> bool
    where
        F: Fn(&FractalConfig) -> image::RgbaImage,
    {
        // The presets' gallery first, then the entries' previews.
        if self.shown_gallery().is_some_and(|g| g.generate_one_preview(ctx, &render_fn)) {
            return true;
        }
        self.previews.generate_one(ctx, render_fn)
    }

    /// WASM: spawn the next preset preview of the gallery drawn this frame.
    #[cfg(target_arch = "wasm32")]
    pub fn start_next_preview(&mut self, device: &egui_wgpu::wgpu::Device, queue: &egui_wgpu::wgpu::Queue, ctx: &egui::Context) {
        if let Some(g) = self.shown_gallery() {
            g.start_next_preview(device, queue, ctx);
        }
        self.previews.start_next(device, queue, ctx);
    }

    /// The gallery drawn this frame, every gallery's flag taken: only it
    /// makes previews, so a closed panel or another family makes none.
    fn shown_gallery(&mut self) -> Option<&mut FractalConfigGallery> {
        let mut shown = None;
        for g in self.preset_galleries.iter_mut().flatten() {
            if g.take_shown() {
                shown = Some(g);
            }
        }
        shown
    }

    // ----------------------------------------------------------------- info

    fn render_info(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext, out: &mut InventoryResponse) {
        if let Some(key @ EntryKey::Online(..)) = self.selected.clone() {
            match self.online.render_info(ui, &key, cx, out) {
                Some(super::inventory_online::InfoAction::ShowLocal(local)) => {
                    if let Some(e) = self.entries.iter().find(|e| e.key == local) {
                        self.family = e.kind.family();
                        self.kind = None;
                    }
                    self.place = Place::Local;
                    self.selected = Some(local);
                }
                None if self.online.find(&key).is_none() => {
                    ui.weak(t!("catalog.select_hint"));
                }
                None => {}
            }
            return;
        }
        let Some(e) = self.selected().cloned() else {
            ui.weak(t!("catalog.select_hint"));
            return;
        };
        let palettes: Vec<crate::scene::palette::Palette> = cx.palette_library.iter().cloned().collect();
        let preview = self.previews.want(&e, ui.ctx(), &palettes);
        entry_summary(ui, &e, Some(cx.palette_library), Some(&preview));
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
            match usable(&e, cx.config_manager.active_config()) {
                Ok(()) => {
                    if ui.button(t!(use_label_key(&e))).clicked() {
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
        // Your own: edit, rename, duplicate, delete, the plug-in flag,
        // publishing; a shipped file or a built-in: a copy of yours.
        if let Some(super::inventory_manage::Act::Select(key)) = self.manage.render_actions(ui, &e, cx, out) {
            self.selected = Some(key);
        }
    }
}

/// The presets from the library, sorted into their categories' galleries
/// (`PresetCategory::ALL` order), each in the library's order.
fn preset_galleries() -> Vec<FractalConfigGallery> {
    let mut by: Vec<Vec<FractalConfig>> = vec![Vec::new(); PresetCategory::ALL.len()];
    for p in crate::scene::presets::global_preset_library().presets() {
        by[PresetCategory::of(p).index()].push(p.clone());
    }
    by.into_iter().map(FractalConfigGallery::new).collect()
}

/// A palette row: its name, and its gradient at the right; the texture is
/// made only once the row is on screen.
pub(super) fn palette_row(ui: &mut egui::Ui, hash: &str, title: &str, palette: &crate::scene::palette::Palette, selected: bool) -> egui::Response {
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
    let tex = super::palette_library::palette_preview(ui.ctx(), hash, palette, [200, 20]);
    egui::Image::new(&tex).fit_to_exact_size(img.size()).paint_at(ui, img);
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
    format!("{}{}{available}", e.title, crate::storage::library::marks(e.origin, e.plugin))
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
    if e.plugin {
        format!("{base} {}", t!("catalog.plugin_note"))
    } else {
        base.to_string()
    }
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

/// A registered plug-in's record, by its file.
pub(crate) fn plugin_of(e: &Entry) -> Option<crate::storage::library::Plugin> {
    let EntryKey::File { store: Store::Formulas, file, shipped: false } = &e.key else { return None };
    crate::storage::library::plugins().into_iter().find(|p| &p.file == file)
}

/// The registry name an entry applies by: its own, or a plug-in's key.
pub(crate) fn applied_name(e: &Entry) -> Option<String> {
    match &e.key {
        EntryKey::Registered(_, n) => Some(n.clone()),
        EntryKey::File { .. } if e.plugin => plugin_of(e).map(|p| p.key),
        _ => None,
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
    // A file not registered is used from the editor (a Rhai entry), or
    // run from the Scripts panel (a CPU script).
    if let EntryKey::File { store, .. } = &e.key {
        if *store == Store::Formulas && !e.plugin {
            return Err(t!("catalog.open_to_use").to_string());
        }
        if e.plugin && plugin_of(e).is_none() {
            return Err(t!("catalog.plugin_not_loaded").to_string());
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
    let plugin = if e.plugin { plugin_of(e) } else { None };
    let name = applied_name(e).unwrap_or_default();
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

    /// The Catalog holds every preset, each in its category's gallery (as
    /// the Fractal Browser's Presets tab did).
    #[test]
    fn the_catalog_sorts_the_presets_into_their_galleries() {
        let presets = crate::resources::load_presets_with_fallback();
        let galleries = preset_galleries();
        for category in PresetCategory::ALL {
            let want = presets.iter().filter(|p| PresetCategory::of(p) == category).count();
            assert_eq!(galleries[category.index()].len(), want, "{category:?}");
        }
    }

    /// The origin filter: built-in, the user's, somebody else's, plug-ins.
    #[test]
    fn the_origin_filter_reads_the_origin_and_the_plugin_flag() {
        let mut e = Entry {
            kind: Kind::Formula,
            key: EntryKey::Registered(Kind::Formula, "mandelbrot".into()),
            title: "Mandelbrot".into(),
            description: String::new(),
            authors: Vec::new(),
            version: None,
            origin: Origin::Builtin,
            plugin: false,
            group: inventory::Group { order: 0, label: String::new() },
            link: None,
            installed: true,
            category: None,
            update: None,
            online: None,
        };
        assert!(OriginFilter::Builtin.admits(&e) && !OriginFilter::Yours.admits(&e) && !OriginFilter::Plugins.admits(&e));
        e.origin = Origin::Local;
        e.plugin = true;
        assert!(OriginFilter::Yours.admits(&e) && OriginFilter::Plugins.admits(&e) && !OriginFilter::FromOthers.admits(&e));
        assert!(OriginFilter::ALL.iter().any(|o| *o == OriginFilter::All && o.admits(&e)));
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
            plugin: false,
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

    /// Narrow (a phone): the list fills the panel; the selected row's ›
    /// opens its info as a screen of its own, Back returns to the list.
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
        let chevron = list.iter().find(|(t, _)| t == "\u{203a}").map(|(_, p)| *p).expect("the selected row's ›");
        assert!(!list.iter().any(|(t, _)| t == &t!("catalog.back").to_string()), "the list alone");

        let info = frame(&mut panel, Some(chevron + egui::vec2(3.0, 6.0)));
        assert!(panel.details, "the › opens the info");
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
        panel.grid = true;
        let ctx = egui::Context::default();
        let mut drawn = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))), ..Default::default() };
            let out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    let mut cx = InventoryContext { config_manager: &mut cm, palette_library: &mut library, account: CloudAccount::default(), script_cloud: &cloud };
                    let _ = panel.render(ui, &mut cx);
                });
            });
            drawn = placed(&out.shapes);
        }
        assert!(drawn.iter().any(|(t, _)| t == "Linear"), "a cell names its entry");
        let waiting = panel.previews.waiting();
        let variations = panel.entries.iter().filter(|e| e.kind == Kind::Variation).count();
        assert!(waiting > 0 && waiting < variations / 4, "the cells on screen ask, not all {variations}: {waiting}");
    }
}
