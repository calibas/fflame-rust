//! Thumbnail cache for FractalConfig gallery previews
//!
//! Uses hash-based filenames for content-addressable storage:
//! - Desktop: Persistent disk cache in user data directory
//! - WASM: Memory-only cache (regenerate each session)

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::config::FractalConfig;

/// FNV-1a 64-bit hash - fast, simple, and stable across program runs
/// Unlike DefaultHasher (SipHash), this produces deterministic output
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Thumbnail disk/memory cache
pub struct ThumbnailCache {
    /// Cache directory path (desktop only)
    #[allow(dead_code)]
    cache_dir: PathBuf,

    /// In-memory index of known cached hashes
    cached_hashes: HashSet<String>,

    /// Whether disk operations are enabled
    disk_enabled: bool,
}

impl ThumbnailCache {
    /// Create cache, scanning existing thumbnails on desktop
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            // WASM: Memory-only cache
            Self {
                cache_dir: PathBuf::new(),
                cached_hashes: HashSet::new(),
                disk_enabled: false,
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Desktop: Full disk cache
            let cache_dir = get_cache_dir().join("thumbnails");
            std::fs::create_dir_all(&cache_dir).ok();
            let cached_hashes = Self::scan_cache_dir(&cache_dir);
            Self {
                cache_dir,
                cached_hashes,
                disk_enabled: true,
            }
        }
    }

    /// Scan cache directory for existing thumbnail files
    #[cfg(not(target_arch = "wasm32"))]
    fn scan_cache_dir(cache_dir: &PathBuf) -> HashSet<String> {
        let mut hashes = HashSet::new();
        if let Ok(entries) = std::fs::read_dir(cache_dir) {
            for entry in entries.flatten() {
                if let Some(filename) = entry.path().file_stem() {
                    if let Some(hash) = filename.to_str() {
                        // Only add if it looks like a valid hash (16 hex chars)
                        if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                            hashes.insert(hash.to_string());
                        }
                    }
                }
            }
        }
        hashes
    }

    /// Check if thumbnail exists in cache (disk or memory index)
    pub fn exists(&self, hash: &str) -> bool {
        self.cached_hashes.contains(hash)
    }

    /// Load thumbnail from disk cache
    /// Returns None on WASM or if file doesn't exist
    pub fn load(&self, hash: &str) -> Option<image::RgbaImage> {
        if !self.disk_enabled {
            return None;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.cache_dir.join(format!("{}.png", hash));
            image::open(&path).ok().map(|img| img.into_rgba8())
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = hash;
            None
        }
    }

    /// Save thumbnail to disk cache
    /// No-op on WASM
    pub fn save(&mut self, hash: &str, image: &image::RgbaImage) -> anyhow::Result<()> {
        // Always add to memory index
        self.cached_hashes.insert(hash.to_string());

        if !self.disk_enabled {
            return Ok(());
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.cache_dir.join(format!("{}.png", hash));
            image.save(&path)?;
        }

        Ok(())
    }

    /// Mark a hash as cached (for memory-only tracking after GPU texture upload)
    pub fn mark_cached(&mut self, hash: &str) {
        self.cached_hashes.insert(hash.to_string());
    }

    /// Generate hash from FractalConfig
    /// Uses FNV-1a hash - fast, simple, and stable across program runs
    /// (DefaultHasher/SipHash is randomly seeded per-run for DoS protection)
    pub fn config_hash(config: &FractalConfig) -> String {
        // Serialize to JSON for consistent hashing
        let json = serde_json::to_string(config).unwrap_or_default();

        // FNV-1a 64-bit hash (stable, deterministic)
        let hash = fnv1a_hash(json.as_bytes());
        format!("{:016x}", hash)
    }

    /// Get the number of cached thumbnails
    pub fn len(&self) -> usize {
        self.cached_hashes.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cached_hashes.is_empty()
    }

    /// Clear the cache (removes disk files on desktop)
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if self.disk_enabled {
            for hash in &self.cached_hashes {
                let path = self.cache_dir.join(format!("{}.png", hash));
                std::fs::remove_file(path).ok();
            }
        }
        self.cached_hashes.clear();
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the cache directory path
#[cfg(not(target_arch = "wasm32"))]
fn get_cache_dir() -> PathBuf {
    use directories::ProjectDirs;

    ProjectDirs::from("com", "fractal-flame", "fractal_flame_wgpu")
        .map(|dirs| dirs.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".cache"))
}

/// Gallery item wrapping a config with its precomputed hash
#[derive(Clone)]
pub struct GalleryItem {
    pub config: FractalConfig,
    pub hash: String,
}

impl GalleryItem {
    pub fn new(config: FractalConfig) -> Self {
        let hash = ThumbnailCache::config_hash(&config);
        Self { config, hash }
    }
}

/// In-memory texture cache for uploaded thumbnails
/// Maps hash -> egui TextureHandle
pub struct TextureCache {
    textures: HashMap<String, egui::TextureHandle>,
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    pub fn get(&self, hash: &str) -> Option<&egui::TextureHandle> {
        self.textures.get(hash)
    }

    pub fn insert(&mut self, hash: String, texture: egui::TextureHandle) {
        self.textures.insert(hash, texture);
    }

    pub fn contains(&self, hash: &str) -> bool {
        self.textures.contains_key(hash)
    }
}

impl Default for TextureCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_hash_deterministic() {
        let config = FractalConfig::default();
        let hash1 = ThumbnailCache::config_hash(&config);
        let hash2 = ThumbnailCache::config_hash(&config);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16);
    }

    #[test]
    fn test_config_hash_different_configs() {
        let config1 = FractalConfig::default();
        let mut config2 = FractalConfig::default();
        config2.zoom = 2.0;

        let hash1 = ThumbnailCache::config_hash(&config1);
        let hash2 = ThumbnailCache::config_hash(&config2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_gallery_item() {
        let config = FractalConfig::default();
        let item = GalleryItem::new(config.clone());
        assert_eq!(item.hash, ThumbnailCache::config_hash(&config));
    }
}
