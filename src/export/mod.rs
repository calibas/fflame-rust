//! High-resolution export with optimized tiled rendering
//!
//! This module provides two export strategies:
//!
//! 1. **TiledRenderer** (GPU histogram): Fast, single-pass rendering with GPU-side
//!    histogram accumulation. Limited by GPU buffer size (~256MB on most hardware).
//!    Good for exports up to ~4000x4000.
//!
//! 2. **HighResExporter** (CPU histogram): GPU generates samples, CPU accumulates
//!    histogram. No buffer size limits - works for any resolution limited only by
//!    system RAM. Used automatically when GPU histogram would exceed device limits.

mod tiled;
mod renderer;
mod high_res;

pub use tiled::*;
pub use renderer::*;
pub use high_res::*;

/// Calculate required histogram buffer size for a given resolution
pub fn calculate_histogram_size(width: u32, height: u32) -> u64 {
    let (tiles_x, tiles_y, tile_size) = calculate_tile_grid(width, height);
    let total_tiles = tiles_x * tiles_y;
    // 4 channels × 4 bytes per channel = 16 bytes per pixel
    (tile_size as u64) * (tile_size as u64) * 16 * (total_tiles as u64)
}

/// Check if high-res CPU export is needed (histogram would exceed GPU buffer limits)
pub fn needs_cpu_export(width: u32, height: u32) -> bool {
    // Most GPUs have a max_storage_buffer_binding_size around 128-134 MB
    // Use conservative 128 MB limit to ensure compatibility
    const MAX_GPU_HISTOGRAM_SIZE: u64 = 128 * 1024 * 1024; // 128 MB

    // Simple calculation: width × height × 16 bytes (4 channels × 4 bytes per f32)
    let histogram_size = (width as u64) * (height as u64) * 16;
    histogram_size > MAX_GPU_HISTOGRAM_SIZE
}
