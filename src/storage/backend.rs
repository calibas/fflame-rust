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
/// Named for the product, never for the crate: these paths are things a
/// user browses to, and `fractal_flame_wgpu` means nothing to them.
/// `directories` applies each platform's own convention, so the strings
/// differ by design:
///
/// - Windows: `%APPDATA%\Fractals for All\Fractal Art Editor\data\`
/// - macOS: `~/Library/Application Support/com.Fractals-for-All.Fractal-Art-Editor/`
/// - Linux: `$XDG_DATA_HOME/fractalarteditor/`
///
/// Changing these strings **moves everyone's data**. There is no
/// migration: settings, scripts, plugins and cached downloads under a
/// previous name are orphaned, not lost, and have to be moved by hand.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_app_data_dir() -> StorageResult<PathBuf> {
    use directories::ProjectDirs;

    ProjectDirs::from("com", "Fractals for All", "Fractal Art Editor")
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

/// Names of the entries stored directly under `prefix`.
///
/// Desktop: the file names in that directory. WASM: the localStorage
/// keys beginning `<prefix>/`, with the prefix stripped. Either way the
/// caller gets bare entry names (`julia.json`), not full paths, so both
/// platforms are handled by the same code.
///
/// A missing prefix is not an error — it means nothing has been stored
/// yet, which is the normal first-run state.
///
/// This exists because localStorage has no directory listing, and the
/// WASM half of the variation cache was consequently **write-only**: it
/// stored downloads and could never enumerate them, so every web
/// session re-downloaded everything and "Clear Cache" always reported
/// zero. `Storage` does expose `length()`/`key(i)`, so enumeration was
/// always possible — it had simply been stubbed out with a TODO.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_entries(prefix: &Path) -> StorageResult<Vec<String>> {
    let dir = match get_app_data_dir() {
        Ok(d) => d.join(prefix),
        Err(e) => return Err(e),
    };
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(StorageError::from)?.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            out.push(name.to_string());
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
pub fn list_entries(prefix: &Path) -> StorageResult<Vec<String>> {
    use web_sys::window;

    let window = window().ok_or_else(|| {
        StorageError::NotAvailable("No window object available".to_string())
    })?;
    let storage = window
        .local_storage()
        .map_err(|_| StorageError::NotAvailable("localStorage not available".to_string()))?
        .ok_or_else(|| StorageError::NotAvailable("localStorage is null".to_string()))?;

    let prefix = format!("{}/", prefix.to_str().unwrap_or(""));
    let len = storage.length().map_err(|_| {
        StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "localStorage.length failed",
        ))
    })?;

    let mut out = Vec::new();
    for i in 0..len {
        let Ok(Some(key)) = storage.key(i) else { continue };
        let Some(rest) = key.strip_prefix(&prefix) else { continue };
        // Direct children only, matching the desktop read_dir semantics.
        if !rest.is_empty() && !rest.contains('/') {
            out.push(rest.to_string());
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    /// The data directory resolves to a real absolute path.
    ///
    /// This used to assert the path contained `fractal_flame_wgpu`,
    /// which is precisely what must NOT be there — see
    /// `naming_tests::storage_paths_carry_no_internal_names`, which owns
    /// that question now. Two tests asserting opposite things about one
    /// string is worth avoiding.
    #[test]
    fn test_get_app_data_dir() {
        let dir = get_app_data_dir().unwrap();
        println!("App data dir: {}", dir.display());
        assert!(dir.is_absolute(), "must be absolute: {}", dir.display());
        assert!(dir.components().count() > 2, "suspiciously shallow: {}", dir.display());
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

#[cfg(test)]
mod naming_tests {
    use super::*;

    /// No internal name may appear in a path a user can see.
    ///
    /// The repo is `fflame-rust`, the crate is `fractal_flame_wgpu`, and
    /// an earlier build wrote to `FractalFlame`. None of those mean
    /// anything to somebody browsing their AppData folder, and every one
    /// of them has been in a user-visible path at some point.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn storage_paths_carry_no_internal_names() {
        let dir = match get_app_data_dir() {
            Ok(d) => d.to_string_lossy().to_lowercase(),
            // No home directory in this environment; nothing to check.
            Err(_) => return,
        };
        for internal in ["fractal_flame_wgpu", "fractal-flame", "fflame", "flame_wgpu"] {
            assert!(
                !dir.contains(internal),
                "`{internal}` is visible in the data path: {dir}"
            );
        }
        assert!(
            dir.contains("fractal") && dir.contains("art") && dir.contains("editor"),
            "the product name should be recognisable: {dir}"
        );
    }
}
