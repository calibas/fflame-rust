//! The Catalog's one view (docs/archive/projects/one-catalog-view.md): every
//! catalog drawn by the same rules, the device's entries and the server's
//! pages in ONE list.
//!
//! The filters are **Category** (a catalog's own: a fractal's engine and
//! dimension, a variation's category, an escape-time kind, ...), **Source**
//! (online, this device), **Owner** (curated, yours, somebody else's) and
//! **Type** (only where a catalog has several kinds: fractals and
//! animations). **Group by** is also the order: rows are sorted by name
//! within their group, there is no other sort (D5).
//!
//! An online item the device holds -- a curated copy of a preset or a
//! palette, a script it adopted -- is one row, the device's, which counts
//! as online too.
//!
//! Everything here is the rules, not the drawing: what a catalog offers,
//! which rows the filters admit, how they group, and which online lists
//! to ask for. The panel (`ui::inventory`) draws them.

use crate::api::Scope;
use crate::inventory::online::Item;
use crate::inventory::{Entry, EntryKey, Family, Group, Kind};
use crate::provenance::Origin;
use crate::scene::presets::PresetCategory;

/// Where an entry is: online, on this device, or either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    #[default]
    All,
    Online,
    Device,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::All, Source::Online, Source::Device];

    pub fn label_key(self) -> &'static str {
        match self {
            Source::All => "catalog.filter_all",
            Source::Online => "catalog.source_online",
            Source::Device => "catalog.source_device",
        }
    }
}

/// Whose an entry is: curated (the app's own and the reviewed online
/// collection), yours (your files, your account's items), or somebody
/// else's (what you downloaded or adopted from someone, D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Owner {
    #[default]
    All,
    Curated,
    Yours,
    Others,
}

impl Owner {
    pub const ALL: [Owner; 4] = [Owner::All, Owner::Curated, Owner::Yours, Owner::Others];

    pub fn label_key(self) -> &'static str {
        match self {
            Owner::All => "catalog.filter_all",
            Owner::Curated => "catalog.owner_curated",
            Owner::Yours => "catalog.owner_yours",
            Owner::Others => "catalog.owner_others",
        }
    }
}

/// What the list is grouped by (and so ordered by).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Category,
    Source,
    Owner,
    Type,
    Name,
    /// A palette's pack (D4): the packs' own sections, their checkboxes.
    Pack,
}

impl GroupBy {
    /// What a catalog offers to group by, its default first.
    pub fn offered(family: Family) -> &'static [GroupBy] {
        match family {
            Family::Fractals => &[GroupBy::Category, GroupBy::Source, GroupBy::Owner, GroupBy::Type, GroupBy::Name],
            Family::Palettes => &[GroupBy::Pack, GroupBy::Source, GroupBy::Owner, GroupBy::Name],
            _ => &[GroupBy::Category, GroupBy::Source, GroupBy::Owner, GroupBy::Name],
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            GroupBy::Category => "catalog.group_category",
            GroupBy::Source => "catalog.group_source",
            GroupBy::Owner => "catalog.group_owner",
            GroupBy::Type => "catalog.group_type",
            GroupBy::Name => "catalog.group_name",
            GroupBy::Pack => "catalog.group_pack",
        }
    }
}

/// How a catalog is shown: grid or list, the grid's cell size, the
/// grouping. Each catalog keeps its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub grid: bool,
    pub size: f32,
    pub group: GroupBy,
}

/// The grid's cell sizes, and its default.
pub const SIZES: std::ops::RangeInclusive<f32> = 64.0..=256.0;
pub const DEFAULT_SIZE: f32 = 120.0;

impl View {
    /// A catalog's view before the user changes it: a grid where its
    /// entries are pictures (fractals, escape-time, simulations,
    /// effects), a list where the list was the view (variations) or there
    /// is no grid.
    pub fn default_for(family: Family) -> View {
        let grid = has_grid(family) && !matches!(family, Family::Variations);
        View { grid, size: DEFAULT_SIZE, group: GroupBy::offered(family)[0] }
    }
}

/// Whether a catalog offers a grid: not palettes (a palette's row is its
/// picture) nor scripts (D8: list only).
pub fn has_grid(family: Family) -> bool {
    !matches!(family, Family::Palettes | Family::Scripts)
}

/// What the filters choose.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    /// A category's `order` ([`categories`]); `None` for all.
    pub category: Option<u32>,
    pub source: Source,
    pub owner: Owner,
    /// Only where a catalog has several kinds ([`types`]); `None` for all.
    pub kind: Option<Kind>,
}

// --------------------------------------------------------------- categories

/// The escape-time catalog's categories: its kinds.
const ESCAPE: [Kind; 7] = [Kind::Formula, Kind::Coloring, Kind::Field, Kind::FieldColoring, Kind::Solid, Kind::SolidColoring, Kind::FormulaFile];
const SIMULATION: [Kind; 2] = [Kind::SimModel, Kind::SimColoring];
const SCRIPTS: [Kind; 4] = [Kind::Generator, Kind::Modifier, Kind::ScriptLibrary, Kind::Track];
/// The effects' categories, as the catalogs spell them.
const EFFECT_CATEGORIES: [(&str, &str); 2] = [("color", "inventory.effect_category.color"), ("density", "inventory.effect_category.density")];

use crate::inventory::preset_category_key as preset_key;

/// Variations this flame uses that were downloaded.
///
/// "Is this flame running third-party code" is the question §1 of the
/// shared-resources plan asks, and the answer must not depend on
/// scrolling the list looking for downloads: the Variations catalog says
/// so above its list. Downloads only, as the banner says: a Rhai
/// variation of the user's (`Provenance::Local`) is theirs, and its key
/// would read as noise (rhai-plugins.md, P3).
///
/// A name the registry does not know is NOT reported here — it is
/// missing rather than untrusted, and the fetch path handles it.
pub fn downloaded_variations_in_use(
    flame: &crate::scene::transforms::Flame,
    registry: &crate::variations::VariationRegistry,
) -> Vec<String> {
    flame
        .active_variation_names_ordered(registry)
        .into_iter()
        .filter(|name| registry.get(name).is_some_and(|v| v.provenance.is_cached_download()))
        .collect()
}


/// The API's catalog words for a fractal category (D1:
/// api-catalog-spec.md's `engine` and `dimension`).
pub fn fractal_words(key: &str) -> Vec<(String, String)> {
    let w = |engine: &str, dimension: Option<&str>| {
        let mut v = vec![("engine".to_string(), engine.to_string())];
        if let Some(d) = dimension {
            v.push(("dimension".to_string(), d.to_string()));
        }
        v
    };
    match key {
        "flame2d" => w("flame", Some("2")),
        "flame3d" => w("flame", Some("3")),
        "escape2d" => w("escape", Some("2")),
        "escape3d" => w("escape", Some("3")),
        "simulation" => w("simulation", None),
        _ => Vec::new(),
    }
}

/// A catalog's categories, as groups: their order is what
/// [`Filters::category`] names, their label the select's and the
/// heading's. Empty for a catalog without (palettes).
pub fn categories(family: Family) -> Vec<Group> {
    let of = |labels: Vec<String>| labels.into_iter().enumerate().map(|(i, label)| Group { order: i as u32, label }).collect();
    match family {
        Family::Fractals => of(PresetCategory::ALL.iter().map(|c| t!(c.label_key()).to_string()).collect()),
        Family::Variations => of(crate::variations::VariationCategory::ALL.iter().map(|c| t!(crate::inventory::variation_category_key(*c)).to_string()).collect()),
        Family::Escape => of(ESCAPE.iter().map(|k| t!(k.label_key()).to_string()).collect()),
        Family::Simulation => of(SIMULATION.iter().map(|k| t!(k.label_key()).to_string()).collect()),
        Family::Effects => of(EFFECT_CATEGORIES.iter().map(|(_, l)| t!(*l).to_string()).collect()),
        Family::Scripts => of(SCRIPTS.iter().map(|k| t!(k.label_key()).to_string()).collect()),
        Family::Palettes => Vec::new(),
    }
}

/// The category an entry is in (its order in [`categories`]); `None`
/// when it is in none (it shows under All only).
pub fn category_of(e: &Entry) -> Option<u32> {
    let at = |i: Option<usize>| i.map(|i| i as u32);
    match e.kind.family() {
        Family::Fractals => at(e.category.as_deref().and_then(|c| PresetCategory::ALL.iter().position(|p| preset_key(*p) == c))),
        Family::Variations => {
            use crate::variations::VariationCategory as C;
            let named = e.category.as_deref().and_then(|c| C::ALL.iter().position(|v| format!("{v:?}") == c));
            // A Rhai variation (a file, an online script) is a plug-in.
            at(named.or_else(|| C::ALL.iter().position(|v| *v == C::Plugin)))
        }
        Family::Escape | Family::Simulation => kind_category(e.kind.family(), e.kind),
        Family::Effects => at(e.category.as_deref().and_then(|c| EFFECT_CATEGORIES.iter().position(|(k, _)| k.eq_ignore_ascii_case(c)))),
        Family::Scripts => kind_category(Family::Scripts, e.kind),
        Family::Palettes => None,
    }
}

/// A kind's category, in a catalog whose categories are its kinds.
fn kind_category(family: Family, kind: Kind) -> Option<u32> {
    let kinds: &[Kind] = match family {
        Family::Escape => &ESCAPE,
        Family::Simulation => &SIMULATION,
        Family::Scripts => &SCRIPTS,
        _ => &[],
    };
    kinds.iter().position(|k| *k == kind).map(|i| i as u32)
}

/// A catalog's types, where it has several (D9).
pub fn types(family: Family) -> Vec<Kind> {
    match family {
        Family::Fractals => vec![Kind::Fractal, Kind::Animation],
        _ => Vec::new(),
    }
}

// --------------------------------------------------------------------- rows

/// An online row, with the scope of the list it came from.
pub struct OnlineRow<'a> {
    pub entry: &'a Entry,
    pub item: &'a Item,
    pub scope: Scope,
}

/// One row of the list.
#[derive(Clone, Copy)]
pub struct Row<'a> {
    pub entry: &'a Entry,
    /// An online row's item (its picture, its colours).
    pub item: Option<&'a Item>,
    /// On this device (installed).
    pub device: bool,
    /// Online: an online row, something the server offers to download, or
    /// a device entry the server holds too.
    pub online: bool,
    pub owner: Owner,
    pub category: Option<u32>,
}

/// Whose a device entry is.
pub fn owner_of(e: &Entry) -> Owner {
    match (e.origin, &e.key) {
        (Origin::Builtin, _) => Owner::Curated,
        (Origin::Local, _) => Owner::Yours,
        // Adopted from somebody (a file whose link says so).
        (Origin::Api, EntryKey::File { .. }) => Owner::Others,
        // The server's own catalog (a variation or effect download).
        (Origin::Api, _) => Owner::Curated,
    }
}

/// Whose an online row is: curated, or the scope's.
fn owner_online(e: &Entry, scope: Scope) -> Owner {
    match (e.online.as_ref().is_some_and(|o| o.curated.is_some()), scope) {
        (true, _) | (false, Scope::Curated) => Owner::Curated,
        (false, Scope::Mine) => Owner::Yours,
        (false, Scope::Public) => Owner::Others,
    }
}

/// The rows of `family` the filters and the search admit: the device's
/// entries, then the online rows the device does not hold. Unordered
/// (see [`group`]).
pub fn rows<'a>(family: Family, local: &'a [Entry], online: &[OnlineRow<'a>], f: &Filters, query: &str) -> Vec<Row<'a>> {
    // What the server holds of the device's: its key, and whether the
    // online copy is curated.
    let held: std::collections::HashMap<&EntryKey, bool> =
        online.iter().filter_map(|o| o.entry.online.as_ref().and_then(|i| i.held.as_ref()).map(|k| (k, owner_online(o.entry, o.scope) == Owner::Curated))).collect();
    let admit = |r: &Row| {
        let source = match f.source {
            Source::All => true,
            Source::Online => r.online,
            Source::Device => r.device,
        };
        source
            && (f.owner == Owner::All || r.owner == f.owner)
            && f.category.is_none_or(|c| r.category == Some(c))
            && f.kind.is_none_or(|k| r.entry.kind == k)
    };
    let mut out = Vec::new();
    for e in local {
        // The pickers' curated rows (L3) are the online lists' here.
        if e.kind.family() != family || matches!(e.key, EntryKey::Online(..)) || !e.matches(query) {
            continue;
        }
        let curated_online = held.get(&e.key).copied();
        let owner = if curated_online == Some(true) { Owner::Curated } else { owner_of(e) };
        let r = Row { entry: e, item: None, device: e.installed, online: !e.installed || curated_online.is_some(), owner, category: category_of(e) };
        if admit(&r) {
            out.push(r);
        }
    }
    for o in online {
        // Held: the device's row is it. The search is the server's.
        if o.entry.kind.family() != family || o.entry.online.as_ref().is_some_and(|i| i.held.is_some()) {
            continue;
        }
        let r = Row { entry: o.entry, item: Some(o.item), device: false, online: true, owner: owner_online(o.entry, o.scope), category: category_of(o.entry) };
        if admit(&r) {
            out.push(r);
        }
    }
    out
}

/// The rows in groups, as `by` groups them, each group's rows by name.
/// Group by Name is one group with no heading.
pub fn group<'a>(family: Family, mut rows: Vec<Row<'a>>, by: GroupBy) -> Vec<(Group, Vec<Row<'a>>)> {
    let cats = categories(family);
    let types = types(family);
    let heading = |r: &Row| -> Group {
        match by {
            GroupBy::Name => Group { order: 0, label: String::new() },
            GroupBy::Category => match r.category.and_then(|c| cats.iter().find(|g| g.order == c)) {
                Some(g) => g.clone(),
                None => Group { order: u32::MAX, label: t!("catalog.group_other").to_string() },
            },
            GroupBy::Source => match r.device {
                true => Group { order: 0, label: t!("catalog.source_device").to_string() },
                false => Group { order: 1, label: t!("catalog.source_online").to_string() },
            },
            GroupBy::Owner => {
                let i = Owner::ALL.iter().position(|o| *o == r.owner).unwrap_or(0) as u32;
                Group { order: i, label: t!(r.owner.label_key()).to_string() }
            }
            GroupBy::Type => {
                let i = types.iter().position(|k| *k == r.entry.kind).unwrap_or(types.len()) as u32;
                Group { order: i, label: t!(r.entry.kind.label_key()).to_string() }
            }
            GroupBy::Pack => match r.item {
                // An online palette: a section after the packs.
                Some(_) => Group { order: u32::MAX, label: t!("catalog.source_online").to_string() },
                None => r.entry.group.clone(),
            },
        }
    };
    rows.sort_by_cached_key(|r| {
        let g = heading(r);
        (g.order, g.label, r.entry.title.to_lowercase())
    });
    let mut out: Vec<(Group, Vec<Row<'a>>)> = Vec::new();
    for r in rows {
        let g = heading(&r);
        match out.last_mut() {
            Some((last, members)) if *last == g => members.push(r),
            _ => out.push((g, vec![r])),
        }
    }
    out
}

// ------------------------------------------------------------- online lists

/// One online list to ask for: a kind, a scope, and a fractal category's
/// key (its `engine` / `dimension` words).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnlineList {
    pub kind: Kind,
    pub scope: Scope,
    pub category: Option<&'static str>,
}

/// The online lists the filters need: none for This device; curated, and
/// your own when signed in, as the owner admits (others' public items are
/// not browsed, D2); a catalog's kinds as the category and type admit.
pub fn online_lists(family: Family, f: &Filters, signed_in: bool) -> Vec<OnlineList> {
    if f.source == Source::Device {
        return Vec::new();
    }
    let mut scopes = Vec::new();
    if matches!(f.owner, Owner::All | Owner::Curated) {
        scopes.push(Scope::Curated);
    }
    if signed_in && matches!(f.owner, Owner::All | Owner::Yours) {
        scopes.push(Scope::Mine);
    }
    let category_key = |c: u32| PresetCategory::ALL.get(c as usize).map(|p| preset_key(*p));
    let mut out = Vec::new();
    for kind in super::inventory_online::kinds(family) {
        if f.kind.is_some_and(|k| k != kind) {
            continue;
        }
        let category = match (family, f.category) {
            (_, None) => None,
            // A fractal category is the server's filter; an animation has
            // none, so a category leaves animations out.
            (Family::Fractals, Some(c)) if kind == Kind::Fractal => category_key(c),
            (Family::Fractals, Some(_)) => continue,
            // A catalog whose categories are its kinds: that kind only.
            (Family::Escape | Family::Simulation | Family::Scripts, Some(c)) => {
                if kind_category(family, kind) != Some(c) {
                    continue;
                }
                None
            }
            // Variations and effects: the server cannot filter by these;
            // the rows are filtered as they come.
            (_, Some(_)) => None,
        };
        for scope in &scopes {
            out.push(OnlineList { kind, scope: *scope, category });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: Kind, key: EntryKey, title: &str, origin: Origin) -> Entry {
        Entry {
            kind,
            key,
            title: title.into(),
            description: String::new(),
            authors: Vec::new(),
            version: None,
            origin,
            group: Group { order: 0, label: "Pack".into() },
            link: None,
            installed: true,
            category: None,
            update: None,
            online: None,
        }
    }

    fn online(kind: Kind, id: &str, title: &str, held: Option<EntryKey>, curated: bool) -> Entry {
        let mut e = entry(kind, EntryKey::Online(kind, id.into()), title, Origin::Api);
        e.online = Some(crate::inventory::online::OnlineInfo {
            id: id.into(),
            owner: "ana".into(),
            updated: String::new(),
            thumbnail_url: None,
            visibility: None,
            held,
            curated: curated.then(|| serde_json::from_value(serde_json::json!({"name": id, "current": true})).unwrap()),
        });
        e
    }

    fn item() -> Item {
        Item::Flame(
            serde_json::from_value(serde_json::json!({
                "id": "x", "name": "x", "render_mode": "2d", "transform_count": 1, "variation_names": [], "has_3d": false, "created_at": "", "updated_at": ""
            }))
            .unwrap(),
        )
    }

    /// One list: the device's rows, and the online rows it does not hold;
    /// a held one counts as online too, and as curated when its online
    /// copy is.
    #[test]
    fn device_and_online_rows_are_one_list_without_duplicates() {
        let mut preset = entry(Kind::Fractal, EntryKey::Preset("Reef".into()), "Reef", Origin::Builtin);
        preset.category = Some("flame2d".into());
        let local = vec![preset.clone(), entry(Kind::Palette, EntryKey::Palette("h".into()), "Fire", Origin::Builtin)];
        let curated_copy = online(Kind::Fractal, "f1", "Reef", Some(preset.key.clone()), true);
        let mut yours = online(Kind::Fractal, "f2", "Mine", None, false);
        yours.category = Some("escape3d".into());
        let it = item();
        let on = vec![OnlineRow { entry: &curated_copy, item: &it, scope: Scope::Curated }, OnlineRow { entry: &yours, item: &it, scope: Scope::Mine }];
        let titles = |f: &Filters, q: &str| -> Vec<String> { rows(Family::Fractals, &local, &on, f, q).iter().map(|r| r.entry.title.clone()).collect() };

        let all = Filters::default();
        assert_eq!(titles(&all, ""), vec!["Reef", "Mine"], "the curated copy is the preset's row; the palette is another catalog's");
        assert_eq!(titles(&Filters { source: Source::Online, ..Default::default() }, ""), vec!["Reef", "Mine"], "a held preset is online too");
        assert_eq!(titles(&Filters { source: Source::Device, ..Default::default() }, ""), vec!["Reef"]);
        assert_eq!(titles(&Filters { owner: Owner::Yours, ..Default::default() }, ""), vec!["Mine"]);
        assert_eq!(titles(&Filters { owner: Owner::Curated, ..Default::default() }, ""), vec!["Reef"]);
        assert!(titles(&Filters { owner: Owner::Others, ..Default::default() }, "").is_empty());
        assert_eq!(titles(&Filters { category: Some(3), ..Default::default() }, ""), vec!["Mine"], "3D Escape");
        assert_eq!(titles(&Filters { kind: Some(Kind::Animation), ..Default::default() }, ""), Vec::<String>::new());
        assert_eq!(titles(&all, "ree"), vec!["Reef", "Mine"], "the device's rows are searched here; the server searched its own");
        assert_eq!(titles(&all, "zzz"), vec!["Mine"]);
    }

    /// Owner on the device: built-in and the server's own catalog are
    /// curated; your files yours; a file adopted from someone theirs.
    #[test]
    fn whose_an_entry_is() {
        let e = |key: EntryKey, origin| entry(Kind::Formula, key, "x", origin);
        let file = || EntryKey::File { store: crate::storage::library::Store::Formulas, file: "x.rhai".into(), shipped: false };
        assert_eq!(owner_of(&e(EntryKey::Registered(Kind::Formula, "m".into()), Origin::Builtin)), Owner::Curated);
        assert_eq!(owner_of(&e(EntryKey::Registered(Kind::Variation, "v".into()), Origin::Api)), Owner::Curated, "a download from the server's catalog");
        assert_eq!(owner_of(&e(file(), Origin::Local)), Owner::Yours);
        assert_eq!(owner_of(&e(file(), Origin::Api)), Owner::Others);
    }

    /// Grouped by category (in the categories' order, the uncategorised
    /// last), by source, or by name (one group, A-Z); each group by name.
    #[test]
    fn rows_group_and_sort() {
        let mk = |title: &str, cat: Option<&str>| {
            let mut e = entry(Kind::Fractal, EntryKey::Preset(title.into()), title, Origin::Builtin);
            e.category = cat.map(str::to_string);
            e
        };
        let local = vec![mk("b", Some("flame3d")), mk("a", Some("flame3d")), mk("c", Some("flame2d")), mk("d", None)];
        let all = rows(Family::Fractals, &local, &[], &Filters::default(), "");
        let by_cat: Vec<(u32, Vec<String>)> = group(Family::Fractals, all.clone(), GroupBy::Category).into_iter().map(|(g, r)| (g.order, r.iter().map(|r| r.entry.title.clone()).collect())).collect();
        assert_eq!(by_cat, vec![(0, vec!["c".to_string()]), (1, vec!["a".into(), "b".into()]), (u32::MAX, vec!["d".into()])]);
        let by_name = group(Family::Fractals, all, GroupBy::Name);
        assert_eq!(by_name.len(), 1);
        assert!(by_name[0].0.label.is_empty(), "no heading");
        assert_eq!(by_name[0].1.iter().map(|r| r.entry.title.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c", "d"]);
    }

    /// The online lists: none for This device; curated, plus your own
    /// signed in; a fractal category asks by the API's engine and
    /// dimension and leaves animations out; an escape category is its kind.
    #[test]
    fn the_online_lists_the_filters_need() {
        let f = Filters::default();
        let lists = online_lists(Family::Fractals, &f, false);
        assert!(lists.iter().all(|l| l.scope == Scope::Curated) && lists.iter().any(|l| l.kind == Kind::Animation));
        assert!(online_lists(Family::Fractals, &f, true).iter().any(|l| l.scope == Scope::Mine));
        assert!(online_lists(Family::Fractals, &Filters { source: Source::Device, ..Default::default() }, true).is_empty());
        assert!(online_lists(Family::Fractals, &Filters { owner: Owner::Others, ..Default::default() }, true).is_empty(), "others' items are not browsed");
        let escape3d = online_lists(Family::Fractals, &Filters { category: Some(3), ..Default::default() }, false);
        assert_eq!(escape3d, vec![OnlineList { kind: Kind::Fractal, scope: Scope::Curated, category: Some("escape3d") }]);
        assert_eq!(fractal_words("escape3d"), vec![("engine".to_string(), "escape".to_string()), ("dimension".to_string(), "3".to_string())]);
        let colorings = online_lists(Family::Escape, &Filters { category: Some(1), ..Default::default() }, false);
        assert!(!colorings.is_empty() && colorings.iter().all(|l| l.kind == Kind::Coloring), "{colorings:?}");
    }

    /// Every catalog's category of each of its entries is one it offers.
    #[test]
    fn entries_fall_in_their_catalogs_categories() {
        let entries = crate::inventory::scan(&crate::scene::palette::global_palette_library().read().unwrap().clone(), crate::inventory::Online::default());
        for family in Family::ALL {
            let n = categories(family).len() as u32;
            for e in entries.iter().filter(|e| e.kind.family() == family) {
                if let Some(c) = category_of(e) {
                    assert!(c < n, "{family:?} {}: {c}", e.title);
                }
            }
        }
        let presets = entries.iter().filter(|e| matches!(e.key, EntryKey::Preset(_))).collect::<Vec<_>>();
        assert!(!presets.is_empty() && presets.iter().all(|e| category_of(e).is_some()), "every preset has its category");
        let variations = entries.iter().filter(|e| e.kind == Kind::Variation && matches!(e.key, EntryKey::Registered(..))).collect::<Vec<_>>();
        assert!(variations.iter().all(|e| category_of(e).is_some()), "every variation has its category");
    }

    use crate::scene::transforms::{Flame, Transform};

    fn api_download(name: &str) -> crate::api::types::VariationDownload {
        crate::api::types::VariationDownload {
            id: name.into(), name: name.into(), display_name: name.into(),
            description: None, category: "advanced_2d".into(), version: 3,
            phase: crate::api::types::ApiVariationPhase::Normal,
            needs_rng: false, needs_transform: false, writes_color: false,
            parameters: Vec::new(),
            shader_2d: Some("fn variation_x(p: vec2<f32>) -> vec2<f32> { return p; }".into()),
            shader_3d: None, init_param_count: 0, shader_init: None,
            features: Vec::new(), state_count: 0, shader_state_init: None,
            aliases: Vec::new(), plot_emits: 0, authors: Vec::new(),
            description_plain: None,
        }
    }

    /// A flame using a downloaded variation must say so; one using only
    /// built-ins must not cry wolf.
    #[test]
    fn a_flame_reports_the_third_party_code_it_runs() {
        let mut registry = crate::variations::VariationRegistry::new();
        let _ = registry.register_from_api(&api_download("borrowed_thing"), crate::provenance::Provenance::Api { version: 3 });

        // Built-ins only — nothing to warn about.
        let mut flame = Flame::default();
        let mut t = Transform::new();
        t.set_variation("linear", 1.0);
        flame.transforms.push(t);
        assert!(downloaded_variations_in_use(&flame, &registry).is_empty());

        // Add a downloaded one.
        let mut t2 = Transform::new();
        t2.set_variation("borrowed_thing", 1.0);
        flame.transforms.push(t2);
        assert_eq!(
            downloaded_variations_in_use(&flame, &registry),
            vec!["borrowed_thing".to_string()]
        );

        // One of the user's own Rhai variations is not a download.
        let _ = registry.register_from_api(&api_download("mine_in_rhai"), crate::provenance::Provenance::Local);
        let mut t3 = Transform::new();
        t3.set_variation("mine_in_rhai", 1.0);
        flame.transforms.push(t3);
        assert_eq!(downloaded_variations_in_use(&flame, &registry), vec!["borrowed_thing".to_string()]);
    }

    /// An unknown name is missing, not untrusted — the fetch path deals
    /// with it, and reporting it as third-party code would be a lie.
    #[test]
    fn an_unknown_variation_is_not_reported_as_downloaded() {
        let registry = crate::variations::VariationRegistry::new();
        let mut flame = Flame::default();
        let mut t = Transform::new();
        t.set_variation("never_heard_of_it", 1.0);
        flame.transforms.push(t);
        assert!(downloaded_variations_in_use(&flame, &registry).is_empty());
    }
}
