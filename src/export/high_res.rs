//! High-resolution export using CPU-side histogram accumulation
//!
//! This approach avoids GPU buffer size limits by:
//! 1. GPU generates samples → outputs to buffer (x, y, r, g, b)
//! 2. CPU reads samples → accumulates into per-pixel f64 histogram (parallelized with rayon)
//! 3. Upload histogram to GPU texture → GPU tonemaps → outputs final RGBA pixels
//!
//! This allows exports of any resolution limited only by system RAM,
//! while still using GPU for fast tonemapping.

use egui_wgpu::wgpu::util::DeviceExt;
use egui_wgpu::wgpu::*;
use half::f16;
use rayon::prelude::*;

use crate::config::FractalConfig;
use crate::gpu::buffers::{GpuParams, GpuTransform, GpuVariationParams, TonemapParams};
use crate::renderer::effect_chain::EffectChainRunner;
use crate::scene::tonemap::ToneCurve;
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

    // GPU resources for sample generation
    transform_buffer: Buffer,
    params_buffer: Buffer,
    sample_buffer: Buffer,
    sample_counter_buffer: Buffer,
    variation_params_buffer: Buffer,
    xaos_buffer: Buffer,  // Xaos transition weights (identity if not used)
    attachments_buffer: Buffer,  // Per-normal Linked + Final attachment lists
    // Dummy path-tracking buffers — the unified shader's `header.wgsl`
    // declares `path_buffer` (binding 7) and `path_filters` (binding 8)
    // unconditionally, but the export shader builds with
    // PATH_TRACKING=false so the use-sites are stripped. WebGPU still
    // requires every declared binding to be bound; minimum-size dummies
    // (28 bytes for one PathEntry, 16 for one GpuPathFilter) satisfy
    // the layout. Pruning these bindings is a Phase 2d-or-later cleanup.
    dummy_path_buffer: Buffer,
    dummy_path_filter_buffer: Buffer,
    palette_texture: Texture,
    palette_sampler: Sampler,

    // Compute pipeline for sample generation
    compute_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,

    // Variation init pipeline (runs once before sample generation to
    // populate derived params for variations with `wgsl_init` —
    // e.g. Julian's `cpower = dist / |power| / 2`). Mirrors the
    // FlameRenderer init-pass machinery in compute_kernel.rs. None
    // when no active variation has init, in which case the dispatch
    // is skipped.
    init_pipeline: Option<ComputePipeline>,
    init_bind_group_layout: BindGroupLayout,
    init_pair_count: u32,

    // GPU resources for tonemapping
    tonemap_pipeline: RenderPipeline,
    tonemap_bind_group_layout: BindGroupLayout,
    tonemap_params_buffer: Buffer,
    curve_lut_texture: Texture,
    curve_lut_sampler: Sampler,
    accumulation_sampler: Sampler,

    // Configuration
    render_mode: RenderMode,
    samples_per_dispatch: u64,
    iterations_per_thread: u32,
}

impl HighResExporter {
    /// Threads per workgroup (fixed by shader)
    const THREADS_PER_WORKGROUP: u64 = 64;

    /// Default iterations per thread for high-res export
    const DEFAULT_ITERATIONS_PER_THREAD: u32 = 256;

    /// Target buffer size in bytes (~128MB - within GPU max_storage_buffer_binding_size)
    /// Most GPUs have a limit of 128-134MB, so we use 128MB to be safe
    /// Larger buffer = fewer round-trips = faster export
    const TARGET_BUFFER_SIZE: u64 = 128 * 1024 * 1024;

    /// Calculate optimal workgroups based on target buffer size and iterations_per_thread
    fn calculate_workgroups(iterations_per_thread: u32) -> u64 {
        let sample_size = std::mem::size_of::<Sample>() as u64; // 32 bytes
        let samples_per_workgroup = Self::THREADS_PER_WORKGROUP * iterations_per_thread as u64;
        let bytes_per_workgroup = samples_per_workgroup * sample_size;

        // Calculate workgroups to fill target buffer
        let workgroups = Self::TARGET_BUFFER_SIZE / bytes_per_workgroup;

        // Clamp to reasonable range (min 128, max 65535 for GPU compatibility)
        workgroups.clamp(128, 65535)
    }

    /// Calculate samples per dispatch for given iterations_per_thread
    fn samples_per_dispatch(iterations_per_thread: u32) -> u64 {
        let workgroups = Self::calculate_workgroups(iterations_per_thread);
        workgroups * Self::THREADS_PER_WORKGROUP * iterations_per_thread as u64
    }

    /// Create a new high-resolution exporter
    ///
    /// `iterations_per_thread`: Number of iterations each GPU thread performs per dispatch.
    /// Higher values = fewer dispatches but same total work. Affects tonemap brightness scaling.
    /// Use `None` for default (256).
    pub async fn new(
        config: &FractalConfig,
        width: u32,
        height: u32,
        iterations_per_thread: Option<u32>,
    ) -> Result<Self, String> {
        let iterations_per_thread = iterations_per_thread.unwrap_or(Self::DEFAULT_ITERATIONS_PER_THREAD);
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

        // Log export configuration
        let workgroups = Self::calculate_workgroups(iterations_per_thread);
        let samples_per_dispatch = Self::samples_per_dispatch(iterations_per_thread);
        let buffer_size_mb = (samples_per_dispatch * std::mem::size_of::<Sample>() as u64) / (1024 * 1024);
        log::info!(
            "High-res export: {}x{}, {} workgroups, {} samples/dispatch (~{}MB buffer)",
            width, height, workgroups, samples_per_dispatch, buffer_size_mb
        );

        // Create transform buffer with solo mode handling
        let transforms = GpuTransform::from_flame(&config.flame, &global_registry());

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

        // Create sample output buffer (sized for samples per dispatch based on iterations_per_thread)
        let samples_per_dispatch = Self::samples_per_dispatch(iterations_per_thread);
        let sample_buffer_size = samples_per_dispatch * std::mem::size_of::<Sample>() as u64;
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

        // Variation params buffer — sized for the worst-case
        // MAX_TRANSFORMS slots so flames whose pool count exceeds the
        // old 32-slot cap don't overflow the write.
        let variation_params = GpuVariationParams::from_flame(&config.flame, &global_registry());
        let variation_params_size = (crate::gpu::buffers::MAX_TRANSFORMS
            * std::mem::size_of::<GpuVariationParams>()) as u64;
        let variation_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Variation Params Buffer"),
            size: variation_params_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&variation_params_buffer, 0, bytemuck::cast_slice(&variation_params));

        // Create xaos buffer (identity weights if not used)
        let num_transforms = config.flame.transforms.len().max(1) as u32;
        let xaos_size = (num_transforms * num_transforms * 4) as u64;
        let xaos_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Xaos Buffer"),
            size: xaos_size.max(4), // At least 4 bytes for empty buffer
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Initialize xaos buffer with weights
        if let Some(flat_weights) = config.flame.xaos_flat() {
            queue.write_buffer(&xaos_buffer, 0, bytemuck::cast_slice(&flat_weights));
        } else {
            // Fill with 1.0 (identity - no xaos modification)
            let identity: Vec<f32> = vec![1.0; (num_transforms * num_transforms) as usize];
            queue.write_buffer(&xaos_buffer, 0, bytemuck::cast_slice(&identity));
        }

        // Per-normal attachment lists (Linked + Final chains). The GPU
        // struct stride matches the per-flame `attachment_cap` — must
        // agree with the value the shader was built with.
        // See per-transform-linked-and-final.md.
        let cap = config.flame.attachment_cap();
        let stride = crate::gpu::buffers::attachment_stride_bytes(cap);
        let attachments_buffer_size = (crate::gpu::buffers::MAX_TRANSFORMS * stride) as u64;
        let attachments_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Attachments Buffer"),
            size: attachments_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let n = config.flame.transforms.len();
        let l = config.flame.linked_transforms.len();
        let f = config.flame.final_transforms.len();
        let mut buf = vec![0u8; crate::gpu::buffers::MAX_TRANSFORMS * stride];
        for (i, t) in config.flame.transforms.iter().enumerate() {
            crate::gpu::buffers::pack_attachment_entry(
                &mut buf[i * stride..(i + 1) * stride],
                t, cap, n, l, n + l, f,
            );
        }
        queue.write_buffer(&attachments_buffer, 0, &buf);

        // Dummy path_buffer (binding 7) and path_filters (binding 8). The
        // unified shader declares these unconditionally in header.wgsl;
        // PATH_TRACKING=false in the export build strips the use-sites
        // but the bindings still need a buffer. Sizes match the FlameRenderer
        // dummies in gpu/buffers.rs: 28 bytes for one PathEntry,
        // 16 bytes for one GpuPathFilter.
        let dummy_path_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Dummy Path Buffer"),
            size: 28,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy_path_filter_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Dummy Path Filter Buffer"),
            size: 16,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create palette texture (palette is always present)
        let palette = &config.palette;
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

        // Build active variations map (include final transform)
        let mut active_variations = std::collections::HashMap::new();
        for transform in &config.flame.transforms {
            for name in transform.active_variations() {
                let weight = transform.get_variation(&name);
                if weight != 0.0 {
                    active_variations.insert(name, weight);
                }
            }
        }
        // Include all final-pool transforms' variations in shader.
        for final_xform in &config.flame.final_transforms {
            for name in final_xform.active_variations() {
                let weight = final_xform.get_variation(&name);
                if weight != 0.0 {
                    active_variations.insert(name, weight);
                }
            }
        }
        // Include all linked-pool transforms' variations in shader.
        for linked_xform in &config.flame.linked_transforms {
            for name in linked_xform.active_variations() {
                let weight = linked_xform.get_variation(&name);
                if weight != 0.0 {
                    active_variations.insert(name, weight);
                }
            }
        }

        // Build shader through the unified template path with
        // OUTPUT_HISTOGRAM_DIRECT=false. The shader writes one Sample per
        // plotted point to `sample_buffer` (binding 2) and bumps an atomic
        // count in `sample_counter_buffer` (binding 6) — a host-side
        // accumulate scatters those into the CPU histogram below.
        //
        // render_3d=true regardless of the flame's render_mode: high-res
        // export reuses the 3D code path so configs with 3D variations
        // (flatten/hemisphere/zcone) render correctly even from a 2D
        // flame (Z=0 falls through projection unchanged). This matches
        // the previous `build_export` behavior.
        //
        // path_features_enabled=false: PathMap export was lossy via path
        // hashing in the old export shader and is gated out here. Configs
        // exporting in PathMap COLOR_MODE will fall back to the white
        // default initialized in main_template.wgsl. See
        // docs/projects/unified-render-pipeline.md.
        let shader_builder = ShaderBuilder::new(global_registry().clone());
        let constants = crate::shader_cache::ShaderCache::constants_from_config(config);
        let shader_source = shader_builder.build_from_template(
            &config.flame,
            &active_variations,
            true,                       // render_3d
            false,                      // path_features_enabled
            config.flame.has_xaos(),    // xaos_enabled
            false,                      // output_histogram_direct → sample-emit
            &constants,
        );

        // Mirror build_from_template's debug dump for the export shader.
        // Writes to a distinct filename so a session that goes through
        // both the interactive and export paths leaves both shaders on
        // disk for inspection.
        if crate::shader_builder_v2::should_dump_shader() {
            let filename = "debug_shader_export.wgsl";
            if let Err(e) = std::fs::write(filename, &shader_source) {
                log::error!("Failed to write export debug shader: {}", e);
            } else {
                log::info!(
                    "Wrote export shader to {} ({} bytes, {} lines)",
                    filename, shader_source.len(), shader_source.lines().count()
                );
            }
        }

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Export Compute Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Build the variation init shader if any active variation has
        // `wgsl_init`. Mirrors `FlameRenderer`'s init pass in
        // compute_kernel.rs — without this, init-derived params (like
        // Julian's `cpower`) stay at 0.0 in the variation_params buffer
        // and parameterized variations render as their degenerate
        // defaults. Pre-existing bug only fixed once HighResExporter
        // was wired up to the same machinery.
        let init_bind_group_layout = crate::shader_cache::ShaderCache::create_init_bind_group_layout(&device);
        let (init_pipeline, init_pair_count) = match shader_builder.build_init_shader(&config.flame, &active_variations) {
            Some(init_source) => {
                let pair_count = init_source
                    .lines()
                    .filter(|l| {
                        let t = l.trim_start();
                        t.starts_with("case ") && t.contains("u: {")
                    })
                    .count() as u32;
                let init_module = device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("Export Variation Init Shader"),
                    source: ShaderSource::Wgsl(init_source.into()),
                });
                let init_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Export Init Pipeline Layout"),
                    bind_group_layouts: &[&init_bind_group_layout],
                    push_constant_ranges: &[],
                });
                let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some("Export Variation Init"),
                    layout: Some(&init_layout),
                    module: &init_module,
                    entry_point: Some("init_main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
                (Some(pipeline), pair_count)
            }
            None => (None, 0),
        };

        // Create bind group layout matching the unified template's 11-slot
        // scheme — same as the interactive renderer's layout but with
        // sample-emit replacements at slots 2 (samples) and 6 (counter).
        // Slots 7 and 8 (path_buffer, path_filters) are dummy bindings:
        // the export shader builds with PATH_TRACKING=false so the
        // use-sites are stripped, but WebGPU still requires every
        // declared binding to be bound.
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
                // binding 1: params (uniform)
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
                // binding 2: samples (sample-emit output)
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
                // binding 6: sample counter (sample-emit write cursor)
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
                // binding 7: path_buffer (dummy — PATH_TRACKING=false in export)
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 8: path_filters (dummy — PATH_TRACKING=false in export)
                BindGroupLayoutEntry {
                    binding: 8,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 9: xaos weights
                BindGroupLayoutEntry {
                    binding: 9,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 10: per-normal attachment lists (Linked + Final chains)
                BindGroupLayoutEntry {
                    binding: 10,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
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

        // ===== Create tonemap pipeline for GPU tonemapping =====
        // Use export-specific shader without path buffer/palette bindings (only 5 bindings: 0-4)
        let tonemap_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Export Tonemap Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/tonemap_export.wgsl").into()),
        });

        // Tonemap bind group layout (matches FlamePipelines)
        let tonemap_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Export Tonemap Bind Group Layout"),
            entries: &[
                // Accumulation texture (sampled)
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
                // Sampler
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // Tonemap params (uniform)
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
                // Curve LUT texture
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
                // Curve LUT sampler
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let tonemap_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Export Tonemap Pipeline Layout"),
            bind_group_layouts: &[&tonemap_bind_group_layout],
            push_constant_ranges: &[],
        });

        let tonemap_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Export Tonemap Pipeline"),
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

        // Create tonemap params buffer
        let tonemap_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Tonemap Params Buffer"),
            size: std::mem::size_of::<TonemapParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create curve LUT texture (linear curve by default)
        let default_curve = ToneCurve::linear();
        let curve_lut_data = default_curve.generate_lut();

        let curve_lut_texture = device.create_texture(&TextureDescriptor {
            label: Some("Export Curve LUT Texture"),
            size: Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &curve_lut_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let curve_lut_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Export Curve LUT Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let accumulation_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Export Accumulation Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
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
            xaos_buffer,
            attachments_buffer,
            dummy_path_buffer,
            dummy_path_filter_buffer,
            palette_texture,
            palette_sampler,
            compute_pipeline,
            bind_group_layout,
            init_pipeline,
            init_bind_group_layout,
            init_pair_count,
            tonemap_pipeline,
            tonemap_bind_group_layout,
            tonemap_params_buffer,
            curve_lut_texture,
            curve_lut_sampler,
            accumulation_sampler,
            render_mode: config.flame.render_mode,
            samples_per_dispatch,
            iterations_per_thread,
        })
    }

    /// Export to RGBA pixel data
    pub async fn export(
        &mut self,
        config: &FractalConfig,
        total_iterations: u64,
        transparent: bool,
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
                BindGroupEntry {
                    binding: 7,
                    resource: self.dummy_path_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: self.dummy_path_filter_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 9,
                    resource: self.xaos_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 10,
                    resource: self.attachments_buffer.as_entire_binding(),
                },
            ],
        });

        // Run the variation init pass once if any active variation has
        // `wgsl_init`. Populates derived params (e.g. Julian's `cpower`)
        // in `variation_params_buffer` so the main sample-generation
        // pass reads correct values via `get_param`. Without this,
        // parameterized variations render as their degenerate defaults
        // (Julian collapses to a unit circle, Blob loses its shape, etc).
        if let Some(init_pipeline) = self.init_pipeline.as_ref() {
            if self.init_pair_count > 0 {
                let init_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Export Variation Init Bind Group"),
                    layout: &self.init_bind_group_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: self.variation_params_buffer.as_entire_binding(),
                    }],
                });
                let mut init_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Export Variation Init Encoder"),
                });
                {
                    let mut init_pass = init_encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("Export Variation Init Pass"),
                        timestamp_writes: None,
                    });
                    init_pass.set_pipeline(init_pipeline);
                    init_pass.set_bind_group(0, &init_bind_group, &[]);
                    let workgroups = (self.init_pair_count + 63) / 64;
                    init_pass.dispatch_workgroups(workgroups, 1, 1);
                }
                self.queue.submit(std::iter::once(init_encoder.finish()));
            }
        }

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

        // Calculate dispatch parameters using dynamic workgroup count
        let workgroups_per_dispatch = Self::calculate_workgroups(self.iterations_per_thread) as u32;
        let iterations_per_dispatch = self.samples_per_dispatch;
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
                iterations_per_thread: self.iterations_per_thread,
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
                dof_focus_distance: config.dof_focus_distance,
                dof_blur_strength: config.dof_blur_strength,
                fog_strength: config.fog_strength,
                fog_start: config.fog_start,
                histogram_color_scale: config.histogram_color_scale,
                has_final_transform: if !config.flame.final_transforms.is_empty() { 1 } else { 0 },
                final_transform_index: 0,  // Legacy field — shader uses attachments chain now
                bits_per_transform: crate::gpu::buffers::bits_per_transform(config.flame.transforms.len() as u32),
                path_map_style: config.path_map_style as u32,
                path_capture_mode: config.path_capture_mode as u32,
                path_tracking_mode: config.path_tracking_mode as u32,
                num_path_filters: 0, // Path filters not supported in export mode
                min_suffix_filter_length: 0,
                background_r: config.background_color[0],
                background_g: config.background_color[1],
                background_b: config.background_color[2],
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

                // Read and accumulate samples (parallelized by row)
                let samples = self
                    .read_samples(&readback_buffer, sample_count.min(self.samples_per_dispatch as u32))
                    .await?;

                // Strategy: Bin samples by row, then accumulate each row in parallel
                // This avoids allocating full histogram copies while still parallelizing
                let width = self.width as i32;
                let height = self.height as i32;
                let width_usize = self.width as usize;
                let height_usize = self.height as usize;

                // Pre-bin samples by their Y coordinate (row)
                // Vec of samples for each row
                let mut row_samples: Vec<Vec<&Sample>> = vec![Vec::new(); height_usize];
                for sample in &samples {
                    let y = sample.y as i32;
                    if y >= 0 && y < height {
                        row_samples[y as usize].push(sample);
                    }
                }

                // Parallel accumulation: each thread processes a chunk of rows
                histogram
                    .par_chunks_mut(width_usize)
                    .enumerate()
                    .for_each(|(row_idx, row_pixels)| {
                        // Process all samples that land in this row
                        for sample in &row_samples[row_idx] {
                            let x = sample.x as i32;
                            if x >= 0 && x < width {
                                let pixel = &mut row_pixels[x as usize];
                                pixel.r += sample.r as f64;
                                pixel.g += sample.g as f64;
                                pixel.b += sample.b as f64;
                                pixel.count += 1.0;
                            }
                        }
                    });

                total_samples_accumulated += samples.len() as u64;
            }
        }

        progress.on_accumulating(total_samples_accumulated);
        progress.on_tonemapping();

        // Tonemap histogram to RGBA using GPU
        let pixels = self.tonemap_gpu(&histogram, config, transparent).await?;

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

    /// GPU tonemap: histogram → RGBA pixels using GPU shader
    /// Uploads CPU histogram to GPU texture, runs tonemap shader, reads back result
    async fn tonemap_gpu(
        &self,
        histogram: &[HistogramPixel],
        config: &FractalConfig,
        transparent: bool,
    ) -> Result<Vec<u8>, String> {
        use crate::config::defaults::{DEFAULT_WHITE_LEVEL, PREFILTER_WHITE, BRIGHT_ADJUST};

        // ===== Step 1: Convert histogram to Rgba16Float format (parallelized) =====
        // The GPU accumulation buffer stores:
        // - R, G, B: averaged colors (sum/count)
        // - A: density as count * 0.01
        //
        // Our CPU histogram stores:
        // - r, g, b: raw sums
        // - count: raw hit count
        //
        // Convert to GPU format: average the colors, scale density
        // Pre-allocate buffer and write in parallel chunks for efficiency
        let mut texture_data = vec![0u8; histogram.len() * 8];
        texture_data
            .par_chunks_mut(8)
            .zip(histogram.par_iter())
            .for_each(|(chunk, pixel)| {
                let (r, g, b, density) = if pixel.count > 0.0 {
                    let r = (pixel.r / pixel.count) as f32;
                    let g = (pixel.g / pixel.count) as f32;
                    let b = (pixel.b / pixel.count) as f32;
                    let density = pixel.count as f32;
                    (r, g, b, density)
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };

                // Convert to f16 bytes (8 bytes per pixel)
                chunk[0..2].copy_from_slice(&f16::from_f32(r).to_le_bytes());
                chunk[2..4].copy_from_slice(&f16::from_f32(g).to_le_bytes());
                chunk[4..6].copy_from_slice(&f16::from_f32(b).to_le_bytes());
                chunk[6..8].copy_from_slice(&f16::from_f32(density).to_le_bytes());
            });


        // ===== Step 2: Create and upload accumulation texture =====
        let accumulation_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("Export Accumulation Texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let bytes_per_row = self.width * 8; // 4 channels × 2 bytes (f16)
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &accumulation_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &texture_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(self.height),
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        let accumulation_view = accumulation_texture.create_view(&TextureViewDescriptor::default());

        // ===== Step 3: Set up tonemap params =====
        // Calculate area and sample_density matching GPU formula
        let zoom = config.zoom;
        let base_pixels_per_unit = (self.width.min(self.height) as f32) * 0.25;
        let apophysis_zoom = zoom.log2();
        let pixels_per_unit_zoomed = base_pixels_per_unit * 2.0_f32.powf(apophysis_zoom);
        let area = (self.width as f32 * self.height as f32) / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);

        // Sample density: scaled by iterations_per_thread AND resolution.
        // Mirrors `FlameRenderer::tonemap_for_export` in compute_kernel.rs.
        // The resolution factor compensates for the fact that at 8000×8000
        // (64M pixels) the per-pixel density is 64× lower than at 1000×1000
        // for the same total iteration count — without this factor the
        // tonemap divides by a too-high sample_density and the image goes
        // perceptually black.
        let total_pixels = (self.width * self.height) as f32;
        let reference_pixels = 1_000_000.0;
        let sample_density = 5000.0
            * (self.iterations_per_thread as f32 / 256.0)
            * (reference_pixels / total_pixels);

        let tonemap_mode = match config.tonemap_mode {
            crate::scene::tonemap::ToneMapMode::Linear => 0u32,
            crate::scene::tonemap::ToneMapMode::Logarithmic => 1u32,
            crate::scene::tonemap::ToneMapMode::DensityVisualization => 2u32,
        };

        let tonemap_params = TonemapParams {
            exposure: config.exposure,
            gamma: config.gamma,
            density_scale: config.density_scale,
            tonemap_mode,
            background_color: config.background_color,
            _pad_bg: 0.0,
            use_curve: if config.use_curve { 1 } else { 0 },
            vibrancy: config.vibrancy,
            brightness: config.brightness,
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: config.saturation,
            hue_shift: config.hue_shift,
            gamma_threshold: config.gamma_threshold,
            alpha_blend_low: config.alpha_blend_low,
            alpha_blend_high: config.alpha_blend_high,
            transparent_mode: if transparent { 1 } else { 0 },
            color_mode: config.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: config.path_map_style as u32,
            burn_in: 20, // Default burn-in for export
            num_transforms: config.flame.transforms.len() as u32,
            palette_size: config.palette_size,
            // Levels defaults for export (no histogram-based adjustment)
            levels_low: 0.0,
            levels_high: 1000.0,
            levels_gamma: 1.0,
        };

        self.queue.write_buffer(
            &self.tonemap_params_buffer,
            0,
            bytemuck::bytes_of(&tonemap_params),
        );

        // Update curve LUT if using curve
        if config.use_curve {
            let curve_lut_data = config.tonemap_curve.generate_lut();
            self.queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &self.curve_lut_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &curve_lut_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None,
                    rows_per_image: None,
                },
                Extent3d {
                    width: 256,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }

        // ===== Step 4: Create bind group and output texture =====
        let curve_lut_view = self.curve_lut_texture.create_view(&TextureViewDescriptor::default());

        let tonemap_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Export Tonemap Bind Group"),
            layout: &self.tonemap_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&accumulation_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.accumulation_sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.tonemap_params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&curve_lut_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::Sampler(&self.curve_lut_sampler),
                },
            ],
        });

        // Create output texture (needs TEXTURE_BINDING for color effects input)
        let output_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("Export Output Texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&TextureViewDescriptor::default());

        // ===== Step 5: Run tonemap render pass =====
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Export Tonemap Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Export Tonemap Pass"),
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
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        // ===== Step 5.5: Run color effects (if enabled) =====
        let has_color_effects = EffectChainRunner::has_enabled_effects(&config.color_effects);
        let mut effect_chain: Option<EffectChainRunner> = None;
        let color_effects_ran = if has_color_effects {
            log::info!("High-res export: Running {} color effect(s)",
                config.color_effects.iter().filter(|e| e.enabled).count());

            let mut chain = EffectChainRunner::new(&self.device, self.width, self.height);

            let mut effect_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Export Color Effects Encoder"),
            });

            chain.reset_slots();
            let ran = chain.run_color_effects(
                &self.device,
                &self.queue,
                &mut effect_encoder,
                &output_view,
                &config.color_effects,
            );

            self.queue.submit(std::iter::once(effect_encoder.finish()));
            effect_chain = Some(chain);
            ran
        } else {
            false
        };

        // ===== Step 6: Read back result =====
        // If color effects ran, read from effect chain output; otherwise read from tonemap output
        if color_effects_ran {
            if let Some(chain) = effect_chain.as_ref() {
                return chain.read_color_output_pixels(&self.device, &self.queue).await
                    .map(|(_, _, pixels)| pixels);
            }
        }

        // Read from tonemap output texture
        let bytes_per_pixel = 4u32; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        let readback_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Export Tonemap Readback Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut copy_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Export Readback Encoder"),
        });

        copy_encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &output_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(copy_encoder.finish()));

        // Map and read pixels
        let buffer_slice = readback_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await
            .map_err(|_| "Failed to receive tonemap readback result".to_string())?
            .map_err(|e| format!("Failed to map tonemap readback buffer: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Copy pixels, removing row padding
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + (self.width * bytes_per_pixel) as usize;
            pixels.extend_from_slice(&data[row_start..row_end]);
        }

        drop(data);
        readback_buffer.unmap();

        Ok(pixels)
    }
}
