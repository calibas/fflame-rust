//! Optimized tiled rendering for high-resolution export
//!
//! Single-pass approach: generate samples once, route to tile buffers by coordinates.
//! This avoids N× iteration overhead for N tiles.

use serde::{Deserialize, Serialize};

/// Maximum pixels per tile (GPU memory constraint)
/// 2000×2000 × 4 channels × 4 bytes = 64 MB per tile
pub const MAX_TILE_PIXELS: u64 = 4_000_000;

/// Maximum total histogram buffer size (256 MB, safe for most GPUs)
pub const MAX_HISTOGRAM_BUFFER_SIZE: u64 = 256 * 1024 * 1024;

/// Supersampling level for anti-aliasing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SupersampleLevel {
    #[default]
    Off,
    X2,
    X4,
}

impl SupersampleLevel {
    pub fn multiplier(&self) -> u32 {
        match self {
            SupersampleLevel::Off => 1,
            SupersampleLevel::X2 => 2,
            SupersampleLevel::X4 => 4,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SupersampleLevel::Off => "Off",
            SupersampleLevel::X2 => "2× (4 samples)",
            SupersampleLevel::X4 => "4× (16 samples)",
        }
    }

    pub fn all() -> &'static [SupersampleLevel] {
        &[SupersampleLevel::Off, SupersampleLevel::X2, SupersampleLevel::X4]
    }
}

/// Configuration for high-resolution export
#[derive(Debug, Clone)]
pub struct HighResExportConfig {
    /// Final output width (after downsampling if supersampling)
    pub output_width: u32,
    /// Final output height (after downsampling if supersampling)
    pub output_height: u32,
    /// Supersampling level
    pub supersample: SupersampleLevel,
    /// Iterations per GPU thread
    pub iterations_per_thread: u32,
    /// Export with transparent background
    pub transparent: bool,
}

impl HighResExportConfig {
    /// Calculate the actual render resolution (before downsampling)
    pub fn render_size(&self) -> (u32, u32) {
        let mult = self.supersample.multiplier();
        (self.output_width * mult, self.output_height * mult)
    }
}

/// Calculate tile grid dimensions for a given render size
///
/// Returns (tiles_x, tiles_y, tile_size)
pub fn calculate_tile_grid(render_width: u32, render_height: u32) -> (u32, u32, u32) {
    // Calculate tile size based on MAX_TILE_PIXELS
    let tile_size = (MAX_TILE_PIXELS as f64).sqrt() as u32;

    // How many tiles needed in each dimension?
    let tiles_x = (render_width + tile_size - 1) / tile_size;
    let tiles_y = (render_height + tile_size - 1) / tile_size;

    (tiles_x, tiles_y, tile_size)
}

/// Check if tiling is needed for the given render size
pub fn needs_tiling(render_width: u32, render_height: u32) -> bool {
    let total_pixels = render_width as u64 * render_height as u64;
    total_pixels > MAX_TILE_PIXELS
}

/// Calculate how many tiles can fit in a single histogram buffer
pub fn max_tiles_per_buffer(tile_size: u32) -> u32 {
    let bytes_per_tile = tile_size as u64 * tile_size as u64 * 4 * 4; // 4 channels × 4 bytes
    (MAX_HISTOGRAM_BUFFER_SIZE / bytes_per_tile) as u32
}

/// Tile parameters for GPU uniform buffer
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileParams {
    /// Full output width (all tiles combined)
    pub full_width: u32,
    /// Full output height (all tiles combined)
    pub full_height: u32,
    /// Size of each tile (square)
    pub tile_size: u32,
    /// Number of tiles in X direction
    pub tiles_x: u32,
    /// Number of tiles in Y direction
    pub tiles_y: u32,
    /// Total number of tiles being rendered in this pass
    pub num_tiles: u32,
    /// Offset into tile array for chunked processing
    pub tile_offset: u32,
    /// Padding for 16-byte alignment
    pub _padding: u32,
}

impl Default for TileParams {
    fn default() -> Self {
        Self {
            full_width: 0,
            full_height: 0,
            tile_size: 1000,
            tiles_x: 1,
            tiles_y: 1,
            num_tiles: 1,
            tile_offset: 0,
            _padding: 0,
        }
    }
}

/// Downsample an image using box filter (simple averaging)
///
/// Used for supersampling anti-aliasing.
pub fn downsample(
    src: &[u8],
    src_width: u32,
    _src_height: u32,
    dst_width: u32,
    dst_height: u32,
) -> Vec<u8> {
    let scale = src_width / dst_width; // Assumes integer scale (2 or 4)
    let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];

    for y in 0..dst_height {
        for x in 0..dst_width {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut a_sum = 0u32;

            // Average NxN source pixels
            for sy in 0..scale {
                for sx in 0..scale {
                    let src_x = x * scale + sx;
                    let src_y = y * scale + sy;
                    let src_idx = ((src_y * src_width + src_x) * 4) as usize;

                    r_sum += src[src_idx] as u32;
                    g_sum += src[src_idx + 1] as u32;
                    b_sum += src[src_idx + 2] as u32;
                    a_sum += src[src_idx + 3] as u32;
                }
            }

            let count = (scale * scale) as u32;
            let dst_idx = ((y * dst_width + x) * 4) as usize;
            dst[dst_idx] = (r_sum / count) as u8;
            dst[dst_idx + 1] = (g_sum / count) as u8;
            dst[dst_idx + 2] = (b_sum / count) as u8;
            dst[dst_idx + 3] = (a_sum / count) as u8;
        }
    }

    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_grid_small() {
        // Small image - no tiling needed
        let (tiles_x, tiles_y, _) = calculate_tile_grid(800, 600);
        assert_eq!(tiles_x, 1);
        assert_eq!(tiles_y, 1);
    }

    #[test]
    fn test_tile_grid_large() {
        // 8000×8000 should need multiple tiles (tile_size ~2000 for 4M pixel limit)
        let (tiles_x, tiles_y, tile_size) = calculate_tile_grid(8000, 8000);
        assert!(tiles_x >= 4);  // 8000/2000 = 4 tiles
        assert!(tiles_y >= 4);
        assert!(tile_size <= 2000);
    }

    #[test]
    fn test_needs_tiling() {
        // MAX_TILE_PIXELS = 4_000_000 (4M pixels)
        assert!(!needs_tiling(800, 600));      // 480K pixels < 4M limit
        assert!(!needs_tiling(1920, 1080));    // 2M pixels < 4M limit
        assert!(!needs_tiling(2000, 2000));    // 4M pixels = 4M limit (edge case, not >)
        assert!(needs_tiling(2001, 2000));     // Just over 4M pixels
        assert!(needs_tiling(4000, 4000));     // 16M pixels > 4M limit
        assert!(needs_tiling(8000, 8000));     // 64M pixels > 4M limit
    }

    #[test]
    fn test_supersample_multiplier() {
        assert_eq!(SupersampleLevel::Off.multiplier(), 1);
        assert_eq!(SupersampleLevel::X2.multiplier(), 2);
        assert_eq!(SupersampleLevel::X4.multiplier(), 4);
    }

    #[test]
    fn test_downsample_2x() {
        // 4×4 image downsampled to 2×2
        let src = vec![
            // Row 0
            255, 0, 0, 255,  0, 255, 0, 255,  255, 0, 0, 255,  0, 255, 0, 255,
            // Row 1
            0, 0, 255, 255,  255, 255, 0, 255,  0, 0, 255, 255,  255, 255, 0, 255,
            // Row 2
            255, 0, 0, 255,  0, 255, 0, 255,  255, 0, 0, 255,  0, 255, 0, 255,
            // Row 3
            0, 0, 255, 255,  255, 255, 0, 255,  0, 0, 255, 255,  255, 255, 0, 255,
        ];

        let dst = downsample(&src, 4, 4, 2, 2);

        // Each 2×2 block averaged
        assert_eq!(dst.len(), 16); // 2×2×4
    }
}
