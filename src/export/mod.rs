//! High-resolution export.
//!
//! Sub–GPU-buffer-size resolutions render through the interactive
//! FlameRenderer's direct-histogram path. Above the binding-size limit
//! we route through HighResExporter, which uses the unified shader
//! template configured for sample-emit output and accumulates into a
//! CPU-side histogram before tonemapping. The strategy picker that
//! Phases 4–6 add will replace the current threshold check with a
//! runtime device-limits query and per-tile parallel/serial strategies.

mod high_res;

pub use high_res::*;

/// Histogram buffer size in bytes for a full-resolution image: 4 channels
/// (R, G, B, density) × 4 bytes per u32 = 16 bytes per pixel.
pub fn histogram_size_bytes(width: u32, height: u32) -> u64 {
    (width as u64) * (height as u64) * 16
}

/// Check if high-res CPU export is needed (histogram would exceed GPU
/// buffer-binding limits).
pub fn needs_cpu_export(width: u32, height: u32) -> bool {
    // Most GPUs cap max_storage_buffer_binding_size at 128–134 MB; we
    // use the conservative end. Phase 4 swaps this static check for a
    // runtime device-limits query.
    const MAX_GPU_HISTOGRAM_SIZE: u64 = 128 * 1024 * 1024;
    histogram_size_bytes(width, height) > MAX_GPU_HISTOGRAM_SIZE
}
