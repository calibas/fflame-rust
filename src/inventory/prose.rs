//! What the app ships to say about its built-ins, read once from where
//! it is written:
//!
//! - **Variations**: each twin opens with its built-in's doc block
//!   (`variations::twins::every_variation_twin_keeps_its_builtins_doc`),
//!   so the prose and authors of 640 of the 647 are in the app. The rest,
//!   and downloads, take the cached server catalog's.
//! - **Escape kinds, effects, simulation kinds**: the twin's `INFO`
//!   (`description`, `author`), else the doc block it opens with
//!   ([`crate::lang::rhai::source_labels`]).
//!
//! Parsed on first use and kept: the sources are compiled in.

use super::Kind;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A description and its authors.
#[derive(Debug, Clone, Default)]
pub(super) struct Prose {
    pub description: String,
    pub authors: Vec<String>,
}

/// A built-in variation's prose, from its twin's doc block.
pub(super) fn variation(name: &str) -> Option<&'static Prose> {
    static MAP: OnceLock<HashMap<&'static str, Prose>> = OnceLock::new();
    MAP.get_or_init(|| {
        crate::variations::twins::TWINS
            .iter()
            .filter_map(|(name, src)| {
                let doc = crate::variations::docs::leading_block(src)?;
                Some((*name, Prose { description: doc.description_plain, authors: doc.authors }))
            })
            .collect()
    })
    .get(name)
}

/// A built-in's prose from its Rhai twin, for the kinds that have them.
pub(super) fn twin(kind: Kind, name: &str) -> Option<&'static Prose> {
    static MAP: OnceLock<HashMap<Kind, HashMap<&'static str, Prose>>> = OnceLock::new();
    MAP.get_or_init(|| {
        let tables: [(Kind, &[(&'static str, &'static str)]); 9] = [
            (Kind::Formula, crate::escape::twins::TWINS),
            (Kind::Coloring, crate::escape::twins::COLORING_TWINS),
            (Kind::Solid, crate::escape::twins::SOLID_TWINS),
            (Kind::Field, crate::escape::twins::FIELD_TWINS),
            (Kind::FieldColoring, crate::escape::twins::FIELD_COLORING_TWINS),
            (Kind::Effect, crate::effects::custom::BUILTIN_TWINS),
            (Kind::Effect, crate::effects::custom::RHAI_BUILTINS),
            (Kind::SimColoring, crate::sim::custom::SHIPPED),
            (Kind::SimModel, crate::sim::custom::SHIPPED_MODELS),
        ];
        let mut map: HashMap<Kind, HashMap<&'static str, Prose>> = HashMap::new();
        for (kind, table) in tables {
            for (name, src) in table {
                if let Some(labels) = crate::lang::rhai::source_labels(src) {
                    let prose = Prose { description: labels.description.resolve(), authors: authors(&labels.author) };
                    if !prose.description.is_empty() || !prose.authors.is_empty() {
                        map.entry(kind).or_default().insert(*name, prose);
                    }
                }
            }
        }
        map
    })
    .get(&kind)?
    .get(name)
}

/// The prose of a built-in with no twin to carry it (the flame attractors
/// and the solids' colourings): the locale's, by kind and name.
pub(super) fn untwinned(kind: Kind, name: &str) -> Option<Prose> {
    let kind = match kind {
        Kind::Solid => "solid",
        Kind::SolidColoring => "solid_coloring",
        _ => return None,
    };
    let key = format!("inventory.descriptions.{kind}.{name}");
    let text = t!(&key).to_string();
    (text != key).then(|| Prose { description: text, authors: Vec::new() })
}

/// A server catalog's prose for a variation: what a download, and a
/// built-in without a twin, have.
pub(super) fn variation_catalog() -> HashMap<String, Prose> {
    crate::storage::variation_catalog::load()
        .map(|c| {
            c.items
                .into_iter()
                .map(|i| (i.name, Prose { description: i.description_plain.unwrap_or_default(), authors: i.authors }))
                .collect()
        })
        .unwrap_or_default()
}

/// The same for effects.
pub(super) fn effect_catalog() -> HashMap<String, Prose> {
    crate::storage::effect_catalog::load()
        .map(|c| {
            c.items
                .into_iter()
                .map(|i| (i.name, Prose { description: i.description_plain.unwrap_or_default(), authors: i.authors }))
                .collect()
        })
        .unwrap_or_default()
}

/// `INFO.author` as the list an entry shows: whole, as written -- not
/// split at commas, which a single name may hold (a doc block's
/// `# Authors` arrive joined by `", "`, `scripted::Labels::author`).
pub(super) fn authors(written: &str) -> Vec<String> {
    let a = written.trim();
    if a.is_empty() {
        Vec::new()
    } else {
        vec![a.to_string()]
    }
}
