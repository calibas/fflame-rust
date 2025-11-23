//! Storage backend - cross-platform persistent storage
//!
//! Provides unified API for:
//! - Desktop: Filesystem (via `directories` crate)
//! - WASM: localStorage (small data) / IndexedDB (large data)

use std::path::{Path, PathBuf};

/// Storage backend errors
#[derive(Debug)]
pub enum StorageError {
    /// IO error (filesystem, network, etc.)
    Io(std::io::Error),
    /// JSON serialization/deserialization error
    Json(serde_json::Error),
    /// Storage not available (browser privacy mode, permission denied, etc.)
    NotAvailable(String),
    /// Item not found
    NotFound(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "IO error: {}", e),
            StorageError::Json(e) => write!(f, "JSON error: {}", e),
            StorageError::NotAvailable(msg) => write!(f, "Storage not available: {}", msg),
            StorageError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        StorageError::Json(e)
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Get the application data directory (cross-platform)
///
/// Returns:
/// - Windows: `%APPDATA%\fractal_flame_wgpu\`
/// - macOS: `~/Library/Application Support/fractal_flame_wgpu/`
/// - Linux: `~/.config/fractal_flame_wgpu/` or `$XDG_CONFIG_HOME/fractal_flame_wgpu/`
#[cfg(not(target_arch = "wasm32"))]
pub fn get_app_data_dir() -> StorageResult<PathBuf> {
    use directories::ProjectDirs;

    ProjectDirs::from("com", "fractal-flame", "fractal_flame_wgpu")
        .map(|proj_dirs| proj_dirs.data_dir().to_path_buf())
        .ok_or_else(|| StorageError::NotAvailable("Could not determine application data directory".to_string()))
}

/// Read a text file from application data directory
#[cfg(not(target_arch = "wasm32"))]
pub fn read_file(relative_path: &Path) -> StorageResult<String> {
    let app_dir = get_app_data_dir()?;
    let full_path = app_dir.join(relative_path);

    if !full_path.exists() {
        return Err(StorageError::NotFound(format!(
            "File not found: {}",
            full_path.display()
        )));
    }

    std::fs::read_to_string(&full_path).map_err(StorageError::from)
}

/// Write a text file to application data directory
/// Creates parent directories if they don't exist
#[cfg(not(target_arch = "wasm32"))]
pub fn write_file(relative_path: &Path, content: &str) -> StorageResult<()> {
    let app_dir = get_app_data_dir()?;
    let full_path = app_dir.join(relative_path);

    // Create parent directories
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&full_path, content).map_err(StorageError::from)
}

/// Delete a file from application data directory
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_file(relative_path: &Path) -> StorageResult<()> {
    let app_dir = get_app_data_dir()?;
    let full_path = app_dir.join(relative_path);

    if !full_path.exists() {
        return Err(StorageError::NotFound(format!(
            "File not found: {}",
            full_path.display()
        )));
    }

    std::fs::remove_file(&full_path).map_err(StorageError::from)
}

/// Check if a file exists in application data directory
#[cfg(not(target_arch = "wasm32"))]
pub fn file_exists(relative_path: &Path) -> bool {
    if let Ok(app_dir) = get_app_data_dir() {
        app_dir.join(relative_path).exists()
    } else {
        false
    }
}

// ===== WASM IMPLEMENTATION (localStorage) =====

#[cfg(target_arch = "wasm32")]
pub fn read_file(key: &Path) -> StorageResult<String> {
    use wasm_bindgen::JsValue;
    use web_sys::window;

    let window = window().ok_or_else(|| {
        StorageError::NotAvailable("No window object available".to_string())
    })?;

    let storage = window
        .local_storage()
        .map_err(|_| StorageError::NotAvailable("localStorage not available".to_string()))?
        .ok_or_else(|| StorageError::NotAvailable("localStorage is null".to_string()))?;

    let key_str = key.to_str().unwrap_or("");

    storage
        .get_item(key_str)
        .map_err(|_| StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "localStorage.getItem failed"
        )))?
        .ok_or_else(|| StorageError::NotFound(format!("Key not found: {}", key_str)))
}

#[cfg(target_arch = "wasm32")]
pub fn write_file(key: &Path, content: &str) -> StorageResult<()> {
    use web_sys::window;

    let window = window().ok_or_else(|| {
        StorageError::NotAvailable("No window object available".to_string())
    })?;

    let storage = window
        .local_storage()
        .map_err(|_| StorageError::NotAvailable("localStorage not available".to_string()))?
        .ok_or_else(|| StorageError::NotAvailable("localStorage is null".to_string()))?;

    let key_str = key.to_str().unwrap_or("");

    storage
        .set_item(key_str, content)
        .map_err(|_| StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "localStorage.setItem failed (quota exceeded?)"
        )))
}

#[cfg(target_arch = "wasm32")]
pub fn delete_file(key: &Path) -> StorageResult<()> {
    use web_sys::window;

    let window = window().ok_or_else(|| {
        StorageError::NotAvailable("No window object available".to_string())
    })?;

    let storage = window
        .local_storage()
        .map_err(|_| StorageError::NotAvailable("localStorage not available".to_string()))?
        .ok_or_else(|| StorageError::NotAvailable("localStorage is null".to_string()))?;

    let key_str = key.to_str().unwrap_or("");

    storage
        .remove_item(key_str)
        .map_err(|_| StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "localStorage.removeItem failed"
        )))
}

#[cfg(target_arch = "wasm32")]
pub fn file_exists(key: &Path) -> bool {
    use web_sys::window;

    if let Some(window) = window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let key_str = key.to_str().unwrap_or("");
            return storage.get_item(key_str).ok().flatten().is_some();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_get_app_data_dir() {
        let dir = get_app_data_dir().unwrap();
        println!("App data dir: {}", dir.display());

        // Should contain app name
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains("fractal_flame_wgpu"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_file_operations() {
        let test_path = Path::new("test_storage.txt");
        let test_content = "Hello, storage!";

        // Write
        write_file(test_path, test_content).unwrap();

        // Check exists
        assert!(file_exists(test_path));

        // Read
        let read_content = read_file(test_path).unwrap();
        assert_eq!(read_content, test_content);

        // Delete
        delete_file(test_path).unwrap();

        // Check doesn't exist
        assert!(!file_exists(test_path));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_read_nonexistent_file() {
        let result = read_file(Path::new("nonexistent.txt"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StorageError::NotFound(_)));
    }
}
