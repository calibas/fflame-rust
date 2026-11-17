use super::*;

fn lib() -> crate::scene::palette::PaletteLibrary {
    crate::scene::palette::global_palette_library().read().unwrap().clone()
}

fn of_kind(entries: &[Entry], kind: Kind) -> Vec<&Entry> {
    entries.iter().filter(|e| e.kind == kind).collect()
}

/// Every registered built-in is listed once, under its registry name:
/// the escape and simulation registries, the variations and effects that
/// are not the user's, and every preset.
#[test]
fn every_registered_resource_is_listed_once() {
    let all = scan(&lib(), Online::default());
    let names = |kind: Kind| -> Vec<String> {
        of_kind(&all, kind)
            .iter()
            .filter_map(|e| match &e.key {
                EntryKey::Registered(k, n) if *k == kind => Some(n.clone()),
                _ => None,
            })
            .collect()
    };
    let want = |kind: Kind, list: Vec<&str>| {
        let got = names(kind);
        assert_eq!(got.len(), list.len(), "{kind:?}: {} listed, {} registered", got.len(), list.len());
        for n in list {
            assert_eq!(got.iter().filter(|g| *g == n).count(), 1, "{kind:?} {n}");
        }
    };
    want(Kind::Formula, crate::escape::FORMULAS.iter().map(|f| f.name).collect());
    want(Kind::Coloring, crate::escape::COLORINGS.iter().map(|c| c.name).collect());
    want(Kind::Field, crate::escape::fields::FIELDS.iter().map(|f| f.name).collect());
    want(Kind::FieldColoring, crate::escape::fields::FIELD_COLORINGS.iter().map(|c| c.name).collect());
    want(Kind::Solid, crate::escape::ifs::IFS_DEFS.iter().map(|d| d.name).collect());
    want(Kind::SolidColoring, crate::escape::ifs::IFS_COLORINGS.iter().map(|c| c.name).collect());
    want(Kind::SimModel, crate::sim::MODELS.iter().map(|m| m.name).collect());
    want(Kind::SimColoring, crate::sim::COLORINGS.iter().map(|c| c.name).collect());
    {
        let reg = crate::variations::global_registry();
        let list: Vec<String> =
            reg.all().iter().filter(|v| v.provenance != crate::provenance::Provenance::Local).map(|v| v.name.clone()).collect();
        drop(reg);
        want(Kind::Variation, list.iter().map(String::as_str).collect());
    }
    {
        let reg = crate::effects::global_effect_registry();
        let list: Vec<String> =
            reg.all().filter(|e| e.provenance != crate::provenance::Provenance::Local).map(|e| e.name.clone()).collect();
        drop(reg);
        want(Kind::Effect, list.iter().map(String::as_str).collect());
    }
    let presets = crate::scene::presets::global_preset_library().len();
    assert_eq!(of_kind(&all, Kind::Fractal).len(), presets, "every preset");
}

/// A key names one entry -- but a palette in two packs is one palette,
/// listed in each -- and the list comes out in the same order every time.
#[test]
fn keys_are_unique_and_the_order_is_stable() {
    let a = scan(&lib(), Online::default());
    let mut seen = std::collections::HashSet::new();
    for e in a.iter().filter(|e| e.kind != Kind::Palette) {
        assert!(seen.insert(e.key.clone()), "{:?} listed twice", e.key);
    }
    // The same order every time -- compared over the kinds whose sources
    // are static, since tests running beside this one load palette packs
    // and register variations into the global libraries.
    let b = scan(&lib(), Online::default());
    let fixed = |l: &[Entry]| -> Vec<EntryKey> {
        l.iter()
            .filter(|e| match &e.key {
                EntryKey::Registered(k, _) => k.family() == Family::Escape || k.family() == Family::Simulation,
                EntryKey::Preset(_) => true,
                _ => false,
            })
            .map(|e| e.key.clone())
            .collect()
    };
    assert_eq!(fixed(&a), fixed(&b));
    // By kind, then group.
    for w in a.windows(2) {
        assert!((w[0].kind, w[0].group.order) <= (w[1].kind, w[1].group.order), "{:?} before {:?}", w[0].key, w[1].key);
    }
}

/// A palette and a preset are found again by their keys, and are what
/// their sources hold.
#[test]
fn a_key_finds_its_resource_again() {
    let all = scan(&lib(), Online::default());
    let p = of_kind(&all, Kind::Palette).into_iter().next().expect("a palette");
    let EntryKey::Palette(hash) = &p.key else { panic!("{:?}", p.key) };
    let found = palette(&lib(), hash).expect("found by its hash");
    assert_eq!(found.name, p.title);
    assert_eq!(&crate::api::sync::palette_content_hash(&found), hash);
    let f = of_kind(&all, Kind::Fractal).into_iter().next().expect("a preset");
    let EntryKey::Preset(name) = &f.key else { panic!("{:?}", f.key) };
    assert_eq!(preset(name).expect("found by its name").flame.name, f.title);
}

/// The prose the app ships: the variations' from their twins' doc blocks
/// (all but the few without a twin), every simulation model's and
/// colouring's line, and no author a source does not name.
#[test]
fn the_built_ins_carry_their_prose() {
    let all = scan(&lib(), Online::default());
    // The registered built-ins; the seven variations without a twin
    // (`analytic_blur`, `mondrianomies`, the volumes, the subflames) take
    // the server catalog's prose when it is cached. (shipped example files are files).
    let builtin = |kind: Kind| {
        of_kind(&all, kind)
            .into_iter()
            .filter(|e| e.origin == Origin::Builtin && matches!(e.key, EntryKey::Registered(..)))
            .collect::<Vec<_>>()
    };
    let vars = builtin(Kind::Variation);
    let described = vars.iter().filter(|e| !e.description.is_empty()).count();
    println!("variations: {described} of {} described", vars.len());
    for e in vars.iter().filter(|e| e.description.is_empty()) {
        println!("  undescribed: {:?}", e.key);
    }
    assert!(described + 10 >= vars.len(), "{described} of {} variations described", vars.len());
    let linear = vars.iter().find(|e| e.key == EntryKey::Registered(Kind::Variation, "linear".into())).expect("linear");
    assert!(!linear.description.is_empty());
    // Every escape-time built-in and every effect: its twin's
    // INFO.description, or the locale's for those without a twin.
    for kind in [Kind::SimModel, Kind::SimColoring, Kind::Formula, Kind::Coloring, Kind::Field, Kind::FieldColoring, Kind::Solid, Kind::SolidColoring, Kind::Effect] {
        for e in builtin(kind) {
            assert!(!e.description.is_empty(), "{kind:?} {:?}", e.key);
        }
    }
    // Authors only as written: a variation's are its doc block's list.
    for e in &vars {
        if let EntryKey::Registered(_, name) = &e.key {
            if let Some(p) = super::prose::variation(name) {
                assert_eq!(e.authors, p.authors, "{name}");
            }
        }
    }
}

/// A search matches the title, the name, the description and the
/// authors, ignoring case; an empty one matches everything.
#[test]
fn a_search_reads_names_prose_and_authors() {
    let e = Entry {
        kind: Kind::Variation,
        key: EntryKey::Registered(Kind::Variation, "julian".into()),
        title: "Julian".into(),
        description: "Spreads points around n arms".into(),
        authors: vec!["Someone Else".into()],
        version: None,
        origin: Origin::Builtin,
        plugin: false,
        group: Group { order: 0, label: String::new() },
        link: None,
        installed: true,
        category: None,
        update: None,
        online: None,
    };
    for q in ["", "JULIAN", "arms", "someone", "  julian  "] {
        assert!(e.matches(q), "{q:?}");
    }
    assert!(!e.matches("spherical"));
}

/// What the server offers and the app lacks is listed, not installed, with
/// the catalog's prose and category, once; what the registry holds (a
/// built-in of the same name) and what may not be downloaded are not.
#[test]
fn what_the_server_offers_is_listed_to_download() {
    use crate::api::types::{EffectListItem, VariationListItem};
    let variation = |name: &str, downloadable: bool| VariationListItem {
        id: name.into(),
        name: name.into(),
        display_name: format!("{name} display"),
        category: "Plugin".into(),
        version: 3,
        description: Some("From **afar**".into()),
        description_plain: None,
        authors: vec!["Someone".into()],
        downloadable,
        has_shader_3d: true,
    };
    let variations = [variation("it_offered_only", true), variation("it_not_downloadable", false), variation("linear", true)];
    let effect: EffectListItem = serde_json::from_value(serde_json::json!({
        "id": "it_offered_effect", "name": "it_offered_effect", "category": "density", "downloadable": true, "version": 2
    }))
    .unwrap();
    let effects = [effect];
    let all = scan(&lib(), Online { variations: &variations, effects: &effects });
    let named = |name: &str| all.iter().filter(|e| matches!(&e.key, EntryKey::Registered(_, n) if n == name)).collect::<Vec<_>>();

    let offered = named("it_offered_only");
    assert_eq!(offered.len(), 1);
    let e = offered[0];
    assert!(!e.installed && e.origin == Origin::Api && e.kind == Kind::Variation);
    assert_eq!((e.description.as_str(), e.version.as_deref(), e.category.as_deref()), ("From **afar**", Some("v3"), Some("Plugin")));
    assert!(named("it_not_downloadable").is_empty(), "not downloadable: not offered");
    let linear = named("linear");
    assert_eq!(linear.len(), 1, "a built-in is listed once");
    assert!(linear[0].installed);
    let fx = named("it_offered_effect");
    assert_eq!((fx.len(), fx[0].installed, fx[0].category.as_deref()), (1, false, Some("density")));

    // The list is read again when the catalog changes.
    assert_ne!(revision(&lib(), 0, Online::default()), revision(&lib(), 0, Online { variations: &variations, effects: &effects }));
}
