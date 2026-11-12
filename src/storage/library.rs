//! One library over the user's two stores (docs/archive/projects/rhai-plugins.md,
//! P3): the CPU scripts (`scripts/`, [`crate::script::store`]) and the
//! GPU formulas, colourings, variations, solids and effects
//! (`formulas/`, [`crate::storage::formula_store`]), with what ships of
//! each. The two folders stay -- UF files live in `formulas/` -- and each
//! store keeps its own file rules; what is one is what a person reads of
//! an entry, its link record ([`crate::storage::links`]) and the scan.

use crate::storage::links::Link;

/// Which store an entry is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// The CPU scripts: generators, modifiers, libraries.
    Scripts,
    /// The GPU language's files: Rhai, UF, Fractint.
    Formulas,
}

/// What an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Generator,
    Modifier,
    ScriptLibrary,
    Formula,
    Coloring,
    Variation,
    Solid,
    Effect,
    /// A simulation's colouring (P4).
    SimColoring,
    /// A simulation's model (P4).
    SimModel,
    /// A mode B field, and a field's colouring
    /// (docs/archive/projects/rhai-solids-fields.md).
    Field,
    FieldColoring,
    /// An animation track (docs/archive/projects/rhai-tracks.md).
    Track,
    /// An Ultra Fractal or Fractint file: many entries, of its kind.
    UfFormulas,
    UfColorings,
    Fractint,
    /// A file whose kind could not be read (it does not parse).
    Unknown,
}

impl Kind {
    /// A GPU Rhai entry that can be a plug-in (`storage::links::Link::plugin`).
    pub fn can_be_plugin(self) -> bool {
        matches!(self, Kind::Variation | Kind::Effect | Kind::Formula | Kind::Coloring | Kind::Solid | Kind::SimColoring | Kind::SimModel | Kind::Field | Kind::FieldColoring | Kind::Track)
    }
}

impl Kind {
    /// What the server's scripts resource calls it (ffapi's `script_kind`,
    /// the app's spellings: docs/projects/rhai-scripts-online.md, decision
    /// 1). `None` for what is not published: a UF or Fractint file, one
    /// whose kind could not be read.
    pub fn wire(self) -> Option<&'static str> {
        Some(match self {
            Kind::Generator => "generator",
            Kind::Modifier => "modifier",
            Kind::ScriptLibrary => "library",
            Kind::Formula => "formula",
            Kind::Coloring => "coloring",
            Kind::Variation => "variation",
            Kind::Solid => "solid",
            Kind::Effect => "effect",
            Kind::SimColoring => "sim_coloring",
            Kind::SimModel => "sim_model",
            Kind::Field => "field",
            Kind::FieldColoring => "field_coloring",
            Kind::Track => "track",
            Kind::UfFormulas | Kind::UfColorings | Kind::Fractint | Kind::Unknown => return None,
        })
    }

    /// The kind the server names `wire`; `None` for one this build does
    /// not know (a newer server's).
    pub fn from_wire(wire: &str) -> Option<Kind> {
        ALL_PUBLISHED.iter().copied().find(|k| k.wire() == Some(wire))
    }

    /// A kind the formula library holds: one Rhai entry, published under
    /// its own `INFO.id`. The rest are the CPU scripts.
    pub fn is_entry(self) -> bool {
        self.can_be_plugin()
    }
}

/// Every kind the scripts resource stores, in the order a kind filter
/// offers them.
pub const ALL_PUBLISHED: &[Kind] = &[
    Kind::Generator,
    Kind::Modifier,
    Kind::ScriptLibrary,
    Kind::Formula,
    Kind::Coloring,
    Kind::Solid,
    Kind::Field,
    Kind::FieldColoring,
    Kind::Variation,
    Kind::Effect,
    Kind::SimModel,
    Kind::SimColoring,
    Kind::Track,
];

/// What a Rhai file of the formula store holds, by its functions
/// ([`crate::lang::rhai::kind_of`]), a track by its shape.
pub fn rhai_kind(source: &str) -> Kind {
    use crate::lang::{rhai, EntryKind};
    match rhai::kind_of(source) {
        Some(EntryKind::Formula) => Kind::Formula,
        Some(EntryKind::Coloring) => Kind::Coloring,
        Some(EntryKind::Transform) => Kind::Variation,
        Some(EntryKind::Solid) => Kind::Solid,
        Some(EntryKind::Effect) => Kind::Effect,
        Some(EntryKind::SimColoring) => Kind::SimColoring,
        Some(EntryKind::SimModel) => Kind::SimModel,
        Some(EntryKind::Field) => Kind::Field,
        Some(EntryKind::FieldColoring) => Kind::FieldColoring,
        // A track is the CPU world's: no GPU kind reads it.
        None if crate::animation::script::looks_like_track(source) => Kind::Track,
        None => Kind::Unknown,
    }
}

/// Where an entry came from (design rule 7's namespaces, D6): the one
/// origin model every kind shares.
pub use crate::provenance::Origin;

/// One file of the library, as a person reads it.
#[derive(Debug, Clone)]
pub struct Entry {
    pub store: Store,
    /// Its file name in the store (a script's stem with `.rhai`), or a
    /// shipped one's name.
    pub file: String,
    pub kind: Kind,
    /// In the current language; the file's name when it declares none.
    pub title: String,
    pub description: String,
    pub author: String,
    /// `INFO.id`, when it has one.
    pub id: Option<String>,
    /// The first 8 hex of the source's SHA-256 (`scripted::version_of`).
    pub version: String,
    pub origin: Origin,
    pub link: Link,
    pub source: String,
}

/// Every entry of both stores, the shipped ones first, each group by
/// title.
pub fn scan() -> Vec<Entry> {
    let mut out = Vec::new();
    for (name, source) in crate::script::library::EMBEDDED {
        out.push(script_entry(name, source, Origin::Builtin, Link::default()));
    }
    for (name, source) in crate::storage::formula_store::EXAMPLES {
        out.push(formula_entry(name, source, Origin::Builtin, Link::default()));
    }
    for (name, source) in crate::effects::custom::BUILTIN_TWINS {
        out.push(formula_entry(&format!("{name}.rhai"), source, Origin::Builtin, Link::default()));
    }
    for (name, source) in crate::sim::custom::SHIPPED.iter().chain(crate::sim::custom::SHIPPED_MODELS) {
        out.push(formula_entry(&format!("sim-{name}.rhai"), source, Origin::Builtin, Link::default()));
    }
    let script_links = crate::script::store::load_links();
    for (stem, source) in crate::script::store::list() {
        let link = script_links.get(&stem).cloned().unwrap_or_default();
        out.push(script_entry(&format!("{stem}.rhai"), &source, origin_of_link(&link), link));
    }
    let formula_links = crate::storage::formula_store::links();
    for file in crate::storage::formula_store::list() {
        let Some(source) = crate::storage::formula_store::load(&file) else { continue };
        let link = formula_links.get(&file).cloned().unwrap_or_default();
        out.push(formula_entry(&file, &source, origin_of_link(&link), link));
    }
    out.sort_by_key(|e| (e.origin != Origin::Builtin, e.store == Store::Formulas, e.title.to_lowercase()));
    out
}

/// What a list shows after an entry's title -- the Scripts panel's and
/// the Formula Editor's alike (P3, step 4): the origin's mark
/// ([`Origin::mark`]), and whether it is a plug-in.
pub fn marks(origin: Origin, plugin: bool) -> String {
    let mut out = origin.mark().to_string();
    if plugin {
        out.push(' ');
        out.push_str(&t!("library.plugin"));
    }
    out
}

/// A stored file's origin, from its link.
pub fn origin_of_link(link: &Link) -> Origin {
    if link.from_others {
        Origin::Api
    } else {
        Origin::Local
    }
}

fn version(source: &str) -> String {
    crate::scripted::version_of(source)[..8].to_string()
}

fn stem(file: &str) -> String {
    file.rsplit_once('.').map_or(file, |(s, _)| s).to_string()
}

/// A CPU script, read from its `script("Name", "kind")` declaration by
/// parsing alone: running each script to learn it costs ~5 ms
/// (`script::library::discover`), which a scan of the library must not.
fn script_entry(file: &str, source: &str, origin: Origin, link: Link) -> Entry {
    let (name, kind) = crate::script::library::declared(source).unwrap_or_else(|| (String::new(), crate::script::ScriptKind::Generator));
    let kind = match kind {
        crate::script::ScriptKind::Generator => Kind::Generator,
        crate::script::ScriptKind::Modifier => Kind::Modifier,
        crate::script::ScriptKind::Library => Kind::ScriptLibrary,
    };
    Entry {
        store: Store::Scripts,
        file: file.to_string(),
        kind,
        title: if name.is_empty() { stem(file) } else { name },
        description: String::new(),
        author: String::new(),
        id: None,
        version: version(source),
        origin,
        link,
        source: source.to_string(),
    }
}

/// A formula store file: a Rhai one read for its kind and `INFO`, a UF or
/// Fractint one by its extension.
fn formula_entry(file: &str, source: &str, origin: Origin, link: Link) -> Entry {
    use crate::storage::formula_store::{is_coloring_file, is_fractint, is_rhai};
    let mut e = Entry {
        store: Store::Formulas,
        file: file.to_string(),
        kind: Kind::Unknown,
        title: stem(file),
        description: String::new(),
        author: String::new(),
        id: None,
        version: version(source),
        origin,
        link,
        source: source.to_string(),
    };
    if !is_rhai(file) {
        e.kind = if is_fractint(file) {
            Kind::Fractint
        } else if is_coloring_file(file) {
            Kind::UfColorings
        } else {
            Kind::UfFormulas
        };
        return e;
    }
    use crate::lang::rhai;
    e.kind = rhai_kind(source);
    if let Some(labels) = rhai::source_labels(source) {
        let title = labels.title.resolve();
        if !title.is_empty() {
            e.title = title;
        }
        e.description = labels.description.resolve();
        e.author = labels.author;
    }
    e.id = rhai::info_string(source, "id");
    e
}

// ============================================================================
// Plug-ins (P3, step 2)
// ============================================================================

/// A plug-in as registered: a GPU Rhai entry of the formula store with
/// its link's `plugin` set, which every start registers
/// ([`load_plugins`]) so the lists offer it without a config carrying it.
#[derive(Debug, Clone)]
pub struct Plugin {
    pub kind: Kind,
    /// Its file in the formula store.
    pub file: String,
    pub title: String,
    /// What it registered as, which a config names: a variation's or an
    /// effect's key, an escape slot's name.
    pub key: String,
    /// Its source as a config carries it, under `key`: choosing it from a
    /// list puts this in the config, as the Formula Editor's Use does.
    pub carried: crate::config::escape::CustomSource,
}

fn registered() -> &'static std::sync::RwLock<Vec<Plugin>> {
    static P: std::sync::OnceLock<std::sync::RwLock<Vec<Plugin>>> = std::sync::OnceLock::new();
    P.get_or_init(Default::default)
}

/// The plug-ins registered, in the order they loaded.
pub fn plugins() -> Vec<Plugin> {
    registered().read().map(|p| p.clone()).unwrap_or_default()
}

/// The plug-ins of a kind.
pub fn plugins_of(kind: Kind) -> Vec<Plugin> {
    plugins().into_iter().filter(|p| p.kind == kind).collect()
}

/// Register every plug-in of the formula store: what each start does
/// where the downloads register (`storage::load_installed_resources`), on
/// every platform and entry point. Gives what did not register, each with
/// why, which is also logged.
pub fn load_plugins() -> Vec<String> {
    let mut failed = Vec::new();
    if let Ok(mut f) = failures().write() {
        f.clear();
    }
    for (file, link) in crate::storage::formula_store::links() {
        if !link.plugin {
            continue;
        }
        let Some(source) = crate::storage::formula_store::load(&file) else {
            failed.push(format!("{file}: not in your formulas"));
            continue;
        };
        if let Err(e) = register_plugin(&file, &source) {
            log::warn!("The plug-in `{file}` did not register: {e}");
            failed.push(format!("{file}: {e}"));
        }
    }
    if let Ok(mut f) = failures().write() {
        f.clone_from(&failed);
    }
    failed
}

fn failures() -> &'static std::sync::RwLock<Vec<String>> {
    static F: std::sync::OnceLock<std::sync::RwLock<Vec<String>>> = std::sync::OnceLock::new();
    F.get_or_init(Default::default)
}

/// The plug-ins the last [`load_plugins`] could not register, each with
/// why: what the app's start reports, as a plug-in that fails silently is
/// a list missing an entry for no reason anyone can see.
pub fn plugin_failures() -> Vec<String> {
    failures().read().map(|f| f.clone()).unwrap_or_default()
}

/// Register one formula store file as a plug-in, replacing what it
/// registered before (an edit is a new version: a new key). A variation
/// or an effect registers in its registry under its key (`Provenance::
/// Local`, the user's namespace); a formula, colouring or solid in the
/// escape engine's under its slot name.
pub fn register_plugin(file: &str, source: &str) -> Result<Plugin, String> {
    let entry = formula_entry(file, source, Origin::Local, Link::default());
    if !entry.kind.can_be_plugin() {
        return Err("a plug-in is a Rhai variation, effect, formula, colouring or solid".to_string());
    }
    let (key, carried) = register_kind(entry.kind, &entry.title, source)?;
    let plugin = Plugin { kind: entry.kind, file: file.to_string(), title: entry.title, key, carried };
    if let Ok(mut all) = registered().write() {
        all.retain(|p| p.file != file);
        all.push(plugin.clone());
    }
    Ok(plugin)
}

/// Stop offering a file's plug-in. What it registered stays registered
/// until the app restarts -- a config may already be using it -- but the
/// lists no longer offer it.
pub fn unlist_plugin(file: &str) {
    if let Ok(mut all) = registered().write() {
        all.retain(|p| p.file != file);
    }
}

fn register_kind(kind: Kind, title: &str, source: &str) -> Result<(String, crate::config::escape::CustomSource), String> {
    match kind {
        Kind::Variation => {
            let key = crate::variations::custom::register(source)?;
            let mut carried = crate::variations::custom::carried(source).ok_or("it does not name itself")?;
            carried.entry = key.clone();
            Ok((key, carried))
        }
        Kind::Effect => {
            let key = crate::effects::custom::register(source, crate::provenance::Provenance::Local)?;
            let mut carried = crate::effects::custom::carried(source).ok_or("it has no id")?;
            carried.entry = key.clone();
            Ok((key, carried))
        }
        Kind::Formula | Kind::Coloring | Kind::Solid | Kind::Field | Kind::FieldColoring => {
            use crate::config::escape::{CustomDialect, CustomSource};
            use crate::escape::custom;
            use crate::lang::EntryKind;
            let ek = match kind {
                Kind::Formula => EntryKind::Formula,
                Kind::Coloring => EntryKind::Coloring,
                Kind::Field => EntryKind::Field,
                Kind::FieldColoring => EntryKind::FieldColoring,
                _ => EntryKind::Solid,
            };
            let src = custom::with_called_variations(ek, CustomSource { dialect: CustomDialect::Rhai, entry: title.to_string(), source: source.to_string(), variations: Vec::new() });
            let name = custom::name_of(ek, &src);
            let compiled = match ek {
                EntryKind::Formula => custom::compile_formula(&src, &name).map(|_| ()),
                EntryKind::Coloring => custom::compile_coloring(&src, &name).map(|_| ()),
                EntryKind::Field => custom::compile_field(&src, &name).map(|_| ()),
                EntryKind::FieldColoring => custom::compile_field_coloring(&src, &name).map(|_| ()),
                _ => custom::compile_solid(&src, &name).map(|_| ()),
            };
            compiled.map_err(|d| d.first().map(|x| x.render(source)).unwrap_or_else(|| "it does not compile".to_string()))?;
            Ok((name, src))
        }
        Kind::SimColoring => {
            let name = crate::sim::custom::register(source)?;
            Ok((name, crate::sim::custom::carried(source)))
        }
        Kind::SimModel => {
            let name = crate::sim::custom::register_model(source)?;
            Ok((name, crate::sim::custom::carried(source)))
        }
        // Nothing to register: the track editor offers it, and adding it
        // puts its source in the animation. Its key is its id, else its
        // title.
        Kind::Track => {
            let declared = crate::animation::script::declare(source)?;
            let key = crate::lang::rhai::info_string(source, "id").unwrap_or_else(|| declared.title.clone());
            use crate::config::escape::{CustomDialect, CustomSource};
            Ok((key, CustomSource { dialect: CustomDialect::Rhai, entry: title.to_string(), source: source.to_string(), variations: Vec::new() }))
        }
        _ => Err("not a kind of plug-in".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped entries are all there, each read: the script starters
    /// by their declarations, the formula examples and the effects by
    /// their kinds and titles.
    /// An animation track reads as one (docs/archive/projects/rhai-tracks.md), and
    /// can be a plug-in, keyed by its id.
    #[test]
    fn a_track_is_a_track() {
        let src = "const INFO = #{ title: \"Turn\", id: \"5b0f8c1e-2d3a-4b5c-8d9e-0f1a2b3c4d5e\" };
const TARGETS = #{ rotation: \"set\" };
fn value(t, p, ctx) { #{ rotation: t } }";
        let e = formula_entry("turn.rhai", src, Origin::Local, Link::default());
        assert_eq!((e.kind, e.title.as_str()), (Kind::Track, "Turn"));
        assert!(Kind::Track.can_be_plugin());
        let (key, carried) = register_kind(Kind::Track, "Turn", src).expect("registers");
        assert_eq!(key, "5b0f8c1e-2d3a-4b5c-8d9e-0f1a2b3c4d5e");
        assert_eq!(carried.source, src);
    }

    #[test]
    fn the_scan_reads_every_shipped_entry() {
        let all = scan();
        let shipped: Vec<&Entry> = all.iter().filter(|e| e.origin == Origin::Builtin).collect();
        let scripts = shipped.iter().filter(|e| e.store == Store::Scripts).count();
        assert_eq!(scripts, crate::script::library::EMBEDDED.len());
        assert!(shipped.iter().filter(|e| e.store == Store::Scripts).all(|e| e.kind != Kind::Unknown && !e.title.is_empty()));
        let basic = shipped.iter().find(|e| e.file == "basic_random.rhai").expect("a starter");
        assert_eq!((basic.kind, basic.title.as_str()), (Kind::Generator, "Basic Random"));
        let vignette = shipped.iter().find(|e| e.file == "vignette.rhai" && e.kind == Kind::Effect).expect("the effect");
        assert_eq!((vignette.title.as_str(), vignette.id.as_deref()), ("Vignette", Some("vignette")));
        assert_eq!(vignette.version.len(), 8);
        assert!(shipped.iter().any(|e| e.kind == Kind::Formula), "the formula examples");
    }

    /// Both lists mark an entry alike: whose it is, and a plug-in.
    #[test]
    fn the_marks_say_whose_it_is_and_a_plugin() {
        assert_eq!(marks(Origin::Builtin, false), "");
        assert_eq!(marks(Origin::Local, false), " *");
        assert_eq!(marks(Origin::Api, false), " ↓");
        let plugin = marks(Origin::Local, true);
        assert!(plugin.starts_with(" * ") && plugin.contains("plug-in"), "{plugin}");
    }

    /// A formula store file's link: set, read back, cleared by deleting the
    /// file, so a name reused later inherits nothing. Against the real
    /// store, as the script store's tests are, and cleaned up.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_formula_files_link_goes_with_it() {
        use crate::storage::formula_store as store;
        let file = store::save("__library link test__.rhai", "fn variation(z, p) { z }\n").expect("save");
        store::set_link(&file, Link { plugin: true, ..Default::default() }).expect("link");
        assert!(store::link_of(&file).is_some_and(|l| l.plugin));
        assert!(scan().iter().any(|e| e.file == file && e.origin == Origin::Local && e.link.plugin), "the scan reads it");
        store::delete(&file).expect("delete");
        assert!(store::link_of(&file).is_none(), "deleting the file forgets its link");
        // An empty link is no entry at all.
        let file = store::save("__library link test__.rhai", "x").expect("save");
        store::set_link(&file, Link::default()).expect("link");
        assert!(!store::links().contains_key(&file));
        store::delete(&file).expect("delete");
    }

    /// An effect plug-in registers under its key, its source carried
    /// under it, and is offered as one; a file of no plug-in's kind is
    /// refused; unlisting stops the offer.
    #[test]
    fn an_effect_plugin_registers_and_is_offered() {
        let src = "const INFO = #{ title: \"Tint test\", id: \"1d2e3f4a-5b6c-4d7e-8f9a-0b1c2d3e4f5a\" };\nfn effect(c, p) { c }\n";
        let p = register_plugin("__tint test__.rhai", src).expect("registers");
        assert_eq!(p.kind, Kind::Effect);
        assert!(crate::effects::global_effect_registry().get(&p.key).is_some_and(|i| i.provenance == crate::provenance::Provenance::Local));
        assert_eq!(p.carried.entry, p.key);
        assert!(plugins_of(Kind::Effect).iter().any(|q| q.file == p.file));
        unlist_plugin(&p.file);
        assert!(!plugins_of(Kind::Effect).iter().any(|q| q.file == p.file));
        crate::effects::global_effect_registry_mut().remove_by_name(&p.key);
        assert!(register_plugin("__lib__.rhai", "script(\"x\", \"generator\");\n").is_err());
    }

    /// A formula and a colouring plug-in register in the escape engine's
    /// registry under their slot names, which a config names.
    #[test]
    fn an_escape_plugin_registers_in_its_slot() {
        let formula = "const INFO = #{ title: \"Plug test\" };\nfn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\n\
                       fn step(s, p) { s.z = s.z * s.z + s.c; s }\nfn escaped(s, p) { s.z.norm_sqr() > 4.0 }\n";
        let p = register_plugin("__plug test__.rhai", formula).expect("registers");
        assert_eq!((p.kind, p.title.as_str()), (Kind::Formula, "Plug test"));
        assert!(p.key.starts_with(crate::escape::custom::PREFIX));
        assert!(crate::escape::custom::formula(&p.key).is_some(), "registered under its slot name");
        assert_eq!(p.carried.source, formula);
        unlist_plugin(&p.file);
    }

    /// A variation plug-in registers under its key. Writes the global
    /// variation registry: run alone (VARIATIONS.md, Testing).
    #[test]
    #[ignore = "writes the global variation registry: run alone"]
    fn a_variation_plugin_registers_under_its_key() {
        let src = "const INFO = #{ title: \"Plug var\", name: \"rhai_plug_test\" };\nfn variation(z, p) { z * 0.5 }\n";
        let p = register_plugin("__plug var__.rhai", src).expect("registers");
        assert_eq!(p.kind, Kind::Variation);
        assert!(crate::scripted::is_key(&p.key));
        assert!(crate::variations::global_registry().has(&p.key));
        unlist_plugin(&p.file);
        crate::variations::global_registry_mut().remove_by_name(&p.key);
    }

    /// `INFO.description` and `INFO.author` reach the entry, a
    /// description in the current language.
    #[test]
    fn a_formula_entry_reads_its_info() {
        let src = "const INFO = #{ title: \"Spiral\", description: #{ en: \"A spiral\", ja: \"螺旋\" }, author: \"Ada\" };\n\
                   fn variation(z, p) { z }\n";
        let e = formula_entry("spiral.rhai", src, Origin::Local, Link::default());
        assert_eq!((e.kind, e.title.as_str(), e.description.as_str(), e.author.as_str()), (Kind::Variation, "Spiral", "A spiral", "Ada"));
        assert!(crate::lang::rhai::transformation_any(src).is_ok(), "the checker accepts both");
        let uf = formula_entry("traps.ucl", "x {\n}\n", Origin::Api, Link::default());
        assert_eq!(uf.kind, Kind::UfColorings);
    }

    /// A source that opens with a def's doc block -- a twin of a built-in
    /// does -- has its prose and authors from it: the description in one
    /// paragraph as the API keeps it, the authors joined.
    #[test]
    fn a_doc_block_reaches_the_entry() {
        let src = "/// Voronoi cells, picked in a noise-distorted\n/// square lattice.\n///\n/// # Authors\n/// - slobo777\n/// - Andreas Maschke\n\
                   const INFO = #{ title: \"Crackle\", name: \"rhai_crackle\" };\nfn variation(z, p) { z }\n";
        let e = formula_entry("crackle.rhai", src, Origin::Local, Link::default());
        assert_eq!(e.description, "Voronoi cells, picked in a noise-distorted square lattice.");
        assert_eq!(e.author, "slobo777, Andreas Maschke");
        assert!(crate::lang::rhai::transformation_any(src).is_ok(), "the block is a comment to Rhai");
    }
}
