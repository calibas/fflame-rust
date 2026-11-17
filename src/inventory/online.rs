//! What the server holds, as entries (docs/projects/catalog.md, *Phase 3*):
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
use crate::api::types::{FlameListItem, PaletteItem, ScriptListItem};
use crate::provenance::Origin;

/// An item a page held, kept for what its entry does not say: a palette's
/// colours (a click applies them), a fractal's thumbnail and mode.
#[derive(Debug, Clone)]
pub enum Item {
    Flame(FlameListItem),
    Palette(PaletteItem),
    Script(ScriptListItem),
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
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Flame(f) => &f.id,
            Item::Palette(p) => &p.id,
            Item::Script(s) => &s.id,
        }
    }

    /// Its kind; `None` for a script of a kind this build does not know.
    pub fn kind(&self) -> Option<Kind> {
        match self {
            Item::Flame(_) => Some(Kind::Fractal),
            Item::Palette(_) => Some(Kind::Palette),
            Item::Script(s) => script_kind(s.kind.as_deref()?),
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
    };
    let held = match item {
        Item::Palette(p) => local.iter().find(|e| e.key == EntryKey::Palette(p.content_hash.clone())).map(|e| e.key.clone()),
        Item::Script(s) => local.iter().find(|e| e.link.as_ref().and_then(|l| l.cloud_id.as_deref()) == Some(s.id.as_str())).map(|e| e.key.clone()),
        Item::Flame(_) => None,
    };
    Some(Entry {
        kind,
        key: EntryKey::Online(kind, item.id().to_string()),
        title,
        description,
        authors,
        version: None,
        origin: Origin::Api,
        plugin: false,
        group,
        link: None,
        installed: true,
        category: None,
        update: None,
        online: Some(OnlineInfo {
            id: item.id().to_string(),
            owner: owner.unwrap_or_default(),
            updated,
            thumbnail_url,
            visibility: vis,
            held,
        }),
    })
}

/// The sorts a resource takes (docs/projects/catalog-api.md, *The
/// routes*), the first its default; each with its label's locale key.
pub fn sorts(kind: Kind) -> &'static [(&'static str, &'static str)] {
    match kind {
        Kind::Fractal => &[
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

#[cfg(test)]
mod tests {
    use super::*;

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
