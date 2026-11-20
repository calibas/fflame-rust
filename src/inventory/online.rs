//! What the server holds, as entries (docs/archive/projects/catalog.md, *Phase 3*):
//! the Catalog's **Online** place lists a family's public items -- or the
//! caller's own -- page by page, and each item read from a page becomes an
//! [`Entry`] keyed [`EntryKey::Online`], drawn and selected as every row.
//!
//! Variations and effects are not here: the cached catalog is their whole
//! list, and the local list already offers its downloads
//! (`sources::available`). Fetching, paging and searching are the UI's
//! (`ui::inventory_online`); this is the pure part, testable without a
//! network.

use super::{Entry, EntryKey, Group, Kind};
use crate::api::types::{AnimationListItem, Curation, FlameListItem, PaletteItem, ScriptListItem};
use crate::provenance::Origin;

/// An item a page held, kept for what its entry does not say: a palette's
/// colours (a click applies them), a fractal's thumbnail and mode.
#[derive(Debug, Clone)]
pub enum Item {
    Flame(FlameListItem),
    Palette(PaletteItem),
    Script(ScriptListItem),
    Animation(AnimationListItem),
}

/// What an online row says beyond an entry's fields.
#[derive(Debug, Clone, PartialEq)]
pub struct OnlineInfo {
    /// The server's id.
    pub id: String,
    /// The owner's display name; empty for a built-in script.
    pub owner: String,
    /// When it last changed (the server's time, RFC 3339).
    pub updated: String,
    /// Its thumbnail on the API, used as given.
    pub thumbnail_url: Option<String>,
    /// `public`, `unlisted` or `private`, when the server says.
    pub visibility: Option<String>,
    /// What the app already holds of it: an adopted script's file, a
    /// palette of the same colours.
    pub held: Option<EntryKey>,
    /// Curated: its canonical name, version and credit.
    pub curated: Option<Curation>,
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Flame(f) => &f.id,
            Item::Palette(p) => &p.id,
            Item::Script(s) => &s.id,
            Item::Animation(a) => &a.id,
        }
    }

    /// Its curation, when curated.
    pub fn curated(&self) -> Option<&Curation> {
        match self {
            Item::Flame(f) => f.curated.as_ref(),
            Item::Palette(p) => p.curated.as_ref(),
            Item::Script(s) => s.curated.as_ref(),
            Item::Animation(a) => a.curated.as_ref(),
        }
    }

    /// Its kind; `None` for a script of a kind this build does not know.
    pub fn kind(&self) -> Option<Kind> {
        match self {
            Item::Flame(_) => Some(Kind::Fractal),
            Item::Palette(_) => Some(Kind::Palette),
            Item::Script(s) => script_kind(s.kind.as_deref()?),
            Item::Animation(_) => Some(Kind::Animation),
        }
    }
}

/// The Catalog's kind of a script the server names `wire`.
pub fn script_kind(wire: &str) -> Option<Kind> {
    crate::storage::library::Kind::from_wire(wire).and_then(Kind::of_file)
}

/// The server's word for a kind the scripts resource stores; `None` for
/// one it does not (palettes, fractals, a UF file).
pub fn script_wire(kind: Kind) -> Option<&'static str> {
    crate::storage::library::ALL_PUBLISHED.iter().find(|k| Kind::of_file(**k) == Some(kind)).and_then(|k| k.wire())
}

fn visibility(v: &Option<crate::api::types::ApiVisibility>) -> Option<String> {
    v.as_ref().and_then(|v| serde_json::to_value(v).ok()).and_then(|v| v.as_str().map(str::to_string))
}

/// An item's entry, under `group`; `local` is the list on this device,
/// for what the app already holds of it.
pub fn entry(item: &Item, group: Group, local: &[Entry]) -> Option<Entry> {
    let kind = item.kind()?;
    let (title, description, authors, owner, updated, thumbnail_url, vis) = match item {
        Item::Flame(f) => (
            f.name.clone(),
            f.description.clone().unwrap_or_default(),
            Vec::new(),
            f.owner.as_ref().map(|o| o.display_name.clone()),
            f.updated_at.clone(),
            f.thumbnail_url.clone(),
            visibility(&f.visibility),
        ),
        Item::Palette(p) => (
            p.name.clone(),
            p.description.clone().unwrap_or_default(),
            Vec::new(),
            p.owner.as_ref().map(|o| o.display_name.clone()),
            p.updated_at.clone().unwrap_or_default(),
            None,
            visibility(&p.visibility),
        ),
        Item::Script(s) => (
            if s.display_name.is_empty() { s.name.clone() } else { s.display_name.clone() },
            s.description.clone().unwrap_or_default(),
            s.authors.clone(),
            s.owner.as_ref().map(|o| o.display_name.clone()),
            s.updated_at.clone().unwrap_or_default(),
            None,
            visibility(&s.visibility),
        ),
        Item::Animation(a) => (
            a.name.clone(),
            a.description.clone().unwrap_or_default(),
            Vec::new(),
            a.owner.as_ref().map(|o| o.display_name.clone()),
            a.updated_at.clone(),
            None,
            visibility(&Some(a.visibility.clone())),
        ),
    };
    // A curated item credits whose work it is.
    let authors = match item.curated().and_then(|c| c.credit.clone()) {
        Some(credit) if authors.is_empty() => vec![credit],
        _ => authors,
    };
    let held = match item {
        Item::Palette(p) => local.iter().find(|e| e.key == EntryKey::Palette(p.content_hash.clone())).map(|e| e.key.clone()),
        Item::Script(s) => local.iter().find(|e| e.link.as_ref().and_then(|l| l.cloud_id.as_deref()) == Some(s.id.as_str())).map(|e| e.key.clone()),
        // A curated fractal is a preset's online copy (curated-content.md:
        // every preset is curated under its name).
        Item::Flame(f) if f.curated.is_some() => local.iter().find(|e| matches!(e.key, EntryKey::Preset(_)) && e.title == f.name).map(|e| e.key.clone()),
        Item::Flame(_) | Item::Animation(_) => None,
    };
    let category = match item {
        Item::Flame(f) => fractal_category(f.engine.as_deref(), f.dimension, &f.render_mode).map(str::to_string),
        _ => None,
    };
    Some(Entry {
        kind,
        key: EntryKey::Online(kind, item.id().to_string()),
        title,
        description,
        authors,
        version: None,
        origin: Origin::Api,
        group,
        link: None,
        installed: true,
        category,
        update: None,
        online: Some(OnlineInfo {
            id: item.id().to_string(),
            owner: owner.unwrap_or_default(),
            updated,
            thumbnail_url,
            visibility: vis,
            held,
            curated: item.curated().cloned(),
        }),
    })
}

/// The fractal category an online fractal is in ([`super::preset_category_key`]),
/// by what the server says of it: its engine and dimension
/// (api-catalog-spec.md), or, on an item the server has not summarised,
/// its render mode where that settles it (an escape-time one's dimension
/// it does not).
pub fn fractal_category(engine: Option<&str>, dimension: Option<i16>, render_mode: &crate::api::types::ApiRenderMode) -> Option<&'static str> {
    use crate::api::types::ApiRenderMode as M;
    match (engine, dimension) {
        (Some("flame"), Some(2)) => Some("flame2d"),
        (Some("flame"), Some(3)) => Some("flame3d"),
        (Some("escape"), Some(2)) => Some("escape2d"),
        (Some("escape"), Some(3)) => Some("escape3d"),
        (Some("simulation"), _) => Some("simulation"),
        _ => match render_mode {
            M::TwoD => Some("flame2d"),
            M::ThreeD => Some("flame3d"),
            M::Simulation => Some("simulation"),
            M::Escape | M::Unknown(_) => None,
        },
    }
}

/// The sorts a resource takes (docs/projects/catalog-api.md, *The
/// routes*), the first its default; each with its label's locale key.
pub fn sorts(kind: Kind) -> &'static [(&'static str, &'static str)] {
    match kind {
        Kind::Fractal | Kind::Animation => &[
            ("recent", "catalog.sort_recent"),
            ("popular", "catalog.sort_popular"),
            ("views", "catalog.sort_views"),
            ("updated", "catalog.sort_updated"),
            ("name", "catalog.sort_name"),
        ],
        Kind::Palette => &[
            ("recent", "catalog.sort_recent"),
            ("popular", "catalog.sort_popular"),
            ("updated", "catalog.sort_updated"),
            ("name", "catalog.sort_name"),
        ],
        _ => &[("recent", "catalog.sort_recent"), ("updated", "catalog.sort_updated"), ("name", "catalog.sort_name")],
    }
}

/// What the Online place's *Open by ID or link* names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTarget {
    /// An item by its id; `resource` (`flames`, `animations`, `palettes`,
    /// `scripts`) when the text says which.
    Id { resource: Option<&'static str>, id: String },
    /// A canonical name, written `kind/name` (`fractal/sierpinski`).
    Name { kind: String, name: String },
}

/// The first UUID in `text` (any case), where it starts.
fn find_uuid(text: &str) -> Option<(usize, String)> {
    let lower = text.to_ascii_lowercase();
    let b = lower.as_bytes();
    (0..b.len().saturating_sub(35)).find_map(|i| {
        let s = &lower[i..i + 36];
        let edge_ok = |j: usize| j >= b.len() || !b[j].is_ascii_hexdigit();
        (crate::scripted::is_uuid(s) && (i == 0 || !b[i - 1].is_ascii_hexdigit()) && edge_ok(i + 36)).then(|| (i, s.to_string()))
    })
}

/// Read what a person pastes to open: an id, a link that holds one (the
/// web app's `?flame=` / `?animation=`, an item's API path, a page of the
/// browser app's), or a canonical name `kind/name`. `None` for anything
/// else.
pub fn parse_open(text: &str) -> Option<OpenTarget> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some((at, id)) = find_uuid(text) {
        // The resource the text names last before the id.
        let before = text[..at].to_ascii_lowercase();
        let hints: [(&str, &'static str); 5] =
            [("animation", "animations"), ("flame", "flames"), ("fractal", "flames"), ("palette", "palettes"), ("script", "scripts")];
        let resource = hints.iter().filter_map(|(word, r)| before.rfind(word).map(|i| (i, *r))).max_by_key(|(i, _)| *i).map(|(_, r)| r);
        return Some(OpenTarget::Id { resource, id });
    }
    let (kind, name) = text.split_once('/')?;
    let (kind, name) = (kind.trim().to_ascii_lowercase(), name.trim());
    let known = matches!(kind.as_str(), "fractal" | "palette" | "animation" | "variation" | "effect")
        || crate::storage::library::Kind::from_wire(&kind).is_some();
    let name_ok = !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    (known && name_ok).then(|| OpenTarget::Name { kind, name: name.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A curated item says so, and credits whose work it is.
    #[test]
    fn a_curated_item_reads_its_curation() {
        let item: crate::api::types::FlameListItem = serde_json::from_value(serde_json::json!({
            "id": "f1", "name": "Sunrise", "render_mode": "2d", "transform_count": 2, "variation_names": [],
            "has_3d": false, "created_at": "", "updated_at": "", "origin": "user",
            "owner": {"id": "a", "display_name": "curator"},
            "curated": {"name": "sunrise", "current": true, "content_hash": "abc", "credit": "Ana"}
        }))
        .unwrap();
        let e = entry(&Item::Flame(item), Group { order: 0, label: String::new() }, &[]).unwrap();
        let info = e.online.as_ref().unwrap();
        assert_eq!(info.curated.as_ref().and_then(|c| c.name.as_deref()), Some("sunrise"));
        assert_eq!(e.authors, vec!["Ana".to_string()], "the credit, as its authors");
        let anim: crate::api::types::AnimationListItem = serde_json::from_value(serde_json::json!({
            "id": "a1", "name": "Dance", "duration": 10.0, "loop_mode": "loop", "visibility": "public",
            "flame_id": null, "created_at": "", "updated_at": ""
        }))
        .unwrap();
        let e = entry(&Item::Animation(anim), Group { order: 0, label: String::new() }, &[]).unwrap();
        assert_eq!(e.kind, Kind::Animation);
        assert_eq!(e.kind.family(), crate::inventory::Family::Fractals);
        assert!(e.online.unwrap().curated.is_none());
    }

    #[test]
    fn what_a_person_pastes_opens() {
        let id = "5b0f8c1e-2d3a-4b5c-8d9e-0f1a2b3c4d5e";
        let at = |resource: Option<&'static str>| Some(OpenTarget::Id { resource, id: id.to_string() });
        assert_eq!(parse_open(id), at(None));
        assert_eq!(parse_open(&format!("  {} ", id.to_uppercase())), at(None));
        assert_eq!(parse_open(&format!("https://fractalsforall.com/app/?flame={id}")), at(Some("flames")));
        assert_eq!(parse_open(&format!("https://fractalsforall.com/app/?animation={id}")), at(Some("animations")));
        assert_eq!(parse_open(&format!("https://fractalsforall.com/api/v1/palettes/{id}")), at(Some("palettes")));
        assert_eq!(parse_open(&format!("/api/v1/scripts/{id}?reader=7")), at(Some("scripts")));
        // The last resource named before the id wins (a flame's animation).
        assert_eq!(parse_open(&format!("/flames/x/animations/{id}")), at(Some("animations")));
        assert_eq!(
            parse_open("fractal/sierpinski-carpet"),
            Some(OpenTarget::Name { kind: "fractal".into(), name: "sierpinski-carpet".into() })
        );
        assert_eq!(parse_open("Formula/Mandelbrot"), Some(OpenTarget::Name { kind: "formula".into(), name: "Mandelbrot".into() }));
        for nothing in ["", "hello", "room/x", "fractal/", "fractal/has space", "5b0f8c1e-2d3a", "https://example.com/a/b"] {
            assert_eq!(parse_open(nothing), None, "{nothing}");
        }
    }

    fn script(id: &str, kind: &str) -> Item {
        Item::Script(
            serde_json::from_value(serde_json::json!({
                "id": id, "name": "orbit_trap", "display_name": "Orbit Trap", "kind": kind,
                "owner": {"id": "u1", "display_name": "ana"}, "authors": ["Ana"],
                "description": "Traps *orbits*.", "updated_at": "2026-11-16T10:00:00Z"
            }))
            .unwrap(),
        )
    }

    /// An online item reads as a row: its kind from the server's word, its
    /// owner and time, keyed by its id; a script the user adopted is held.
    #[test]
    fn an_online_item_is_an_entry() {
        let group = Group { order: 0, label: "Coloring".into() };
        let e = entry(&script("s1", "coloring"), group.clone(), &[]).unwrap();
        assert_eq!(e.kind, Kind::Coloring);
        assert_eq!(e.key, EntryKey::Online(Kind::Coloring, "s1".into()));
        assert_eq!((e.title.as_str(), e.origin, e.authors.as_slice()), ("Orbit Trap", Origin::Api, &["Ana".to_string()][..]));
        let info = e.online.as_ref().unwrap();
        assert_eq!((info.owner.as_str(), info.updated.as_str(), info.held.as_ref()), ("ana", "2026-11-16T10:00:00Z", None));

        let mut adopted = e.clone();
        adopted.key = EntryKey::File { store: crate::storage::library::Store::Formulas, file: "orbit_trap.rhai".into(), shipped: false };
        adopted.online = None;
        adopted.link = Some(crate::storage::links::Link { cloud_id: Some("s1".into()), ..Default::default() });
        let e = entry(&script("s1", "coloring"), group.clone(), &[adopted.clone()]).unwrap();
        assert_eq!(e.online.unwrap().held, Some(adopted.key), "the adopted file is what the app holds of it");

        assert!(entry(&script("s2", "a_kind_from_the_future"), group, &[]).is_none(), "a kind this build cannot use is left out");
    }

    /// The server's script words and the Catalog's kinds agree both ways.
    #[test]
    fn script_kinds_round_trip() {
        for k in crate::storage::library::ALL_PUBLISHED {
            let kind = Kind::of_file(*k).unwrap();
            assert_eq!(script_wire(kind), k.wire(), "{kind:?}");
            assert_eq!(script_kind(k.wire().unwrap()), Some(kind));
        }
        assert_eq!(script_wire(Kind::Palette), None);
    }
}
