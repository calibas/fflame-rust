//! Density histogram computation for tone mapping visualization
//!
//! Reads the accumulation buffer and computes a histogram of density values,
//! providing statistics useful for Levels-style tone mapping controls.

use std::sync::Arc;
use egui_wgpu::wgpu::*;

/// Number of bins in the histogram
pub const HISTOGRAM_BINS: usize = 256;

/// Computed density histogram with statistics
#[derive(Clone, Debug)]
pub struct DensityHistogram {
    /// Histogram bins (log-scaled density values)
    pub bins: [u32; HISTOGRAM_BINS],
    /// Total number of pixels sampled
    pub total_pixels: u32,
    /// Number of pixels with zero density (empty)
    pub empty_pixels: u32,
    /// Minimum non-zero density value
    pub min_density: f32,
    /// Maximum density value
    pub max_density: f32,
    /// Density at 1st percentile (for auto black point)
    pub percentile_1: f32,
    /// Density at 50th percentile (median)
    pub percentile_50: f32,
    /// Density at 99th percentile (for auto white point)
    pub percentile_99: f32,
    /// Whether the histogram is valid (has been computed)
    pub valid: bool,
}

impl Default for DensityHistogram {
    fn default() -> Self {
        Self {
            bins: [0; HISTOGRAM_BINS],
            total_pixels: 0,
            empty_pixels: 0,
            min_density: 0.0,
            max_density: 0.0,
            percentile_1: 0.0,
            percentile_50: 0.0,
            percentile_99: 0.0,
            valid: false,
        }
    }
}

impl DensityHistogram {
    /// Create a new empty histogram
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute histogram from raw accumulation buffer data (f16 RGBA format)
    ///
    /// The accumulation buffer stores density in the alpha channel as f16.
    /// Density is stored as: actual_density * 0.01 per hit (see accumulate.wgsl)
    pub fn compute_from_f16_data(&mut self, data: &[u8], width: u32, height: u32, padded_bytes_per_row: u32) {
        // Reset
        self.bins = [0; HISTOGRAM_BINS];
        self.total_pixels = width * height;
        self.empty_pixels = 0;
        self.min_density = f32::MAX;
        self.max_density = 0.0;

        let bytes_per_pixel = 8; // Rgba16Float = 4 channels × 2 bytes

        // First pass: find min/max density
        let mut densities: Vec<f32> = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_data = &data[row_start..row_start + (width * bytes_per_pixel) as usize];

            for chunk in row_data.chunks_exact(8) {
                // Density is in alpha channel (bytes 6-7)
                let density_f16 = half::f16::from_le_bytes([chunk[6], chunk[7]]);
                let density = density_f16.to_f32();

                // Scale back from 0.01 per hit to actual hit count
                let hit_count = density * 100.0;

                if hit_count < 0.001 {
                    self.empty_pixels += 1;
                } else {
                    self.min_density = self.min_density.min(hit_count);
                    self.max_density = self.max_density.max(hit_count);
                }
                densities.push(hit_count);
            }
        }

        // Handle edge case: all pixels empty
        if self.max_density <= 0.0 {
            self.min_density = 0.0;
            self.max_density = 1.0;
            self.valid = true;
            return;
        }

        // Use logarithmic binning for better visualization of wide dynamic range
        // Map log(density) to bins 0..255
        let log_min = self.min_density.max(0.001).ln();
        let log_max = self.max_density.ln();
        let log_range = log_max - log_min;

        // Second pass: bin the densities
        for &density in &densities {
            if density > 0.001 {
                let log_density = density.ln();
                let normalized = if log_range > 0.0 {
                    (log_density - log_min) / log_range
                } else {
                    0.5
                };
                let bin = ((normalized * (HISTOGRAM_BINS - 1) as f32) as usize).min(HISTOGRAM_BINS - 1);
                self.bins[bin] += 1;
            }
        }

        // Calculate percentiles from sorted densities
        densities.retain(|&d| d > 0.001); // Remove empty pixels
        if !densities.is_empty() {
            densities.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let p1_idx = (densities.len() as f32 * 0.01) as usize;
            let p50_idx = densities.len() / 2;
            let p99_idx = ((densities.len() as f32 * 0.99) as usize).min(densities.len() - 1);

            self.percentile_1 = densities[p1_idx];
            self.percentile_50 = densities[p50_idx];
            self.percentile_99 = densities[p99_idx];
        }

        self.valid = true;
    }

    /// Get the maximum bin count (for scaling the histogram display)
    pub fn max_bin_count(&self) -> u32 {
        *self.bins.iter().max().unwrap_or(&0)
    }

    /// Convert a bin index to density value (inverse of binning)
    pub fn bin_to_density(&self, bin: usize) -> f32 {
        if !self.valid || self.max_density <= 0.0 {
            return 0.0;
        }

        let log_min = self.min_density.max(0.001).ln();
        let log_max = self.max_density.ln();
        let log_range = log_max - log_min;

        let normalized = bin as f32 / (HISTOGRAM_BINS - 1) as f32;
        let log_density = log_min + normalized * log_range;
        log_density.exp()
    }

    /// Convert a density value to bin index
    pub fn density_to_bin(&self, density: f32) -> usize {
        if !self.valid || self.max_density <= 0.0 || density <= 0.001 {
            return 0;
        }

        let log_min = self.min_density.max(0.001).ln();
        let log_max = self.max_density.ln();
        let log_range = log_max - log_min;

        let log_density = density.ln();
        let normalized = if log_range > 0.0 {
            (log_density - log_min) / log_range
        } else {
            0.5
        };
        ((normalized * (HISTOGRAM_BINS - 1) as f32) as usize).min(HISTOGRAM_BINS - 1)
    }
}

/// Pending histogram readback — owns the staging buffer after GPU copy is submitted.
/// Call `compute()` to async-map the buffer and produce the histogram.
///
/// This is `'static` so it can be moved into `spawn_local` on WASM.
pub struct HistogramReadback {
    #[cfg(not(target_arch = "wasm32"))]
    device: Arc<Device>,
    buffer: Buffer,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
}

/// Submit a GPU texture-to-buffer copy for histogram computation.
///
/// This is synchronous and borrows the texture only briefly. The returned
/// `HistogramReadback` owns the staging buffer and can be sent into an
/// async task (e.g. `spawn_local` on WASM) to finish the work.
pub fn submit_histogram_readback(
    device: &Arc<Device>,
    queue: &Queue,
    accumulation_texture: &Texture,
    width: u32,
    height: u32,
) -> HistogramReadback {
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Histogram Compute Encoder"),
    });

    // Create buffer to copy accumulation texture data (Rgba16Float format)
    let bytes_per_pixel = 8; // Rgba16Float = 4 channels × 2 bytes each
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let align = COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
    let buffer_size = (padded_bytes_per_row * height) as u64;

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Histogram Capture Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Copy accumulation texture to buffer
    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: accumulation_texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    HistogramReadback {
        #[cfg(not(target_arch = "wasm32"))]
        device: device.clone(),
        buffer,
        width,
        height,
        padded_bytes_per_row,
    }
}

impl HistogramReadback {
    /// Async map the staging buffer, read back the data, and compute the histogram.
    ///
    /// On desktop this is typically called via `pollster::block_on()`.
    /// On WASM this runs inside `spawn_local`.
    pub async fn compute(self) -> Result<DensityHistogram, String> {
        let buffer_slice = self.buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // On desktop, poll the device to drive the GPU work to completion.
        // On WASM, the browser's WebGPU implementation handles this automatically
        // via the event loop — explicit polling would panic.
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await
            .map_err(|_| "Failed to receive buffer map result".to_string())?
            .map_err(|e| format!("Buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Compute histogram
        let mut histogram = DensityHistogram::new();
        histogram.compute_from_f16_data(&data, self.width, self.height, self.padded_bytes_per_row);

        drop(data);
        self.buffer.unmap();

        Ok(histogram)
    }
}

/// Async function to compute histogram from FlameRenderer's accumulation buffer.
///
/// Convenience wrapper that combines `submit_histogram_readback` + `compute`.
/// Primarily used on desktop where both steps can run in one `pollster::block_on`.
pub async fn compute_histogram_async(
    device: &Arc<Device>,
    queue: &Queue,
    accumulation_texture: &Texture,
    width: u32,
    height: u32,
) -> Result<DensityHistogram, String> {
    let readback = submit_histogram_readback(device, queue, accumulation_texture, width, height);
    readback.compute().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_binning() {
        let mut histogram = DensityHistogram::new();
        histogram.min_density = 1.0;
        histogram.max_density = 1000.0;
        histogram.valid = true;

        // Test round-trip conversion
        let test_densities = [1.0, 10.0, 100.0, 1000.0];
        for &density in &test_densities {
            let bin = histogram.density_to_bin(density);
            let recovered = histogram.bin_to_density(bin);
            // Should be close (within one bin)
            assert!((recovered / density - 1.0).abs() < 0.1,
                "density {} -> bin {} -> {} (error {})",
                density, bin, recovered, (recovered / density - 1.0).abs());
        }
    }
}
