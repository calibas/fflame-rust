//! Tiled renderer for high-resolution export
//!
//! Creates a single-pass tiled rendering pipeline where iterations run once
//! and samples are routed to tile buffers based on screen coordinates.

use egui_wgpu::wgpu::*;
use egui_wgpu::wgpu::util::DeviceExt;
use crate::config::FractalConfig;
use crate::gpu::buffers::{GpuParams, GpuTransform, AccumulateParams, TonemapParams};
use crate::scene::palette::Palette;
use crate::scene::transforms::RenderMode;
use crate::shader_builder_v2::ShaderBuilder;
use crate::variations::global_registry;
use super::{TileParams, calculate_tile_grid, max_tiles_per_buffer};

/// Tile info uniform for accumulate shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileInfo {
    pub tile_index: u32,
    pub tile_size: u32,
    pub _padding: [u32; 2],
}

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

    // Compute pass buffers
    transform_buffer: Buffer,
    params_buffer: Buffer,
    tile_params_buffer: Buffer,
    histogram_buffer: Buffer,  // Large buffer for multiple tiles
    iteration_counts_buffer: Buffer,
    variation_params_buffer: Buffer,
    palette_texture: Texture,
    palette_sampler: Sampler,

    // Accumulate pass resources
    tile_info_buffer: Buffer,
    accumulate_params_buffer: Buffer,
    accumulation_texture_a: Texture,
    accumulation_texture_b: Texture,
    accumulate_pipeline: ComputePipeline,
    accumulate_bind_group_layout: BindGroupLayout,

    // Tonemap pass resources
    tonemap_params_buffer: Buffer,
    tonemap_output_texture: Texture,
    curve_lut_texture: Texture,
    curve_lut_sampler: Sampler,
    tonemap_pipeline: RenderPipeline,
    tonemap_bind_group_layout: BindGroupLayout,

    // Readback buffer for final image
    readback_buffer: Buffer,

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

        // Get adapter limits to request higher storage buffer size if available
        let adapter_limits = adapter.limits();
        log::info!(
            "Adapter max_storage_buffer_binding_size: {} bytes ({} MB)",
            adapter_limits.max_storage_buffer_binding_size,
            adapter_limits.max_storage_buffer_binding_size / (1024 * 1024)
        );

        // Request limits with higher storage buffer size (up to adapter's max)
        let mut required_limits = Limits::default();
        required_limits.max_storage_buffer_binding_size =
            adapter_limits.max_storage_buffer_binding_size;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Tiled Export Device"),
                    required_features: Features::empty(),
                    required_limits,
                    memory_hints: MemoryHints::default(),
                    trace: Default::default(),
                    experimental_features: Default::default(),
                },
            )
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;

        log::info!(
            "Device max_storage_buffer_binding_size: {} bytes ({} MB)",
            device.limits().max_storage_buffer_binding_size,
            device.limits().max_storage_buffer_binding_size / (1024 * 1024)
        );

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
        let max_binding_size = device.limits().max_storage_buffer_binding_size as u64;

        if histogram_size > max_binding_size {
            return Err(format!(
                "Histogram buffer size ({} MB) exceeds device limit ({} MB). \
                Try reducing output resolution or number of tiles.",
                histogram_size / (1024 * 1024),
                max_binding_size / (1024 * 1024)
            ));
        }

        log::info!(
            "Creating histogram buffer: {} MB ({} tiles × {} pixels/tile × 16 bytes/pixel)",
            histogram_size / (1024 * 1024),
            total_tiles,
            tile_size * tile_size
        );

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

        // ========== Accumulate pass resources ==========
        let tile_info_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Tile Info Buffer"),
            size: std::mem::size_of::<TileInfo>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let accumulate_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Accumulate Params Buffer"),
            size: std::mem::size_of::<AccumulateParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Tile-sized accumulation textures (ping-pong)
        let accumulation_texture_a = device.create_texture(&TextureDescriptor {
            label: Some("Accumulation Texture A"),
            size: Extent3d { width: tile_size, height: tile_size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let accumulation_texture_b = device.create_texture(&TextureDescriptor {
            label: Some("Accumulation Texture B"),
            size: Extent3d { width: tile_size, height: tile_size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Accumulate shader and pipeline
        let accumulate_shader_source = include_str!("../../shaders/accumulate_tiled.wgsl");
        let accumulate_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Tiled Accumulate Shader"),
            source: ShaderSource::Wgsl(accumulate_shader_source.into()),
        });

        let accumulate_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Accumulate Bind Group Layout"),
            entries: &[
                // binding 0: previous accumulation texture
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
                // binding 1: histogram buffer
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: output texture
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba16Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 3: accumulate params
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 4: iteration counts
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 5: tile info
                BindGroupLayoutEntry {
                    binding: 5,
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

        let accumulate_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Accumulate Pipeline Layout"),
            bind_group_layouts: &[&accumulate_bind_group_layout],
            push_constant_ranges: &[],
        });

        let accumulate_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Tiled Accumulate Pipeline"),
            layout: Some(&accumulate_pipeline_layout),
            module: &accumulate_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ========== Tonemap pass resources ==========
        let tonemap_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Tonemap Params Buffer"),
            size: std::mem::size_of::<TonemapParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Tile-sized output texture for tonemapped result
        let tonemap_output_texture = device.create_texture(&TextureDescriptor {
            label: Some("Tonemap Output Texture"),
            size: Extent3d { width: tile_size, height: tile_size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Curve LUT texture (identity curve for now)
        let curve_lut_texture = device.create_texture(&TextureDescriptor {
            label: Some("Curve LUT Texture"),
            size: Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Write identity curve
        let identity_curve: Vec<u8> = (0..256).map(|i| i as u8).collect();
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &identity_curve,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: None,
            },
            Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
        );

        let curve_lut_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Curve LUT Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // Tonemap shader and pipeline
        let tonemap_shader_source = include_str!("../../shaders/tonemap.wgsl");
        let tonemap_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Tonemap Shader"),
            source: ShaderSource::Wgsl(tonemap_shader_source.into()),
        });

        let tonemap_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Tonemap Bind Group Layout"),
            entries: &[
                // binding 0: accumulation texture
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 1: sampler
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: tonemap params
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: curve LUT texture
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: curve LUT sampler
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let tonemap_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Tonemap Pipeline Layout"),
            bind_group_layouts: &[&tonemap_bind_group_layout],
            push_constant_ranges: &[],
        });

        let tonemap_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Tonemap Pipeline"),
            layout: Some(&tonemap_pipeline_layout),
            vertex: VertexState {
                module: &tonemap_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &tonemap_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ========== Readback buffer for final image ==========
        // We need to copy tonemapped tiles to CPU, so create a buffer for one tile
        let bytes_per_row = ((tile_size * 4 + 255) / 256) * 256; // 256-byte alignment
        let readback_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Tile Readback Buffer"),
            size: (bytes_per_row * tile_size) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
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
            tile_info_buffer,
            accumulate_params_buffer,
            accumulation_texture_a,
            accumulation_texture_b,
            accumulate_pipeline,
            accumulate_bind_group_layout,
            tonemap_params_buffer,
            tonemap_output_texture,
            curve_lut_texture,
            curve_lut_sampler,
            tonemap_pipeline,
            tonemap_bind_group_layout,
            readback_buffer,
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
                bits_per_transform: crate::gpu::buffers::bits_per_transform(config.flame.transforms.len() as u32),
                path_map_style: config.path_map_style as u32,
                path_capture_mode: config.path_capture_mode as u32,
                path_tracking_mode: config.path_tracking_mode as u32,
                num_path_filters: 0, // Path filters not supported in export mode
                min_suffix_filter_length: 0,
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

    /// Run complete tiled export: compute → accumulate → tonemap → stitch
    pub async fn export(
        &mut self,
        config: &FractalConfig,
        total_iterations: u64,
        iterations_per_dispatch: u32,
        progress: &mut dyn TiledExportProgress,
    ) -> Result<Vec<u8>, String> {
        // Step 1: Run iterations (fills histogram buffer)
        self.run_iterations(config, total_iterations, iterations_per_dispatch, progress).await?;

        // Step 2: Process each tile (accumulate + tonemap + readback)
        let total_tiles = self.tiles_x * self.tiles_y;
        let mut final_image = vec![0u8; (self.full_width * self.full_height * 4) as usize];

        for tile_idx in 0..total_tiles {
            progress.on_tile_complete(tile_idx, total_tiles);

            // Process this tile
            let tile_data = self.process_tile(config, tile_idx).await?;

            // Calculate tile position in final image
            let tile_x = tile_idx % self.tiles_x;
            let tile_y = tile_idx / self.tiles_x;
            let start_x = tile_x * self.tile_size;
            let start_y = tile_y * self.tile_size;

            // Copy tile data to final image
            for row in 0..self.tile_size {
                let dst_y = start_y + row;
                if dst_y >= self.full_height {
                    continue;
                }

                let src_row_start = (row * self.tile_size * 4) as usize;
                let dst_row_start = ((dst_y * self.full_width + start_x) * 4) as usize;

                let copy_width = (self.tile_size).min(self.full_width - start_x);
                let src_row_end = src_row_start + (copy_width * 4) as usize;
                let dst_row_end = dst_row_start + (copy_width * 4) as usize;

                final_image[dst_row_start..dst_row_end]
                    .copy_from_slice(&tile_data[src_row_start..src_row_end]);
            }
        }

        progress.on_complete();
        Ok(final_image)
    }

    /// Process a single tile: accumulate histogram → tonemap → readback
    async fn process_tile(
        &mut self,
        config: &FractalConfig,
        tile_idx: u32,
    ) -> Result<Vec<u8>, String> {
        // Update tile info for accumulate shader
        let tile_info = TileInfo {
            tile_index: tile_idx,
            tile_size: self.tile_size,
            _padding: [0, 0],
        };
        self.queue.write_buffer(&self.tile_info_buffer, 0, bytemuck::bytes_of(&tile_info));

        // Update accumulate params (blend_factor = 1.0 for single-pass export)
        let accumulate_params = AccumulateParams {
            width: self.tile_size,
            height: self.tile_size,
            blend_factor: 1.0,  // Overwrite mode for export
            histogram_color_scale: config.histogram_color_scale,
            target_iterations_per_pixel: 0,  // No per-pixel limiting for export
            _pad0: 0.0,
            background_r: config.background_color[0],
            background_g: config.background_color[1],
            background_b: config.background_color[2],
            _pad1: 0.0,
        };
        self.queue.write_buffer(&self.accumulate_params_buffer, 0, bytemuck::bytes_of(&accumulate_params));

        // Create texture views
        let accum_view_a = self.accumulation_texture_a.create_view(&TextureViewDescriptor::default());
        let accum_view_b = self.accumulation_texture_b.create_view(&TextureViewDescriptor::default());

        // Note: blend_factor=1.0 in overwrite mode ignores previous values,
        // so we don't need to explicitly clear texture A

        // Run accumulate pass (reads histogram[tile_idx], writes to texture B)
        let accumulate_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Accumulate Bind Group"),
            layout: &self.accumulate_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&accum_view_a) },
                BindGroupEntry { binding: 1, resource: self.histogram_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&accum_view_b) },
                BindGroupEntry { binding: 3, resource: self.accumulate_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: self.iteration_counts_buffer.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: self.tile_info_buffer.as_entire_binding() },
            ],
        });

        let workgroups_x = (self.tile_size + 7) / 8;
        let workgroups_y = (self.tile_size + 7) / 8;

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Accumulate Encoder"),
        });
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Accumulate Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.accumulate_pipeline);
            compute_pass.set_bind_group(0, &accumulate_bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));

        // Run tonemap pass (reads texture B, renders to output texture)
        self.run_tonemap_pass(config, &accum_view_b)?;

        // Read back the tonemapped tile
        self.readback_tile().await
    }

    /// Run tonemap render pass
    fn run_tonemap_pass(
        &self,
        config: &FractalConfig,
        accum_view: &TextureView,
    ) -> Result<(), String> {
        use crate::config::defaults::*;

        // Update tonemap params
        let tonemap_params = TonemapParams {
            exposure: config.exposure,
            gamma: config.gamma,
            density_scale: config.density_scale,
            tonemap_mode: config.tonemap_mode as u32,
            background_color: config.background_color,
            _pad_bg: 0.0,
            use_curve: if config.use_curve { 1 } else { 0 },
            vibrancy: config.vibrancy,
            brightness: config.brightness,
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area: (self.tile_size * self.tile_size) as f32,
            sample_density: 1.0,  // Will be calculated based on iterations
            saturation: config.saturation,
            hue_shift: config.hue_shift,
            gamma_threshold: config.gamma_threshold,
            alpha_blend_low: config.alpha_blend_low,
            alpha_blend_high: config.alpha_blend_high,
            transparent_mode: 0,  // Opaque export for now
            color_mode: config.color_mode as u32,
            width: self.tile_size,
            height: self.tile_size,
            path_map_style: config.path_map_style as u32,
            burn_in: 20, // Default burn-in for export
            num_transforms: config.flame.transforms.len() as u32,
            _pad_end: [0, 0, 0, 0],
        };
        self.queue.write_buffer(&self.tonemap_params_buffer, 0, bytemuck::bytes_of(&tonemap_params));

        // Create sampler for accumulation texture
        let accum_sampler = self.device.create_sampler(&SamplerDescriptor {
            label: Some("Accum Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let curve_view = self.curve_lut_texture.create_view(&TextureViewDescriptor::default());

        let tonemap_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Tonemap Bind Group"),
            layout: &self.tonemap_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: BindingResource::TextureView(accum_view) },
                BindGroupEntry { binding: 1, resource: BindingResource::Sampler(&accum_sampler) },
                BindGroupEntry { binding: 2, resource: self.tonemap_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&curve_view) },
                BindGroupEntry { binding: 4, resource: BindingResource::Sampler(&self.curve_lut_sampler) },
            ],
        });

        let output_view = self.tonemap_output_texture.create_view(&TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Tonemap Encoder"),
        });
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Tonemap Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.tonemap_pipeline);
            render_pass.set_bind_group(0, &tonemap_bind_group, &[]);
            render_pass.draw(0..3, 0..1);  // Fullscreen triangle
        }
        self.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Read back the tonemapped tile from GPU
    async fn readback_tile(&self) -> Result<Vec<u8>, String> {
        let bytes_per_row = ((self.tile_size * 4 + 255) / 256) * 256;

        // Copy texture to readback buffer
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Readback Encoder"),
        });
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &self.tonemap_output_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &self.readback_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.tile_size),
                },
            },
            Extent3d {
                width: self.tile_size,
                height: self.tile_size,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = self.readback_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await
            .map_err(|_| "Failed to receive map result".to_string())?
            .map_err(|e| format!("Failed to map buffer: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Remove padding from rows
        let mut result = Vec::with_capacity((self.tile_size * self.tile_size * 4) as usize);
        for row in 0..self.tile_size {
            let row_start = (row * bytes_per_row) as usize;
            let row_end = row_start + (self.tile_size * 4) as usize;
            result.extend_from_slice(&data[row_start..row_end]);
        }

        drop(data);
        self.readback_buffer.unmap();

        Ok(result)
    }
}
