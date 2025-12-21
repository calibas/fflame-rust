use egui_wgpu::wgpu::*;
use crate::gpu::{buffers::*, pipelines::FlamePipelines};
use crate::scene::transforms::Flame;
use crate::scene::palette::{Palette, ColorMode, PathMapStyle, PathCaptureMode};
use crate::config::FractalConfig;

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
}

/// Manages fractal flame rendering via GPU compute shaders
pub struct FlameRenderer {
    pipelines: FlamePipelines,
    buffers: FlameBuffers,
    compute_bind_group: BindGroup,
    accumulate_bind_group: BindGroup,
    // adjust_scale_bind_group removed - pipeline unused
    tonemap_bind_group: BindGroup,

    // Output texture that tonemap_pass renders to (for both display and export)
    fractal_texture: Texture,
    fractal_texture_view: TextureView,

    pub width: u32,
    pub height: u32,
    samples_accumulated: u64,
    total_iterations: u64,
    effective_iterations: u64, // For brightness calculation - doesn't reset during overwrite mode
    color_mode: ColorMode,
    path_map_style: PathMapStyle,
    path_capture_mode: PathCaptureMode,
    density_scale: f32,
    background_color: [f32; 3],
    current_render_mode: crate::scene::transforms::RenderMode,
    perspective_strength: f32,
    deterministic_rng: bool,
    frame_counter: u32, // For deterministic seed progression
    histogram_color_scale: f32, // Precision vs overflow (default: 10.0)
    low_density_smoothing: f32, // 0.0 = no smoothing, 1.0 = max smoothing (default: 0.5)
    density_compression_strength: f32, // 0.0 = linear, 5.0 = strong compression (default: 0.0)
    burn_in: u32, // Burn-in iterations (for Depth gradient in PathMap mode)
    blend_factor: f32, // Accumulation blend rate: 0.01 (slow/smooth) to 1.0 (fast/flickery), default: 0.1
    use_dynamic_blend: bool, // true = exponential convergence (old), false = fixed blend rate (new)
    target_iterations_per_pixel: u32, // Per-pixel convergence: stop updating pixel after N iterations (0 = disabled)
    overwrite_mode: bool, // When true, replace accumulation buffer instead of blending (for live preview)
    num_transforms: u32, // Number of regular transforms (not including final transform)
    has_final_transform: bool, // Whether final transform is present
}

impl FlameRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        width: u32,
        height: u32,
        flame: &Flame,
    ) -> Self {
        let pipelines = FlamePipelines::new(device, surface_format, flame);
        let buffers = FlameBuffers::new(device, queue, width, height, flame);

        let compute_bind_group = pipelines.create_compute_bind_group(device, &buffers);
        let accumulate_bind_group = pipelines.create_accumulate_bind_group(device, &buffers);
        // adjust_scale_bind_group removed - pipeline unused
        let tonemap_bind_group = pipelines.create_tonemap_bind_group(device, &buffers);

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
            // adjust_scale_bind_group removed
            tonemap_bind_group,
            fractal_texture,
            fractal_texture_view,
            width,
            height,
            samples_accumulated: 0,
            total_iterations: 0,
            effective_iterations: 0,
            color_mode: ColorMode::Palette,
            path_map_style: PathMapStyle::default(),
            path_capture_mode: PathCaptureMode::default(),
            density_scale: 1.0,
            background_color: [0.0, 0.0, 0.0],
            current_render_mode: flame.render_mode,
            perspective_strength: flame.perspective_strength,
            deterministic_rng: true, // Default to deterministic for reproducible rendering
            frame_counter: 0,
            histogram_color_scale: crate::config::DEFAULT_HISTOGRAM_COLOR_SCALE,
            low_density_smoothing: 0.5, // Moderate smoothing default
            density_compression_strength: 0.0, // Linear accumulation default (no compression)
            burn_in: 20, // Default burn-in iterations
            blend_factor: 0.1, // 10% blend rate - good balance between speed and smoothness
            use_dynamic_blend: true, // Default to clamped exponential (0.8 → 0.01)
            target_iterations_per_pixel: 0, // Default: disabled (no per-pixel convergence)
            overwrite_mode: false, // Default to normal blending (progressive refinement)
            num_transforms: flame.transforms.len() as u32,
            has_final_transform: flame.final_transform.is_some(),
        }
    }

    /// Resize the accumulation buffer
    pub fn resize(&mut self, device: &Device, encoder: &mut CommandEncoder, queue: &Queue, width: u32, height: u32, flame: &Flame, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_z: f32, speed_factor: f32) {
        self.width = width;
        self.height = height;

        // Recreate buffers with new size
        self.buffers = FlameBuffers::new(device, queue, width, height, flame);

        // Recreate bind groups
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
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
        self.reset(encoder, queue, iterations_per_thread, zoom, pan_x, pan_y, rotation, camera_rotation_x, camera_rotation_y, camera_z, speed_factor);

        // NOTE: Tonemap params need to be restored after buffer recreation
        // The caller should call update_tonemap() with current config values after resize()
    }

    /// Reset iteration counters without clearing accumulation buffer
    /// Used when transitioning from overwrite mode to normal accumulation
    pub fn reset_iteration_counter(&mut self) {
        self.samples_accumulated = 0;
        self.total_iterations = 0;
        self.effective_iterations = 0; // Reset for new accumulation phase
        self.frame_counter = 0; // Reset frame counter for deterministic seed progression
    }

    /// Reset accumulation buffer and sample count (full reset including effective iterations)
    pub fn reset(&mut self, encoder: &mut CommandEncoder, queue: &Queue, _iterations_per_thread: u32, _zoom: f32, _pan_x: f32, _pan_y: f32, _rotation: f32, _camera_rotation_x: f32, _camera_rotation_y: f32, _camera_z: f32, _speed_factor: f32) {
        self.reset_iteration_counter();
        // Reset effective_iterations when doing a full reset (buffer cleared)
        self.effective_iterations = 0;

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
    pub fn compute_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, num_workgroups: u32, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_z: f32, speed_factor: f32, clear_histogram: bool, clear_paths: bool) -> u64 {
        // Update seed for new random samples each frame
        // projection_type removed - shader now uses perspective_strength directly
        // 0.0 = orthographic (flat), higher values = increasing perspective

        let seed = self.get_rng_seed();
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
            },
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            perspective_strength: self.perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            camera_z,
            histogram_color_scale: self.histogram_color_scale,
            has_final_transform: if self.has_final_transform { 1 } else { 0 },
            final_transform_index: self.num_transforms, // Final transform is appended after regular transforms
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
        };
        self.buffers.update_params(queue, &params);

        // Track total iterations: workgroups * threads_per_workgroup * iterations_per_thread
        // Each workgroup has 64 threads (8x8)
        let threads_per_workgroup = 64u64;
        let samples_this_frame = num_workgroups as u64 * threads_per_workgroup * iterations_per_thread as u64;
        self.total_iterations += samples_this_frame;

        // Clear histogram buffer before each batch (needed for proper accumulation math)
        if clear_histogram {
            self.buffers.clear_histogram(encoder);
        }
        // Clear path buffer only on full reset (view change, flame change, etc.)
        // Path buffer persists across batches to accumulate path data for all pixels
        if clear_paths {
            self.buffers.clear_paths(encoder);
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
        self.samples_accumulated += samples_this_frame;

        // Update effective_iterations (for brightness) only when NOT in overwrite mode
        // This prevents brightness flash when exiting overwrite/preview mode
        if !self.overwrite_mode {
            self.effective_iterations += samples_this_frame;
        }

        // Calculate blend_factor based on mode
        let blend_factor = if self.overwrite_mode {
            // Overwrite mode (live preview): Replace old buffer entirely
            // Prevents mixing of different fractal states during drag
            1.0
        } else if self.use_dynamic_blend {
            // Clamped exponential decay: Start at 0.8 for fast initial convergence,
            // decay over time but never drop below 0.01 so iterations always contribute
            let raw_blend = samples_this_frame as f32 / self.samples_accumulated as f32;
            let clamped_blend = raw_blend.max(0.01).min(0.8);
            clamped_blend
        } else {
            // Fixed blend rate: constant blend per frame
            // Useful for testing density compression effects
            self.blend_factor
        };

        let params = AccumulateParams {
            width: self.width,
            height: self.height,
            blend_factor,
            histogram_color_scale: self.histogram_color_scale,
            low_density_smoothing: self.low_density_smoothing,
            density_compression_strength: self.density_compression_strength,
            target_iterations_per_pixel: self.target_iterations_per_pixel,
            _pad0: 0.0,
            background_r: self.background_color[0],
            background_g: self.background_color[1],
            background_b: self.background_color[2],
            _pad1: 0.0,
        };

        self.buffers.update_accumulate_params(queue, &params);

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

    /// Render the accumulation buffer to internal fractal texture with tone mapping
    pub fn tonemap_pass(&self, encoder: &mut CommandEncoder) {
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
        });

        render_pass.set_pipeline(&self.pipelines.tonemap_pipeline);
        render_pass.set_bind_group(0, &self.tonemap_bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Fullscreen triangle

        drop(render_pass);
    }

    /// Debug: Read back scale buffer and compute statistics
    // Note: debug_scale_stats() removed - scale_buffer no longer exists

    /// Load a complete FractalConfig (preset or imported config)
    /// This ensures all GPU state is properly synchronized
    pub fn load_config(&mut self, device: &Device, encoder: &mut CommandEncoder, queue: &Queue, config: &FractalConfig, palette: &Palette, iterations_per_thread: u32, burn_in: u32) {
        // 0. Check if shaders need to be recompiled (variations changed)
        let shaders_changed = self.pipelines.ensure_shaders_current(device, &config.flame);
        if shaders_changed {
            log::info!("Shaders recompiled during preset load - recreating bind group");
            // Recreate compute bind group with new pipeline
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        }

        // 1. Update transforms in GPU buffer
        self.buffers.update_transforms(queue, &config.flame);

        // 2. Update color mode, path map style, and capture mode
        self.color_mode = config.color_mode;
        self.path_map_style = config.path_map_style;
        self.path_capture_mode = config.path_capture_mode;

        // 3. Update density and background
        self.density_scale = config.density_scale;
        self.background_color = config.background_color;

        // 4. Update render mode and perspective
        self.current_render_mode = config.flame.render_mode;
        self.perspective_strength = config.flame.perspective_strength;
        self.histogram_color_scale = config.histogram_color_scale;
        self.low_density_smoothing = config.low_density_smoothing;
        self.burn_in = burn_in;

        // 5. Update palette with hue rotation
        self.buffers.update_palette(queue, palette, config.palette_rotation);

        // Note: scale_buffer removed - scale is now in params.histogram_color_scale

        // 6. Update ALL GPU params with correct num_transforms, render_mode, perspective

        // Update transform tracking
        self.num_transforms = config.flame.transforms.len() as u32;
        self.has_final_transform = config.flame.final_transform.is_some();

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
            },
            splat_size: 1.0,
            zoom: config.zoom,
            pan_x: config.pan_x,
            pan_y: config.pan_y,
            rotation: config.rotation,
            speed_factor: config.speed_factor,
            perspective_strength: self.perspective_strength,
            camera_rotation_x: config.camera_rotation_x,
            camera_rotation_y: config.camera_rotation_y,
            camera_z: config.camera_z,
            histogram_color_scale: config.histogram_color_scale,
            has_final_transform: if self.has_final_transform { 1 } else { 0 },
            final_transform_index: self.num_transforms,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
        };
        self.buffers.update_params(queue, &params);

        // 7. Update deterministic RNG setting
        self.deterministic_rng = config.deterministic_rng;

        // 8. Update tone mapping settings from config
        // Not in live preview mode (loading config)
        self.update_tonemap(queue, config.tonemap_mode, config.use_curve, config.exposure, config.gamma, config.gamma_threshold, config.brightness, config.vibrancy, config.saturation, config.hue_shift, config.value_scale, config.alpha_blend_low, config.alpha_blend_high, self.width, self.height, self.total_iterations, config.max_iterations, config.zoom, iterations_per_thread, 1, false);
        self.update_curve_lut(queue, &config.tonemap_curve);

        // 9. Clear accumulation buffers
        self.buffers.clear_all(encoder, queue);
        self.samples_accumulated = 0;
        self.total_iterations = 0;
    }

    /// Update the flame being rendered
    pub fn update_flame(&mut self, device: &Device, queue: &Queue, flame: &Flame, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_z: f32, speed_factor: f32) {
        // Check if shaders need to be recompiled (variations changed)
        let shaders_changed = self.pipelines.ensure_shaders_current(device, flame);
        if shaders_changed {
            log::info!("Shaders recompiled due to variation changes - recreating bind group");
            // Recreate compute bind group with new pipeline
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        }

        self.buffers.update_transforms(queue, flame);
        self.buffers.update_variation_params(queue, flame);

        // Update render mode and perspective
        self.current_render_mode = flame.render_mode;
        self.perspective_strength = flame.perspective_strength;

        // Update transform tracking
        self.num_transforms = flame.transforms.len() as u32;
        self.has_final_transform = flame.final_transform.is_some();

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
            },
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            perspective_strength: self.perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            camera_z,
            histogram_color_scale: self.histogram_color_scale,
            has_final_transform: if self.has_final_transform { 1 } else { 0 },
            final_transform_index: self.num_transforms,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
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
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: DEFAULT_SATURATION,
            hue_shift: DEFAULT_HUE_SHIFT,
            value_scale: DEFAULT_VALUE_SCALE,
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
            _pad_end: [0, 0, 0],
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    pub fn samples_accumulated(&self) -> u64 {
        self.samples_accumulated
    }

    pub fn total_iterations(&self) -> u64 {
        self.total_iterations
    }

    /// Get fractal output texture view for display
    pub fn get_fractal_texture_view(&self) -> &TextureView {
        &self.fractal_texture_view
    }

    /// Get fractal output texture (for copy operations)
    pub fn fractal_texture(&self) -> &Texture {
        &self.fractal_texture
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

    pub fn set_histogram_color_scale(&mut self, queue: &Queue, scale: f32, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_z: f32, speed_factor: f32) {
        self.histogram_color_scale = scale;
        // Update GPU params immediately so new scale takes effect
        self.update_iterations(queue, iterations_per_thread, burn_in, zoom, pan_x, pan_y, rotation, camera_rotation_x, camera_rotation_y, camera_z, speed_factor);
    }

    pub fn set_low_density_smoothing(&mut self, smoothing: f32) {
        self.low_density_smoothing = smoothing;
        // Note: This will take effect on the next accumulate pass (no need to update GPU params immediately)
    }

    /// Set density compression strength (0.0 = linear, 100.0 = strong compression)
    pub fn set_density_compression_strength(&mut self, strength: f32) {
        self.density_compression_strength = strength;
        // Note: This will take effect on the next accumulate pass (no need to update GPU params immediately)
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

    /// Set per-pixel iteration limit (0 = disabled)
    pub fn set_target_iterations_per_pixel(&mut self, target: u32) {
        self.target_iterations_per_pixel = target;
    }

    /// Set overwrite mode (live preview)
    /// When true, accumulation buffer is replaced instead of blended
    pub fn set_overwrite_mode(&mut self, overwrite: bool) {
        self.overwrite_mode = overwrite;
    }

    /// Update iterations per thread
    pub fn update_iterations(&mut self, queue: &Queue, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_z: f32, speed_factor: f32) {
        self.burn_in = burn_in;

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
            },
            perspective_strength: self.perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            camera_z,
            histogram_color_scale: self.histogram_color_scale,
            has_final_transform: if self.has_final_transform { 1 } else { 0 },
            final_transform_index: self.num_transforms,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
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
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: DEFAULT_SATURATION,
            hue_shift: DEFAULT_HUE_SHIFT,
            value_scale: DEFAULT_VALUE_SCALE,
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
            _pad_end: [0, 0, 0],
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
    pub fn set_transparent_mode(&self, queue: &Queue, transparent: bool, config: &FractalConfig, iterations_per_thread: u32) {
        use crate::config::defaults::*;

        let tonemap_mode_u32 = match config.tonemap_mode {
            crate::scene::tonemap::ToneMapMode::Linear => 0u32,
            crate::scene::tonemap::ToneMapMode::Logarithmic => 1u32,
            crate::scene::tonemap::ToneMapMode::DensityVisualization => 2u32,
        };

        // Calculate area and sample_density (simplified for export)
        let apophysis_zoom = config.zoom.log2();
        let base_pixels_per_unit = (self.width.min(self.height) as f32) * 0.25;
        let pixels_per_unit_zoomed = base_pixels_per_unit * (2.0_f32).powf(apophysis_zoom);
        let area = (self.width * self.height) as f32 / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);
        // Resolution normalization: scale inversely with pixel count (reference: 1M pixels)
        let total_pixels = (self.width * self.height) as f32;
        let reference_pixels = 1_000_000.0;
        let sample_density = 5000.0 * (iterations_per_thread as f32 / 256.0) * (reference_pixels / total_pixels);

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
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation: config.saturation,
            hue_shift: config.hue_shift,
            value_scale: config.value_scale,
            gamma_threshold: config.gamma_threshold,
            alpha_blend_low: config.alpha_blend_low,
            alpha_blend_high: config.alpha_blend_high,
            transparent_mode: if transparent { 1 } else { 0 },
            color_mode: self.color_mode as u32,
            width: self.width,
            height: self.height,
            path_map_style: self.path_map_style as u32,
            burn_in: self.burn_in,
            num_transforms: self.num_transforms,
            _pad_end: [0, 0, 0],
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Update tone mapping mode, curve usage, exposure, gamma, gamma_threshold, brightness, vibrancy, saturation, hue shift, value scale, and alpha blend
    pub fn update_tonemap(&self, queue: &Queue, tonemap_mode: crate::scene::tonemap::ToneMapMode, use_curve: bool, exposure: f32, gamma: f32, gamma_threshold: f32, brightness: f32, vibrancy: f32, saturation: f32, hue_shift: f32, value_scale: f32, alpha_blend_low: f32, alpha_blend_high: f32, width: u32, height: u32, _total_iterations: u64, _max_iterations: u64, zoom: f32, iterations_per_thread: u32, _batch_size: u32, is_live_preview: bool) {
        use crate::config::defaults::*;

        let tonemap_mode_u32 = match tonemap_mode {
            crate::scene::tonemap::ToneMapMode::Linear => 0u32,
            crate::scene::tonemap::ToneMapMode::Logarithmic => 1u32,
            crate::scene::tonemap::ToneMapMode::DensityVisualization => 2u32,
        };

        // Calculate area and sample_density for brightness lookup table with Apophysis zoom compensation
        // Apophysis ImageMaker.pas:448-452:
        //   sample_density := fcp.actual_density * sqr(power(2, fcp.zoom));
        //   area := FBitmap.Width * FBitmap.Height / (fcp.ppux * fcp.ppuy);
        //   where ppux = pixels_per_unit * 2^zoom
        //
        // This normalizes brightness across zoom levels:
        // - Zoomed in: Higher sample_density → smaller k2 → less brightness boost
        // - Zoomed out: Lower sample_density → larger k2 → more brightness boost
        //
        // NOTE: Our zoom is LINEAR (zoom=1.0 is default, zoom=2.0 is 2x scale)
        //       Apophysis zoom is LOGARITHMIC (zoom=0 is default, zoom=1 means scale by 2^1=2)
        //       Convert: apophysis_zoom = log2(our_zoom)

        // Convert our linear zoom to Apophysis logarithmic zoom
        let apophysis_zoom = zoom.log2();  // our zoom=1.0 → apophysis zoom=0, our zoom=2.0 → apophysis zoom=1

        // Calculate pixels per unit at current zoom (ppux = ppuy for square pixels)
        // Base pixels_per_unit is chosen to match our coordinate system
        let base_pixels_per_unit = (width.min(height) as f32) * 0.25;  // From world_to_pixel scale
        let pixels_per_unit_zoomed = base_pixels_per_unit * (2.0_f32).powf(apophysis_zoom);

        // Area in fractal space (not pixel space!)
        let area = (width * height) as f32 / (pixels_per_unit_zoomed * pixels_per_unit_zoomed);

        // Sample density: Normalized reference value for consistent brightness
        //
        // KEY INSIGHT: bucket_count accumulation rate depends on iterations_per_thread!
        // - More iterations per frame → more hits per frame → higher bucket_count growth rate
        // - So sample_density must scale proportionally to match the hit rate
        //
        // SOLUTION: Use a reference value normalized to default iterations_per_thread (256)
        // - Base value: 5000.0 (empirically chosen for good exposure)
        //   - Much higher than Apophysis (50-100) because we generate ~100x more iterations per batch
        // - Scale factor: (iterations_per_thread / 256.0)
        //   - At default (256): sample_density = 5000.0 × 1.0 = 5000.0
        //   - At half (128): sample_density = 5000.0 × 0.5 = 2500.0
        //   - At double (512): sample_density = 5000.0 × 2.0 = 10000.0
        //
        // This ensures brightness remains consistent when changing iterations_per_thread:
        // - Both bucket_count growth and sample_density scale together
        // - The ratio stays constant → brightness stays constant
        // - iterations_per_thread only affects render speed, not appearance
        //
        // Resolution normalization: Scale sample_density inversely with pixel count
        // to keep area × sample_density constant across different render sizes.
        // Reference: 1,000,000 pixels (1000×1000)
        let total_pixels = (width * height) as f32;
        let reference_pixels = 1_000_000.0;
        let mut sample_density = 5000.0 * (iterations_per_thread as f32 / 256.0) * (reference_pixels / total_pixels);

        // Live preview mode: Divide by 8 for brighter preview
        // This compensates for lower density accumulation during live parameter editing
        // Only applies during active editing (is_live_preview), not when rendering stops
        if is_live_preview {
            sample_density /= 8.0;
        }

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
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area,
            sample_density,
            saturation,
            hue_shift,
            value_scale,
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
            _pad_end: [0, 0, 0],
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Update tone curve LUT texture
    pub fn update_curve_lut(&self, queue: &Queue, curve: &crate::scene::tonemap::ToneCurve) {
        self.buffers.update_curve_lut(queue, curve);
    }

    /// Update palette texture
    pub fn update_palette(&mut self, device: &Device, queue: &Queue, palette: &Palette, hue_rotation: f32) {
        self.buffers.update_palette(queue, palette, hue_rotation);
        // Recreate compute bind group to ensure palette texture is bound
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
    }

    /// Set color mode
    pub fn set_color_mode(&mut self, queue: &Queue, color_mode: ColorMode, iterations_per_thread: u32, burn_in: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, camera_z: f32, speed_factor: f32) {
        self.color_mode = color_mode;

        // Update params to reflect new color mode
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
            },
            perspective_strength: self.perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            camera_z,
            histogram_color_scale: self.histogram_color_scale,
            has_final_transform: if self.has_final_transform { 1 } else { 0 },
            final_transform_index: self.num_transforms,
            bits_per_transform: crate::gpu::buffers::bits_per_transform(self.num_transforms),
            path_map_style: self.path_map_style as u32,
            path_capture_mode: self.path_capture_mode as u32,
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

    /// Read pixels from the fractal_texture (after tonemap_pass has rendered to it)
    /// This is the unified method that reads what was actually displayed on screen.
    ///
    /// # Arguments
    /// * `transparent` - If true, preserve alpha channel; if false, blend with background and set alpha=255
    /// * `background_color` - RGB background color for opaque mode (ignored in transparent mode)
    pub async fn read_fractal_pixels(
        &self,
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

        // Create staging buffer
        let bytes_per_pixel = 4; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Fractal Read Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

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

        // Map and read
        let buffer_slice = buffer.slice(..);
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

        Ok((self.width, self.height, rgba_data))
    }

    /// Read path buffer from GPU for CPU-side path queries
    /// Returns a 2D array of PathEntry indexed by [y][x]
    pub async fn read_path_buffer(
        &self,
        device: &Device,
        queue: &Queue,
    ) -> Result<Vec<Vec<PathEntry>>, String> {
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
            &self.buffers.path_buffer,
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
    /// This preserves true alpha values by reading raw Rgba16Float accumulation data
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

        // Create buffer to copy accumulation texture data (Rgba16Float format)
        let bytes_per_pixel = 8; // Rgba16Float = 4 channels × 2 bytes each
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

        // Map buffer and read Rgba16Float data
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await.map_err(|_| "Failed to map buffer".to_string())?
            .map_err(|e| format!("Buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Convert Rgba16Float to Rgba8 with CPU tone mapping
        let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);

        // Iterate row by row to handle padding
        for y in 0..self.height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_data = &data[row_start..row_start + (self.width * bytes_per_pixel) as usize];

            for chunk in row_data.chunks_exact(8) {
                // Read f16 values and convert to f32
                let r = half::f16::from_le_bytes([chunk[0], chunk[1]]).to_f32();
                let g = half::f16::from_le_bytes([chunk[2], chunk[3]]).to_f32();
                let b = half::f16::from_le_bytes([chunk[4], chunk[5]]).to_f32();
                let density = half::f16::from_le_bytes([chunk[6], chunk[7]]).to_f32();

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
        self.tonemap_pass(&mut encoder);

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
