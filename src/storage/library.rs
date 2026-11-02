//! One library over the user's two stores (docs/projects/rhai-plugins.md,
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
    /// An Ultra Fractal or Fractint file: many entries, of its kind.
    UfFormulas,
    UfColorings,
    Fractint,
    /// A file whose kind could not be read (it does not parse, or this
    /// build has no compiler).
    Unknown,
}

impl Kind {
    /// A GPU Rhai entry that can be a plug-in (`storage::links::Link::plugin`).
    pub fn can_be_plugin(self) -> bool {
        matches!(self, Kind::Variation | Kind::Effect | Kind::Formula | Kind::Coloring | Kind::Solid)
    }
}

/// Where an entry came from (design rule 7's namespaces, D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Ships with the app: read-only, edited as a copy.
    Shipped,
    /// The user's own.
    Yours,
    /// Somebody else's, adopted from the online library.
    Downloaded,
}

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
        out.push(script_entry(name, source, Origin::Shipped, Link::default()));
    }
    for (name, source) in crate::storage::formula_store::EXAMPLES {
        out.push(formula_entry(name, source, Origin::Shipped, Link::default()));
    }
    #[cfg(feature = "lang")]
    for (name, source) in crate::effects::custom::BUILTIN_TWINS {
        out.push(formula_entry(&format!("{name}.rhai"), source, Origin::Shipped, Link::default()));
    }
    let script_links = crate::script::store::load_links();
    for (stem, source) in crate::script::store::list() {
        let link = script_links.get(&stem).cloned().unwrap_or_default();
        out.push(script_entry(&format!("{stem}.rhai"), &source, origin_of(&link), link));
    }
    let formula_links = crate::storage::formula_store::links();
    for file in crate::storage::formula_store::list() {
        let Some(source) = crate::storage::formula_store::load(&file) else { continue };
        let link = formula_links.get(&file).cloned().unwrap_or_default();
        out.push(formula_entry(&file, &source, origin_of(&link), link));
    }
    out.sort_by_key(|e| (e.origin != Origin::Shipped, e.store == Store::Formulas, e.title.to_lowercase()));
    out
}

fn origin_of(link: &Link) -> Origin {
    if link.from_others {
        Origin::Downloaded
    } else {
        Origin::Yours
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
    #[cfg(feature = "lang")]
    {
        use crate::lang::{rhai, EntryKind};
        e.kind = match rhai::kind_of(source) {
            Some(EntryKind::Formula) => Kind::Formula,
            Some(EntryKind::Coloring) => Kind::Coloring,
            Some(EntryKind::Transform) => Kind::Variation,
            Some(EntryKind::Solid) => Kind::Solid,
            Some(EntryKind::Effect) => Kind::Effect,
            None => Kind::Unknown,
        };
        if let Some(labels) = rhai::source_labels(source) {
            let title = labels.title.resolve();
            if !title.is_empty() {
                e.title = title;
            }
            e.description = labels.description.resolve();
            e.author = labels.author;
        }
        e.id = rhai::info_string(source, "id");
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped entries are all there, each read: the script starters
    /// by their declarations, the formula examples and the effects by
    /// their kinds and titles.
    #[test]
    fn the_scan_reads_every_shipped_entry() {
        let all = scan();
        let shipped: Vec<&Entry> = all.iter().filter(|e| e.origin == Origin::Shipped).collect();
        let scripts = shipped.iter().filter(|e| e.store == Store::Scripts).count();
        assert_eq!(scripts, crate::script::library::EMBEDDED.len());
        assert!(shipped.iter().filter(|e| e.store == Store::Scripts).all(|e| e.kind != Kind::Unknown && !e.title.is_empty()));
        let basic = shipped.iter().find(|e| e.file == "basic_random.rhai").expect("a starter");
        assert_eq!((basic.kind, basic.title.as_str()), (Kind::Generator, "Basic Random"));
        #[cfg(feature = "lang")]
        {
            let vignette = shipped.iter().find(|e| e.file == "vignette.rhai" && e.kind == Kind::Effect).expect("the effect");
            assert_eq!((vignette.title.as_str(), vignette.id.as_deref()), ("Vignette", Some("vignette")));
            assert_eq!(vignette.version.len(), 8);
            assert!(shipped.iter().any(|e| e.kind == Kind::Formula), "the formula examples");
        }
    }

    /// `INFO.description` and `INFO.author` reach the entry, a
    /// description in the current language.
    #[cfg(feature = "lang")]
    #[test]
    fn a_formula_entry_reads_its_info() {
        let src = "const INFO = #{ title: \"Spiral\", description: #{ en: \"A spiral\", ja: \"螺旋\" }, author: \"Ada\" };\n\
                   fn variation(z, p) { z }\n";
        let e = formula_entry("spiral.rhai", src, Origin::Yours, Link::default());
        assert_eq!((e.kind, e.title.as_str(), e.description.as_str(), e.author.as_str()), (Kind::Variation, "Spiral", "A spiral", "Ada"));
        assert!(crate::lang::rhai::transformation_any(src).is_ok(), "the checker accepts both");
        let uf = formula_entry("traps.ucl", "x {\n}\n", Origin::Downloaded, Link::default());
        assert_eq!(uf.kind, Kind::UfColorings);
    }
}
