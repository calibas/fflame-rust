//! High-resolution export using CPU-side histogram accumulation
//!
//! This approach avoids GPU buffer size limits by:
//! 1. GPU generates samples → outputs to buffer (x, y, r, g, b)
//! 2. CPU reads samples → accumulates into per-pixel f64 histogram
//! 3. CPU tonemaps → outputs final RGBA pixels
//!
//! This allows exports of any resolution limited only by system RAM.

use egui_wgpu::wgpu::util::DeviceExt;
use egui_wgpu::wgpu::*;

use crate::config::FractalConfig;
use crate::gpu::buffers::{GpuParams, GpuTransform, GpuVariationParams};
use crate::scene::palette::Palette;
use crate::scene::transforms::RenderMode;
use crate::shader_builder_v2::ShaderBuilder;
use crate::variations::global_registry;

/// Sample output from GPU (matches shader struct)
/// Padded to 32 bytes (8 floats) for WGSL array alignment requirements
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Sample {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub _pad1: f32,
    pub _pad2: f32,
    pub _pad3: f32,
}

/// CPU-side histogram pixel (f64 for precision)
#[derive(Clone, Default)]
pub struct HistogramPixel {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub count: f64,
}

/// Progress callback for high-res export
pub trait ExportProgress {
    fn on_dispatch(&mut self, current: u64, total: u64);
    fn on_accumulating(&mut self, samples: u64);
    fn on_tonemapping(&mut self);
    fn on_complete(&mut self);
}

/// Simple CLI progress reporter
pub struct CliExportProgress;

impl ExportProgress for CliExportProgress {
    fn on_dispatch(&mut self, current: u64, total: u64) {
        let percent = (current as f64 / total as f64) * 100.0;
        print!("\r  Iterations: {:.1}% ({}/{})", percent, current, total);
        use std::io::Write;
        std::io::stdout().flush().ok();
    }

    fn on_accumulating(&mut self, samples: u64) {
        println!("\n  Accumulating {} samples...", samples);
    }

    fn on_tonemapping(&mut self) {
        println!("  Tonemapping...");
    }

    fn on_complete(&mut self) {
        println!("  Export complete!");
    }
}

/// High-resolution exporter using CPU histogram
pub struct HighResExporter {
    device: Device,
    queue: Queue,
    width: u32,
    height: u32,

    // GPU resources
    transform_buffer: Buffer,
    params_buffer: Buffer,
    sample_buffer: Buffer,
    sample_counter_buffer: Buffer,
    variation_params_buffer: Buffer,
    palette_texture: Texture,
    palette_sampler: Sampler,

    // Pipeline
    compute_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,

    // Configuration
    render_mode: RenderMode,
    samples_per_dispatch: u64,
}

impl HighResExporter {
    /// Maximum samples per dispatch (sized to fit in reasonable buffer)
    /// 128 workgroups × 64 threads × 256 iterations = ~2M samples
    /// At 20 bytes per sample = 40 MB buffer
    const MAX_SAMPLES_PER_DISPATCH: u64 = 128 * 64 * 256;

    /// Create a new high-resolution exporter
    pub async fn new(
        config: &FractalConfig,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        // Create GPU instance
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .map_err(|e| format!("Failed to find GPU adapter: {}", e))?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default())
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;

        log::info!("High-res export: {}x{}", width, height);

        // Create transform buffer
        let transforms: Vec<GpuTransform> = config
            .flame
            .transforms
            .iter()
            .map(|t| GpuTransform::from_transform(t, global_registry()))
            .collect();

        let transform_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Export Transform Buffer"),
            contents: bytemuck::cast_slice(&transforms),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // Create params buffer
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Params Buffer"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sample output buffer (sized for max samples per dispatch)
        let sample_buffer_size = Self::MAX_SAMPLES_PER_DISPATCH * std::mem::size_of::<Sample>() as u64;
        let sample_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Sample Buffer"),
            size: sample_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create sample counter buffer (atomic u32)
        let sample_counter_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Sample Counter"),
            size: 4, // Single u32
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create and populate variation params buffer
        let variation_params: Vec<GpuVariationParams> = config
            .flame
            .transforms
            .iter()
            .map(|xform| GpuVariationParams::from_transform(xform, global_registry()))
            .collect();

        let max_transforms = 32;
        let variation_params_size = max_transforms * std::mem::size_of::<GpuVariationParams>() as u64;
        let variation_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Variation Params Buffer"),
            size: variation_params_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upload variation params
        queue.write_buffer(&variation_params_buffer, 0, bytemuck::cast_slice(&variation_params));

        // Create palette texture
        let palette = config.palette.clone().unwrap_or_else(Palette::fire);
        let palette_data = palette.generate_texture_data(256);
        let palette_data_u8: Vec<u8> = palette_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        let palette_texture = device.create_texture(&TextureDescriptor {
            label: Some("Export Palette Texture"),
            size: Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_u8,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let palette_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Export Palette Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        // Build active variations map
        let mut active_variations = std::collections::HashMap::new();
        for transform in &config.flame.transforms {
            for name in transform.active_variations() {
                let weight = transform.get_variation(&name);
                if weight != 0.0 {
                    active_variations.insert(name, weight);
                }
            }
        }

        // Build shader
        let shader_builder = ShaderBuilder::new(global_registry().clone());
        let shader_source = match config.flame.render_mode {
            RenderMode::TwoD => shader_builder.build_export_2d(&active_variations),
            RenderMode::ThreeD => shader_builder.build_export_3d(&active_variations),
        };

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Export Compute Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Export Bind Group Layout"),
            entries: &[
                // binding 0: transforms
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: params
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
                // binding 2: samples (output)
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
                // binding 3: palette texture
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: palette sampler
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 5: variation params
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 6: sample counter
                BindGroupLayoutEntry {
                    binding: 6,
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
            label: Some("Export Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Export Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            width,
            height,
            transform_buffer,
            params_buffer,
            sample_buffer,
            sample_counter_buffer,
            variation_params_buffer,
            palette_texture,
            palette_sampler,
            compute_pipeline,
            bind_group_layout,
            render_mode: config.flame.render_mode,
            samples_per_dispatch: Self::MAX_SAMPLES_PER_DISPATCH,
        })
    }

    /// Export to RGBA pixel data
    pub async fn export(
        &mut self,
        config: &FractalConfig,
        total_iterations: u64,
        progress: &mut dyn ExportProgress,
    ) -> Result<Vec<u8>, String> {
        // Create CPU histogram
        let num_pixels = (self.width as usize) * (self.height as usize);
        let mut histogram: Vec<HistogramPixel> = vec![HistogramPixel::default(); num_pixels];

        // Create bind group
        let palette_view = self.palette_texture.create_view(&TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Export Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.transform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: self.params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.sample_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&palette_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::Sampler(&self.palette_sampler),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: self.variation_params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: self.sample_counter_buffer.as_entire_binding(),
                },
            ],
        });

        // Create readback buffer for samples
        let readback_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Sample Readback Buffer"),
            size: self.samples_per_dispatch * std::mem::size_of::<Sample>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Create readback buffer for counter
        let counter_readback_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Counter Readback Buffer"),
            size: 4,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Calculate dispatch parameters
        let workgroups_per_dispatch = 128u32;
        let threads_per_workgroup = 64u64;
        let iterations_per_thread = 256u32;
        let iterations_per_dispatch =
            workgroups_per_dispatch as u64 * threads_per_workgroup * iterations_per_thread as u64;
        let num_dispatches =
            (total_iterations + iterations_per_dispatch - 1) / iterations_per_dispatch;

        let mut total_samples_accumulated = 0u64;

        for dispatch in 0..num_dispatches {
            progress.on_dispatch(dispatch + 1, num_dispatches);

            // Reset sample counter
            self.queue
                .write_buffer(&self.sample_counter_buffer, 0, &[0u8; 4]);

            // Update params
            let seed = dispatch as u32 * 12345;

            let params = GpuParams {
                num_transforms: config.flame.transforms.len() as u32,
                iterations_per_thread,
                burn_in: 20,
                width: self.width,
                height: self.height,
                seed,
                color_mode: config.color_mode as u32,
                render_mode: match self.render_mode {
                    RenderMode::TwoD => 0,
                    RenderMode::ThreeD => 1,
                },
                splat_size: 1.0,
                zoom: config.zoom,
                pan_x: config.pan_x,
                pan_y: config.pan_y,
                rotation: config.rotation,
                speed_factor: config.speed_factor,
                perspective_strength: config.flame.perspective_strength,
                camera_rotation_x: config.camera_rotation_x,
                camera_rotation_y: config.camera_rotation_y,
                camera_z: config.camera_z,
                histogram_color_scale: config.histogram_color_scale,
                has_final_transform: 0,
                final_transform_index: 0,
                _pad3: 0.0,
                _pad4: 0.0,
            };
            self.queue
                .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

            // Dispatch compute
            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Export Compute Encoder"),
                });
            {
                let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Export Compute Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);
                compute_pass.dispatch_workgroups(workgroups_per_dispatch, 1, 1);
            }

            // Copy counter to readback
            encoder.copy_buffer_to_buffer(
                &self.sample_counter_buffer,
                0,
                &counter_readback_buffer,
                0,
                4,
            );

            self.queue.submit(std::iter::once(encoder.finish()));

            // Read sample count
            let sample_count = self.read_counter(&counter_readback_buffer).await?;

            if sample_count > 0 {
                // Copy samples to readback buffer
                let bytes_to_copy =
                    (sample_count as u64 * std::mem::size_of::<Sample>() as u64).min(
                        self.samples_per_dispatch * std::mem::size_of::<Sample>() as u64,
                    );

                let mut encoder = self
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("Sample Readback Encoder"),
                    });
                encoder.copy_buffer_to_buffer(
                    &self.sample_buffer,
                    0,
                    &readback_buffer,
                    0,
                    bytes_to_copy,
                );
                self.queue.submit(std::iter::once(encoder.finish()));

                // Read and accumulate samples
                let samples = self
                    .read_samples(&readback_buffer, sample_count.min(self.samples_per_dispatch as u32))
                    .await?;

                for sample in &samples {
                    let x = sample.x as i32;
                    let y = sample.y as i32;

                    if x >= 0
                        && x < self.width as i32
                        && y >= 0
                        && y < self.height as i32
                    {
                        let idx = (y as usize) * (self.width as usize) + (x as usize);
                        histogram[idx].r += sample.r as f64;
                        histogram[idx].g += sample.g as f64;
                        histogram[idx].b += sample.b as f64;
                        histogram[idx].count += 1.0;
                    }
                }

                total_samples_accumulated += samples.len() as u64;
            }
        }

        progress.on_accumulating(total_samples_accumulated);
        progress.on_tonemapping();

        // Tonemap histogram to RGBA
        let pixels = self.tonemap(&histogram, config, total_samples_accumulated);

        progress.on_complete();

        Ok(pixels)
    }

    /// Read sample counter from GPU
    async fn read_counter(&self, buffer: &Buffer) -> Result<u32, String> {
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        let _ = self.device
            .poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await
            .map_err(|_| "Failed to receive counter map result".to_string())?
            .map_err(|e| format!("Failed to map counter buffer: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        drop(data);
        buffer.unmap();

        Ok(count)
    }

    /// Read samples from GPU buffer
    async fn read_samples(&self, buffer: &Buffer, count: u32) -> Result<Vec<Sample>, String> {
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        let _ = self.device
            .poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await
            .map_err(|_| "Failed to receive samples map result".to_string())?
            .map_err(|e| format!("Failed to map samples buffer: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();
        let sample_size = std::mem::size_of::<Sample>();
        let bytes_to_read = (count as usize) * sample_size;
        let samples: Vec<Sample> = bytemuck::cast_slice(&data[..bytes_to_read]).to_vec();
        drop(data);
        buffer.unmap();

        Ok(samples)
    }

    /// CPU tonemap: histogram → RGBA pixels
    /// Implements Apophysis-compatible tone mapping matching GPU tonemap.wgsl
    fn tonemap(&self, histogram: &[HistogramPixel], config: &FractalConfig, _total_samples: u64) -> Vec<u8> {
        use crate::config::defaults::{DEFAULT_WHITE_LEVEL, PREFILTER_WHITE, BRIGHT_ADJUST};

        let num_pixels = (self.width as usize) * (self.height as usize);
        let mut pixels = vec![0u8; num_pixels * 4];

        // Apophysis constants (from tonemap.wgsl / config/defaults.rs)
        let white_level = DEFAULT_WHITE_LEVEL as f64;
        let prefilter_white = PREFILTER_WHITE as f64;
        let bright_adjust = BRIGHT_ADJUST as f64;

        // Calculate area in FRACTAL SPACE (not pixel space!) - matches GPU export mode
        // GPU: let base_pixels_per_unit = (width.min(height) as f32) * 0.25;
        //      let pixels_per_unit_zoomed = base_pixels_per_unit * 2^(log2(zoom))
        //      let area = (width * height) / (pixels_per_unit_zoomed^2)
        let zoom = config.zoom as f64;
        let base_pixels_per_unit = (self.width.min(self.height) as f64) * 0.25;
        let apophysis_zoom = zoom.log2();
        let pixels_per_unit_zoomed = base_pixels_per_unit * 2.0_f64.powf(apophysis_zoom);
        let area = (self.width as f64 * self.height as f64) / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);

        // Sample density: Use GPU's fixed formula (not based on total iterations)
        // GPU export mode: sample_density = 5000.0 * (iterations_per_thread / 256.0)
        // GPU stores 0.01 per hit, we store 1.0 per hit (100× more)
        // Scale down sample_density to compensate for our higher per-sample density
        let sample_density = 5000.0 / 100.0; // 50.0 - compensate for 100× density difference

        let gamma_threshold = config.gamma_threshold as f64;

        let exposure = config.exposure as f64;
        let gamma = config.gamma as f64;
        // Invert gamma (Apophysis ImageMaker.pas:410)
        let inv_gamma = if gamma == 0.0 { gamma } else { 1.0 / gamma };
        let brightness = config.brightness as f64;
        let vibrancy = config.vibrancy as f64;
        let saturation = config.saturation as f64;
        let hue_shift = config.hue_shift as f64;
        let value_scale = config.value_scale as f64;

        // Background color (already in 0-1 range)
        let bg_r = config.background_color[0] as f64;
        let bg_g = config.background_color[1] as f64;
        let bg_b = config.background_color[2] as f64;

        // Calculate k1 and k2 for brightness_scale function (from tonemap.wgsl)
        let contrast = 1.0;
        let k1 = contrast * bright_adjust * brightness * 268.0 * prefilter_white / 256.0;
        let k2 = 1.0 / (contrast * area * white_level * sample_density);

        // Pre-calculate funcval for gamma threshold (Apophysis setup phase)
        let funcval = if gamma_threshold != 0.0 {
            gamma_threshold.powf(inv_gamma - 1.0)
        } else {
            0.0
        };

        // Vibrancy blend factors (Apophysis ImageMaker.pas:412)
        let vib = (vibrancy * 256.0).round();
        let notvib = 256.0 - vib;

        for (i, pixel) in histogram.iter().enumerate() {
            if pixel.count < 0.001 {
                // Background color (apply sRGB conversion)
                let srgb_r = bg_r.powf(1.0 / 2.2);
                let srgb_g = bg_g.powf(1.0 / 2.2);
                let srgb_b = bg_b.powf(1.0 / 2.2);
                pixels[i * 4] = (srgb_r * 255.0).clamp(0.0, 255.0) as u8;
                pixels[i * 4 + 1] = (srgb_g * 255.0).clamp(0.0, 255.0) as u8;
                pixels[i * 4 + 2] = (srgb_b * 255.0).clamp(0.0, 255.0) as u8;
                pixels[i * 4 + 3] = 255;
                continue;
            }

            // Scale count to match GPU accumulation buffer format
            // GPU stores 0.01 per hit, we store 1.0 per sample
            // GPU reads: bucket_count = accum.a * 100.0
            // So our count is already in the right scale (1 sample = 1 count)
            let bucket_count = pixel.count;

            // Raw accumulated sums (GPU format: bucket = sum, not average)
            let bucket_red = pixel.r;
            let bucket_green = pixel.g;
            let bucket_blue = pixel.b;

            // ===== STAGE 3A: Apply Brightness to Palette Colors =====
            // Calculate brightness scaling factor (ls) from logarithmic curve
            let ls = if bucket_count < 0.001 {
                0.0
            } else {
                // lsa[i] = (k1 * log10(1 + white_level * i * k2)) / (white_level * i)
                let log10_value = (1.0 + white_level * bucket_count * k2).log10();
                (k1 * log10_value) / (white_level * bucket_count)
            };

            // Apply brightness scaling to accumulated color sums
            let ls_scaled = ls / prefilter_white;
            let fp0 = ls_scaled * bucket_red;     // brightness-scaled red
            let fp1 = ls_scaled * bucket_green;   // brightness-scaled green
            let fp2 = ls_scaled * bucket_blue;    // brightness-scaled blue
            let fp3 = ls_scaled * bucket_count * white_level;  // weighted density

            // ===== STAGE 3B: Apply Gamma to Density =====
            let alpha = if fp3 <= 0.0 {
                0.0
            } else if fp3 <= gamma_threshold {
                // Blend between linear and gamma curves at low densities
                let frac = fp3 / gamma_threshold;
                (1.0 - frac) * fp3 * funcval + frac * fp3.powf(inv_gamma)
            } else {
                // Standard gamma curve
                fp3.powf(inv_gamma)
            };

            // ===== STAGE 3C: Calculate Vibrancy-Weighted Multiplier =====
            let ls2 = if fp3 > 0.0 {
                vib * alpha / fp3
            } else {
                0.0
            };

            // ===== STAGE 3D: Vibrancy Blend =====
            // Blend between new (gamma on brightness) and old (gamma on colors) algorithms
            // IMPORTANT: This is ADDITIVE, not weighted average!
            let (mut r, mut g, mut b) = if notvib > 0.0 {
                // NEW algorithm: ls * fp[x] (vibrancy-weighted brightness × brightness-scaled color)
                let new_r = ls2 * fp0;
                let new_g = ls2 * fp1;
                let new_b = ls2 * fp2;

                // OLD algorithm: notvib * power(fp[x], gamma) (gamma applied to colors)
                let old_r = notvib * fp0.powf(inv_gamma);
                let old_g = notvib * fp1.powf(inv_gamma);
                let old_b = notvib * fp2.powf(inv_gamma);

                // Additive blend (NOT weighted average!)
                (new_r + old_r, new_g + old_g, new_b + old_b)
            } else {
                // Pure new algorithm (vibrancy >= 1.0)
                (ls2 * fp0, ls2 * fp1, ls2 * fp2)
            };

            // ===== STAGE 3E: HSV Adjustments =====
            let needs_hsv = saturation != 1.0 || hue_shift != 0.0 || value_scale != 1.0;
            if needs_hsv {
                let (mut h, mut s, mut v) = rgb_to_hsv(r, g, b);

                // Hue shift
                if hue_shift != 0.0 {
                    h += hue_shift;
                    if h < 0.0 {
                        h += 360.0;
                    } else if h >= 360.0 {
                        h -= 360.0;
                    }
                }

                // Saturation boost
                if saturation != 1.0 {
                    s = (s * saturation).clamp(0.0, 1.0);
                }

                // Value scaling
                if value_scale != 1.0 {
                    v = (v * value_scale).clamp(0.0, 1.0);
                }

                let (r_new, g_new, b_new) = hsv_to_rgb(h, s, v);
                r = r_new;
                g = g_new;
                b = b_new;
            }

            // Apply exposure
            r *= exposure;
            g *= exposure;
            b *= exposure;

            // Clamp to valid range
            r = r.clamp(0.0, 1.0);
            g = g.clamp(0.0, 1.0);
            b = b.clamp(0.0, 1.0);

            // ===== STAGE 3F: Background Blending =====
            let fractal_alpha = alpha.clamp(0.0, 1.0);

            // Composite with background (normal mode, not transparent export)
            let final_r = bg_r * (1.0 - fractal_alpha) + r * fractal_alpha;
            let final_g = bg_g * (1.0 - fractal_alpha) + g * fractal_alpha;
            let final_b = bg_b * (1.0 - fractal_alpha) + b * fractal_alpha;

            // Convert from linear to sRGB for display
            let srgb_r = final_r.powf(1.0 / 2.2);
            let srgb_g = final_g.powf(1.0 / 2.2);
            let srgb_b = final_b.powf(1.0 / 2.2);

            pixels[i * 4] = (srgb_r * 255.0).clamp(0.0, 255.0) as u8;
            pixels[i * 4 + 1] = (srgb_g * 255.0).clamp(0.0, 255.0) as u8;
            pixels[i * 4 + 2] = (srgb_b * 255.0).clamp(0.0, 255.0) as u8;
            pixels[i * 4 + 3] = 255;
        }

        pixels
    }
}

// Helper: RGB to HSV
fn rgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max_val = r.max(g).max(b);
    let min_val = r.min(g).min(b);
    let delta = max_val - min_val;

    let v = max_val;

    if delta < 0.00001 || max_val < 0.00001 {
        return (0.0, 0.0, v);
    }

    let s = delta / max_val;

    let h = if r >= max_val {
        (g - b) / delta
    } else if g >= max_val {
        2.0 + (b - r) / delta
    } else {
        4.0 + (r - g) / delta
    } * 60.0;

    let h = if h < 0.0 { h + 360.0 } else { h };

    (h, s, v)
}

// Helper: HSV to RGB
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    if s <= 0.0 {
        return (v, v, v);
    }

    let hh = if h >= 360.0 { 0.0 } else { h } / 60.0;
    let i = hh as u32;
    let ff = hh - i as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * ff);
    let t = v * (1.0 - s * (1.0 - ff));

    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}
