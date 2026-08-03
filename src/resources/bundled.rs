//! Where the shipped `assets/` and `shaders/` trees actually live at runtime.
//!
//! Every reference to them in the codebase is written repo-relative
//! (`assets/presets.fflame`, `shaders/effects/...`), which resolves against
//! the CURRENT WORKING DIRECTORY. That is right for `cargo run` from the
//! repo and for a zip the user unpacked and ran from inside, and wrong the
//! moment the process is started from anywhere else.
//!
//! A macOS `.app` is exactly that case: Finder launches the executable with
//! the working directory set to `/`, so every one of those paths misses.
//! The app still starts — the loads are individually non-fatal — and the
//! user gets no presets, no palette packs and no CJK fonts, with nothing in
//! the UI saying why. Windows has the same hazard via a shortcut with an
//! unset "Start in".
//!
//! So: resolve against the executable as well as the working directory.
//!
//! # Order, and why the cwd wins
//!
//! 1. the working directory — today's behaviour, unchanged
//! 2. beside the executable — `build.rs` copies both trees to
//!    `target/<profile>/`, and a zip release has the same shape
//! 3. `../Resources` relative to the executable — the macOS bundle layout,
//!    where the binary sits in `Contents/MacOS` and Apple's convention puts
//!    data in `Contents/Resources`
//!
//! The working directory is tried FIRST so that a developer editing
//! `assets/` in the repo sees their edits immediately, exactly as before,
//! rather than a stale copy under `target/`. This ordering makes the change
//! purely additive: any layout that worked before still resolves the same
//! way, and the fallbacks only engage where the old code found nothing at
//! all.
//!
//! Returns the repo-relative path unchanged when no candidate exists, so
//! callers keep reporting the path the user would recognise.

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// Resolve a shipped-resource path (`assets/...`, `shaders/...`).
#[cfg(not(target_arch = "wasm32"))]
pub fn resource_path(relative: impl AsRef<Path>) -> PathBuf {
    let relative = relative.as_ref();

    // An absolute path is the caller's own business.
    if relative.is_absolute() {
        return relative.to_path_buf();
    }

    if relative.exists() {
        return relative.to_path_buf();
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join(relative);
            if beside.exists() {
                return beside;
            }
            // Contents/MacOS/<exe> -> Contents/Resources/<relative>
            let bundled = dir.join("../Resources").join(relative);
            if bundled.exists() {
                return bundled;
            }
        }
    }

    relative.to_path_buf()
}

/// WASM has no filesystem; resources come over HTTP. Kept so callers do not
/// need their own `cfg`.
#[cfg(target_arch = "wasm32")]
pub fn resource_path(relative: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    relative.as_ref().to_path_buf()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn existing_relative_paths_are_untouched() {
        // Run from the repo root, so this is the working-directory branch —
        // the one every existing deployment already relies on.
        assert_eq!(resource_path("Cargo.toml"), PathBuf::from("Cargo.toml"));
    }

    #[test]
    fn absolute_paths_pass_through() {
        let abs = std::env::temp_dir().join("fflame-nonexistent-probe");
        assert_eq!(resource_path(&abs), abs);
    }

    #[test]
    fn unresolvable_paths_keep_their_original_form() {
        // Callers report this in error messages; rewriting it to some
        // absolute path the user never typed would make those messages worse.
        let missing = "assets/definitely-not-here.json";
        assert_eq!(resource_path(missing), PathBuf::from(missing));
    }

    #[test]
    fn falls_back_to_the_directory_holding_the_executable() {
        // `build.rs` copies assets/ and shaders/ next to the test binary, so
        // this exercises the real fallback rather than a synthetic one.
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()));
        let Some(exe_dir) = exe_dir else { return };
        if !exe_dir.join("shaders").is_dir() {
            return; // no copy in this layout; nothing to assert
        }
        // A path that cannot resolve against the cwd, but can beside the exe.
        let probe = std::path::Path::new("shaders/tonemap.wgsl");
        if probe.exists() {
            return; // cwd would win; this test cannot isolate the fallback
        }
        assert_eq!(resource_path(probe), exe_dir.join(probe));
    }
}
