//! Variations written in the formula language's Rhai (the KIFS plan, C1:
//! [docs/archive/projects/escape-kifs-and-variations.md](../../docs/projects/escape-kifs-and-variations.md)).
//!
//! A transformation -- a Rhai source with `fn variation(z, p)` -- compiles
//! to the payload a plug-in is ([`VariationDownload`]: its parameters, its
//! 2D and 3D WGSL) and registers through the same path, with
//! [`Provenance::Local`]. A flame carries the sources it uses
//! (`Flame::custom_variations`), and [`ensure`] registers them before
//! anything builds the flame's shader, so a saved flame renders anywhere
//! the app runs.
//!
//! It registers under its key (design rule 7): its id and its version,
//! `scripted::key`, so two versions of one variation are two entries and
//! a flame renders the version it carries. The id is `INFO.id`; a source
//! from before ids has its `INFO.name` alone, and its id is derived from
//! the name (`scripted::derived_id`), which is what a config saved then
//! migrates to (config v7). `INFO.name`, a lowercase identifier, stays
//! what a formula's `vary("name", z)` calls it by.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use crate::api::types::VariationDownload;
use crate::config::escape::{CustomDialect, CustomSource};
use crate::lang::{self, EntryKind};
use crate::provenance::Provenance;
use crate::scene::transforms::Flame;

/// Whether `name` can be a custom variation's: a lowercase identifier,
/// as the built-ins' are.
pub fn check_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 48
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(format!("`{name}` is not a variation name: lowercase letters, digits and `_`, starting with a letter"))
    }
}

/// A variation source's id: its `INFO.id`, or for one from before ids,
/// the id its `INFO.name` derives (`scripted::derived_id`).
pub fn id_of_source(source: &str) -> Option<String> {
    match lang::rhai::info_string(source, "id") {
        Some(id) if crate::scripted::is_uuid(&id) => Some(id),
        _ => lang::rhai::variation_name(source).map(|n| crate::scripted::derived_id(&n)),
    }
}

/// The registry key a variation source registers under: its id and its
/// version (`scripted::key`).
pub fn key_of(source: &str) -> Option<String> {
    id_of_source(source).map(|id| crate::scripted::key(&id, source))
}

/// The Rhai sources' parameter layouts (their conditions and grids; one
/// parameter system, step 10), by the name each registers under.
fn layouts() -> &'static RwLock<HashMap<String, Arc<crate::params::layout::ParamLayout>>> {
    static L: OnceLock<RwLock<HashMap<String, Arc<crate::params::layout::ParamLayout>>>> = OnceLock::new();
    L.get_or_init(Default::default)
}

/// `source`'s layout, recorded under `name` (or forgotten, when it has
/// none): what the panel draws its parameters by.
fn record_layout(name: &str, source: &str) -> Result<(), String> {
    let layout = lang::rhai::plain_layout(source)?;
    if let Ok(mut l) = layouts().write() {
        match layout {
            Some(layout) => l.insert(name.to_string(), layout),
            None => l.remove(name),
        };
    }
    Ok(())
}

/// The parameter layout `name` registered with, when its `PARAMS` has
/// conditions or grids.
pub fn param_layout(name: &str) -> Option<Arc<crate::params::layout::ParamLayout>> {
    layouts().read().ok()?.get(name).cloned()
}

/// A transformation's source compiled to the payload a plug-in is, under
/// its key ([`key_of`]): its display name, parameters (a complex one as
/// two, `name_re` and `name_im`, in the slots the WGSL reads), and its 2D
/// and 3D WGSL.
pub fn compile(source: &str) -> Result<VariationDownload, String> {
    let key = key_of(source).ok_or_else(|| {
        "a variation names itself: `const INFO = #{ id: \"...\" }` (the Formula Editor gives it one on Use)".to_string()
    })?;
    compile_as(source, &key)
}

/// [`compile`] under a key given -- the one a flame names, which its
/// transforms' variation keys and the WGSL function both carry.
pub fn compile_as(source: &str, key: &str) -> Result<VariationDownload, String> {
    let first = |d: &[lang::Diag]| {
        d.iter()
            .find(|x| x.severity == lang::Severity::Error)
            .map(|x| x.render(source))
            .unwrap_or_else(|| "it does not compile".to_string())
    };
    // Its 2D function, its 3D one (B2), or both.
    let (two, three) = (lang::rhai::has_function(source, "variation"), lang::rhai::has_function(source, "variation3"));
    if !two && !three {
        return Err("a variation has `fn variation(z, p)`, or `fn variation3(v, p)` for 3D".to_string());
    }
    let p2 = if two { Some(lang::rhai::transformation(source).map_err(|d| first(&d))?) } else { None };
    let p3 = if three { Some(lang::rhai::transformation3(source).map_err(|d| first(&d))?) } else { None };
    let p = p2.as_ref().or(p3.as_ref()).expect("a program");
    // The name `vary` calls it by, when it has one.
    if let Some(name) = lang::rhai::variation_name(source) {
        check_name(&name)?;
    }
    let name = key.to_string();
    if p2.iter().chain(p3.iter()).any(|p| !p.funcs.is_empty()) {
        return Err("a variation's function parameters are a later step".to_string());
    }
    let wgsl = lang::wgsl::variation(p2.as_ref(), p3.as_ref(), &name)?;
    #[cfg(not(target_arch = "wasm32"))]
    lang::wgsl::validate_variation(&wgsl).map_err(|e| format!("the compiler produced invalid WGSL (a bug): {e}"))?;

    record_layout(&name, source)?;
    // As every kind takes a script's (`ir::Program::param_defs`), on the
    // wire a downloaded variation's parameters are.
    let parameters: Vec<serde_json::Value> = param_defs(&p, source)?.iter().map(crate::params::api_json).collect();
    // Its flag for the renderer: orbits started afresh every dispatch.
    let mut features = wgsl.features.clone();
    if lang::rhai::info_bool(source, "resets_orbits") == Some(true) {
        features.push("resets_orbits");
    }
    // It replaces the working point (JWildfire's `=` against `+=`): moved
    // to pre or post it is assigned, and it runs last of the normal ones.
    if lang::rhai::info_bool(source, "replace") == Some(true) {
        features.push("replace");
    }
    // JWildfire's z rule, `INFO.z`: by default a 3D body's z is always kept
    // and a 2D body's passed-through z only when the flame preserves z
    // (`shader_builder_v2::zeroes_z`).
    if let Some(z) = lang::rhai::info_string(source, "z") {
        features.retain(|f| *f != "always_z" && *f != "never_z");
        match z.as_str() {
            "always" => features.push("always_z"),
            "never" => features.push("never_z"),
            _ => {}
        }
    }
    // Where the pickers group it, `INFO.category`; a 3D function alone is a
    // 3D variation only.
    let category = match lang::rhai::info_string(source, "category") {
        Some(c) if !two && c != "only_3d" => return Err(format!("a variation with no `fn variation` is `only_3d`, not `{c}`")),
        Some(c) => c,
        None if two => "plugin".to_string(),
        None => "only_3d".to_string(),
    };
    let payload = serde_json::json!({
        "id": format!("custom:{name}"),
        "name": name,
        "display_name": p.name,
        "category": category,
        "version": 1,
        // The wire's phase has no `any` (VARIATIONS_WIRE_FORMAT.md §4.2):
        // a variation of the user's takes it at registration ([`phase_of`]).
        "phase": match phase_of(source) {
            crate::variations::VariationPhase::Pre => "pre",
            crate::variations::VariationPhase::Post => "post",
            _ => "normal",
        },
        "parameters": parameters,
        "shader_2d": wgsl.wgsl_2d,
        "shader_3d": wgsl.wgsl_3d,
        // The hooks it uses (C2): `rand()`, `ctx.accum`, a colour, state;
        // and the renderer's flag.
        "features": features,
        "state_count": wgsl.state_count,
        "shader_state_init": wgsl.state_init,
        // `prepare`, its derived slots computed by the init pass.
        "init_param_count": wgsl.init_count,
        "shader_init": wgsl.init,
    });
    serde_json::from_value(payload).map_err(|e| format!("the variation's payload: {e}"))
}

/// A transformation's parameters as a variation's (a complex one as two,
/// `name_re` and `name_im`), those `INFO.slow` names slow: committed when
/// a drag ends, as a built-in's `slow`.
pub fn param_defs(p: &lang::ir::Program, source: &str) -> Result<Vec<crate::params::ParamDef>, String> {
    let mut defs = p.param_defs(lang::ir::ComplexParams::Split("_"))?;
    for s in lang::rhai::info_strings(source, "slow") {
        let d = defs.iter_mut().find(|d| d.name == s).ok_or_else(|| format!("`INFO.slow` names `{s}`, which is not in `PARAMS`"))?;
        d.slow = true;
    }
    Ok(defs)
}

/// The phase `INFO.phase` gives a variation source: normal without one,
/// `any` one the user may move to pre or post (a transform's
/// `variation_priorities`), as 295 of the built-ins with twins are.
pub fn phase_of(source: &str) -> crate::variations::VariationPhase {
    use crate::variations::VariationPhase as P;
    match lang::rhai::info_string(source, "phase").as_deref() {
        Some("any") => P::Any,
        Some("pre") => P::Pre,
        Some("post") => P::Post,
        _ => P::Normal,
    }
}

/// Compile a source and register it under its key, replacing what has that
/// key (the same version, again); refused when a built-in or a download
/// has it. Gives the key.
pub fn register(source: &str) -> Result<String, String> {
    let dl = compile(source)?;
    register_payload(dl, source)
}

fn register_payload(dl: VariationDownload, source: &str) -> Result<String, String> {
    let name = register_into(&mut crate::variations::global_registry_mut(), &dl, source)?;
    // So `ensure` does not register this source again.
    if let Ok(mut known) = registered().lock() {
        known.insert(name.clone(), source.to_string());
    }
    Ok(name)
}

/// Register `dl`, compiled from `source`, in `registry` -- the global one,
/// or a test's own -- under its key; refused when a built-in or a
/// download has it.
pub(crate) fn register_into(registry: &mut crate::variations::VariationRegistry, dl: &VariationDownload, source: &str) -> Result<String, String> {
    if let Some(existing) = registry.get(&dl.name) {
        if existing.provenance != Provenance::Local {
            return Err(format!("`{}` is taken by a {} variation", dl.name, existing.provenance.label()));
        }
    }
    registry.register_from_api(dl, Provenance::Local);
    // `any`, which the wire cannot say, from the source.
    if phase_of(source) == crate::variations::VariationPhase::Any {
        registry.set_phase(&dl.name, crate::variations::VariationPhase::Any);
    }
    if !registry.has(&dl.name) {
        return Err(format!("`{}` was refused at registration", dl.name));
    }
    // Whether it is a built-in's twin, unedited (the planner's data, a
    // `.flame` export).
    crate::variations::twins::note(&dl.name, source);
    Ok(dl.name.clone())
}

/// The source of the user's variation `name` -- its key, or the
/// `INFO.name` a formula's `vary` calls it by (the KIFS plan, C1e): as
/// registered, else a Rhai transformation in their formula library, else
/// among the examples.
pub fn source_of(name: &str) -> Option<String> {
    if let Some(s) = registered().lock().ok().and_then(|k| k.get(name).cloned()) {
        return Some(s);
    }
    use crate::storage::formula_store::{is_rhai, list, load, EXAMPLES};
    let named = |text: &str| {
        lang::rhai::kind_of(text) == Some(EntryKind::Transform)
            && (lang::rhai::variation_name(text).as_deref() == Some(name) || key_of(text).as_deref() == Some(name))
    };
    for file in list().into_iter().filter(|f| is_rhai(f)) {
        if let Some(text) = load(&file).filter(|t| named(t)) {
            return Some(text);
        }
    }
    EXAMPLES.iter().find(|(f, t)| is_rhai(f) && named(t)).map(|(_, t)| t.to_string())
}

/// A transformation's source as a flame carries it: under its key, with
/// the variations it calls (`vary`) beside it.
pub fn carried(source: &str) -> Option<CustomSource> {
    let key = key_of(source)?;
    let src = CustomSource { dialect: CustomDialect::Rhai, entry: key, source: source.to_string(), variations: Vec::new() };
    Some(crate::lang::rhai::with_called_variations(EntryKind::Transform, src))
}

/// When a config comes to use any of `names` that is a variation of the
/// user's its flame does not carry yet (a lens or a transformation picked
/// in the escape panel; the KIFS plan, C4), the change that carries it:
/// the flame's list with the source added.
pub fn carry_change<'a>(config: &crate::config::FractalConfig, names: impl IntoIterator<Item = &'a str>) -> Option<(crate::config::ConfigPath, crate::config::ConfigValue)> {
    let mut list = config.flame.custom_variations.clone();
    let mut added = false;
    for name in names {
        let custom = crate::variations::global_registry().get(name).is_some_and(|v| v.provenance == Provenance::Local);
        if !custom || list.iter().any(|s| s.entry == name) {
            continue;
        }
        // Under the name it is registered by, which the config uses.
        if let Some(mut src) = source_of(name).as_deref().and_then(carried) {
            src.entry = name.to_string();
            list.push(src);
            added = true;
        }
    }
    added.then(|| (crate::config::ConfigPath::FlameCustomVariations, crate::config::ConfigValue::String(serde_json::to_string(&list).unwrap_or_default())))
}

/// Whether a variation source needs an id minted before it is Used: its
/// `INFO.id` is not a UUID (typed by hand, a copy of a built-in's), or it
/// has neither an id nor the `INFO.name` one derives from.
pub fn needs_id(source: &str) -> bool {
    match lang::rhai::info_string(source, "id") {
        Some(id) => !crate::scripted::is_uuid(&id),
        None => lang::rhai::variation_name(source).is_none(),
    }
}

/// Move a config from every other version of the variation `id` to the
/// one keyed `key` (`scripted::key`): the flame's transforms (every
/// level, its weight and parameters with it), the escape-time lens and
/// transformations. Whether anything named an older version.
pub fn rekey(config: &mut crate::config::FractalConfig, id: &str, key: &str) -> bool {
    let older = |n: &str| n != key && crate::scripted::key_id(n).as_deref() == Some(id);
    let mut moved = false;
    let flame = &mut config.flame;
    for t in flame.transforms.iter_mut().chain(flame.linked_transforms.iter_mut()).chain(flame.final_transforms.iter_mut()) {
        let names: Vec<String> = t.variations.keys().filter(|n| older(n)).cloned().collect();
        for n in names {
            moved |= t.rename_variation(&n, key);
        }
    }
    if older(&config.escape.lens) {
        config.escape.lens = key.to_string();
        moved = true;
    }
    for t in &mut config.escape.transforms {
        if older(&t.variation) {
            t.variation = key.to_string();
            moved = true;
        }
    }
    moved
}

/// The registered custom variations' sources, by key, as last
/// registered: what [`ensure`] compares against.
fn registered() -> &'static Mutex<HashMap<String, String>> {
    static R: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Register a flame's custom variations (and its subflames'), before
/// anything builds its shader. Cheap when each is registered with this
/// source already; a source that does not compile is logged once and its
/// variation renders as an unknown name does.
pub fn ensure(flame: &Flame) {
    for src in all_sources(flame) {
        let Ok(mut known) = registered().lock() else { return };
        if known.get(&src.entry) == Some(&src.source) {
            continue;
        }
        // Remembered either way, so a failure is not retried every frame.
        known.insert(src.entry.clone(), src.source.clone());
        drop(known);
        // Against the variations it carries (`vary`), under the key the
        // flame names, which its transforms run.
        let carried = crate::lang::rhai::carried_variations(src);
        let result = lang::rhai::with_variations(&carried, || compile_as(&src.source, &src.entry).and_then(|dl| register_payload(dl, &src.source)));
        if let Err(e) = result {
            log::warn!("Custom variation `{}` did not register: {e}", src.entry);
        }
    }
}

/// Whether any transform of `flame` (or of its subflames) has the
/// variation `name`, at any weight.
pub fn used_by(flame: &Flame, name: &str) -> bool {
    let xforms = flame.transforms.iter().chain(&flame.linked_transforms).chain(&flame.final_transforms);
    xforms.into_iter().any(|t| t.variations.contains_key(name)) || flame.subflames.iter().any(|s| used_by(s, name))
}

fn all_sources(flame: &Flame) -> Vec<&CustomSource> {
    let mut out: Vec<&CustomSource> = flame.custom_variations.iter().collect();
    for sub in &flame.subflames {
        out.extend(all_sources(sub));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compiled under its `INFO.name`, so a test reads the WGSL by a name
    /// it knows; [`compile`] gives it its key.
    fn named(src: &str) -> VariationDownload {
        let name = lang::rhai::variation_name(src).expect("a name");
        compile_as(src, &name).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    /// A variation's `PARAMS` layout -- a condition and a grid -- is
    /// recorded for its panel (one parameter system, step 10); a grid's
    /// fill, a function only a simulation's CPU runs yet, is refused.
    #[test]
    fn a_variations_layout_is_its_panels() {
        let src = "const PARAMS = #{\n    mode: #{ value: 0, choices: [\"Plain\", \"Table\"] },\n    \
                   k: #{ value: 1.0, min: 0.0, max: 2.0, show_if: #{ mode: 0 } },\n    \
                   table: #{ type: \"grid\", rows: 2, cell: \"t{row}_{column}\", show_if: #{ mode: 1 }, columns: #{ a: #{ value: [1.0, 2.0], min: 0.0, max: 4.0 } } },\n};\n\
                   const INFO = #{ name: \"rhai_layout\" };\nfn variation(z, p) {\n    z * p.k * p.table[1].a\n}\n";
        named(src);
        let layout = param_layout("rhai_layout").expect("a layout");
        let at = |mode: f32| move |n: &str| if n == "mode" { mode } else { 0.0 };
        assert!(layout.lists("k", &at(0.0)) && !layout.lists("k", &at(1.0)));
        assert!(!layout.lists("t0_a", &at(1.0)), "a grid's cells are the grid's");
        assert_eq!((layout.grids.len(), layout.grids[0].rows, layout.grids[0].show_if.holds(&at(1.0))), (1, 2, true));
        let filled = src
            .replace("show_if: #{ mode: 1 },", "show_if: #{ mode: 1 }, fill: #{ when: #{ mode: 1 }, from: \"ladder\" },")
            .replace("fn variation", "fn ladder(p, row) {\n    #{ a: 1.0 }\n}\nfn variation");
        let e = compile_as(&filled, "rhai_layout_fill").unwrap_err();
        assert!(e.contains("a simulation's alone"), "{e}");
    }

    /// A grid's cell at a row the variation computes (one parameter
    /// system, step 10): its WGSL reads the transform's slot the row
    /// gives, clamped to the grid, and the CPU reads the same parameter.
    #[test]
    fn a_variation_reads_a_grid_row_it_computes() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{transform, ParamValues};
        let src = "const PARAMS = #{\n    pick: #{ value: 1, min: 0, max: 2 },\n    \
                   table: #{ type: \"grid\", rows: 3, cell: \"t{row}_{column}\", columns: #{ k: #{ value: [0.5, 2.0, 3.0], min: 0.0, max: 4.0 } } },\n};\n\
                   const INFO = #{ name: \"rhai_grid_row\" };\nfn variation(z, p) {\n    z * p.table[p.pick].k\n}\n";
        let dl = named(src);
        let names: Vec<&str> = dl.parameters.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["pick", "t0_k", "t1_k", "t2_k"]);
        let wgsl = dl.shader_2d.as_deref().unwrap_or_default();
        assert!(wgsl.contains("get_param(xform_id, variation_id, u32(1i + clamp("), "{wgsl}");
        let p = lang::rhai::transformation(src).unwrap();
        let mut values = ParamValues::defaults(&p);
        assert_eq!(transform(&p, &values, C64::new(1.0, 0.5)), C64::new(2.0, 1.0), "row 1");
        values.values[0].0 = 7.0;
        assert_eq!(transform(&p, &values, C64::new(1.0, 0.5)), C64::new(3.0, 1.5), "clamped to the last row");
    }

    /// A source registers under its id and version: two versions are two
    /// keys of one id, a source from before ids takes its name's derived
    /// one, and an `INFO.id` is its own.
    #[test]
    fn a_variation_is_keyed_by_its_id_and_version() {
        let v1 = "const INFO = #{ name: \"rhai_keyed_test\" };\nfn variation(z, p) { z * 0.5 }\n";
        let v2 = v1.replace("0.5", "0.25");
        let (k1, k2) = (compile(v1).unwrap().name, compile(&v2).unwrap().name);
        assert!(crate::scripted::is_key(&k1), "{k1}");
        assert_ne!(k1, k2);
        let derived = crate::scripted::derived_id("rhai_keyed_test");
        assert_eq!(crate::scripted::key_id(&k1).as_deref(), Some(derived.as_str()));
        assert_eq!(crate::scripted::key_id(&k2).as_deref(), Some(derived.as_str()));
        assert!(compile(v1).unwrap().shader_2d.unwrap().contains(&format!("fn variation_{k1}(p: vec2<f32>)")));
        let id = "5b0d6f0e-3c1a-4e8b-9f5d-2a7c4e6b8d10";
        let own = format!("const INFO = #{{ id: \"{id}\", name: \"rhai_keyed_test\" }};\nfn variation(z, p) {{ z * 0.5 }}\n");
        assert_eq!(crate::scripted::key_id(&key_of(&own).unwrap()).as_deref(), Some(id));
        // An id alone names it too.
        let bare = format!("const INFO = #{{ id: \"{id}\" }};\nfn variation(z, p) {{ z }}\n");
        assert!(compile(&bare).is_ok());
        assert_eq!(super::carried(&bare).map(|c| c.entry), key_of(&bare));
        assert!(!needs_id(v1) && !needs_id(&own), "a name derives one; a UUID is one");
        assert!(needs_id("const INFO = #{ id: \"my_swirl\" };\nfn variation(z, p) { z }\n"), "typed by hand");
        assert!(needs_id("fn variation(z, p) { z }\n"), "none at all");
    }

    /// A picker offers one version of each id: the one the flame carries,
    /// else the last registered; built-ins and other ids are untouched.
    #[test]
    fn a_picker_offers_one_version_of_each() {
        let v1 = "const INFO = #{ name: \"rhai_pick_test\" };\nfn variation(z, p) { z * 0.5 }\n";
        let v2 = v1.replace("0.5", "0.25");
        let other = "const INFO = #{ name: \"rhai_pick_other\" };\nfn variation(z, p) { z }\n";
        let mut registry = crate::variations::VariationRegistry::new();
        let (d1, d2, d3) = (compile(v1).unwrap(), compile(&v2).unwrap(), compile(other).unwrap());
        for d in [&d1, &d2, &d3] {
            registry.register_from_api(d, Provenance::Local);
        }
        let hidden = registry.superseded(&[]);
        assert_eq!(hidden.into_iter().collect::<Vec<_>>(), [d1.name.clone()], "the last registered");
        let carried = [CustomSource { dialect: CustomDialect::Rhai, entry: d1.name.clone(), source: v1.to_string(), variations: Vec::new() }];
        assert_eq!(registry.superseded(&carried).into_iter().collect::<Vec<_>>(), [d2.name.clone()], "the flame's");
    }

    /// Use of a new version moves the config to it: every transform's
    /// weight, parameters and order, the lens and the escape
    /// transformations; another id and the built-ins stay.
    #[test]
    fn a_new_version_takes_over_from_the_old() {
        let id = crate::scripted::derived_id("rhai_rekey_test");
        let (old, new) = (crate::scripted::key(&id, "a"), crate::scripted::key(&id, "b"));
        let other = crate::scripted::key(&crate::scripted::derived_id("rhai_rekey_other"), "a");
        let mut config = crate::config::FractalConfig::default();
        config.flame.transforms.push(crate::scene::transforms::Transform::default());
        let t = &mut config.flame.transforms[0];
        t.variations.insert(old.clone(), 0.75);
        t.variations.insert(other.clone(), 0.5);
        t.variation_params.insert(format!("{old}.k"), 2.0);
        t.variation_order = vec!["linear".into(), old.clone(), other.clone()];
        config.flame.final_transforms.push(config.flame.transforms[0].clone());
        config.escape.lens = old.clone();
        config.escape.transforms.push(crate::config::escape::EscapeTransform::new(Default::default(), &old, 1.0));
        assert!(rekey(&mut config, &id, &new));
        for t in config.flame.transforms.iter().take(1).chain(config.flame.final_transforms.iter()) {
            assert_eq!(t.variations.get(&new), Some(&0.75));
            assert!(!t.variations.contains_key(&old));
            assert_eq!(t.variations.get(&other), Some(&0.5));
            assert_eq!(t.variation_params.get(&format!("{new}.k")), Some(&2.0));
            assert_eq!(t.variation_order, ["linear".to_string(), new.clone(), other.clone()]);
        }
        assert_eq!(config.escape.lens, new);
        assert_eq!(config.escape.transforms[0].variation, new);
        assert!(!rekey(&mut config, &id, &new), "nothing older left");
    }

    /// Twins of five built-ins, as their sources (the plan's C1 gates).
    pub(crate) const TWINS: &[(&str, &str)] = &[
        (
            "linear",
            "const INFO = #{ title: \"Linear (Rhai)\", name: \"rhai_linear\" };\nfn variation(z, p) {\n    z\n}\n",
        ),
        (
            "spherical",
            "const INFO = #{ title: \"Spherical (Rhai)\", name: \"rhai_spherical\" };\n\
             fn variation(z, p) {\n    z / (z.norm_sqr() + 1e-6)\n}\n",
        ),
        (
            "swirl",
            "const INFO = #{ title: \"Swirl (Rhai)\", name: \"rhai_swirl\" };\n\
             fn variation(z, p) {\n    let r2 = z.norm_sqr();\n    let s = sin(r2);\n    let c = cos(r2);\n    \
             complex(z.re * s - z.im * c, z.re * c + z.im * s)\n}\n",
        ),
        (
            "polar",
            "const INFO = #{ title: \"Polar (Rhai)\", name: \"rhai_polar\" };\n\
             fn variation(z, p) {\n    complex(atan2(z.re, z.im) / PI(), z.abs() - 1.0)\n}\n",
        ),
        (
            "horseshoe",
            "const INFO = #{ title: \"Horseshoe (Rhai)\", name: \"rhai_horseshoe\" };\n\
             fn variation(z, p) {\n    let r_inv = 1.0 / (z.abs() + 1e-6);\n    complex((z.re - z.im) * (z.re + z.im) * r_inv, 2.0 * z.re * z.im * r_inv)\n}\n",
        ),
    ];

    /// Each twin compiles to a payload with WGSL that validates, a name
    /// of its own and no parameters -- so its function takes the point
    /// alone, as the flame shader calls a variation without parameters;
    /// a parameter of each kind becomes the payload's, a complex one as
    /// two, and the function takes the ids.
    #[test]
    fn a_transformation_compiles_to_a_variations_payload() {
        for (builtin, src) in TWINS {
            let dl = named(src);
            assert_eq!(dl.name, format!("rhai_{builtin}"));
            assert!(dl.shader_2d.as_deref().is_some_and(|s| s.contains(&format!("fn variation_rhai_{builtin}(p: vec2<f32>) -> vec2<f32>"))));
            assert!(dl.shader_3d.as_deref().is_some_and(|s| s.contains(&format!("fn variation_rhai_{builtin}(p: vec3<f32>) -> vec3<f32>"))));
            assert!(dl.parameters.is_empty());
        }
        let src = "const PARAMS = #{\n    k: #{ value: 0.5, min: 0.0, max: 2.0, caption: \"Amount\" },\n    c: #{ value: complex(0.1, -0.2) },\n    \
                   n: #{ value: 3, min: 1, max: 9 },\n    flip: #{ value: false },\n};\nconst INFO = #{ title: \"Params\", name: \"rhai_params\" };\n\
                   fn variation(z, p) {\n    let w = z * p.k + p.c * to_float(p.n);\n    if p.flip { -w } else { w }\n}\n";
        let dl = named(src);
        let names: Vec<&str> = dl.parameters.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["k", "c_re", "c_im", "n", "flip"]);
        assert!(dl.shader_2d.as_deref().is_some_and(|s| s.contains("get_param(xform_id, variation_id, 2u)")), "c's imaginary part in slot 2");
        assert!(dl.shader_2d.as_deref().is_some_and(|s| s.contains("fn variation_rhai_params(p: vec2<f32>, xform_id: u32, variation_id: u32)")));
        // An angle parameter is the registry's angle (degrees, its control).
        let angled = "const PARAMS = #{ turn: #{ value: 30.0, min: 0.0, max: 360.0, type: \"angle\" } };\nconst INFO = #{ name: \"rhai_turn\" };\nfn variation(z, p) { z * p.turn }\n";
        let dl = named(angled);
        assert!(matches!(dl.parameters[0].param_type, crate::api::types::ApiParamType::Angle), "{:?}", dl.parameters[0].param_type);
        // Refused: no name, a name that is not one, the image's symbols.
        assert!(compile("fn variation(z, p) { z }").unwrap_err().contains("names itself"));
        assert!(compile("const INFO = #{ name: \"Bad Name\" };\nfn variation(z, p) { z }").unwrap_err().contains("not a variation name"));
        let e = compile("const INFO = #{ name: \"rhai_bad\" };\nfn variation(z, p, ctx) { z + ctx.pixel }").unwrap_err();
        assert!(!e.is_empty());
    }

    /// The interpreter runs a twin as the built-in's formula: the CPU
    /// side of what the GPU tests render.
    #[test]
    fn the_twins_run_as_their_formulas() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{transform, ParamValues};
        let at = |builtin: &str, z: C64| {
            let src = TWINS.iter().find(|(n, _)| *n == builtin).unwrap().1;
            let p = lang::rhai::transformation(src).unwrap();
            transform(&p, &ParamValues::defaults(&p), z)
        };
        for z in [C64::new(0.3, -0.7), C64::new(-1.2, 0.4), C64::new(2.0, 1.5)] {
            let (x, y) = (z.re, z.im);
            let r2 = x * x + y * y;
            let r = r2.sqrt();
            let close = |a: C64, b: (f64, f64)| (a.re - b.0).abs() < 1e-12 && (a.im - b.1).abs() < 1e-12;
            assert!(close(at("linear", z), (x, y)));
            assert!(close(at("spherical", z), (x / (r2 + 1e-6), y / (r2 + 1e-6))));
            assert!(close(at("swirl", z), (x * r2.sin() - y * r2.cos(), x * r2.cos() + y * r2.sin())));
            assert!(close(at("polar", z), (x.atan2(y) / std::f64::consts::PI, r - 1.0)));
            let rr = 1.0 / (r + 1e-6);
            assert!(close(at("horseshoe", z), ((x - y) * (x + y) * rr, 2.0 * x * y * rr)));
        }
    }

    /// A `.flame` export drops a custom variation, weight and parameters
    /// (JWildfire has no such variation), and keeps the rest of the
    /// transform; the `.fflame` keeps its source. Known by the flame
    /// carrying it: unit tests never write the global registry, whose
    /// readers run in parallel (a writer waiting behind one test's read
    /// guard blocks the next read, and a test re-reading under its guard
    /// deadlocks -- `every_variation_validates_in_the_layer_warp` does).
    #[test]
    fn a_flame_export_drops_a_custom_variation() {
        let src = "const PARAMS = #{ k: #{ value: 0.5 } };\nconst INFO = #{ title: \"Export\", name: \"rhai_export_test\" };\n\
                   fn variation(z, p) {\n    z * p.k\n}\n";
        let mut cfg = crate::config::FractalConfig::default();
        cfg.flame.transforms.push(crate::scene::transforms::Transform::default());
        let t = cfg.flame.transforms.last_mut().unwrap();
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("swirl", 0.25);
        t.set_variation("rhai_export_test", 0.75);
        t.set_variation_param("rhai_export_test", "k", 0.3);
        cfg.flame.custom_variations = vec![CustomSource { dialect: crate::config::escape::CustomDialect::Rhai, entry: "rhai_export_test".into(), source: src.into(), variations: Vec::new() }];
        let xml = crate::flame_xml::write_flame_xml(&cfg);
        assert!(xml.contains(" swirl=\"0.25\""), "{xml}");
        assert!(!xml.contains("rhai_export_test"), "{xml}");
        let back: crate::config::FractalConfig = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back.flame.custom_variations, cfg.flame.custom_variations, "the .fflame carries the source");
    }

    // ---- a variation called inside a formula (C1e) ----------------------

    /// A variation with parameters of two kinds, that reassigns its point.
    const BEND: &str = "const PARAMS = #{\n    k: #{ value: 0.3 },\n    shift: #{ value: complex(0.1, 0.0) },\n};\n\
                        const INFO = #{ title: \"Bend\", name: \"rhai_bend_test\" };\n\
                        fn variation(z, p) {\n    let r2 = z.norm_sqr();\n    z = z * complex(cos(p.k * r2), sin(p.k * r2));\n    z + p.shift\n}\n";

    /// A Mandelbrot whose step bends through `rhai_bend_test`, its `k` the
    /// formula's `twist` and its shift the default -- and the same
    /// formula with the variation written out.
    const VARIED: &str = "const PARAMS = #{ bailout: 4.0, twist: 0.5 };\nconst INFO = #{ title: \"Varied\" };\n\
                          fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\n\
                          fn step(s, p) {\n    s.z = vary(\"rhai_bend_test\", s.z * s.z + s.c, #{ k: p.twist });\n    s\n}\n\
                          fn escaped(s, p) { s.z.norm_sqr() > p.bailout }\n";
    const WRITTEN_OUT: &str = "const PARAMS = #{ bailout: 4.0, twist: 0.5 };\nconst INFO = #{ title: \"Written Out\" };\n\
                               fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\n\
                               fn step(s, p) {\n    let w = s.z * s.z + s.c;\n    let r2 = w.norm_sqr();\n    \
                               w = w * complex(cos(p.twist * r2), sin(p.twist * r2));\n    s.z = w + complex(0.1, 0.0);\n    s\n}\n\
                               fn escaped(s, p) { s.z.norm_sqr() > p.bailout }\n";

    fn bend_scope() -> Vec<(String, String)> {
        vec![("rhai_bend_test".to_string(), BEND.to_string())]
    }

    /// A formula that calls a variation runs as the formula with the
    /// variation written out, pixel by pixel, in the interpreter; deep
    /// zoom derives its equations and the derivative as for that one.
    #[test]
    fn a_formula_calls_a_variation_as_written_out() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{run, Env, ParamValues};
        let varied = lang::rhai::with_variations(&bend_scope(), || lang::rhai::program(VARIED)).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(VARIED)).collect::<Vec<_>>()));
        let written = lang::rhai::program(WRITTEN_OUT).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(WRITTEN_OUT)).collect::<Vec<_>>()));
        let (pv, pw) = (ParamValues::defaults(&varied), ParamValues::defaults(&written));
        let mut escaped = 0;
        for i in 0..24 {
            for j in 0..24 {
                let px = C64::new(-2.0 + 2.5 * i as f64 / 23.0, -1.25 + 2.5 * j as f64 / 23.0);
                let (a, b) = (run(&varied, &pv, None, &Env::at(px, 200)), run(&written, &pw, None, &Env::at(px, 200)));
                assert_eq!((a.n, a.escaped), (b.n, b.escaped), "at {px:?}");
                assert!((a.z.re - b.z.re).abs() < 1e-12 && (a.z.im - b.z.im).abs() < 1e-12, "at {px:?}: {:?} against {:?}", a.z, b.z);
                escaped += a.escaped as u32;
            }
        }
        assert!(escaped > 100 && escaped < 24 * 24, "the view holds both: {escaped} escaped");
        assert!(lang::delta::with_derived_equations(&varied).is_some(), "deep zoom derives through the variation");
        assert_eq!(lang::derive::with_derivative(&varied).is_some(), lang::derive::with_derivative(&written).is_some());
    }

    /// What `vary` refuses: a name that is no variation of the user's, a
    /// built-in's (a later step), a parameter the variation lacks, a
    /// form that is not one.
    /// A solid calls a built-in variation's 3D body (3d-look-and-tools
    /// P5, C3): `vary("name", v)` of a vector is one map call, its
    /// parameters numbers or the solid's own, and its WGSL calls the maps
    /// flame (`esc_solid_map`); and it is refused where it cannot be -- in
    /// `prepare`, of a variation of the user's, with a value per pixel, of
    /// a parameter it does not have.
    #[test]
    fn a_solid_calls_a_built_in_variation() {
        use crate::lang::ir::{MapCall, MapValue};
        let solid = |step: &str, extra: &str| {
            format!(
                "const PARAMS = #{{ k: #{{ value: 0.5 }} }};\nconst INFO = #{{ title: \"t\", radius: 2.0 }};\n{extra}\n\
                 fn start(pos, p) {{ #{{ z: pos }} }}\nfn step(s, p) {{ {step} s }}\nfn distance(s, p) {{ length(s.z) - 1.0 }}"
            )
        };
        let src = solid("s.z = vary(\"linear3D\", s.z) * 0.9; s.z = vary(\"julia3D\", s.z, #{ power: p.k }); s.z = vary(\"julia3D\", s.z, #{ power: -3 });", "");
        let p = crate::lang::rhai::solid(&src).unwrap_or_else(|d| panic!("{}", d[0].message));
        assert_eq!(
            p.maps,
            vec![
                MapCall { variation: "linear3D".into(), params: vec![] },
                MapCall { variation: "julia3D".into(), params: vec![("power".into(), MapValue::Param("k".into()))] },
                MapCall { variation: "julia3D".into(), params: vec![("power".into(), MapValue::Const(-3.0))] },
            ]
        );
        let base = crate::escape::ifs::template_parameters().len();
        let w = crate::lang::wgsl::solid(&p, base, base + p.param_slots()).expect("WGSL");
        for k in 0..3 {
            assert!(w.contains(&format!("esc_solid_map({k}u, vec3<f32>(")), "call {k}:\n{w}");
        }
        crate::lang::wgsl::validate_solid(&w).expect("validates");
        let refused = |src: String, says: &str| {
            let d = crate::lang::rhai::solid(&src).err().unwrap_or_else(|| panic!("compiled: {src}"));
            assert!(d.iter().any(|x| x.message.contains(says)), "{says}: {:?}", d.iter().map(|x| &x.message).collect::<Vec<_>>());
        };
        refused(solid("", "fn prepare(p) { #{ q: vary(\"linear3D\", vec3(1.0, 0.0, 0.0)) } }"), "GPU");
        refused(solid("s.z = vary(\"no_such_thing\", s.z);", ""), "not a built-in variation with a 3D body");
        refused(solid("s.z = vary(\"julia3D\", s.z, #{ power: s.z.x });", ""), "per picture");
        refused(solid("s.z = vary(\"julia3D\", s.z, #{ weight: 2.0 });", ""), "has no parameter `weight`");
    }

    #[test]
    fn vary_refuses_what_it_cannot_inline() {
        let msg = |src: &str| match lang::rhai::with_variations(&bend_scope(), || lang::rhai::program(src)) {
            Ok(_) => panic!("compiled: {src}"),
            Err(d) => d.iter().map(|x| x.message.clone()).collect::<Vec<_>>().join("; "),
        };
        let with = |step: &str| {
            format!("const PARAMS = #{{ bailout: 4.0 }};\nfn start(pixel, p) {{ #{{ z: complex(0.0, 0.0), c: pixel }} }}\nfn step(s, p) {{ {step} s }}\nfn escaped(s, p) {{ s.z.norm_sqr() > p.bailout }}\n")
        };
        assert!(msg(&with("s.z = vary(\"rhai_nowhere_test\", s.z);")).contains("no variation of yours"));
        assert!(msg(&with("s.z = vary(\"spherical\", s.z);")).contains("built-in"));
        assert!(msg(&with("s.z = vary(\"rhai_bend_test\", s.z, #{ q: 1.0 });")).contains("no parameter `q`"));
        assert!(msg(&with("s.z = vary(s.z);")).contains("vary(\"name\", z)"));
    }

    /// An entry that calls a variation carries it: filled from where it
    /// is registered, compiled from what it carries alone, and named
    /// anew when the variation changes.
    #[test]
    fn an_entry_carries_the_variations_it_calls() {
        use crate::escape::custom::{check_entry, name_of, with_called_variations};
        let bend = BEND.replace("rhai_bend_test", "rhai_bend_carried");
        let varied = VARIED.replace("rhai_bend_test", "rhai_bend_carried");
        // Where `source_of` finds a registered one (never the registry
        // itself in a unit test: see the export test).
        registered().lock().unwrap().insert("rhai_bend_carried".into(), bend.clone());
        let plain = CustomSource { dialect: CustomDialect::Rhai, entry: "Varied".into(), source: varied, variations: Vec::new() };
        let src = with_called_variations(EntryKind::Formula, plain.clone());
        assert_eq!(src.variations.len(), 1);
        assert_eq!((src.variations[0].entry.as_str(), src.variations[0].source.as_str()), ("rhai_bend_carried", bend.as_str()));
        // Carried: compiles with no registry to look in.
        let mut moved = src.clone();
        for v in &mut moved.variations {
            v.entry = "rhai_bend_moved".into();
            v.source = v.source.replace("rhai_bend_carried", "rhai_bend_moved");
        }
        moved.source = moved.source.replace("rhai_bend_carried", "rhai_bend_moved");
        assert!(check_entry(EntryKind::Formula, &moved).is_ok(), "compiles from what it carries");
        // A changed variation is a changed entry.
        let mut changed = src.clone();
        changed.variations[0].source = changed.variations[0].source.replace("value: 0.3", "value: 0.4");
        assert_ne!(name_of(EntryKind::Formula, &src), name_of(EntryKind::Formula, &changed));
        assert_ne!(name_of(EntryKind::Formula, &src), name_of(EntryKind::Formula, &plain));
        // A variation calling a variation carries both, flat.
        let outer = "const INFO = #{ title: \"Outer\", name: \"rhai_outer_test\" };\nfn variation(z, p) {\n    vary(\"rhai_bend_carried\", z) * 0.5\n}\n";
        let c = super::carried(outer).expect("named");
        assert_eq!(c.variations.iter().map(|v| v.entry.as_str()).collect::<Vec<_>>(), ["rhai_bend_carried"]);
    }

    // ---- the hooks (C2) ----------------------------------------------------

    /// Twins of built-ins that use the hooks: `blur` draws (C2a);
    /// `macmillan` reads the accumulator and its weight (C2b), keeps state
    /// that its parameters seed (C2c) and writes a colour (C2d).
    pub(crate) const HOOKED_TWINS: &[(&str, &str)] = &[
        (
            "blur",
            "const INFO = #{ title: \"Blur (Rhai)\", name: \"rhai_blur\" };\n\
             fn variation(z, p) {\n    let theta = rand() * 6.28318530718;\n    let r = rand();\n    complex(r * cos(theta), r * sin(theta))\n}\n",
        ),
        (
            "macmillan",
            "const PARAMS = #{\n    a: #{ value: 1.6, min: -10.0, max: 10.0 },\n    b: #{ value: 0.4, min: -10.0, max: 10.0 },\n    \
             N: #{ value: 1.0, min: -10.0, max: 10.0 },\n    startx: #{ value: 0.1, min: -10.0, max: 10.0 },\n    \
             starty: #{ value: 0.1, min: -10.0, max: 10.0 },\n};\n\
             const INFO = #{ title: \"MacMillan (Rhai)\", name: \"rhai_macmillan\" };\n\
             fn init(p) { #{ xa: p.startx, x: p.startx, y: p.starty } }\n\
             fn variation(z, p, ctx) {\n    let xa = this.xa;\n    let x = this.x;\n    let y = this.y;\n    \
             x = y;\n    y = -xa + 2.0 * p.a * (y / (1.0 + y * y)) + p.b * y;\n    xa = x;\n    \
             x = y;\n    y = -xa + 2.0 * p.a * (y / (1.0 + y * y)) + p.b * y;\n    \
             let fp = ctx.accum + complex(x, y) * ctx.weight;\n    let s = abs(0.5 * (fp.re + fp.im + 1.0));\n    \
             this.xa = x;\n    this.x = x;\n    this.y = y;\n    #{ z: complex(x, y), color: s - floor(s) }\n}\n",
        ),
    ];

    fn hooked(builtin: &str) -> &'static str {
        HOOKED_TWINS.iter().find(|(n, _)| *n == builtin).unwrap().1
    }

    /// Each hook declares the built-in's `Feature`, takes its argument in
    /// the shader builder's order, and validates; the state's slots and
    /// the statement that seeds them come with it.
    /// `resets_orbits: true` in INFO is the renderer's flag
    /// (`Feature::ResetsOrbits`): Dynamic System carries it, and a flame
    /// using it at a weight other than 0 needs fresh orbits -- in a local
    /// registry, as a test writing the global one can deadlock the suite.
    #[test]
    fn a_variation_declares_that_it_resets_orbits() {
        let (_, source) = crate::storage::formula_store::EXAMPLES.iter().find(|(f, _)| *f == "variation-dynamic-system.rhai").unwrap();
        let dl = named(source);
        assert!(dl.features.iter().any(|f| f == "resets_orbits"), "{:?}", dl.features);
        let whorl = crate::storage::formula_store::EXAMPLES.iter().find(|(f, _)| *f == "variation-whorl.rhai").unwrap().1;
        assert!(!compile(whorl).unwrap().features.iter().any(|f| f == "resets_orbits"), "Whorl's orbits persist");
        assert!(compile("const INFO = #{ name: \"rhai_x\", resets_orbits: 1 };\nfn variation(z, p) { z }\n").unwrap_err().contains("true or false"));

        let mut registry = crate::variations::VariationRegistry::new();
        registry.register_from_api(&dl, Provenance::Local);
        let mut flame = crate::scene::transforms::Flame::default();
        flame.transforms = vec![crate::scene::transforms::Transform::default()];
        flame.transforms[0].variations.clear();
        flame.transforms[0].variations.insert("linear".into(), 1.0);
        assert!(!flame.needs_fresh_orbits(&registry));
        flame.transforms[0].variations.insert(dl.name.clone(), 0.0);
        assert!(!flame.needs_fresh_orbits(&registry), "at weight 0");
        flame.transforms[0].variations.insert(dl.name.clone(), 0.5);
        assert!(flame.needs_fresh_orbits(&registry));
        assert_eq!(flame.fresh_orbits_variation(&registry).as_deref(), Some(dl.name.as_str()));
        // Its Orbit Length is the flame's orbits' length: the default, the
        // transform's value, and no longer than a dispatch may be.
        assert_eq!(flame.orbit_length_source(&registry), Some((dl.name.clone(), 1024)));
        flame.transforms[0].set_variation_param(&dl.name, crate::variations::ORBIT_LENGTH_PARAM, 3000.0);
        assert_eq!(flame.orbit_length_source(&registry).map(|(_, n)| n), Some(3000));
        flame.transforms[0].set_variation_param(&dl.name, crate::variations::ORBIT_LENGTH_PARAM, 50_000.0);
        assert_eq!(flame.orbit_length_source(&registry).map(|(_, n)| n), Some(crate::variations::MAX_ORBIT_LENGTH));
        let mut parent = crate::scene::transforms::Flame::default();
        parent.subflames.push(flame.clone());
        assert!(parent.needs_fresh_orbits(&registry), "in a subflame");
        assert!(parent.orbit_length_source(&registry).is_some(), "in a subflame");
        flame.transforms[0].variations.insert(dl.name.clone(), 0.0);
        assert_eq!(flame.orbit_length_source(&registry), None, "at weight 0");
    }

    #[test]
    fn the_hooks_compile_to_the_builtins_features() {
        let blur = named(hooked("blur"));
        assert_eq!(blur.features, ["needs_rng"]);
        assert!(blur.shader_2d.as_deref().is_some_and(|s| s.contains("fn variation_rhai_blur(p: vec2<f32>, rng: ptr<function, RngState>)") && s.contains("rng_nextf(rng)")));
        assert_eq!(blur.state_count, 0);
        let mac = named(hooked("macmillan"));
        assert_eq!(mac.features, ["needs_accum", "writes_color"]);
        let s2 = mac.shader_2d.as_deref().unwrap();
        assert!(s2.contains("fn variation_rhai_macmillan(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>)"), "{s2}");
        assert!(mac.shader_3d.as_deref().is_some_and(|s| s.contains("accum: vec3<f32>") && s.contains("accum.xy")));
        assert_eq!(mac.state_count, 3);
        assert!(mac.shader_state_init.as_deref().is_some_and(|s| s.contains("cv_rhai_macmillan_variation_init(xform_id, variation_id)")));
        // State without parameters still takes the ids: needs_transform.
        let counter = "const INFO = #{ name: \"rhai_counter_test\" };\nfn init(p) { #{ n: 0.0 } }\n\
                       fn variation(z, p) {\n    this.n = this.n + 1.0;\n    z * (1.0 / this.n)\n}\n";
        let c = named(counter);
        assert_eq!(c.features, ["needs_transform"]);
        assert_eq!(c.state_count, 1);
        // What the hooks refuse.
        let refused = |src: &str| compile(src).err().unwrap_or_else(|| panic!("compiled: {src}"));
        assert!(refused("const INFO = #{ name: \"rhai_bad\" };\nfn init(p) { #{ n: rand() } }\nfn variation(z, p) { z * this.n }\n").contains("no random numbers"));
        assert!(refused("const INFO = #{ name: \"rhai_bad\" };\nfn variation(z, p) { z * this.n }\n").contains("no `init`"));
        assert!(refused("const INFO = #{ name: \"rhai_bad\" };\nfn variation(z, p, ctx) { z * ctx.julia }\n").contains("`accum`, `weight`, `color` and `hide`"));
        assert!(refused("const INFO = #{ name: \"rhai_bad\" };\nfn variation(z, p) { #{ z: z, hue: 0.5 } }\n").contains("`z`, `color` and `hide`"));
    }

    /// The MacMillan twin runs as the built-in's arithmetic (its WGSL,
    /// written out in f64), call after call: the state kept, the
    /// accumulator and the weight read, the colour written.
    #[test]
    fn the_macmillan_twin_runs_as_the_builtin() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{ParamValues, VariationRun};
        let p = lang::rhai::transformation(hooked("macmillan")).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.message.clone()).collect::<Vec<_>>()));
        let params = ParamValues::defaults(&p);
        let mut run = VariationRun::new(&p, &params, 7);
        let (a, b) = (1.6f64, 0.4f64);
        let (mut xa, mut y) = (0.1f64, 0.1f64);
        let mut x: f64;
        for i in 0..50 {
            let accum = C64::new(0.3 * (i as f64).sin(), -0.2);
            let weight = 0.75;
            x = y;
            y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;
            xa = x;
            x = y;
            y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;
            let (fpx, fpy) = (accum.re + weight * x, accum.im + weight * y);
            let s = (0.5 * (fpx + fpy + 1.0)).abs();
            let color = s - s.floor();
            xa = x;
            let (point, c) = run.call(C64::new(9.0, 9.0), accum, weight);
            assert!((point.re - x).abs() < 1e-12 && (point.im - y).abs() < 1e-12, "call {i}: {point:?} against ({x}, {y})");
            assert!(c.is_some_and(|c| (c - color).abs() < 1e-12), "call {i}: colour {c:?} against {color}");
        }
    }

    /// `rand()` draws the flame's stream in a variation (the shader's
    /// PCG, in [0, 1)), and a stream per pixel in a formula: the same
    /// pixel draws the same numbers, another pixel others; an entry that
    /// draws has no deep zoom.
    #[test]
    fn rand_draws_a_stream() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{run, Env, ParamValues, VariationRun};
        let p = lang::rhai::transformation(hooked("blur")).unwrap();
        let params = ParamValues::defaults(&p);
        let mut v = VariationRun::new(&p, &params, 12345);
        let mut mean_r = 0.0;
        for _ in 0..4000 {
            let (z, _) = v.call(C64::ZERO, C64::ZERO, 1.0);
            let r = z.abs();
            assert!((0.0..1.0).contains(&r));
            mean_r += r / 4000.0;
        }
        assert!((mean_r - 0.5).abs() < 0.02, "the radius is uniform: mean {mean_r}");
        // In a formula: the step adds a draw; where it escapes is the
        // stream's, the same at the same pixel.
        let src = "const PARAMS = #{ bailout: 4.0 };\nfn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\n\
                   fn step(s, p) { s.z = complex(rand(), 0.0); s }\nfn escaped(s, p) { s.z.re > 0.9 }\n";
        let f = lang::rhai::program(src).unwrap();
        let fp = ParamValues::defaults(&f);
        let at = |x: i32, y: i32| {
            let mut env = Env::at(C64::ZERO, 1000);
            (env.x, env.y) = (x, y);
            run(&f, &fp, None, &env).n
        };
        assert_eq!(at(3, 4), at(3, 4));
        let counts: std::collections::BTreeSet<u32> = (0..40).map(|i| at(i, 7)).collect();
        assert!(counts.len() > 5, "pixels draw apart: {counts:?}");
        assert!(lang::perturb::perturbation(&f).is_err(), "no deep zoom for an entry that draws");
    }

    /// `vary` inlines a variation that uses the hooks: its draws are the
    /// formula's stream, its context the identity's (no earlier
    /// variations, weight 1), its result map its point; state is refused.
    #[test]
    fn vary_inlines_a_hooked_variation() {
        let scope = vec![
            ("rhai_ctx_test".to_string(), "const INFO = #{ name: \"rhai_ctx_test\" };\nfn variation(z, p, ctx) {\n    #{ z: z * ctx.weight + ctx.accum, color: 0.5 }\n}\n".to_string()),
            ("rhai_macmillan".to_string(), hooked("macmillan").to_string()),
            ("rhai_blur".to_string(), hooked("blur").to_string()),
        ];
        let formula = |step: &str| {
            format!("const PARAMS = #{{ bailout: 4.0 }};\nfn start(pixel, p) {{ #{{ z: complex(0.0, 0.0), c: pixel }} }}\nfn step(s, p) {{ {step} s }}\nfn escaped(s, p) {{ s.z.norm_sqr() > p.bailout }}\n")
        };
        let compiled = |step: &str| lang::rhai::with_variations(&scope, || lang::rhai::program(&formula(step)));
        let varied = compiled("s.z = vary(\"rhai_ctx_test\", s.z * s.z + s.c);").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.message.clone()).collect::<Vec<_>>()));
        let plain = lang::rhai::program(&formula("s.z = s.z * s.z + s.c;")).unwrap();
        use crate::lang::builtins::C64;
        use crate::lang::interp::{run, Env, ParamValues};
        for px in [C64::new(-0.5, 0.3), C64::new(0.3, 0.5), C64::new(-1.2, 0.1)] {
            let (a, b) = (run(&varied, &ParamValues::defaults(&varied), None, &Env::at(px, 100)), run(&plain, &ParamValues::defaults(&plain), None, &Env::at(px, 100)));
            assert_eq!((a.n, a.escaped), (b.n, b.escaped));
        }
        assert!(compiled("s.z = vary(\"rhai_blur\", s.z * s.z + s.c);").is_ok(), "a drawing variation draws the formula's stream");
        let e = compiled("s.z = vary(\"rhai_macmillan\", s.z);").err().map(|d| d[0].message.clone()).unwrap_or_default();
        assert!(e.contains("keeps state"), "{e}");
    }

    // ---- 3D variations (B2) -------------------------------------------------

    /// Twins of 3D built-ins, written as `variation3` alone (3D only).
    pub(crate) const TWINS_3D: &[(&str, &str)] = &[
        (
            "spherical3D",
            "const INFO = #{ title: \"Spherical 3D (Rhai)\", name: \"rhai_spherical3d\" };\n\
             fn variation3(v, p) {\n    let t = max(dot(v, v), 1e-30);\n    v * (1.0 / t)\n}\n",
        ),
        (
            "hemisphere",
            "const INFO = #{ title: \"Hemisphere (Rhai)\", name: \"rhai_hemisphere\" };\n\
             fn variation3(v, p) {\n    let t = 1.0 / sqrt(v.x * v.x + v.y * v.y + 1.0);\n    vec3(v.x * t, v.y * t, t)\n}\n",
        ),
        (
            "zcone",
            "const INFO = #{ title: \"Z Cone (Rhai)\", name: \"rhai_zcone\" };\n\
             fn variation3(v, p) {\n    vec3(0.0, 0.0, sqrt(v.x * v.x + v.y * v.y))\n}\n",
        ),
    ];

    /// A `variation3` alone makes a 3D variation only, its z kept as the
    /// built-in 3D variations keep theirs; with `variation` too, each
    /// body is its own; the hooks reach the 3D body (a vector accumulator,
    /// state, a colour), and its WGSL validates.
    #[test]
    fn a_3d_variation_compiles_to_its_own_body() {
        for (builtin, src) in TWINS_3D {
            let dl = named(src);
            assert_eq!(dl.category, "only_3d", "{builtin}");
            assert!(dl.shader_2d.is_none());
            assert_eq!(dl.features, ["always_z"]);
            let name = format!("rhai_{}", builtin.to_lowercase());
            assert!(dl.shader_3d.as_deref().is_some_and(|s| s.contains(&format!("fn variation_{name}(p: vec3<f32>) -> vec3<f32>"))), "{builtin}");
        }
        let both = "const INFO = #{ name: \"rhai_both_test\" };\nfn variation(z, p) { z * 0.5 }\nfn variation3(v, p) { v * 0.25 }\n";
        let dl = named(both);
        assert_eq!(dl.category, "plugin");
        assert!(dl.shader_2d.as_deref().is_some_and(|s| s.contains("cv_rhai_both_test_variation_2d")));
        assert!(dl.shader_3d.as_deref().is_some_and(|s| s.contains("cv_rhai_both_test_variation_3d") && !s.contains("p.xy), p.z)")));
        let hooked = "const PARAMS = #{ k: 0.1 };\nconst INFO = #{ name: \"rhai_hooked3_test\" };\nfn init(p) { #{ last: vec3(0.0) } }\n\
                      fn variation3(v, p, ctx) {\n    let w = (v + this.last) * 0.5 + ctx.accum * p.k;\n    this.last = v;\n    \
                      #{ z: w, color: clamp(length(w), 0.0, 1.0) }\n}\n";
        let dl = named(hooked);
        assert_eq!(dl.features, ["needs_accum", "writes_color", "always_z"]);
        assert_eq!(dl.state_count, 3);
        let s3 = dl.shader_3d.as_deref().unwrap();
        assert!(s3.contains("accum: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>"), "{s3}");
        assert!(s3.contains("accum3.z"), "{s3}");
        // A 3D variation only is not a formula's to call.
        let e = lang::rhai::with_variations(&[("rhai_spherical3d".into(), TWINS_3D[0].1.into())], || {
            lang::rhai::program("const PARAMS = #{ bailout: 4.0 };\nfn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\n\
                                 fn step(s, p) { s.z = vary(\"rhai_spherical3d\", s.z); s }\nfn escaped(s, p) { s.z.norm_sqr() > p.bailout }\n")
        });
        assert!(e.is_err_and(|d| d[0].message.contains("3D variation only")));
    }

    /// `prepare(p)` is a variation's derived values, as a built-in's
    /// `wgsl_init`: the init pass's `init_<key>` from the parameters, the
    /// values the body reads (`p.name`) in the slots after them; the
    /// interpreter computes them too. Not from `ctx`, a draw, or no
    /// parameters, and not inside a formula.
    #[test]
    fn prepare_is_the_init_pass() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{ParamValues, VariationRun};
        let head = "const PARAMS = #{ power: 2.0 };\nconst INFO = #{ name: \"rhai_prepare_test\" };\n";
        let src = format!(
            "{head}fn prepare(p) {{\n    let k = 1.0 / p.power;\n    #{{ inv: k, shift: complex(p.power, 2.0 * p.power), unused: 3.0 }}\n}}\n\
             fn variation(z, p) {{ z * p.inv + p.shift }}\n"
        );
        let dl = named(&src);
        assert_eq!(dl.init_param_count, 4, "the three it returns, a complex in two");
        let init = dl.shader_init.as_deref().unwrap();
        assert!(init.contains("fn init_rhai_prepare_test(user: array<f32, 1>) -> array<f32, 4>"), "{init}");
        assert!(init.contains("user[0u]") && !init.contains("get_param"), "{init}");
        let body = dl.shader_2d.as_deref().unwrap();
        assert!(body.contains("get_param(xform_id, variation_id, 1u)") && body.contains("get_param(xform_id, variation_id, 3u)"), "{body}");
        let p = lang::rhai::transformation(&src).unwrap();
        let params = ParamValues::defaults(&p);
        let (z, _) = VariationRun::new(&p, &params, 1).call(C64::new(1.0, -1.0), C64::ZERO, 1.0);
        assert_eq!((z.re, z.im), (2.5, 3.5));

        for (bad, why) in [
            ("fn prepare(p, ctx) { #{ inv: 1.0 } }", "`prepare(p)`"),
            ("fn prepare(p) { #{ inv: rand() } }", "draws no random"),
        ] {
            let s = format!("{head}{bad}\nfn variation(z, p) {{ z * p.inv }}\n");
            assert!(compile(&s).is_err_and(|e| e.contains(why)), "{bad}: {:?}", compile(&s).err());
        }
        let none = "const INFO = #{ name: \"rhai_prepare_none\" };\nfn prepare(p) { #{ k: 2.0 } }\nfn variation(z, p) { z * p.k }\n";
        assert!(compile(none).is_err_and(|e| e.contains("without any")));
        let e = lang::rhai::with_variations(&[("rhai_prepare_test".into(), src.clone())], || {
            lang::rhai::program("const PARAMS = #{ bailout: 4.0 };\nfn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\n\
                                 fn step(s, p) { s.z = vary(\"rhai_prepare_test\", s.z); s }\nfn escaped(s, p) { s.z.norm_sqr() > p.bailout }\n")
        });
        assert!(e.is_err_and(|d| d[0].message.contains("`prepare`")));
    }

    /// `u32(x)` is WGSL's unsigned int (the user's call, 2026-11-06):
    /// `+ - *` wrap, `/ %` and comparisons unsigned, `>>` logical,
    /// `to_float` 0..2^32-1, a float truncated and clamped at 0; an int
    /// joins to it by its bits, and it becomes an int only by `to_int`.
    #[test]
    fn u32_is_wgsl_unsigned() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{ParamValues, VariationRun};
        let src = "const INFO = #{ name: \"rhai_u32_test\" };\n\
                   fn variation(z, p) {\n\
                   \x20   let a = u32(7) - u32(8);\n\
                   \x20   let half = a / 2;\n\
                   \x20   let bigger = if a > u32(1) { 1.0 } else { 0.0 };\n\
                   \x20   let top = a >> 31;\n\
                   \x20   let h = (a ^ 0x9e3779b9) * 2654435761;\n\
                   \x20   let r = h % 1000;\n\
                   \x20   let floor_neg = u32(-3.5);\n\
                   \x20   let signed = to_int(a);\n\
                   \x20   complex(to_float(a) + bigger + to_float(top) + to_float(floor_neg), to_float(half) + to_float(r) * 1.0e-9 + to_float(signed))\n\
                   }\n";
        let dl = named(src);
        assert!(dl.shader_2d.as_deref().is_some_and(|s| s.contains("u32")));
        let p = lang::rhai::transformation(src).unwrap();
        let params = ParamValues::defaults(&p);
        let (z, _) = VariationRun::new(&p, &params, 1).call(C64::ZERO, C64::ZERO, 1.0);
        let h = (u32::MAX ^ 0x9e37_79b9).wrapping_mul(2_654_435_761);
        assert_eq!(z.re, 4_294_967_295.0 + 1.0 + 1.0 + 0.0, "wrapped, unsigned compare, logical shift, clamped at 0");
        assert_eq!(z.im, 2_147_483_647.0 + f64::from(h % 1000) * 1.0e-9 - 1.0, "unsigned division and remainder, to_int its bits");
        // A u32 is an int again only by `to_int`.
        let slip = "const INFO = #{ name: \"rhai_u32_slip\" };\nfn variation(z, p) {\n    let n = 0;\n    n = u32(3);\n    z * to_float(n)\n}\n";
        assert!(compile(slip).is_err_and(|e| e.contains("a u32 where an int is wanted")), "{:?}", compile(slip).err());
    }

    /// `INFO.z` is JWildfire's z rule, as the built-ins' features: by
    /// default a 3D body's z is always kept and a 2D body's only when the
    /// flame preserves z; `always`, `preserve_z` and `never` say so of
    /// either, and anything else is refused.
    #[test]
    fn info_z_is_the_z_rule() {
        let src = |z: &str, three: bool| {
            let info = if z.is_empty() { String::new() } else { format!(", z: \"{z}\"") };
            let body = if three { "fn variation3(v, p) { v * 0.5 }" } else { "fn variation(z, p) { z * 0.5 }" };
            format!("const INFO = #{{ name: \"rhai_z_rule_test\"{info} }};\n{body}\n")
        };
        let z_of = |z: &str, three: bool| named(&src(z, three)).features.into_iter().filter(|f| f.ends_with("_z")).collect::<Vec<_>>();
        assert_eq!(z_of("", true), ["always_z"]);
        assert!(z_of("", false).is_empty());
        assert!(z_of("preserve_z", true).is_empty());
        assert_eq!(z_of("always", false), ["always_z"]);
        assert_eq!(z_of("never", true), ["never_z"]);
        assert!(compile(&src("sometimes", true)).is_err_and(|e| e.contains("`z` is")), "refused");
    }

    /// The 3D twins run as the built-ins' formulas, and a hooked 3D
    /// variation keeps its state and reads its vector accumulator.
    #[test]
    fn the_3d_twins_run_as_their_formulas() {
        use crate::lang::interp::{ParamValues, VariationRun};
        let at = |builtin: &str, v: [f64; 3]| {
            let src = TWINS_3D.iter().find(|(n, _)| *n == builtin).unwrap().1;
            let p = lang::rhai::transformation3(src).unwrap();
            let params = ParamValues::defaults(&p);
            VariationRun::new(&p, &params, 1).call3(v, [0.0; 3], 1.0).0
        };
        let close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-12);
        for v in [[0.3f64, -0.7, 0.2], [-1.2, 0.4, 0.9], [2.0, 1.5, -0.5]] {
            let [x, y, z] = v;
            let t = (x * x + y * y + z * z).max(1e-30);
            assert!(close(at("spherical3D", v), [x / t, y / t, z / t]));
            let h = 1.0 / (x * x + y * y + 1.0).sqrt();
            assert!(close(at("hemisphere", v), [x * h, y * h, h]));
            assert!(close(at("zcone", v), [0.0, 0.0, (x * x + y * y).sqrt()]));
        }
        let hooked = "const PARAMS = #{ k: 0.1 };\nconst INFO = #{ name: \"rhai_hooked3_test\" };\nfn init(p) { #{ last: vec3(0.0) } }\n\
                      fn variation3(v, p, ctx) {\n    let w = (v + this.last) * 0.5 + ctx.accum * p.k;\n    this.last = v;\n    \
                      #{ z: w, color: clamp(length(w), 0.0, 1.0) }\n}\n";
        let p = lang::rhai::transformation3(hooked).unwrap();
        let params = ParamValues::defaults(&p);
        let mut run = VariationRun::new(&p, &params, 1);
        let mut last = [0.0f64; 3];
        for i in 0..10 {
            let v = [0.1 * i as f64, -0.2, 0.3 + 0.05 * i as f64];
            let accum = [0.5, -0.25, 1.0];
            let want: Vec<f64> = (0..3).map(|k| (v[k] + last[k]) * 0.5 + accum[k] * 0.1).collect();
            let (got, color) = run.call3(v, accum, 1.0);
            assert!(close(got, [want[0], want[1], want[2]]), "call {i}: {got:?} against {want:?}");
            let len = (want[0] * want[0] + want[1] * want[1] + want[2] * want[2]).sqrt();
            assert!(color.is_some_and(|c| (c - len.clamp(0.0, 1.0)).abs() < 1e-12));
            last = v;
        }
    }

    /// **The 3D twins render as the built-ins in a 3D flame** (B2's
    /// gate), by density over replicates, seen from a pitched camera so
    /// the depth the variations make shows.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_3d_twins_render_as_the_builtins() {
        use crate::renderer::density_gates::{compare_blocks, print_compared, render_blocks, Run};
        const N: u32 = 256;
        let (device, queue) = device();
        let run = Run::default();
        let flame = |v: &str, carries: Option<CustomSource>| {
            let mut cfg = flame_with(v, carries);
            cfg.render_mode = RenderMode::ThreeD;
            cfg.preserve_z = true;
            cfg.camera_rotation_x = 0.7;
            cfg.camera_rotation_y = 0.4;
            cfg
        };
        let mut failures = Vec::new();
        for (builtin, src) in TWINS_3D {
            let name = format!("rhai_{}", builtin.to_lowercase());
            let carried = CustomSource { dialect: CustomDialect::Rhai, entry: name.clone(), source: src.to_string(), variations: Vec::new() };
            let reference = render_blocks(&device, &queue, &flame(builtin, None), N, &run);
            let twin = render_blocks(&device, &queue, &flame(&name, Some(carried)), N, &run);
            let c = compare_blocks(&twin.density, &reference.density, &reference.density);
            print_compared(&format!("{name} against {builtin}"), &c);
            let (_, _, mx, _, my, _, z) = c.pooled[0];
            if mx <= 0.0 {
                failures.push(format!("{name}: drew nothing"));
            }
            if z.abs() > 4.0 && (mx / my - 1.0).abs() > 0.005 {
                failures.push(format!("{name}: the whole view's density is {:.4} of the built-in's", mx / my));
            }
            if c.worst_dense > 0.05 || c.worst_middle > 0.15 {
                failures.push(format!("{name}: a block is {:.3} off (dense), {:.3} (middle)", c.worst_dense, c.worst_middle));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// **A variation's Orbit Length is the dispatch's** where orbits are
    /// reset: the flame's own, over the caller's Iterations per Thread.
    /// Dynamic System at an Orbit Length of 300, dispatched as 64 a thread,
    /// runs 300 a thread, a new generation each, plotting 300 less the
    /// burn-in.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_variations_orbit_length_is_the_dispatchs() {
        let (device, queue) = device();
        let (_, source) = crate::storage::formula_store::EXAMPLES.iter().find(|(f, _)| *f == "variation-dynamic-system.rhai").unwrap();
        let carried = CustomSource { dialect: CustomDialect::Rhai, entry: "dynamic_system".into(), source: source.to_string(), variations: Vec::new() };
        let mut cfg = flame_with("dynamic_system", Some(carried));
        cfg.flame.transforms[0].set_variation_param("dynamic_system", crate::variations::ORBIT_LENGTH_PARAM, 300.0);
        assert!(cfg.resets_orbits() && cfg.orbit_length() == Some(300));
        let mut r = crate::renderer::FlameRenderer::with_palette_size(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, 64, 64, &cfg.flame, cfg.palette_size);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
        r.load_config(&device, &mut enc, &queue, &cfg, &cfg.palette, 64, 20);
        queue.submit(Some(enc.finish()));
        assert_eq!(r.dispatch_length(64), 300);
        let mut gens = Vec::new();
        for _ in 0..3 {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
            let n = r.compute_pass(&mut enc, &queue, &device, 8, 64, 20, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor, true, false);
            queue.submit(Some(enc.finish()));
            assert_eq!(n, 8 * 64 * (300 - 20), "a dispatch's count");
            gens.push(r.orbit_generation());
        }
        assert!(gens.windows(2).all(|w| w[0] != w[1]), "a dispatch kept the orbits: {gens:?}");
    }

    // ---- the GPU gates ----------------------------------------------------

    use crate::config::FractalConfig;
    use crate::scene::transforms::{RenderMode, Transform};

    fn device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        let al = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
        limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
        limits.max_buffer_size = al.max_buffer_size;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("custom variation gates"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            ..Default::default()
        }))
        .expect("device")
    }

    /// The twin's source, carried as a flame carries it.
    fn carried(builtin: &str) -> CustomSource {
        let src = TWINS.iter().find(|(n, _)| *n == builtin).unwrap().1;
        CustomSource { dialect: crate::config::escape::CustomDialect::Rhai, entry: format!("rhai_{builtin}"), source: src.to_string(), variations: Vec::new() }
    }

    /// A 2D flame of three transforms: `variation` on a turning shrink,
    /// and two linear shrinks that keep it bounded. A twin's flame
    /// carries its source and nothing registers it but the renderer.
    fn flame_with(variation: &str, carries: Option<CustomSource>) -> FractalConfig {
        let mut cfg = FractalConfig::default();
        cfg.render_mode = RenderMode::TwoD;
        cfg.flame.transforms.clear();
        for (i, (m, v)) in [
            ((0.6f32, -0.3f32, 0.3f32, 0.6f32, 0.1f32, -0.2f32), variation),
            ((0.5, 0.0, 0.0, 0.5, 0.5, 0.3), "linear"),
            ((0.5, 0.0, 0.0, 0.5, -0.4, 0.4), "linear"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = Transform::default();
            (t.a, t.b, t.c, t.d, t.e, t.f) = m;
            t.weight = 1.0;
            t.color = i as f32 / 2.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(v, 1.0);
            cfg.flame.transforms.push(t);
        }
        cfg.flame.custom_variations = carries.into_iter().collect();
        cfg.zoom = 1.6;
        cfg.levels_enabled = false;
        cfg
    }

    /// **The hooked twins render as the built-ins in a flame** (C2's
    /// gate): `blur` (draws) and `macmillan` (the accumulator, its weight,
    /// state its parameters seed, a colour) after a linear at half weight
    /// on the first transform -- so the accumulator holds something --
    /// against the built-ins, by density and by each colour's light, block
    /// by block over replicates.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_hooked_twins_render_as_the_builtins() {
        use crate::renderer::density_gates::{compare_blocks, print_compared, render_blocks, Run};
        const N: u32 = 256;
        let (device, queue) = device();
        let run = Run::default();
        let flame = |v: &str, carries: Option<CustomSource>| {
            let mut cfg = flame_with(v, carries);
            let t = &mut cfg.flame.transforms[0];
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 0.5);
            t.set_variation(v, 1.0);
            cfg
        };
        let mut failures = Vec::new();
        for (builtin, src) in HOOKED_TWINS {
            let name = format!("rhai_{builtin}");
            let carried = CustomSource { dialect: CustomDialect::Rhai, entry: name.clone(), source: src.to_string(), variations: Vec::new() };
            let reference = render_blocks(&device, &queue, &flame(builtin, None), N, &run);
            let twin = render_blocks(&device, &queue, &flame(&name, Some(carried)), N, &run);
            for (what, x, y) in [
                ("density", &twin.density, &reference.density),
                ("red", &twin.light[0], &reference.light[0]),
                ("green", &twin.light[1], &reference.light[1]),
                ("blue", &twin.light[2], &reference.light[2]),
            ] {
                let c = compare_blocks(x, y, y);
                print_compared(&format!("{name} against {builtin}, {what}"), &c);
                let (_, _, mx, _, my, _, z) = c.pooled[0];
                if mx <= 0.0 {
                    failures.push(format!("{name}: no {what}"));
                }
                if z.abs() > 4.0 && (mx / my - 1.0).abs() > 0.005 {
                    failures.push(format!("{name}: the whole view's {what} is {:.4} of the built-in's", mx / my));
                }
                if c.worst_dense > 0.05 || c.worst_middle > 0.15 {
                    failures.push(format!("{name}: a block's {what} is {:.3} off (dense), {:.3} (middle)", c.worst_dense, c.worst_middle));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// **The twins render as the built-ins in a flame** (the plan's C1
    /// gate): each Rhai twin's flame against the built-in's, by density
    /// block by block over replicates -- the chaos game parts the two
    /// orbits at the first rounding, so they agree as measures, not
    /// pixel for pixel. And the measure tells a different variation
    /// apart, or agreeing would mean nothing.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_twins_render_as_the_builtins_in_a_flame() {
        use crate::renderer::density_gates::{compare_blocks, print_compared, render_blocks, Run};
        const N: u32 = 256;
        let (device, queue) = device();
        let run = Run::default();
        let mut failures = Vec::new();
        let mut renders = Vec::new();
        for (builtin, _) in TWINS {
            let reference = render_blocks(&device, &queue, &flame_with(builtin, None), N, &run);
            let twin = render_blocks(&device, &queue, &flame_with(&format!("rhai_{builtin}"), Some(carried(builtin))), N, &run);
            let c = compare_blocks(&twin.density, &reference.density, &reference.density);
            print_compared(&format!("rhai_{builtin} against {builtin}"), &c);
            let (_, _, mx, _, my, _, z) = c.pooled[0];
            if mx <= 0.0 {
                failures.push(format!("rhai_{builtin}: drew nothing"));
            }
            if z.abs() > 4.0 && (mx / my - 1.0).abs() > 0.005 {
                failures.push(format!("rhai_{builtin}: the whole view's density is {:.4} of the built-in's", mx / my));
            }
            if c.worst_dense > 0.05 || c.worst_middle > 0.15 {
                failures.push(format!("rhai_{builtin}: a block is {:.3} off (dense), {:.3} (middle)", c.worst_dense, c.worst_middle));
            }
            renders.push((builtin, reference));
        }
        // Each twin's built-in against the next one's: told apart.
        for w in renders.windows(2) {
            let c = compare_blocks(&w[0].1.density, &w[1].1.density, &w[1].1.density);
            if c.worst_dense < 0.2 {
                failures.push(format!("{} and {} look alike to this measure: {:.3}", w[0].0, w[1].0, c.worst_dense));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// **The twins are lenses as the built-ins are** (the plan's C1 gate):
    /// a Mandelbrot seen through each twin against the built-in's lens. A
    /// lens is a map, deterministic per pixel, so the pictures agree but
    /// for the pixels the twin's rounding (and its `1e-6` guards) puts
    /// across a band edge; a different lens differs in most.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_twins_are_lenses_as_the_builtins() {
        const W: u32 = 128;
        let (device, queue) = device();
        let config = |lens: &str, carries: Option<CustomSource>| {
            let mut c = FractalConfig::default();
            c.render_mode = RenderMode::Escape;
            c.escape.formula = "mandelbrot".to_string();
            c.escape.coloring = "smooth".to_string();
            c.escape.max_iter = 64;
            c.escape.lens = lens.to_string();
            c.escape.lens_amount = 1.0;
            c.flame.custom_variations = carries.into_iter().collect();
            c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
            c.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
            c.gamma = crate::config::defaults::DEFAULT_GAMMA;
            c
        };
        let render = |c: &FractalConfig| {
            pollster::block_on(crate::renderer::render(&device, &queue, crate::renderer::RenderJob::new(c, W, W), &mut crate::renderer::NoProgress))
                .expect("render")
                .rgba_data
        };
        // Pixels a channel more than 8 levels apart.
        let apart = |a: &[u8], b: &[u8]| a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 8)).count();
        let n = (W * W) as usize;
        let plain = render(&config("", None));
        let mut failures = Vec::new();
        for (builtin, _) in TWINS {
            let reference = render(&config(builtin, None));
            let twin = render(&config(&format!("rhai_{builtin}"), Some(carried(builtin))));
            let d = apart(&twin, &reference);
            println!("  rhai_{builtin}: {d}/{n} pixels apart from {builtin}'s lens; {builtin} moved {}", apart(&reference, &plain));
            if d * 100 > n {
                failures.push(format!("rhai_{builtin}: {d}/{n} pixels apart"));
            }
        }
        let spherical = render(&config("rhai_spherical", Some(carried("spherical"))));
        let swirl = render(&config("swirl", None));
        assert!(apart(&spherical, &swirl) > n / 4, "a spherical lens looks like a swirl one");
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
