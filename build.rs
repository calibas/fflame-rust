/// Build script to capture version information and auto-increment build numbers
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Tell Cargo to rerun this build script if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // Get version from Cargo.toml
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=APP_VERSION={}", version);

    // Get target triple
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_TARGET={}", target);

    // Get build profile.
    //
    // NOT `PROFILE`: cargo sets that to "release" or "debug" only — a
    // custom profile reports the one it inherits from. So a `dist`
    // build (LTO, stripped, panic=abort) was indistinguishable from a
    // developer's `release` build in every exported PNG, which defeats
    // the point of embedding build provenance at all: a bug report is
    // supposed to identify the binary it came from.
    //
    // OUT_DIR carries the real directory name — `target/dist/build/...`
    // — and the asset copy below already relies on that shape.
    let profile = env::var("OUT_DIR")
        .ok()
        .and_then(|dir| {
            std::path::PathBuf::from(dir)
                .ancestors()
                .nth(3) // .../target/<profile>/build/<crate>-<hash>/out
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        })
        .or_else(|| env::var("PROFILE").ok())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_PROFILE={}", profile);

    // Rebuild when the commit changes, or GIT_HASH silently goes stale.
    //
    // Cargo only re-runs this script when a declared input changes, and
    // none of the others move on `git commit`. The value was therefore
    // baked at whatever commit happened to trigger the last rerun and
    // stayed there: a census regenerated at 5317d46c labelled itself
    // 8912253b, several commits behind. That label is the provenance of
    // every generated report AND of every exported PNG — the thing
    // `docs/RELEASE.md` relies on to identify which binary produced a
    // bug report — so a wrong one is worse than none.
    //
    // Both files are needed and neither alone suffices: `.git/HEAD`
    // changes on checkout but not on commit, while the ref it names
    // changes on commit but not on checkout. Missing files are fine —
    // cargo treats an absent rerun-if-changed path as "always rerun",
    // which is the safe direction, and a tarball with no `.git` still
    // builds (the hash falls back to "unknown" below).
    let git_dir = std::path::Path::new(".git");
    if git_dir.exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
        if let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) {
            // "ref: refs/heads/main" -> watch that ref. A detached HEAD
            // holds a bare sha instead, and `.git/HEAD` itself covers it.
            if let Some(r) = head.strip_prefix("ref:").map(str::trim) {
                println!("cargo:rerun-if-changed=.git/{r}");
            }
        }
    }

    // Get git commit hash (if available)
    let git_hash = std::process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    // Get git branch (if available)
    let git_branch = std::process::Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_BRANCH={}", git_branch);

    // Build timestamp from git commit (stable - doesn't change unless you commit)
    let build_time = std::process::Command::new("git")
        .args(&["log", "-1", "--format=%cI"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()); // Fallback to current time if not in git
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);

    // Rustc version
    let rustc_version = rustc_version::version().map(|v| v.to_string()).unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=RUSTC_VERSION={}", rustc_version);

    println!("cargo:warning=Building version {} ({})", version, build_time);

    // Copy assets folder to target directory (for standalone .exe)
    copy_assets_to_target();

    // Copy shaders folder to target directory
    copy_shaders_to_target();

    // Generate the palette pack manifest for WASM discovery
    generate_palette_manifest();
}

/// Generate `palette_manifest.json` in OUT_DIR from the packs folder.
///
/// A browser cannot list a directory, so the WASM build embeds this
/// manifest to discover packs (`src/resources/palettes.rs`). Desktop
/// ignores it entirely and scans `assets/palettes/packs` at startup.
/// Generating it here — instead of committing one — means the folder
/// IS the catalog: a pack added to the repo appears on every platform
/// with no second file to keep in step.
///
/// A pack that fails to parse is skipped with a warning rather than
/// failing the build; `generated_manifest_matches_packs_folder` in
/// `src/resources/palettes.rs` turns that into a visible test failure.
fn generate_palette_manifest() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let packs_dir = Path::new("assets/palettes/packs");

    let mut files: Vec<String> = fs::read_dir(packs_dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|n| n.ends_with(".json") && n != "manifest.json")
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut packs: Vec<serde_json::Value> = Vec::new();
    for filename in &files {
        let path = packs_dir.join(filename);
        let parsed: Option<serde_json::Value> = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok());
        let Some(pack) = parsed else {
            println!("cargo:warning=palette pack {filename} is not valid JSON; left out of the WASM manifest");
            continue;
        };
        let Some(name) = pack.get("pack_name").and_then(|v| v.as_str()) else {
            println!("cargo:warning=palette pack {filename} has no pack_name; left out of the WASM manifest");
            continue;
        };
        packs.push(serde_json::json!({
            "id": filename.trim_end_matches(".json"),
            "name": name,
            "description": pack.get("description").and_then(|v| v.as_str()).unwrap_or(""),
            "file": filename,
            "item_count": pack.get("palettes").and_then(|v| v.as_array()).map_or(0, |a| a.len()),
            // Absent means enabled: a drop-in pack should appear.
            // Mirrors the serde default on PalettePack.
            "enabled_by_default": pack.get("enabled_by_default").and_then(|v| v.as_bool()).unwrap_or(true),
        }));
    }

    let manifest = serde_json::json!({
        "version": 1,
        "resource_type": "palettes",
        "packs": packs,
    });
    let out_path = Path::new(&out_dir).join("palette_manifest.json");
    fs::write(&out_path, serde_json::to_string_pretty(&manifest).unwrap())
        .unwrap_or_else(|e| panic!("could not write {}: {e}", out_path.display()));
    // No rerun-if-changed needed: copy_assets_to_target already watches
    // the whole assets tree.
}

fn copy_assets_to_target() {
    use std::path::PathBuf;

    // Get target directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(out_dir)
        .ancestors()
        .nth(3) // OUT_DIR is usually target/[profile]/build/[crate]/out
        .unwrap()
        .to_path_buf();

    let assets_src = Path::new("assets");
    let assets_dst = target_dir.join("assets");

    // Only copy if assets folder exists
    if assets_src.exists() {
        // Remove old assets if they exist
        if assets_dst.exists() {
            let _ = fs::remove_dir_all(&assets_dst);
        }

        // Copy assets recursively
        if let Err(e) = copy_dir_recursive(assets_src, &assets_dst) {
            println!("cargo:warning=Failed to copy assets: {}", e);
        } else {
            println!("cargo:warning=Copied assets to {:?}", assets_dst);
        }
    }

    // Tell cargo to rerun if assets change
    println!("cargo:rerun-if-changed=assets");
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}

fn copy_shaders_to_target() {
    use std::path::PathBuf;

    // Get target directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(out_dir)
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();

    let shaders_src = Path::new("shaders");
    let shaders_dst = target_dir.join("shaders");

    // Only copy if shaders folder exists
    if shaders_src.exists() {
        // Remove old shaders if they exist
        if shaders_dst.exists() {
            let _ = fs::remove_dir_all(&shaders_dst);
        }

        // Copy shaders recursively
        if let Err(e) = copy_dir_recursive(shaders_src, &shaders_dst) {
            println!("cargo:warning=Failed to copy shaders: {}", e);
        } else {
            println!("cargo:warning=Copied shaders to {:?}", shaders_dst);
        }
    }

    // Tell cargo to rerun if shaders change
    println!("cargo:rerun-if-changed=shaders");

    embed_windows_icon();
}

/// Put the app icon into the executable's resource table.
///
/// This is what Explorer, the taskbar and Alt-Tab read. It is separate
/// from the icon the running window sets for itself (see
/// `desktop_main`): the resource is what the file looks like before it
/// is launched, and there is no way to set it from inside the program.
#[cfg(windows)]
fn embed_windows_icon() {
    let icon = "assets/branding/icon.ico";
    println!("cargo:rerun-if-changed={icon}");
    if !Path::new(icon).exists() {
        // Not fatal: a checkout without the artwork should still build.
        println!("cargo:warning=No {icon}; the executable will have no icon");
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon);
    // Shown on the file's Properties > Details tab.
    res.set("ProductName", "Fractal Art Editor");
    res.set("FileDescription", "Fractal Art Editor");
    res.set("CompanyName", "Fractals for All");
    if let Err(e) = res.compile() {
        // Missing rc.exe on a cross-build, say. Worth saying out loud,
        // not worth failing the build over.
        println!("cargo:warning=Could not embed the icon: {e}");
    }
}

#[cfg(not(windows))]
fn embed_windows_icon() {}
