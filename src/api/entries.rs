//! A formula-library entry on the scripts resource
//! (docs/projects/rhai-scripts-online.md): one GPU Rhai script -- a
//! formula, colouring, solid, field, field colouring, simulation model or
//! colouring, variation, effect or track -- published under its own
//! `INFO.id`, so the catalog's `script:<uuid>` type and the stored script
//! are one id. The first to publish an id owns it (the server answers 409
//! to anyone after).
//!
//! What is never sent: a source that does not compile, a built-in's
//! shipped source unedited (it is the built-in, and the catalog groups it
//! so), and a UF or Fractint file (many entries, no id).

use crate::api::types::{ApiVisibility, ScriptCreateRequest};
use crate::lang::rhai;
use crate::storage::library::{self, Kind};

/// Why the formula store's `file` (its text `source`) cannot be
/// published; `None` when it can, once it has an id of its own
/// ([`needs_id`]).
pub fn refusal(file: &str, source: &str) -> Option<String> {
    if !crate::storage::formula_store::is_rhai(file) {
        return Some(
            "an Ultra Fractal or Fractint file holds many entries and no id: \
             convert the entry to Rhai (Edit as Rhai) and publish that"
                .into(),
        );
    }
    let kind = library::rhai_kind(source);
    if kind.wire().is_none() {
        return Some("this file is not a formula, colouring, solid, field, variation, effect, simulation or track".into());
    }
    if let Some(e) = compile_error(kind, source) {
        return Some(format!("it does not compile, so it cannot be published: {e}"));
    }
    if let Some(builtin) = shipped_of(source) {
        return Some(format!("this is the built-in `{builtin}` unedited: it is already everybody's"));
    }
    None
}

/// Whether `source` needs an id of its own written in before it is
/// published: it has no `INFO.id`, or one that is not a UUID (a copy of a
/// built-in's, one typed by hand). `lang::rhai::with_own_id` gives it one.
pub fn needs_id(source: &str) -> bool {
    !rhai::info_string(source, "id").is_some_and(|id| crate::scripted::is_uuid(&id))
}

/// `source` with `id` in place of its own UUID (`INFO.id`), or given
/// one (`lang::rhai::with_own_id`) when it has none: what saving an edit
/// of somebody else's published entry does, so the edit is a script of
/// its own.
pub fn with_new_id(source: &str, id: &str) -> String {
    match rhai::info_string(source, "id").filter(|have| crate::scripted::is_uuid(have)) {
        Some(have) => source.replacen(&format!("\"{have}\""), &format!("\"{id}\""), 1),
        None => rhai::with_own_id(source, id),
    }
}

/// The create (or, with `version`, update) body for the formula store's
/// `file`. Refused as [`refusal`] refuses, and when it has no id of its
/// own.
pub fn create_request(
    file: &str,
    source: &str,
    visibility: Option<ApiVisibility>,
    version: Option<u32>,
) -> Result<ScriptCreateRequest, String> {
    if let Some(why) = refusal(file, source) {
        return Err(why);
    }
    let id = rhai::info_string(source, "id")
        .filter(|id| crate::scripted::is_uuid(id))
        .ok_or("it has no id of its own yet: save it from the Formula Editor first")?;
    let kind = library::rhai_kind(source);
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s).to_string();
    // The server's copy is read in every language: the plain text (or
    // English) of a label, not this session's.
    let (title, description, author) = match rhai::source_labels(source) {
        Some(l) => (l.title.resolve_in("en"), l.description.resolve_in("en"), l.author),
        None => (String::new(), String::new(), String::new()),
    };
    let title = if !title.is_empty() {
        title
    } else if kind == Kind::Track {
        crate::animation::script::declare(source).map(|d| d.title).unwrap_or_default()
    } else {
        String::new()
    };
    Ok(ScriptCreateRequest {
        name: stem.clone(),
        display_name: if title.is_empty() { stem } else { title },
        kind: kind.wire().expect("refusal checked the kind").to_string(),
        // The names the source credits, as written: credit, not
        // ownership (the uploader is the server's `user_id`).
        authors: if author.is_empty() { Vec::new() } else { vec![author] },
        description: (!description.is_empty()).then_some(description),
        source: source.to_string(),
        flags: Vec::new(),
        visibility,
        version,
        id: Some(id),
    })
}

/// The first compile error of `source` as `kind`, the Formula Editor's
/// first check of each (`recheck_rhai`).
fn compile_error(kind: Kind, source: &str) -> Option<String> {
    let checked = match kind {
        Kind::Formula => rhai::program(source),
        Kind::Coloring => rhai::coloring(source),
        Kind::Variation => rhai::transformation_any(source),
        Kind::Solid => rhai::solid(source),
        Kind::Effect => rhai::effect(source),
        Kind::SimColoring => rhai::sim_coloring(source),
        Kind::SimModel => rhai::sim_model(source, rhai::SimFn::Step(0)),
        Kind::Field => rhai::field(source),
        Kind::FieldColoring => rhai::field_coloring(source),
        Kind::Track => return crate::animation::script::try_run(source).err(),
        _ => return Some("not a Rhai entry".into()),
    };
    checked.err().map(|d| d.iter().map(|x| x.render(source)).collect::<Vec<_>>().join("; "))
}

/// The built-in whose shipped source `source` is, unedited: any of the
/// escape, effect, simulation and variation twins and the formula store's
/// examples, with its id aside (none, a built-in's name, or the UUID Edit
/// as Rhai or Use writes in, where either puts it) and line ends aside.
pub fn shipped_of(source: &str) -> Option<&'static str> {
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    fn hash(text: &str) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        h.finish()
    }
    static SHIPPED: std::sync::OnceLock<HashMap<u64, &'static str>> = std::sync::OnceLock::new();
    let shipped = SHIPPED.get_or_init(|| {
        let tables: [&[(&'static str, &'static str)]; 10] = [
            crate::escape::twins::TWINS,
            crate::escape::twins::COLORING_TWINS,
            crate::escape::twins::SOLID_TWINS,
            crate::escape::twins::FIELD_TWINS,
            crate::escape::twins::FIELD_COLORING_TWINS,
            crate::effects::custom::BUILTIN_TWINS,
            crate::sim::custom::SHIPPED,
            crate::sim::custom::SHIPPED_MODELS,
            crate::variations::twins::TWINS,
            crate::storage::formula_store::EXAMPLES,
        ];
        tables.iter().flat_map(|t| t.iter()).map(|(name, text)| (hash(&bare(text)), *name)).collect()
    });
    shipped.get(&hash(&bare(source))).copied()
}

/// `text` with its line ends and its id aside: the `INFO` line
/// `with_info_id` adds, else the field it adds, else the id's value.
fn bare(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let Some(id) = rhai::info_string(&text, "id") else { return text };
    let line = format!("const INFO = #{{ id: \"{id}\" }};\n");
    let field = format!(" id: \"{id}\",");
    if text.contains(&line) {
        text.replacen(&line, "", 1)
    } else if text.contains(&field) {
        text.replacen(&field, "", 1)
    } else {
        text.replacen(&format!("\"{id}\""), "\"@id\"", 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "6f1c2a7e-0b9d-4c3e-8a51-2f7d9e4b6c10";

    fn own(source: &str) -> String {
        rhai::with_own_id(source, ID)
    }

    #[test]
    fn an_entry_of_its_own_is_published_under_its_id() {
        let src = own("const INFO = #{ title: \"Spiral\", author: \"Somebody\" };\n// Squares and adds.\nfn start(pixel, p) { #{ z: pixel } }\nfn step(s, p) { s.z = s.z * s.z; s }\nfn escaped(s, p) { s.z.norm_sqr() > 4.0 }\n");
        assert!(!needs_id(&src));
        let req = create_request("spiral.rhai", &src, Some(ApiVisibility::Public), None).unwrap();
        assert_eq!(req.kind, "formula");
        assert_eq!(req.id.as_deref(), Some(ID));
        assert_eq!(req.name, "spiral");
        assert_eq!(req.display_name, "Spiral");
        assert_eq!(req.authors, ["Somebody"]);
        assert!(req.source.contains(ID), "the server checks the id is the source's");
        let body = serde_json::to_value(&req).unwrap();
        assert_eq!(body["id"], ID);
        assert!(body.get("version").is_none());
    }

    #[test]
    fn a_source_without_an_id_of_its_own_is_not_sent() {
        let src = "const INFO = #{ title: \"Bare\" };\nfn variation(z, p) {\n    z\n}\n";
        assert!(needs_id(src));
        assert!(refusal("bare.rhai", src).is_none(), "refused only for want of an id");
        assert!(create_request("bare.rhai", src, None, None).is_err());
        assert_eq!(create_request("bare.rhai", &own(src), None, None).unwrap().kind, "variation");
    }

    #[test]
    fn every_shipped_source_unedited_is_refused_with_or_without_an_id() {
        let mb = crate::escape::twins::source("mandelbrot").unwrap();
        assert_eq!(shipped_of(mb), Some("mandelbrot"));
        assert_eq!(shipped_of(&own(mb)), Some("mandelbrot"), "the id Use writes in is aside");
        let lf = mb.replace("\r\n", "\n");
        assert_eq!(shipped_of(&lf), Some("mandelbrot"), "line ends aside");
        assert_eq!(shipped_of(&lf.replace('\n', "\r\n")), Some("mandelbrot"));
        assert!(refusal("mandelbrot.rhai", &own(mb)).is_some());
        let julia = crate::variations::twins::source("julia").unwrap();
        assert_eq!(shipped_of(&own(julia)), Some("julia"));
        let vignette = crate::effects::custom::BUILTIN_TWINS[0].1;
        assert_eq!(shipped_of(&own(vignette)), Some("vignette"));
        let gray_scott = crate::sim::custom::SHIPPED_MODELS[0];
        assert_eq!(shipped_of(&own(gray_scott.1)), Some(gray_scott.0));
        // One edit and it is somebody's own.
        let edited = own(&mb.replacen("4.0", "8.0", 1));
        assert_ne!(edited, own(mb), "the edit changed something");
        assert_eq!(shipped_of(&edited), None);
    }

    #[test]
    fn what_is_never_sent() {
        assert!(refusal("collection.ufm", "Spiral { loop: z = z*z bailout: |z| < 4 }").is_some());
        assert!(refusal("old.frm", "x { z = 0: z = z*z, |z| < 4 }").is_some());
        assert!(refusal("broken.rhai", &own("fn step(s, p) { s.z = ; s }")).is_some());
        assert!(refusal("nothing.rhai", &own("fn helper(x) { x }")).is_some());
    }

    /// Saving an edit of somebody else's entry: its UUID replaced, the
    /// rest of the text as it was.
    #[test]
    fn an_edit_of_somebody_elses_entry_gets_an_id_of_its_own() {
        let theirs = own("const INFO = #{ title: \"Theirs\" };\nfn variation(z, p) {\n    z * 2.0\n}\n");
        let new = "0d4b8a3c-9e2f-4a17-b6c5-1e8f7a2d3c40";
        let mine = with_new_id(&theirs, new);
        assert_eq!(rhai::info_string(&mine, "id").as_deref(), Some(new));
        assert_eq!(mine.replace(new, ID), theirs);
        let bare = "fn variation(z, p) {\n    z\n}\n";
        assert_eq!(rhai::info_string(&with_new_id(bare, new), "id").as_deref(), Some(new), "none: given one");
    }

    #[test]
    fn every_kind_has_a_wire_name_and_back() {
        for k in library::ALL_PUBLISHED {
            let w = k.wire().unwrap();
            assert_eq!(Kind::from_wire(w), Some(*k), "{w}");
        }
        assert_eq!(Kind::from_wire("hologram"), None);
        assert_eq!(Kind::UfFormulas.wire(), None);
    }
}
