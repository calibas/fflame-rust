//! System settings - device-specific application preferences
//!
//! These settings apply globally to the application on this device,
//! independent of which fractal is loaded.

use serde::{Deserialize, Serialize};

/// Current system settings format version
pub const CURRENT_SETTINGS_VERSION: u32 = 1;

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
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        // Serialize to JSON value
        let mut value = serde_json::to_value(self)?;

        // Inject version at the top
        if let Some(obj) = value.as_object_mut() {
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

    /// Import settings from JSON string with version checking
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        // Parse to check version first
        let value: serde_json::Value = serde_json::from_str(json)?;

        // Check version if present
        if let Some(version) = value.get("version").and_then(|v| v.as_u64()) {
            let version = version as u32;

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

            // TODO: Add migration logic here when needed
            // if version < CURRENT_SETTINGS_VERSION {
            //     value = migrate(value, version)?;
            // }
        }

        // Deserialize (version field is ignored by struct)
        serde_json::from_value(value)
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
    fn test_serialization_with_version() {
        let settings = SystemSettings::default();
        let json = settings.to_json().unwrap();

        // Check version is present (cross-platform line endings)
        assert!(json.contains("\"version\": 1"));

        // Should contain all fields
        assert!(json.contains("\"vsync_enabled\""));
        assert!(json.contains("\"target_fps\""));
        assert!(json.contains("\"iterations_per_thread\""));
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
    fn test_future_version_rejection() {
        let json = r#"{"version": 999, "vsync_enabled": true}"#;
        let result = SystemSettings::from_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("newer than supported"));
    }

    #[test]
    fn test_missing_version_uses_defaults() {
        // Old format without version field
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
