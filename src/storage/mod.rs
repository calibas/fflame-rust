//! Local storage system for persistent application data
//!
//! This module provides cross-platform storage for:
//! - System settings (device-specific preferences)
//! - Custom fractal library
//! - Custom palette library
//! - Custom workspace layouts
//! - Auto-backup system
//! - Thumbnail cache for gallery previews
//!
//! Storage backends:
//! - Desktop (Windows/macOS): Filesystem (via `directories` crate)
//! - WASM (Browser): localStorage for small data, IndexedDB for large data

pub mod backend;
#[cfg(not(target_arch = "wasm32"))]
pub mod credentials;
pub mod custom_palettes;
pub mod settings;
pub mod thumbnail_cache;

pub use backend::{StorageError, StorageResult};
pub use custom_palettes::CustomPaletteLibrary;
pub use settings::SystemSettings;
pub use thumbnail_cache::{ThumbnailCache, GalleryItem, TextureCache};
