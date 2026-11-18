//! Managing the user's own Rhai files (docs/archive/projects/catalog.md, *Phase
//! 4*): what the Catalog's Rename, Duplicate, Delete, Import and New do,
//! over the two stores ([`crate::storage::library::Store`]) and their
//! rules -- a formula file is keyed by its name with its extension, a
//! script by its stem (a shipped stem is reserved) -- and what goes with a
//! file: its link record (cloud id, origin, the plug-in flag) and, for a
//! plug-in, its registration.
//!
//! A file is named as the Catalog names it (`EntryKey::File`): a formula
//! file's name, a script's stem with `.rhai`.
//!
//! - **Rename** keeps the link and the plug-in (re-registered under the
//!   new name); the text is untouched, so a published file stays that
//!   script.
//! - **Duplicate** is a new file: no cloud id, no plug-in flag, and an
//!   entry's `INFO.id` replaced (two files with one id would be one
//!   script to the server and the plug-in lists). A copy of somebody
//!   else's CPU script stays somebody else's -- it runs restricted
//!   (phase 4, decision 3).
//! - **Delete** unlists a plug-in and drops the link with the file.
//! - **Import** and **New** never overwrite: a taken name gains `-2`,
//!   `-3`, ...

use crate::script::store as scripts;
use crate::storage::formula_store as formulas;
use crate::storage::library::Store;
use crate::storage::links::Link;
use std::sync::atomic::{AtomicU64, Ordering};

/// Bumped by every change this module makes to a store: what the lists
/// that show the files (the Catalog, the editors) watch, beside the
/// script cloud's adoptions (`ScriptCloudState::library_generation`).
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// How many changes this module has made to the stores.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

fn changed() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// A script file's stem (`orbit.rhai` -> `orbit`).
fn stem(file: &str) -> &str {
    file.strip_suffix(".rhai").unwrap_or(file)
}

/// A file's text, if the store has it.
pub fn load(store: Store, file: &str) -> Option<String> {
    match store {
        Store::Formulas => formulas::load(file),
        Store::Scripts => scripts::load(stem(file)),
    }
}

/// A file's link record.
pub fn link_of(store: Store, file: &str) -> Option<Link> {
    match store {
        Store::Formulas => formulas::link_of(file),
        Store::Scripts => scripts::link_of(stem(file)),
    }
}

/// Record (or replace) a stored file's link.
pub fn set_link(store: Store, file: &str, link: Link) -> Result<(), String> {
    changed();
    match store {
        Store::Formulas => formulas::set_link(file, link),
        Store::Scripts => scripts::set_link(stem(file), link),
    }
}

/// The file `name` would be stored as: a formula file keeps `ext` when the
/// name has no known extension of its own; a script is its stem with
/// `.rhai`. `None` when the name has nothing usable, or names a shipped
/// script.
pub fn file_name(store: Store, name: &str, ext: &str) -> Option<String> {
    match store {
        Store::Formulas => {
            let has_ext = name.rsplit_once('.').is_some_and(|(_, e)| formulas::EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
            let name = if has_ext { name.to_string() } else { format!("{name}.{ext}") };
            formulas::file_name_for(&name)
        }
        Store::Scripts => {
            let s = scripts::stem_for(stem(name));
            (!s.is_empty() && !crate::script::library::is_builtin_stem(&s)).then(|| format!("{s}.rhai"))
        }
    }
}

/// The first of `name`, `name-2`, `name-3`, ... that the store does not
/// hold (and, for a script, no shipped script has).
pub fn free_name(store: Store, name: &str, ext: &str) -> Option<String> {
    let (base, ext) = match name.rsplit_once('.') {
        Some((b, e)) if formulas::EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()) => (b, e.to_ascii_lowercase()),
        _ => (name, ext.to_ascii_lowercase()),
    };
    std::iter::once(base.to_string())
        .chain((2..1000).map(|n| format!("{base}-{n}")))
        .filter_map(|candidate| file_name(store, &format!("{candidate}.{ext}"), &ext))
        .find(|f| load(store, f).is_none())
}

/// Write `text` as `file` (over it, when it is there): what Save does.
pub fn write(store: Store, file: &str, text: &str) -> Result<String, String> {
    changed();
    match store {
        Store::Formulas => formulas::save(file, text),
        Store::Scripts => scripts::save(stem(file), text).map(|s| format!("{s}.rhai")),
    }
}

fn remove(store: Store, file: &str) -> Result<(), String> {
    changed();
    match store {
        Store::Formulas => formulas::delete(file),
        Store::Scripts => scripts::delete(stem(file)),
    }
}

/// Rename `file` to `name` (a formula file keeps its extension unless the
/// name gives one); gives the new file. Its link and plug-in go with it.
pub fn rename(store: Store, file: &str, name: &str) -> Result<String, String> {
    let ext = file.rsplit_once('.').map_or("rhai", |(_, e)| e);
    let to = file_name(store, name, ext).ok_or_else(|| t!("manage.bad_name").to_string())?;
    if to == file {
        return Ok(to);
    }
    if load(store, &to).is_some() {
        return Err(t!("manage.name_taken", name = to).to_string());
    }
    let text = load(store, file).ok_or_else(|| t!("manage.missing", name = file).to_string())?;
    let link = link_of(store, file);
    let to = write(store, &to, &text)?;
    remove(store, file)?;
    if let Some(link) = link {
        set_link(store, &to, link.clone())?;
        if store == Store::Formulas && link.plugin {
            crate::storage::library::unlist_plugin(file);
            if let Err(e) = crate::storage::library::register_plugin(&to, &text) {
                log::warn!("The plug-in `{to}` did not register after its rename: {e}");
            }
        }
    }
    Ok(to)
}

/// A copy of `file`, under the first free `<name>-copy` name; gives the new
/// file. A new file of the user's: no cloud id, no plug-in, an entry's id
/// its own; a copy of somebody else's CPU script still runs restricted.
pub fn duplicate(store: Store, file: &str) -> Result<String, String> {
    let text = load(store, file).ok_or_else(|| t!("manage.missing", name = file).to_string())?;
    let (base, ext) = file.rsplit_once('.').unwrap_or((file, "rhai"));
    let to = free_name(store, &format!("{base}-copy"), ext).ok_or_else(|| t!("manage.bad_name").to_string())?;
    let text = with_own_new_id(store, &to, &text);
    let to = write(store, &to, &text)?;
    if store == Store::Scripts && link_of(store, file).is_some_and(|l| l.from_others) {
        set_link(store, &to, Link { from_others: true, ..Link::default() })?;
    }
    Ok(to)
}

/// A formula store Rhai entry's text with a fresh id of its own (when it
/// carries one, or is a kind that is published by id); other text as is.
fn with_own_new_id(store: Store, file: &str, text: &str) -> String {
    let rhai = store == Store::Formulas && formulas::is_rhai(file);
    if rhai && crate::lang::rhai::info_string(text, "id").is_some() {
        crate::api::entries::with_new_id(text, &crate::scripted::mint_id())
    } else {
        text.to_string()
    }
}

/// Delete `file`: its plug-in unlisted, its link dropped.
pub fn delete(store: Store, file: &str) -> Result<(), String> {
    if store == Store::Formulas {
        crate::storage::library::unlist_plugin(file);
    }
    remove(store, file)
}

/// Whether a stored file can be a plug-in: a Rhai entry of the formula
/// store of a kind registered at start ([`crate::storage::library::Kind::can_be_plugin`]).
pub fn can_be_plugin(store: Store, file: &str, text: &str) -> bool {
    store == Store::Formulas && formulas::is_rhai(file) && crate::storage::library::rhai_kind(text).can_be_plugin()
}

/// Make a stored file a plug-in, registered at every start, or stop;
/// gives what it registered as (its title), or the file. One that needs
/// an id of its own to register (a variation without a name, an effect)
/// gets one, written into the file.
pub fn set_plugin(file: &str, on: bool) -> Result<String, String> {
    let mut link = formulas::link_of(file).unwrap_or_default();
    if !on {
        link.plugin = false;
        crate::storage::library::unlist_plugin(file);
        set_link(Store::Formulas, file, link)?;
        return Ok(file.to_string());
    }
    let mut text = load(Store::Formulas, file).ok_or_else(|| t!("manage.missing", name = file).to_string())?;
    let needs_id = match crate::storage::library::rhai_kind(&text) {
        crate::storage::library::Kind::Variation => crate::variations::custom::needs_id(&text),
        crate::storage::library::Kind::Effect => crate::api::entries::needs_id(&text),
        _ => false,
    };
    if needs_id {
        text = crate::lang::rhai::with_own_id(&text, &crate::scripted::mint_id());
        write(Store::Formulas, file, &text)?;
    }
    let plugin = crate::storage::library::register_plugin(file, &text)?;
    link.plugin = true;
    set_link(Store::Formulas, file, link)?;
    Ok(plugin.title)
}

/// Why a stored file cannot be published, if it cannot: somebody else's
/// as adopted, still that online script (theirs to publish -- an edit or
/// a copy is the user's own, decision 3), and for the formula store what
/// the server refuses
/// (`api::entries::refusal`: a UF or Fractint file, a kind with no
/// online form, one that does not compile, a built-in unedited).
pub fn publish_refusal(store: Store, file: &str, text: &str) -> Option<String> {
    let link = link_of(store, file).unwrap_or_default();
    if link.from_others && link.cloud_id.is_some() {
        return Some(t!("manage.not_yours", owner = link.owner.unwrap_or_default()).to_string());
    }
    match store {
        Store::Formulas => crate::api::entries::refusal(file, text),
        Store::Scripts => None,
    }
}

/// A stored file's text ready to publish: a formula store entry is given
/// an id of its own first, written into the file -- what the server
/// stores it under, and what a config using it is catalogued by.
pub fn prepare_publish(store: Store, file: &str) -> Result<String, String> {
    let text = load(store, file).ok_or_else(|| t!("manage.missing", name = file).to_string())?;
    if store == Store::Formulas && crate::api::entries::needs_id(&text) {
        let text = crate::lang::rhai::with_own_id(&text, &crate::scripted::mint_id());
        write(store, file, &text)?;
        return Ok(text);
    }
    Ok(text)
}

/// The store a Rhai text belongs in: a CPU script declares itself
/// (`script("Name", "generator")`), everything else is the formula
/// store's (Rhai entries, UF and Fractint files).
pub fn store_of(name: &str, text: &str) -> Store {
    let rhai = name.to_ascii_lowercase().ends_with(".rhai") || !name.contains('.');
    if rhai && crate::script::library::declared(text).is_some() {
        Store::Scripts
    } else {
        Store::Formulas
    }
}

/// Bring in a file's text under a free name near `name`, in the store it
/// belongs in; gives where it went. Nothing is overwritten.
pub fn import(name: &str, text: &str) -> Result<(Store, String), String> {
    let store = store_of(name, text);
    let (base, ext) = match name.rsplit_once('.') {
        Some((b, e)) => (b, e.to_ascii_lowercase()),
        None => (name, "rhai".to_string()),
    };
    let to = free_name(store, base, &ext).ok_or_else(|| t!("manage.bad_name").to_string())?;
    Ok((store, write(store, &to, text)?))
}

/// A new file from `text` (a template), under a free name near `name`, in
/// the store the text belongs in; an entry gets an id of its own.
pub fn create(name: &str, text: &str) -> Result<(Store, String), String> {
    let store = store_of(&format!("{name}.rhai"), text);
    let to = free_name(store, name, "rhai").ok_or_else(|| t!("manage.bad_name").to_string())?;
    let text = if store == Store::Formulas && crate::storage::library::rhai_kind(text).can_be_plugin() {
        crate::lang::rhai::with_own_id(text, &crate::scripted::mint_id())
    } else {
        text.to_string()
    };
    Ok((store, write(store, &to, &text)?))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    const ENTRY: &str = "const INFO = #{ title: \"It Spiral\", id: \"5d4f1c0e-1111-4222-8333-944445555666\" };\n\
fn start(pixel, p) { #{ z: pixel } }\nfn step(s, p) { s.z = s.z * s.z; s }\nfn escaped(s, p) { s.z.norm_sqr() > 4.0 }\n";
    const SCRIPT: &str = "script(\"It Manage\", \"generator\", []);\n";

    fn tag() -> String {
        crate::scripted::mint_id()[..8].to_string()
    }

    /// A formula file renamed keeps its extension, its link and its
    /// plug-in flag; the old name is free again; a taken name is refused.
    #[test]
    fn a_rename_keeps_the_link_and_refuses_a_taken_name() {
        let t = tag();
        let file = formulas::save(&format!("it_manage_{t}.rhai"), ENTRY).unwrap();
        formulas::set_link(&file, Link { cloud_id: Some("cloud-1".into()), ..Link::default() }).unwrap();
        let other = formulas::save(&format!("it_other_{t}.rhai"), ENTRY).unwrap();

        let refused = rename(Store::Formulas, &file, &format!("it_other_{t}"));
        assert!(refused.is_err(), "a taken name is refused");
        let to = rename(Store::Formulas, &file, &format!("it_renamed_{t}")).unwrap();
        assert_eq!(to, format!("it_renamed_{t}.rhai"), "the extension kept");
        assert!(load(Store::Formulas, &file).is_none(), "the old name is free");
        assert_eq!(load(Store::Formulas, &to).as_deref(), Some(ENTRY), "the text untouched");
        assert_eq!(link_of(Store::Formulas, &to).and_then(|l| l.cloud_id).as_deref(), Some("cloud-1"), "the link goes with it");
        assert!(link_of(Store::Formulas, &file).is_none());

        delete(Store::Formulas, &to).unwrap();
        delete(Store::Formulas, &other).unwrap();
        assert!(link_of(Store::Formulas, &to).is_none(), "a delete drops the link");
    }

    /// A copy is a new file of the user's: no cloud id, an entry's id its
    /// own; a copy of somebody else's CPU script still runs restricted.
    #[test]
    fn a_duplicate_is_the_users_with_its_own_id() {
        let t = tag();
        let file = formulas::save(&format!("it_dup_{t}.rhai"), ENTRY).unwrap();
        formulas::set_link(&file, Link { cloud_id: Some("cloud-2".into()), plugin: true, ..Link::default() }).unwrap();
        let copy = duplicate(Store::Formulas, &file).unwrap();
        assert_eq!(copy, format!("it_dup_{t}-copy.rhai"));
        let copy2 = duplicate(Store::Formulas, &file).unwrap();
        assert_eq!(copy2, format!("it_dup_{t}-copy-2.rhai"), "never over the first copy");
        let text = load(Store::Formulas, &copy).unwrap();
        let id = crate::lang::rhai::info_string(&text, "id").unwrap();
        assert!(crate::scripted::is_uuid(&id) && id != "5d4f1c0e-1111-4222-8333-944445555666", "an id of its own: {id}");
        assert!(link_of(Store::Formulas, &copy).is_none(), "no cloud id, no plug-in");
        for f in [&file, &copy, &copy2] {
            delete(Store::Formulas, f).unwrap();
        }

        let stem = format!("it_dup_script_{t}");
        scripts::save(&stem, SCRIPT).unwrap();
        scripts::set_link(&stem, Link { cloud_id: Some("cloud-3".into()), from_others: true, ..Link::default() }).unwrap();
        let copy = duplicate(Store::Scripts, &format!("{stem}.rhai")).unwrap();
        assert_eq!(copy, format!("{stem}-copy.rhai"));
        let link = link_of(Store::Scripts, &copy).unwrap();
        assert!(link.from_others && link.cloud_id.is_none(), "still restricted, not that online script: {link:?}");
        delete(Store::Scripts, &copy).unwrap();
        delete(Store::Scripts, &format!("{stem}.rhai")).unwrap();
    }

    /// Import puts a CPU script in the scripts and anything else in the
    /// formulas, never over a file that is there; a shipped script's
    /// name is not the user's to take.
    #[test]
    fn import_finds_the_store_and_never_overwrites() {
        let t = tag();
        let (store, a) = import(&format!("it_import_{t}.rhai"), SCRIPT).unwrap();
        assert_eq!((store, a.as_str()), (Store::Scripts, format!("it_import_{t}.rhai").as_str()));
        let (_, b) = import(&format!("it_import_{t}.rhai"), SCRIPT).unwrap();
        assert_eq!(b, format!("it_import_{t}-2.rhai"), "the first stays");
        let (store, c) = import(&format!("it_import_{t}.ufm"), "Spiral {\nz = pixel:\nz = z*z + #pixel\n|z| < 4\n}\n").unwrap();
        assert_eq!((store, c.as_str()), (Store::Formulas, format!("it_import_{t}.ufm").as_str()));
        delete(Store::Scripts, &a).unwrap();
        delete(Store::Scripts, &b).unwrap();
        delete(Store::Formulas, &c).unwrap();

        let shipped = crate::script::library::EMBEDDED[0].0;
        assert!(file_name(Store::Scripts, shipped, "rhai").is_none(), "{shipped} is a shipped script's name");
    }

    /// New gives an entry an id of its own, in the store its text says.
    #[test]
    fn a_new_entry_has_an_id_of_its_own() {
        let t = tag();
        let template = "const INFO = #{ title: \"New\" };\nfn start(pixel, p) { #{ z: pixel } }\nfn step(s, p) { s }\nfn escaped(s, p) { false }\n";
        let (store, file) = create(&format!("it_new_{t}"), template).unwrap();
        assert_eq!(store, Store::Formulas);
        let text = load(store, &file).unwrap();
        assert!(crate::lang::rhai::info_string(&text, "id").is_some_and(|id| crate::scripted::is_uuid(&id)));
        delete(store, &file).unwrap();
    }
}
