//! High-resolution export.
//!
//! Sub–GPU-buffer-size resolutions render through the interactive
//! FlameRenderer's direct-histogram path. Above the binding-size limit
//! we route through HighResExporter, which uses the unified shader
//! template configured for sample-emit output and accumulates into a
//! CPU-side histogram before tonemapping. The strategy picker below
//! is the runtime decision point — Phases 5–6 wire the dispatch loops
//! for the parallel-tiles and serial-tiles strategies, replacing the
//! CPU accumulate path.

mod high_res;
pub mod accumulate;

pub use high_res::*;

use egui_wgpu::wgpu::Limits;

/// Histogram buffer size in bytes for a full-resolution image: 4 channels
/// (R, G, B, density) × 4 bytes per u32 = 16 bytes per pixel.
pub fn histogram_size_bytes(width: u32, height: u32) -> u64 {
    (width as u64) * (height as u64) * 16
}

/// Render strategy chosen at runtime based on resolution + device limits.
///
/// The driver of the choice is whether the full-resolution histogram fits
/// in a single storage-buffer binding. When it does, the existing direct-
/// histogram path runs unchanged. When it doesn't, we tile — and the
/// picker further chooses between binding all tiles simultaneously
/// (parallel scatter) or sequencing one tile at a time (serial re-iterate).
///
/// See `docs/projects/unified-render-pipeline.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStrategy {
    /// Single full-resolution histogram fits in one binding. The
    /// interactive renderer's direct-histogram path; no tiling, no
    /// accumulate pass. Sub-4K and most 4K cases hit this.
    Direct,

    /// Multi-tile, all tiles GPU-resident. Iterate fills a sample
    /// stream; a single accumulate dispatch scatters every sample
    /// into the right tile's slice of one concatenated tile-histogram
    /// buffer. 1× iteration cost. Memory: tile-histograms +
    /// sample-stream simultaneously.
    ParallelTiles {
        tiles_x: u32,
        tiles_y: u32,
        tile_width: u32,
        tile_height: u32,
    },
    /// Multi-tile, serial. For each tile in turn: re-seed the RNG,
    /// iterate, accumulate (filtering to this tile only), readback,
    /// reset, next tile. N× iteration cost. Memory: only one
    /// tile-sized histogram at a time — fits any GPU.
    SerialTiles {
        tiles_x: u32,
        tiles_y: u32,
        tile_width: u32,
        tile_height: u32,
    },
}

/// Pick a render strategy based on output resolution and device limits.
///
/// `Direct` when the full-resolution histogram fits one storage-buffer
/// binding. Otherwise tiled, with the choice between `ParallelTiles` and
/// `SerialTiles` driven by whether all tiles fit in the device's
/// per-stage storage-buffer count budget after reserving a slot for the
/// sample stream.
///
/// Tiles are sized to fit one binding's worth of histogram, with the
/// long axis carved into rows: `tile_width = width`,
/// `tile_height = floor(max_binding_size / (width × 16))`. This keeps
/// the row-major pixel layout intact, so tile boundaries are
/// horizontal slices and stitching is a contiguous memcpy.
pub fn pick_strategy(width: u32, height: u32, limits: &Limits) -> RenderStrategy {
    let total_bytes = histogram_size_bytes(width, height);
    let max_binding = limits.max_storage_buffer_binding_size as u64;
    let max_buffer = limits.max_buffer_size as u64;

    if total_bytes <= max_binding {
        return RenderStrategy::Direct;
    }

    // Tile by rows so the layout matches the flat row-major histogram
    // the rest of the pipeline already speaks. Tile height is chosen
    // so each tile's region fits one storage-buffer binding.
    let bytes_per_row = (width as u64) * 16;
    let tile_height = (max_binding / bytes_per_row).max(1) as u32;
    let tile_width = width;
    let tiles_y = (height + tile_height - 1) / tile_height;
    let tiles_x = 1;

    // ParallelTiles vs SerialTiles: ParallelTiles puts the entire
    // concatenated histogram in *one* GPU buffer and binds tile
    // regions via offset+size for the accumulate dispatch (1× iter
    // cost — iteration runs once, accumulate dispatch loop runs N
    // times reading the same sample stream). That only works when
    // the total size fits one buffer (`max_buffer_size`, typically
    // 2 GB on desktop).
    //
    // SerialTiles would mean re-iterating the chaos game per tile
    // — N× iter cost, only worth it if even tile-sized GPU
    // histograms aren't workable. Per
    // docs/projects/parallel-tiles.md we don't actually build it:
    // when total exceeds one buffer, HighResExporter's CPU path
    // (1× iter cost, main-RAM histogram) beats SerialTiles on
    // wall time. The SerialTiles enum variant survives as a
    // "fall through to CPU" signal.
    if total_bytes <= max_buffer {
        RenderStrategy::ParallelTiles {
            tiles_x,
            tiles_y,
            tile_width,
            tile_height,
        }
    } else {
        RenderStrategy::SerialTiles {
            tiles_x,
            tiles_y,
            tile_width,
            tile_height,
        }
    }
}

#[cfg(test)]
mod strategy_tests {
    use super::*;

    /// Most desktop GPUs report at least these limits.
    fn typical_limits() -> Limits {
        Limits {
            max_storage_buffer_binding_size: 128 * 1024 * 1024,
            max_storage_buffers_per_shader_stage: 8,
            ..Limits::default()
        }
    }

    #[test]
    fn small_resolutions_pick_direct() {
        let l = typical_limits();
        // 1080p, 1440p, 4K(UHD), 4096² all need verification
        assert!(matches!(pick_strategy(1920, 1080, &l), RenderStrategy::Direct));
        assert!(matches!(pick_strategy(2560, 1440, &l), RenderStrategy::Direct));
        // 3840×2160 = 127 MB, just under 128 MB
        assert!(matches!(pick_strategy(3840, 2160, &l), RenderStrategy::Direct));
    }

    #[test]
    fn just_over_threshold_picks_tiles() {
        let l = typical_limits();
        // 4096² = 256 MB, doesn't fit one 128 MB binding → tiled.
        let s = pick_strategy(4096, 4096, &l);
        assert!(!matches!(s, RenderStrategy::Direct));
    }

    #[test]
    fn parallel_tiles_when_buffer_fits() {
        // Bump max_buffer_size high enough that 8K UHD's 506 MB
        // total histogram fits in one buffer.
        let l = Limits {
            max_buffer_size: 1024 * 1024 * 1024, // 1 GB
            ..typical_limits()
        };
        let s = pick_strategy(7680, 4320, &l);
        match s {
            RenderStrategy::ParallelTiles {
                tiles_x,
                tiles_y,
                tile_width,
                tile_height,
            } => {
                assert_eq!(tiles_x, 1);
                assert_eq!(tile_width, 7680);
                assert!(tiles_y >= 2);
                assert!(tile_height < 4320);
            }
            other => panic!(
                "expected ParallelTiles for 8K UHD when total fits one buffer, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn extreme_resolutions_fall_back_to_serial() {
        // 8K UHD's 506 MB total exceeds typical_limits' default
        // max_buffer_size (256 MB Limits::default floor) so it can't
        // be one buffer → SerialTiles, which the routing treats as
        // CPU-fallback per parallel-tiles.md.
        let l = typical_limits();
        let s = pick_strategy(7680, 4320, &l);
        assert!(matches!(s, RenderStrategy::SerialTiles { .. }));
    }

    #[test]
    fn tile_layout_is_row_major() {
        let l = typical_limits();
        let s = pick_strategy(8000, 8000, &l);
        match s {
            RenderStrategy::ParallelTiles {
                tiles_x,
                tile_width,
                ..
            }
            | RenderStrategy::SerialTiles {
                tiles_x,
                tile_width,
                ..
            } => {
                assert_eq!(tiles_x, 1, "tiles must be horizontal slices");
                assert_eq!(tile_width, 8000, "tile_width must equal full width");
            }
            RenderStrategy::Direct => panic!("8000² should not be Direct"),
        }
    }

    #[test]
    fn tile_height_max_fits_binding() {
        // For a given resolution, the chosen tile_height must keep
        // each tile under max_storage_buffer_binding_size.
        let l = typical_limits();
        for (w, h) in [(4096, 4096), (8000, 8000), (16000, 16000)] {
            let s = pick_strategy(w, h, &l);
            if let RenderStrategy::ParallelTiles {
                tile_width,
                tile_height,
                ..
            }
            | RenderStrategy::SerialTiles {
                tile_width,
                tile_height,
                ..
            } = s
            {
                let bytes = (tile_width as u64) * (tile_height as u64) * 16;
                assert!(
                    bytes <= l.max_storage_buffer_binding_size as u64,
                    "{}×{}: tile {}×{} = {} bytes > binding limit",
                    w,
                    h,
                    tile_width,
                    tile_height,
                    bytes
                );
            }
        }
    }
}
