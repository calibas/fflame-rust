//! The Catalog's list as a picker (docs/projects/catalog.md, *The picker*),
//! in a popup under a button:
//! the entries of [`super::inventory::shared`] a filter admits, in the
//! Catalog's groups, with a search, an info pane beside the list saying
//! what the entry under the pointer is, and **Download** for what the server offers and the app lacks
//! (pickable once installed). It replaces the hand-written pickers, which
//! each sorted, grouped and searched their own way and none of which could
//! download.
//!
//! ```ignore
//! let button = ui.button("Add variation");
//! if let Some(entry) = Picker::new("add_variation", &|e| e.kind == Kind::Variation).popup(ui, &button) {
//!     // the entry's own Use: the picker's caller knows what choosing means
//! }
//! ```
//!
//! A pick is an installed entry the caller's `unusable` did not refuse;
//! the caller applies it (`inventory::applied_name` is the registry name
//! a plug-in file applies by).

use crate::inventory::{Entry, EntryKey, Group, Kind};
use crate::provenance::Origin;

/// A filtered view of the Catalog's list, shown as a popup or in place.
pub struct Picker<'a> {
    id: egui::Id,
    admit: &'a dyn Fn(&Entry) -> bool,
    /// Rows before the list's, shown whatever `admit` says (a "None", a
    /// shortlist).
    lead: Vec<Entry>,
    /// Entries the Catalog does not list but this picker offers (a
    /// fractal's own Rhai variations), after the list's.
    extra: Vec<Entry>,
    /// The registry name of the current choice, shown selected.
    current: Option<&'a str>,
    /// Why an entry cannot be picked here, shown greyed with the reason.
    unusable: Option<&'a dyn Fn(&Entry) -> Option<String>>,
    /// Group headings name the kind too (a picker of several kinds).
    kind_headings: bool,
    /// The palette library, for a palette's gradient in the info pane.
    palettes: Option<&'a crate::scene::palette::PaletteLibrary>,
    min_width: f32,
    max_height: f32,
}

/// The info pane's width, beside the list.
const PANE_WIDTH: f32 = 280.0;

impl<'a> Picker<'a> {
    pub fn new(id_salt: impl std::hash::Hash, admit: &'a dyn Fn(&Entry) -> bool) -> Self {
        Picker {
            id: egui::Id::new(("catalog_picker", id_salt)),
            admit,
            lead: Vec::new(),
            extra: Vec::new(),
            current: None,
            unusable: None,
            kind_headings: false,
            palettes: None,
            min_width: 260.0,
            max_height: 320.0,
        }
    }

    pub fn lead(mut self, entries: Vec<Entry>) -> Self {
        self.lead = entries;
        self
    }

    pub fn extra(mut self, entries: Vec<Entry>) -> Self {
        self.extra = entries;
        self
    }

    pub fn current(mut self, name: Option<&'a str>) -> Self {
        self.current = name;
        self
    }

    pub fn unusable(mut self, why: &'a dyn Fn(&Entry) -> Option<String>) -> Self {
        self.unusable = Some(why);
        self
    }

    pub fn kind_headings(mut self) -> Self {
        self.kind_headings = true;
        self
    }

    pub fn palettes(mut self, library: &'a crate::scene::palette::PaletteLibrary) -> Self {
        self.palettes = Some(library);
        self
    }

    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = h;
        self
    }

    /// The popup's id, for a caller that opens or closes it itself.
    pub fn popup_id(&self) -> egui::Id {
        self.id.with("popup")
    }

    /// A popup under `button`, opened and closed by its clicks (and closed
    /// by a click outside or a pick); the search has the focus on opening.
    /// A Popup, not a ComboBox: a combo closes on any click inside it,
    /// its search box included.
    pub fn popup(self, ui: &mut egui::Ui, button: &egui::Response) -> Option<Entry> {
        let popup_id = self.popup_id();
        let focus_id = self.id.with("focus");
        if button.clicked() {
            egui::Popup::toggle_id(ui.ctx(), popup_id);
            ui.data_mut(|d| d.insert_temp(focus_id, true));
        }
        let mut picked = None;
        egui::Popup::from_response(button)
            .id(popup_id)
            .open_memory(None)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                let focus = ui.data_mut(|d| d.remove_temp::<bool>(focus_id).unwrap_or(false));
                picked = self.body(ui, focus);
                if picked.is_some() {
                    egui::Popup::close_id(ui.ctx(), popup_id);
                }
            });
        picked
    }

    /// The list, and beside it the info pane: what the entry under the
    /// pointer is (the last one pointed at stays while the pointer crosses
    /// to the pane; on opening, the current choice). A screen too narrow
    /// for both (a phone) keeps the list alone, with descriptions as
    /// tooltips.
    fn body(self, ui: &mut egui::Ui, focus: bool) -> Option<Entry> {
        let pane = ui.ctx().content_rect().width() >= self.min_width + PANE_WIDTH + 64.0;
        let shown_id = self.id.with("shown");
        if focus {
            ui.data_mut(|d| d.remove::<Entry>(shown_id));
        }
        let mut picked = None;
        let mut hovered = None;
        let mut current = None;
        ui.horizontal_top(|ui| {
            let list = ui
                .vertical(|ui| {
                    ui.set_width(self.min_width);
                    picked = self.list(ui, focus, !pane, &mut hovered, &mut current);
                })
                .response
                .rect;
            if !pane {
                return;
            }
            ui.add_space(12.0);
            if let Some(e) = &hovered {
                ui.data_mut(|d| d.insert_temp(shown_id, e.clone()));
            }
            let shown = hovered.or_else(|| ui.data(|d| d.get_temp::<Entry>(shown_id))).or(current);
            let info = ui
                .vertical(|ui| {
                    ui.set_width(PANE_WIDTH);
                    egui::ScrollArea::vertical().id_salt(self.id.with("info")).max_height(self.max_height + 32.0).show(ui, |ui| self.info(ui, shown.as_ref()));
                })
                .response
                .rect;
            let x = list.max.x + 6.0;
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().vline(x, list.min.y..=list.max.y.max(info.max.y), stroke);
        });
        picked
    }

    /// What `e` is, in the pane: the Catalog's own summary (a row made for
    /// this picker -- a shortlist's -- reads the Catalog's entry of the
    /// same name), why it cannot be picked here, Download.
    fn info(&self, ui: &mut egui::Ui, e: Option<&Entry>) {
        let Some(e) = e else {
            ui.weak(t!("catalog.hover_hint"));
            return;
        };
        let shared = super::inventory::shared();
        let e = match e.description.is_empty() {
            true => shared.iter().find(|s| s.key == e.key && !s.description.is_empty()).unwrap_or(e),
            false => e,
        };
        if key_name(e) == Some("") {
            // A "None": nothing to say.
            ui.heading(&e.title);
            return;
        }
        super::inventory::entry_summary(ui, e, self.palettes, None);
        if !e.installed {
            ui.separator();
            super::inventory::download_button(ui, e);
        } else if let Some(reason) = self.unusable.and_then(|why| why(e)) {
            ui.separator();
            ui.colored_label(ui.visuals().warn_fg_color, reason);
        }
    }

    fn list(&self, ui: &mut egui::Ui, focus: bool, tips: bool, hovered: &mut Option<Entry>, current: &mut Option<Entry>) -> Option<Entry> {
        let search_id = self.id.with("search");
        let mut query = ui.data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut query).hint_text(t!("catalog.search")).desired_width(f32::INFINITY));
            super::vkb_sync(ui, &r, &query);
            if focus {
                r.request_focus();
            }
        });
        ui.data_mut(|d| d.insert_temp(search_id, query.clone()));
        ui.separator();

        let shared = super::inventory::shared();
        let rows: Vec<&Entry> = self
            .lead
            .iter()
            .filter(|e| e.matches(&query))
            .chain(shared.iter().chain(self.extra.iter()).filter(|e| (self.admit)(e) && e.matches(&query)))
            .collect();
        let mut picked = None;
        egui::ScrollArea::vertical().id_salt(self.id.with("scroll")).max_height(self.max_height).show(ui, |ui| {
            if rows.is_empty() {
                ui.weak(t!("catalog.nothing_matches"));
                return;
            }
            let mut start = 0;
            while start < rows.len() {
                let first = rows[start];
                let mut end = start + 1;
                while end < rows.len() && rows[end].kind == first.kind && rows[end].group == first.group {
                    end += 1;
                }
                let heading = if self.kind_headings && first.group.label != t!(first.kind.label_key()) {
                    format!("{} · {}", t!(first.kind.label_key()), first.group.label)
                } else {
                    first.group.label.clone()
                };
                if !heading.is_empty() {
                    ui.label(egui::RichText::new(heading).strong());
                }
                for e in &rows[start..end] {
                    let (p, rect, is_current) = self.row(ui, e, tips);
                    if p.is_some() {
                        picked = p;
                    }
                    if ui.rect_contains_pointer(rect) {
                        *hovered = Some((*e).clone());
                    }
                    if is_current && current.is_none() {
                        *current = Some((*e).clone());
                    }
                }
                ui.add_space(4.0);
                start = end;
            }
        });
        picked
    }

    /// One entry: selectable when it can be picked; greyed (with the reason
    /// as a tooltip, without the pane) when not; Download when it is not
    /// installed. Gives the pick, the row's rect and whether it is the
    /// current choice.
    fn row(&self, ui: &mut egui::Ui, e: &Entry, tips: bool) -> (Option<Entry>, egui::Rect, bool) {
        let tip = (tips && !e.description.is_empty()).then(|| crate::markdown::first_paragraph(&e.description));
        if !e.installed {
            let rect = ui
                .horizontal(|ui| {
                    let r = ui.weak(super::inventory::row_label(e));
                    if let Some(t) = &tip {
                        r.on_hover_text(t);
                    }
                    super::inventory::download_button(ui, e);
                })
                .response
                .rect;
            return (None, rect, false);
        }
        let current = self.current.is_some_and(|c| super::inventory::applied_name(e).as_deref() == Some(c) || key_name(e) == Some(c));
        if let Some(reason) = self.unusable.and_then(|why| why(e)) {
            let r = ui.add_enabled(false, egui::Button::selectable(current, super::inventory::row_label(e)));
            let rect = r.rect;
            if tips {
                r.on_disabled_hover_text(reason);
            }
            return (None, rect, current);
        }
        let r = ui.selectable_label(current, super::inventory::row_label(e));
        let r = match &tip {
            Some(t) => r.on_hover_text(t),
            None => r,
        };
        (r.clicked().then(|| e.clone()), r.rect, current)
    }
}

/// A picker's button: the current choice, and the popup's mark.
pub fn button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(format!("{label}  \u{25be}")).min_size(egui::vec2(200.0, 0.0)))
}

/// An entry's registry name, when it is registered (a palette's: its
/// content hash).
pub fn key_name(e: &Entry) -> Option<&str> {
    match &e.key {
        EntryKey::Registered(_, n) | EntryKey::Palette(n) => Some(n.as_str()),
        _ => None,
    }
}

/// A row that is not an entry of the Catalog: a registered name of `kind`
/// under `group` (a "None" is the empty name).
pub fn row(kind: Kind, name: &str, title: &str, group: Group) -> Entry {
    Entry {
        kind,
        key: EntryKey::Registered(kind, name.to_string()),
        title: title.to_string(),
        description: String::new(),
        authors: Vec::new(),
        version: None,
        origin: Origin::Local,
        plugin: false,
        group,
        link: None,
        installed: true,
        category: None,
        update: None,
        online: None,
    }
}

/// The fractal's own Rhai variations as rows: those the registry holds as
/// the user's (`Provenance::Local`) that no plug-in file stands for (the
/// Catalog lists those), one version each, under "Yours".
pub fn own_variations(flame: &crate::scene::transforms::Flame) -> Vec<Entry> {
    let shared = super::inventory::shared();
    let plugin_keys: std::collections::HashSet<String> = shared
        .iter()
        .filter(|e| e.plugin && e.kind == Kind::Variation)
        .filter_map(super::inventory::applied_name)
        .collect();
    let registry = crate::variations::global_registry();
    let superseded = registry.superseded(&flame.custom_variations);
    let group = Group { order: 2000, label: t!("inventory.group.yours").to_string() };
    registry
        .all()
        .into_iter()
        .filter(|v| v.provenance == crate::provenance::Provenance::Local && !plugin_keys.contains(&v.name) && !superseded.contains(&v.name))
        // Registered only to draw a Catalog preview, unless the fractal
        // carries it.
        .filter(|v| !super::inventory_preview::preview_only(&v.name) || flame.custom_variations.iter().any(|c| c.entry == v.name))
        .map(|v| row(Kind::Variation, &v.name, &v.display_name, group.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    /// The popup must not be a `ComboBox`: a combo closes on ANY click
    /// inside it, so its search box was unusable -- clicking it dismissed
    /// the list instead of filtering it (reported from use, on the lens
    /// picker). A memory-backed `Popup` that closes on a click outside or
    /// a pick. Source-scanning because the fault is a widget choice, and
    /// the difference only shows under a click a headless test does not
    /// deliver.
    #[test]
    fn the_picker_stays_open_while_it_is_used() {
        // Whatever line ends the checkout gave it.
        let src = include_str!("picker.rs").replace("\r\n", "\n");
        let start = src.find("pub fn popup(").expect("the popup moved");
        let end = start + src[start..].find("\n    }\n").expect("its end");
        let body: String = src[start..end].lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
        assert!(!body.contains("ComboBox"), "the picker is a ComboBox again");
        assert!(body.contains("PopupCloseBehavior::CloseOnClickOutside"), "it must close only on a click outside");
        assert!(body.contains("Popup::close_id"), "a pick must close it explicitly");
    }

    /// What the tests' server offers, shared: the list the pickers read is
    /// one static, so every test that fills it must fill it alike.
    fn offer() {
        let item = |name: &str, title: &str| -> crate::api::types::VariationListItem {
            serde_json::from_value(serde_json::json!({
                "id": name, "name": name, "display_name": title, "category": "Plugin", "version": 1, "downloadable": true
            }))
            .unwrap()
        };
        let offered = [item("it_picker_offered", "Offered Afar"), item("it_pane_offered", "Pane Faraway")];
        let lib = crate::scene::palette::global_palette_library().read().unwrap().clone();
        super::super::inventory::InventoryPanel::new().refresh(&lib, 0, crate::inventory::Online { variations: &offered, effects: &[] });
    }

    /// Every piece of text a frame drew.
    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
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

    /// The popup, laid out for real over the shared list: the Catalog's
    /// groups and rows, a search that narrows them, and Download for what
    /// the server offers and the app lacks.
    #[test]
    fn the_picker_shows_groups_searches_and_offers_downloads() {
        use super::*;
        offer();

        let ctx = egui::Context::default();
        let admit = |e: &Entry| e.kind == Kind::Variation;
        let frame = |ctx: &egui::Context, query: Option<&str>| -> Vec<String> {
            let mut drawn = Vec::new();
            for _ in 0..2 {
                let out = ctx.run_ui(Default::default(), |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        let picker = Picker::new("picker_test", &admit).max_height(100_000.0);
                        if let Some(q) = query {
                            ui.data_mut(|d| d.insert_temp(picker.id.with("search"), q.to_string()));
                        }
                        let btn = ui.button("open");
                        egui::Popup::open_id(ui.ctx(), picker.popup_id());
                        let _ = picker.popup(ui, &btn);
                    });
                });
                drawn = texts(&out.shapes);
            }
            drawn
        };

        let all = frame(&ctx, None);
        assert!(all.iter().any(|t| t == "Linear"), "a built-in row");
        assert!(all.iter().any(|t| t == &t!("inventory.variation_category.basic_2d").to_string()), "its group's heading");
        // Past the 647 built-ins, so found by a search.
        let offered = frame(&ctx, Some("afar"));
        assert!(offered.iter().any(|t| t.starts_with("Offered Afar")), "what the server offers is listed: {offered:?}");
        assert!(offered.iter().any(|t| t == &t!("catalog.download").to_string()), "with its Download");
        assert!(offered.iter().any(|t| t == &t!("inventory.group.available").to_string()), "under its own heading");

        let searched = frame(&ctx, Some("julian"));
        assert!(searched.iter().any(|t| t == "JuliaN"), "the search keeps a match: {searched:?}");
        assert!(!searched.iter().any(|t| t == "Linear"), "and narrows the list: {searched:?}");
    }

    /// Where each piece of text a frame drew sits.
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

    /// The info pane beside the list: on opening it says what the current
    /// choice is; pointing at a row shows that row instead (an offered
    /// one with its Download), and it stays while the pointer leaves.
    #[test]
    fn the_picker_shows_what_the_pointer_is_on() {
        use super::*;
        offer();

        let ctx = egui::Context::default();
        // A short list, so every row is on the headless screen.
        let admit = |e: &Entry| e.kind == Kind::Variation && key_name(e).is_some_and(|n| ["julian", "linear", "it_pane_offered"].contains(&n));
        let frame = |pointer: Option<egui::Pos2>| -> Vec<(String, egui::Pos2)> {
            let mut drawn = Vec::new();
            for _ in 0..2 {
                let mut input = egui::RawInput::default();
                if let Some(p) = pointer {
                    input.events.push(egui::Event::PointerMoved(p));
                }
                let out = ctx.run_ui(input, |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        let picker = Picker::new("pane_test", &admit).max_height(100_000.0).current(Some("julian"));
                        let btn = ui.button("open");
                        egui::Popup::open_id(ui.ctx(), picker.popup_id());
                        let _ = picker.popup(ui, &btn);
                    });
                });
                drawn = placed(&out.shapes);
            }
            drawn
        };
        let count = |drawn: &[(String, egui::Pos2)], text: &str| drawn.iter().filter(|(t, _)| t == text).count();

        let opened = frame(None);
        assert_eq!(count(&opened, "JuliaN"), 2, "the current choice's row, and its name heading the pane");

        let row = opened.iter().find(|(t, _)| t.starts_with("Pane Faraway")).expect("the offered row").1;
        let pointed = frame(Some(row + egui::vec2(4.0, 4.0)));
        let heading = pointed.iter().filter(|(t, _)| t == "Pane Faraway").count();
        assert_eq!(heading, 1, "the pane names the row pointed at: {pointed:?}");
        assert_eq!(count(&pointed, &t!("catalog.download")), 2, "with its Download beside the row's");
        assert_eq!(count(&pointed, "JuliaN"), 1, "in place of the current choice");

        let away = frame(Some(egui::pos2(5000.0, 5000.0)));
        assert_eq!(count(&away, "Pane Faraway"), 1, "and keeps it when the pointer leaves the list");
    }
}
