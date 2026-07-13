//! Solid-rendering density statistics (brightness renormalization).
//!
//! Hard-solid occlusion culls most dispatched samples, but the tonemap
//! normalizes brightness by DISPATCHED iterations — so solids render dark
//! by exactly the culled fraction. This module measures the real accepted
//! density (sum of the accumulator's alpha) with a one-workgroup GPU
//! reduction (`shaders/density_stats.wgsl`) and hands the host a
//! `survival_fraction` to scale the tonemap's `sample_density` with.
//!
//! Interactive path: the readback is ASYNC with a couple frames of lag,
//! smoothed by an EMA (a brightness scalar doesn't need frame-exactness).
//! The CLI export path uses `measure_blocking` for an exact value before
//! its single final tonemap.

use std::sync::{Arc, Mutex};

use egui_wgpu::wgpu::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct StatsParams {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
}

pub struct DensityStats {
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    params_buffer: Buffer,
    sum_buffer: Buffer,
    readback: Buffer,
    /// Completed async measurement (sum of accumulator alpha), parked by
    /// the map callback until the renderer consumes it.
    completed: Arc<Mutex<Option<f32>>>,
    map_in_flight: bool,
    /// Copy encoded this frame — request the map only after the caller
    /// has submitted the encoder.
    map_pending_submit: bool,
    frames_since_dispatch: u32,
}

impl DensityStats {
    /// Frames between measurements on the interactive path.
    const INTERVAL: u32 = 16;

    pub fn new(device: &Device) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Density Stats Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/density_stats.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Density Stats Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Density Stats Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Density Stats Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("stats_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Density Stats Params"),
            size: std::mem::size_of::<StatsParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sum_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Density Stats Sum"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&BufferDescriptor {
            label: Some("Density Stats Readback"),
            size: 4,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            sum_buffer,
            readback,
            completed: Arc::new(Mutex::new(None)),
            map_in_flight: false,
            map_pending_submit: false,
            frames_since_dispatch: 0,
        }
    }

    fn encode(&self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder, accum_view: &TextureView, width: u32, height: u32) {
        let params = StatsParams { width, height, _pad0: 0, _pad1: 0 };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Density Stats Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: BindingResource::TextureView(accum_view) },
                BindGroupEntry { binding: 1, resource: self.params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.sum_buffer.as_entire_binding() },
            ],
        });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Density Stats Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.sum_buffer, 0, &self.readback, 0, 4);
    }

    /// Interactive-path tick: consume a completed measurement (returns the
    /// alpha sum if one arrived), pump the async map state machine, and
    /// (every INTERVAL frames) encode a fresh measurement into `encoder`.
    pub fn tick(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        accum_view: &TextureView,
        width: u32,
        height: u32,
    ) -> Option<f32> {
        // Pump map callbacks without blocking.
        let _ = device.poll(PollType::Poll);

        let result = self.completed.lock().ok().and_then(|mut g| g.take());
        if result.is_some() {
            self.readback.unmap();
            self.map_in_flight = false;
        }

        // The copy encoded last frame has been submitted by now — map it.
        if self.map_pending_submit && !self.map_in_flight {
            self.map_pending_submit = false;
            self.map_in_flight = true;
            let completed = Arc::clone(&self.completed);
            let slice = self.readback.slice(..);
            // Read inside the callback: the buffer contents are valid the
            // moment the map succeeds, and parking the value (not the
            // guard) keeps borrowck simple across frames.
            let buffer = self.readback.clone();
            slice.map_async(MapMode::Read, move |res| {
                if res.is_ok() {
                    let data = buffer.slice(..).get_mapped_range();
                    let sum = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                    drop(data);
                    if let Ok(mut g) = completed.lock() {
                        *g = Some(sum);
                    }
                }
            });
        }

        self.frames_since_dispatch += 1;
        if self.frames_since_dispatch >= Self::INTERVAL && !self.map_in_flight && !self.map_pending_submit {
            self.frames_since_dispatch = 0;
            self.encode(device, queue, encoder, accum_view, width, height);
            self.map_pending_submit = true;
        }

        result
    }

    /// Exact, blocking measurement (CLI export: one value before the final
    /// tonemap). Submits its own encoder and waits.
    pub fn measure_blocking(
        &mut self,
        device: &Device,
        queue: &Queue,
        accum_view: &TextureView,
        width: u32,
        height: u32,
    ) -> Option<f32> {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Density Stats (blocking)"),
        });
        self.encode(device, queue, &mut encoder, accum_view, width, height);
        queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        let slice = self.readback.slice(..);
        slice.map_async(MapMode::Read, move |res| {
            let _ = tx.send(res.is_ok());
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let ok = rx.recv().unwrap_or(false);
        if !ok {
            return None;
        }
        let sum = {
            let data = self.readback.slice(..).get_mapped_range();
            f32::from_le_bytes([data[0], data[1], data[2], data[3]])
        };
        self.readback.unmap();
        Some(sum)
    }
}

/// Attractor-bounds readback (volume auto-fit).
///
/// The main pass's VOLUME block maintains a running world-space AABB of
/// plotted samples (subsampled ordered-float atomicMax, 8-word tail after
/// the histogram's depth region). This tracker copies those 32 bytes back
/// (async on the interactive path, blocking for exports) so
/// `FlameRenderer::volume_placement` can fit the cube to the FLAME
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

    /// Interactive tick (same submit-then-map cadence as DensityStats).
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
