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
use rayon::prelude::*;

use crate::config::FractalConfig;
use crate::gpu::buffers::{GpuParams, GpuVariationParams, TonemapParams};
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
    /// Density weight (depth-density compensation; 1.0 = neutral).
    /// Scales the sample's contribution to all four histogram
    /// channels — mirror of the WGSL `Sample.weight` field.
    pub weight: f32,
    /// Solid rendering: camera-space depth (positive = in front). Only
    /// meaningful when the iterate shader was built with SOLID; 0 otherwise.
    pub depth: f32,
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

/// Cubic B-spline basis weights for the four taps at offsets (-1,0,1,2) around
/// fractional position `t`. Sum to 1; smooth, no ringing. Identical to
/// `bspline4` in `blur_upscale.wgsl` (the analytic-blur CPU fold must match the
/// GPU upscale math). See docs/projects/analytic-blur-buffer.md.
fn bspline4(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        (1.0 - 3.0 * t + 3.0 * t2 - t3) / 6.0,
        (4.0 - 6.0 * t2 + 3.0 * t3) / 6.0,
        (1.0 + 3.0 * t + 3.0 * t2 - 3.0 * t3) / 6.0,
        t3 / 6.0,
    ]
}

/// Layout describing how the concatenated tile-histogram buffer is
/// sliced for the ParallelTiles GPU accumulate path. Tiles are
/// horizontal slices of the full image — `tile_width` always equals
/// `image_width`, so each tile's region is a contiguous row-range
/// in pixel-index space and stitching at the end is just sequential
/// memcpy with no per-row gymnastics.
#[derive(Debug, Clone, Copy)]
pub struct TileLayout {
    pub num_tiles: u32,
    pub tile_height: u32,
    pub image_width: u32,
    pub image_height: u32,
    /// 4 (RGBD) normally; 5 with solid rendering — each tile's span then
    /// ends with one u32/pixel of nearest-depth region (the scatter shader
    /// indexes it at bound_width*bound_height*4 + pixel_idx).
    pub words_per_pixel: u32,
}

impl TileLayout {
    /// Byte offset of `tile_idx` inside the concatenated buffer.
    /// Each tile spans `tile_width × tile_height × words_per_pixel × 4`
    /// bytes (RGBD, plus the depth word under solid rendering).
    pub fn tile_byte_offset(&self, tile_idx: u32) -> u64 {
        let tile_pixels = (self.image_width as u64) * (self.tile_height as u64);
        (tile_idx as u64) * tile_pixels * (self.words_per_pixel as u64) * 4
    }

    /// Y-coordinate of the first row of `tile_idx` in full-image
    /// pixel space.
    pub fn tile_y_origin(&self, tile_idx: u32) -> u32 {
        tile_idx * self.tile_height
    }

    /// Actual row count of `tile_idx`. The last tile may be shorter
    /// than `tile_height` when `image_height` doesn't divide evenly.
    pub fn tile_actual_height(&self, tile_idx: u32) -> u32 {
        let y0 = self.tile_y_origin(tile_idx);
        (self.image_height - y0).min(self.tile_height)
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
    subflame_metadata_buffer: Buffer,  // binding 12: per-subflame metadata
    // Dummy path-tracking buffers — the unified shader's `header.wgsl`
    // declares `path_buffer` (binding 7) and `path_filters` (binding 8)
    // unconditionally, but the export shader builds with
    // PATH_TRACKING=false so the use-sites are stripped. WebGPU still
    // requires every declared binding to be bound; minimum-size dummies
    // (28 bytes for one PathEntry, 16 for one GpuPathFilter) satisfy
    // the layout. Pruning these bindings is a Phase 2d-or-later cleanup.
    dummy_path_buffer: Buffer,
    dummy_path_filter_buffer: Buffer,
    // Analytic-blur bindings (13/14) for the now-mode-independent routing.
    // In Phase 2 step 2a these are a dummy splat buffer + a params buffer with
    // count=0, so the routing falls back to stochastic; step 2b makes them a
    // real low-res buffer + nonzero count.
    blur_splat_buffer: Buffer,
    blur_convolve_params_buffer: Buffer,
    /// Analytic-blur convolution setup (None when the flame isn't analytic).
    /// The CPU fold at the end of `render` convolves + upscales the low-res
    /// splat buffer into the histogram using this.
    blur_setup: Option<crate::variations::analytic_blur::BlurSetup>,
    palette_texture: Texture,
    palette_sampler: Sampler,

    // Compute pipeline for sample generation
    compute_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,

    // GPU accumulate pass. The per-iterate-dispatch CPU readback +
    // histogram fill is replaced with one or more compute dispatches
    // that atomic-add samples into a GPU histogram. We read back the
    // histogram exactly once at the end. When the strategy picker
    // can't put the whole thing on GPU these fields are `None` and
    // the exporter falls back to the CPU accumulate path.
    //
    // The single histogram is allocated as one concatenated buffer
    // covering all tile regions (tiles are row-strip slices of the
    // full image; `tile_layout` describes the slicing). For
    // num_tiles == 1 this degenerates to a single-binding direct
    // dispatch; for num_tiles > 1 the accumulate dispatch runs N
    // times per iterate with offset+size sub-range bindings.
    accumulate_pipeline: ComputePipeline,
    accumulate_bind_group_layout: BindGroupLayout,
    accumulate_params_buffer: Buffer,
    tile_histograms_buffer: Option<Buffer>,
    tile_layout: Option<TileLayout>,

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
    /// 1 + the flame's multi-emit cap — samples-per-iteration multiplier
    /// for buffer capacity and workgroup sizing.
    emit_mult: u64,
    iterations_per_thread: u32,
    // Solid rendering (Phase 0): occlusion state mirrored from the config.
    // solid_enabled = strength > 0 && 3D; drives the SOLID iterate shader,
    // the 5-words/px tile spans, and the scatter-pass gating.
    solid_enabled: bool,
    solid_strength: f32,
    surface_thickness: f32,
    /// Shade pass (pipeline only) — present when the config wants lighting.
    shade_pass: Option<crate::renderer::shade_pass::ShadePass>,
    /// Full-image encoded depth gathered before the tile buffer is freed.
    shade_depth_buffer: Option<Buffer>,
    // Light-space shadow maps (Stage 2, GPU-tiled path): written by the
    // scatter pass, copied into shade_depth_buffer's tail before the
    // shade. None on the CPU-histogram path (no maps, v1) or when
    // shadows are off.
    shadow_maps_buffer: Option<Buffer>,
    /// 16-byte stand-in for binding 3 when shadow maps are off.
    shadow_dummy_buffer: Buffer,
    /// CPU-histogram path's exact occlusion counters (Σocc, Σ1);
    /// None on the GPU-tiles path (counters live in the shadow buffer).
    cpu_occ_stats: Option<(f64, f64)>,
    /// Frozen shadow fit (center, radius) — view-derived (the exporter
    /// has no bounds measurement).
    shadow_fit: ([f32; 3], f32),
    /// Per-slot world light dirs (w = enabled) + camera rows/pos for
    /// the scatter's world reconstruction.
    shadow_dirs: [[f32; 4]; 4],
    shadow_cam_rows: [[f32; 3]; 3],
    shadow_cam_pos: [f32; 3],
    /// View-transform inputs the scatter needs to reconstruct world
    /// positions from (pixel, depth) samples.
    shadow_view: (f32, f32, f32, f32, f32), // zoom, rotation, pan_x, pan_y, persp
    /// Accepted/dispatched fraction for solid brightness renormalization.
    solid_density_fraction: f32,
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

    /// Calculate optimal workgroups based on target buffer size and
    /// iterations_per_thread. `emit_mult` = 1 + the flame's multi-emit cap
    /// (`Flame::plot_emit_cap`): an emitting flame produces up to that many
    /// samples per iteration, so fewer workgroups fit the target buffer —
    /// same total iterations, spread over more dispatches.
    fn calculate_workgroups(iterations_per_thread: u32, emit_mult: u64) -> u64 {
        let sample_size = std::mem::size_of::<Sample>() as u64; // 32 bytes
        let samples_per_workgroup =
            Self::THREADS_PER_WORKGROUP * iterations_per_thread as u64 * emit_mult.max(1);
        let bytes_per_workgroup = samples_per_workgroup * sample_size;

        // Calculate workgroups to fill target buffer
        let workgroups = Self::TARGET_BUFFER_SIZE / bytes_per_workgroup;

        // Budget wins over occupancy. The old floor of 128 workgroups
        // silently overrode the 128 MB target once a single workgroup's
        // samples got big: at iterations_per_thread = 4096 it produced a
        // 1 GB sample buffer, and the new 10000 cap would have meant
        // 2.4 GB (times the multi-emit multiplier on emitting flames).
        // Deep-trajectory settings mean each thread already carries
        // plenty of work, so low workgroup counts are the correct trade
        // — exports run slower at extreme depth, they don't OOM. Floor
        // of 1 keeps a degenerate config renderable.
        workgroups.clamp(1, 65535)
    }

    /// Sample-buffer CAPACITY per dispatch (not the iteration count —
    /// with multi-emit each iteration may deposit up to emit_mult samples).
    fn samples_per_dispatch(iterations_per_thread: u32, emit_mult: u64) -> u64 {
        let workgroups = Self::calculate_workgroups(iterations_per_thread, emit_mult);
        workgroups * Self::THREADS_PER_WORKGROUP * iterations_per_thread as u64 * emit_mult.max(1)
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
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..InstanceDescriptor::new_without_display_handle()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .map_err(|e| format!("Failed to find GPU adapter: {}", e))?;

        // Request the adapter's full storage-buffer binding size (the
        // default `DeviceDescriptor` uses WebGPU's spec-minimum 128 MB
        // limit, which forces tile fallback even on desktop GPUs that
        // support gigabyte-sized bindings). Without this, 8K exports
        // get classified as SerialTiles on every desktop and miss the
        // GPU accumulate fast path. Mirrors `gpu/device.rs`'s
        // initialization for the interactive renderer.
        let adapter_limits = adapter.limits();
        log::info!(
            "Export adapter limits: max_storage_buffer_binding_size = {} MB, max_texture_dimension_2d = {}",
            adapter_limits.max_storage_buffer_binding_size / (1024 * 1024),
            adapter_limits.max_texture_dimension_2d,
        );
        let mut limits = Limits::default();
        limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
        limits.max_buffer_size = adapter_limits.max_buffer_size;
        // The accumulation/output textures in `tonemap_gpu` are sized
        // to the requested width/height. Default `Limits` caps at the
        // 8192 WebGPU floor; raise it so 16K+ exports don't fail
        // texture creation on desktops that natively support 16384+.
        limits.max_texture_dimension_2d = adapter_limits.max_texture_dimension_2d;
        // Compute bind group has 10 storage buffers post-subflames; spec floor is 8.
        limits.max_storage_buffers_per_shader_stage =
            adapter_limits.max_storage_buffers_per_shader_stage;

        // Request FLOAT32_FILTERABLE when the adapter advertises it so the
        // density-effect chain can bilinear-sample the Rgba32Float accumulation
        // (run_density_effects skips density entirely without it). Mirrors the
        // FlameRenderer device in gpu/device.rs and the headless device in
        // app/export.rs — without it, density effects silently no-op on the
        // tiled export path.
        let mut required_features = Features::empty();
        if adapter.features().contains(Features::FLOAT32_FILTERABLE) {
            required_features |= Features::FLOAT32_FILTERABLE;
        }

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("Export Device"),
                required_features,
                required_limits: limits,
                memory_hints: MemoryHints::Performance,
                experimental_features: Default::default(),
                trace: Default::default(),
            })
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;

        // Log export configuration
        let emit_mult = 1 + config.flame.plot_emit_cap(&global_registry()) as u64;
        let workgroups = Self::calculate_workgroups(iterations_per_thread, emit_mult);
        let samples_per_dispatch = Self::samples_per_dispatch(iterations_per_thread, emit_mult);
        let buffer_size_mb = (samples_per_dispatch * std::mem::size_of::<Sample>() as u64) / (1024 * 1024);
        log::info!(
            "High-res export: {}x{}, {} workgroups, {} samples/dispatch (~{}MB buffer)",
            width, height, workgroups, samples_per_dispatch, buffer_size_mb
        );

        // Create transform buffer with solo mode handling. The tiled export
        // path is sample-emit (OUTPUT_HISTOGRAM_DIRECT=false) and currently
        // strips analytic-blur routing, so the assigned slots go unread
        // (stochastic blur fallback) until Phase 2 step 2 wires this path.
        // Pack parent + subflame transforms into the unified layout (shared
        // with the FlameRenderer path) so subflames render identically. The
        // buffer is sized for MAX_TRANSFORMS_UNIFIED; subflame xforms occupy
        // [MAX_TRANSFORMS, MAX_TRANSFORMS_UNIFIED).
        let transforms = crate::gpu::buffers::pack_gpu_transforms(&config.flame, config.render_mode);

        let transform_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Export Transform Buffer"),
            contents: bytemuck::cast_slice(&transforms),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // Per-subflame metadata (binding 12). Empty/zeroed when the flame has no
        // subflames; the shader only reads it when subflame variations are active.
        let subflame_metas = crate::gpu::buffers::build_subflame_metas(&config.flame.subflames)
            .map_err(|e| format!("Subflame metadata: {e}"))?;
        let subflame_metadata_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Export Subflame Metadata Buffer"),
            contents: bytemuck::cast_slice(&subflame_metas),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // Create params buffer
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Params Buffer"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sample output buffer. Capacity accounts for multi-emit:
        // up to emit_mult samples per iteration (see calculate_workgroups).
        let samples_per_dispatch = Self::samples_per_dispatch(iterations_per_thread, emit_mult);
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

        // Analytic-blur (13/14). When the flame is analytic (always 2D), build
        // the convolution setup and a REAL low-res splat buffer; the chaos game
        // splats means into it (mean ÷ D) and the CPU folds it in at the end
        // (convolve + upscale) using `setup`. Otherwise a 1-element dummy +
        // count=0 params so the routing falls back to stochastic.
        let blur_slots = config.flame.blur_slots(&global_registry(), config.render_mode);
        let (blur_splat_buffer, blur_setup, blur_count) = if !blur_slots.is_empty() {
            let setup = crate::variations::analytic_blur::compute_blur_setup(
                width, height, config.zoom, config.rotation, &blur_slots,
            );
            let count = blur_slots.len() as u32;
            let slice = (setup.lowres_w as u64) * (setup.lowres_h as u64) * 16;
            let buf = device.create_buffer(&BufferDescriptor {
                label: Some("Export Blur Splat"),
                size: slice * count as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            log::info!(
                "Export analytic blur: {} slice(s) {}×{} low-res, D={}",
                count, setup.lowres_w, setup.lowres_h, setup.downscale
            );
            (buf, Some(setup), count)
        } else {
            let buf = device.create_buffer(&BufferDescriptor {
                label: Some("Export Blur Splat (dummy)"),
                size: 16,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            (buf, None, 0)
        };
        let blur_convolve_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Blur Convolve Params"),
            size: std::mem::size_of::<crate::gpu::buffers::BlurConvolveParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        {
            // Params with `count` so the shader routing splats (or, count=0,
            // falls back to stochastic). Per-slot meta is unused by the shader
            // routing (it only needs D / lowres / count); the CPU fold reads
            // meta straight from `blur_setup`.
            let mut meta = [[0u32; 4]; crate::gpu::buffers::MAX_BLUR_BUFFERS as usize];
            if let Some(ref s) = blur_setup {
                for (i, m) in s.meta.iter().enumerate().take(meta.len()) {
                    meta[i] = *m;
                }
            }
            let params = crate::gpu::buffers::BlurConvolveParams {
                full_width: width,
                full_height: height,
                lowres_width: blur_setup.as_ref().map(|s| s.lowres_w).unwrap_or(0),
                lowres_height: blur_setup.as_ref().map(|s| s.lowres_h).unwrap_or(0),
                downscale: blur_setup.as_ref().map(|s| s.downscale).unwrap_or(1),
                count: blur_count,
                frame_seed: 0,
                _pad1: 0,
                meta,
            };
            queue.write_buffer(&blur_convolve_params_buffer, 0, bytemuck::bytes_of(&params));
        }

        // Variation params buffer — sized for the worst-case
        // MAX_TRANSFORMS slots so flames whose pool count exceeds the
        // old 32-slot cap don't overflow the write.
        // Parent + subflame variation params, sized for the unified slot space
        // (MAX_TRANSFORMS_UNIFIED) so subflame slots [MAX_TRANSFORMS..) exist.
        let variation_params = crate::gpu::buffers::pack_gpu_variation_params(&config.flame);
        let variation_params_size = (crate::gpu::buffers::MAX_TRANSFORMS_UNIFIED
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

        // Create palette texture (palette is always present). Apply the FULL
        // transform pipeline (squeeze / log / rotation / reverse) via
        // render_palette_lookup — the same source of truth the interactive /
        // FlameRenderer path uses (Buffers::update_palette). Using the raw
        // generate_texture_data here made high-res exports ignore palette
        // squeeze/rotation, showing the full palette instead of the squeezed
        // colors.
        let palette = &config.palette;
        let palette_transform = {
            use crate::scene::palette::{PaletteTransform, SqueezeMode};
            let squeeze_factor = match config.palette_squeeze_mode {
                SqueezeMode::Linear => config.palette_squeeze,
                SqueezeMode::Geometric => config.palette_squeeze_falloff,
            };
            PaletteTransform {
                squeeze_mode: config.palette_squeeze_mode,
                squeeze_factor,
                log_strength: config.palette_log_strength,
                rotation: config.palette_rotation,
                reverse: config.palette_reverse,
            }
        };
        // Use the config's palette size (clamped to the valid range, matching
        // FlameBuffers), not a hardcoded 256 — an oversampled palette
        // (size > 256, smoother gradients) otherwise rendered coarser in the
        // tiled export than in-app. `TonemapParams.palette_size` below already
        // uses config.palette_size, so the texture must match it.
        let palette_size = config
            .palette_size
            .clamp(crate::gpu::buffers::DEFAULT_PALETTE_SIZE, crate::gpu::buffers::MAX_PALETTE_SIZE);
        let palette_data =
            crate::scene::palette::render_palette_lookup(palette, &palette_transform, palette_size as usize);
        let palette_data_u8: Vec<u8> = palette_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        let palette_texture = device.create_texture(&TextureDescriptor {
            label: Some("Export Palette Texture"),
            size: Extent3d {
                width: palette_size,
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
                bytes_per_row: Some(palette_size * 4),
                rows_per_image: None,
            },
            Extent3d {
                width: palette_size,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let palette_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Export Palette Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            // NEAREST (not Linear) to match FlameRenderer's palette sampler
            // (gpu/buffers.rs) and flam3/Apophysis, which floor the colormap
            // index (`j = (int)(color * cmap_size)`) with no interpolation.
            // Linear filtering here interpolates between adjacent palette
            // entries, shifting recovered colors a fraction of a percent per
            // channel versus the in-app/FlameRenderer path — visible as a
            // warmth/hue difference in dense regions after tone mapping.
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
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
        // render_3d=true by default: the high-res export reuses the 3D code
        // path so configs with 3D variations (flatten/hemisphere/zcone) render
        // correctly even from a 2D flame (Z=0 falls through projection
        // unchanged). EXCEPTION: an analytic-blur flame (always 2D, per the
        // gate) builds the 2D shader so the 2D-only analytic routing is
        // included — its blur is folded in by the CPU convolve/upscale below.
        //
        // path_features_enabled=false: PathMap export was lossy via path
        // hashing in the old export shader and is gated out here. Configs
        // exporting in PathMap COLOR_MODE will fall back to the white
        // default initialized in main_template.wgsl. See
        // docs/projects/unified-render-pipeline.md.
        let analytic_active = config.flame.analytic_blur_active(&global_registry(), config.render_mode);
        // NeedsW variations break the "Z=0 falls through unchanged" invariant
        // that lets the 3D shader stand in for 2D: their 2D and 3D bodies are
        // deliberately DIFFERENT maps (2D complex Julia vs 4D quaternion with
        // a randomly-seeded w), so a 2D-authored flame must build the 2D
        // shader here or the export won't match the viewport.
        let has_needs_w = active_variations.keys().any(|name| {
            global_registry()
                .get(name)
                .is_some_and(|info| info.has_feature(crate::variations::definition::Feature::NeedsW))
        });
        let force_2d = analytic_active
            || (config.render_mode == crate::scene::transforms::RenderMode::TwoD && has_needs_w);
        let shader_builder = ShaderBuilder::new(global_registry().clone());
        let constants = crate::shader_cache::ShaderCache::constants_from_config(config);
        let shader_source = shader_builder.build_from_template(
            &config.flame,
            &active_variations,
            !force_2d,                  // render_3d (2D shader for analytic / 2D-NeedsW flames)
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
                    bind_group_layouts: &[Some(&init_bind_group_layout)],
                    immediate_size: 0,
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
                // binding 12: subflame metadata (array<SubflameMeta>). Binding 11
                // is intentionally unbound (legacy subflame_transforms, removed).
                BindGroupLayoutEntry {
                    binding: 12,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 13: analytic-blur low-res splat buffer (used by the
                // HAS_ANALYTIC_BLUR routing — now mode-independent). Bound to a
                // real low-res buffer when wired, else a dummy with count=0
                // params so the routing falls back to stochastic.
                BindGroupLayoutEntry {
                    binding: 13,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 14: analytic-blur convolve params (D / lowres / count).
                BindGroupLayoutEntry {
                    binding: 14,
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

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Export Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Export Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ===== Create GPU accumulate pipeline =====
        // Builds the pipeline regardless of whether this device's
        // strategy ends up using it — cheap to construct (single
        // shader module, single pipeline) and lets the export() loop
        // pick the path based on `tile_histograms_buffer.is_some()`.
        let accumulate_bind_group_layout = crate::export::accumulate::create_bind_group_layout(&device);
        let accumulate_pipeline = crate::export::accumulate::create_pipeline(
            &device,
            &accumulate_bind_group_layout,
        );
        let accumulate_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Accumulate Params Buffer"),
            size: std::mem::size_of::<crate::export::accumulate::AccumulateParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Pick GPU accumulate strategy based on resolution + device limits.
        //
        // The Direct strategy (full histogram fits one binding) was a
        // path inside HighResExporter pre-P6 of this branch. As of
        // accumulator-unification's Phase 8d, the routing layer sends
        // those cases to FlameRenderer.render() instead, so Direct
        // is unreachable here and the dedicated buffer/bind-group
        // pair has been removed. ParallelTiles handles `num_tiles==1`
        // trivially anyway, so the unified path covers every
        // resolution that fits one buffer.
        //
        // `ParallelTiles`: total histogram fits one buffer
        //                  (≤ max_buffer_size). One concatenated GPU
        //                  buffer split into N row-strip tiles;
        //                  per-iterate batch runs N accumulate
        //                  dispatches, one per tile, bound via
        //                  offset+size. Single iteration loop.
        //
        // `SerialTiles`:   total exceeds one buffer → CPU fallback.
        //                  See docs/projects/parallel-tiles.md on why
        //                  we don't build SerialTiles GPU (N× iter
        //                  cost loses to CPU's 1× iter + main-RAM
        //                  histogram).
        //
        // `Direct`:        also routed to CPU fallback here as a
        //                  defensive arm — through the production
        //                  routing this is unreachable, but a future
        //                  caller hitting HighResExporter::new for a
        //                  small image (e.g. testing) shouldn't crash.
        let solid_enabled = config.solid_strength > 0.0
            && matches!(config.render_mode, RenderMode::ThreeD);
        let hist_words: u64 = if solid_enabled { 5 } else { 4 };
        let strategy = crate::export::pick_strategy(width, height, &device.limits(), solid_enabled);
        let device_limits = device.limits();
        let total_hist_size = (width as u64) * (height as u64) * hist_words * 4;

        let mut tile_histograms_buffer: Option<Buffer> = None;
        let mut tile_layout: Option<TileLayout> = None;

        // `max_buffer_size` is what the DRIVER permits for one buffer —
        // modern drivers report values far beyond physically-free VRAM,
        // and an allocation that doesn't fit is a FATAL wgpu OOM
        // (field-reported: 19200×10800 solid = a 3.9 GB histogram
        // panicking in-app). Cap the GPU-tiles histogram at a
        // conservative budget; anything larger takes the CPU-histogram
        // path, which is slower but memory-bounded.
        const GPU_HISTOGRAM_BUDGET: u64 = 2_560 * 1024 * 1024;
        match strategy {
            crate::export::RenderStrategy::ParallelTiles { tiles_y, tile_height, .. } => {
                if total_hist_size <= (device_limits.max_buffer_size as u64).min(GPU_HISTOGRAM_BUDGET) {
                    log::info!(
                        "High-res export: GPU accumulate (ParallelTiles) — {}x{}, \
                         {} tile{} of {} rows × {} cols, {} MB total",
                        width, height,
                        tiles_y, if tiles_y == 1 { "" } else { "s" },
                        tile_height, width,
                        total_hist_size / (1024 * 1024),
                    );
                    tile_histograms_buffer = Some(device.create_buffer(&BufferDescriptor {
                        label: Some("Export Concatenated Tile Histograms"),
                        size: total_hist_size,
                        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }));
                    tile_layout = Some(TileLayout {
                        num_tiles: tiles_y,
                        tile_height,
                        image_width: width,
                        image_height: height,
                        words_per_pixel: hist_words as u32,
                    });
                } else {
                    log::info!(
                        "High-res export: CPU accumulate fallback ({}x{}, \
                         total {} MB > GPU histogram cap {} MB — bounded-memory path)",
                        width, height,
                        total_hist_size / (1024 * 1024),
                        (device_limits.max_buffer_size as u64).min(GPU_HISTOGRAM_BUDGET) / (1024 * 1024),
                    );
                }
            }
            crate::export::RenderStrategy::SerialTiles { .. } => {
                log::info!(
                    "High-res export: CPU accumulate fallback ({}x{}, strategy=SerialTiles — \
                     CPU path is faster than SerialTiles GPU per parallel-tiles.md)",
                    width, height,
                );
            }
            crate::export::RenderStrategy::Direct => {
                // The full histogram fits one binding. Reached when an in-app
                // "long render" export is routed here for background progress
                // without freezing (the synchronous FlameRenderer path is
                // reserved for quick renders that fit one binding). Render it
                // as a single GPU tile: far leaner on the export device than
                // FlameRenderer's full-res path (Rgba16Float accumulator, no
                // path-tracking buffers), so it fits alongside the live app's
                // device where render() would OOM. Degenerate ParallelTiles
                // (one tile spanning the whole image).
                if total_hist_size <= device_limits.max_buffer_size as u64 {
                    log::info!(
                        "High-res export: GPU accumulate (single tile) — {}x{}, {} MB total",
                        width, height,
                        total_hist_size / (1024 * 1024),
                    );
                    tile_histograms_buffer = Some(device.create_buffer(&BufferDescriptor {
                        label: Some("Export Single-Tile Histogram"),
                        size: total_hist_size,
                        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }));
                    tile_layout = Some(TileLayout {
                        num_tiles: 1,
                        tile_height: height,
                        image_width: width,
                        image_height: height,
                        words_per_pixel: hist_words as u32,
                    });
                } else {
                    log::warn!(
                        "High-res export: Direct strategy at {}x{} but {} MB > max_buffer — CPU fallback.",
                        width, height, total_hist_size / (1024 * 1024),
                    );
                }
            }
        }

        // ===== Create tonemap pipeline for GPU tonemapping =====
        // Unified with the interactive renderer's tonemap shader so
        // there's a single source of truth for tonemap math. The
        // interactive shader (tonemap.wgsl) has PathMap color overrides
        // and Levels logic that the export path used to silently
        // skip — meaning >binding-size renders produced different
        // images than sub-binding-size renders for the same config.
        // Now both paths run the same shader; path_buffer is bound to
        // the dummy buffer in export (PathMap color mode would produce
        // garbage there, but it was already broken for that case since
        // the compute pass doesn't track paths in export).
        let tonemap_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Export Tonemap Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/tonemap.wgsl").into()),
        });

        // Tonemap bind group layout (matches FlamePipelines exactly —
        // bindings 0-4 are the basic tonemap inputs; 5-7 are the
        // path/palette bindings that PathMap color mode needs).
        let tonemap_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Export Tonemap Bind Group Layout"),
            entries: &[
                // Accumulation texture (point-fetched via textureLoad). Rgba32Float
                // so the raw per-pixel density (a hit count that can reach
                // millions at high iteration counts) survives — Rgba16Float caps
                // at 65504 and dense pixels overflowed to inf → black "holes".
                // Rgba32Float is unfilterable (no FLOAT32_FILTERABLE feature), but
                // the tonemap uses textureLoad, so filtering isn't needed; matches
                // the FlameRenderer path.
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
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
                // Path buffer (storage, read-only — PathMap mode only)
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Palette texture
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Palette sampler
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let tonemap_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Export Tonemap Pipeline Layout"),
            bind_group_layouts: &[Some(&tonemap_bind_group_layout)],
            immediate_size: 0,
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
            multiview_mask: None,
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

        // Built before the struct literal moves `device` into the field.
        let shade_pass = if solid_enabled && config.solid_shading.active() {
            Some(crate::renderer::shade_pass::ShadePass::new_pipeline_only(&device))
        } else {
            None
        };

        // ── Light-space shadow maps (Stage 2) ──
        let shadow_cam_rows = crate::renderer::shade_pass::effective_camera_rows(
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_bank,
        );
        let shadow_cam_pos = [config.camera_x, config.camera_y, config.camera_z];
        let mut shadow_dirs = [[0.0f32; 4]; 4];
        let mut any_light = false;
        for (i, l) in config.solid_shading.lights.iter().enumerate().take(4) {
            if !(l.enabled && l.intensity > 0.0) {
                continue;
            }
            any_light = true;
            let az = l.azimuth.to_radians();
            let el = l.elevation.to_radians();
            let c = [el.cos() * az.sin(), el.sin(), el.cos() * az.cos()];
            let mut w = [0.0f32; 3];
            for k in 0..3 {
                w[k] = c[0] * shadow_cam_rows[0][k]
                    + c[1] * shadow_cam_rows[1][k]
                    + c[2] * shadow_cam_rows[2][k];
            }
            shadow_dirs[i] = [w[0], w[1], w[2], 1.0];
        }
        let shadow_wanted =
            solid_enabled && config.solid_shading.shadow_strength > 0.0 && any_light;
        // View-derived fit (no bounds measurement on this path): center
        // on the world point at screen center, radius = the view fit.
        let aspect = width.max(height).max(1) as f32 / width.min(height).max(1) as f32;
        let mut fit_center = shadow_cam_pos;
        for k in 0..3 {
            fit_center[k] += config.pan_x * shadow_cam_rows[0][k]
                + config.pan_y * shadow_cam_rows[1][k];
        }
        let shadow_fit = (
            fit_center,
            (4.0 * aspect / config.zoom.max(1e-6)).clamp(1e-3, 1e6),
        );
        // Maps only on the GPU-tiled path (the CPU-histogram fallback
        // has no scatter dispatch to write them, v1).
        let shadow_maps_buffer = if shadow_wanted && tile_histograms_buffer.is_some() {
            Some(device.create_buffer(&BufferDescriptor {
                label: Some("Export Shadow Maps"),
                // + 8 bytes: occlusion-survival counter pair at the tail.
                size: 4 * 1024 * 1024 * 4 + 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }))
        } else {
            if shadow_wanted {
                log::warn!("High-res export: shadow maps unavailable on the CPU-histogram path");
            }
            None
        };
        // Also the occlusion-counter home when shadow maps are absent —
        // hence COPY_SRC.
        let shadow_dummy_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Export Shadow Dummy"),
            size: 16,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
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
            subflame_metadata_buffer,
            dummy_path_buffer,
            dummy_path_filter_buffer,
            blur_splat_buffer,
            blur_convolve_params_buffer,
            blur_setup,
            palette_texture,
            palette_sampler,
            compute_pipeline,
            bind_group_layout,
            accumulate_pipeline,
            accumulate_bind_group_layout,
            accumulate_params_buffer,
            tile_histograms_buffer,
            tile_layout,
            init_pipeline,
            init_bind_group_layout,
            init_pair_count,
            tonemap_pipeline,
            tonemap_bind_group_layout,
            tonemap_params_buffer,
            curve_lut_texture,
            curve_lut_sampler,
            accumulation_sampler,
            render_mode: config.render_mode,
            emit_mult,
            samples_per_dispatch,
            solid_enabled,
            solid_strength: config.solid_strength,
            surface_thickness: config.surface_thickness,
            shade_pass,
            shade_depth_buffer: None,
            shadow_maps_buffer,
            shadow_dummy_buffer,
            cpu_occ_stats: None,
            shadow_fit,
            shadow_dirs,
            shadow_cam_rows,
            shadow_cam_pos,
            shadow_view: (
                config.zoom,
                config.rotation,
                config.pan_x,
                config.pan_y,
                config.perspective_strength,
            ),
            solid_density_fraction: 1.0,
            iterations_per_thread,
        })
    }

    /// Export to RGBA pixel data
    pub async fn export(
        &mut self,
        config: &FractalConfig,
        total_iterations: u64,
        transparent: bool,
        premultiplied: bool,
        reporter: &mut dyn crate::export::ExportReporter,
    ) -> Result<Vec<u8>, String> {
        // Create CPU histogram. The GPU accumulate path leaves this
        // empty until the final readback; the CPU path fills it
        // incrementally per-dispatch.
        let num_pixels = (self.width as usize) * (self.height as usize);
        let mut histogram: Vec<HistogramPixel> = vec![HistogramPixel::default(); num_pixels];
        // Solid rendering (CPU accumulate path): per-pixel nearest depth,
        // persisted across dispatches like the GPU depth regions. The CPU
        // path is sequential per row, so its gating is deterministic.
        let mut depth_near: Vec<f32> = if self.solid_enabled && self.tile_histograms_buffer.is_none() {
            vec![f32::INFINITY; num_pixels]
        } else {
            Vec::new()
        };

        // GPU accumulate path uses this scale to widen the [0,1] sample
        // RGB into u32 atomic-add precision; we divide back by it on
        // readback so HistogramPixel's units stay equivalent to the CPU
        // path's (r,g,b ∈ [0,n], count = sample count). Hardcoded — was
        // a user-tunable slider, removed (see shader const comment).
        let color_scale_f: f32 = 100.0;
        let use_gpu_accumulate = self.tile_histograms_buffer.is_some();

        // Pre-zero the concatenated tile-histogram buffer. The per-tile
        // accumulate dispatches below treat their sub-range bindings as
        // independent histograms, so the whole buffer needs to start at
        // zero exactly once per export.
        if let Some(hist) = self.tile_histograms_buffer.as_ref() {
            let mut clear_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Export GPU Histogram Clear"),
            });
            clear_encoder.clear_buffer(hist, 0, None);
            self.queue.submit(std::iter::once(clear_encoder.finish()));
        }

        // Analytic-blur low-res splat buffer accumulates mean-splats across ALL
        // dispatches (convolved + upscaled once at the end), so clear it once.
        if self.blur_setup.is_some() {
            let mut e = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Export Blur Splat Clear"),
            });
            e.clear_buffer(&self.blur_splat_buffer, 0, None);
            self.queue.submit(std::iter::once(e.finish()));
        }

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
                BindGroupEntry {
                    binding: 12,
                    resource: self.subflame_metadata_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 13,
                    resource: self.blur_splat_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 14,
                    resource: self.blur_convolve_params_buffer.as_entire_binding(),
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

        // Accumulate bind groups. Created once outside the loop —
        // buffer handles are long-lived and bind groups are reusable
        // across iterate dispatches.
        //
        // ParallelTiles path: one bind group per tile, each binding
        // a sub-range of the concatenated buffer at the tile's byte
        // offset with size equal to the tile's pixel-count × 16
        // bytes. The shader's bound_x/bound_y/bound_width/bound_height
        // uniform filters out samples that don't belong to the
        // currently-bound tile, and `local_pixel_idx` indexes into
        // the sub-range as if it were a fresh tile-sized buffer.
        // For `num_tiles == 1` this degenerates to a single
        // full-image bind group — the path that used to live behind
        // the dedicated Direct branch.
        //
        // CPU fallback path: empty vec; the dispatch loop reads back
        // samples and bins them on the host instead.
        let tile_accumulate_bind_groups: Vec<BindGroup> = match (
            self.tile_histograms_buffer.as_ref(),
            self.tile_layout,
        ) {
            (Some(hist), Some(layout)) => (0..layout.num_tiles)
                .map(|tile_idx| {
                    let offset = layout.tile_byte_offset(tile_idx);
                    let tile_pixels = (layout.image_width as u64)
                        * (layout.tile_actual_height(tile_idx) as u64);
                    let size = tile_pixels * (layout.words_per_pixel as u64) * 4;
                    self.device.create_bind_group(&BindGroupDescriptor {
                        label: Some("Export Accumulate Bind Group (Tile)"),
                        layout: &self.accumulate_bind_group_layout,
                        entries: &[
                            BindGroupEntry {
                                binding: 0,
                                resource: self.sample_buffer.as_entire_binding(),
                            },
                            BindGroupEntry {
                                binding: 1,
                                resource: self.accumulate_params_buffer.as_entire_binding(),
                            },
                            BindGroupEntry {
                                binding: 2,
                                resource: BindingResource::Buffer(BufferBinding {
                                    buffer: hist,
                                    offset,
                                    size: std::num::NonZeroU64::new(size),
                                }),
                            },
                            BindGroupEntry {
                                binding: 3,
                                resource: self
                                    .shadow_maps_buffer
                                    .as_ref()
                                    .unwrap_or(&self.shadow_dummy_buffer)
                                    .as_entire_binding(),
                            },
                        ],
                    })
                })
                .collect(),
            _ => Vec::new(),
        };

        // Create readback buffer for samples (CPU path only). On the
        // GPU path the samples never leave the GPU; the accumulate
        // dispatch consumes them in place.
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

        // Dispatch in FlameRenderer-sized passes (render.rs NUM_WORKGROUPS = 128
        // workgroups × 64 threads × ipt) so HR dispatches the SAME total
        // iteration count FR does: both round the target up to a whole pass,
        // i.e. ceil(target / pass) × pass. The tonemap normalizes brightness by
        // sample_density = total_iters / pixels and is only *approximately*
        // iteration-invariant at low density, so a coarser HR overshoot (its
        // buffer-derived dispatch was 2× FR's pass) made HR systematically
        // dimmer than FR at low spp — the gap shrank as 1/spp, exactly tracking
        // the overshoot ratio. The sample buffer (sized for the larger
        // calculate_workgroups count) comfortably holds one 128-workgroup pass.
        const FR_PASS_WORKGROUPS: u32 = 128; // mirrors render.rs NUM_WORKGROUPS
        let workgroups_per_dispatch =
            FR_PASS_WORKGROUPS.min(Self::calculate_workgroups(self.iterations_per_thread, self.emit_mult) as u32);
        let iterations_per_dispatch = workgroups_per_dispatch as u64
            * Self::THREADS_PER_WORKGROUP
            * self.iterations_per_thread as u64;
        let num_dispatches =
            (total_iterations + iterations_per_dispatch - 1) / iterations_per_dispatch;

        let mut total_samples_accumulated = 0u64;

        for dispatch in 0..num_dispatches {
            // Rendering is the bulk of the work; map it to 0..0.9 and reserve
            // the tail for accumulate/tonemap/encode.
            let frac = (dispatch + 1) as f32 / num_dispatches as f32 * 0.9;
            reporter.progress(frac, &format!("Rendering · pass {}/{}", dispatch + 1, num_dispatches));

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
                    // Never reached: escape exports go through the
                    // fragment path, not the tiled chaos-game exporter.
                    RenderMode::Escape => 0,
                },
                splat_size: 1.0,
                zoom: config.zoom,
                pan_x: config.pan_x,
                pan_y: config.pan_y,
                rotation: config.rotation,
                speed_factor: config.speed_factor,
                perspective_strength: config.perspective_strength,
                depth_density_compensation: config.depth_density_compensation,
                far_density_fade: config.far_density_fade,
                far_density_fade_start: config.far_density_fade_start,
                // Solid rendering unsupported on the tiled/sample-emit
                // export path (Phase 0 scope: direct-histogram only) —
                // constants gate SOLID off there, so these are inert.
                solid_strength: 0.0,
                surface_thickness: crate::config::DEFAULT_SURFACE_THICKNESS,
                depth_prime: 0,
                camera_rotation_x: config.camera_rotation_x,
                camera_rotation_y: config.camera_rotation_y,
                camera_bank: config.camera_bank,

                camera_x: config.camera_x,

                camera_y: config.camera_y,
                camera_z: config.camera_z,
                dof_focus_distance: config.dof_focus_distance,
                dof_blur_strength: config.dof_blur_strength,
                fog_strength: if config.fog_strength > 0.0
                    && config.solid_shading.active()
                    && matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD)
                {
                    // The shade pass owns fog (post-lighting).
                    0.0
                } else {
                    config.fog_strength
                },
                fog_start: config.fog_start,
                bits_per_transform: crate::gpu::buffers::bits_per_transform(config.flame.transforms.len() as u32),
                path_map_style: config.path_map_style as u32,
                path_capture_mode: config.path_capture_mode as u32,
                path_tracking_mode: config.path_tracking_mode as u32,
                num_path_filters: 0, // Path filters not supported in export mode
                min_suffix_filter_length: 0,
                background_r: config.background_color[0],
                background_g: config.background_color[1],
                background_b: config.background_color[2],
                post_symmetry: (&config.flame.post_symmetry).into(),
                // No shadow maps on the tiled sample-emit path (v1).
                shadow_center_x: 0.0,
                shadow_center_y: 0.0,
                shadow_center_z: 0.0,
                shadow_radius: 1.0,
                shadow_count: 0,
                _pad_shadow: [0; 3],
                shadow_dirs: [[0.0; 4]; 4],
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
                if !tile_accumulate_bind_groups.is_empty() {
                    // GPU ParallelTiles path: the histogram is sliced
                    // into N tile regions (N=1 for sub-binding sizes,
                    // N>1 above). The same sample stream is dispatched
                    // through the accumulate shader once per tile with
                    // `bound_x/y/width/height` set to the tile's region.
                    // Samples landing outside the bound region get
                    // rejected by the shader's bounds check, so each
                    // dispatch only writes to its own tile. No sample
                    // readback — the data stays GPU-resident until the
                    // final histogram readback below.
                    //
                    // Per-tile submit (rather than one encoder with N
                    // passes) is needed because `queue.write_buffer`
                    // calls between encoded compute passes coalesce —
                    // every dispatch in a single submit would see the
                    // *last* params write. Separate submits sequence the
                    // write → dispatch pairs in queue order.
                    //
                    // Wall time at N>1: a constant N× factor on the
                    // accumulate-pass time only — iteration is unchanged.
                    // Accumulate is much cheaper than iteration so this
                    // is fine in practice; the alternative (multi-binding
                    // shader templated for N tiles) caps at
                    // max_storage_buffers_per_shader_stage (often 8),
                    // doesn't scale, and adds shader-codegen complexity.
                    let layout = self.tile_layout.expect("tile_layout present when bind groups are");
                    for (tile_idx, tile_bg) in tile_accumulate_bind_groups.iter().enumerate() {
                        let tile_idx_u32 = tile_idx as u32;
                        let acc_params = crate::export::accumulate::AccumulateParams {
                            bound_x: 0,
                            bound_y: layout.tile_y_origin(tile_idx_u32),
                            bound_width: layout.image_width,
                            bound_height: layout.tile_actual_height(tile_idx_u32),
                            sample_count,
                            color_scale: color_scale_f,
                            solid_strength: if self.solid_enabled { self.solid_strength } else { 0.0 },
                            surface_thickness: self.surface_thickness,
                            // First dispatch primes the depth region (depth
                            // only, nothing plotted) — mirrors the interactive
                            // renderer's post-reset priming.
                            depth_prime: u32::from(self.solid_enabled && dispatch == 0),
                            _pad0: 0,
                            _pad1: 0,
                            _pad2: 0,
                            full_width: self.width,
                            full_height: self.height,
                            shadow_count: if self.shadow_maps_buffer.is_some() { 4 } else { 0 },
                            occ_stats_offset: if self.shadow_maps_buffer.is_some() { 4 * 1024 * 1024 } else { 0 },
                            zoom: self.shadow_view.0,
                            rotation: self.shadow_view.1,
                            pan_x: self.shadow_view.2,
                            pan_y: self.shadow_view.3,
                            persp: self.shadow_view.4,
                            _pad4: 0.0,
                            _pad5: 0.0,
                            _pad6: 0.0,
                            cam_row0: [self.shadow_cam_rows[0][0], self.shadow_cam_rows[0][1], self.shadow_cam_rows[0][2], 0.0],
                            cam_row1: [self.shadow_cam_rows[1][0], self.shadow_cam_rows[1][1], self.shadow_cam_rows[1][2], 0.0],
                            cam_row2: [self.shadow_cam_rows[2][0], self.shadow_cam_rows[2][1], self.shadow_cam_rows[2][2], 0.0],
                            cam_pos: [self.shadow_cam_pos[0], self.shadow_cam_pos[1], self.shadow_cam_pos[2], 0.0],
                            shadow_fit: [self.shadow_fit.0[0], self.shadow_fit.0[1], self.shadow_fit.0[2], self.shadow_fit.1],
                            shadow_dirs: self.shadow_dirs,
                        };
                        self.queue.write_buffer(
                            &self.accumulate_params_buffer,
                            0,
                            bytemuck::bytes_of(&acc_params),
                        );

                        let mut acc_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                            label: Some("Export Accumulate Encoder (Tile)"),
                        });
                        {
                            let mut acc_pass = acc_encoder.begin_compute_pass(&ComputePassDescriptor {
                                label: Some("Export Accumulate Pass (Tile)"),
                                timestamp_writes: None,
                            });
                            acc_pass.set_pipeline(&self.accumulate_pipeline);
                            acc_pass.set_bind_group(0, tile_bg, &[]);
                            let groups = crate::export::accumulate::accumulate_dispatch_groups(sample_count);
                            acc_pass.dispatch_workgroups(groups, 1, 1);
                        }
                        self.queue.submit(std::iter::once(acc_encoder.finish()));
                    }

                    total_samples_accumulated += sample_count as u64;
                } else {
                    // CPU fallback: sample buffer is read back to host
                    // and binned into the per-row CPU histogram.
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

                    let samples = self
                        .read_samples(&readback_buffer, sample_count.min(self.samples_per_dispatch as u32))
                        .await?;

                    let width = self.width as i32;
                    let height = self.height as i32;
                    let width_usize = self.width as usize;
                    let height_usize = self.height as usize;

                    let mut row_samples: Vec<Vec<&Sample>> = vec![Vec::new(); height_usize];
                    for sample in &samples {
                        let y = sample.y as i32;
                        if y >= 0 && y < height {
                            row_samples[y as usize].push(sample);
                        }
                    }

                    let cs = color_scale_f as f64;
                    if self.solid_enabled {
                        // Solid rendering: nearest-depth gating, mirroring the
                        // GPU scatter (update depth FIRST so a pixel's own
                        // nearest sample always passes; prime dispatch records
                        // depth only). Rows are independent, samples within a
                        // row process in readback order — deterministic.
                        let prime = dispatch == 0;
                        let solid_strength = self.solid_strength;
                        let surface_thickness = self.surface_thickness;
                        // Occlusion-survival counters (exact on this
                        // path): Σocc and Σ1 over plotted samples, for
                        // the occlusion-only brightness renorm.
                        let occ_num = std::sync::atomic::AtomicU64::new(0);
                        let occ_den = std::sync::atomic::AtomicU64::new(0);
                        histogram
                            .par_chunks_mut(width_usize)
                            .zip(depth_near.par_chunks_mut(width_usize))
                            .enumerate()
                            .for_each(|(row_idx, (row_pixels, row_depth))| {
                                let mut row_occ: u64 = 0;
                                let mut row_n: u64 = 0;
                                for sample in &row_samples[row_idx] {
                                    let x = sample.x as i32;
                                    if x >= 0 && x < width {
                                        let xi = x as usize;
                                        if sample.depth < row_depth[xi] {
                                            row_depth[xi] = sample.depth;
                                        }
                                        if prime {
                                            continue;
                                        }
                                        let mut w = sample.weight;
                                        let mut occ = 1.0f32;
                                        if sample.depth > row_depth[xi] + surface_thickness {
                                            occ = 1.0 - solid_strength;
                                            w *= occ;
                                        }
                                        row_occ += (occ * 16.0).round() as u64;
                                        row_n += 16;
                                        if w <= 0.0 {
                                            continue;
                                        }
                                        let pixel = &mut row_pixels[xi];
                                        let ws = color_scale_f * w;
                                        pixel.r += (sample.r.clamp(0.0, 1.0) * ws).floor() as f64 / cs;
                                        pixel.g += (sample.g.clamp(0.0, 1.0) * ws).floor() as f64 / cs;
                                        pixel.b += (sample.b.clamp(0.0, 1.0) * ws).floor() as f64 / cs;
                                        pixel.count += ws.floor() as f64 / cs;
                                    }
                                }
                                occ_num.fetch_add(row_occ, std::sync::atomic::Ordering::Relaxed);
                                occ_den.fetch_add(row_n, std::sync::atomic::Ordering::Relaxed);
                            });
                        let num = occ_num.load(std::sync::atomic::Ordering::Relaxed) as f64;
                        let den = occ_den.load(std::sync::atomic::Ordering::Relaxed) as f64;
                        let (pn, pd) = self.cpu_occ_stats.unwrap_or((0.0, 0.0));
                        self.cpu_occ_stats = Some((pn + num, pd + den));
                    } else {
                    histogram
                        .par_chunks_mut(width_usize)
                        .enumerate()
                        .for_each(|(row_idx, row_pixels)| {
                            for sample in &row_samples[row_idx] {
                                let x = sample.x as i32;
                                if x >= 0 && x < width {
                                    let pixel = &mut row_pixels[x as usize];
                                    // Replicate the GPU scatter
                                    // (accumulate_samples.wgsl) EXACTLY so the
                                    // CPU/SerialTiles path matches ParallelTiles
                                    // and the in-app/FlameRenderer paths: clamp,
                                    // scale by color_scale·weight, then TRUNCATE
                                    // per sample (the GPU's `u32(...)`), then
                                    // divide back by color_scale. Binning exact
                                    // f64 here instead made the series export
                                    // subtly brighter / more saturated than the
                                    // (truncating) GPU paths.
                                    let ws = color_scale_f * sample.weight;
                                    pixel.r += (sample.r.clamp(0.0, 1.0) * ws).floor() as f64 / cs;
                                    pixel.g += (sample.g.clamp(0.0, 1.0) * ws).floor() as f64 / cs;
                                    pixel.b += (sample.b.clamp(0.0, 1.0) * ws).floor() as f64 / cs;
                                    pixel.count += ws.floor() as f64 / cs;
                                }
                            }
                        });
                    }

                    total_samples_accumulated += samples.len() as u64;
                }
            }
        }

        // GPU accumulate path: read the histogram back once and
        // convert to the same Vec<HistogramPixel> shape the CPU path
        // produced, so the existing tonemap_gpu code path runs
        // unchanged.
        if use_gpu_accumulate {
            histogram = self.read_gpu_histogram(color_scale_f).await?;
        }

        // Analytic-blur fold: read the low-res splat buffer back, convolve +
        // upscale it (the SAME bicubic+dither math as blur_upscale.wgsl) and
        // ADD into the CPU histogram — serving BOTH tiled strategies through
        // this one inject point. See docs/projects/analytic-blur-buffer.md.
        if self.blur_setup.is_some() {
            self.fold_analytic_blur(&mut histogram, color_scale_f).await?;
        }

        // Solid shading: gather the full-image depth (per-tile regions /
        // CPU depth vec) BEFORE the tile buffer is freed below.
        self.prepare_shade_depth(&depth_near);

        // Free the GPU tile histogram (up to ~2 GB) now that it's all in the
        // CPU `histogram` Vec — the tonemap + color-effect passes below
        // allocate their own full-res textures, so releasing this first gives
        // them headroom at high resolution. Drop + poll so wgpu actually
        // reclaims the GPU memory before those allocs (a plain drop is
        // deferred).
        self.tile_histograms_buffer = None;
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });

        reporter.progress(0.92, &format!("Accumulating {} samples…", total_samples_accumulated));
        reporter.progress(0.97, "Tonemapping…");

        // Tonemap histogram to RGBA using GPU. The actual iteration
        // count is the loop's ceiling-rounded `num_dispatches × per-dispatch`,
        // not the user-passed target — feed that to the scale-invariant
        // sample_density formula (Phase 8a).
        let total_iters_dispatched = num_dispatches * iterations_per_dispatch;

        // Solid brightness renormalization (exact): OCCLUSION-ONLY
        // survival fraction from the dedicated counters — never the
        // accumulated density, which folds artistic per-sample weights
        // (far-fade, depth-density compensation) into the "culled"
        // fraction and makes those dials shift global brightness.
        self.solid_density_fraction = if self.solid_enabled && self.solid_strength > 0.0 {
            match self.read_occ_stats().await {
                Some((num, den)) if den > 0.0 => {
                    let f = ((num / den) as f32).clamp(0.005, 1.0);
                    log::info!("High-res export: solid brightness renorm fraction {:.4} (occlusion-only)", f);
                    f
                }
                _ => {
                    log::warn!("High-res export: occlusion counters empty — brightness renorm skipped");
                    1.0
                }
            }
        } else {
            1.0
        };

        let pixels = self.tonemap_gpu(&histogram, config, transparent, premultiplied, total_iters_dispatched).await?;

        reporter.progress(1.0, "Encoding…");

        Ok(pixels)
    }

    /// Gather the full-image encoded depth for the shade pass — from the
    /// per-tile depth regions (GPU path) or the CPU depth vec — BEFORE the
    /// tile histogram buffer is freed. Disables shading (with a warning)
    /// when no depth source exists or the buffer exceeds one binding.
    fn prepare_shade_depth(&mut self, cpu_depth: &[f32]) {
        self.shade_depth_buffer = None;
        if self.shade_pass.is_none() {
            return;
        }
        let map_bytes: u64 = if self.shadow_maps_buffer.is_some() {
            4 * 1024 * 1024 * 4
        } else {
            0
        };
        let bytes = (self.width as u64) * (self.height as u64) * 4 + map_bytes;
        if bytes > self.device.limits().max_storage_buffer_binding_size as u64 {
            log::warn!(
                "High-res export: skipping lighting — depth buffer {} MB exceeds one storage binding",
                bytes / (1024 * 1024)
            );
            self.shade_pass = None;
            return;
        }
        let buf = self.device.create_buffer(&BufferDescriptor {
            label: Some("Export Shade Depth"),
            size: bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Shadow maps ride the tail (word offset W·H) so the shade
        // pass reaches them through the same binding as the depth.
        if let Some(maps) = self.shadow_maps_buffer.as_ref() {
            let mut enc = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Export Shadow Map Gather"),
            });
            enc.copy_buffer_to_buffer(
                maps,
                0,
                &buf,
                (self.width as u64) * (self.height as u64) * 4,
                map_bytes,
            );
            self.queue.submit(std::iter::once(enc.finish()));
        }
        match (self.tile_histograms_buffer.as_ref(), self.tile_layout) {
            (Some(hist), Some(layout)) if layout.words_per_pixel > 4 => {
                let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Export Shade Depth Gather"),
                });
                let mut dst: u64 = 0;
                for tile_idx in 0..layout.num_tiles {
                    let rows = layout.tile_actual_height(tile_idx) as u64;
                    let tile_pixels = (layout.image_width as u64) * rows;
                    let src = layout.tile_byte_offset(tile_idx) + tile_pixels * 16;
                    encoder.copy_buffer_to_buffer(hist, src, &buf, dst, tile_pixels * 4);
                    dst += tile_pixels * 4;
                }
                self.queue.submit(std::iter::once(encoder.finish()));
            }
            _ if !cpu_depth.is_empty() => {
                // CPU accumulate path: encode world depths into the shader's
                // inverted ordered-float format (INFINITY = empty = 0).
                let words: Vec<u32> = cpu_depth
                    .par_iter()
                    .map(|d| {
                        if !d.is_finite() {
                            0u32
                        } else {
                            let b = d.to_bits();
                            let ord = if b & 0x8000_0000 != 0 { !b } else { b | 0x8000_0000 };
                            !ord
                        }
                    })
                    .collect();
                self.queue.write_buffer(&buf, 0, bytemuck::cast_slice(&words));
            }
            _ => {
                log::warn!("High-res export: no depth source for lighting — rendering unlit");
                self.shade_pass = None;
                return;
            }
        }
        self.shade_depth_buffer = Some(buf);
    }

    /// Occlusion-survival counter pair: GPU paths read the 2 words at
    /// the tail of the shadow-maps buffer (or the stand-in); the CPU
    /// path sums exactly during accumulation (`cpu_occ_stats`).
    async fn read_occ_stats(&self) -> Option<(f64, f64)> {
        if let Some((num, den)) = self.cpu_occ_stats {
            return Some((num, den));
        }
        let (buf, word_off): (&Buffer, u64) = match &self.shadow_maps_buffer {
            Some(b) => (b, 4 * 1024 * 1024),
            None => (&self.shadow_dummy_buffer, 0),
        };
        let staging = self.device.create_buffer(&BufferDescriptor {
            label: Some("Occ Stats Staging"),
            size: 8,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Occ Stats Readback"),
        });
        enc.copy_buffer_to_buffer(buf, word_off * 4, &staging, 0, 8);
        self.queue.submit(std::iter::once(enc.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        slice.map_async(MapMode::Read, move |result| {
            tx.send(result).ok();
        });
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });
        rx.await.ok()?.ok()?;
        let data = slice.get_mapped_range();
        let num = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let den = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        drop(data);
        staging.unmap();
        Some((f64::from(num), f64::from(den)))
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

    /// Read the GPU histogram buffer back to host and convert to the
    /// `Vec<HistogramPixel>` shape the existing tonemap_gpu path
    /// expects. Layout in the GPU buffer matches what
    /// accumulate_samples.wgsl writes: 4× u32 per pixel —
    /// `[R_sum, G_sum, B_sum, density_sum]`, each scaled by
    /// `color_scale`. We divide back by `color_scale` so the
    /// HistogramPixel units match the CPU path's
    /// (r,g,b summed in [0,n], count = sample count).
    async fn read_gpu_histogram(&self, color_scale: f32) -> Result<Vec<HistogramPixel>, String> {
        // ParallelTiles tiles are horizontal slices laid out
        // back-to-back in the concatenated buffer, which is exactly
        // the natural image row order — no stitching needed at the
        // byte level, just a single full-buffer readback.
        let hist = self
            .tile_histograms_buffer
            .as_ref()
            .ok_or_else(|| "read_gpu_histogram called without a GPU histogram buffer".to_string())?;

        let num_pixels = (self.width as usize) * (self.height as usize);
        let bytes = (num_pixels * 16) as u64;

        let readback = self.device.create_buffer(&BufferDescriptor {
            label: Some("Export GPU Histogram Readback"),
            size: bytes,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Export GPU Histogram Readback Encoder"),
        });
        match self.tile_layout {
            Some(layout) if layout.words_per_pixel > 4 => {
                // Solid rendering: each tile's span ends with its depth
                // region — gather only the RGBD words, packed contiguously
                // into the readback buffer (row order is preserved because
                // tiles are horizontal slices in image order).
                let mut dst: u64 = 0;
                for tile_idx in 0..layout.num_tiles {
                    let rgbd_bytes = (layout.image_width as u64)
                        * (layout.tile_actual_height(tile_idx) as u64)
                        * 16;
                    encoder.copy_buffer_to_buffer(
                        hist,
                        layout.tile_byte_offset(tile_idx),
                        &readback,
                        dst,
                        rgbd_bytes,
                    );
                    dst += rgbd_bytes;
                }
            }
            _ => {
                encoder.copy_buffer_to_buffer(hist, 0, &readback, 0, bytes);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        slice.map_async(MapMode::Read, move |r| {
            tx.send(r).ok();
        });
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });
        rx.await
            .map_err(|_| "GPU histogram readback receiver dropped".to_string())?
            .map_err(|e| format!("Failed to map GPU histogram: {:?}", e))?;

        let data = slice.get_mapped_range();
        let words: &[u32] = bytemuck::cast_slice(&data);
        let scale = color_scale as f64;

        // Convert in parallel — at 8K we're processing 64M pixels and
        // serial conversion measurably stretches export time.
        let pixels: Vec<HistogramPixel> = (0..num_pixels)
            .into_par_iter()
            .map(|i| {
                let base = i * 4;
                HistogramPixel {
                    r: (words[base] as f64) / scale,
                    g: (words[base + 1] as f64) / scale,
                    b: (words[base + 2] as f64) / scale,
                    count: (words[base + 3] as f64) / scale,
                }
            })
            .collect();

        drop(data);
        readback.unmap();
        Ok(pixels)
    }

    /// Read the low-res analytic-blur splat buffer back, convolve it with each
    /// slot's kernel and bicubic-upscale-add the result into the CPU histogram
    /// — the tiled-path counterpart of the FlameRenderer's GPU convolve +
    /// upscale. The math mirrors `blur_convolve.wgsl` + `blur_upscale.wgsl`
    /// (cubic B-spline, ÷D²); the GPU's stochastic-rounding dither is dropped
    /// because this f64 histogram has no quantization to dither. Adds into the
    /// histogram in its `Σcolor` / `Σsamples` units (÷ color_scale).
    async fn fold_analytic_blur(
        &self,
        histogram: &mut [HistogramPixel],
        color_scale: f32,
    ) -> Result<(), String> {
        let setup = match self.blur_setup.as_ref() {
            Some(s) => s,
            None => return Ok(()),
        };
        let lw = setup.lowres_w as usize;
        let lh = setup.lowres_h as usize;
        let d = setup.downscale as i32;
        let count = setup.meta.len();
        let plane = lw * lh;
        if count == 0 || plane == 0 {
            return Ok(());
        }

        // ---- read the low-res splat buffer back to CPU ----
        let bytes = (count * plane * 16) as u64;
        let readback = self.device.create_buffer(&BufferDescriptor {
            label: Some("Export Blur Splat Readback"),
            size: bytes,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Export Blur Splat Readback Encoder"),
        });
        encoder.copy_buffer_to_buffer(&self.blur_splat_buffer, 0, &readback, 0, bytes);
        self.queue.submit(std::iter::once(encoder.finish()));
        let slice = readback.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        slice.map_async(MapMode::Read, move |r| {
            tx.send(r).ok();
        });
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });
        rx.await
            .map_err(|_| "blur splat readback receiver dropped".to_string())?
            .map_err(|e| format!("Failed to map blur splat: {:?}", e))?;
        let data = slice.get_mapped_range();
        let splat: &[u32] = bytemuck::cast_slice(&data);

        // ---- convolve each slot's slice at low res (mirror blur_convolve.wgsl) ----
        let mut conv = vec![[0f32; 4]; count * plane];
        for s in 0..count {
            let half = setup.meta[s][0] as i32;
            let woff = setup.meta[s][1] as usize;
            let size = (2 * half + 1) as usize;
            let base = s * plane;
            let weights = &setup.weights;
            conv[base..base + plane]
                .par_iter_mut()
                .enumerate()
                .for_each(|(p, out)| {
                    let x = (p % lw) as i32;
                    let y = (p / lw) as i32;
                    let mut acc = [0f32; 4];
                    for dy in -half..=half {
                        let sy = y - dy;
                        if sy < 0 || sy >= lh as i32 {
                            continue;
                        }
                        let krow = (dy + half) as usize * size;
                        for dx in -half..=half {
                            let sx = x - dx;
                            if sx < 0 || sx >= lw as i32 {
                                continue;
                            }
                            let w = weights[woff + krow + (dx + half) as usize];
                            if w == 0.0 {
                                continue;
                            }
                            let q = (base + sy as usize * lw + sx as usize) * 4;
                            acc[0] += w * splat[q] as f32;
                            acc[1] += w * splat[q + 1] as f32;
                            acc[2] += w * splat[q + 2] as f32;
                            acc[3] += w * splat[q + 3] as f32;
                        }
                    }
                    *out = acc;
                });
        }

        // ---- bicubic upscale + add (mirror blur_upscale.wgsl; ÷D²; ÷color_scale) ----
        let w = self.width as usize;
        let df = d as f32;
        let inv = 1.0 / (df * df);
        let cs = color_scale as f64;
        let conv_ref = &conv;
        histogram.par_iter_mut().enumerate().for_each(|(p, px)| {
            let fx = (p % w) as f32;
            let fy = (p / w) as f32;
            let gx = (fx + 0.5) / df - 0.5;
            let gy = (fy + 0.5) / df - 0.5;
            let x0 = gx.floor() as i32;
            let y0 = gy.floor() as i32;
            let wx = bspline4(gx - gx.floor());
            let wy = bspline4(gy - gy.floor());
            let mut acc = [0f32; 4];
            for s in 0..count {
                let base = s * plane;
                for j in 0..4usize {
                    let yy = (y0 - 1 + j as i32).clamp(0, lh as i32 - 1) as usize;
                    let mut row = [0f32; 4];
                    for i in 0..4usize {
                        let xx = (x0 - 1 + i as i32).clamp(0, lw as i32 - 1) as usize;
                        let t = conv_ref[base + yy * lw + xx];
                        let wi = wx[i];
                        row[0] += wi * t[0];
                        row[1] += wi * t[1];
                        row[2] += wi * t[2];
                        row[3] += wi * t[3];
                    }
                    let wj = wy[j];
                    acc[0] += wj * row[0];
                    acc[1] += wj * row[1];
                    acc[2] += wj * row[2];
                    acc[3] += wj * row[3];
                }
            }
            px.r += (acc[0].max(0.0) * inv) as f64 / cs;
            px.g += (acc[1].max(0.0) * inv) as f64 / cs;
            px.b += (acc[2].max(0.0) * inv) as f64 / cs;
            px.count += (acc[3].max(0.0) * inv) as f64 / cs;
        });

        drop(data);
        readback.unmap();
        Ok(())
    }

    /// Conservative budget for the post-histogram (Phase B) GPU texture set
    /// when density effects are applied. By this point the histogram buffer has
    /// been freed (drop+poll in the render loop), so Phase B is the peak. At
    /// full resolution it allocates the accumulation texture (Rgba32Float,
    /// 16 B/px) + the density ping-pong pair (2× Rgba16Float, 16 B/px) + the
    /// tonemap output (Rgba8, 4 B/px) = 36 B/px. ~4 GB lets density effects
    /// apply through ~10K² exports; beyond that we skip them (the marginal
    /// ~16 B/px over the always-present accumulation) rather than risk OOM on
    /// limited-VRAM GPUs. Color effects and the accumulation texture are
    /// already unconditional, so this gates only the density addition.
    const DENSITY_EFFECT_PHASE_B_BUDGET_BYTES: u64 = 4 * 1024 * 1024 * 1024;

    /// Whether the Phase-B texture set with density effects fits the budget.
    fn density_effects_fit(width: u32, height: u32) -> bool {
        (width as u64) * (height as u64) * 36 <= Self::DENSITY_EFFECT_PHASE_B_BUDGET_BYTES
    }

    /// Above this full-accumulation size, the tonemap runs strip-tiled so the
    /// 16 B/px Rgba32Float accumulation texture (~2.3 GB at 12K) doesn't OOM
    /// alongside the live app's GPU device. Below it the single-shot path is one
    /// pass and fits fine. 1 GB ≈ 8000².
    const TONEMAP_TILE_THRESHOLD_BYTES: u64 = 1024 * 1024 * 1024;

    /// Per-strip accumulation-texture budget for the tiled tonemap. `strip_h` is
    /// sized so one strip (W × strip_h × 16) fits this; peak GPU is then the
    /// full output (W·H·4) + one strip accumulation + one strip output.
    const TONEMAP_STRIP_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

    /// Convert a slice of the CPU histogram (row-major `HistogramPixel`s) into
    /// Rgba32Float texel bytes: RGB = density-weighted mean colour (sum/count)
    /// ∈ [0,1], A = raw hit count. Shared by the single-shot and tiled paths.
    fn build_texture_data(slice: &[HistogramPixel]) -> Vec<u8> {
        let mut texture_data = vec![0u8; slice.len() * 16];
        texture_data
            .par_chunks_mut(16)
            .zip(slice.par_iter())
            .for_each(|(chunk, pixel)| {
                let (r, g, b, density) = if pixel.count > 0.0 {
                    (
                        (pixel.r / pixel.count) as f32,
                        (pixel.g / pixel.count) as f32,
                        (pixel.b / pixel.count) as f32,
                        pixel.count as f32,
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };
                chunk[0..4].copy_from_slice(&r.to_le_bytes());
                chunk[4..8].copy_from_slice(&g.to_le_bytes());
                chunk[8..12].copy_from_slice(&b.to_le_bytes());
                chunk[12..16].copy_from_slice(&density.to_le_bytes());
            });
        texture_data
    }

    /// Build the global `TonemapParams` for an export (full width/height). The
    /// tiled path overrides only `.height` per strip; `area`/`sample_density`
    /// stay global so brightness is identical regardless of tiling. Mirrors the
    /// single-shot Step 3 exactly.
    fn build_tonemap_params(
        &self,
        config: &FractalConfig,
        transparent: bool,
        premultiplied: bool,
        total_iterations: u64,
    ) -> TonemapParams {
        use crate::config::defaults::{PREFILTER_WHITE, BRIGHT_ADJUST};

        let zoom = config.zoom;
        let base_pixels_per_unit = (self.width.min(self.height) as f32) * 0.25;
        let apophysis_zoom = zoom.log2();
        let pixels_per_unit_zoomed = base_pixels_per_unit * 2.0_f32.powf(apophysis_zoom);
        let area = (self.width as f32 * self.height as f32) / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);

        let total_pixels = (self.width as f32) * (self.height as f32);
        let sample_density = ((total_iterations as f32) * self.solid_density_fraction
            / total_pixels.max(1.0))
            .max(1e-6);

        let tonemap_mode = match config.tonemap_mode {
            crate::scene::tonemap::ToneMapMode::Linear => 0u32,
            crate::scene::tonemap::ToneMapMode::Logarithmic => 1u32,
            crate::scene::tonemap::ToneMapMode::DensityVisualization => 2u32,
        };

        TonemapParams {
            exposure: config.exposure,
            gamma: config.gamma,
            density_scale: config.density_scale,
            tonemap_mode,
            background_color: config.background_color,
            _pad_bg: 0.0,
            use_curve: if config.use_curve { 1 } else { 0 },
            vibrancy: config.vibrancy,
            brightness: config.brightness,
            white_level: config.white_level,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: config.saturation,
            hue_shift: config.hue_shift,
            gamma_threshold: config.gamma_threshold,
            alpha_blend_low: config.alpha_blend_low,
            alpha_blend_high: config.alpha_blend_high,
            // 0 = opaque, 1 = straight-alpha reconstruction, 2 = premultiplied.
            transparent_mode: if !transparent { 0 } else if premultiplied { 2 } else { 1 },
            color_mode: config.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: config.path_map_style as u32,
            burn_in: 20,
            num_transforms: config.flame.transforms.len() as u32,
            palette_size: config.palette_size,
            levels_low: config.levels_low,
            levels_high: config.levels_high,
            levels_gamma: config.levels_gamma,
            highlight_mode: match config.highlight_mode {
                crate::scene::tonemap::HighlightMode::Clip => 0,
                crate::scene::tonemap::HighlightMode::MaxNorm => 1,
                crate::scene::tonemap::HighlightMode::Reinhard => 2,
                crate::scene::tonemap::HighlightMode::Filmic => 3,
            },
            levels_enabled: if config.levels_enabled { 1 } else { 0 },
            _pad_levels: [0; 2],
        }
    }

    /// Upload the tone-curve LUT if the flame uses one. Shared by both paths.
    fn upload_curve_lut(&self, config: &FractalConfig) {
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
                TexelCopyBufferLayout { offset: 0, bytes_per_row: None, rows_per_image: None },
                Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            );
        }
    }

    /// Create a texture, turning a GPU out-of-memory failure into a recoverable
    /// `Err` instead of wgpu's default fatal panic. On the background export
    /// thread that panic would leave the progress overlay stuck "exporting"
    /// forever; returning an error lets the caller report it and clean up. The
    /// OutOfMemory error scope captures the failure at the allocation, so we
    /// bail before the invalid texture triggers follow-on validation errors.
    /// Used for the large export textures (the realistic OOM sites).
    async fn try_create_texture(&self, desc: &TextureDescriptor<'_>) -> Result<Texture, String> {
        let scope = self.device.push_error_scope(ErrorFilter::OutOfMemory);
        let texture = self.device.create_texture(desc);
        if let Some(err) = scope.pop().await {
            return Err(format!(
                "GPU out of memory allocating '{}' ({:?}) — try a smaller export size.",
                desc.label.unwrap_or("texture"),
                err
            ));
        }
        Ok(texture)
    }

    /// Shared tonemap tail: run color effects on the tonemapped output (if any),
    /// then read it back to RGBA8. Color effects are non-local so they always
    /// run on the full output, after any tiling.
    async fn finish_output(
        &self,
        output_texture: &Texture,
        output_view: &TextureView,
        config: &FractalConfig,
    ) -> Result<Vec<u8>, String> {
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
                output_view,
                &config.color_effects,
            );
            self.queue.submit(std::iter::once(effect_encoder.finish()));
            effect_chain = Some(chain);
            ran
        } else {
            false
        };

        if color_effects_ran {
            if let Some(chain) = effect_chain.as_ref() {
                return chain.read_color_output_pixels(&self.device, &self.queue).await
                    .map(|(_, _, pixels)| pixels);
            }
        }

        // Read from the tonemap output texture.
        let bytes_per_pixel = 4u32;
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
                texture: output_texture,
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
            Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.queue.submit(std::iter::once(copy_encoder.finish()));

        let buffer_slice = readback_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| { tx.send(result).ok(); });
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });
        rx.await
            .map_err(|_| "Failed to receive tonemap readback result".to_string())?
            .map_err(|e| format!("Failed to map tonemap readback buffer: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();
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

    /// Strip-tiled tonemap for large exports (see `TONEMAP_TILE_THRESHOLD_BYTES`).
    /// Uploads + tonemaps the accumulation in horizontal strips into the full
    /// output texture, so peak VRAM is one strip instead of a full W·H·16
    /// accumulation texture. Byte-identical to the single-shot path: each strip
    /// uploads a (W × strip_h) accumulation texture and sets
    /// `tonemap_params.height = strip_h` so the shader's `uv.y * height` indexes
    /// it correctly, while width/area/sample_density stay global. Density
    /// effects (non-local) are never tiled — the caller only routes here when
    /// none run.
    async fn tonemap_gpu_tiled(
        &self,
        histogram: &[HistogramPixel],
        config: &FractalConfig,
        mut tonemap_params: TonemapParams,
    ) -> Result<Vec<u8>, String> {
        self.upload_curve_lut(config);

        // The one full-resolution GPU resource (color effects + readback need it
        // whole). COPY_DST so each strip's tonemapped rows can be copied in.
        let output_texture = self.try_create_texture(&TextureDescriptor {
            label: Some("Export Output Texture (tiled)"),
            size: Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }).await?;
        let output_view = output_texture.create_view(&TextureViewDescriptor::default());
        let curve_lut_view = self.curve_lut_texture.create_view(&TextureViewDescriptor::default());
        let palette_view = self.palette_texture.create_view(&TextureViewDescriptor::default());

        let bytes_per_row = (self.width as u64) * 16;
        let strip_h = ((Self::TONEMAP_STRIP_BUDGET_BYTES / bytes_per_row.max(1)).max(1) as u32)
            .min(self.height);
        let num_strips = (self.height + strip_h - 1) / strip_h;
        log::info!(
            "High-res export: strip-tiled tonemap {}x{} — {} strips of ≤{} rows",
            self.width, self.height, num_strips, strip_h
        );

        for s in 0..num_strips {
            let y0 = s * strip_h;
            let sh = strip_h.min(self.height - y0);

            // This strip's accumulation data (rows [y0, y0+sh)).
            let row0 = (y0 as usize) * (self.width as usize);
            let row1 = row0 + (sh as usize) * (self.width as usize);
            let strip_data = Self::build_texture_data(&histogram[row0..row1]);

            let strip_accum = self.try_create_texture(&TextureDescriptor {
                label: Some("Export Strip Accumulation"),
                size: Extent3d { width: self.width, height: sh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba32Float,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            }).await?;
            self.queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &strip_accum,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &strip_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * 16),
                    rows_per_image: Some(sh),
                },
                Extent3d { width: self.width, height: sh, depth_or_array_layers: 1 },
            );
            let strip_accum_view = strip_accum.create_view(&TextureViewDescriptor::default());

            // Solid shading per strip: full-image depth buffer + this
            // strip's albedo rows (the shade pass reads the accumulator
            // only at its own pixel, so strips shade identically to a
            // one-shot — see shade.wgsl region params).
            if y0 == 0 && config.dof_blur_strength > 0.0
                && matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD)
            {
                // A gather blur can't run per strip without aprons —
                // taps would cross strip boundaries and seam.
                log::warn!("Post-process DoF is not yet supported on the tiled export path — skipped");
            }
            let mut strip_shade: Option<(Texture, TextureView)> = None;
            if let (Some(shade), Some(depth)) = (self.shade_pass.as_ref(), self.shade_depth_buffer.as_ref()) {
                let tex = self.try_create_texture(&TextureDescriptor {
                    label: Some("Export Strip Shaded"),
                    size: Extent3d { width: self.width, height: sh, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba32Float,
                    usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                }).await?;
                let view = tex.create_view(&TextureViewDescriptor::default());
                let mut shade_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Export Strip Shade Encoder"),
                });
                shade.run_region(
                    &self.device,
                    &self.queue,
                    &mut shade_encoder,
                    &strip_accum_view,
                    depth,
                    &view,
                    &config.solid_shading,
                    config.zoom,
                    config.rotation,
                    config.pan_x,
                    config.pan_y,
                    config.perspective_strength,
                    self.surface_thickness,
                    self.width,
                    self.height,
                    0,
                    y0,
                    sh,
                    (config.camera_rotation_x, config.camera_rotation_y, config.camera_bank,
                     [config.camera_x, config.camera_y, config.camera_z]),
                    self.shadow_maps_buffer.as_ref().map(|_| {
                        (self.width * self.height, self.shadow_fit.0, self.shadow_fit.1)
                    }),
                    (config.fog_strength, config.fog_start, config.background_color),
                    0.0,
                    None,
                );
                self.queue.submit(std::iter::once(shade_encoder.finish()));
                strip_shade = Some((tex, view));
            }
            let strip_input_view: &TextureView =
                strip_shade.as_ref().map(|(_, v)| v).unwrap_or(&strip_accum_view);

            // Per-strip height: the shader's `uv.y * height` then maps onto
            // these `sh` rows. width/area/sample_density stay global.
            tonemap_params.height = sh;
            self.queue.write_buffer(&self.tonemap_params_buffer, 0, bytemuck::bytes_of(&tonemap_params));

            let strip_out = self.device.create_texture(&TextureDescriptor {
                label: Some("Export Strip Output"),
                size: Extent3d { width: self.width, height: sh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let strip_out_view = strip_out.create_view(&TextureViewDescriptor::default());

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Export Tonemap Bind Group (strip)"),
                layout: &self.tonemap_bind_group_layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: BindingResource::TextureView(strip_input_view) },
                    BindGroupEntry { binding: 1, resource: BindingResource::Sampler(&self.accumulation_sampler) },
                    BindGroupEntry { binding: 2, resource: self.tonemap_params_buffer.as_entire_binding() },
                    BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&curve_lut_view) },
                    BindGroupEntry { binding: 4, resource: BindingResource::Sampler(&self.curve_lut_sampler) },
                    BindGroupEntry { binding: 5, resource: self.dummy_path_buffer.as_entire_binding() },
                    BindGroupEntry { binding: 6, resource: BindingResource::TextureView(&palette_view) },
                    BindGroupEntry { binding: 7, resource: BindingResource::Sampler(&self.palette_sampler) },
                ],
            });

            let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Export Strip Tonemap Encoder"),
            });
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Export Strip Tonemap Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &strip_out_view,
                        resolve_target: None,
                        ops: Operations { load: LoadOp::Clear(Color::BLACK), store: StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                render_pass.set_pipeline(&self.tonemap_pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
            // Copy this strip's tonemapped rows into the full output at y0.
            encoder.copy_texture_to_texture(
                TexelCopyTextureInfo {
                    texture: &strip_out,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                TexelCopyTextureInfo {
                    texture: &output_texture,
                    mip_level: 0,
                    origin: Origin3d { x: 0, y: y0, z: 0 },
                    aspect: TextureAspect::All,
                },
                Extent3d { width: self.width, height: sh, depth_or_array_layers: 1 },
            );
            self.queue.submit(std::iter::once(encoder.finish()));
            // Serialize so this strip's GPU work finishes and its textures free
            // (on the drop below) before the next strip allocates — peak GPU
            // stays at one strip's worth.
            let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });
        }

        self.finish_output(&output_texture, &output_view, config).await
    }

    /// GPU tonemap: histogram → RGBA8 pixels via the tonemap shader.
    /// `total_iterations` feeds the scale-invariant sample_density formula.
    /// Large, density-effect-free exports route to `tonemap_gpu_tiled` to bound
    /// peak VRAM; everything else uses this single-shot path.
    async fn tonemap_gpu(
        &self,
        histogram: &[HistogramPixel],
        config: &FractalConfig,
        transparent: bool,
        premultiplied: bool,
        total_iterations: u64,
    ) -> Result<Vec<u8>, String> {
        use crate::config::defaults::{PREFILTER_WHITE, BRIGHT_ADJUST};

        // Route large, density-free exports to the strip-tiled tonemap to bound
        // peak VRAM: a full W·H·16 accumulation texture (~2.3 GB at 12K) OOMs
        // alongside the live app's GPU device. Density effects are non-local
        // (can't be strip-tiled) but are already skipped at the sizes that route
        // here (density_effects_fit), so they never collide. Everything else
        // uses the single-shot path below, unchanged.
        if (self.width as u64) * (self.height as u64) * 16 > Self::TONEMAP_TILE_THRESHOLD_BYTES
            && !(EffectChainRunner::has_enabled_effects(&config.density_effects)
                && Self::density_effects_fit(self.width, self.height))
        {
            let params = self.build_tonemap_params(config, transparent, premultiplied, total_iterations);
            return self.tonemap_gpu_tiled(histogram, config, params).await;
        }

        // ===== Step 1: Convert histogram to Rgba32Float format (parallelized) =====
        // The accumulation texture stores:
        // - R, G, B: averaged colors (sum/count), ∈ [0,1]
        // - A: raw per-pixel density (hit count)
        //
        // Our CPU histogram stores raw sums + raw hit count, so we average the
        // colors and pass the count through as density. Rgba32Float (not f16):
        // the density is a raw count that reaches millions on dense pixels at
        // high iteration counts, far past f16's 65504 ceiling — overflowing it
        // gave `inf` density that tonemapped to black "holes" in the brightest
        // areas. Matches the FlameRenderer accumulator. 16 bytes/pixel.
        let mut texture_data = vec![0u8; histogram.len() * 16];
        texture_data
            .par_chunks_mut(16)
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

                // f32 bytes (16 bytes per pixel)
                chunk[0..4].copy_from_slice(&r.to_le_bytes());
                chunk[4..8].copy_from_slice(&g.to_le_bytes());
                chunk[8..12].copy_from_slice(&b.to_le_bytes());
                chunk[12..16].copy_from_slice(&density.to_le_bytes());
            });


        // ===== Step 2: Create and upload accumulation texture =====
        let accumulation_texture = self.try_create_texture(&TextureDescriptor {
            label: Some("Export Accumulation Texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        }).await?;

        let bytes_per_row = self.width * 16; // 4 channels × 4 bytes (f32)
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

        // Solid shading (Phase 1): shade the accumulation into a separate
        // texture and feed THAT to density effects / tonemap — the same
        // ordering the interactive frame uses.
        let mut shade_holder: Option<(Texture, TextureView)> = None;
        if let (Some(shade), Some(depth)) = (self.shade_pass.as_ref(), self.shade_depth_buffer.as_ref()) {
            let tex = self.try_create_texture(&TextureDescriptor {
                label: Some("Export Shaded Accumulation"),
                size: Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba32Float,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }).await?;
            let view = tex.create_view(&TextureViewDescriptor::default());
            let mut shade_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Export Shade Encoder"),
            });
            shade.run_region(
                &self.device,
                &self.queue,
                &mut shade_encoder,
                &accumulation_view,
                depth,
                &view,
                &config.solid_shading,
                config.zoom,
                config.rotation,
                config.pan_x,
                config.pan_y,
                config.perspective_strength,
                self.surface_thickness,
                self.width,
                self.height,
                0,
                0,
                self.height,
                (config.camera_rotation_x, config.camera_rotation_y, config.camera_bank,
                 [config.camera_x, config.camera_y, config.camera_z]),
                self.shadow_maps_buffer.as_ref().map(|_| {
                    (self.width * self.height, self.shadow_fit.0, self.shadow_fit.1)
                }),
                (config.fog_strength, config.fog_start, config.background_color),
                0.0,
                None,
            );
            self.queue.submit(std::iter::once(shade_encoder.finish()));
            log::info!("High-res export: applied solid lighting (shade pass)");
            shade_holder = Some((tex, view));
        }

        // Post-process DoF (solid mode): between shade and density
        // effects/tonemap, same ordering as the interactive frame. Needs
        // the per-pixel depth buffer (present whenever solid shading
        // ran on this path).
        let mut dof_holder: Option<crate::renderer::dof_pass::DofPass> = None;
        if config.dof_blur_strength > 0.0
            && matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD)
        {
            if let Some(depth) = self.shade_depth_buffer.as_ref() {
                let input_view: &TextureView =
                    shade_holder.as_ref().map(|(_, v)| v).unwrap_or(&accumulation_view);
                let mut dp = crate::renderer::dof_pass::DofPass::new(&self.device);
                let mut dof_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Export DoF Encoder"),
                });
                dp.run(
                    &self.device,
                    &self.queue,
                    &mut dof_encoder,
                    input_view,
                    depth,
                    0, // dedicated depth buffer: depth at word offset 0
                    self.width,
                    self.height,
                    config.zoom,
                    config.dof_focus_distance,
                    config.dof_blur_strength,
                    self.surface_thickness,
                );
                self.queue.submit(std::iter::once(dof_encoder.finish()));
                log::info!("High-res export: applied post-process DoF");
                dof_holder = Some(dp);
            } else {
                log::warn!("High-res export: DoF requested but no depth buffer (solid rendering off) — skipped");
            }
        }
        let base_accum_view: &TextureView = match &dof_holder {
            Some(dp) => dp.output_view(),
            None => shade_holder.as_ref().map(|(_, v)| v).unwrap_or(&accumulation_view),
        };

        // ===== Step 3: Set up tonemap params =====
        // Calculate area and sample_density matching GPU formula
        let zoom = config.zoom;
        let base_pixels_per_unit = (self.width.min(self.height) as f32) * 0.25;
        let apophysis_zoom = zoom.log2();
        let pixels_per_unit_zoomed = base_pixels_per_unit * 2.0_f32.powf(apophysis_zoom);
        let area = (self.width as f32 * self.height as f32) / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);

        // Sample density: running iterations-per-pixel (Ember/Apophysis
        // formula). See docs/projects/accumulator-unification.md and
        // the matching site in compute_kernel.rs::update_tonemap. The
        // shader's `k2 = 1 / (area × white_level × sample_density)`
        // combined with `area = pixels / ppu²` and this `sample_density`
        // yields a scale-invariant `density × k2` so brightness doesn't
        // drift with sample count.
        let total_pixels = (self.width as f32) * (self.height as f32);
        let sample_density = ((total_iterations as f32) * self.solid_density_fraction
            / total_pixels.max(1.0))
            .max(1e-6);

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
            white_level: config.white_level,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: config.saturation,
            hue_shift: config.hue_shift,
            gamma_threshold: config.gamma_threshold,
            alpha_blend_low: config.alpha_blend_low,
            alpha_blend_high: config.alpha_blend_high,
            // 0 = opaque, 1 = straight-alpha reconstruction, 2 = premultiplied.
            transparent_mode: if !transparent { 0 } else if premultiplied { 2 } else { 1 },
            color_mode: config.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: config.path_map_style as u32,
            burn_in: 20, // Default burn-in for export
            num_transforms: config.flame.transforms.len() as u32,
            palette_size: config.palette_size,
            // Levels (× mean density units) — read from the config, matching
            // FlameRenderer::update_tonemap. Hardcoding defaults here made the
            // tiled export ignore the flame's Levels and render at different
            // contrast than the FlameRenderer path. `levels_enabled` below
            // gates whether they apply at all.
            levels_low: config.levels_low,
            levels_high: config.levels_high,
            levels_gamma: config.levels_gamma,
            highlight_mode: match config.highlight_mode {
                crate::scene::tonemap::HighlightMode::Clip => 0,
                crate::scene::tonemap::HighlightMode::MaxNorm => 1,
                crate::scene::tonemap::HighlightMode::Reinhard => 2,
                crate::scene::tonemap::HighlightMode::Filmic => 3,
            },
            // Levels apply identically in transparent and opaque export: they
            // gate the fractal ALPHA (opacity), and the transparent PNG carries
            // that same alpha so compositing it over the background reconstructs
            // the opaque export exactly. Respect the flame's setting in both
            // modes (matches FlameRenderer::set_transparent_mode after its fix).
            levels_enabled: if config.levels_enabled { 1 } else { 0 },
            _pad_levels: [0; 2],
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

        // ===== Step 3.5: Density effects (Phase B, before tonemap) =====
        // Apply density effects on the full-res accumulation BEFORE tonemap,
        // symmetric with the color-effect pass below. The GPU histogram buffer
        // was freed (drop+poll) before this function, so these full-res
        // ping-pong textures don't coexist with it. Mirrors the FlameRenderer
        // path (render.rs): first effect reads the Rgba32Float accumulation
        // (needs FLOAT32_FILTERABLE — run_density_effects guards that), writes
        // the Rgba16Float ping-pong, and the result feeds tonemap's binding 0
        // (textureLoad, so no filtering needed). Guarded by a resolution
        // ceiling so the extra texture memory doesn't OOM at extreme sizes.
        let has_density_effects = EffectChainRunner::has_enabled_effects(&config.density_effects);
        let mut density_chain: Option<EffectChainRunner> = None;
        if has_density_effects {
            if Self::density_effects_fit(self.width, self.height) {
                let mut chain = EffectChainRunner::new(&self.device, self.width, self.height);
                let mut density_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Export Density Effects Encoder"),
                });
                chain.reset_slots();
                let ran = chain.run_density_effects(
                    &self.device,
                    &self.queue,
                    &mut density_encoder,
                    base_accum_view,
                    &config.density_effects,
                );
                self.queue.submit(std::iter::once(density_encoder.finish()));
                if ran {
                    log::info!(
                        "High-res export: applied {} density effect(s)",
                        config.density_effects.iter().filter(|e| e.enabled).count()
                    );
                    density_chain = Some(chain);
                }
            } else {
                log::warn!(
                    "High-res export: skipping density effects at {}x{} — estimated Phase B \
                     texture memory ({} MB) exceeds the safe budget ({} MB); export continues \
                     without them to avoid OOM.",
                    self.width,
                    self.height,
                    (self.width as u64 * self.height as u64 * 36) / (1024 * 1024),
                    Self::DENSITY_EFFECT_PHASE_B_BUDGET_BYTES / (1024 * 1024),
                );
            }
        }
        // Density output (Rgba16Float) when effects ran, else the raw
        // accumulation. Held by `density_chain`, which must outlive the tonemap
        // pass below (it owns the ping-pong textures the bind group samples).
        let tonemap_input_view: &TextureView = density_chain
            .as_ref()
            .and_then(|c| c.get_density_output())
            .unwrap_or(base_accum_view);

        // ===== Step 4: Create bind group and output texture =====
        let curve_lut_view = self.curve_lut_texture.create_view(&TextureViewDescriptor::default());
        let palette_view = self.palette_texture.create_view(&TextureViewDescriptor::default());

        let tonemap_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Export Tonemap Bind Group"),
            layout: &self.tonemap_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(tonemap_input_view),
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
                // Bindings 5-7: required by the unified tonemap.wgsl,
                // used only when color_mode == 2 (PathMap). Export uses
                // a dummy path buffer (compute pass doesn't track paths
                // in export); PathMap export was already broken
                // pre-unification.
                BindGroupEntry {
                    binding: 5,
                    resource: self.dummy_path_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: BindingResource::TextureView(&palette_view),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: BindingResource::Sampler(&self.palette_sampler),
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
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.tonemap_pipeline);
            render_pass.set_bind_group(0, &tonemap_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        // Free the accumulation texture (≈ W·H·16, ~2.3 GB at 12K) and any
        // density-effect ping-pong now that tonemap has written `output_view`.
        // They're dead weight through the color-effect pass below, whose
        // full-res ping-pong (≈ W·H·8) would otherwise allocate ON TOP of them
        // and OOM at high resolution (wgpu treats OOM as a fatal panic). Drop
        // the bind group (it holds Arc refs to the accumulation/density views),
        // then the textures, then poll so wgpu actually reclaims the VRAM
        // before the color ping-pong allocates (a plain drop is deferred).
        drop(tonemap_bind_group);
        drop(density_chain);
        drop(accumulation_view);
        drop(accumulation_texture);
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });

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
