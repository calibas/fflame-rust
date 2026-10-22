//! The user's formula files: Rhai formulas (`.rhai`, one formula a
//! file: the language's own), and the Ultra Fractal (`.ufm` formulas,
//! `.ucl` colourings) and Fractint (`.frm`) files it imports, edited in
//! the Formula Editor (`docs/projects/escape-formula-language.md`).
//!
//! One store on both platforms, through [`crate::storage::backend`]: a
//! `formulas/` folder in the app's data directory on desktop, keys under
//! `formulas/` in localStorage on the web, where it is the only copy. A
//! file keeps its extension, because the extension says which language
//! it is in and what its entries are.

use std::path::{Path, PathBuf};

use crate::storage::backend;

const PREFIX: &str = "formulas";

/// The extensions a formula file may have.
pub const EXTENSIONS: &[&str] = &["rhai", "ufm", "ucl", "frm"];

/// The example files the editor offers, written for this project (no
/// third-party formulas ship until their terms are checked).
pub const EXAMPLES: &[(&str, &str)] = &[
    ("mandelbrot.rhai", include_str!("../../assets/formulas/mandelbrot.rhai")),
    ("burning-ship.rhai", include_str!("../../assets/formulas/burning-ship.rhai")),
    ("phoenix.rhai", include_str!("../../assets/formulas/phoenix.rhai")),
    ("smooth.rhai", include_str!("../../assets/formulas/smooth.rhai")),
    ("orbit-trap.rhai", include_str!("../../assets/formulas/orbit-trap.rhai")),
    ("variation-whorl.rhai", include_str!("../../assets/formulas/variation-whorl.rhai")),
    ("variation-scatter.rhai", include_str!("../../assets/formulas/variation-scatter.rhai")),
    ("solid-mandelbox.rhai", include_str!("../../assets/formulas/solid-mandelbox.rhai")),
    ("solid-kifs-menger.rhai", include_str!("../../assets/formulas/solid-kifs-menger.rhai")),
    ("solid-inverted-menger.rhai", include_str!("../../assets/formulas/solid-inverted-menger.rhai")),
    ("kifs-2d.rhai", include_str!("../../assets/formulas/kifs-2d.rhai")),
    ("whorled-mandelbrot.rhai", include_str!("../../assets/formulas/whorled-mandelbrot.rhai")),
    ("frothy-basin.rhai", include_str!("../../assets/formulas/frothy-basin.rhai")),
    ("frothy-basin-coloring.rhai", include_str!("../../assets/formulas/frothy-basin-coloring.rhai")),
    ("volterra-lotka.rhai", include_str!("../../assets/formulas/volterra-lotka.rhai")),
    ("unity.rhai", include_str!("../../assets/formulas/unity.rhai")),
    ("examples.ufm", include_str!("../../assets/formulas/examples.ufm")),
    ("examples.ucl", include_str!("../../assets/formulas/examples.ucl")),
];

/// Largest file we store. Formula files are small; the largest public
/// collections run to a few hundred kilobytes, and on the web the
/// localStorage quota is shared with everything else.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

/// The name [`save`] would store `name` under: the stem sanitized (as the
/// scripts' store does: nothing outside `[alphanumeric-_]` survives, so
/// no path can), and a known extension, `.ufm` when it has none.
pub fn file_name_for(name: &str) -> Option<String> {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()) => (s, e.to_ascii_lowercase()),
        _ => (name, "ufm".to_string()),
    };
    let safe: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe = safe.trim_matches('_');
    (!safe.is_empty()).then(|| format!("{safe}.{ext}"))
}

fn key_for(file: &str) -> PathBuf {
    Path::new(PREFIX).join(file)
}

/// Write a file; returns the name it was stored under.
pub fn save(name: &str, text: &str) -> Result<String, String> {
    let file = file_name_for(name).ok_or("that name has nothing usable in it")?;
    if text.len() > MAX_FILE_BYTES {
        return Err(format!("that file is {} KB; the limit is {} KB", text.len() / 1024, MAX_FILE_BYTES / 1024));
    }
    backend::write_file(&key_for(&file), text).map_err(|e| format!("cannot save `{file}`: {e}"))?;
    Ok(file)
}

pub fn load(file: &str) -> Option<String> {
    backend::read_file(&key_for(file)).ok()
}

pub fn delete(file: &str) -> Result<(), String> {
    backend::delete_file(&key_for(file)).map_err(|e| format!("cannot delete `{file}`: {e}"))
}

/// Every formula file in the store, sorted.
pub fn list() -> Vec<String> {
    let mut names: Vec<String> = backend::list_entries(Path::new(PREFIX))
        .unwrap_or_default()
        .into_iter()
        .filter(|n| n.rsplit_once('.').is_some_and(|(_, e)| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str())))
        .collect();
    names.sort_by_key(|n| n.to_ascii_lowercase());
    names
}

/// The language a file name says it is in.
pub fn is_fractint(file: &str) -> bool {
    file.to_ascii_lowercase().ends_with(".frm")
}

/// A Rhai formula (`.rhai`): one formula, the whole file.
pub fn is_rhai(file: &str) -> bool {
    file.to_ascii_lowercase().ends_with(".rhai")
}

/// A colouring file (`.ucl`): its entries are colourings.
pub fn is_coloring_file(file: &str) -> bool {
    file.to_ascii_lowercase().ends_with(".ucl")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized_and_keep_a_known_extension() {
        assert_eq!(file_name_for("My Formulas.ufm").as_deref(), Some("My_Formulas.ufm"));
        assert_eq!(file_name_for("traps.UCL").as_deref(), Some("traps.ucl"));
        assert_eq!(file_name_for("old").as_deref(), Some("old.ufm"));
        assert_eq!(file_name_for("../../etc/passwd").as_deref(), Some("etc_passwd.ufm"));
        assert_eq!(file_name_for("a.txt").as_deref(), Some("a_txt.ufm"));
        assert_eq!(file_name_for("..."), None);
        assert!(is_fractint("FRACTINT.FRM") && is_coloring_file("x.ucl"));
        assert_eq!(file_name_for("Spiral.RHAI").as_deref(), Some("Spiral.rhai"));
        assert!(is_rhai("a.rhai") && !is_rhai("a.ufm"));
    }
}
