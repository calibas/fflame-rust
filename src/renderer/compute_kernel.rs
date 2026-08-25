use egui_wgpu::wgpu::*;
use crate::gpu::{buffers::*, pipelines::FlamePipelines};
use crate::scene::transforms::Flame;
use crate::scene::palette::{Palette, ColorMode, PathMapStyle, PathCaptureMode, PathTrackingMode};
use crate::config::FractalConfig;
use crate::shader_builder_v2::ShaderConstants;

/// Path entry storing first 32 iterations of transform sequence
/// Also stores initial random X/Y coordinates for complete path reconstruction
/// Matches GPU PathEntry struct layout (7 × u32 = 5 u32 + 2 f32)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct PathEntry {
    /// Iterations 0-7 (4 bits each, LSB = iteration 0)
    pub path0: u32,
    /// Iterations 8-15
    pub path1: u32,
    /// Iterations 16-23
    pub path2: u32,
    /// Iterations 24-31
    pub path3: u32,
    /// Number of valid iterations stored (0-32)
    pub iteration_count: u32,
    /// Initial random X coordinate [-1, 1]
    pub initial_x: f32,
    /// Initial random Y coordinate [-1, 1]
    pub initial_y: f32,
}

impl PathEntry {
    /// Extract transform index at given iteration (0-31)
    /// Returns None if iteration >= iteration_count
    pub fn get_transform(&self, iteration: u32) -> Option<u32> {
        if iteration >= self.iteration_count {
            return None;
        }
        let slot = iteration / 8;
        let pos = (iteration % 8) * 4;
        let path = match slot {
            0 => self.path0,
            1 => self.path1,
            2 => self.path2,
            3 => self.path3,
            _ => return None,
        };
        Some((path >> pos) & 0xF)
    }

    /// Get full path as Vec of transform indices
    pub fn to_vec(&self) -> Vec<u32> {
        (0..self.iteration_count)
            .filter_map(|i| self.get_transform(i))
            .collect()
    }

    /// Get prefix data: first 8 iterations (path0 only)
    /// Matches GPU get_prefix() function
    pub fn get_prefix(&self) -> u32 {
        self.path0
    }

    /// Get suffix data: last 8 valid iterations based on iteration_count
    /// Matches GPU get_suffix() function
    pub fn get_suffix(&self) -> u32 {
        let count = self.iteration_count;
        if count <= 8 {
            self.path0
        } else if count <= 16 {
            self.path1
        } else if count <= 24 {
            self.path2
        } else {
            self.path3
        }
    }

    /// Scramble hash for maximum color separation
    /// Matches GPU scramble_hash() function (MurmurHash3 finalizer)
    pub fn scramble_hash(x: u32) -> u32 {
        let mut h = x;
        h ^= h >> 16;
        h = h.wrapping_mul(0x85ebca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2ae35);
        h ^= h >> 16;
        h
    }

    /// Compute hue value for Prefix Distinct coloring mode (style 2)
    /// Matches GPU path_to_color_prefix_distinct() function
    /// Incorporates iteration_count to distinguish paths of different lengths
    pub fn compute_prefix_distinct_hue(&self) -> f32 {
        let value = self.get_prefix();
        // Mix iteration_count into the value before hashing (same as GPU)
        let mixed = value ^ (self.iteration_count.wrapping_mul(0x9E3779B9));
        let scrambled = Self::scramble_hash(mixed);
        let golden_ratio: f64 = 0.618033988749895;
        let hue = (scrambled as f64 * golden_ratio / u32::MAX as f64).fract();
        hue as f32
    }

    /// Compute hue value for Suffix Distinct coloring mode (style 3)
    /// Matches GPU path_to_color_distinct() function
    pub fn compute_suffix_distinct_hue(&self) -> f32 {
        let value = self.get_suffix();
        let scrambled = Self::scramble_hash(value);
        let golden_ratio: f64 = 0.618033988749895;
        let hue = (scrambled as f64 * golden_ratio / u32::MAX as f64).fract();
        hue as f32
    }
}

use crate::variations::analytic_blur::BlurSlotInfo;

pub struct FlameRenderer {
    /// Reachability-census mode (see `src/census/`). Set only by
    /// `enable_census`, read into `ShaderConstants::census`. Never a
    /// render: census renders are the census runner's own.
    census: bool,

    pipelines: FlamePipelines,
    buffers: FlameBuffers,
    compute_bind_group: BindGroup,
    accumulate_bind_group: BindGroup,
    histogram_blur_h_bind_group: BindGroup,
    histogram_blur_v_bind_group: BindGroup,
    // adjust_scale_bind_group removed - pipeline unused
    tonemap_bind_group: BindGroup,

    /// Analytic-blur convolution-stage bind groups (convolve → upscale).
    /// Recreated whenever the low-res blur buffers are (re)allocated (which
    /// happens when the downscale D changes). See analytic-blur-buffer.md.
    blur_convolve_bind_group: BindGroup,
    blur_upscale_bind_group: BindGroup,
    /// Per-slot kernel inputs captured from the flame (weight + post-affine
    /// linear), in the same order as the blur slot assignment. Empty when the
    /// feature is inactive. The pixel-space kernel is rebuilt from these +
    /// the current zoom/rotation (see `maybe_rebuild_blur_kernels`).
    blur_slots: Vec<BlurSlotInfo>,
    /// Cache keys for the last kernel build, so the (CPU) Monte-Carlo rebuild
    /// only runs when the view or flame actually changed.
    blur_kernel_zoom: f32,
    blur_kernel_rotation: f32,
    blur_kernels_dirty: bool,
    /// Low-res convolution dims for the current kernel build (downsample +
    /// convolve dispatch sizes). Set by `maybe_rebuild_blur_kernels`.
    blur_lowres_w: u32,
    blur_lowres_h: u32,
    /// Per-frame dither seed for the upscale's stochastic rounding. Advanced
    /// every frame the convolution runs.
    blur_frame_seed: u32,

    /// Bind group for the init compute pass. Stable across the renderer's
    /// lifetime — references the variation_params buffer with read_write
    /// access. Init pipeline lives in `pipelines.shader_cache.init_pipeline`
    /// and is `None` when no active variation has `wgsl_init`.
    init_bind_group: BindGroup,

    /// Set whenever variation params are written to the buffer. The next
    /// `compute_pass` will dispatch the init shader (if one is built) and
    /// clear this flag.
    init_dirty: bool,

    // Output texture that tonemap_pass renders to (for both display and export)
    fractal_texture: Texture,
    fractal_texture_view: TextureView,

    /// Persistent staging buffer for `read_fractal_pixels`, grow-only.
    ///
    /// This used to be created per read and never destroyed — and on
    /// WebGPU, dropping a buffer frees nothing (wgpu's wasm `Drop` is a
    /// no-op; the memory lives until the JS GC notices the wrapper,
    /// which it barely does, since GPU memory exerts no JS heap
    /// pressure). Measured in the gallery client: `createBuffer` 2,137
    /// times over 1,000 renders, `destroy()` zero — ~width·height·4
    /// leaked per tile, and MAP_READ buffers in Firefox pin CPU-side
    /// shmem on top, several times their nominal size. A buffer can be
    /// re-mapped after `unmap`, so one persistent buffer serves every
    /// read; it grows to the largest size seen and dies with the
    /// renderer in `destroy()`.
    readback_staging: Option<Buffer>,

    pub width: u32,
    pub height: u32,
    samples_accumulated: u64,
    total_iterations: u64,
    effective_iterations: u64, // For brightness calculation - doesn't reset during overwrite mode
    /// Iterations whose contribution is *currently in the persistent
    /// accumulation buffer*. Used by the tonemap's sample_density
    /// formula. Differs from `total_iterations` during overwrite mode:
    /// overwrite_mode discards prev each accumulate so the buffer
    /// only ever holds the latest dispatch, but compute_pass keeps
    /// incrementing total_iterations every frame. Without this
    /// separate counter, sample_density grows while bucket_count
    /// stays put → tonemap reads scale-mismatched data → preview
    /// mode goes ~N× dimmer than steady-state for N drag frames.
    samples_in_buffer: u64,
    /// Samples of the most recent compute batch — the unit for the
    /// auto-refit's "how many batches deep are we" estimate.
    last_batch_samples: u64,
    /// Attractor-bounds readback (shadow-map auto-fit).
    bounds_stats: crate::renderer::density_stats::BoundsTracker,

    /// The sticky variation superset — Layer B of
    /// docs/projects/sticky-shader-compilation.md. Renderer state, never
    /// serialized. Default-on; `set_sticky_enabled(false)` restores
    /// specialized-per-flame compilation.
    sticky: crate::renderer::sticky::StickyVariations,
    /// Last decoded world AABB of plotted samples. Persists across
    /// resets as "last known" — the placement frozen at each reset uses
    /// it, and fresh measurements replace it as they arrive.
    measured_bounds: Option<([f32; 3], [f32; 3])>,
    /// A fresh bounds measurement arrived since the placement froze.
    bounds_dirty: bool,
    /// Light-space shadow-map fit (center, radius) FROZEN at the last
    /// accumulation reset — splat texel coordinates depend on it.
    frozen_shadow_fit: Option<([f32; 3], f32)>,
    /// Whether frozen_shadow_fit was derived from MEASURED bounds (vs a
    /// view guess). A guess-based fit gets one tightening refit when the
    /// first real measurement lands; measured fits only refit on growth.
    frozen_fit_measured: bool,
    /// The auto-refit already fired once this accumulation run. HARD
    /// cap: however the bounds evolve, the auto-refit may interrupt a
    /// run at most once — repeat interruptions are exactly the reset
    /// loop the refit logic keeps re-growing (field-reported twice).
    fit_refit_done: bool,
    /// Shade inputs changed since the last shade (accumulate ran,
    /// lighting edited, reset, ...). When false and the temporal blend
    /// has settled, run_shade_pass skips entirely — the previous output
    /// texture is still exact. The march + normals chain + AO + shadows
    /// are NOT cheap; without this they burned GPU every frame even
    /// after rendering completed (field-reported).
    shade_dirty: bool,
    /// Temporal-blend settle countdown after the last dirty shade.
    shade_settle: u32,
    color_mode: ColorMode,
    path_map_style: PathMapStyle,
    path_capture_mode: PathCaptureMode,
    path_tracking_mode: PathTrackingMode,
    density_scale: f32,
    white_level: f32,
    highlight_mode: u32,
    background_color: [f32; 3],
    current_render_mode: crate::scene::transforms::RenderMode,
    /// Scene-level `preserve_z` (config-level since v3). Cached on the renderer
    /// so the incremental shader-rebuild path (`update_path_features` →
    /// `build_shader_constants`) can compute `flatten_z_per_iter` without a
    /// `FractalConfig` in hand.
    preserve_z: bool,
    perspective_strength: f32,
    depth_density_compensation: f32,
    far_density_fade: f32,
    far_density_fade_start: f32,
    deterministic_rng: bool,
    frame_counter: u32, // For deterministic seed progression
    dof_focus_distance: f32, // DOF: Distance from origin where image is sharpest
    dof_blur_strength: f32, // DOF: Blur amount (0.0 = disabled)
    fog_strength: f32, // Depth fog: exponential fog density (0.0 = disabled)
    fog_start: f32, // Depth fog: distance where fog begins
    solid_strength: f32, // Solid rendering: occlusion strength (0 = off)
    surface_thickness: f32, // Solid rendering: depth shell (world units)
    needs_depth_prime: bool, // Next compute batch records depth only (set on reset while solid)
    solid_shading: crate::config::SolidShadingSettings, // Phase 1 lighting (shade pass); active() => depth capture even at solid_strength 0
    shade_pass: crate::renderer::shade_pass::ShadePass, // Deferred shade pass (dispatched only when shading is active)
    dof_pass: crate::renderer::dof_pass::DofPass, // Post-process DoF (solid mode; at-splat DoF compiles out under SOLID)
    dof_dirty: bool, // DoF input (shade output / accumulator) changed since the last DoF dispatch
    solid_density_fraction: f32, // Measured accepted/dispatched fraction (1.0 = no correction); scales tonemap sample_density
    filter_radius: f32, // Spatial filter (Apo's `filter`): Gaussian sigma in pixels on histogram, 0 = off
    filter_blur_edges: f32, // Bilateral edge-handling [0..1]: 0 = preserve edges (default), 1 = uniform Gaussian
    background_r: f32, // Background color R (for depth fog)
    background_g: f32, // Background color G (for depth fog)
    background_b: f32, // Background color B (for depth fog)
    /// Post-symmetry — see `Flame.post_symmetry`. Cached on the
    /// renderer so the per-frame `GpuParams` write can pull from it
    /// without re-walking the config. Driven by
    /// `set_post_symmetry()` from the GPU-update pipeline.
    post_symmetry: crate::scene::transforms::PostSymmetry,
    burn_in: u32, // Burn-in iterations (for Depth gradient in PathMap mode)
    blend_factor: f32, // Accumulation blend rate: 0.01 (slow/smooth) to 1.0 (fast/flickery), default: 0.1
    use_dynamic_blend: bool, // true = exponential convergence (old), false = fixed blend rate (new)
    overwrite_mode: bool, // When true, replace accumulation buffer instead of blending (for live preview)
    num_transforms: u32, // Number of normal transforms
    path_filters: Vec<crate::gpu::buffers::GpuPathFilter>, // Active path filters
    min_suffix_filter_length: u32, // Minimum length among depth=0 filters (optimization)
}

impl FlameRenderer {
    /// Create new FlameRenderer with default palette size (256)
    pub fn new(
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        width: u32,
        height: u32,
        flame: &Flame,
    ) -> Self {
        Self::with_palette_size(device, queue, surface_format, width, height, flame, crate::gpu::buffers::DEFAULT_PALETTE_SIZE)
    }

    /// Create new FlameRenderer with specified palette size
    pub fn with_palette_size(
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        width: u32,
        height: u32,
        flame: &Flame,
        palette_size: u32,
    ) -> Self {
        let pipelines = FlamePipelines::new(device, surface_format, flame);
        let buffers = FlameBuffers::with_palette_size(device, queue, width, height, flame, palette_size);

        let compute_bind_group = pipelines.create_compute_bind_group(device, &buffers);
        let accumulate_bind_group = pipelines.create_accumulate_bind_group(device, &buffers);
        let histogram_blur_h_bind_group = pipelines.create_histogram_blur_h_bind_group(device, &buffers);
        let histogram_blur_v_bind_group = pipelines.create_histogram_blur_v_bind_group(device, &buffers);
        // adjust_scale_bind_group removed - pipeline unused
        let tonemap_bind_group = pipelines.create_tonemap_bind_group(device, &buffers);
        let init_bind_group = pipelines.create_init_bind_group(device, &buffers);
        let blur_convolve_bind_group = pipelines.create_blur_convolve_bind_group(device, &buffers);
        let blur_upscale_bind_group = pipelines.create_blur_upscale_bind_group(device, &buffers);

        // Create fractal output texture (Rgba8Unorm for compatibility with tonemap pipeline)
        let fractal_texture = device.create_texture(&TextureDescriptor {
            label: Some("Fractal Output"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fractal_texture_view = fractal_texture.create_view(&TextureViewDescriptor::default());

        // DEBUG: Log renderer initialization
        #[cfg(target_arch = "wasm32")]
        log::info!("=== FlameRenderer Created ===");
        #[cfg(target_arch = "wasm32")]
        log::info!("  Render resolution: {}x{}", width, height);
        #[cfg(target_arch = "wasm32")]
        log::info!("  Surface format: {:?}", surface_format);
        #[cfg(target_arch = "wasm32")]
        log::info!("==============================");

        Self {
            pipelines,
            buffers,
            compute_bind_group,
            accumulate_bind_group,
            histogram_blur_h_bind_group,
            histogram_blur_v_bind_group,
            // adjust_scale_bind_group removed
            tonemap_bind_group,
            blur_convolve_bind_group,
            blur_upscale_bind_group,
            blur_slots: Vec::new(),
            blur_kernel_zoom: f32::NAN,      // force a build on first active frame
            blur_kernel_rotation: f32::NAN,
            blur_kernels_dirty: true,
            blur_lowres_w: 0,
            blur_lowres_h: 0,
            blur_frame_seed: 0,
            init_bind_group,
            init_dirty: true, // Run init once on first frame to populate slots
            fractal_texture,
            fractal_texture_view,
            readback_staging: None,
            width,
            height,
            samples_accumulated: 0,
            total_iterations: 0,
            effective_iterations: 0,
            samples_in_buffer: 0,
            last_batch_samples: 0,
            bounds_stats: crate::renderer::density_stats::BoundsTracker::new(device),
            sticky: crate::renderer::sticky::StickyVariations::new(),
            measured_bounds: None,
            bounds_dirty: false,
            frozen_shadow_fit: None,
            frozen_fit_measured: false,
            fit_refit_done: false,
            shade_dirty: true,
            shade_settle: 0,
            color_mode: ColorMode::Palette,
            path_map_style: PathMapStyle::default(),
            path_capture_mode: PathCaptureMode::default(),
            path_tracking_mode: PathTrackingMode::default(),
            density_scale: 1.0,
            white_level: crate::config::defaults::DEFAULT_WHITE_LEVEL,
            highlight_mode: 0,  // Clip — Apophysis-compatible default
            background_color: [0.0, 0.0, 0.0],
            // Scene-level render state (lives on FractalConfig since v3, not
            // Flame). These are placeholder defaults like the other constants
            // here; the real values arrive via `update_flame` before any
            // render.
            current_render_mode: crate::scene::transforms::RenderMode::TwoD,
            preserve_z: false,
            perspective_strength: 0.0,
            depth_density_compensation: 0.0,
            far_density_fade: 0.0,
            far_density_fade_start: 0.0,
            deterministic_rng: true, // Default to deterministic for reproducible rendering
            frame_counter: 0,
            dof_focus_distance: crate::config::DEFAULT_DOF_FOCUS_DISTANCE,
            dof_blur_strength: crate::config::DEFAULT_DOF_BLUR_STRENGTH,
            fog_strength: crate::config::DEFAULT_FOG_STRENGTH,
            fog_start: crate::config::DEFAULT_FOG_START,
            solid_strength: crate::config::DEFAULT_SOLID_STRENGTH,
            surface_thickness: crate::config::DEFAULT_SURFACE_THICKNESS,
            needs_depth_prime: false,
            solid_shading: crate::config::SolidShadingSettings::default(),
            shade_pass: crate::renderer::shade_pass::ShadePass::new(device, width, height),
            dof_pass: crate::renderer::dof_pass::DofPass::new(device),
            dof_dirty: true,
            solid_density_fraction: 1.0,
            filter_radius: 0.0,
            filter_blur_edges: 0.0,
            background_r: 0.0,
            background_g: 0.0,
            background_b: 0.0,
            post_symmetry: crate::scene::transforms::PostSymmetry::default(),
            burn_in: 20, // Default burn-in iterations (this is a FlameRenderer field, not GpuParams)
            blend_factor: 0.1, // 10% blend rate - good balance between speed and smoothness
            use_dynamic_blend: true, // Default to clamped exponential (0.8 → 0.01)
            overwrite_mode: false, // Default to normal blending (progressive refinement)
            num_transforms: flame.transforms.len() as u32,
            path_filters: Vec::new(), // No filters by default
            min_suffix_filter_length: 0,
            census: false,
        }
    }

    /// Resize the accumulation buffer
    pub fn resize(&mut self, device: &Device, encoder: &mut CommandEncoder, queue: &Queue, width: u32, height: u32, flame: &Flame, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_x: f32, camera_y: f32, camera_z: f32, speed_factor: f32) {
        // Sticky superset: FlameBuffers::with_palette_size below repacks
        // transforms and variation params from this flame, and the
        // compiled shader still holds the last adopt's canonical map —
        // repack against that same map or every weight offset misaligns
        // after a window resize.
        let flame = &self.sticky.augmented(flame);
        self.width = width;
        self.height = height;

        // Update transform tracking from flame (critical for final transform support)
        self.num_transforms = flame.transforms.len() as u32;

        // Recreate buffers with new size (preserve palette_size)
        let palette_size = self.buffers.palette_size();
        self.buffers = FlameBuffers::with_palette_size(device, queue, width, height, flame, palette_size);

        // Re-apply the solid depth region — fresh buffers default to none.
        // Bind groups referencing the histogram are recreated just below.
        let solid_enabled = (self.solid_strength > 0.0 || self.solid_shading.active())
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD);
        self.buffers.set_solid_depth_region(device, solid_enabled);
        self.buffers.set_census_region(device, self.census);
        self.needs_depth_prime = solid_enabled;
        self.shade_pass.resize(device, width, height);

        // Recreating variation_params_buffer wipes the init-derived slots
        // (slots N..N+M, written by the init dispatch). User-param slots
        // 0..N are repopulated by FlameBuffers::with_palette_size's call to
        // update_variation_params, but init slots will be zeros until init
        // dispatches again — flag it dirty so next compute_pass re-runs init.
        self.init_dirty = true;

        // Restore xaos buffer if flame has xaos weights
        self.update_xaos_buffer(device, queue, flame);
        // Refresh the blur slot list + bind to the freshly-recreated (dummy)
        // blur buffers; maybe_rebuild_blur_kernels reallocates at the new size
        // on the next compute_pass.
        self.update_blur_buffers(flame);
        self.blur_convolve_bind_group = self.pipelines.create_blur_convolve_bind_group(device, &self.buffers);
        self.blur_upscale_bind_group = self.pipelines.create_blur_upscale_bind_group(device, &self.buffers);

        // Recreate bind groups (must be after xaos buffer is restored)
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
        self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
        self.histogram_blur_h_bind_group = self.pipelines.create_histogram_blur_h_bind_group(device, &self.buffers);
        self.histogram_blur_v_bind_group = self.pipelines.create_histogram_blur_v_bind_group(device, &self.buffers);
        self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);

        // Recreate fractal output texture with new size
        self.fractal_texture = device.create_texture(&TextureDescriptor {
            label: Some("Fractal Output"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.fractal_texture_view = self.fractal_texture.create_view(&TextureViewDescriptor::default());

        // Clear accumulation counter
        self.reset(encoder, queue, iterations_per_thread, zoom, pan_x, pan_y, rotation, camera_rotation_x, camera_rotation_y, camera_bank, camera_x, camera_y, camera_z, speed_factor);

        // NOTE: Tonemap params need to be restored after buffer recreation
        // The caller should call update_tonemap() with current config values after resize()
    }

    /// Reset iteration counters without clearing accumulation buffer
    /// Used when transitioning from overwrite mode to normal accumulation
    pub fn reset_iteration_counter(&mut self) {
        self.samples_accumulated = 0;
        self.total_iterations = 0;
        self.effective_iterations = 0; // Reset for new accumulation phase
        self.samples_in_buffer = 0; // Buffer's contents will be replaced/regrown after the reset
        self.frame_counter = 0; // Reset frame counter for deterministic seed progression
    }

    /// Like `reset_iteration_counter`, but preserves `samples_in_buffer`.
    ///
    /// Use this at overwrite-exit in cumulative-mean mode, where the
    /// accumulator texture is intentionally NOT cleared (the leftover
    /// drag-frame samples dilute naturally as new iterations arrive).
    /// `samples_in_buffer` must stay aligned with what's actually in
    /// the accumulator, or the next `refresh_sample_density()` writes
    /// `sample_density ≈ 0` while density values in the buffer are
    /// non-zero — making `density / sample_density` huge in the
    /// shader, clamping `apply_levels` to 1, and briefly disabling
    /// Levels for one frame. Visible as a bright flash at
    /// preview-to-normal transitions.
    ///
    /// Fixed-EMA mode keeps using `reset_iteration_counter` (which
    /// zeros samples_in_buffer) because that path also clears the
    /// accumulator immediately afterward — both go to zero together.
    pub fn reset_iteration_counter_keep_buffer(&mut self) {
        self.samples_accumulated = 0;
        self.total_iterations = 0;
        self.effective_iterations = 0;
        self.frame_counter = 0;
        // NOT zeroed: self.samples_in_buffer
    }

    /// Whether the renderer is currently in cumulative-mean accumulate
    /// mode (true) or fixed-EMA mode (false). Mirrors the
    /// `Dynamic blend` UI checkbox.
    pub fn use_dynamic_blend(&self) -> bool {
        self.use_dynamic_blend
    }

    /// Clear both ping-pong accumulation textures. Both halves of the
    /// ping-pong are cleared so it doesn't matter which is "current"
    /// when iteration resumes — the next read of `previous_accumulation`
    /// returns zeros either way. Used by the overwrite-exit path in
    /// fixed-EMA mode (see `app/gpu_updates.rs::update_overwrite_mode`),
    /// where leftover drag-frame samples would otherwise dominate the
    /// EMA's bootstrap and produce a "way too bright" frame for
    /// ~1/blend_factor frames before the EMA averages them out.
    /// Cumulative mode skips this — the leftover drag samples are one
    /// batch's worth of valid data that dilutes naturally.
    pub fn clear_accumulation_buffers(&self, encoder: &mut CommandEncoder, queue: &Queue) {
        self.buffers.clear_all(encoder, queue);
    }

    /// Build shader constants from current renderer state
    /// Used for incremental updates where FractalConfig isn't available
    /// Note: This creates non-inlined constants (legacy mode) for compatibility
    fn build_shader_constants(
        &self,
        flame: &Flame,
        render_mode: crate::scene::transforms::RenderMode,
        preserve_z: bool,
    ) -> ShaderConstants {
        ShaderConstants {
            // .max(1): empty flames (e.g., a freshly-added empty subflame
            // before the user has populated it) would compile a shader
            // with `NUM_TRANSFORMS - 1u` underflowing to u32::MAX, which
            // WGSL catches at compile time and aborts the device on.
            // The other constants path (`with_inlined_transforms`)
            // already applies the same guard.
            num_transforms: (flame.transforms.len() as u32).max(1),
            color_mode: self.color_mode as u32,
            has_post_affine: flame.has_post_affine(),
            has_attachments: flame.has_attachments(),
            has_post_symmetry: flame.post_symmetry.ty != crate::scene::transforms::PostSymmetryType::None,
            has_analytic_blur: flame.analytic_blur_active(&crate::variations::global_registry(), render_mode),
            flatten_z_per_iter: matches!(render_mode, crate::scene::transforms::RenderMode::ThreeD)
                && !preserve_z,
            solid_enabled: (self.solid_strength > 0.0 || self.solid_shading.active())
                && matches!(render_mode, crate::scene::transforms::RenderMode::ThreeD),
            probe: false,
            census: self.census,
            attachment_cap: flame.attachment_cap() as u32,
            // No inlining for incremental updates (would trigger too many shader rebuilds)
            inlined_transforms: None,
            cumulative_weights: None,
            // fx_priority phase overrides are baked per-flame (the
            // interactive shader can't read per-transform priorities at
            // runtime); resolve them with the same local index map the
            // buffer populator uses.
            variation_priorities: {
                let registry = crate::variations::global_registry();
                let id_map = crate::scene::transforms::compute_local_index_map(
                    flame.active_variation_names_ordered(&registry),
                );
                crate::shader_builder_v2::collect_phase_overrides(flame, &registry, &id_map)
            },
        }
    }

    /// Reset accumulation buffer and sample count (full reset including effective iterations)
    pub fn reset(&mut self, encoder: &mut CommandEncoder, queue: &Queue, _iterations_per_thread: u32, _zoom: f32, _pan_x: f32, _pan_y: f32, _rotation: f32, _camera_rotation_x: f32, _camera_rotation_y: f32, _camera_bank: f32, _camera_x: f32, _camera_y: f32, _camera_z: f32, _speed_factor: f32) {
        // Solid rendering: the depth region is about to be cleared —
        // record depth only on the first batch after this reset.
        self.needs_depth_prime = (self.solid_strength > 0.0 || self.solid_shading.active())
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD);
        self.reset_iteration_counter();
        // Reset effective_iterations when doing a full reset (buffer cleared)
        self.effective_iterations = 0;
        // Shade temporal history belongs to the pre-reset accumulation
        // (and, in video export, to the PREVIOUS animation frame —
        // blending against it would motion-ghost).
        self.shade_pass.reset_temporal();
        self.shade_dirty = true;
        self.dof_dirty = true;

        // Clear accumulation buffers
        self.buffers.clear_all(encoder, queue);

        // Note: scale_buffer removed - scale is now in params.histogram_color_scale
        // Note: We don't update params here because update_flame() already set them correctly.
        // Updating params here would overwrite num_transforms which was just set by update_flame().
    }

    /// Run compute pass to generate flame samples
    /// Returns the number of samples generated this frame
    /// - `clear_histogram`: Clear histogram buffer (needed each batch for proper accumulation math)
    /// - `clear_paths`: Clear path buffer (only needed on full reset, not each batch)
    pub fn compute_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, device: &Device, num_workgroups: u32, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_x: f32, camera_y: f32, camera_z: f32, speed_factor: f32, clear_histogram: bool, clear_paths: bool) -> u64 {
        // Update seed for new random samples each frame
        // projection_type removed - shader now uses perspective_strength directly
        // 0.0 = orthographic (flat), higher values = increasing perspective

        let seed = self.get_rng_seed();

        // Depth-priming: the first batch after a full reset (while solid
        // rendering is active) records depth only — the SOLID shader path
        // zeroes every sample's plot weight — so the accumulator never
        // ingests interior samples gated against an empty depth buffer.
        let depth_prime_flag: u32 = if self.needs_depth_prime {
            self.needs_depth_prime = false;
            1
        } else {
            0
        };

        let sh_fit = self.frozen_shadow_fit.unwrap_or_else(|| self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, [camera_x, camera_y, camera_z]));
        let sh_dirs = self.shadow_light_dirs(camera_rotation_x, camera_rotation_y, camera_bank);
        let params = GpuParams {
            num_transforms: self.num_transforms,
            iterations_per_thread,
            burn_in,
            width: self.width,
            height: self.height,
            seed,
            color_mode: self.color_mode as u32,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
                // The flame renderer never receives an escape config —
                // the app and render_with branch before this point. If
                // one is mis-routed anyway, render it as 2D rather than
                // panicking inside a GPU pass.
                crate::scene::transforms::RenderMode::Escape => 0,
            },
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            perspective_strength: self.perspective_strength,
            depth_density_compensation: self.depth_density_compensation,
            far_density_fade: self.far_density_fade,
            far_density_fade_start: self.far_density_fade_start,
            solid_strength: self.solid_strength,
            surface_thickness: self.surface_thickness,
            depth_prime: depth_prime_flag,
            camera_rotation_x,
            camera_rotation_y,
            camera_bank,

            camera_x,

            camera_y,
            camera_z,
            dof_focus_distance: self.dof_focus_distance,
            dof_blur_strength: self.dof_blur_strength,
            fog_strength: self.atsplat_fog_strength(),
            fog_start: self.fog_start,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
            path_tracking_mode: self.path_tracking_mode as u32,
            num_path_filters: self.path_filters.len() as u32,
            min_suffix_filter_length: self.min_suffix_filter_length,
            background_r: self.background_r,
            background_g: self.background_g,
            background_b: self.background_b,
            post_symmetry: (&self.post_symmetry).into(),
            shadow_center_x: sh_fit.0[0],
            shadow_center_y: sh_fit.0[1],
            shadow_center_z: sh_fit.0[2],
            shadow_radius: sh_fit.1,
            shadow_count: sh_dirs.0,
            _pad_shadow: [0; 3],
            shadow_dirs: sh_dirs.1,
        };
        self.buffers.update_params(queue, &params);

        // Update path filter buffer if filters are active and buffers exist
        if !self.path_filters.is_empty() {
            self.buffers.write_path_filters(queue, &self.path_filters);
        }

        // Track total iterations as the count of iterations that
        // actually contribute to the histogram — i.e. dispatched iters
        // *minus* burn-in. The shader runs `iterations_per_thread`
        // total iterations per thread but only plots after `burn_in`,
        // so the plottable fraction is `(iters_per_thread - burn_in) /
        // iters_per_thread`. Using the plotted count here makes
        // `sample_density = total_iters / pixel_count` (Phase 8a's
        // formula) match the *actual* density growth in the
        // accumulator, which is what keeps the tonemap invariant
        // across `iterations_per_thread` choices. Counting dispatched
        // (pre-burn-in) iterations leaks a (1 - burn_in/ipt) factor
        // into brightness — small ipt with the same burn_in produces
        // dimmer images.
        let threads_per_workgroup = 64u64;
        let plotted_per_thread = iterations_per_thread.saturating_sub(burn_in) as u64;
        let samples_this_frame = num_workgroups as u64 * threads_per_workgroup * plotted_per_thread;
        // In overwrite mode, total_iterations reflects only this frame's
        // samples — matches how the accumulator works (prev cleared
        // each frame in the overwrite branch of accumulate.wgsl). Keeps
        // `has_stopped` from tripping during a long drag and triggering
        // the max_iterations-stop code path mid-interaction.
        if self.overwrite_mode {
            self.total_iterations = samples_this_frame;
        } else {
            self.total_iterations += samples_this_frame;
        }

        // Clear histogram buffer before each batch (needed for proper accumulation
        // math). In overwrite mode the solid depth region resets with it — the
        // fractal is changing between frames, so the OLD shape's surface must not
        // occlude the NEW shape's samples (see FlameBuffers::clear_histogram).
        if clear_histogram {
            self.buffers.clear_histogram(encoder, self.overwrite_mode);
        }
        // Clear path buffer only on full reset (view change, flame change, etc.)
        // Path buffer persists across batches to accumulate path data for all pixels
        if clear_paths {
            self.buffers.clear_paths(encoder);
        }

        // Rebuild analytic-blur kernels + (re)allocate the low-res buffers if
        // the view (zoom/rotation) or flame changed. MUST run before the main
        // dispatch below, which splats into the low-res buffer this sizes.
        self.maybe_rebuild_blur_kernels(device, queue, zoom, rotation);

        // Run init dispatch if any active variation has wgsl_init AND params
        // have changed since last dispatch. The init pipeline writes derived
        // values into slots N..N+M of the variation_params buffer; the main
        // pass then reads them via get_param() like any other slot.
        if self.init_dirty {
            if let Some(init_pipeline) = self.pipelines.shader_cache.init_pipeline.as_ref() {
                let pair_count = self.pipelines.shader_cache.init_pair_count;
                if pair_count > 0 {
                    let workgroup_count = (pair_count + 63) / 64;
                    let mut init_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("Variation Init Pass"),
                        timestamp_writes: None,
                    });
                    init_pass.set_pipeline(init_pipeline);
                    init_pass.set_bind_group(0, &self.init_bind_group, &[]);
                    init_pass.dispatch_workgroups(workgroup_count, 1, 1);
                    drop(init_pass);
                }
            }
            self.init_dirty = false;
        }

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Flame Compute Pass"),
            timestamp_writes: None,
        });

        // Select pipeline based on render mode
        let pipeline = self.pipelines.get_trajectory_pipeline(self.current_render_mode);

        compute_pass.set_pipeline(pipeline);
        compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
        compute_pass.dispatch_workgroups(num_workgroups, 1, 1);

        drop(compute_pass);

        samples_this_frame
    }

    /// Run adjust scale pass to dynamically adjust per-pixel scales based on density
    /// This prevents overflow in high-density areas and maximizes precision in low-density areas
    // Note: adjust_scale_pass() removed - pipeline unused

    /// Run accumulation pass to blend new samples with previous accumulation
    pub fn accumulate_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, device: &Device, samples_this_frame: u64) {
        self.shade_dirty = true;
        self.dof_dirty = true;
        self.samples_accumulated += samples_this_frame;

        // Update effective_iterations (for brightness) only when NOT in overwrite mode
        // This prevents brightness flash when exiting overwrite/preview mode
        if !self.overwrite_mode {
            self.effective_iterations += samples_this_frame;
        }

        // Track what's actually in the accumulation buffer right now,
        // since the shader's overwrite branch (blend_factor ≥ 0.99)
        // discards `prev` before adding this dispatch's contribution.
        // Without this, sample_density (which the tonemap uses) would
        // reflect cumulative compute work while bucket_count in the
        // shader only reflects one frame — preview-mode brightness
        // would drop ~N× across N drag frames.
        if self.overwrite_mode {
            self.samples_in_buffer = samples_this_frame;
            self.last_batch_samples = samples_this_frame;
        } else {
            self.samples_in_buffer += samples_this_frame;
            self.last_batch_samples = samples_this_frame;
        }

        // Pick blend mode + rate for the accumulate shader. See
        // docs/projects/accumulator-unification.md and the comment on
        // `AccumulateParams::use_fixed_ema` in gpu/buffers.rs.
        //   - overwrite (slider drag): blend_factor=1.0 triggers the
        //     shader's clear-prev branch; mode is irrelevant.
        //   - use_dynamic_blend=true (default): pure cumulative-mean.
        //     blend_factor unused.
        //   - use_dynamic_blend=false: fixed EMA at user's blend_factor.
        //     Dim early frames as the EMA bootstraps from 0; settles
        //     to a stable steady-state. Precision-stable indefinitely
        //     (each batch contributes a constant proportion regardless
        //     of total sample count). Use ~0.001 for high-quality
        //     renders past ~10^9 iters/pixel where cumulative-mean
        //     hits f32's precision floor.
        let (blend_factor, use_fixed_ema) = if self.overwrite_mode {
            (1.0, 0u32)
        } else if self.use_dynamic_blend {
            (0.0, 0u32)
        } else {
            (self.blend_factor, 1u32)
        };

        let params = AccumulateParams {
            width: self.width,
            height: self.height,
            blend_factor,
            use_fixed_ema,
            background_r: self.background_color[0],
            background_g: self.background_color[1],
            background_b: self.background_color[2],
            _pad1: 0.0,
            surface_thickness: self.surface_thickness,
            has_depth: u32::from(
                self.buffers.solid_depth_region && self.buffers.accum_depth_buffer.is_some(),
            ),
            _pad2: [0; 2],
        };

        self.buffers.update_accumulate_params(queue, &params);

        // Analytic-blur — fold each transform's low-res mean-splat blur into
        // the main histogram, before the spatial filter + accumulate so the
        // convolved blur is treated exactly like the direct samples already in
        // the histogram. Two cheap stages (the chaos game already splatted
        // straight to low res):
        //   1. convolve each low-res slice with its kernel,
        //   2. cubic-upscale + dithered-add into the main histogram (÷D² energy).
        // Gated on a live buffer (count > 0).
        if self.buffers.blur_buffer_count > 0 {
            // Advance the per-frame dither seed (offset 24 B = the `frame_seed`
            // field) so the upscale's stochastic rounding varies each frame and
            // averages out across accumulation → band-free, no residual noise.
            self.blur_frame_seed = self.blur_frame_seed.wrapping_add(0x9E3779B9);
            queue.write_buffer(
                &self.buffers.blur_convolve_params_buffer,
                24,
                bytemuck::bytes_of(&self.blur_frame_seed),
            );

            let low_x = (self.blur_lowres_w + 7) / 8;
            let low_y = (self.blur_lowres_h + 7) / 8;
            let full_x = (self.width + 7) / 8;
            let full_y = (self.height + 7) / 8;

            // (No downsample stage — the chaos game splatted means straight to
            // the low-res buffer, which is the convolve input.)
            let mut conv = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Analytic Blur Convolve"),
                timestamp_writes: None,
            });
            conv.set_pipeline(&self.pipelines.blur_convolve_pipeline);
            conv.set_bind_group(0, &self.blur_convolve_bind_group, &[]);
            conv.dispatch_workgroups(low_x, low_y, 1);
            drop(conv);

            let mut up = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Analytic Blur Upscale"),
                timestamp_writes: None,
            });
            up.set_pipeline(&self.pipelines.blur_upscale_pipeline);
            up.set_bind_group(0, &self.blur_upscale_bind_group, &[]);
            up.dispatch_workgroups(full_x, full_y, 1);
            drop(up);
        }

        // Spatial filter — Gaussian blur on the per-batch histogram (Apo's
        // `filter` attribute). Two separable passes (H then V) immediately
        // before the accumulate dispatch, so accumulate reads the filtered
        // histogram unchanged. Skipped entirely when radius is 0.
        if self.filter_radius > 0.0 {
            // Bilateral `σ_d` scales with the per-batch typical histogram
            // density value. Histogram density stores `count × 100`
            // (HISTOGRAM_COLOR_SCALE), so the typical value at a
            // mean-density pixel is `samples_per_pixel × 100`. Symmetric
            // exponential mapping centered at slider 0.5 = "σ_d at mean
            // density":
            //   blur_edges = 0   → σ_d = mean / 100   (tight — preserves
            //                                          most non-uniform
            //                                          pixels including
            //                                          midtones)
            //   blur_edges = 0.5 → σ_d = mean         (moderate — only
            //                                          well-above-mean
            //                                          pixels preserved)
            //   blur_edges = 1   → σ_d = mean × 100   (loose — only
            //                                          extreme outliers
            //                                          preserved, close
            //                                          to uniform blur)
            const HISTOGRAM_COLOR_SCALE: f32 = 100.0;
            let pixel_count = (self.width as f32) * (self.height as f32);
            let mean_density_scaled = (samples_this_frame as f32 / pixel_count) * HISTOGRAM_COLOR_SCALE;
            let density_sigma = mean_density_scaled * 100.0_f32.powf(2.0 * self.filter_blur_edges - 1.0);
            self.buffers.update_histogram_blur_params(queue, self.width, self.height, self.filter_radius, density_sigma);

            let workgroups_x = (self.width + 7) / 8;
            let workgroups_y = (self.height + 7) / 8;

            let mut h_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Histogram Blur Pass (H)"),
                timestamp_writes: None,
            });
            h_pass.set_pipeline(&self.pipelines.histogram_blur_pipeline);
            h_pass.set_bind_group(0, &self.histogram_blur_h_bind_group, &[]);
            h_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            drop(h_pass);

            let mut v_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Histogram Blur Pass (V)"),
                timestamp_writes: None,
            });
            v_pass.set_pipeline(&self.pipelines.histogram_blur_pipeline);
            v_pass.set_bind_group(0, &self.histogram_blur_v_bind_group, &[]);
            v_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            drop(v_pass);
        }

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Accumulation Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.pipelines.accumulate_pipeline);
        compute_pass.set_bind_group(0, &self.accumulate_bind_group, &[]);

        // Dispatch one thread per 8x8 tile
        let workgroups_x = (self.width + 7) / 8;
        let workgroups_y = (self.height + 7) / 8;
        compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);

        drop(compute_pass);

        // Swap textures for next frame
        self.buffers.swap_textures();

        // Recreate bind groups to point to the new current/previous textures
        self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
        // adjust_scale_bind_group removed - pipeline unused
        self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);
    }

    /// Refresh the sample-count-dependent fields of the tonemap params
    /// uniform. Must run every frame before `tonemap_pass` — otherwise
    /// `sample_density` stays frozen at whatever it was when
    /// `update_tonemap` last fired (config load / user interaction)
    /// and the Ember-style scale-invariant formula breaks: density
    /// keeps growing while k2 stays put, so brightness drifts up
    /// with sample count and `iterations_per_thread` becomes a
    /// brightness knob instead of a speed knob.
    ///
    /// Cheap — a 4-byte uniform write at the offset of
    /// `TonemapParams::sample_density`. See
    /// docs/projects/accumulator-unification.md, Phase 8a.
    fn refresh_sample_density(&self, queue: &Queue) {
        let total_pixels = (self.width as f32) * (self.height as f32);
        // Floor at 1e-6 (defensive non-zero, prevents `k2 = 1/0` in
        // the tonemap shader). Was `.max(1.0)`, which clamped
        // sample_density at ~"1 iter per pixel" — fine in steady
        // state but artificially dimmed early frames at high
        // resolution: 4K preview-mode at frame 1 has ~0.002
        // iters/pixel, clamping to 1.0 multiplied k2's denominator
        // 500× and made the image 500× dimmer than the same flame
        // at steady state. The clamp also broke the scale-invariance
        // promise (sample_density should track total_iterations
        // linearly through any iter count); 1e-6 is small enough to
        // never bind in practice.
        // Solid brightness renormalization: hard occlusion culls most
        // dispatched samples; scale the normalization density by the
        // MEASURED accepted fraction (1.0 when solid is off) so solids
        // tone-map at the brightness their surviving samples deserve.
        let sample_density = ((self.samples_in_buffer as f32) * self.solid_density_fraction
            / total_pixels.max(1.0))
            .max(1e-6);
        let offset = std::mem::offset_of!(TonemapParams, sample_density) as u64;
        queue.write_buffer(
            &self.buffers.tonemap_params_buffer,
            offset,
            bytemuck::bytes_of(&sample_density),
        );
    }

    /// Render the accumulation buffer to internal fractal texture with tone mapping
    pub fn tonemap_pass(&self, queue: &Queue, encoder: &mut CommandEncoder) {
        self.refresh_sample_density(queue);
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Tonemap Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &self.fractal_texture_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.pipelines.tonemap_pipeline);
        render_pass.set_bind_group(0, &self.tonemap_bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Fullscreen triangle

        drop(render_pass);
    }

    /// Render with tone mapping using a custom input texture (for density effects)
    ///
    /// This creates a temporary bind group with the provided input texture instead of
    /// the accumulation texture. Used when density effects have processed the accumulation
    /// data and we need to tonemap their output.
    pub fn tonemap_pass_with_input(&self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder, input_view: &TextureView) {
        self.refresh_sample_density(queue);
        // Create a temporary bind group with the custom input texture
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Tonemap Bind Group (Density Effect Input)"),
            layout: &self.pipelines.tonemap_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(input_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.buffers.sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.buffers.tonemap_params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&self.buffers.curve_lut_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::Sampler(&self.buffers.curve_lut_sampler),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: self.buffers.get_path_buffer_for_binding().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: BindingResource::TextureView(&self.buffers.palette_view),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: BindingResource::Sampler(&self.buffers.sampler),
                },
            ],
        });

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Tonemap Pass (Density Effect Input)"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &self.fractal_texture_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.pipelines.tonemap_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Fullscreen triangle

        drop(render_pass);
    }

    /// Get the current accumulation texture view (for density effects input)
    pub fn get_accumulation_view(&self) -> &TextureView {
        self.buffers.current_accumulation_view()
    }

    /// Solid-rendering shade pass (Phase 1). Dispatches when lighting is
    /// active and the depth region exists; returns whether it ran (the
    /// caller then feeds `shade_output_view()` to density effects /
    /// tonemap instead of the accumulator). Zero cost when off — nothing
    /// is dispatched.
    #[allow(clippy::too_many_arguments)]
    pub fn run_shade_pass(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        zoom: f32,
        rotation: f32,
        pan_x: f32,
        pan_y: f32,
        camera_rotation_x: f32,
        camera_rotation_y: f32,
        camera_bank: f32,
        camera_x: f32,
        camera_y: f32,
        camera_z: f32,
    ) -> bool {
        let lit = self.solid_shading.active()
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD)
            && self.buffers.solid_depth_region;
        if !lit {
            return false;
        }
        // Inputs unchanged and the temporal blend has settled: the
        // previous shade output is still exact — skip the whole chain.
        if !self.shade_dirty && self.shade_settle == 0 {
            return true;
        }
        if self.shade_dirty {
            self.shade_dirty = false;
            // ~2 blend time-constants at ema 0.85.
            self.shade_settle = 16;
        } else {
            self.shade_settle -= 1;
        }
        // The shade output is about to change (settling counts too) —
        // any DoF built on it is stale.
        self.dof_dirty = true;
        let sh_fit = self.frozen_shadow_fit.unwrap_or_else(|| self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, [camera_x, camera_y, camera_z]));
        self.shade_pass.run(
            device,
            queue,
            encoder,
            self.buffers.current_accumulation_view(),
            &self.buffers.histogram_buffer,
            &self.solid_shading,
            zoom,
            rotation,
            pan_x,
            pan_y,
            self.perspective_strength,
            self.surface_thickness,
            (camera_rotation_x, camera_rotation_y, camera_bank, [camera_x, camera_y, camera_z]),
            if self.shadow_capture_wanted() && self.buffers.solid_depth_region {
                Some((self.buffers.shadow_map_word_offset(), sh_fit.0, sh_fit.1))
            } else {
                None
            },
            // Post-lighting fog (owned by the shade pass when active).
            if self.fog_in_shade() {
                (self.fog_strength, self.fog_start,
                 [self.background_r, self.background_g, self.background_b])
            } else {
                (0.0, 0.0, [0.0; 3])
            },
            // Temporal smoothing of the shade output: the shading tracks
            // genuinely drifting data during accumulation; raw per-frame
            // it strobes. Overwrite mode gets 0 — every frame is a fresh
            // single-batch preview.
            if self.overwrite_mode { 0.0 } else { 0.85 },
        );
        true
    }

    /// Shaded output view (valid after `run_shade_pass` returned true).
    pub fn shade_output_view(&self) -> &TextureView {
        self.shade_pass.output_view()
    }

    /// Post-process depth of field (solid mode). At-splat DoF is
    /// compiled out under SOLID (position jitter corrupts the
    /// nearest-depth buffer); this gather blur replaces it, running on
    /// the HDR pre-tonemap image (shade output when lighting is on,
    /// else the accumulator) using the depth region. Returns whether
    /// the DoF output should feed the tonemap. Zero cost when off.
    pub fn run_dof_pass(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        shade_ran: bool,
        zoom: f32,
    ) -> bool {
        let active = self.dof_blur_strength > 0.0
            && self.buffers.solid_depth_region
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD);
        if !active {
            return false;
        }
        if !self.dof_dirty {
            // Input unchanged — the previous output is still exact.
            return true;
        }
        self.dof_dirty = false;
        let input: &TextureView = if shade_ran {
            self.shade_pass.output_view()
        } else {
            self.buffers.current_accumulation_view()
        };
        self.dof_pass.run(
            device,
            queue,
            encoder,
            input,
            &self.buffers.histogram_buffer,
            self.width * self.height * 4, // depth region inside the histogram
            self.width,
            self.height,
            zoom,
            self.dof_focus_distance,
            self.dof_blur_strength,
            self.surface_thickness,
        );
        true
    }

    /// DoF output view (valid after `run_dof_pass` returned true).
    pub fn dof_output_view(&self) -> &TextureView {
        self.dof_pass.output_view()
    }

    /// Lightweight lighting update: refresh the shade-pass settings
    /// WITHOUT touching iteration state. Only valid when the change
    /// doesn't flip the depth-capture requirement — the caller checks
    /// `has_solid_depth_region()` against the desired state and
    /// escalates to `update_flame` when they differ.
    pub fn set_solid_shading(&mut self, shading: crate::config::SolidShadingSettings) {
        self.solid_shading = shading;
        // Lighting changed: blending the new look against the old one
        // would lag/ghost the edit — restart the temporal history.
        self.shade_pass.reset_temporal();
        self.shade_dirty = true;
        self.dof_dirty = true;
    }

    /// Whether the histogram currently carries the solid depth region.
    pub fn has_solid_depth_region(&self) -> bool {
        self.buffers.solid_depth_region
    }


    /// Interactive shadow-fit auto-refit: re-freeze (and report true so
    /// the caller resets accumulation) ONLY when the measured attractor
    /// extends beyond the frozen fit's coverage — clipped geometry casts
    /// no shadow, so growth is a correctness refit. Shrinkage NEVER
    /// refits: an oversized map merely wastes resolution, and every
    /// natural reset re-freezes from the latest bounds anyway (free
    /// tightening). A symmetric "changed by >10%" test here caused an
    /// infinite reset loop (field-reported at ~250M iterations): each
    /// reset clears the bounds tail, the fresh run's PARTIAL AABB reads
    /// as a material shrink, refit resets again, ad infinitum.
    #[allow(clippy::too_many_arguments)]
    pub fn maybe_refit_shadow(&mut self, zoom: f32, pan_x: f32, pan_y: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_pos: [f32; 3]) -> bool {
        if !self.bounds_dirty || !self.shadow_capture_wanted() {
            return false;
        }
        self.bounds_dirty = false;
        // At most ONE auto-refit per accumulation run, and only while
        // the run is young: a refit restarts accumulation — invisible
        // at ~1 s in, enraging at 20 min in. Later growth keeps the
        // current maps (worst case: an off-fit tail renders unshadowed
        // this run; the next natural reset re-freezes from the latest
        // bounds anyway).
        if self.fit_refit_done {
            return false;
        }
        let batches = self.samples_in_buffer / self.last_batch_samples.max(1);
        if batches > 64 {
            return false;
        }
        let new_fit = self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, camera_pos);
        let needs = match self.frozen_shadow_fit {
            // A view-guess fit tightens ONCE when the first real
            // measurement lands (can't loop: the refit below marks the
            // fit measured, and measured fits only refit on growth).
            Some(_) if !self.frozen_fit_measured => true,
            Some((c, r)) => {
                // New bounding sphere not contained in the frozen one
                // (5% hysteresis so boundary noise can't retrigger).
                let d = ((c[0] - new_fit.0[0]).powi(2)
                    + (c[1] - new_fit.0[1]).powi(2)
                    + (c[2] - new_fit.0[2]).powi(2))
                    .sqrt();
                d + new_fit.1 > r * 1.05
            }
            None => true,
        };
        if needs {
            log::info!(
                "Shadow auto-refit (once per run): {:?} -> {:?} (restarting accumulation)",
                self.frozen_shadow_fit, new_fit
            );
            self.frozen_shadow_fit = Some(new_fit);
            // bounds_dirty implied a fresh measurement, so the new fit
            // is measurement-derived.
            self.frozen_fit_measured = self.measured_bounds.is_some();
            self.fit_refit_done = true;
        }
        needs
    }

    /// Light-space shadow-map fit: cover the measured attractor bounds
    /// (bounding-sphere radius) — falls back to a view-derived guess
    /// until the first measurement lands. Frozen per accumulation run.
    ///
    /// The measured AABB is CLAMPED to a window of +/-12 view-extents
    /// around the view center before the fit is derived. Chaos-game
    /// attractors are heavy-tailed: rare far excursions keep growing
    /// the raw min/max AABB for as long as the render runs, which both
    /// starves the maps of resolution (one outlier at 1e6 spreads 1024^2
    /// texels over nothing) and made the growth-refit re-trigger
    /// forever (field-reported repeat resets). Inside the window the
    /// fit is exact; outliers beyond it render with clamped-edge
    /// shadow coverage — invisible, since they are far off-view too.
    #[allow(clippy::too_many_arguments)]
    fn shadow_placement(&self, zoom: f32, pan_x: f32, pan_y: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_pos: [f32; 3]) -> ([f32; 3], f32) {
        let rows = crate::renderer::shade_pass::effective_camera_rows(
            camera_rotation_x, camera_rotation_y, camera_bank);
        let mut view_center = camera_pos;
        for i in 0..3 {
            view_center[i] += pan_x * rows[0][i] + pan_y * rows[1][i];
        }
        let aspect = self.width.max(self.height).max(1) as f32
            / self.width.min(self.height).max(1) as f32;
        let view_extent = (4.0 * aspect / zoom.max(1e-6)).clamp(1e-3, 1e6);
        if let Some((mn, mx)) = self.measured_bounds {
            let win = 12.0 * view_extent;
            let mut cmn = [0.0f32; 3];
            let mut cmx = [0.0f32; 3];
            for i in 0..3 {
                cmn[i] = mn[i].clamp(view_center[i] - win, view_center[i] + win);
                cmx[i] = mx[i].clamp(view_center[i] - win, view_center[i] + win);
            }
            let c = [
                (cmn[0] + cmx[0]) * 0.5,
                (cmn[1] + cmx[1]) * 0.5,
                (cmn[2] + cmx[2]) * 0.5,
            ];
            let dx = cmx[0] - cmn[0];
            let dy = cmx[1] - cmn[1];
            let dz = cmx[2] - cmn[2];
            let r = ((dx * dx + dy * dy + dz * dz).sqrt() * 0.5 * 1.1).max(1e-3);
            return (c, r);
        }
        (view_center, view_extent)
    }

    /// The renderer's CURRENT shading settings (pre-update state — used
    /// by gpu_updates to detect shadow-relevant light changes).
    pub fn solid_shading(&self) -> &crate::config::SolidShadingSettings {
        &self.solid_shading
    }

    /// Whether the SHADE pass owns depth fog (post-lighting) instead of
    /// the at-splat path: fog + active lighting + 3D. The GpuParams
    /// writes zero the at-splat fog when true; flipping this needs an
    /// accumulation reset (gpu_updates escalates it) because at-splat
    /// fog is baked into accumulated samples.
    fn fog_in_shade(&self) -> bool {
        self.fog_strength > 0.0
            && self.solid_shading.active()
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD)
    }

    /// At-splat fog strength for GpuParams: zero when the shade pass
    /// owns fog.
    fn atsplat_fog_strength(&self) -> f32 {
        if self.fog_in_shade() { 0.0 } else { self.fog_strength }
    }

    /// Whether shadow maps should capture for the current settings.
    pub fn shadow_capture_wanted(&self) -> bool {
        self.solid_shading.shadow_strength > 0.0
            && (self.solid_strength > 0.0 || self.solid_shading.active())
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD)
            && self.solid_shading.lights.iter().any(|l| l.enabled && l.intensity > 0.0)
    }

    /// Per-slot world-space light directions (xyz; w = enabled) plus the
    /// runtime shadow_count gate (4 when capturing, 0 otherwise). Slot
    /// order matches the lights array so the shade lookup maps
    /// light i ↔ map i directly.
    fn shadow_light_dirs(&self, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32) -> (u32, [[f32; 4]; 4]) {
        let mut dirs = [[0.0f32; 4]; 4];
        if !self.shadow_capture_wanted() {
            return (0, dirs);
        }
        let rows = crate::renderer::shade_pass::effective_camera_rows(
            camera_rotation_x, camera_rotation_y, camera_bank);
        for (i, l) in self.solid_shading.lights.iter().enumerate().take(4) {
            if !(l.enabled && l.intensity > 0.0) {
                continue;
            }
            let az = l.azimuth.to_radians();
            let el = l.elevation.to_radians();
            // Camera-space direction TO the light (same formula as the
            // shade pass), rotated to world by E^T.
            let c = [el.cos() * az.sin(), el.sin(), el.cos() * az.cos()];
            let mut w = [0.0f32; 3];
            for k in 0..3 {
                w[k] = c[0] * rows[0][k] + c[1] * rows[1][k] + c[2] * rows[2][k];
            }
            dirs[i] = [w[0], w[1], w[2], 1.0];
        }
        (4, dirs)
    }

    /// Byte offset of the attractor-bounds tail in the histogram buffer
    /// (only meaningful when the depth region exists).
    fn bounds_tail_offset(&self) -> u64 {
        (self.width as u64) * (self.height as u64) * 5 * 4
    }

    /// Blocking bounds measurement + shadow-fit refresh (export warmup:
    /// run a few batches, call this, reset, render for real — the maps
    /// then cover the actual flame). Returns true when the fit changed
    /// materially.
    #[allow(clippy::too_many_arguments)]
    pub fn refresh_shadow_placement_blocking(&mut self, device: &Device, queue: &Queue, zoom: f32, pan_x: f32, pan_y: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_pos: [f32; 3]) -> bool {
        if !(self.shadow_capture_wanted() && self.buffers.solid_depth_region) {
            return false;
        }
        let off = self.bounds_tail_offset();
        if let Some(words) = self.bounds_stats.read_blocking(device, queue, &self.buffers.histogram_buffer, off) {
            match crate::renderer::density_stats::decode_bounds(&words) {
                Some(b) => {
                    log::debug!("Attractor bounds measured: min {:?} max {:?}", b.0, b.1);
                    self.measured_bounds = Some(b);
                }
                None => log::debug!("Attractor bounds not available yet"),
            }
        }
        let new_fit = self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, camera_pos);
        let changed = match self.frozen_shadow_fit {
            Some((c, r)) => {
                let dc = (c[0] - new_fit.0[0]).abs().max((c[1] - new_fit.0[1]).abs()).max((c[2] - new_fit.0[2]).abs());
                dc > r * 0.05 || (r - new_fit.1).abs() > r * 0.05
            }
            None => true,
        };
        self.frozen_shadow_fit = Some(new_fit);
        self.frozen_fit_measured = self.measured_bounds.is_some();
        self.fit_refit_done = false;
        changed
    }

    /// Turn on the reachability census for this renderer: the next
    /// shader build carries `ShaderConstants::census`, the histogram
    /// grows the counter tail, and the bind groups that reference it
    /// are recreated. Call after construction and before `load_config`.
    /// v1 refuses solid (see the census module docs).
    pub fn enable_census(&mut self, device: &Device) {
        assert!(
            !self.buffers.solid_depth_region,
            "census excludes solid rendering (v1)"
        );
        self.census = true;
        self.buffers.set_census_region(device, true);
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
        self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
        self.histogram_blur_h_bind_group = self.pipelines.create_histogram_blur_h_bind_group(device, &self.buffers);
        self.histogram_blur_v_bind_group = self.pipelines.create_histogram_blur_v_bind_group(device, &self.buffers);
    }

    /// Zero the census tail. The tail is deliberately excluded from
    /// every ordinary clear (counters accumulate across the run), which
    /// leaves buffer-creation zeroing as its only initializer — and
    /// that is a guarantee about logical buffers, not about recycled
    /// allocations behaving well under destroy/create cycles. A corpus
    /// sweep that creates one renderer per flame on a shared device
    /// showed cross-flame count contamination; single flames on a fresh
    /// device were bit-clean. Call once before the first census pass.
    pub fn clear_census_tail(&self, encoder: &mut CommandEncoder) {
        if self.buffers.census_region {
            let rgbd =
                (self.width as u64) * (self.height as u64) * 4 * std::mem::size_of::<u32>() as u64;
            encoder.clear_buffer(&self.buffers.histogram_buffer, rgbd, None);
        }
    }

    /// Copy the census tail off the GPU and return its
    /// `census::TOTAL_WORDS` words. Blocking; census tooling only.
    pub fn read_census_blocking(&self, device: &Device, queue: &Queue) -> Option<Vec<u32>> {
        if !self.buffers.census_region {
            return None;
        }
        let bytes = (crate::census::TOTAL_WORDS * std::mem::size_of::<u32>()) as u64;
        let offset =
            (self.width as u64) * (self.height as u64) * 4 * std::mem::size_of::<u32>() as u64;
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("Census Readback"),
            size: bytes,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Census Readback"),
        });
        enc.copy_buffer_to_buffer(&self.buffers.histogram_buffer, offset, &staging, 0, bytes);
        queue.submit(std::iter::once(enc.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        rx.recv().ok()?.ok()?;
        let words: Vec<u32> = {
            let data = slice.get_mapped_range();
            bytemuck::cast_slice(&data).to_vec()
        };
        staging.unmap();
        // Explicit, because dropping frees nothing on WebGPU.
        staging.destroy();
        Some(words)
    }

    /// Interactive-path density-stats tick (solid brightness renorm):
    /// pumps the async measurement and encodes a fresh reduction every N
    /// frames while occlusion is actively culling. Call once per frame,
    /// after accumulate and before tonemap.
    pub fn update_density_stats(&mut self, device: &Device, _queue: &Queue, encoder: &mut CommandEncoder) {
        // Attractor-bounds tick (shadow-map auto-fit): async readback of
        // the 8-word tail. Applied to `measured_bounds` immediately; the
        // frozen fit only picks it up at the next reset (frozen per run).
        let mut occ_words: Option<(u32, u32)> = None;
        if self.buffers.solid_depth_region {
            let off = self.bounds_tail_offset();
            if let Some(words) = self.bounds_stats.tick(device, encoder, &self.buffers.histogram_buffer, off) {
                if let Some(b) = crate::renderer::density_stats::decode_bounds(&words) {
                    self.measured_bounds = Some(b);
                    self.bounds_dirty = true;
                }
                occ_words = Some((words[6], words[7]));
            }
        }
        let active = self.solid_strength > 0.0
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD)
            && self.buffers.solid_depth_region;
        if !active {
            self.solid_density_fraction = 1.0;
            return;
        }
        // OCCLUSION-ONLY survival fraction from the tail counters (see
        // main_template.wgsl) — never accumulated density, which folds
        // artistic per-sample weights (far-fade, depth-density comp)
        // into the "culled" fraction and makes those dials shift
        // global brightness (field-audited).
        if let Some((num, den)) = occ_words {
            // The counters saturate around 17B iterations; freeze the
            // fraction rather than let a saturated ratio drift.
            if den > 0 && den < 3_000_000_000 {
                let measured = (num as f32 / den as f32).clamp(0.005, 1.0);
                // EMA: brightness scalar — smooth over the measurement
                // lag so fraction drift never pumps the image.
                self.solid_density_fraction = self.solid_density_fraction * 0.7 + measured * 0.3;
            }
        }
    }

    /// Exact (blocking) density-fraction measurement for one-shot renders
    /// (CLI export) — sets the fraction the final tonemap will use.
    pub fn apply_exact_density_fraction(&mut self, device: &Device, queue: &Queue) {
        let active = self.solid_strength > 0.0
            && matches!(self.current_render_mode, crate::scene::transforms::RenderMode::ThreeD)
            && self.buffers.solid_depth_region;
        if !active {
            self.solid_density_fraction = 1.0;
            return;
        }
        let off = self.bounds_tail_offset();
        if let Some(words) = self.bounds_stats.read_blocking(device, queue, &self.buffers.histogram_buffer, off) {
            let (num, den) = (words[6], words[7]);
            if den > 0 {
                self.solid_density_fraction = (num as f32 / den as f32).clamp(0.005, 1.0);
                log::info!(
                    "solid brightness renorm: occlusion survival = {:.4}",
                    self.solid_density_fraction
                );
            }
        }
    }

    /// Debug: Read back scale buffer and compute statistics
    // Note: debug_scale_stats() removed - scale_buffer no longer exists

    /// Load a complete FractalConfig (preset or imported config)
    /// This ensures all GPU state is properly synchronized
    pub fn load_config(&mut self, device: &Device, encoder: &mut CommandEncoder, queue: &Queue, config: &FractalConfig, palette: &Palette, iterations_per_thread: u32, burn_in: u32) {
        // Sticky superset (Layer B): adopt this flame's variations and
        // shadow `config` with a clone whose flame carries the retained
        // extras at weight 0. Everything below — shader cache, constants,
        // buffer packing, subflame map, init shader — reads the augmented
        // flame, so the compiled map and the packed offsets cannot
        // disagree. A fresh renderer has an empty sticky set, so one-shot
        // paths (CLI export, visual tests, probe, census) see a plain
        // clone and compile specialized, unchanged.
        let sticky_config;
        let config: &FractalConfig = if self.sticky.enabled() {
            let mut c = config.clone();
            c.flame = self.sticky.adopt(&config.flame);
            sticky_config = c;
            &sticky_config
        } else {
            config
        };
        // 0. Check if shaders need to be recompiled (variations or constants changed)
        // Determine if path features are needed (PathMap mode or path filters active)
        let path_features_enabled = config.color_mode == ColorMode::PathMap
            || !self.path_filters.is_empty();
        let shaders_changed = self.pipelines.ensure_shaders_current_with_config(device, config, path_features_enabled, self.census);
        if shaders_changed {
            log::info!("Shaders recompiled during preset load - recreating bind group");
            // Recreate compute bind group with new pipeline
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
            self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
        }

        // 1. Update transforms and variation parameters in GPU buffer
        self.buffers.update_transforms(queue, &config.flame, config.render_mode);
        self.buffers.update_variation_params(queue, &config.flame);
        self.buffers.update_attachments(queue, &config.flame, config.flame.attachment_cap());
        // Pack subflames against the same local_map the parent transforms used.
        // `get_id_mapping()` returns the union map (extract_active_variations
        // already recurses into subflames), so parent and subflame xforms see
        // consistent variation indices.
        if let Err(e) = self.buffers.update_subflames(
            queue,
            &config.flame.subflames,
            &config.flame.get_id_mapping(),
        ) {
            log::error!("Failed to update subflames: {}", e);
        }
        self.init_dirty = true;

        // 1b. Update xaos buffer (create/drop as needed)
        let xaos_buffer_changed = self.update_xaos_buffer(device, queue, &config.flame);
        // 1c. Refresh analytic-blur slot list (buffers (re)allocate in
        // maybe_rebuild_blur_kernels on the next compute_pass).
        self.update_blur_buffers(&config.flame);
        if xaos_buffer_changed {
            // Recreate bind group with new xaos buffer
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
            self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
        }

        // 2. Update color mode, path map style, and capture mode
        self.color_mode = config.color_mode;
        self.path_map_style = config.path_map_style;
        self.path_capture_mode = config.path_capture_mode;
        self.path_tracking_mode = config.path_tracking_mode;

        // 3. Update density and background
        self.density_scale = config.density_scale;
        self.white_level = config.white_level;
        self.highlight_mode = match config.highlight_mode {
            crate::scene::tonemap::HighlightMode::Clip => 0,
            crate::scene::tonemap::HighlightMode::MaxNorm => 1,
            crate::scene::tonemap::HighlightMode::Reinhard => 2,
            crate::scene::tonemap::HighlightMode::Filmic => 3,
        };
        self.background_color = config.background_color;

        // 4. Update render mode and perspective
        self.current_render_mode = config.render_mode;
        self.preserve_z = config.preserve_z;
        self.perspective_strength = config.perspective_strength;
        self.depth_density_compensation = config.depth_density_compensation;
        self.far_density_fade = config.far_density_fade;
        self.far_density_fade_start = config.far_density_fade_start;
        self.dof_focus_distance = config.dof_focus_distance;
        self.dof_blur_strength = config.dof_blur_strength;
        self.fog_strength = config.fog_strength;
        self.fog_start = config.fog_start;
        self.filter_radius = config.filter_radius;
        self.filter_blur_edges = config.filter_blur_edges;
        self.post_symmetry = config.flame.post_symmetry.clone();
        self.burn_in = burn_in;

        // 4b. Solid rendering: sync state + histogram depth region.
        // Depth capture activates for occlusion OR lighting (see update_flame).
        self.solid_strength = config.solid_strength;
        self.surface_thickness = config.surface_thickness;
        self.solid_shading = config.solid_shading.clone();
        // load_config is a full reset point: drop the shade temporal
        // history and force a fresh shade. The VIDEO EXPORT path loads a
        // config per frame WITHOUT calling reset() — with the history
        // kept, the temporal blend mixed ~85% of the PREVIOUS animation
        // frame into each new one (field-reported smearing/trails on
        // moving fractals; in-app was immune because interactive motion
        // runs in overwrite mode where the blend is disabled).
        self.shade_pass.reset_temporal();
        self.shade_dirty = true;
        self.dof_dirty = true;
        let solid_enabled = (config.solid_strength > 0.0 || self.solid_shading.active())
            && matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD);
        let rebind = self.buffers.set_solid_depth_region(device, solid_enabled);
        if rebind {
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
            self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
            self.histogram_blur_h_bind_group = self.pipelines.create_histogram_blur_h_bind_group(device, &self.buffers);
            self.histogram_blur_v_bind_group = self.pipelines.create_histogram_blur_v_bind_group(device, &self.buffers);
        }
        // load_config resets accumulation — prime depth on the next batch.
        self.needs_depth_prime = solid_enabled;

        // 5. Update palette size (recreates texture + bind groups if changed)
        if self.set_palette_size(device, queue, &config.flame, config.palette_size) {
            log::info!("Palette size changed to {} during config load", config.palette_size);
        }

        // 5b. Update palette with rotation and squeeze
        self.buffers.update_palette(
            queue,
            palette,
            config.palette_rotation,
            config.palette_squeeze,
            config.palette_squeeze_mode,
            config.palette_squeeze_falloff,
            config.palette_log_strength,
            config.palette_reverse,
        );

        // Note: scale_buffer removed - scale is now in params.histogram_color_scale

        // 6. Update ALL GPU params with correct num_transforms, render_mode, perspective

        // Update transform tracking
        self.num_transforms = config.flame.transforms.len() as u32;

        self.frozen_shadow_fit = Some(self.shadow_placement(config.zoom, config.pan_x, config.pan_y, config.camera_rotation_x, config.camera_rotation_y, config.camera_bank, [config.camera_x, config.camera_y, config.camera_z]));
        self.frozen_fit_measured = self.measured_bounds.is_some();
        self.fit_refit_done = false;
        let sh_fit = self.frozen_shadow_fit.unwrap();
        let sh_dirs = self.shadow_light_dirs(config.camera_rotation_x, config.camera_rotation_y, config.camera_bank);
        let params = GpuParams {
            num_transforms: self.num_transforms,
            iterations_per_thread,
            burn_in,
            width: self.width,
            height: self.height,
            seed: self.get_rng_seed(),
            color_mode: config.color_mode as u32,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
                // The flame renderer never receives an escape config —
                // the app and render_with branch before this point. If
                // one is mis-routed anyway, render it as 2D rather than
                // panicking inside a GPU pass.
                crate::scene::transforms::RenderMode::Escape => 0,
            },
            splat_size: 1.0,
            zoom: config.zoom,
            pan_x: config.pan_x,
            pan_y: config.pan_y,
            rotation: config.rotation,
            speed_factor: config.speed_factor,
            perspective_strength: self.perspective_strength,
            depth_density_compensation: self.depth_density_compensation,
            far_density_fade: self.far_density_fade,
            far_density_fade_start: self.far_density_fade_start,
            solid_strength: self.solid_strength,
            surface_thickness: self.surface_thickness,
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
                0.0
            } else {
                config.fog_strength
            },
            fog_start: config.fog_start,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
            path_tracking_mode: self.path_tracking_mode as u32,
            num_path_filters: self.path_filters.len() as u32,
            min_suffix_filter_length: self.min_suffix_filter_length,
            background_r: config.background_color[0],
            background_g: config.background_color[1],
            background_b: config.background_color[2],
            post_symmetry: (&config.flame.post_symmetry).into(),
            shadow_center_x: sh_fit.0[0],
            shadow_center_y: sh_fit.0[1],
            shadow_center_z: sh_fit.0[2],
            shadow_radius: sh_fit.1,
            shadow_count: sh_dirs.0,
            _pad_shadow: [0; 3],
            shadow_dirs: sh_dirs.1,
        };
        self.buffers.update_params(queue, &params);

        // 7. Update deterministic RNG setting
        self.deterministic_rng = config.deterministic_rng;

        // 8. Update tone mapping settings from config
        // Pass config's levels values through — the previous hardcoded
        // (0.0, 1000.0, 1.0) was the legacy raw-density default and
        // diverged from the live path's update_tonemap (which uses
        // config.levels_*). Headless renders and the in-app viewport
        // would otherwise render the same flame with different Levels
        // settings.
        self.update_tonemap(queue, config.tonemap_mode, config.highlight_mode, config.use_curve, config.exposure, config.gamma, config.gamma_threshold, config.brightness, config.vibrancy, config.white_level, config.saturation, config.hue_shift, config.alpha_blend_low, config.alpha_blend_high, self.width, self.height, self.total_iterations, config.max_iterations, config.zoom, iterations_per_thread, 1, false,
            config.levels_enabled, config.levels_low, config.levels_high, config.levels_gamma);
        self.update_curve_lut(queue, &config.tonemap_curve);

        // 9. Clear accumulation buffers + reset ALL iteration counters
        // (not just samples_accumulated + total_iterations). Leaving
        // samples_in_buffer at its pre-clear value desyncs the next
        // frame's `refresh_sample_density()` from the (now-empty)
        // accumulator: it writes `stale_value / area` into the
        // tonemap uniform, the shader's `apply_levels` divides by
        // that inflated mean, and the image renders ~N× dimmer than
        // it should until samples_in_buffer naturally catches up
        // over the next many frames. Most visible after undo/redo
        // restores a config that load_config processes.
        self.buffers.clear_all(encoder, queue);
        self.reset_iteration_counter();
    }

    /// Update the flame being rendered
    pub fn update_flame(&mut self, device: &Device, queue: &Queue, flame: &Flame, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_x: f32, camera_y: f32, camera_z: f32, speed_factor: f32, dof_focus_distance: f32, dof_blur_strength: f32, fog_strength: f32, fog_start: f32, background_color: [f32; 3], filter_radius: f32, filter_blur_edges: f32, render_mode: crate::scene::transforms::RenderMode, perspective_strength: f32, depth_density_compensation: f32, far_density_fade: f32, far_density_fade_start: f32, preserve_z: bool, solid_strength: f32, surface_thickness: f32, solid_shading: crate::config::SolidShadingSettings) {
        // Sticky superset (Layer B): same shadowing as load_config, for
        // the editor's incremental path — this is what makes toggling a
        // variation off and back on a cache hit instead of two rebuilds.
        let sticky_flame;
        let flame: &Flame = if self.sticky.enabled() {
            sticky_flame = self.sticky.adopt(flame);
            &sticky_flame
        } else {
            flame
        };

        // Solid rendering state must be set BEFORE build_shader_constants
        // below (it feeds ShaderConstants::solid_enabled) and before the
        // histogram depth-region toggle.
        self.solid_strength = solid_strength;
        self.surface_thickness = surface_thickness;
        self.solid_shading = solid_shading;
        // Depth capture activates for occlusion OR lighting: the shade pass
        // needs the depth region even when solid_strength is 0 (gating is a
        // no-op multiplier there), so transparent flames can be lit.
        let solid_enabled = (solid_strength > 0.0 || self.solid_shading.active())
            && matches!(render_mode, crate::scene::transforms::RenderMode::ThreeD);
        // Reset point: re-freeze the shadow fit from the latest measured
        // bounds (splat texel coordinates must not move mid-run).
        self.frozen_shadow_fit = Some(self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, [camera_x, camera_y, camera_z]));
        self.frozen_fit_measured = self.measured_bounds.is_some();
        self.fit_refit_done = false;
        let depth_changed = self.buffers.set_solid_depth_region(device, solid_enabled);
        if depth_changed {
            // Histogram buffer was recreated — every bind group that
            // references it must be too.
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
            self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
            self.histogram_blur_h_bind_group = self.pipelines.create_histogram_blur_h_bind_group(device, &self.buffers);
            self.histogram_blur_v_bind_group = self.pipelines.create_histogram_blur_v_bind_group(device, &self.buffers);
        }
        if depth_changed {
            // Fresh (zeroed) depth region: prime before plotting again.
            self.needs_depth_prime = solid_enabled;
        }

        // Check if shaders need to be recompiled (variations or constants changed)
        let constants = self.build_shader_constants(flame, render_mode, preserve_z);
        let path_features_enabled = self.color_mode == ColorMode::PathMap
            || !self.path_filters.is_empty();
        let shaders_changed = self.pipelines.ensure_shaders_current_with_constants(
            device,
            flame,
            path_features_enabled,
            constants,
            render_mode,
        );
        if shaders_changed {
            log::info!("Shaders recompiled due to variation/constant changes - recreating bind group");
            // Recreate compute bind group with new pipeline
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
            self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
        }

        self.buffers.update_transforms(queue, flame, render_mode);
        self.buffers.update_variation_params(queue, flame);
        self.buffers.update_attachments(queue, flame, flame.attachment_cap());
        if let Err(e) = self.buffers.update_subflames(
            queue,
            &flame.subflames,
            &flame.get_id_mapping(),
        ) {
            log::error!("Failed to update subflames: {}", e);
        }
        self.init_dirty = true;

        // Update xaos buffer (create/drop as needed)
        let xaos_buffer_changed = self.update_xaos_buffer(device, queue, flame);
        // Refresh analytic-blur slot list (buffers (re)allocate in
        // maybe_rebuild_blur_kernels on the next compute_pass).
        self.update_blur_buffers(flame);
        if xaos_buffer_changed {
            // Recreate bind group with new xaos buffer
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
            self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
        }

        // Update render mode, perspective, DOF, fog, and background color
        self.current_render_mode = render_mode;
        self.preserve_z = preserve_z;
        self.perspective_strength = perspective_strength;
        self.depth_density_compensation = depth_density_compensation;
        self.far_density_fade = far_density_fade;
        self.far_density_fade_start = far_density_fade_start;
        self.dof_focus_distance = dof_focus_distance;
        self.dof_blur_strength = dof_blur_strength;
        self.fog_strength = fog_strength;
        self.filter_radius = filter_radius;
        self.filter_blur_edges = filter_blur_edges;
        self.fog_start = fog_start;
        self.background_r = background_color[0];
        self.background_g = background_color[1];
        self.background_b = background_color[2];
        // Mirror per-flame post-symmetry into the renderer cache so
        // the GpuParams construction below picks it up. The shader
        // rebuild for type changes is already handled by
        // ensure_shaders_current_with_constants above (since
        // ShaderConstants.has_post_symmetry shifts).
        self.post_symmetry = flame.post_symmetry.clone();

        // Update transform tracking
        self.num_transforms = flame.transforms.len() as u32;

        let sh_fit = self.frozen_shadow_fit.unwrap_or_else(|| self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, [camera_x, camera_y, camera_z]));
        let sh_dirs = self.shadow_light_dirs(camera_rotation_x, camera_rotation_y, camera_bank);
        let params = GpuParams {
            num_transforms: self.num_transforms,
            iterations_per_thread,
            burn_in,
            width: self.width,
            height: self.height,
            seed: self.get_rng_seed(),
            color_mode: self.color_mode as u32,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
                // The flame renderer never receives an escape config —
                // the app and render_with branch before this point. If
                // one is mis-routed anyway, render it as 2D rather than
                // panicking inside a GPU pass.
                crate::scene::transforms::RenderMode::Escape => 0,
            },
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            perspective_strength: self.perspective_strength,
            depth_density_compensation: self.depth_density_compensation,
            far_density_fade: self.far_density_fade,
            far_density_fade_start: self.far_density_fade_start,
            solid_strength: self.solid_strength,
            surface_thickness: self.surface_thickness,
            depth_prime: 0,
            camera_rotation_x,
            camera_rotation_y,
            camera_bank,

            camera_x,

            camera_y,
            camera_z,
            dof_focus_distance: self.dof_focus_distance,
            dof_blur_strength: self.dof_blur_strength,
            fog_strength: self.atsplat_fog_strength(),
            fog_start: self.fog_start,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
            path_tracking_mode: self.path_tracking_mode as u32,
            num_path_filters: self.path_filters.len() as u32,
            min_suffix_filter_length: self.min_suffix_filter_length,
            background_r: self.background_r,
            background_g: self.background_g,
            background_b: self.background_b,
            post_symmetry: (&self.post_symmetry).into(),
            shadow_center_x: sh_fit.0[0],
            shadow_center_y: sh_fit.0[1],
            shadow_center_z: sh_fit.0[2],
            shadow_radius: sh_fit.1,
            shadow_count: sh_dirs.0,
            _pad_shadow: [0; 3],
            shadow_dirs: sh_dirs.1,
        };

        self.buffers.update_params(queue, &params);
        self.samples_accumulated = 0;
        self.total_iterations = 0;
    }

    /// Update tonemap parameters (exposure, gamma)
    pub fn update_tonemap_params(&self, queue: &Queue, exposure: f32, gamma: f32) {
        use crate::config::defaults::*;
        let area = (self.width * self.height) as f32;
        let sample_density = if area > 0.0 { self.total_iterations as f32 / area } else { 1.0 };

        let params = TonemapParams {
            exposure,
            gamma,
            density_scale: 1.0,
            tonemap_mode: 1,  // Logarithmic
            background_color: [0.0, 0.0, 0.0],
            _pad_bg: 0.0,
            use_curve: 0,  // Disabled
            vibrancy: 1.0,  // Default
            brightness: DEFAULT_BRIGHTNESS,
            white_level: self.white_level,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: DEFAULT_SATURATION,
            hue_shift: DEFAULT_HUE_SHIFT,
            gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
            alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
            alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
            transparent_mode: 0,
            color_mode: self.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: self.path_map_style as u32,
            burn_in: self.burn_in,
            num_transforms: self.num_transforms,
            palette_size: self.buffers.palette_size(),
            levels_low: 0.0,
            levels_high: crate::config::defaults::DEFAULT_LEVELS_HIGH,
            levels_gamma: 1.0,
            highlight_mode: self.highlight_mode,
            levels_enabled: 0,
            _pad_levels: [0; 2],
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Enable or disable the sticky variation superset (Layer B).
    /// Disabling also forgets the retained set; see StickyVariations.
    pub fn set_sticky_enabled(&mut self, on: bool) {
        self.sticky.set_enabled(on);
    }

    /// (retained names, extras compiled into the current shader).
    pub fn sticky_stats(&self) -> (usize, usize) {
        (self.sticky.retained(), self.sticky.extras())
    }

    /// (rebuilds, cache hits, total compile ms) since this renderer was
    /// created. A hit is a shader change served from the pipeline LRU
    /// without compiling; the sticky-superset work exists to make
    /// rebuilds stop growing across a batch — see
    /// docs/projects/sticky-shader-compilation.md.
    pub fn shader_rebuild_stats(&self) -> (u64, u64, f64) {
        self.pipelines.shader_rebuild_stats()
    }

    pub fn samples_accumulated(&self) -> u64 {
        self.samples_accumulated
    }

    pub fn total_iterations(&self) -> u64 {
        self.total_iterations
    }

    /// Explicitly release every GPU resource this renderer owns (its
    /// `FlameBuffers` plus the fractal output texture). On WebGPU, dropping
    /// alone defers reclamation to JS GC, so a throwaway export renderer's
    /// multi-gigabyte buffers linger on the device and repeated large WASM
    /// exports OOM it (all-black output). Call once after the final pixel read
    /// completes, before dropping the renderer. See `FlameBuffers::destroy`.
    pub fn destroy(&self) {
        self.buffers.destroy();
        self.fractal_texture.destroy();
        if let Some(staging) = &self.readback_staging {
            staging.destroy();
        }
    }

    /// Ensure the persistent readback staging buffer holds at least
    /// `size` bytes. Grow-only; the old buffer is destroyed explicitly
    /// (see the field's comment for why dropping is not enough on
    /// WebGPU).
    fn ensure_readback_staging(&mut self, device: &Device, size: u64) {
        let needs_new = match &self.readback_staging {
            Some(b) => b.size() < size,
            None => true,
        };
        if needs_new {
            if let Some(old) = self.readback_staging.take() {
                old.destroy();
            }
            self.readback_staging = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Fractal Readback Staging (persistent)"),
                size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }
    }

    /// Free the large iteration buffers (histogram + accumulation + sample/path/
    /// blur), keeping the fractal output texture. For memory-constrained WASM
    /// exports: call after tonemap (and after its GPU work has completed) to
    /// reclaim ~3-4 GB at 8000² before a color-effect pass allocates its
    /// full-res ping-pong. The renderer must not iterate or tonemap again
    /// afterward; reading the fractal texture / running color effects is fine.
    pub fn free_iteration_buffers(&self) {
        self.buffers.free_iteration_buffers();
    }

    /// Samples currently in the accumulator. Used by the render loop
    /// to decide whether to keep iterating past `max_iterations`: when
    /// this is zero (e.g., right after a resize cleared the
    /// accumulator), iteration has to run at least once to populate
    /// the buffer, even on flames with a very low max_iterations.
    pub fn samples_in_buffer(&self) -> u64 {
        self.samples_in_buffer
    }

    /// Get effective iterations for brightness calculation
    /// This only counts iterations done in normal (non-overwrite) mode
    pub fn effective_iterations(&self) -> u64 {
        self.effective_iterations
    }

    /// Get fractal output texture view for display
    pub fn get_fractal_texture_view(&self) -> &TextureView {
        &self.fractal_texture_view
    }

    /// Get fractal output texture (for copy operations)
    pub fn fractal_texture(&self) -> &Texture {
        &self.fractal_texture
    }

    /// Get current accumulation texture (for histogram computation)
    pub fn accumulation_texture(&self) -> &Texture {
        self.buffers.current_accumulation_texture()
    }

    /// Create a staging buffer for reading fractal pixels
    /// Returns (buffer, padded_bytes_per_row)
    pub fn create_pixel_staging_buffer(&self, device: &Device) -> (Buffer, u32) {
        let bytes_per_pixel = 4u32; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Fractal Staging Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        (buffer, padded_bytes_per_row)
    }

    /// Copy fractal texture to a staging buffer
    pub fn copy_fractal_to_buffer(&self, encoder: &mut CommandEncoder, buffer: &Buffer, padded_bytes_per_row: u32) {
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &self.fractal_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer,
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
    }

    /// Get RNG seed based on deterministic mode
    fn get_rng_seed(&mut self) -> u32 {
        if self.deterministic_rng {
            // Deterministic progression: multiply frame counter by large prime (Knuth's multiplicative hash)
            // Simple +1 increments cause subtle correlations in the RNG output across frames,
            // likely due to how consecutive seeds interact with thread_id XOR in shader.
            // Using a large prime (2654435761 = 2^32 / φ, golden ratio) ensures maximum bit mixing.
            let seed = 12345u32.wrapping_add(self.frame_counter.wrapping_mul(2654435761u32));
            self.frame_counter = self.frame_counter.wrapping_add(1);
            seed
        } else {
            rand::random::<u32>()
        }
    }

    /// Set deterministic RNG mode
    pub fn set_deterministic_rng(&mut self, deterministic: bool) {
        self.deterministic_rng = deterministic;
    }

    /// Set blend factor for accumulation (0.01 = slow/smooth, 1.0 = fast/flickery)
    pub fn set_blend_factor(&mut self, blend_factor: f32) {
        self.blend_factor = blend_factor;
        // Note: This will take effect on the next accumulate pass (no need to update GPU params immediately)
    }

    /// Set whether to use dynamic blend (exponential convergence) or fixed blend rate
    pub fn set_use_dynamic_blend(&mut self, use_dynamic: bool) {
        self.use_dynamic_blend = use_dynamic;
        // Note: This will take effect on the next accumulate pass (no need to update GPU params immediately)
    }

    /// Set overwrite mode (live preview)
    /// When true, accumulation buffer is replaced instead of blended
    pub fn set_overwrite_mode(&mut self, overwrite: bool) {
        self.overwrite_mode = overwrite;
    }

    /// Update iterations per thread
    pub fn update_iterations(&mut self, queue: &Queue, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_x: f32, camera_y: f32, camera_z: f32, speed_factor: f32) {
        self.burn_in = burn_in;

        let sh_fit = self.frozen_shadow_fit.unwrap_or_else(|| self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, [camera_x, camera_y, camera_z]));
        let sh_dirs = self.shadow_light_dirs(camera_rotation_x, camera_rotation_y, camera_bank);
        let params = GpuParams {
            num_transforms: self.num_transforms,
            iterations_per_thread,
            burn_in,
            width: self.width,
            height: self.height,
            seed: self.get_rng_seed(),
            color_mode: self.color_mode as u32,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
                // The flame renderer never receives an escape config —
                // the app and render_with branch before this point. If
                // one is mis-routed anyway, render it as 2D rather than
                // panicking inside a GPU pass.
                crate::scene::transforms::RenderMode::Escape => 0,
            },
            perspective_strength: self.perspective_strength,
            depth_density_compensation: self.depth_density_compensation,
            far_density_fade: self.far_density_fade,
            far_density_fade_start: self.far_density_fade_start,
            solid_strength: self.solid_strength,
            surface_thickness: self.surface_thickness,
            depth_prime: 0,
            camera_rotation_x,
            camera_rotation_y,
            camera_bank,

            camera_x,

            camera_y,
            camera_z,
            dof_focus_distance: self.dof_focus_distance,
            dof_blur_strength: self.dof_blur_strength,
            fog_strength: self.atsplat_fog_strength(),
            fog_start: self.fog_start,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
            path_tracking_mode: self.path_tracking_mode as u32,
            num_path_filters: self.path_filters.len() as u32,
            min_suffix_filter_length: self.min_suffix_filter_length,
            background_r: self.background_r,
            background_g: self.background_g,
            background_b: self.background_b,
            post_symmetry: (&self.post_symmetry).into(),
            shadow_center_x: sh_fit.0[0],
            shadow_center_y: sh_fit.0[1],
            shadow_center_z: sh_fit.0[2],
            shadow_radius: sh_fit.1,
            shadow_count: sh_dirs.0,
            _pad_shadow: [0; 3],
            shadow_dirs: sh_dirs.1,
        };
        self.buffers.update_params(queue, &params);
    }

    /// Helper method to update tonemap parameters with current state
    fn update_tonemap_state(&self, queue: &Queue) {
        use crate::config::defaults::*;
        let area = (self.width * self.height) as f32;
        let sample_density = if area > 0.0 { self.total_iterations as f32 / area } else { 1.0 };

        let params = TonemapParams {
            exposure: 1.0,
            gamma: 2.2,
            density_scale: self.density_scale,
            tonemap_mode: 1,  // Logarithmic
            background_color: self.background_color,
            _pad_bg: 0.0,
            use_curve: 0,  // Disabled
            vibrancy: 1.0,  // Default
            brightness: DEFAULT_BRIGHTNESS,
            white_level: self.white_level,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: DEFAULT_SATURATION,
            hue_shift: DEFAULT_HUE_SHIFT,
            gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
            alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
            alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
            transparent_mode: 0,
            color_mode: self.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: self.path_map_style as u32,
            burn_in: self.burn_in,
            num_transforms: self.num_transforms,
            palette_size: self.buffers.palette_size(),
            levels_low: 0.0,
            levels_high: crate::config::defaults::DEFAULT_LEVELS_HIGH,
            levels_gamma: 1.0,
            highlight_mode: self.highlight_mode,
            levels_enabled: 0,
            _pad_levels: [0; 2],
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Update density scale for alpha blending
    pub fn update_density_scale(&mut self, queue: &Queue, density_scale: f32) {
        self.density_scale = density_scale;
        self.update_tonemap_state(queue);
    }

    /// Update background color
    pub fn update_background_color(&mut self, queue: &Queue, background_color: [f32; 3]) {
        self.background_color = background_color;
        self.update_tonemap_state(queue);
    }

    /// Set transparent mode for PNG export
    /// When enabled, tonemap shader outputs fractal alpha instead of blending with background
    pub fn set_transparent_mode(&self, queue: &Queue, transparent: bool, premultiplied: bool, config: &FractalConfig, iterations_per_thread: u32) {
        use crate::config::defaults::*;

        let tonemap_mode_u32 = match config.tonemap_mode {
            crate::scene::tonemap::ToneMapMode::Linear => 0u32,
            crate::scene::tonemap::ToneMapMode::Logarithmic => 1u32,
            crate::scene::tonemap::ToneMapMode::DensityVisualization => 2u32,
        };

        // Calculate area and sample_density (Ember/Apophysis-style).
        // sample_density = running iterations-per-pixel; recomputed
        // every tonemap pass so density × k2 stays scale-invariant
        // as samples accumulate. See
        // docs/projects/accumulator-unification.md, "How Ember solves
        // it" — `Source/Ember/Renderer.cpp:618-636`.
        let apophysis_zoom = config.zoom.log2();
        let base_pixels_per_unit = (self.width.min(self.height) as f32) * 0.25;
        let pixels_per_unit_zoomed = base_pixels_per_unit * (2.0_f32).powf(apophysis_zoom);
        let area = (self.width * self.height) as f32 / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);
        let total_pixels = (self.width as f32) * (self.height as f32);
        // Floor at 1e-6 (defensive non-zero, prevents `k2 = 1/0` in
        // the tonemap shader). Was `.max(1.0)`, which clamped
        // sample_density at ~"1 iter per pixel" — fine in steady
        // state but artificially dimmed early frames at high
        // resolution: 4K preview-mode at frame 1 has ~0.002
        // iters/pixel, clamping to 1.0 multiplied k2's denominator
        // 500× and made the image 500× dimmer than the same flame
        // at steady state. The clamp also broke the scale-invariance
        // promise (sample_density should track total_iterations
        // linearly through any iter count); 1e-6 is small enough to
        // never bind in practice.
        let sample_density = ((self.samples_in_buffer as f32) / total_pixels.max(1.0)).max(1e-6);
        let _ = iterations_per_thread; // formerly part of sample_density; now scale-invariant via total_iterations.

        let params = TonemapParams {
            exposure: config.exposure,
            gamma: config.gamma,
            density_scale: self.density_scale,
            tonemap_mode: tonemap_mode_u32,
            background_color: self.background_color,
            _pad_bg: 0.0,
            use_curve: if config.use_curve { 1u32 } else { 0u32 },
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
            // 0 = opaque, 1 = straight-alpha reconstruction (flatten over black),
            // 2 = premultiplied. See tonemap.wgsl.
            transparent_mode: if !transparent { 0 } else if premultiplied { 2 } else { 1 },
            color_mode: self.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: self.path_map_style as u32,
            burn_in: self.burn_in,
            num_transforms: self.num_transforms,
            palette_size: self.buffers.palette_size(),
            // Respect the flame's Levels in transparent export exactly as the
            // opaque/display path does. Levels gate the fractal ALPHA, and the
            // transparent PNG carries that same alpha, so compositing it over
            // the background reconstructs the with-background export. Forcing
            // Levels off here (the old behavior) made a transparent PNG render
            // differently from the with-background one for the same flame.
            levels_low: config.levels_low,
            levels_high: config.levels_high,
            levels_gamma: config.levels_gamma,
            highlight_mode: self.highlight_mode,
            levels_enabled: if config.levels_enabled { 1 } else { 0 },
            _pad_levels: [0; 2],
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Update tone mapping mode, curve usage, exposure, gamma, gamma_threshold, brightness, vibrancy, saturation, hue shift, and alpha blend
    pub fn update_tonemap(&mut self, queue: &Queue, tonemap_mode: crate::scene::tonemap::ToneMapMode, highlight_mode: crate::scene::tonemap::HighlightMode, use_curve: bool, exposure: f32, gamma: f32, gamma_threshold: f32, brightness: f32, vibrancy: f32, white_level: f32, saturation: f32, hue_shift: f32, alpha_blend_low: f32, alpha_blend_high: f32, width: u32, height: u32, _total_iterations: u64, _max_iterations: u64, zoom: f32, iterations_per_thread: u32, _batch_size: u32, is_live_preview: bool, levels_enabled: bool, levels_low: f32, levels_high: f32, levels_gamma: f32) {
        use crate::config::defaults::*;
        // Cache on self so internal helpers (update_density_scale,
        // update_background_color → update_tonemap_state) don't reset
        // white_level / highlight_mode back to their defaults on the next refresh.
        self.white_level = white_level;
        self.highlight_mode = match highlight_mode {
            crate::scene::tonemap::HighlightMode::Clip => 0,
            crate::scene::tonemap::HighlightMode::MaxNorm => 1,
            crate::scene::tonemap::HighlightMode::Reinhard => 2,
            crate::scene::tonemap::HighlightMode::Filmic => 3,
        };

        let tonemap_mode_u32 = match tonemap_mode {
            crate::scene::tonemap::ToneMapMode::Linear => 0u32,
            crate::scene::tonemap::ToneMapMode::Logarithmic => 1u32,
            crate::scene::tonemap::ToneMapMode::DensityVisualization => 2u32,
        };

        // Calculate area and sample_density (Ember/Apophysis-style).
        // Mirrors `Source/Ember/Renderer.cpp:618-636` — `sample_density`
        // is the *running* iterations-per-pixel, recomputed every
        // tonemap pass. Combined with `area = pixels / ppu²`, this
        // makes the product `density × k2` scale-invariant in sample
        // count: as iteration accumulates, density grows linearly,
        // k2 shrinks linearly, and `log(1 + density × k2)` stabilizes.
        // Brightness no longer drifts with sample count, no more
        // magic-number 5000.0 calibration constant, and
        // `iterations_per_thread` becomes a pure speed knob with no
        // brightness side effects. See
        // docs/projects/accumulator-unification.md.
        //
        // Apophysis zoom convention: ours is linear (zoom=1.0 default,
        // 2.0 = 2× scale); Apophysis is logarithmic (zoom=0 default,
        // 1 = 2× scale). Convert via log2.
        let apophysis_zoom = zoom.log2();
        let base_pixels_per_unit = (width.min(height) as f32) * 0.25;
        let pixels_per_unit_zoomed = base_pixels_per_unit * (2.0_f32).powf(apophysis_zoom);

        // Area in fractal space (not pixel space).
        let area = (width * height) as f32 / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);

        // Running iterations per pixel. `.max(1.0)` guards against
        // zero on the very first frame before any iteration has run.
        let total_pixels = (width as f32) * (height as f32);
        // Floor at 1e-6 (defensive non-zero, prevents `k2 = 1/0` in
        // the tonemap shader). Was `.max(1.0)`, which clamped
        // sample_density at ~"1 iter per pixel" — fine in steady
        // state but artificially dimmed early frames at high
        // resolution: 4K preview-mode at frame 1 has ~0.002
        // iters/pixel, clamping to 1.0 multiplied k2's denominator
        // 500× and made the image 500× dimmer than the same flame
        // at steady state. The clamp also broke the scale-invariance
        // promise (sample_density should track total_iterations
        // linearly through any iter count); 1e-6 is small enough to
        // never bind in practice.
        let sample_density = ((self.samples_in_buffer as f32) / total_pixels.max(1.0)).max(1e-6);

        // `is_live_preview` and `iterations_per_thread` no longer
        // affect the formula. The old code scaled sample_density by
        // `iterations_per_thread / 256` and divided by 8 during live
        // preview to compensate for the EMA accumulator's slower
        // convergence; with a scale-invariant tonemap, neither knob
        // is needed for brightness stability.
        let _ = is_live_preview;
        let _ = iterations_per_thread;

        let params = TonemapParams {
            exposure,
            gamma,
            density_scale: self.density_scale,
            tonemap_mode: tonemap_mode_u32,
            background_color: self.background_color,
            _pad_bg: 0.0,
            use_curve: if use_curve { 1u32 } else { 0u32 },
            vibrancy,
            brightness,
            white_level,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation,
            hue_shift,
            gamma_threshold,
            alpha_blend_low,
            alpha_blend_high,
            transparent_mode: 0,
            color_mode: self.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: self.path_map_style as u32,
            burn_in: self.burn_in,
            num_transforms: self.num_transforms,
            palette_size: self.buffers.palette_size(),
            levels_low,
            levels_high,
            levels_gamma,
            highlight_mode: self.highlight_mode,
            levels_enabled: if levels_enabled { 1 } else { 0 },
            _pad_levels: [0; 2],
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Update tone curve LUT texture
    pub fn update_curve_lut(&self, queue: &Queue, curve: &crate::scene::tonemap::ToneCurve) {
        self.buffers.update_curve_lut(queue, curve);
    }

    /// Update palette texture
    pub fn update_palette(
        &mut self,
        device: &Device,
        queue: &Queue,
        palette: &Palette,
        palette_rotation: f32,
        palette_squeeze: f32,
        palette_squeeze_mode: crate::scene::palette::SqueezeMode,
        palette_squeeze_falloff: f32,
        palette_log_strength: f32,
        palette_reverse: bool,
    ) {
        self.buffers.update_palette(
            queue,
            palette,
            palette_rotation,
            palette_squeeze,
            palette_squeeze_mode,
            palette_squeeze_falloff,
            palette_log_strength,
            palette_reverse,
        );
        // Recreate compute bind group to ensure palette texture is bound
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
    }

    /// Change palette texture size (requires recreating buffers)
    /// Returns true if size actually changed
    pub fn set_palette_size(&mut self, device: &Device, _queue: &Queue, _flame: &Flame, new_size: u32) -> bool {
        if !self.buffers.resize_palette(device, new_size) {
            return false;
        }

        // Only the compute bind group references the palette texture view
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);

        true
    }

    /// Get current palette texture size
    pub fn palette_size(&self) -> u32 {
        self.buffers.palette_size()
    }

    /// Set color mode
    pub fn set_color_mode(&mut self, queue: &Queue, color_mode: ColorMode, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32, camera_x: f32, camera_y: f32, camera_z: f32, speed_factor: f32) {
        self.color_mode = color_mode;

        // Update params to reflect new color mode
        let sh_fit = self.frozen_shadow_fit.unwrap_or_else(|| self.shadow_placement(zoom, pan_x, pan_y, camera_rotation_x, camera_rotation_y, camera_bank, [camera_x, camera_y, camera_z]));
        let sh_dirs = self.shadow_light_dirs(camera_rotation_x, camera_rotation_y, camera_bank);
        let params = GpuParams {
            num_transforms: self.num_transforms,
            iterations_per_thread,
            burn_in,
            width: self.width,
            height: self.height,
            seed: self.get_rng_seed(),
            color_mode: self.color_mode as u32,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
                // The flame renderer never receives an escape config —
                // the app and render_with branch before this point. If
                // one is mis-routed anyway, render it as 2D rather than
                // panicking inside a GPU pass.
                crate::scene::transforms::RenderMode::Escape => 0,
            },
            perspective_strength: self.perspective_strength,
            depth_density_compensation: self.depth_density_compensation,
            far_density_fade: self.far_density_fade,
            far_density_fade_start: self.far_density_fade_start,
            solid_strength: self.solid_strength,
            surface_thickness: self.surface_thickness,
            depth_prime: 0,
            camera_rotation_x,
            camera_rotation_y,
            camera_bank,

            camera_x,

            camera_y,
            camera_z,
            dof_focus_distance: self.dof_focus_distance,
            dof_blur_strength: self.dof_blur_strength,
            fog_strength: self.atsplat_fog_strength(),
            fog_start: self.fog_start,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
            path_tracking_mode: self.path_tracking_mode as u32,
            num_path_filters: self.path_filters.len() as u32,
            min_suffix_filter_length: self.min_suffix_filter_length,
            background_r: self.background_r,
            background_g: self.background_g,
            background_b: self.background_b,
            post_symmetry: (&self.post_symmetry).into(),
            shadow_center_x: sh_fit.0[0],
            shadow_center_y: sh_fit.0[1],
            shadow_center_z: sh_fit.0[2],
            shadow_radius: sh_fit.1,
            shadow_count: sh_dirs.0,
            _pad_shadow: [0; 3],
            shadow_dirs: sh_dirs.1,
        };
        self.buffers.update_params(queue, &params);
    }

    /// Get current color mode
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    /// Set path map style (Prefix = color by path start, Suffix = color by path end)
    pub fn set_path_map_style(&mut self, path_map_style: PathMapStyle) {
        self.path_map_style = path_map_style;
        // Note: tonemap params will be updated on next render via update_tonemap
    }

    /// Get current path map style
    pub fn path_map_style(&self) -> PathMapStyle {
        self.path_map_style
    }

    /// Set path capture mode (FirstHit, FirstAfterBurnIn, or LastHit)
    pub fn set_path_capture_mode(&mut self, path_capture_mode: PathCaptureMode) {
        self.path_capture_mode = path_capture_mode;
        // Note: GPU params will be updated on next render
    }

    /// Get current path capture mode
    pub fn path_capture_mode(&self) -> PathCaptureMode {
        self.path_capture_mode
    }

    /// Set path tracking mode (First = first 32 iterations, Recent = rolling window of 32 most recent)
    pub fn set_path_tracking_mode(&mut self, path_tracking_mode: PathTrackingMode) {
        self.path_tracking_mode = path_tracking_mode;
        // Note: GPU params will be updated on next render
    }

    /// Get current path tracking mode
    pub fn path_tracking_mode(&self) -> PathTrackingMode {
        self.path_tracking_mode
    }

    /// Set path filters for blocking specific transform sequences
    ///
    /// # Arguments
    /// * `filters` - Vector of GpuPathFilter structs defining patterns to block
    ///
    /// # Example
    /// ```ignore
    /// // Block all paths ending with transform [0,0,0,0,1] (suffix filter)
    /// renderer.set_path_filters(vec![GpuPathFilter::suffix(&[0, 0, 0, 0, 1])]);
    ///
    /// // Block paths matching [0,1] at iteration depth 2 (exact depth filter)
    /// renderer.set_path_filters(vec![GpuPathFilter::at_depth(&[0, 1], 2)]);
    /// ```
    pub fn set_path_filters(&mut self, filters: Vec<crate::gpu::buffers::GpuPathFilter>) {
        // Calculate min_suffix_filter_length for optimization
        self.min_suffix_filter_length = filters
            .iter()
            .filter(|f| f.depth == 0) // Only suffix filters
            .map(|f| f.length)
            .min()
            .unwrap_or(0);

        self.path_filters = filters;
        // Note: GPU buffer will be updated on next compute pass
    }

    /// Clear all path filters
    pub fn clear_path_filters(&mut self) {
        self.path_filters.clear();
        self.min_suffix_filter_length = 0;
    }

    /// Get current path filters
    pub fn path_filters(&self) -> &[crate::gpu::buffers::GpuPathFilter] {
        &self.path_filters
    }

    /// Check if path features (PathMap color mode or path filters) require buffers
    /// Returns true if path buffers should be enabled
    pub fn needs_path_features(&self) -> bool {
        self.color_mode == crate::scene::palette::ColorMode::PathMap || !self.path_filters.is_empty()
    }

    /// Check if path buffers are currently allocated
    pub fn path_features_enabled(&self) -> bool {
        self.buffers.path_features_enabled()
    }

    /// Enable or disable path features based on current state
    /// Call this when color_mode or path_filters change
    /// Returns true if bind groups or shaders were rebuilt
    pub fn update_path_features(&mut self, device: &Device, queue: &Queue, flame: &crate::scene::transforms::Flame) -> bool {
        // Sticky superset: this refresh may rebuild the shader, and it
        // must rebuild against the SAME map the buffers are packed with —
        // the last adopt's. `augmented` (not `adopt`) re-applies exactly
        // that map; with the raw flame this forked the shader back to the
        // specialized map on every app flame change, misaligning every
        // weight and param offset — the single-pixel collapse.
        let flame = &self.sticky.augmented(flame);
        let needs_path = self.needs_path_features();
        let has_path = self.buffers.path_features_enabled();
        let mut changed = false;

        // Update buffers if needed
        if needs_path && !has_path {
            // Need to create path buffers
            if self.buffers.create_path_buffers(device, queue) {
                // Rebuild bind groups with new buffers
                self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
                self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
                self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);
                changed = true;
            }
        } else if !needs_path && has_path {
            // Can drop path buffers to save memory
            if self.buffers.drop_path_buffers() {
                // Rebuild bind groups with dummy buffers
                self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
                self.init_bind_group = self.pipelines.create_init_bind_group(device, &self.buffers);
                self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);
                changed = true;
            }
        }

        // Update shaders if any shader constants changed (path features, color mode, etc.)
        // The shader cache compares all constants and only rebuilds if something changed
        let constants = self.build_shader_constants(flame, self.current_render_mode, self.preserve_z);
        if self.pipelines.ensure_shaders_current_with_constants(device, flame, needs_path, constants, self.current_render_mode) {
            changed = true;
        }

        changed
    }

    /// Refresh the analytic-blur slot list from the flame and flag a kernel
    /// rebuild. The actual low-res buffer (re)allocation + bind-group rebuild
    /// happen in `maybe_rebuild_blur_kernels` (which knows D), called from
    /// `compute_pass` before the splat.
    fn update_blur_buffers(&mut self, flame: &crate::scene::transforms::Flame) {
        let registry = crate::variations::global_registry();
        let registry = &*registry;

        // Per-slot kernel inputs (same order as GpuTransform::from_flame's slot
        // assignment). Buffer allocation + the count cap happen in
        // maybe_rebuild_blur_kernels (which knows D); here we just record them.
        self.blur_slots = flame.blur_slots(registry, self.current_render_mode);
        // Flame changed → the kernel inputs and slot set may have changed;
        // force a rebuild (maybe_rebuild reallocates + rebinds as needed).
        self.blur_kernels_dirty = true;
    }

    /// Rebuild the per-slot analytic-blur convolution kernels and upload them
    /// (weights + meta) to the GPU, but only when something that affects them
    /// changed: the flame (`blur_kernels_dirty`) or the view zoom/rotation.
    /// The kernel is the variation's offset distribution sampled in pixel
    /// space through `world→pixel linear · weight · post-affine linear`, so it
    /// reproduces the stochastic splat by construction. Cheap CPU Monte-Carlo;
    /// runs at most once per view/flame change, never per accumulation frame.
    fn maybe_rebuild_blur_kernels(&mut self, device: &Device, queue: &Queue, zoom: f32, rotation: f32) {
        use crate::gpu::buffers::{BlurConvolveParams, MAX_BLUR_BUFFERS};

        if self.blur_slots.is_empty() {
            // Feature inactive — drop the low-res buffers + rebind to the dummy.
            if self.buffers.ensure_lowres_blur_buffers(device, 0, 0, 0) {
                self.recreate_blur_bind_groups(device);
            }
            return;
        }
        let unchanged = !self.blur_kernels_dirty
            && self.blur_kernel_zoom == zoom
            && self.blur_kernel_rotation == rotation;
        if unchanged {
            return;
        }

        // Size + build the convolution (shared with the high-res exporter so
        // both produce identical kernels). See compute_blur_setup.
        let setup = crate::variations::analytic_blur::compute_blur_setup(
            self.width, self.height, zoom, rotation, &self.blur_slots,
        );
        let lowres_w = setup.lowres_w;
        let lowres_h = setup.lowres_h;
        let downscale = setup.downscale;
        self.blur_lowres_w = lowres_w;
        self.blur_lowres_h = lowres_h;

        // (Re)allocate the low-res buffers for these dims + slot count (capped
        // by the memory budget). MUST happen before the splat dispatch that
        // writes them. Recreate bind groups when the buffer set changed.
        let num_slots = self.blur_slots.len() as u32;
        if self.buffers.ensure_lowres_blur_buffers(device, lowres_w, lowres_h, num_slots) {
            self.recreate_blur_bind_groups(device);
        }

        let weights = setup.weights;
        let mut meta = [[0u32; 4]; MAX_BLUR_BUFFERS as usize];
        for (i, m) in setup.meta.iter().enumerate().take(MAX_BLUR_BUFFERS as usize) {
            meta[i] = *m;
        }

        let params = BlurConvolveParams {
            full_width: self.width,
            full_height: self.height,
            lowres_width: lowres_w,
            lowres_height: lowres_h,
            downscale,
            // The ACTUALLY allocated count (the memory cap may be < num_slots);
            // the shader gates `slot < count` so over-cap transforms stay
            // stochastic.
            count: self.buffers.blur_buffer_count,
            frame_seed: 0,
            _pad1: 0,
            meta,
        };
        queue.write_buffer(&self.buffers.blur_convolve_params_buffer, 0, bytemuck::cast_slice(&[params]));
        if !weights.is_empty() {
            queue.write_buffer(&self.buffers.blur_kernel_weights_buffer, 0, bytemuck::cast_slice(&weights));
        }

        self.blur_kernel_zoom = zoom;
        self.blur_kernel_rotation = rotation;
        self.blur_kernels_dirty = false;
    }

    /// Recreate the bind groups that reference the analytic-blur buffers, after
    /// a (re)allocation: the main compute group (binding 13 splat + 14 params)
    /// and the convolve/upscale groups.
    fn recreate_blur_bind_groups(&mut self, device: &Device) {
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.blur_convolve_bind_group = self.pipelines.create_blur_convolve_bind_group(device, &self.buffers);
        self.blur_upscale_bind_group = self.pipelines.create_blur_upscale_bind_group(device, &self.buffers);
    }

    fn update_xaos_buffer(&mut self, device: &Device, queue: &Queue, flame: &crate::scene::transforms::Flame) -> bool {
        let needs_xaos = flame.has_xaos();
        let has_xaos = self.buffers.xaos_enabled();

        if needs_xaos && !has_xaos {
            // Need to create xaos buffer
            let num_transforms = flame.transforms.len() as u32;
            if self.buffers.create_xaos_buffer(device, num_transforms) {
                self.buffers.update_xaos(queue, flame);
                return true;
            }
        } else if !needs_xaos && has_xaos {
            // Can drop xaos buffer
            if self.buffers.drop_xaos_buffer() {
                return true;
            }
        } else if needs_xaos && has_xaos {
            // Recreate buffer if transform count changed (e.g., clone/add/delete)
            let required_size = (flame.transforms.len() * flame.transforms.len() * std::mem::size_of::<f32>()) as u64;
            let current_size = self.buffers.xaos_buffer.as_ref().map(|b| b.size()).unwrap_or(0);
            if required_size != current_size {
                self.buffers.drop_xaos_buffer();
                self.buffers.create_xaos_buffer(device, flame.transforms.len() as u32);
                self.buffers.update_xaos(queue, flame);
                return true;
            }
            // Just update the data
            self.buffers.update_xaos(queue, flame);
        }

        false
    }

    /// Read pixels from the fractal_texture (after tonemap_pass has rendered to it)
    /// This is the unified method that reads what was actually displayed on screen.
    ///
    /// # Arguments
    /// * `transparent` - If true, preserve alpha channel; if false, blend with background and set alpha=255
    /// * `background_color` - RGB background color for opaque mode (ignored in transparent mode)
    pub async fn read_fractal_pixels(
        &mut self,
        device: &Device,
        queue: &Queue,
        transparent: bool,
        background_color: [f32; 3],
    ) -> Result<(u32, u32, Vec<u8>), String> {
        // Wait for any pending rendering to complete
        let sync_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Pre-Read Sync"),
        });
        queue.submit(std::iter::once(sync_encoder.finish()));
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

        // The persistent staging buffer (see `readback_staging`) —
        // re-mapped every read, never recreated at a steady size.
        let bytes_per_pixel = 4; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        self.ensure_readback_staging(device, buffer_size);
        let buffer = self.readback_staging.as_ref().expect("ensured above");

        // Copy texture to buffer
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Fractal Read Encoder"),
        });
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &self.fractal_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buffer,
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
        queue.submit(std::iter::once(encoder.finish()));

        // Map and read. Slice only the bytes this read needs — the
        // persistent buffer may be larger (grow-only) than this size.
        let buffer_slice = buffer.slice(0..buffer_size);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        rx.await
            .map_err(|_| "Failed to map buffer".to_string())?
            .map_err(|e| format!("Buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Copy data, optionally blend background
        let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + (self.width * bytes_per_pixel) as usize;
            let row_data = &data[row_start..row_end];

            for x in 0..self.width {
                let pixel_start = (x * bytes_per_pixel) as usize;
                let r = row_data[pixel_start];
                let g = row_data[pixel_start + 1];
                let b = row_data[pixel_start + 2];
                let a = row_data[pixel_start + 3];

                if transparent {
                    // Transparent mode: keep original RGBA
                    rgba_data.extend_from_slice(&[r, g, b, a]);
                } else {
                    // Opaque mode: blend with background, set alpha=255
                    let alpha = a as f32 / 255.0;
                    let bg_r = (background_color[0] * 255.0) as u8;
                    let bg_g = (background_color[1] * 255.0) as u8;
                    let bg_b = (background_color[2] * 255.0) as u8;

                    let out_r = ((r as f32 * alpha) + (bg_r as f32 * (1.0 - alpha))) as u8;
                    let out_g = ((g as f32 * alpha) + (bg_g as f32 * (1.0 - alpha))) as u8;
                    let out_b = ((b as f32 * alpha) + (bg_b as f32 * (1.0 - alpha))) as u8;

                    rgba_data.extend_from_slice(&[out_r, out_g, out_b, 255]);
                }
            }
        }

        // Unmap, so the persistent buffer can be re-mapped next read.
        // (The old per-read buffer relied on drop, which unmaps but
        // frees nothing on WebGPU.)
        drop(data);
        buffer.unmap();

        Ok((self.width, self.height, rgba_data))
    }

    /// Read path buffer from GPU for CPU-side path queries
    /// Returns a 2D array of PathEntry indexed by [y][x]
    /// Returns empty grid if path buffers are not enabled
    pub async fn read_path_buffer(
        &self,
        device: &Device,
        queue: &Queue,
    ) -> Result<Vec<Vec<PathEntry>>, String> {
        // Check if path buffer exists
        let path_buffer = match &self.buffers.path_buffer {
            Some(buf) => buf,
            None => {
                // Return empty PathEntry grid if path features are disabled
                return Ok(vec![vec![PathEntry::default(); self.width as usize]; self.height as usize]);
            }
        };

        // Wait for any pending rendering to complete
        let sync_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Pre-Read Path Sync"),
        });
        queue.submit(std::iter::once(sync_encoder.finish()));
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

        // PathEntry is 7 × u32 = 28 bytes per pixel (5 u32 + 2 f32)
        let bytes_per_entry = 7 * std::mem::size_of::<u32>() as u32;
        let buffer_size = (self.width * self.height * bytes_per_entry) as u64;

        // Create staging buffer for readback
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Path Buffer Staging"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy path buffer to staging buffer
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Path Buffer Read Encoder"),
        });
        encoder.copy_buffer_to_buffer(
            path_buffer,
            0,
            &staging_buffer,
            0,
            buffer_size,
        );
        queue.submit(std::iter::once(encoder.finish()));

        // Map and read
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        rx.await
            .map_err(|_| "Failed to map path buffer".to_string())?
            .map_err(|e| format!("Path buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Convert raw bytes to PathEntry grid
        let mut result = Vec::with_capacity(self.height as usize);
        for y in 0..self.height {
            let mut row = Vec::with_capacity(self.width as usize);
            for x in 0..self.width {
                let idx = ((y * self.width + x) * bytes_per_entry) as usize;
                let path0 = u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                let path1 = u32::from_le_bytes([data[idx + 4], data[idx + 5], data[idx + 6], data[idx + 7]]);
                let path2 = u32::from_le_bytes([data[idx + 8], data[idx + 9], data[idx + 10], data[idx + 11]]);
                let path3 = u32::from_le_bytes([data[idx + 12], data[idx + 13], data[idx + 14], data[idx + 15]]);
                let iteration_count = u32::from_le_bytes([data[idx + 16], data[idx + 17], data[idx + 18], data[idx + 19]]);
                let initial_x = f32::from_le_bytes([data[idx + 20], data[idx + 21], data[idx + 22], data[idx + 23]]);
                let initial_y = f32::from_le_bytes([data[idx + 24], data[idx + 25], data[idx + 26], data[idx + 27]]);

                row.push(PathEntry {
                    path0,
                    path1,
                    path2,
                    path3,
                    iteration_count,
                    initial_x,
                    initial_y,
                });
            }
            result.push(row);
        }

        drop(data);
        staging_buffer.unmap();
        // Explicit, because dropping frees nothing on WebGPU.
        staging_buffer.destroy();

        Ok(result)
    }

    /// Get path at a specific pixel coordinate
    /// This is a convenience method that reads the entire buffer
    /// For frequent queries, cache the result of read_path_buffer()
    pub async fn get_path_at(
        &self,
        device: &Device,
        queue: &Queue,
        x: u32,
        y: u32,
    ) -> Result<Option<PathEntry>, String> {
        if x >= self.width || y >= self.height {
            return Ok(None);
        }
        let paths = self.read_path_buffer(device, queue).await?;
        Ok(Some(paths[y as usize][x as usize]))
    }

    /// Read a region of pixels from the fractal texture centered at (center_x, center_y)
    /// Returns a Vec of [R, G, B, A] values in row-major order
    /// Region is clamped to texture boundaries
    pub async fn read_pixel_region(
        &self,
        device: &Device,
        queue: &Queue,
        center_x: u32,
        center_y: u32,
        region_width: u32,
        region_height: u32,
    ) -> Result<Vec<[u8; 4]>, String> {
        // Calculate region bounds, clamped to texture size
        let half_w = region_width / 2;
        let half_h = region_height / 2;

        let start_x = center_x.saturating_sub(half_w);
        let start_y = center_y.saturating_sub(half_h);
        let end_x = (center_x + half_w + 1).min(self.width);
        let end_y = (center_y + half_h + 1).min(self.height);

        let actual_width = end_x - start_x;
        let actual_height = end_y - start_y;

        if actual_width == 0 || actual_height == 0 {
            return Ok(vec![]);
        }

        // Create staging buffer for readback
        let bytes_per_pixel = 4u32; // RGBA8
        let bytes_per_row = actual_width * bytes_per_pixel;
        // wgpu requires rows to be aligned
        let align = 256u32;
        let padded_bytes_per_row = ((bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * actual_height) as u64;

        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Pixel Region Staging"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy from fractal texture to staging buffer
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Pixel Region Read Encoder"),
        });

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &self.fractal_texture,
                mip_level: 0,
                origin: Origin3d { x: start_x, y: start_y, z: 0 },
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(actual_height),
                },
            },
            Extent3d {
                width: actual_width,
                height: actual_height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        // Map and read
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        rx.await
            .map_err(|_| "Failed to map pixel region buffer".to_string())?
            .map_err(|e| format!("Pixel region map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Extract pixels (accounting for row padding)
        let mut pixels = Vec::with_capacity((actual_width * actual_height) as usize);
        for y in 0..actual_height {
            let row_start = (y * padded_bytes_per_row) as usize;
            for x in 0..actual_width {
                let idx = row_start + (x * bytes_per_pixel) as usize;
                pixels.push([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
            }
        }

        drop(data);
        staging_buffer.unmap();
        // Explicit, because dropping frees nothing on WebGPU.
        staging_buffer.destroy();

        Ok(pixels)
    }

    #[allow(dead_code)]
    /// OLD METHOD - DEPRECATED - Use read_fractal_pixels() instead
    /// blends RGB channels with the background before outputting. Even though it outputs
    /// the alpha channel, the RGB values are already pre-multiplied/blended, making the
    /// alpha useless for compositing. For transparency, we must read raw accumulation data.
    pub async fn capture_pixels(&mut self, device: &Device, queue: &Queue, transparent: bool, surface_format: TextureFormat) -> Result<(u32, u32, Vec<u8>), String> {
        if transparent {
            // For transparent export, read directly from accumulation buffer
            // and apply tone mapping on CPU to preserve true alpha values
            self.capture_from_accumulation_buffer(device, queue).await
        } else {
            // For opaque export, force non-black background to trigger opaque mode
            // even if user has set background to black in UI
            let original_bg = self.background_color;
            let needs_override = original_bg[0] < 0.001 && original_bg[1] < 0.001 && original_bg[2] < 0.001;

            if needs_override {
                // Temporarily set to nearly-black to force opaque alpha output
                self.update_background_color(queue, [0.001, 0.001, 0.001]);
            }

            let result = self.capture_from_tonemap_render(device, queue, surface_format).await;

            if needs_override {
                // Restore original background
                self.update_background_color(queue, original_bg);
            }

            result
        }
    }

    #[allow(dead_code)]
    /// OLD METHOD - DEPRECATED - Use read_fractal_pixels() instead
    /// Capture pixels from accumulation buffer (for transparent PNG export)
    ///
    /// This preserves true alpha values by reading raw Rgba32Float accumulation data
    /// and applying tone mapping on the CPU. The accumulation buffer stores:
    /// - RGB: averaged fractal colors (no background blending)
    /// - A: accumulated density (sum across all frames)
    ///
    /// We apply the same tone mapping as the GPU shader (exposure → log → gamma)
    /// but calculate alpha directly from density without any background blending.
    async fn capture_from_accumulation_buffer(&self, device: &Device, queue: &Queue) -> Result<(u32, u32, Vec<u8>), String> {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Accumulation Capture Encoder"),
        });

        // Create buffer to copy accumulation texture data (Rgba32Float
        // since Phase 8c — was Rgba16Float).
        let bytes_per_pixel = 16; // Rgba32Float = 4 channels × 4 bytes each
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = egui_wgpu::wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Accumulation Capture Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy accumulation texture to buffer
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: self.buffers.current_accumulation_texture(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buffer,
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

        queue.submit(Some(encoder.finish()));

        // Map buffer and read Rgba32Float data
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await.map_err(|_| "Failed to map buffer".to_string())?
            .map_err(|e| format!("Buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Convert Rgba32Float to Rgba8 with CPU tone mapping
        let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);

        // Iterate row by row to handle padding
        for y in 0..self.height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_data = &data[row_start..row_start + (self.width * bytes_per_pixel) as usize];

            for chunk in row_data.chunks_exact(16) {
                // Read f32 values directly from 4 bytes each.
                let r = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let g = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                let b = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
                let density = f32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]);

            // Apply same tone mapping as shader (exposure + log + gamma)
            let exposure = 1.0f32;
            let gamma = 2.2f32;

            let mut color_r = r * exposure;
            let mut color_g = g * exposure;
            let mut color_b = b * exposure;

            // Log tone mapping
            color_r = (color_r + 1.0).log10();
            color_g = (color_g + 1.0).log10();
            color_b = (color_b + 1.0).log10();

            // Gamma correction
            color_r = color_r.powf(1.0 / gamma);
            color_g = color_g.powf(1.0 / gamma);
            color_b = color_b.powf(1.0 / gamma);

            // Clamp to [0, 1]
            color_r = color_r.clamp(0.0, 1.0);
            color_g = color_g.clamp(0.0, 1.0);
            color_b = color_b.clamp(0.0, 1.0);

            // Calculate alpha from density (same as shader)
            let alpha = (density * self.density_scale).clamp(0.0, 1.0);

                // Convert to u8
                rgba_data.push((color_r * 255.0) as u8);
                rgba_data.push((color_g * 255.0) as u8);
                rgba_data.push((color_b * 255.0) as u8);
                rgba_data.push((alpha * 255.0) as u8);
            }
        }

        drop(data);
        buffer.unmap();

        Ok((self.width, self.height, rgba_data))
    }

    #[allow(dead_code)]
    /// OLD METHOD - DEPRECATED - Use read_fractal_pixels() instead
    /// Capture pixels from tonemapped render (for opaque PNG export)
    async fn capture_from_tonemap_render(&self, device: &Device, queue: &Queue, surface_format: TextureFormat) -> Result<(u32, u32, Vec<u8>), String> {
        // Create a temporary texture to render to (use same format as surface)
        let texture_desc = TextureDescriptor {
            label: Some("Screenshot Texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let texture = device.create_texture(&texture_desc);

        // Render to the texture
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Screenshot Encoder"),
        });
        self.tonemap_pass(queue, &mut encoder);

        // Create buffer to copy texture data to
        let bytes_per_pixel = 4; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Screenshot Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buffer,
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

        queue.submit(Some(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await.map_err(|_| "Failed to map buffer".to_string())?
            .map_err(|e| format!("Buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Copy data row by row, skipping padding
        let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + (self.width * bytes_per_pixel) as usize;
            rgba_data.extend_from_slice(&data[row_start..row_end]);
        }

        drop(data);
        buffer.unmap();

        // Convert BGRA to RGBA if needed
        let rgba_data = if surface_format == TextureFormat::Bgra8UnormSrgb || surface_format == TextureFormat::Bgra8Unorm {
            // Swap B and R channels
            rgba_data.chunks_exact(4)
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
                .collect()
        } else {
            rgba_data
        };

        // Return raw pixel data (width, height, rgba_bytes)
        Ok((self.width, self.height, rgba_data))
    }

    /// Rewrite the packed variation parameters from `flame`, without
    /// touching anything else.
    ///
    /// For the probe's parameter sweep, which changes one parameter and
    /// re-dispatches ~2000 times. `load_config` would reallocate every
    /// buffer and recreate every bind group each time; this is the
    /// `queue.write_buffer` underneath it. The flame's *structure* must
    /// not have changed — same transforms, same variations, same order —
    /// because the packing offsets are derived from it.
    ///
    /// Marks the init-derived slots dirty: they are computed from the
    /// user parameters, so changing one invalidates them. Call
    /// [`Self::run_init_pass`] afterwards or they keep the previous
    /// step's values.
    pub fn set_variation_params(&mut self, queue: &Queue, flame: &crate::scene::transforms::Flame) {
        // Must pack against the SAME map the compiled shader was built
        // with — re-apply the last adopt's extras (a no-op when empty).
        let flame = self.sticky.augmented(flame);
        self.buffers.update_variation_params(queue, &flame);
        self.init_dirty = true;
    }

    /// Run the variation init dispatch on its own, outside a render.
    ///
    /// `load_config` marks the derived parameters dirty but does not
    /// compute them — the init pass rides along inside `render()`, which
    /// is fine for every caller that renders and wrong for the one that
    /// does not. The numerical probe dispatches its own entry point
    /// directly, so without this the 134 variations with init-derived
    /// slots read a buffer of zeros. That does not fail loudly: it
    /// produces stable, reproducible, identical-on-every-platform
    /// nonsense, which is the worst possible outcome for a tool whose
    /// entire output is a cross-platform comparison.
    ///
    /// Idempotent — a no-op when nothing is dirty or no active variation
    /// has an init function.
    pub fn run_init_pass(&mut self, device: &Device, queue: &Queue) {
        if !self.init_dirty {
            return;
        }
        let Some(pipeline) = self.pipelines.shader_cache.init_pipeline.as_ref() else {
            self.init_dirty = false;
            return;
        };
        let pair_count = self.pipelines.shader_cache.init_pair_count;
        if pair_count == 0 {
            self.init_dirty = false;
            return;
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Standalone Variation Init"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Variation Init Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.init_bind_group, &[]);
            pass.dispatch_workgroups(pair_count.div_ceil(64), 1, 1);
        }
        queue.submit(Some(encoder.finish()));
        self.init_dirty = false;
    }

    /// Run one dispatch of a caller-supplied compute shader against this
    /// renderer's live bind group, and read words back from the
    /// histogram buffer.
    ///
    /// This exists for the numerical probe (`src/probe/`), and it is
    /// deliberately the *only* thing the probe needs from the renderer.
    /// Everything the probe knows — the buffer layout, the input grid,
    /// how results are classified — stays on its side; what it cannot
    /// get from outside this module is the bind group, which is private
    /// and correct, holding the transforms, params and packed variation
    /// parameters that `apply_variations` reads.
    ///
    /// Passing WGSL in rather than building it here keeps the direction
    /// of the dependency right: the renderer does not know what a probe
    /// is.
    ///
    /// `input_words` are written at the head of the histogram buffer
    /// before the dispatch, and `output_words` are read from the same
    /// buffer after it — the probe shader's contract, not this
    /// function's business.
    ///
    /// `phase` is called with `(name, milliseconds)` for the compile and
    /// the dispatch separately. The split is what makes a slow result
    /// actionable: a shader the driver takes a minute to compile and one
    /// whose math takes a minute to run are entirely different problems,
    /// and a single total cannot tell them apart.
    pub fn dispatch_readback(
        &self,
        device: &Device,
        queue: &Queue,
        source: &str,
        entry_point: &str,
        input_words: &[u32],
        output_words: usize,
        threads: u32,
        phase: &mut dyn FnMut(&str, f64),
    ) -> Result<Vec<u32>, String> {
        let byte_len = (output_words * std::mem::size_of::<u32>()) as u64;
        if byte_len > self.buffers.histogram_buffer.size() {
            return Err(format!(
                "probe needs {byte_len} bytes but the histogram buffer is {} — \
                 construct the renderer at a larger resolution",
                self.buffers.histogram_buffer.size()
            ));
        }

        let compile_started = web_time::Instant::now();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probe shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        // Reuse the render path's bind group layout rather than an auto
        // layout: the probe entry point touches only a few of the
        // bindings, and an auto layout would derive a *narrower* one
        // that the existing bind group no longer satisfies.
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("probe pipeline layout"),
            bind_group_layouts: &[Some(&self.pipelines.compute_bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("probe pipeline"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache: None,
        });

        // The driver does its real work lazily, so timing
        // `create_compute_pipeline` alone can under-report. Polling here
        // forces the compile to have happened before the clock stops.
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        phase("compile", compile_started.elapsed().as_secs_f64() * 1000.0);

        let dispatch_started = web_time::Instant::now();
        queue.write_buffer(
            &self.buffers.histogram_buffer,
            0,
            bytemuck::cast_slice(input_words),
        );

        let readback = device.create_buffer(&BufferDescriptor {
            label: Some("probe readback"),
            size: byte_len,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("probe encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("probe pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &self.compute_bind_group, &[]);
            pass.dispatch_workgroups(threads.div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &self.buffers.histogram_buffer,
            0,
            &readback,
            0,
            byte_len,
        );
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        rx.recv()
            .map_err(|e| format!("probe readback channel closed: {e}"))?
            .map_err(|e| format!("probe readback failed: {e}"))?;

        let words = bytemuck::cast_slice::<u8, u32>(&slice.get_mapped_range()).to_vec();
        readback.unmap();
        phase("dispatch", dispatch_started.elapsed().as_secs_f64() * 1000.0);
        Ok(words)
    }
}

/// Standalone function to encode RGBA pixel data as PNG with metadata
/// This doesn't borrow anything and can be moved into async contexts
pub fn encode_png_from_rgba(width: u32, height: u32, rgba_data: Vec<u8>, metadata: Option<crate::png_metadata::PngMetadata>) -> Result<Vec<u8>, String> {
    // Use new metadata-aware encoder if metadata provided
    if let Some(meta) = metadata {
        return crate::png_metadata::encode_png_with_metadata(width, height, rgba_data, &meta);
    }

    // Fallback to simple encoding without metadata
    use image::{ImageBuffer, Rgba};

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
        width,
        height,
        rgba_data,
    ).ok_or("Failed to create image buffer")?;

    // Don't flip - wgpu textures are already in the correct orientation
    // The Origin3d::ZERO starts at top-left, which matches PNG format
    // Previous flip_vertical was causing 180° rotation artifacts

    let mut png_data = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_data), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(png_data)
}
