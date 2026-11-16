//! One adapter a kind: what exists, read into [`Entry`]s.

use super::prose;
use super::{Entry, EntryKey, Group, Kind};
use crate::config::FractalConfig;
use crate::provenance::{Origin, Provenance};
use crate::scene::palette::Palette;

fn entry(kind: Kind, key: EntryKey, title: &str, group: Group, origin: Origin) -> Entry {
    Entry {
        kind,
        key,
        title: title.to_string(),
        description: String::new(),
        authors: Vec::new(),
        version: None,
        origin,
        plugin: false,
        group,
        link: None,
    }
}

fn with_prose(mut e: Entry, p: Option<&prose::Prose>) -> Entry {
    if let Some(p) = p {
        e.description = p.description.clone();
        e.authors = p.authors.clone();
    }
    e
}

/// A registry entry's version: a download's server version.
fn version_of(p: &Provenance) -> Option<String> {
    p.version().map(|v| format!("v{v}"))
}

// ---------------------------------------------------------------- palettes

/// The palettes of every loaded pack, in the library's pack order, each
/// pack a group. The user's packs (Custom, and the loose files of Local
/// Files) are theirs; the rest ship.
pub(super) fn palettes(out: &mut Vec<Entry>) {
    let Ok(lib) = crate::scene::palette::global_palette_library().read() else { return };
    let custom = lib.custom_pack_index();
    for i in 0..lib.pack_count() {
        let Some(pack) = lib.get_pack(i) else { continue };
        let origin = if Some(i) == custom || pack.pack_name == "Local Files" { Origin::Local } else { Origin::Builtin };
        let group = Group { order: i as u32, label: pack.pack_name.clone() };
        for p in &pack.palettes {
            let key = EntryKey::Palette(crate::api::sync::palette_content_hash(p));
            out.push(entry(Kind::Palette, key, &p.name, group.clone(), origin));
        }
    }
}

/// The palette `hash` names, from the first loaded pack holding it.
pub fn palette(hash: &str) -> Option<Palette> {
    let lib = crate::scene::palette::global_palette_library().read().ok()?;
    (0..lib.pack_count())
        .filter_map(|i| lib.get_pack(i))
        .flat_map(|pack| pack.palettes.iter())
        .find(|p| crate::api::sync::palette_content_hash(p) == hash)
        .cloned()
}

// ----------------------------------------------------------------- presets

/// The presets, each category a group, a repeated name made unique.
pub(super) fn presets(out: &mut Vec<Entry>) {
    for (name, config) in preset_names() {
        let category = crate::scene::presets::PresetCategory::of(config);
        let group = Group { order: category.index() as u32, label: t!(category.label_key()).to_string() };
        out.push(entry(Kind::Fractal, EntryKey::Preset(name.clone()), &name, group, Origin::Builtin));
    }
}

/// Every preset under its key's name: its own, or with ` (2)`, ` (3)`
/// after a name used before it.
fn preset_names() -> Vec<(String, &'static FractalConfig)> {
    let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    crate::scene::presets::global_preset_library()
        .presets()
        .iter()
        .map(|c| {
            let n = seen.entry(c.flame.name.clone()).or_insert(0);
            *n += 1;
            let name = if *n == 1 { c.flame.name.clone() } else { format!("{} ({n})", c.flame.name) };
            (name, c)
        })
        .collect()
}

/// The preset `name` names.
pub fn preset(name: &str) -> Option<FractalConfig> {
    preset_names().into_iter().find(|(n, _)| n == name).map(|(_, c)| c.clone())
}

// -------------------------------------------------------------- variations

fn variation_category_key(c: crate::variations::VariationCategory) -> &'static str {
    use crate::variations::VariationCategory as C;
    match c {
        C::Basic2D => "inventory.variation_category.basic_2d",
        C::Advanced2D => "inventory.variation_category.advanced_2d",
        C::Depth3D => "inventory.variation_category.depth_3d",
        C::Rotation3D => "inventory.variation_category.rotation_3d",
        C::Full3D => "inventory.variation_category.full_3d",
        C::Only3D => "inventory.variation_category.only_3d",
        C::Plugin => "inventory.variation_category.plugin",
    }
}

/// The registry's variations, each category a group, in registration
/// order: built-ins and downloads. The user's own (plug-ins, and those a
/// config carries) are files or the config's, not listed here.
pub(super) fn variations(out: &mut Vec<Entry>) {
    let catalog = prose::variation_catalog();
    let reg = crate::variations::global_registry();
    for v in reg.all() {
        if v.provenance == Provenance::Local {
            continue;
        }
        let order = crate::variations::VariationCategory::ALL.iter().position(|c| *c == v.category).unwrap_or(99) as u32;
        let group = Group { order, label: t!(variation_category_key(v.category)).to_string() };
        let mut e = entry(Kind::Variation, EntryKey::Registered(Kind::Variation, v.name.clone()), &v.display_name, group, v.provenance.origin());
        e.version = version_of(&v.provenance);
        let p = if v.provenance.is_builtin() { prose::variation(&v.name) } else { None };
        out.push(with_prose(e, p.or_else(|| catalog.get(&v.name))));
    }
}

// ----------------------------------------------------------------- effects

/// The registry's effects, colour then density, built-ins and downloads.
pub(super) fn effects(out: &mut Vec<Entry>) {
    use crate::effects::EffectCategory;
    let catalog = prose::effect_catalog();
    let reg = crate::effects::global_effect_registry();
    for e in reg.all() {
        if e.provenance == Provenance::Local {
            continue;
        }
        let (order, key) = match e.category {
            EffectCategory::Color => (0, "inventory.effect_category.color"),
            EffectCategory::Density => (1, "inventory.effect_category.density"),
        };
        let group = Group { order, label: t!(key).to_string() };
        let mut entry = entry(Kind::Effect, EntryKey::Registered(Kind::Effect, e.name.clone()), &e.translated_name(), group, e.provenance.origin());
        entry.version = version_of(&e.provenance);
        let p = if e.provenance.is_builtin() { prose::twin(Kind::Effect, &e.name) } else { None };
        out.push(with_prose(entry, p.or_else(|| catalog.get(&e.name))));
    }
}

// ------------------------------------------------------------ escape kinds

/// The escape registries' built-ins: formulas, colourings, fields and
/// their colourings, solids and theirs.
pub(super) fn escape(out: &mut Vec<Entry>) {
    let mut push = |kind: Kind, name: &str, title: &str| {
        let group = Group { order: 0, label: t!(kind.label_key()).to_string() };
        let e = entry(kind, EntryKey::Registered(kind, name.to_string()), title, group, Origin::Builtin);
        out.push(with_prose(e, prose::twin(kind, name)));
    };
    for f in crate::escape::FORMULAS {
        push(Kind::Formula, f.name, f.display_name);
    }
    for c in crate::escape::COLORINGS {
        push(Kind::Coloring, c.name, c.display_name);
    }
    for f in crate::escape::fields::FIELDS {
        push(Kind::Field, f.name, f.display_name);
    }
    for c in crate::escape::fields::FIELD_COLORINGS {
        push(Kind::FieldColoring, c.name, c.display_name);
    }
    for d in crate::escape::ifs::IFS_DEFS {
        push(Kind::Solid, d.name, d.display_name);
    }
    for c in crate::escape::ifs::IFS_COLORINGS {
        push(Kind::SolidColoring, c.name, c.display_name);
    }
}

// -------------------------------------------------------------- simulation

/// The simulation's models and colourings: the twin's prose, else the
/// def's line.
pub(super) fn simulation(out: &mut Vec<Entry>) {
    let mut push = |kind: Kind, name: &str, title: &str, line: &str| {
        let group = Group { order: 0, label: t!(kind.label_key()).to_string() };
        let mut e = entry(kind, EntryKey::Registered(kind, name.to_string()), title, group, Origin::Builtin);
        e.description = line.to_string();
        out.push(match prose::twin(kind, name) {
            Some(p) if !p.description.is_empty() => with_prose(e, Some(p)),
            _ => e,
        });
    };
    for m in crate::sim::MODELS {
        push(Kind::SimModel, m.name, m.display_name, m.description);
    }
    for c in crate::sim::COLORINGS {
        push(Kind::SimColoring, c.name, c.display_name, c.description);
    }
}

// ------------------------------------------------------------------- files

/// The stores' files and the shipped ones (`storage::library::scan`):
/// the examples and starter scripts, then the user's, then what they
/// adopted. A built-in's twin is not listed: it is the built-in's own
/// source, which its entry opens.
pub(super) fn files(out: &mut Vec<Entry>) {
    use crate::storage::library::Store;
    let twins: std::collections::HashSet<String> = crate::effects::custom::BUILTIN_TWINS
        .iter()
        .map(|(n, _)| format!("{n}.rhai"))
        .chain(crate::sim::custom::SHIPPED.iter().chain(crate::sim::custom::SHIPPED_MODELS).map(|(n, _)| format!("sim-{n}.rhai")))
        .collect();
    for f in crate::storage::library::scan() {
        let Some(kind) = Kind::of_file(f.kind) else { continue };
        let shipped = f.origin == Origin::Builtin;
        if shipped && f.store == Store::Formulas && twins.contains(&f.file) {
            continue;
        }
        let (order, label) = match f.origin {
            Origin::Builtin => (100, t!("inventory.group.examples")),
            Origin::Local => (101, t!("inventory.group.yours")),
            Origin::Api => (102, t!("inventory.group.adopted")),
        };
        let group = Group { order, label: label.to_string() };
        let mut e = entry(kind, EntryKey::File { store: f.store, file: f.file.clone(), shipped }, &f.title, group, f.origin);
        e.description = f.description;
        e.authors = prose::authors(&f.author);
        e.version = Some(f.version);
        e.plugin = f.link.plugin;
        if !shipped {
            e.link = Some(f.link);
        }
        out.push(e);
    }
}
