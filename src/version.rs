/// Version and build information
///
/// This module provides compile-time version information captured by build.rs

use serde::{Deserialize, Serialize};

/// Complete version and build information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Semantic version from Cargo.toml (e.g., "0.1.0")
    pub version: &'static str,

    /// Auto-incrementing build number
    pub build_number: u32,

    /// Git commit hash (short)
    pub git_hash: &'static str,

    /// Git branch name
    pub git_branch: &'static str,

    /// Build target triple (e.g., "x86_64-pc-windows-msvc")
    pub target: &'static str,

    /// Build profile ("debug" or "release")
    pub profile: &'static str,

    /// Build timestamp (RFC3339 format)
    pub build_time: &'static str,

    /// Rust compiler version used
    pub rustc_version: &'static str,
}

impl VersionInfo {
    /// Get the current version information
    pub fn current() -> Self {
        Self {
            version: env!("APP_VERSION"),
            build_number: parse_build_number(env!("BUILD_NUMBER")),
            git_hash: env!("GIT_HASH"),
            git_branch: env!("GIT_BRANCH"),
            target: env!("BUILD_TARGET"),
            profile: env!("BUILD_PROFILE"),
            build_time: env!("BUILD_TIME"),
            rustc_version: env!("RUSTC_VERSION"),
        }
    }

    /// Get full version string (e.g., "0.1.0 (build 42)")
    pub fn full_version(&self) -> String {
        format!("{} (build #{})", self.version, self.build_number)
    }

    /// Get short version string (e.g., "0.1.0")
    pub fn short_version(&self) -> &str {
        self.version
    }

    /// Get build identifier (e.g., "42-abc1234")
    pub fn build_id(&self) -> String {
        format!("{}-{}", self.build_number, self.git_hash)
    }

    /// Get full build info string (multi-line)
    pub fn detailed_info(&self) -> String {
        format!(
            "Version: {} (build #{})\n\
             Git: {} ({})\n\
             Target: {}\n\
             Profile: {}\n\
             Built: {}\n\
             Rustc: {}",
            self.version,
            self.build_number,
            self.git_hash,
            self.git_branch,
            self.target,
            self.profile,
            self.build_time,
            self.rustc_version
        )
    }

    /// Get compact build info (single line)
    pub fn compact_info(&self) -> String {
        format!(
            "v{} build #{} ({}) {}",
            self.version, self.build_number, self.git_hash, self.profile
        )
    }

    /// Check if this is a release build
    pub fn is_release(&self) -> bool {
        self.profile == "release"
    }

    /// Check if this is a debug build
    pub fn is_debug(&self) -> bool {
        self.profile == "debug"
    }

    /// Get platform string (human-readable)
    pub fn platform(&self) -> &str {
        // Extract platform from target triple
        if self.target.contains("windows") {
            "Windows"
        } else if self.target.contains("darwin") || self.target.contains("macos") {
            "macOS"
        } else if self.target.contains("linux") {
            "Linux"
        } else if self.target.contains("wasm32") {
            "WebAssembly"
        } else if self.target.contains("android") {
            "Android"
        } else if self.target.contains("ios") {
            "iOS"
        } else {
            "Unknown"
        }
    }

    /// Get architecture string (human-readable)
    pub fn architecture(&self) -> &str {
        if self.target.contains("x86_64") {
            "x86_64"
        } else if self.target.contains("aarch64") {
            "ARM64"
        } else if self.target.contains("wasm32") {
            "WebAssembly"
        } else {
            "Unknown"
        }
    }
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self::current()
    }
}

impl std::fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.compact_info())
    }
}

/// Parse build number from string (with fallback to 0)
fn parse_build_number(s: &str) -> u32 {
    s.parse().unwrap_or(0)
}

/// Get the global version info singleton
pub fn get_version_info() -> &'static VersionInfo {
    static VERSION: once_cell::sync::Lazy<VersionInfo> = once_cell::sync::Lazy::new(VersionInfo::current);
    &VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        let info = VersionInfo::current();

        // Check that version is non-empty
        assert!(!info.version.is_empty());

        // Check that build number is reasonable
        assert!(info.build_number > 0);

        // Check full version format
        let full = info.full_version();
        assert!(full.contains(info.version));
        assert!(full.contains("build"));

        // Check compact info
        let compact = info.compact_info();
        assert!(compact.contains(info.version));
        assert!(compact.contains(&info.build_number.to_string()));
    }

    #[test]
    fn test_platform_detection() {
        let info = VersionInfo::current();
        let platform = info.platform();

        // Should be one of the known platforms
        assert!(
            platform == "Windows"
                || platform == "macOS"
                || platform == "Linux"
                || platform == "WebAssembly"
                || platform == "Android"
                || platform == "iOS"
                || platform == "Unknown"
        );
    }

    #[test]
    fn test_profile_detection() {
        let info = VersionInfo::current();

        // Should be debug or release
        assert!(info.is_debug() || info.is_release());
        assert!(!(info.is_debug() && info.is_release()));
    }

    #[test]
    fn test_serialization() {
        let info = VersionInfo::current();

        // Test JSON serialization
        let json = serde_json::to_string(&info).expect("Failed to serialize");
        assert!(json.contains(info.version));

        // Test deserialization
        let _deserialized: VersionInfo = serde_json::from_str(&json).expect("Failed to deserialize");
    }
}
