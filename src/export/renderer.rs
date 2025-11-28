//! Tiled renderer for high-resolution export
//!
//! Creates a single-pass tiled rendering pipeline where iterations run once
//! and samples are routed to tile buffers based on screen coordinates.

use egui_wgpu::wgpu::*;
use egui_wgpu::wgpu::util::DeviceExt;
use crate::config::FractalConfig;
use crate::gpu::buffers::{GpuParams, GpuTransform};
use crate::scene::palette::Palette;
use crate::scene::transforms::RenderMode;
use crate::shader_builder_v2::ShaderBuilder;
use crate::variations::global_registry;
use super::{TileParams, calculate_tile_grid, max_tiles_per_buffer};

/// Progress callback for tiled export
pub trait TiledExportProgress {
    fn on_chunk_start(&mut self, chunk: u32, total_chunks: u32);
    fn on_iterations(&mut self, current: u64, total: u64);
    fn on_tile_complete(&mut self, tile: u32, total_tiles: u32);
    fn on_complete(&mut self);
}

/// Simple CLI progress callback
pub struct CliProgress;

impl TiledExportProgress for CliProgress {
    fn on_chunk_start(&mut self, chunk: u32, total_chunks: u32) {
        println!("  Chunk {}/{}", chunk + 1, total_chunks);
    }

    fn on_iterations(&mut self, current: u64, total: u64) {
        let percent = (current as f64 / total as f64) * 100.0;
        print!("\r  Iterations: {:.1}%", percent);
    }

    fn on_tile_complete(&mut self, tile: u32, total_tiles: u32) {
        println!("\n  Tile {}/{} complete", tile + 1, total_tiles);
    }

    fn on_complete(&mut self) {
        println!("\n  Export complete!");
    }
}

/// Tiled renderer for high-resolution export
pub struct TiledRenderer {
    // GPU resources
    device: Device,
    queue: Queue,

    // Dimensions
    full_width: u32,
    full_height: u32,
    tile_size: u32,
    tiles_x: u32,
    tiles_y: u32,

    // Buffers
    transform_buffer: Buffer,
    params_buffer: Buffer,
    tile_params_buffer: Buffer,
    histogram_buffer: Buffer,  // Large buffer for multiple tiles
    iteration_counts_buffer: Buffer,
    variation_params_buffer: Buffer,
    palette_texture: Texture,
    palette_sampler: Sampler,

    // Pipelines
    compute_pipeline: ComputePipeline,
    compute_bind_group_layout: BindGroupLayout,

    // State
    render_mode: RenderMode,
}

impl TiledRenderer {
    /// Create a new tiled renderer for export
    pub async fn new(
        config: &FractalConfig,
        full_width: u32,
        full_height: u32,
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
            .request_device(
                &DeviceDescriptor {
                    label: Some("Tiled Export Device"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    memory_hints: MemoryHints::default(),
                    trace: Default::default(),
                    experimental_features: Default::default(),
                },
            )
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;

        // Calculate tile grid
        let (tiles_x, tiles_y, tile_size) = calculate_tile_grid(full_width, full_height);
        let total_tiles = tiles_x * tiles_y;
        let max_tiles = max_tiles_per_buffer(tile_size);

        if total_tiles > max_tiles {
            return Err(format!(
                "Too many tiles ({}) for single buffer (max {}). Chunked processing not yet implemented.",
                total_tiles, max_tiles
            ));
        }

        // Create transform buffer
        let transforms: Vec<GpuTransform> = config.flame.transforms
            .iter()
            .map(|t| GpuTransform::from_transform(t, global_registry()))
            .collect();

        let transform_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Transform Buffer"),
            contents: bytemuck::cast_slice(&transforms),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // Create params buffer
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Params Buffer"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create tile params buffer
        let tile_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Tile Params Buffer"),
            size: std::mem::size_of::<TileParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create histogram buffer for all tiles
        // Each tile: tile_size × tile_size × 4 channels × 4 bytes
        let histogram_size = (tile_size as u64) * (tile_size as u64) * 4 * 4 * (total_tiles as u64);
        let histogram_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Tiled Histogram Buffer"),
            size: histogram_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create iteration counts buffer
        let counts_size = (tile_size as u64) * (tile_size as u64) * 4 * (total_tiles as u64);
        let iteration_counts_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Iteration Counts Buffer"),
            size: counts_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create variation params buffer (same as normal renderer)
        let max_transforms = 32;
        let variation_params_size = max_transforms * 1200 * 4; // 1200 floats per transform
        let variation_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Variation Params Buffer"),
            size: variation_params_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create palette texture
        let palette = config.palette.clone().unwrap_or_else(Palette::fire);
        let palette_data = palette.generate_texture_data(256);
        // Convert f32 to u8
        let palette_data_u8: Vec<u8> = palette_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        let palette_texture = device.create_texture(&TextureDescriptor {
            label: Some("Palette Texture"),
            size: Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
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
            Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
        );

        let palette_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Palette Sampler"),
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
            RenderMode::TwoD => shader_builder.build_trajectory_2d_tiled(&active_variations),
            RenderMode::ThreeD => shader_builder.build_trajectory_3d_tiled(&active_variations),
        };

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Tiled Compute Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout for tiled compute
        let compute_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Tiled Compute Bind Group Layout"),
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
                // binding 2: histogram
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
                // binding 6: iteration counts
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
                // binding 7: tile params
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let compute_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Tiled Compute Pipeline Layout"),
            bind_group_layouts: &[&compute_bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Tiled Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            full_width,
            full_height,
            tile_size,
            tiles_x,
            tiles_y,
            transform_buffer,
            params_buffer,
            tile_params_buffer,
            histogram_buffer,
            iteration_counts_buffer,
            variation_params_buffer,
            palette_texture,
            palette_sampler,
            compute_pipeline,
            compute_bind_group_layout,
            render_mode: config.flame.render_mode,
        })
    }

    /// Run the tiled export - compute pass only (accumulate/tonemap TODO)
    pub async fn run_iterations(
        &mut self,
        config: &FractalConfig,
        total_iterations: u64,
        iterations_per_dispatch: u32,
        progress: &mut dyn TiledExportProgress,
    ) -> Result<(), String> {
        // Update tile params
        let tile_params = TileParams {
            full_width: self.full_width,
            full_height: self.full_height,
            tile_size: self.tile_size,
            tiles_x: self.tiles_x,
            tiles_y: self.tiles_y,
            num_tiles: self.tiles_x * self.tiles_y,
            tile_offset: 0,
            _padding: 0,
        };
        self.queue.write_buffer(&self.tile_params_buffer, 0, bytemuck::bytes_of(&tile_params));

        // Create bind group
        let palette_view = self.palette_texture.create_view(&TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Tiled Compute Bind Group"),
            layout: &self.compute_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: self.transform_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.histogram_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&palette_view) },
                BindGroupEntry { binding: 4, resource: BindingResource::Sampler(&self.palette_sampler) },
                BindGroupEntry { binding: 5, resource: self.variation_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 6, resource: self.iteration_counts_buffer.as_entire_binding() },
                BindGroupEntry { binding: 7, resource: self.tile_params_buffer.as_entire_binding() },
            ],
        });

        // Clear histogram
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Clear Encoder"),
        });
        encoder.clear_buffer(&self.histogram_buffer, 0, None);
        encoder.clear_buffer(&self.iteration_counts_buffer, 0, None);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Run iterations
        let workgroups_per_dispatch = 128u32;
        let threads_per_workgroup = 64u64;
        let iterations_per_dispatch_total = workgroups_per_dispatch as u64 * threads_per_workgroup * iterations_per_dispatch as u64;
        let num_dispatches = (total_iterations + iterations_per_dispatch_total - 1) / iterations_per_dispatch_total;

        let mut current_iterations = 0u64;
        for dispatch in 0..num_dispatches {
            // Update params for this dispatch
            let seed = dispatch as u32 * 12345; // Simple seed progression
            let params = GpuParams {
                num_transforms: config.flame.transforms.len() as u32,
                iterations_per_thread: iterations_per_dispatch,
                burn_in: 20,
                width: self.tile_size,  // Tile size for histogram indexing
                height: self.tile_size,
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
            self.queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

            // Dispatch compute
            let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Compute Encoder"),
            });
            {
                let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Tiled Compute Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);
                compute_pass.dispatch_workgroups(workgroups_per_dispatch, 1, 1);
            }
            self.queue.submit(std::iter::once(encoder.finish()));

            current_iterations += iterations_per_dispatch_total;
            progress.on_iterations(current_iterations.min(total_iterations), total_iterations);
        }

        progress.on_complete();
        Ok(())
    }

    /// Get tile grid info
    pub fn tile_info(&self) -> (u32, u32, u32) {
        (self.tiles_x, self.tiles_y, self.tile_size)
    }
}
