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

/// The list every picker reads: the Catalog's, as last refreshed.
pub fn shared() -> Arc<Vec<Entry>> {
    SHARED.read().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// Ask for an entry the app lacks (`!installed`) to be downloaded.
pub fn request_download(e: &Entry) {
    let EntryKey::Registered(kind, name) = &e.key else { return };
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

/// What the panel asks of the app this frame.
#[derive(Default)]
pub struct InventoryResponse {
    /// A fractal to load (`App::load_config_with_undo`).
    pub load_preset: Option<FractalConfig>,
    pub open_palette_editor: bool,
    /// A formula store file to open in the Formula Editor: its name, and
    /// whether it is a shipped example.
    pub open_formula_file: Option<(String, bool)>,
    /// A CPU script to select in the Scripts panel, by its stem.
    pub open_script: Option<String>,
}

/// What the panel needs from the app to draw.
pub struct InventoryContext<'a> {
    pub config_manager: &'a mut ConfigManager,
    /// The palette library the app shows: its packs, as enabled and loaded.
    pub palette_library: &'a mut PaletteLibrary,
    /// With the online account on: the user's cloud palettes, after the
    /// packs (the Palette Library's Cloud Palettes).
    pub cloud_palettes: Option<CloudPalettes<'a>>,
}

/// The cloud palettes' state, and the credentials when signed in.
pub struct CloudPalettes<'a> {
    pub state: &'a mut super::CloudPaletteState,
    pub auth: Option<(&'a str, &'a str)>,
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

    /// The entry selected, as the list last read it.
    pub fn selected(&self) -> Option<&Entry> {
        let key = self.selected.as_ref()?;
        self.entries.iter().find(|e| &e.key == key)
    }

    /// Read the sources again when what they hold has changed, and share
    /// the list with the pickers. Once a frame, before any panel draws.
    pub fn refresh(&mut self, library: &PaletteLibrary, files: u64, online: inventory::Online) {
        let rev = inventory::revision(library, files, online);
        if self.revision != Some(rev) {
            self.entries = Arc::new(inventory::scan(library, online));
            self.revision = Some(rev);
            if let Ok(mut s) = SHARED.write() {
                *s = Some(self.entries.clone());
            }
        }
    }

    fn visible(&self, e: &Entry) -> bool {
        e.kind.family() == self.family && self.kind.is_none_or(|k| k == e.kind) && self.origin.admits(e) && e.matches(&self.query)
    }

    pub fn render(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext) -> InventoryResponse {
        let mut out = InventoryResponse::default();
        self.render_top(ui);
        ui.separator();
        if ui.available_width() >= 560.0 {
            egui::Panel::right("catalog_info").resizable(true).default_size(280.0).min_size(200.0).show_inside(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("catalog_info_scroll").show(ui, |ui| self.render_info(ui, cx, &mut out));
            });
            egui::CentralPanel::default().show_inside(ui, |ui| self.render_list(ui, cx, &mut out));
        } else {
            // Narrow (a docked strip; the phone layout is its own phase):
            // the list, then what is selected under it.
            let height = (ui.available_height() * 0.6).max(160.0);
            ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| self.render_list(ui, cx, &mut out));
            ui.separator();
            egui::ScrollArea::vertical().id_salt("catalog_info_scroll").show(ui, |ui| self.render_info(ui, cx, &mut out));
        }
        out
    }

    // ------------------------------------------------------------------ top

    fn render_top(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for family in Family::ALL {
                let count = self.entries.iter().filter(|e| e.kind.family() == family).count();
                let label = format!("{} ({count})", t!(family.label_key()));
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
            egui::ComboBox::from_id_salt("catalog_origin").selected_text(t!(self.origin.label_key())).show_ui(ui, |ui| {
                for o in OriginFilter::ALL {
                    ui.selectable_value(&mut self.origin, o, t!(o.label_key()));
                }
            });
        });
        // A family of several kinds: a chip each that has entries.
        let mut kinds: Vec<Kind> = self.entries.iter().filter(|e| e.kind.family() == self.family).map(|e| e.kind).collect();
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

    // ----------------------------------------------------------------- list

    fn render_list(&mut self, ui: &mut egui::Ui, cx: &mut InventoryContext, out: &mut InventoryResponse) {
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
                    for &i in &shown[start..end] {
                        let e = &self.entries[i];
                        let r = ui.selectable_label(self.selected.as_ref() == Some(&e.key), row_label(e));
                        let r = if e.description.is_empty() { r } else { r.on_hover_text(crate::markdown::first_paragraph(&e.description)) };
                        if r.clicked() {
                            clicked = Some(e.key.clone());
                        }
                        if r.double_clicked() {
                            used = Some(i);
                        }
                    }
                });
            start = end;
        }
        if let Some(k) = clicked {
            self.selected = Some(k);
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
                    if palette_row(ui, hash, title, palette, is_selected).clicked() {
                        clicked = Some((hash.clone(), palette.clone()));
                    }
                }
            });
        }
        if cx.palette_library.pack_count() == 0 {
            ui.label(t!("palette_library.no_packs"));
            ui.label(t!("palette_library.no_packs_hint"));
        }
        if let Some(cloud) = cx.cloud_palettes.as_mut() {
            ui.separator();
            super::palette_library::render_cloud_palettes_section(ui, cloud.state, cx.config_manager, cloud.auth);
        }
        if let Some((hash, palette)) = clicked {
            self.selected = Some(EntryKey::Palette(hash));
            apply_palette(cx.config_manager, palette);
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
        ui.separator();
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
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        self.shown_gallery().is_some_and(|g| g.generate_one_preview(ctx, render_fn))
    }

    /// WASM: spawn the next preset preview of the gallery drawn this frame.
    #[cfg(target_arch = "wasm32")]
    pub fn start_next_preview(&mut self, device: &egui_wgpu::wgpu::Device, queue: &egui_wgpu::wgpu::Queue, ctx: &egui::Context) {
        if let Some(g) = self.shown_gallery() {
            g.start_next_preview(device, queue, ctx);
        }
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
        let Some(e) = self.selected().cloned() else {
            ui.weak(t!("catalog.select_hint"));
            return;
        };
        ui.heading(&e.title);
        ui.label(egui::RichText::new(format!("{} · {}", t!(e.kind.label_key()), e.group.label)).weak());
        ui.label(origin_text(&e));
        if let Some(v) = &e.version {
            ui.label(egui::RichText::new(t!("catalog.version", version = v)).weak());
        }
        if !e.authors.is_empty() {
            ui.label(t!("catalog.by", names = e.authors.join(", ")));
        }
        ui.separator();
        if let EntryKey::Palette(hash) = &e.key {
            if let Some(p) = inventory::palette(cx.palette_library, hash) {
                let w = ui.available_width().clamp(120.0, 360.0);
                let tex = super::palette_library::palette_preview(ui.ctx(), hash, &p, [360, 24]);
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(w, 24.0)));
                ui.add_space(4.0);
            }
        }
        if e.description.is_empty() {
            ui.weak(t!("catalog.no_description"));
        } else {
            super::markdown::show(ui, &e.description);
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if !e.installed {
                download_button(ui, &e);
                return;
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
                EntryKey::File { store: Store::Formulas, file, shipped } => {
                    if ui.button(t!("catalog.open_in_formula_editor")).clicked() {
                        out.open_formula_file = Some((file.clone(), *shipped));
                    }
                }
                _ => {}
            }
        });
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
fn palette_row(ui: &mut egui::Ui, hash: &str, title: &str, palette: &crate::scene::palette::Palette, selected: bool) -> egui::Response {
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

/// A row's text: the title, the origin's mark, a plug-in's; a download
/// arrow for what is not installed.
pub(crate) fn row_label(e: &Entry) -> String {
    let available = if e.installed { "" } else { "  \u{2193}" };
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

fn apply_palette(config_manager: &mut ConfigManager, palette: crate::scene::palette::Palette) {
    // An editable copy: editing it changes only the config's palette,
    // until it is saved to the library.
    let mut copy = palette;
    copy.built_in = false;
    let _ = config_manager.update_param(ConfigPath::Palette, copy.into());
}

/// Use: what each kind's own picker does when it is chosen there.
fn apply(e: &Entry, ctx: &egui::Context, cx: &mut InventoryContext, out: &mut InventoryResponse) {
    let cm = &mut *cx.config_manager;
    if let EntryKey::File { store: Store::Scripts, file, .. } = &e.key {
        out.open_script = Some(file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s).to_string());
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
        };
        assert!(OriginFilter::Builtin.admits(&e) && !OriginFilter::Yours.admits(&e) && !OriginFilter::Plugins.admits(&e));
        e.origin = Origin::Local;
        e.plugin = true;
        assert!(OriginFilter::Yours.admits(&e) && OriginFilter::Plugins.admits(&e) && !OriginFilter::FromOthers.admits(&e));
        assert!(OriginFilter::ALL.iter().any(|o| *o == OriginFilter::All && o.admits(&e)));
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
