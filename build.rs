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

    // Build number (auto-incrementing)
    let build_number_file = "build_number.txt";
    println!("cargo:rerun-if-changed={}", build_number_file);

    let build_number = if Path::new(build_number_file).exists() {
        // Read existing build number
        let content = fs::read_to_string(build_number_file)
            .unwrap_or_else(|_| "0".to_string());
        let num: u32 = content.trim().parse().unwrap_or(0);

        // Increment and write back
        let new_num = num + 1;
        fs::write(build_number_file, new_num.to_string())
            .expect("Failed to write build number");

        new_num
    } else {
        // Create new build number file
        fs::write(build_number_file, "1")
            .expect("Failed to create build number file");
        1
    };

    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);

    // Build timestamp
    let build_time = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);

    // Rustc version
    let rustc_version = rustc_version::version().map(|v| v.to_string()).unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=RUSTC_VERSION={}", rustc_version);

    println!("cargo:warning=Building version {} (build #{})", version, build_number);
}
