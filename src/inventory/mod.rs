//! Every resource the app knows, as one list: the model behind the
//! Catalog panel (docs/projects/catalog.md). The panel's name is the
//! user's ("Catalog"); the code's is `inventory`, because `catalog` is
//! the engine catalog in both repositories (`crate::catalog`,
//! `engine-catalog.json`), `library` the Rhai file scan
//! ([`crate::storage::library`]) and `resources` the fetching
//! ([`crate::resources`]).
//!
//! An [`Entry`] is what a person reads of a resource -- its kind, title,
//! description, authors, version, where it came from -- under a stable
//! [`EntryKey`]. [`scan`] builds the list from what exists: the palette
//! and preset libraries, the variation and effect registries, the escape
//! and simulation registries, and the user's and shipped Rhai files
//! ([`crate::storage::library::scan`]). Nothing is copied that a source
//! keeps: a palette's stops, a preset's config and a file's text stay in
//! their sources, found again by the key ([`palette`], [`preset`]).
//!
//! Prose comes from where it is written: a variation's doc block (each
//! twin opens with its built-in's, `variations::twins`), a built-in's Rhai
//! twin's `INFO` (escape kinds, effects, simulation colourings and
//! models), a simulation def's description, a file's `INFO`. Nothing is
//! attributed that its source does not say.

pub mod online;
mod prose;
mod sources;

use crate::provenance::Origin;
use crate::storage::library::Store;
use crate::storage::links::Link;

pub use sources::{palette, preset};

/// What a resource is: the Catalog's kinds, each one list of things a
/// person chooses from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Palette,
    Fractal,
    Variation,
    Effect,
    /// An escape-time formula (mode A).
    Formula,
    /// An escape-time colouring.
    Coloring,
    /// A mode B field, and a field's colouring.
    Field,
    FieldColoring,
    /// A mode D solid (a distance function, or a 2D IFS's attractor), and
    /// a solid's colouring.
    Solid,
    SolidColoring,
    SimModel,
    SimColoring,
    /// The CPU scripts.
    Generator,
    Modifier,
    ScriptLibrary,
    /// An animation track.
    Track,
    /// An Ultra Fractal or Fractint file: many entries, of its kind.
    FormulaFile,
}

/// The Catalog's top-level groups of kinds, in the order it offers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Palettes,
    Fractals,
    Variations,
    Effects,
    Escape,
    Simulation,
    Scripts,
}

impl Family {
    pub const ALL: [Family; 7] =
        [Family::Palettes, Family::Fractals, Family::Variations, Family::Effects, Family::Escape, Family::Simulation, Family::Scripts];

    /// Its label's locale key.
    pub fn label_key(self) -> &'static str {
        match self {
            Family::Palettes => "inventory.family.palettes",
            Family::Fractals => "inventory.family.fractals",
            Family::Variations => "inventory.family.variations",
            Family::Effects => "inventory.family.effects",
            Family::Escape => "inventory.family.escape",
            Family::Simulation => "inventory.family.simulation",
            Family::Scripts => "inventory.family.scripts",
        }
    }
}

impl Kind {
    pub fn family(self) -> Family {
        match self {
            Kind::Palette => Family::Palettes,
            Kind::Fractal => Family::Fractals,
            Kind::Variation => Family::Variations,
            Kind::Effect => Family::Effects,
            Kind::Formula | Kind::Coloring | Kind::Field | Kind::FieldColoring | Kind::Solid | Kind::SolidColoring | Kind::FormulaFile => {
                Family::Escape
            }
            Kind::SimModel | Kind::SimColoring => Family::Simulation,
            Kind::Generator | Kind::Modifier | Kind::ScriptLibrary | Kind::Track => Family::Scripts,
        }
    }

    /// Its label's locale key (singular, as an entry's kind is shown).
    pub fn label_key(self) -> &'static str {
        match self {
            Kind::Palette => "inventory.kind.palette",
            Kind::Fractal => "inventory.kind.fractal",
            Kind::Variation => "inventory.kind.variation",
            Kind::Effect => "inventory.kind.effect",
            Kind::Formula => "inventory.kind.formula",
            Kind::Coloring => "inventory.kind.coloring",
            Kind::Field => "inventory.kind.field",
            Kind::FieldColoring => "inventory.kind.field_coloring",
            Kind::Solid => "inventory.kind.solid",
            Kind::SolidColoring => "inventory.kind.solid_coloring",
            Kind::SimModel => "inventory.kind.sim_model",
            Kind::SimColoring => "inventory.kind.sim_coloring",
            Kind::Generator => "inventory.kind.generator",
            Kind::Modifier => "inventory.kind.modifier",
            Kind::ScriptLibrary => "inventory.kind.script_library",
            Kind::Track => "inventory.kind.track",
            Kind::FormulaFile => "inventory.kind.formula_file",
        }
    }

    /// The kind a library file is; `None` for one whose kind could not be
    /// read.
    pub fn of_file(kind: crate::storage::library::Kind) -> Option<Kind> {
        use crate::storage::library::Kind as L;
        Some(match kind {
            L::Generator => Kind::Generator,
            L::Modifier => Kind::Modifier,
            L::ScriptLibrary => Kind::ScriptLibrary,
            L::Formula => Kind::Formula,
            L::Coloring => Kind::Coloring,
            L::Variation => Kind::Variation,
            L::Solid => Kind::Solid,
            L::Effect => Kind::Effect,
            L::SimColoring => Kind::SimColoring,
            L::SimModel => Kind::SimModel,
            L::Field => Kind::Field,
            L::FieldColoring => Kind::FieldColoring,
            L::Track => Kind::Track,
            L::UfFormulas | L::UfColorings | L::Fractint => Kind::FormulaFile,
            L::Unknown => return None,
        })
    }
}

/// An entry's stable name: what a selection holds and a Use names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntryKey {
    /// A palette by its content hash, the API's id for it
    /// ([`crate::api::sync::palette_content_hash`]).
    Palette(String),
    /// A preset by its name (made unique within the presets: a repeated
    /// name gains ` (2)`, ` (3)`, in file order).
    Preset(String),
    /// A registered resource of `kind` by its registry name.
    Registered(Kind, String),
    /// A file of a store, a shipped one (`shipped`) or the user's.
    File { store: Store, file: String, shipped: bool },
    /// An item on the server, of `kind`, by its id (the Catalog's Online
    /// place: [`online`]).
    Online(Kind, String),
}

/// The heading an entry is listed under, and where that heading sorts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Headings sort by this, then entries by their source's order.
    pub order: u32,
    pub label: String,
}

/// One resource, as a person reads it.
#[derive(Debug, Clone)]
pub struct Entry {
    pub kind: Kind,
    pub key: EntryKey,
    pub title: String,
    /// Markdown (`crate::markdown`); empty when its source says nothing.
    pub description: String,
    /// In the order its source lists them; empty when it names none.
    pub authors: Vec<String>,
    /// A download's server version (`v3`), a file's content version (8
    /// hex); `None` for a built-in.
    pub version: Option<String>,
    pub origin: Origin,
    /// A Rhai entry the app registers at start (`Link::plugin`).
    pub plugin: bool,
    pub group: Group,
    /// A file's link record: its online id, owner, visibility.
    pub link: Option<Link>,
    /// False for what the server offers and this app has not downloaded
    /// (a variation or an effect in its cached catalog): it is listed to
    /// be downloaded, and used once it is.
    pub installed: bool,
    /// What the server's catalog says of something not installed: an
    /// effect's stage (`color`, `density`), a variation's category.
    pub category: Option<String>,
    /// A download the server has a newer version of (`v4`): its Update.
    pub update: Option<String>,
    /// An item on the server ([`online`]): who owns it, when it changed,
    /// what the app already holds of it.
    pub online: Option<online::OnlineInfo>,
}

/// What the server offers, as the app last cached its catalogs: the
/// entries [`scan`] lists as not installed. Empty offline, or before the
/// first fetch.
#[derive(Debug, Clone, Copy, Default)]
pub struct Online<'a> {
    pub variations: &'a [crate::api::types::VariationListItem],
    pub effects: &'a [crate::api::types::EffectListItem],
}

impl Entry {
    /// What a search matches: the title, the registry or file name, the
    /// description and the authors, case-insensitively.
    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let name = match &self.key {
            EntryKey::Registered(_, n) | EntryKey::Preset(n) => n.as_str(),
            EntryKey::File { file, .. } => file.as_str(),
            EntryKey::Palette(_) | EntryKey::Online(..) => "",
        };
        self.title.to_lowercase().contains(&q)
            || name.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.authors.iter().any(|a| a.to_lowercase().contains(&q))
    }
}

/// Every entry of every kind: by kind, then group, then each source's own
/// order -- never a map's. `palettes` is the library the app shows (its
/// packs enabled and loaded as the app has them); `online`, what the
/// server offers that is not installed.
pub fn scan(palettes: &crate::scene::palette::PaletteLibrary, online: Online) -> Vec<Entry> {
    let mut out = Vec::new();
    sources::palettes(palettes, &mut out);
    sources::presets(&mut out);
    sources::variations(online, &mut out);
    sources::effects(online, &mut out);
    sources::available(online, &mut out);
    sources::escape(&mut out);
    sources::simulation(&mut out);
    sources::files(&mut out);
    // Stable: within a kind and group, each source's order stands.
    out.sort_by_key(|e| (e.kind, e.group.order));
    out
}

/// A built-in's description (markdown) and authors, from what the app
/// compiles in alone -- never the cached server catalogs [`scan`] falls
/// back on -- so it is the same on every machine: what the engine catalog
/// publishes (`catalog`'s `prose` table) and the API serves at `/types`.
/// `None` for a name with nothing written.
pub fn builtin_prose(kind: Kind, name: &str) -> Option<(String, Vec<String>)> {
    let p = match kind {
        Kind::Variation => prose::variation(name).cloned(),
        Kind::SimModel | Kind::SimColoring => {
            let line = if kind == Kind::SimModel {
                crate::sim::MODELS.iter().find(|m| m.name == name).map(|m| m.description)
            } else {
                crate::sim::COLORINGS.iter().find(|c| c.name == name).map(|c| c.description)
            };
            match prose::twin(kind, name) {
                Some(p) if !p.description.is_empty() => Some(p.clone()),
                _ => line.map(|l| prose::Prose { description: l.to_string(), authors: Vec::new() }),
            }
        }
        _ => prose::twin(kind, name).cloned().or_else(|| prose::untwinned(kind, name)),
    }?;
    (!p.description.is_empty() || !p.authors.is_empty()).then_some((p.description, p.authors))
}

/// What [`scan`] reads that changes while the app runs, cheaply: a
/// change means the list is to be made again. The stores' files are not
/// read here (that is the scan's cost); `files` is the caller's count of
/// their changes (the app's library generation).
pub fn revision(palettes: &crate::scene::palette::PaletteLibrary, files: u64, online: Online) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    files.hash(&mut h);
    for v in online.variations {
        (v.name.as_str(), v.version, v.downloadable).hash(&mut h);
    }
    for e in online.effects {
        (e.name.as_str(), e.version, e.downloadable).hash(&mut h);
    }
    palettes.generation().hash(&mut h);
    for i in 0..palettes.pack_count() {
        palettes.get_pack_info(i).map(|p| (p.pack.is_some(), p.enabled)).hash(&mut h);
    }
    {
        let reg = crate::variations::global_registry();
        for v in reg.all() {
            (v.name.as_str(), v.provenance.version()).hash(&mut h);
        }
    }
    {
        let reg = crate::effects::global_effect_registry();
        for e in reg.all() {
            (e.name.as_str(), e.provenance.version()).hash(&mut h);
        }
    }
    crate::storage::library::plugins().len().hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests;
