//! System settings - device-specific application preferences
//!
//! These settings apply globally to the application on this device,
//! independent of which fractal is loaded.
//!
//! ⚠️ WARNING: Changing defaults affects config serialization!
//! With compact serialization (omitting defaults), old configs rely on these values.
//! If you change a default:
//!   1. Bump CURRENT_SETTINGS_VERSION
//!   2. Add migration to preserve old default for existing configs
//!   3. Document the change in VERSION_HISTORY
//! See docs/projects/config-versioning.md for details.

use serde::{Deserialize, Serialize};
use crate::export::SupersampleLevel;

/// Current system settings format version
pub const CURRENT_SETTINGS_VERSION: u32 = 1;

/// Version history for settings migrations
#[allow(dead_code)]
pub const VERSION_HISTORY: &[&str] = &[
    "1: Initial versioned release with compact serialization",
];

/// System settings - device-specific application preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    // Performance & Rendering
    /// Enable VSync (locks frame rate to monitor refresh rate)
    #[serde(default = "default_vsync_enabled")]
    pub vsync_enabled: bool,

    /// Target FPS when VSync is disabled (only used if vsync_enabled = false)
    #[serde(default = "default_target_fps")]
    pub target_fps: f32,

    /// Iterations per thread (GPU workgroup performance tuning, default: 256)
    #[serde(default = "default_iterations_per_thread")]
    pub iterations_per_thread: u32,

    /// Supersampling level for anti-aliasing (Off, 2x, 4x)
    #[serde(default)]
    pub supersample_level: SupersampleLevel,

    // UI/UX
    /// Application language (ISO 639-1 code, e.g., "en", "es", "fr")
    #[serde(default = "default_language")]
    pub language: String,

    // Export Defaults
    /// Default export width in pixels
    #[serde(default = "default_export_width")]
    pub default_export_width: u32,

    /// Default export height in pixels
    #[serde(default = "default_export_height")]
    pub default_export_height: u32,

    // File Paths (Desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    /// Last opened file path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened_file: Option<std::path::PathBuf>,

    #[cfg(not(target_arch = "wasm32"))]
    /// Recently opened files (MRU list, max 10)
    #[serde(default)]
    pub recent_files: Vec<std::path::PathBuf>,

    #[cfg(not(target_arch = "wasm32"))]
    /// Default save location for new files
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_save_location: Option<std::path::PathBuf>,
}

// Default value functions
fn default_vsync_enabled() -> bool {
    true
}

fn default_target_fps() -> f32 {
    60.0
}

fn default_iterations_per_thread() -> u32 {
    256
}

fn default_language() -> String {
    "en".to_string()
}

fn default_export_width() -> u32 {
    1920
}

fn default_export_height() -> u32 {
    1080
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            vsync_enabled: default_vsync_enabled(),
            target_fps: default_target_fps(),
            iterations_per_thread: default_iterations_per_thread(),
            supersample_level: SupersampleLevel::default(),
            language: default_language(),
            default_export_width: default_export_width(),
            default_export_height: default_export_height(),
            #[cfg(not(target_arch = "wasm32"))]
            last_opened_file: None,
            #[cfg(not(target_arch = "wasm32"))]
            recent_files: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            default_save_location: None,
        }
    }
}

impl SystemSettings {
    /// Export settings to JSON string with version header
    /// Uses compact serialization: omits fields that match defaults
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        // Serialize to JSON value
        let mut value = serde_json::to_value(self)?;

        // Remove fields that match defaults (compact serialization)
        if let Some(obj) = value.as_object_mut() {
            let defaults = Self::default();
            Self::remove_default_fields(obj, self, &defaults);

            // Create ordered map with version first
            let mut ordered_obj = serde_json::Map::new();
            ordered_obj.insert("version".to_string(), serde_json::json!(CURRENT_SETTINGS_VERSION));
            for (k, v) in obj.iter() {
                if k != "version" {
                    ordered_obj.insert(k.clone(), v.clone());
                }
            }
            serde_json::to_string_pretty(&ordered_obj)
        } else {
            serde_json::to_string_pretty(&value)
        }
    }

    /// Remove fields that match default values (for compact serialization)
    fn remove_default_fields(
        obj: &mut serde_json::Map<String, serde_json::Value>,
        current: &Self,
        defaults: &Self,
    ) {
        // Performance & Rendering
        if current.vsync_enabled == defaults.vsync_enabled {
            obj.remove("vsync_enabled");
        }
        if current.target_fps == defaults.target_fps {
            obj.remove("target_fps");
        }
        if current.iterations_per_thread == defaults.iterations_per_thread {
            obj.remove("iterations_per_thread");
        }
        if current.supersample_level == defaults.supersample_level {
            obj.remove("supersample_level");
        }

        // UI/UX
        if current.language == defaults.language {
            obj.remove("language");
        }

        // Export Defaults
        if current.default_export_width == defaults.default_export_width {
            obj.remove("default_export_width");
        }
        if current.default_export_height == defaults.default_export_height {
            obj.remove("default_export_height");
        }

        // File Paths (Desktop only) - already use skip_serializing_if
        // last_opened_file, recent_files, default_save_location
    }

    /// Import settings from JSON string with version checking and migration
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        // Parse to check version first
        let value: serde_json::Value = serde_json::from_str(json)?;

        // Extract version (0 = pre-versioning)
        let version = value.get("version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);

        // Reject future versions
        if version > CURRENT_SETTINGS_VERSION {
            let msg = format!(
                "Settings version {} is newer than supported version {}. Please update the application.",
                version, CURRENT_SETTINGS_VERSION
            );
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                msg
            )));
        }

        // Deserialize with defaults for missing fields
        let mut settings: Self = serde_json::from_value(value)?;

        // Apply migrations if needed
        if version < CURRENT_SETTINGS_VERSION {
            settings = Self::migrate(settings, version)?;
        }

        Ok(settings)
    }

    /// Migrate settings from old version to current version
    fn migrate(mut settings: Self, from_version: u32) -> Result<Self, serde_json::Error> {
        let mut version = from_version;

        // Apply migrations in sequence
        while version < CURRENT_SETTINGS_VERSION {
            settings = match version {
                0 => Self::migrate_v0_to_v1(settings)?,
                // Add new migrations here:
                // 1 => Self::migrate_v1_to_v2(settings)?,
                _ => {
                    return Err(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Unknown settings version: {}", version)
                    )));
                }
            };
            version += 1;
        }

        log::info!("Migrated settings from version {} to {}", from_version, CURRENT_SETTINGS_VERSION);
        Ok(settings)
    }

    /// Migrate from pre-versioning (v0) to v1
    /// v1: Added version field and compact serialization
    fn migrate_v0_to_v1(settings: Self) -> Result<Self, serde_json::Error> {
        // No field changes needed - serde(default) handles missing fields
        // This migration exists for logging and future-proofing
        Ok(settings)
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Add a file to the recent files list (desktop only)
    pub fn add_recent_file(&mut self, path: std::path::PathBuf) {
        // Remove if already in list
        self.recent_files.retain(|p| p != &path);

        // Add to front
        self.recent_files.insert(0, path);

        // Keep max 10
        self.recent_files.truncate(10);
    }

    /// Load settings from persistent storage
    /// Returns default settings if file doesn't exist
    pub fn load() -> Self {
        use std::path::Path;

        let settings_path = Path::new("settings.json");

        match super::backend::read_file(settings_path) {
            Ok(json) => {
                match Self::from_json(&json) {
                    Ok(settings) => {
                        log::info!("Loaded system settings from storage");
                        settings
                    }
                    Err(e) => {
                        log::error!("Failed to parse system settings: {}", e);
                        log::info!("Using default settings");
                        Self::default()
                    }
                }
            }
            Err(super::backend::StorageError::NotFound(_)) => {
                log::info!("No saved settings found, using defaults");
                Self::default()
            }
            Err(e) => {
                log::error!("Failed to load system settings: {}", e);
                log::info!("Using default settings");
                Self::default()
            }
        }
    }

    /// Save settings to persistent storage
    pub fn save(&self) -> Result<(), super::backend::StorageError> {
        use std::path::Path;

        let json = self.to_json()?;
        let settings_path = Path::new("settings.json");

        super::backend::write_file(settings_path, &json)?;
        log::info!("Saved system settings to storage");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = SystemSettings::default();
        assert_eq!(settings.vsync_enabled, true);
        assert_eq!(settings.target_fps, 60.0);
        assert_eq!(settings.iterations_per_thread, 256);
        assert_eq!(settings.language, "en");
        assert_eq!(settings.default_export_width, 1920);
        assert_eq!(settings.default_export_height, 1080);
    }

    #[test]
    fn test_compact_serialization_defaults_omitted() {
        // Default settings should produce minimal JSON (only version)
        let settings = SystemSettings::default();
        let json = settings.to_json().unwrap();

        // Version should be present
        assert!(json.contains("\"version\": 1"));

        // Default fields should be omitted (compact serialization)
        assert!(!json.contains("\"vsync_enabled\""), "default vsync_enabled should be omitted");
        assert!(!json.contains("\"target_fps\""), "default target_fps should be omitted");
        assert!(!json.contains("\"iterations_per_thread\""), "default iterations_per_thread should be omitted");
        assert!(!json.contains("\"language\""), "default language should be omitted");
        assert!(!json.contains("\"default_export_width\""), "default export_width should be omitted");
        assert!(!json.contains("\"default_export_height\""), "default export_height should be omitted");
    }

    #[test]
    fn test_compact_serialization_non_defaults_included() {
        // Non-default settings should include changed fields
        let mut settings = SystemSettings::default();
        settings.vsync_enabled = false;
        settings.target_fps = 120.0;
        settings.language = "es".to_string();

        let json = settings.to_json().unwrap();

        // Version always present
        assert!(json.contains("\"version\": 1"));

        // Changed fields should be present
        assert!(json.contains("\"vsync_enabled\": false"));
        assert!(json.contains("\"target_fps\": 120"));
        assert!(json.contains("\"language\": \"es\""));

        // Unchanged defaults should still be omitted
        assert!(!json.contains("\"iterations_per_thread\""));
        assert!(!json.contains("\"default_export_width\""));
        assert!(!json.contains("\"default_export_height\""));
    }

    #[test]
    fn test_deserialization_with_version() {
        let json = r#"{
            "version": 1,
            "vsync_enabled": false,
            "target_fps": 120.0,
            "iterations_per_thread": 512,
            "language": "es",
            "default_export_width": 3840,
            "default_export_height": 2160
        }"#;

        let settings = SystemSettings::from_json(json).unwrap();
        assert_eq!(settings.vsync_enabled, false);
        assert_eq!(settings.target_fps, 120.0);
        assert_eq!(settings.iterations_per_thread, 512);
        assert_eq!(settings.language, "es");
        assert_eq!(settings.default_export_width, 3840);
        assert_eq!(settings.default_export_height, 2160);
    }

    #[test]
    fn test_deserialization_minimal_json() {
        // Minimal JSON with only version - all fields should use defaults
        let json = r#"{"version": 1}"#;

        let settings = SystemSettings::from_json(json).unwrap();
        assert_eq!(settings.vsync_enabled, true);
        assert_eq!(settings.target_fps, 60.0);
        assert_eq!(settings.iterations_per_thread, 256);
        assert_eq!(settings.language, "en");
        assert_eq!(settings.default_export_width, 1920);
        assert_eq!(settings.default_export_height, 1080);
    }

    #[test]
    fn test_future_version_rejection() {
        let json = r#"{"version": 999, "vsync_enabled": true}"#;
        let result = SystemSettings::from_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("newer than supported"));
    }

    #[test]
    fn test_pre_versioning_migration() {
        // Old format without version field (v0)
        let json = r#"{
            "vsync_enabled": false,
            "target_fps": 90.0
        }"#;

        let settings = SystemSettings::from_json(json).unwrap();
        assert_eq!(settings.vsync_enabled, false);
        assert_eq!(settings.target_fps, 90.0);
        // Other fields should use defaults
        assert_eq!(settings.iterations_per_thread, 256);
        assert_eq!(settings.language, "en");
    }

    #[test]
    fn test_roundtrip_serialization() {
        // Create settings with some non-default values
        let mut original = SystemSettings::default();
        original.vsync_enabled = false;
        original.target_fps = 144.0;
        original.iterations_per_thread = 512;
        original.language = "fr".to_string();
        original.default_export_width = 2560;
        original.default_export_height = 1440;

        // Serialize and deserialize
        let json = original.to_json().unwrap();
        let restored = SystemSettings::from_json(&json).unwrap();

        // Verify all values match
        assert_eq!(restored.vsync_enabled, original.vsync_enabled);
        assert_eq!(restored.target_fps, original.target_fps);
        assert_eq!(restored.iterations_per_thread, original.iterations_per_thread);
        assert_eq!(restored.language, original.language);
        assert_eq!(restored.default_export_width, original.default_export_width);
        assert_eq!(restored.default_export_height, original.default_export_height);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_recent_files_management() {
        let mut settings = SystemSettings::default();

        // Add files
        settings.add_recent_file("file1.fflame".into());
        settings.add_recent_file("file2.fflame".into());
        settings.add_recent_file("file3.fflame".into());

        assert_eq!(settings.recent_files.len(), 3);
        assert_eq!(settings.recent_files[0].to_str().unwrap(), "file3.fflame");
        assert_eq!(settings.recent_files[1].to_str().unwrap(), "file2.fflame");
        assert_eq!(settings.recent_files[2].to_str().unwrap(), "file1.fflame");

        // Re-adding existing file moves it to front
        settings.add_recent_file("file1.fflame".into());
        assert_eq!(settings.recent_files.len(), 3);
        assert_eq!(settings.recent_files[0].to_str().unwrap(), "file1.fflame");

        // Add more than 10 files
        for i in 4..=15 {
            settings.add_recent_file(format!("file{}.fflame", i).into());
        }
        assert_eq!(settings.recent_files.len(), 10); // Truncated to 10
    }
}
