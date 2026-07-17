//! Attractor-bounds readback (shadow-map auto-fit) and the
//! occlusion-survival counters that ride the same histogram tail.
//!
//! `BoundsTracker` reads the 8-word tail after the solid depth region:
//! words 0-5 are the subsampled world-AABB atomics (`decode_bounds`),
//! words 6-7 the occlusion-survival counter pair the brightness renorm
//! divides (see main_template.wgsl). Interactive path ticks an async
//! readback every N frames; exports use `read_blocking` for an exact
//! value.
//!
//! (The former `DensityStats` Σalpha reduction lived here until the
//! renorm switched to occlusion-only counters — see
//! docs/projects/solid-rendering.md.)

use std::sync::{Arc, Mutex};

use egui_wgpu::wgpu::*;

/// Attractor-bounds readback (shadow-map auto-fit).
///
/// The main pass's VOLUME block maintains a running world-space AABB of
/// plotted samples (subsampled ordered-float atomicMax, 8-word tail after
/// the histogram's depth region). This tracker copies those 32 bytes back
/// (async on the interactive path, blocking for exports) so
/// `FlameRenderer::shadow_placement` can fit the maps to the FLAME
/// instead of guessing from zoom.
pub struct BoundsTracker {
    readback: Buffer,
    completed: Arc<Mutex<Option<[u32; 8]>>>,
    map_in_flight: bool,
    map_pending_submit: bool,
    frames_since_dispatch: u32,
}

fn bounds_dec(enc: u32) -> f32 {
    // Inverse of the shader's bounds_enc ordered-float mapping.
    let bits = if enc & 0x8000_0000 != 0 { enc & 0x7FFF_FFFF } else { !enc };
    f32::from_bits(bits)
}

/// Decode the 6 used tail words into (min, max) world bounds. 0 is the
/// "no data yet" sentinel — returns None until every axis has samples.
pub fn decode_bounds(words: &[u32; 8]) -> Option<([f32; 3], [f32; 3])> {
    for w in words.iter().take(6) {
        if *w == 0 {
            return None;
        }
    }
    let mx = [bounds_dec(words[0]), bounds_dec(words[2]), bounds_dec(words[4])];
    let mn = [-bounds_dec(words[1]), -bounds_dec(words[3]), -bounds_dec(words[5])];
    if mn.iter().zip(&mx).any(|(a, b)| !a.is_finite() || !b.is_finite() || a > b) {
        return None;
    }
    Some((mn, mx))
}

impl BoundsTracker {
    const INTERVAL: u32 = 16;

    pub fn new(device: &Device) -> Self {
        let readback = device.create_buffer(&BufferDescriptor {
            label: Some("Bounds Readback"),
            size: 32,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            readback,
            completed: Arc::new(Mutex::new(None)),
            map_in_flight: false,
            map_pending_submit: false,
            frames_since_dispatch: 0,
        }
    }

    /// Interactive tick (submit-then-map with a couple frames of lag).
    /// `tail_offset` = byte offset of the bounds tail in `histogram`.
    pub fn tick(
        &mut self,
        device: &Device,
        encoder: &mut CommandEncoder,
        histogram: &Buffer,
        tail_offset: u64,
    ) -> Option<[u32; 8]> {
        let _ = device.poll(PollType::Poll);
        let result = self.completed.lock().ok().and_then(|mut g| g.take());
        if result.is_some() {
            self.readback.unmap();
            self.map_in_flight = false;
        }
        if self.map_pending_submit && !self.map_in_flight {
            self.map_pending_submit = false;
            self.map_in_flight = true;
            let completed = Arc::clone(&self.completed);
            let buffer = self.readback.clone();
            self.readback.slice(..).map_async(MapMode::Read, move |res| {
                if res.is_ok() {
                    let data = buffer.slice(..).get_mapped_range();
                    let mut words = [0u32; 8];
                    for (i, w) in words.iter_mut().enumerate() {
                        *w = u32::from_le_bytes([data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]]);
                    }
                    drop(data);
                    if let Ok(mut g) = completed.lock() {
                        *g = Some(words);
                    }
                }
            });
        }
        self.frames_since_dispatch += 1;
        if self.frames_since_dispatch >= Self::INTERVAL && !self.map_in_flight && !self.map_pending_submit {
            self.frames_since_dispatch = 0;
            encoder.copy_buffer_to_buffer(histogram, tail_offset, &self.readback, 0, 32);
            self.map_pending_submit = true;
        }
        result
    }

    /// Blocking read (export warmup: measure once, re-place, re-render).
    pub fn read_blocking(
        &mut self,
        device: &Device,
        queue: &Queue,
        histogram: &Buffer,
        tail_offset: u64,
    ) -> Option<[u32; 8]> {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Bounds Read (blocking)"),
        });
        encoder.copy_buffer_to_buffer(histogram, tail_offset, &self.readback, 0, 32);
        queue.submit(std::iter::once(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        self.readback.slice(..).map_async(MapMode::Read, move |res| {
            let _ = tx.send(res.is_ok());
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        if !rx.recv().unwrap_or(false) {
            return None;
        }
        let words = {
            let data = self.readback.slice(..).get_mapped_range();
            let mut w = [0u32; 8];
            for (i, out) in w.iter_mut().enumerate() {
                *out = u32::from_le_bytes([data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]]);
            }
            w
        };
        self.readback.unmap();
        Some(words)
    }
}
