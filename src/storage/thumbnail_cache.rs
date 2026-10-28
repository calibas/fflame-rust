//! Disk cache for browser thumbnails.
//!
//! # Why these are JPEG, and smaller than they are rendered
//!
//! Thumbnails were cached as 512x512 RGBA PNG and averaged **474 KB**
//! each — 4.2 MB for nine of them. Measured rather than guessed, because
//! the obvious suspect was wrong: the files carry **no** metadata at all
//! (`IHDR, IDAT, IEND` and nothing else). It was all pixels.
//!
//! Two things were being paid for and neither was used:
//!
//! * **Resolution.** The gallery displays them at 64-256 px, defaulting
//!   to 128. Caching at 512 stored 16x the pixels needed at the default
//!   and 4x at the maximum.
//! * **An alpha channel.** The thumbnail render never sets
//!   `transparent`, so alpha is a constant 255 — a quarter of every raw
//!   byte encoding nothing.
//!
//! So the cache holds JPEG at the largest size the UI can show. PNG is
//! the wrong format here regardless: flame output is noisy and
//! gradient-heavy, which is where lossless compression does worst.
//!
//! The RENDER stays at 512 ([`crate::renderer::THUMBNAIL_SIZE`]) because
//! the API upload wants that size. Only the local cache shrinks.
//!
//! # One index for every gallery
//!
//! The index of what is on disk is shared ([`shared_thumbnail_cache`]).
//! Each gallery used to scan the folder into an index of its own, so a
//! file one gallery evicted stayed listed in all the others: they never
//! queued it, their load found nothing, and the thumbnail stayed blank
//! until a restart rescanned the folder.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::config::FractalConfig;

/// Maximum number of thumbnails to keep in disk cache (~25 KB each)
const MAX_CACHE_SIZE: usize = 1000;

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

    /// Files kept before the least recently used are evicted
    /// (`MAX_CACHE_SIZE`; smaller in tests; desktop only)
    #[allow(dead_code)]
    limit: usize,
}

/// The thumbnail cache every gallery shares. Lock it for one call at a
/// time and drop the guard before anything else might lock it: in
/// edition 2021 an `if let` keeps its scrutinee's guard alive for the
/// whole block.
pub fn shared_thumbnail_cache() -> MutexGuard<'static, ThumbnailCache> {
    static CACHE: OnceLock<Mutex<ThumbnailCache>> = OnceLock::new();
    CACHE
        .get_or_init(|| Mutex::new(ThumbnailCache::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Cache entries are stored at the largest size the gallery can display
/// (`fractal_gallery.rs` caps its slider at 256), not at the size they
/// are rendered. Anything larger is stored and never seen.
const CACHE_PIXELS: u32 = 256;

/// JPEG quality. 88 is above the point where artefacts are visible at
/// thumbnail scale, and well below the size cliff near 95.
const CACHE_QUALITY: u8 = 88;

const CACHE_EXT: &str = "jpg";

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
                limit: MAX_CACHE_SIZE,
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Desktop: Full disk cache
            Self::in_dir(get_cache_dir().join("thumbnails"), MAX_CACHE_SIZE)
        }
    }

    /// A disk cache over `cache_dir`, indexing what is already there
    #[cfg(not(target_arch = "wasm32"))]
    fn in_dir(cache_dir: PathBuf, limit: usize) -> Self {
        std::fs::create_dir_all(&cache_dir).ok();
        let cached_hashes = Self::scan_cache_dir(&cache_dir);
        Self {
            cache_dir,
            cached_hashes,
            disk_enabled: true,
            limit,
        }
    }

    /// Scan cache directory for existing thumbnail files
    #[cfg(not(target_arch = "wasm32"))]
    fn scan_cache_dir(cache_dir: &PathBuf) -> HashSet<String> {
        let mut hashes = HashSet::new();
        let mut stale = Vec::new();

        if let Ok(entries) = std::fs::read_dir(cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(hash) = path.file_stem().and_then(|f| f.to_str()) else {
                    continue;
                };
                // Only a 16-hex-digit stem is one of ours.
                if hash.len() != 16 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                    continue;
                }
                // The extension has to match, not just the stem. Indexing
                // a leftover `.png` from the pre-JPEG cache would make
                // `exists()` say yes while `load()` looked for a `.jpg`
                // and found nothing — a thumbnail permanently blank and
                // never regenerated, because the index claims it is there.
                if path.extension().and_then(|e| e.to_str()) == Some(CACHE_EXT) {
                    hashes.insert(hash.to_string());
                } else {
                    stale.push(path);
                }
            }
        }

        if !stale.is_empty() {
            log::info!(
                "Removing {} thumbnail(s) from the previous cache format",
                stale.len()
            );
            for path in stale {
                let _ = std::fs::remove_file(path);
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
    /// Updates file mtime on access for LRU tracking
    pub fn load(&mut self, hash: &str) -> Option<image::RgbaImage> {
        if !self.disk_enabled {
            return None;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.cache_dir.join(format!("{}.{CACHE_EXT}", hash));
            if let Ok(img) = image::open(&path) {
                // Touch file to update mtime for LRU tracking
                Self::touch_file(&path);
                Some(img.into_rgba8())
            } else {
                // Gone (another process, a hand-cleaned folder) or
                // unreadable: forget it, so `exists` says no and the
                // gallery renders it again instead of waiting forever.
                self.cached_hashes.remove(hash);
                None
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = hash;
            None
        }
    }

    /// Update file modification time (for LRU tracking)
    #[cfg(not(target_arch = "wasm32"))]
    fn touch_file(path: &PathBuf) {
        use std::fs::OpenOptions;
        // Set explicitly: opening with write access does not move the
        // mtime (measured on Windows), so eviction was in creation order
        // and a thumbnail seen every session went first.
        if let Ok(file) = OpenOptions::new().write(true).open(path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
    }

    /// Save thumbnail to disk cache
    /// Enforces the size limit by removing the least recently used files
    /// No-op on WASM
    pub fn save(&mut self, hash: &str, image: &image::RgbaImage) -> anyhow::Result<()> {
        // Always add to memory index
        self.cached_hashes.insert(hash.to_string());

        if !self.disk_enabled {
            return Ok(());
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Enforce cache size limit before adding new file
            self.enforce_cache_limit();

            let path = self.cache_dir.join(format!("{}.{CACHE_EXT}", hash));

            // Downscale to what the UI can actually show, then JPEG.
            // Lanczos3 because a thumbnail is looked at, and the cheaper
            // filters visibly alias on the fine structure flames produce.
            let scaled = image::imageops::resize(
                image,
                CACHE_PIXELS,
                CACHE_PIXELS,
                image::imageops::FilterType::Lanczos3,
            );

            // JPEG has no alpha, which is exactly right — see the module
            // docs. `into_rgb8`-equivalent conversion drops the constant
            // 255s rather than encoding them.
            let rgb = image::DynamicImage::ImageRgba8(scaled).into_rgb8();
            let mut file = std::io::BufWriter::new(std::fs::File::create(&path)?);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, CACHE_QUALITY)
                .encode_image(&rgb)?;
        }

        Ok(())
    }

    /// Remove oldest files if cache exceeds the limit
    #[cfg(not(target_arch = "wasm32"))]
    fn enforce_cache_limit(&mut self) {
        if self.cached_hashes.len() < self.limit {
            return;
        }

        // Get all files with their modification times
        let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                // The cache's own extension: this looked for `.png` after
                // the cache moved to JPEG, found nothing, and never evicted.
                if path.extension().map_or(false, |e| e == CACHE_EXT) {
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(mtime) = metadata.modified() {
                            files.push((path, mtime));
                        }
                    }
                }
            }
        }

        // Sort by mtime (oldest first)
        files.sort_by_key(|(_, mtime)| *mtime);

        // Remove oldest files until we're under the limit
        let to_remove = files.len().saturating_sub(self.limit - 1);
        for (path, _) in files.into_iter().take(to_remove) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                self.cached_hashes.remove(stem);
            }
            std::fs::remove_file(&path).ok();
        }
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
                let path = self.cache_dir.join(format!("{}.{CACHE_EXT}", hash));
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

    ProjectDirs::from("com", "Fractals for All", "Fractal Art Editor")
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

    /// An empty cache folder of the test's own
    #[cfg(not(target_arch = "wasm32"))]
    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fflame-thumb-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn key(i: usize) -> String {
        format!("{i:016x}")
    }

    /// The cache holds at most its limit of thumbnails, evicting the
    /// oldest. Eviction looked for `.png` files after the cache moved to
    /// JPEG, so it never found any and the cache grew without bound.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_cache_evicts_past_its_limit() {
        let dir = test_dir("evicts");
        let limit = 8;
        let mut cache = ThumbnailCache::in_dir(dir.clone(), limit);
        let image = image::RgbaImage::new(8, 8);
        for i in 0..limit + 5 {
            cache.save(&key(i), &image).unwrap();
        }
        let on_disk = std::fs::read_dir(&dir).unwrap().count();
        assert!(on_disk <= limit, "{on_disk} thumbnails on disk, limit {limit}");
        assert!(cache.len() <= limit, "{} indexed, limit {limit}", cache.len());
        assert!(cache.exists(&key(limit + 4)), "the newest stays");
        cache.clear();
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "clear removes the files");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Two indexes over one folder, as every gallery had: the file one
    /// evicts stays listed in the other. Its load must find it gone and
    /// forget it, so the gallery renders it again rather than showing a
    /// blank card for the rest of the run.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_thumbnail_evicted_elsewhere_is_rendered_again() {
        let dir = test_dir("elsewhere");
        let limit = 4;
        let image = image::RgbaImage::new(8, 8);
        let mut writer = ThumbnailCache::in_dir(dir.clone(), limit);
        writer.save(&key(0), &image).unwrap();
        let mut reader = ThumbnailCache::in_dir(dir.clone(), limit);
        assert!(reader.exists(&key(0)));
        for i in 1..=limit {
            writer.save(&key(i), &image).unwrap();
        }
        assert!(!writer.exists(&key(0)), "the writer evicted the oldest");

        assert!(reader.exists(&key(0)), "the reader's index is stale, as a second gallery's was");
        assert!(reader.load(&key(0)).is_none());
        assert!(!reader.exists(&key(0)), "a failed load forgets the file, so it is queued again");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Eviction goes by last use, not by age: a thumbnail loaded since
    /// the others were written outlives them. The load's "touch" opened
    /// the file for writing, which does not move its mtime, so the
    /// eviction order was creation order.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn eviction_spares_a_thumbnail_just_looked_at() {
        let dir = test_dir("lru");
        let limit = 4;
        let image = image::RgbaImage::new(8, 8);
        let mut cache = ThumbnailCache::in_dir(dir.clone(), limit);
        let now = std::time::SystemTime::now();
        for i in 0..limit {
            cache.save(&key(i), &image).unwrap();
            // Distinct ages, oldest first, whatever the clock's resolution
            let path = dir.join(format!("{}.{CACHE_EXT}", key(i)));
            let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            file.set_modified(now - std::time::Duration::from_secs((100 - i) as u64)).unwrap();
        }
        assert!(cache.load(&key(0)).is_some());
        cache.save(&key(limit), &image).unwrap();

        assert!(cache.exists(&key(0)), "the one just loaded stays");
        assert!(!cache.exists(&key(1)), "the least recently used goes");
        assert!(!dir.join(format!("{}.{CACHE_EXT}", key(1))).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

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
