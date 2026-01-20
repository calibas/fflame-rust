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

    // Get build profile
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_PROFILE={}", profile);

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
}
