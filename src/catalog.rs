//! What a saved config is, as the online catalog records it
//! (docs/projects/api-catalog-spec.md): its engine, scene and dimension, its
//! versions, and the components it is made of, each a type -- a built-in, a
//! script, or a source -- so the API can browse by fractal type and find
//! every config that uses Mandelbrot, built-in or unedited twin alike.
//!
//! **The server works this out itself** (the user's decision: "we
//! shouldn't trust the client"), from a file this module generates the way
//! the engine contract is generated: `docs/generated/engine-catalog.json`,
//! copied to the API. It holds
//!
//! - the **tables** only the engine knows: which escape-time names are
//!   formulas, fields or solids, which draw a flame, the built-ins' display
//!   names, and every Rhai twin's registered name with its built-in -- kept
//!   append-only, so a config saved with an older twin still groups under
//!   its built-in;
//! - **examples**: real configs (every bundled preset, and made-up cases
//!   for twins, edits, scripts, terrains, solids, lenses, layers, effects),
//!   each with the summary [`summarize`] gives. The API's own summariser
//!   must give the same for every one (its conformance test), which keeps
//!   the two implementations of [`summarize`] the same program.
//!
//! [`summarize`] reads only the config's JSON and the tables, never the
//! engine, so the server can do exactly what it does. A rule change bumps
//! [`SUMMARY_VERSION`] and fails the API's examples until it follows.
//!
//! Regenerate with `UPDATE_CATALOG=1 cargo test --lib catalog_is_current`.

use serde::Serialize;
use serde_json::{json, Map, Value};

/// The summariser's rules' version. Bump on any change to what
/// [`summarize`] reads or writes; the API pins it.
pub const SUMMARY_VERSION: u32 = 1;

/// Where the generated file lives, from the crate root.
pub const CATALOG_PATH: &str = "docs/generated/engine-catalog.json";

/// The committed catalog, for the twin names earlier builds published
/// (the table is append-only).
const COMMITTED: &str = include_str!("../docs/generated/engine-catalog.json");

/// One piece of a config: a role, and the type that fills it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Component {
    /// What it does here: `formula`, `field`, `solid`, `coloring`,
    /// `layer_coloring`, `lens`, `transform`, `model`, `layer_model`,
    /// `variation`, `effect`.
    pub role: String,
    /// What kind of type it is: `formula`, `field`, `solid`, `coloring`,
    /// `field_coloring`, `sim_model`, `sim_coloring`, `variation`, `effect`.
    pub kind: String,
    /// `builtin` (a built-in, or its twin unedited), `script` (a Rhai source
    /// with an id of its own), `source` (any other written source).
    pub origin: String,
    /// The registry name, the script's UUID, or the source's stable hash.
    pub key: String,
    /// A script's version (the first 8 hex of its source's SHA-256).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The built-in's display name, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The type the config is browsed under: an escape config's formula,
    /// field or solid; a simulation's base model. A flame has none.
    pub primary: bool,
}

/// A saved config's facts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    /// `flame`, `escape` or `simulation`.
    pub engine: String,
    /// `plane`, `space` (a 3D flame), `solid` or `terrain`.
    pub scene: String,
    /// 2 for a plane, else 3.
    pub dimension: u8,
    /// The config version that wrote it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_version: Option<u32>,
    /// The lowest config version that reads it (its version, when it
    /// predates `min_reader`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_reader: Option<u32>,
    /// Every component, sorted, without repeats.
    pub components: Vec<Component>,
    pub summary_version: u32,
}

// ---------------------------------------------------------------------------
// The summariser: the config's JSON and the tables, nothing else.
// ---------------------------------------------------------------------------

/// The facts of a config, from its JSON (as `FractalConfig::to_json_value`
/// writes it, fields at their defaults absent) and the catalog's `tables`.
///
/// This is the program the API implements again; keep it to JSON reads,
/// table lookups and string tests.
pub fn summarize(config: &Value, tables: &Value) -> Summary {
    let text = |ptr: &str| config.pointer(ptr).and_then(Value::as_str);
    let int = |ptr: &str| config.pointer(ptr).and_then(Value::as_u64).map(|v| v as u32);
    let default = |key: &str| tables["defaults"][key].as_str().unwrap_or_default().to_string();

    let mode = text("/render_mode").map_or_else(|| default("render_mode"), str::to_string);
    let engine = match mode.as_str() {
        "escape" => "escape",
        "simulation" => "simulation",
        _ => "flame",
    };
    let config_version = int("/version");
    let min_reader = int("/min_reader").or(config_version);

    let mut out: Vec<Component> = Vec::new();
    let mut scene = String::from("plane");
    let mut flame_drawn = engine == "flame";

    match engine {
        "flame" => {
            if mode == "3d" {
                scene = "space".into();
            }
        }
        "escape" => {
            let name = text("/escape/formula").map_or_else(|| default("escape_formula"), str::to_string);
            let source = text("/escape/custom_formula/source");
            let mut c = identify(&name, "formula", source, tables);
            // A formula slot holds a formula, a field or a solid.
            if c.origin == "builtin" && c.kind == "formula" {
                if let Some(k) = tables["escape_formula_kinds"][name.as_str()].as_str() {
                    c.kind = k.to_string();
                    // Its display name is filed under its kind.
                    c.display_name = tables["display_names"][k][name.as_str()].as_str().map(str::to_string);
                }
            }
            c.role = c.kind.clone();
            c.primary = true;
            if c.kind == "solid" {
                scene = "solid".into();
            } else if config.pointer("/escape/terrain/enabled").and_then(Value::as_bool) == Some(true) {
                scene = "terrain".into();
            }
            flame_drawn = c.origin == "builtin" && tables["needs_flame"].as_array().is_some_and(|a| a.iter().any(|n| n == c.key.as_str()));
            out.push(c);

            let coloring = text("/escape/coloring").map_or_else(|| default("escape_coloring"), str::to_string);
            out.push(with_role(identify(&coloring, "coloring", None, tables), "coloring"));
            if let Some(layer) = text("/escape/layer/coloring").filter(|s| !s.is_empty()) {
                out.push(with_role(identify(layer, "coloring", None, tables), "layer_coloring"));
            }
            if let Some(lens) = text("/escape/lens").filter(|s| !s.is_empty()) {
                out.push(with_role(identify(lens, "variation", None, tables), "lens"));
            }
            for t in list(config, "/escape/transforms") {
                if let Some(v) = t["variation"].as_str().filter(|s| !s.is_empty()) {
                    out.push(with_role(identify(v, "variation", None, tables), "transform"));
                }
            }
        }
        _ => {
            let model = text("/sim/model").map_or_else(|| default("sim_model"), str::to_string);
            let mut c = with_role(identify(&model, "sim_model", None, tables), "model");
            c.primary = true;
            out.push(c);
            for layer in list(config, "/sim/layers") {
                if let Some(m) = layer["model"].as_str().filter(|s| !s.is_empty()) {
                    out.push(with_role(identify(m, "sim_model", None, tables), "layer_model"));
                }
            }
            let coloring = text("/sim/coloring").map_or_else(|| default("sim_coloring"), str::to_string);
            out.push(with_role(identify(&coloring, "sim_coloring", None, tables), "coloring"));
            for layer in list(config, "/sim/color_layers") {
                if let Some(c) = layer["coloring"].as_str().filter(|s| !s.is_empty()) {
                    out.push(with_role(identify(c, "sim_coloring", None, tables), "layer_coloring"));
                }
            }
            if config.pointer("/sim/terrain/enabled").and_then(Value::as_bool) == Some(true) {
                scene = "terrain".into();
            }
        }
    }

    // A flame's variations: of every pool, subflames too, at a non-zero
    // weight. An escape formula that draws a flame (the Flame Attractor)
    // counts its flame's.
    if flame_drawn {
        if let Some(flame) = config.get("flame") {
            flame_variations(flame, tables, &mut out);
        }
    }
    for stage in ["/density_effects", "/color_effects"] {
        for e in list(config, stage) {
            if let Some(name) = e["effect_type"].as_str().filter(|s| !s.is_empty()) {
                out.push(with_role(identify(name, "effect", None, tables), "effect"));
            }
        }
    }

    out.sort();
    out.dedup();
    let dimension = if scene == "plane" { 2 } else { 3 };
    Summary { engine: engine.into(), scene, dimension, config_version, min_reader, components: out, summary_version: SUMMARY_VERSION }
}

fn list<'a>(config: &'a Value, ptr: &str) -> impl Iterator<Item = &'a Value> {
    config.pointer(ptr).and_then(Value::as_array).into_iter().flatten()
}

fn with_role(mut c: Component, role: &str) -> Component {
    c.role = role.to_string();
    c
}

fn flame_variations(flame: &Value, tables: &Value, out: &mut Vec<Component>) {
    for pool in ["transforms", "linked_transforms", "final_transforms"] {
        for t in flame[pool].as_array().into_iter().flatten() {
            for (name, weight) in t["variations"].as_object().into_iter().flatten() {
                if weight.as_f64().is_some_and(|w| w != 0.0) {
                    out.push(with_role(identify(name, "variation", None, tables), "variation"));
                }
            }
        }
    }
    for sub in flame["subflames"].as_array().into_iter().flatten() {
        flame_variations(sub, tables, out);
    }
}

/// The type a registry name stands for, of kind `kind`:
///
/// 1. a twin's registered name: its built-in;
/// 2. `custom-u<32 hex>_<8 hex>` or `u<32 hex>_<8 hex>`: a script (its id and
///    version, `scripted::key`);
/// 3. any other `custom-<x>`: a written source, `x` its stable hash;
/// 4. anything else: a built-in by that name.
///
/// A custom formula's kind is read from its `source` (`source_kinds`).
fn identify(name: &str, kind: &str, source: Option<&str>, tables: &Value) -> Component {
    let display = |kind: &str, key: &str| tables["display_names"][kind][key].as_str().map(str::to_string);
    if let Some(twin) = tables["twins"].get(name) {
        let builtin = twin["builtin"].as_str().unwrap_or(name).to_string();
        let kind = twin["kind"].as_str().unwrap_or(kind).to_string();
        let display_name = display(&kind, &builtin);
        return Component { role: String::new(), kind, origin: "builtin".into(), key: builtin, version: None, display_name, primary: false };
    }
    let written = name.strip_prefix("custom-");
    let bare = written.unwrap_or(name);
    let kind = match (kind, source) {
        ("formula", Some(src)) => source_kind(src, tables).unwrap_or_else(|| "formula".into()),
        _ => kind.to_string(),
    };
    if let Some((id, version)) = script_key(bare) {
        return Component { role: String::new(), kind, origin: "script".into(), key: id, version: Some(version), display_name: None, primary: false };
    }
    if let Some(hash) = written {
        return Component { role: String::new(), kind, origin: "source".into(), key: hash.to_string(), version: None, display_name: None, primary: false };
    }
    let display_name = display(&kind, name);
    Component { role: String::new(), kind, origin: "builtin".into(), key: name.to_string(), version: None, display_name, primary: false }
}

/// `u` + 32 hex + `_` + 8 hex (`scripted::key`): the hyphenated UUID and the
/// version.
fn script_key(s: &str) -> Option<(String, String)> {
    let b = s.as_bytes();
    let hex = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c));
    if b.len() != 42 || b[0] != b'u' || b[33] != b'_' || !hex(1..33) || !hex(34..42) {
        return None;
    }
    let h = &s[1..33];
    Some((format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]), s[34..42].to_string()))
}

/// A Rhai source's kind as the engine reads it (`lang::rhai::kind_of`): the
/// first of the ordered `source_kinds` rules whose function the source
/// defines at the top level (a line starting `fn <name>(`).
fn source_kind(source: &str, tables: &Value) -> Option<String> {
    let defined: Vec<&str> = source
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("fn "))
        .filter_map(|rest| rest.split('(').next())
        .map(str::trim)
        .collect();
    tables["source_kinds"].as_array()?.iter().find_map(|rule| {
        let f = rule[0].as_str()?;
        defined.contains(&f).then(|| rule[1].as_str().map(str::to_string)).flatten()
    })
}

// ---------------------------------------------------------------------------
// The engine's side: the tables, the reference summary, the generated file.
// ---------------------------------------------------------------------------

/// The facts of a config, as the API will work them out.
pub fn summary(config: &crate::config::FractalConfig) -> Summary {
    let json = config.to_json_value().unwrap_or(Value::Null);
    summarize(&json, tables())
}

/// The tables, from the engine's registries, with every twin name an
/// earlier build published kept.
pub fn tables() -> &'static Value {
    static TABLES: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let committed: Value = serde_json::from_str(COMMITTED).unwrap_or(Value::Null);
        build_tables(committed.pointer("/tables/twins"))
    })
}

fn build_tables(earlier_twins: Option<&Value>) -> Value {
    use crate::lang::EntryKind;

    let mut kinds = Map::new();
    for f in crate::escape::FORMULAS {
        kinds.insert(f.name.into(), json!("formula"));
    }
    for d in crate::escape::ifs::IFS_DEFS {
        kinds.insert(d.name.into(), json!(if d.solid { "solid" } else { "formula" }));
    }
    for f in crate::escape::fields::FIELDS {
        kinds.insert(f.name.into(), json!("field"));
    }
    let needs_flame: Vec<&str> = crate::escape::ifs::IFS_DEFS.iter().filter(|d| d.needs_flame).map(|d| d.name).collect();

    let mut display = Map::new();
    let mut put = |kind: &str, name: &str, label: &str| {
        display.entry(kind.to_string()).or_insert_with(|| json!({}))[name] = json!(label);
    };
    for f in crate::escape::FORMULAS {
        put("formula", f.name, f.display_name);
    }
    for d in crate::escape::ifs::IFS_DEFS {
        put(if d.solid { "solid" } else { "formula" }, d.name, d.display_name);
    }
    for f in crate::escape::fields::FIELDS {
        put("field", f.name, f.display_name);
    }
    for c in crate::escape::COLORINGS {
        put("coloring", c.name, c.display_name);
    }
    for c in crate::escape::ifs::IFS_COLORINGS {
        put("coloring", c.name, c.display_name);
    }
    for c in crate::escape::fields::FIELD_COLORINGS {
        put("coloring", c.name, c.display_name);
    }
    for m in crate::sim::MODELS {
        put("sim_model", m.name, m.display_name);
    }
    for c in crate::sim::COLORINGS {
        put("sim_coloring", c.name, c.display_name);
    }
    for v in crate::variations::global_registry().all() {
        if v.provenance == crate::provenance::Provenance::Builtin {
            put("variation", &v.name, &v.display_name);
        }
    }
    for e in crate::effects::global_effect_registry().all() {
        if matches!(e.source, crate::effects::EffectSource::Builtin { .. }) || crate::effects::custom::BUILTIN_TWINS.iter().any(|(n, _)| *n == e.name) {
            put("effect", &e.name, &e.translated_name());
        }
    }

    // Every twin, by the name it registers under when put in its built-in's
    // place, and every name an earlier build published (append-only).
    let mut twins: Map<String, Value> = earlier_twins.and_then(Value::as_object).cloned().unwrap_or_default();
    let mut twin = |registered: String, kind: &str, builtin: &str| {
        twins.insert(registered, json!({"kind": kind, "builtin": builtin}));
    };
    let escape = |kind: EntryKind, src: crate::config::escape::CustomSource| crate::escape::custom::name_of(kind, &src);
    for (name, _) in crate::escape::twins::TWINS {
        if let Some(src) = crate::escape::twins::custom_source(name) {
            twin(escape(EntryKind::Formula, src), "formula", name);
        }
    }
    for (name, _) in crate::escape::twins::COLORING_TWINS {
        if let Some(src) = crate::escape::twins::coloring_custom_source(name) {
            twin(escape(EntryKind::Coloring, src), "coloring", name);
        }
    }
    for (name, _) in crate::escape::twins::SOLID_TWINS {
        if let Some(src) = crate::escape::twins::solid_custom_source(name) {
            twin(escape(EntryKind::Solid, src), "solid", name);
        }
    }
    for (name, _) in crate::escape::twins::FIELD_TWINS {
        if let Some(src) = crate::escape::twins::field_custom_source(name) {
            twin(escape(EntryKind::Field, src), "field", name);
        }
    }
    for (name, _) in crate::escape::twins::FIELD_COLORING_TWINS {
        if let Some(src) = crate::escape::twins::field_coloring_custom_source(name) {
            twin(escape(EntryKind::FieldColoring, src), "field_coloring", name);
        }
    }
    for (name, source) in crate::sim::custom::SHIPPED_MODELS {
        twin(crate::sim::custom::name_of(source), "sim_model", name);
    }
    for (name, source) in crate::sim::custom::SHIPPED {
        twin(crate::sim::custom::name_of(source), "sim_coloring", name);
    }
    for (name, source) in crate::variations::twins::TWINS {
        if let Some(key) = crate::variations::custom::key_of(source) {
            twin(key, "variation", name);
        }
    }

    json!({
        "defaults": {
            "render_mode": "2d",
            "escape_formula": crate::config::escape::EscapeConfig::default().formula,
            "escape_coloring": crate::config::escape::EscapeConfig::default().coloring,
            "sim_model": crate::config::sim::SimConfig::default().model,
            "sim_coloring": crate::config::sim::SimConfig::default().coloring,
        },
        "escape_formula_kinds": kinds,
        "needs_flame": needs_flame,
        "source_kinds": [
            ["effect", "effect"], ["cell_color", "sim_coloring"], ["cell_step", "sim_model"],
            ["variation", "variation"], ["variation3", "variation"], ["term", "field"],
            ["shade", "field_coloring"], ["distance", "solid"], ["finish", "coloring"],
            ["escaped", "formula"], ["step", "formula"],
        ],
        "display_names": display,
        "twins": twins,
    })
}

/// The made-up examples beside the presets: what presets alone do not
/// show.
fn constructed_examples() -> Vec<(String, crate::config::FractalConfig)> {
    use crate::config::FractalConfig;
    use crate::scene::transforms::{RenderMode, Transform};
    let mut out = Vec::new();
    let escape = |formula: &str| {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.formula = formula.into();
        c
    };
    let twin_in_place = |builtin: &str, src: crate::config::escape::CustomSource, kind: crate::lang::EntryKind| {
        let mut c = escape(builtin);
        c.escape.formula = crate::escape::custom::name_of(kind, &src);
        c.escape.custom_formula = Some(src);
        c
    };

    let mb = crate::escape::twins::custom_source("mandelbrot").expect("the Mandelbrot twin");
    out.push(("the Mandelbrot twin, unedited".into(), twin_in_place("mandelbrot", mb.clone(), crate::lang::EntryKind::Formula)));
    let mut edited = mb.clone();
    edited.source.push_str("\n// an edit\n");
    out.push(("the Mandelbrot twin, edited".into(), twin_in_place("mandelbrot", edited, crate::lang::EntryKind::Formula)));
    let mut scripted = mb;
    scripted.source = crate::lang::rhai::with_own_id(&scripted.source, "0f8fad5b-d9cb-469f-a165-70867728950e");
    out.push(("a Mandelbrot of the user's own (an id)".into(), twin_in_place("mandelbrot", scripted, crate::lang::EntryKind::Formula)));
    if let Some((name, _)) = crate::escape::twins::SOLID_TWINS.first() {
        if let Some(src) = crate::escape::twins::solid_custom_source(name) {
            out.push((format!("the {name} twin (a solid)"), twin_in_place(name, src, crate::lang::EntryKind::Solid)));
        }
    }
    if let Some((name, _)) = crate::escape::twins::FIELD_TWINS.first() {
        if let Some(src) = crate::escape::twins::field_custom_source(name) {
            out.push((format!("the {name} twin (a field)"), twin_in_place(name, src, crate::lang::EntryKind::Field)));
        }
    }

    let mut terrain = escape("burning_ship");
    terrain.escape.terrain.enabled = true;
    out.push(("an escape terrain".into(), terrain));
    if let Some(solid) = crate::escape::ifs::IFS_DEFS.iter().find(|d| d.solid && !d.needs_flame) {
        out.push((format!("a built-in solid ({})", solid.name), escape(solid.name)));
    }
    let mut attractor = escape("ifs_flame_3d");
    let mut t = Transform::new();
    t.set_variation("julian", 1.0);
    t.set_variation("spherical", 0.0);
    attractor.flame.transforms = vec![t];
    out.push(("the Flame Attractor (it draws its flame)".into(), attractor));
    let mut lensed = escape("mandelbrot");
    lensed.escape.lens = "swirl".into();
    lensed.escape.layer.coloring = "orbit_trap".into();
    out.push(("a lens and a layer colouring".into(), lensed));

    let mut sim = FractalConfig::default();
    sim.render_mode = RenderMode::Simulation;
    sim.sim.model = "gray_scott".into();
    let mut layered = sim.clone();
    layered.sim.terrain.enabled = true;
    out.push(("a simulation terrain".into(), layered));
    if let Some((name, source)) = crate::sim::custom::SHIPPED_MODELS.first() {
        let mut twin = sim.clone();
        twin.sim.model = crate::sim::custom::name_of(source);
        twin.sim.custom_model = Some(crate::config::escape::CustomSource { dialect: crate::config::escape::CustomDialect::Rhai, entry: String::new(), source: source.to_string(), variations: Vec::new() });
        out.push((format!("the {name} model twin"), twin));
    }

    let mut flame3d = FractalConfig::default();
    flame3d.render_mode = RenderMode::ThreeD;
    let mut t = Transform::new();
    t.set_variation("linear", 1.0);
    flame3d.flame.transforms = vec![t];
    if let Some((name, source)) = crate::variations::twins::TWINS.first() {
        if let Some(key) = crate::variations::custom::key_of(source) {
            flame3d.flame.transforms[0].set_variation(&key, 0.5);
            let _ = name;
        }
    }
    flame3d.color_effects.push(crate::effects::EffectInstance::new("vignette"));
    out.push(("a 3D flame with a variation twin and an effect".into(), flame3d));
    out
}

/// The generated catalog: the tables and the examples.
pub fn catalog() -> Value {
    let committed: Value = serde_json::from_str(COMMITTED).unwrap_or(Value::Null);
    let tables = build_tables(committed.pointer("/tables/twins"));
    let mut examples = Vec::new();
    let presets: Vec<Value> = serde_json::from_str(crate::resources::presets::PRESETS_JSON).unwrap_or_default();
    let mut configs: Vec<(String, crate::config::FractalConfig)> = presets
        .into_iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let name = v.get("name").and_then(Value::as_str).map_or_else(|| format!("preset {i}"), |n| format!("preset: {n}"));
            crate::config::FractalConfig::from_json_value(v).ok().map(|c| (name, c))
        })
        .collect();
    configs.extend(constructed_examples());
    for (name, config) in configs {
        let Ok(json) = config.to_json_value() else { continue };
        let summary = summarize(&json, &tables);
        examples.push(json!({"name": name, "config": json, "summary": summary}));
    }
    json!({
        "$comment": "GENERATED by the client (src/catalog.rs; UPDATE_CATALOG=1 cargo test --lib catalog_is_current). What the API needs to summarise a saved config as the client would: the tables only the engine knows, and examples whose summaries the API's summariser must reproduce exactly. The twin table is append-only. See docs/projects/api-catalog-spec.md.",
        "summary_version": SUMMARY_VERSION,
        "tables": tables,
        "examples": examples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FractalConfig;

    /// The committed file is what this build generates. Regenerate with
    /// `UPDATE_CATALOG=1`, and copy it to the API (docs/generated there).
    #[test]
    fn catalog_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CATALOG_PATH);
        let fresh = serde_json::to_string_pretty(&catalog()).unwrap() + "\n";
        if std::env::var("UPDATE_CATALOG").is_ok() {
            std::fs::write(&path, &fresh).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        assert!(committed == fresh, "{CATALOG_PATH} is stale: UPDATE_CATALOG=1 cargo test --lib catalog_is_current, then copy it to the API");
    }

    /// The summary agrees with the engine's own reading of every example:
    /// the scene from `terrain_active` and the IFS registry, the engine from
    /// the render mode, and an unedited twin as its built-in.
    #[test]
    fn the_summary_agrees_with_the_engine() {
        let c = catalog();
        for ex in c["examples"].as_array().unwrap() {
            let name = ex["name"].as_str().unwrap();
            let config = FractalConfig::from_json_value(ex["config"].clone()).unwrap();
            let s = summary(&config);
            assert_eq!(serde_json::to_value(&s).unwrap(), ex["summary"], "{name}: the file's summary is this build's");
            use crate::scene::transforms::RenderMode;
            let want_scene = match config.render_mode {
                RenderMode::TwoD => "plane",
                RenderMode::ThreeD => "space",
                RenderMode::Escape => {
                    let solid = crate::escape::ifs::get_ifs(&config.escape.formula).is_some_and(|d| d.solid)
                        || config.escape.custom_formula.as_ref().is_some_and(|src| config.escape.formula.starts_with("custom-") && crate::lang::rhai::kind_of(&src.source) == Some(crate::lang::EntryKind::Solid));
                    if solid {
                        "solid"
                    } else if config.escape.terrain_active() {
                        "terrain"
                    } else {
                        "plane"
                    }
                }
                RenderMode::Simulation => {
                    if config.sim.terrain_active() {
                        "terrain"
                    } else {
                        "plane"
                    }
                }
            };
            assert_eq!(s.scene, want_scene, "{name}");
            assert_eq!(s.dimension, if want_scene == "plane" { 2 } else { 3 }, "{name}");
            let primaries = s.components.iter().filter(|c| c.primary).count();
            assert_eq!(primaries, usize::from(!matches!(config.render_mode, RenderMode::TwoD | RenderMode::ThreeD)), "{name}: one primary for escape and simulation, none for a flame");
        }
    }

    /// The made-up cases say what they claim.
    #[test]
    fn twins_group_under_their_built_ins_and_edits_do_not() {
        let c = catalog();
        let primary = |name: &str| -> Value {
            let ex = c["examples"].as_array().unwrap().iter().find(|e| e["name"] == name).unwrap_or_else(|| panic!("no example {name}"));
            ex["summary"]["components"].as_array().unwrap().iter().find(|c| c["primary"] == true).cloned().unwrap()
        };
        let twin = primary("the Mandelbrot twin, unedited");
        assert_eq!((twin["origin"].as_str(), twin["key"].as_str(), twin["display_name"].as_str()), (Some("builtin"), Some("mandelbrot"), Some("Mandelbrot")));
        let edited = primary("the Mandelbrot twin, edited");
        assert_eq!(edited["origin"], "source");
        let own = primary("a Mandelbrot of the user's own (an id)");
        assert_eq!((own["origin"].as_str(), own["key"].as_str()), (Some("script"), Some("0f8fad5b-d9cb-469f-a165-70867728950e")));
        assert!(own["version"].as_str().is_some_and(|v| v.len() == 8));
    }

    /// The source-kind rules are the engine's: every twin source reads as
    /// the kind `lang::rhai::kind_of` gives it.
    #[test]
    fn source_kinds_match_the_engine() {
        use crate::lang::EntryKind;
        let name = |k: EntryKind| match k {
            EntryKind::Formula => "formula",
            EntryKind::Coloring => "coloring",
            EntryKind::Transform => "variation",
            EntryKind::Solid => "solid",
            EntryKind::Effect => "effect",
            EntryKind::SimColoring => "sim_coloring",
            EntryKind::SimModel => "sim_model",
            EntryKind::Field => "field",
            EntryKind::FieldColoring => "field_coloring",
        };
        let mut sources: Vec<&str> = Vec::new();
        sources.extend(crate::escape::twins::TWINS.iter().map(|(_, s)| *s));
        sources.extend(crate::escape::twins::COLORING_TWINS.iter().map(|(_, s)| *s));
        sources.extend(crate::escape::twins::SOLID_TWINS.iter().map(|(_, s)| *s));
        sources.extend(crate::escape::twins::FIELD_TWINS.iter().map(|(_, s)| *s));
        sources.extend(crate::sim::custom::SHIPPED_MODELS.iter().map(|(_, s)| *s));
        sources.extend(crate::sim::custom::SHIPPED.iter().map(|(_, s)| *s));
        sources.extend(crate::variations::twins::TWINS.iter().take(40).map(|(_, s)| *s));
        sources.extend(crate::effects::custom::BUILTIN_TWINS.iter().map(|(_, s)| *s));
        for src in sources {
            let engine = crate::lang::rhai::kind_of(src).map(name).map(str::to_string);
            assert_eq!(source_kind(src, tables()), engine, "{}", &src[..src.len().min(120)]);
        }
    }
}
