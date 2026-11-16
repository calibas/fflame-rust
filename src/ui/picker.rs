//! The Catalog's list as a picker (docs/projects/catalog.md, *The picker*),
//! in a popup under a button:
//! the entries of [`super::inventory::shared`] a filter admits, in the
//! Catalog's groups, with a search, each entry's description as its
//! tooltip, and **Download** for what the server offers and the app lacks
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
    min_width: f32,
    max_height: f32,
}

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
                ui.set_min_width(self.min_width);
                let focus = ui.data_mut(|d| d.remove_temp::<bool>(focus_id).unwrap_or(false));
                picked = self.body(ui, focus);
                if picked.is_some() {
                    egui::Popup::close_id(ui.ctx(), popup_id);
                }
            });
        picked
    }

    fn body(self, ui: &mut egui::Ui, focus: bool) -> Option<Entry> {
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
                    if let Some(p) = self.row(ui, e) {
                        picked = Some(p);
                    }
                }
                ui.add_space(4.0);
                start = end;
            }
        });
        picked
    }

    /// One entry: selectable when it can be picked; greyed with the reason
    /// when not; Download when it is not installed.
    fn row(&self, ui: &mut egui::Ui, e: &Entry) -> Option<Entry> {
        let tip = (!e.description.is_empty()).then(|| crate::markdown::first_paragraph(&e.description));
        if !e.installed {
            ui.horizontal(|ui| {
                let r = ui.weak(super::inventory::row_label(e));
                if let Some(t) = &tip {
                    r.on_hover_text(t);
                }
                super::inventory::download_button(ui, e);
            });
            return None;
        }
        let current = self.current.is_some_and(|c| super::inventory::applied_name(e).as_deref() == Some(c) || key_name(e) == Some(c));
        if let Some(reason) = self.unusable.and_then(|why| why(e)) {
            ui.add_enabled(false, egui::Button::selectable(current, super::inventory::row_label(e))).on_disabled_hover_text(reason);
            return None;
        }
        let r = ui.selectable_label(current, super::inventory::row_label(e));
        let r = match &tip {
            Some(t) => r.on_hover_text(t),
            None => r,
        };
        r.clicked().then(|| e.clone())
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
        let src = include_str!("picker.rs");
        let start = src.find("pub fn popup(").expect("the popup moved");
        let end = start + src[start..].find("\n    }\n").expect("its end");
        let body: String = src[start..end].lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
        assert!(!body.contains("ComboBox"), "the picker is a ComboBox again");
        assert!(body.contains("PopupCloseBehavior::CloseOnClickOutside"), "it must close only on a click outside");
        assert!(body.contains("Popup::close_id"), "a pick must close it explicitly");
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
        use crate::inventory::Online;
        let offered: crate::api::types::VariationListItem = serde_json::from_value(serde_json::json!({
            "id": "it_picker_offered", "name": "it_picker_offered", "display_name": "Offered Afar",
            "category": "Plugin", "version": 1, "downloadable": true
        }))
        .unwrap();
        let offered = [offered];
        let lib = crate::scene::palette::global_palette_library().read().unwrap().clone();
        super::super::inventory::InventoryPanel::new().refresh(&lib, 0, Online { variations: &offered, effects: &[] });

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
}
