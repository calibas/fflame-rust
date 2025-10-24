use wgpu::*;
use crate::gpu::{buffers::*, pipelines::FlamePipelines};
use crate::scene::transforms::Flame;
use crate::scene::palette::{Palette, ColorMode};
use crate::config::FractalConfig;

/// Manages fractal flame rendering via GPU compute shaders
pub struct FlameRenderer {
    pipelines: FlamePipelines,
    buffers: FlameBuffers,
    compute_bind_group: BindGroup,
    accumulate_bind_group: BindGroup,
    tonemap_bind_group: BindGroup,
    pub width: u32,
    pub height: u32,
    samples_accumulated: u64,
    total_iterations: u64,
    color_mode: ColorMode,
    density_scale: f32,
    background_color: [f32; 3],
    current_render_mode: crate::scene::transforms::RenderMode,
    current_projection: crate::scene::transforms::ProjectionType,
    deterministic_rng: bool,
    frame_counter: u32, // For deterministic seed progression
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
        let tonemap_bind_group = pipelines.create_tonemap_bind_group(device, &buffers);

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
            tonemap_bind_group,
            width,
            height,
            samples_accumulated: 0,
            total_iterations: 0,
            color_mode: ColorMode::Transform,
            density_scale: 1.0,
            background_color: [0.0, 0.0, 0.0],
            current_render_mode: flame.render_mode,
            current_projection: flame.projection,
            deterministic_rng: true, // Default to deterministic for reproducible rendering
            frame_counter: 0,
        }
    }

    /// Resize the accumulation buffer
    pub fn resize(&mut self, device: &Device, encoder: &mut CommandEncoder, queue: &Queue, width: u32, height: u32, flame: &Flame, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, speed_factor: f32) {
        self.width = width;
        self.height = height;

        // Recreate buffers with new size
        self.buffers = FlameBuffers::new(device, queue, width, height, flame);

        // Recreate bind groups
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
        self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);

        // Clear accumulation counter
        self.reset(encoder, queue, iterations_per_thread, zoom, pan_x, pan_y, rotation, camera_rotation_x, camera_rotation_y, speed_factor);
    }

    /// Reset accumulation buffer and sample count
    pub fn reset(&mut self, encoder: &mut CommandEncoder, _queue: &Queue, _iterations_per_thread: u32, _zoom: f32, _pan_x: f32, _pan_y: f32, _rotation: f32, _camera_rotation_x: f32, _camera_rotation_y: f32, _speed_factor: f32) {
        self.samples_accumulated = 0;
        self.total_iterations = 0;
        self.frame_counter = 0; // Reset frame counter for deterministic seed progression

        // Clear accumulation buffers
        self.buffers.clear_all(encoder);

        // Note: We don't update params here because update_flame() already set them correctly.
        // Updating params here would overwrite num_transforms which was just set by update_flame().
    }

    /// Run compute pass to generate flame samples
    /// Returns the number of samples generated this frame
    pub fn compute_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, num_workgroups: u32, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, speed_factor: f32) -> u64 {
        // Update seed for new random samples each frame
        let (projection_type, perspective_strength) = match self.current_projection {
            crate::scene::transforms::ProjectionType::Orthographic => (0u32, 2.0f32),
            crate::scene::transforms::ProjectionType::Perspective { strength } => (1u32, strength),
        };

        let seed = self.get_rng_seed();
        let params = GpuParams {
            num_transforms: self.buffers.transform_buffer.size() as u32 / std::mem::size_of::<GpuTransform>() as u32,
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed,
            color_mode: self.color_mode as u32,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
            },
            projection_type,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            _pad3: 0.0,
            _pad4: 0.0,
        };
        log::info!("Uploading params with seed={}", seed);
        self.buffers.update_params(queue, &params);

        // Track total iterations: workgroups * threads_per_workgroup * iterations_per_thread
        // Each workgroup has 64 threads (8x8)
        let threads_per_workgroup = 64u64;
        let samples_this_frame = num_workgroups as u64 * threads_per_workgroup * iterations_per_thread as u64;
        self.total_iterations += samples_this_frame;

        // Clear temp samples texture before rendering new samples
        self.buffers.clear_temp(encoder);

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

    /// Run accumulation pass to blend new samples with previous accumulation
    pub fn accumulate_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, device: &Device, samples_this_frame: u64) {
        self.samples_accumulated += samples_this_frame;

        // Calculate blend factor for exponential moving average
        // blend_factor = samples_this_frame / samples_accumulated
        // This properly weights each frame by the number of samples it contributes
        let blend_factor = samples_this_frame as f32 / self.samples_accumulated as f32;

        let params = AccumulateParams {
            width: self.width,
            height: self.height,
            blend_factor,
            _pad0: 0.0,
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
        self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);
    }

    /// Render the accumulation buffer to a texture view with tone mapping
    pub fn tonemap_pass(&self, encoder: &mut CommandEncoder, target_view: &TextureView) {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Tonemap Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
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

    /// Load a complete FractalConfig (preset or imported config)
    /// This ensures all GPU state is properly synchronized
    pub fn load_config(&mut self, device: &Device, encoder: &mut CommandEncoder, queue: &Queue, config: &FractalConfig, palette: &Palette, iterations_per_thread: u32) {
        // 0. Check if shaders need to be recompiled (variations changed)
        let shaders_changed = self.pipelines.ensure_shaders_current(device, &config.flame);
        if shaders_changed {
            log::info!("Shaders recompiled during preset load - recreating bind group");
            // Recreate compute bind group with new pipeline
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        }

        // 1. Update transforms in GPU buffer
        self.buffers.update_transforms(queue, &config.flame);

        // 2. Update color mode
        self.color_mode = config.color_mode;

        // 3. Update density and background
        self.density_scale = config.density_scale;
        self.background_color = config.background_color;

        // 4. Update render mode and projection
        self.current_render_mode = config.flame.render_mode;
        self.current_projection = config.flame.projection;

        // 5. Update palette
        self.buffers.update_palette(queue, palette);

        // 6. Update ALL GPU params with correct num_transforms, render_mode, projection
        let (projection_type, perspective_strength) = match self.current_projection {
            crate::scene::transforms::ProjectionType::Orthographic => (0u32, 2.0f32),
            crate::scene::transforms::ProjectionType::Perspective { strength } => (1u32, strength),
        };

        let params = GpuParams {
            num_transforms: config.flame.transforms.len() as u32,
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: self.get_rng_seed(),
            color_mode: config.color_mode as u32,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
            },
            projection_type,
            splat_size: 1.0,
            zoom: config.zoom,
            pan_x: config.pan_x,
            pan_y: config.pan_y,
            rotation: config.rotation,
            speed_factor: config.speed_factor,
            perspective_strength,
            camera_rotation_x: config.camera_rotation_x,
            camera_rotation_y: config.camera_rotation_y,
            _pad3: 0.0,
            _pad4: 0.0,
        };
        self.buffers.update_params(queue, &params);

        // 7. Clear accumulation buffers
        self.buffers.clear_all(encoder);
        self.samples_accumulated = 0;
        self.total_iterations = 0;
    }

    /// Update the flame being rendered
    pub fn update_flame(&mut self, device: &Device, queue: &Queue, flame: &Flame, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, speed_factor: f32) {
        // Check if shaders need to be recompiled (variations changed)
        let shaders_changed = self.pipelines.ensure_shaders_current(device, flame);
        if shaders_changed {
            log::info!("Shaders recompiled due to variation changes - recreating bind group");
            // Recreate compute bind group with new pipeline
            self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        }

        self.buffers.update_transforms(queue, flame);
        self.buffers.update_variation_params(queue, flame);

        // Update render mode and projection
        self.current_render_mode = flame.render_mode;
        self.current_projection = flame.projection;

        let (projection_type, perspective_strength) = match self.current_projection {
            crate::scene::transforms::ProjectionType::Orthographic => (0u32, 2.0f32),
            crate::scene::transforms::ProjectionType::Perspective { strength } => (1u32, strength),
        };

        let params = GpuParams {
            num_transforms: flame.transforms.len() as u32,
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: self.get_rng_seed(),
            color_mode: self.color_mode as u32,
            render_mode: match self.current_render_mode {
                crate::scene::transforms::RenderMode::TwoD => 0,
                crate::scene::transforms::RenderMode::ThreeD => 1,
            },
            projection_type,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            rotation,
            speed_factor,
            perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            _pad3: 0.0,
            _pad4: 0.0,
        };

        self.buffers.update_params(queue, &params);
        self.samples_accumulated = 0;
        self.total_iterations = 0;
    }

    /// Update tonemap parameters (exposure, gamma)
    pub fn update_tonemap_params(&self, queue: &Queue, exposure: f32, gamma: f32) {
        let params = TonemapParams {
            exposure,
            gamma,
            density_scale: 1.0,
            tonemap_mode: 1,  // Logarithmic
            background_color: [0.0, 0.0, 0.0],
            use_curve: 0,  // Disabled
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    pub fn samples_accumulated(&self) -> u64 {
        self.samples_accumulated
    }

    pub fn total_iterations(&self) -> u64 {
        self.total_iterations
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

    /// Update iterations per thread
    pub fn update_iterations(&mut self, queue: &Queue, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, speed_factor: f32) {
        let (projection_type, perspective_strength) = match self.current_projection {
            crate::scene::transforms::ProjectionType::Orthographic => (0u32, 2.0f32),
            crate::scene::transforms::ProjectionType::Perspective { strength } => (1u32, strength),
        };

        let params = GpuParams {
            num_transforms: self.buffers.transform_buffer.size() as u32 / std::mem::size_of::<GpuTransform>() as u32,
            iterations_per_thread,
            burn_in: 20,
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
            projection_type,
            perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            _pad3: 0.0,
            _pad4: 0.0,
        };
        self.buffers.update_params(queue, &params);
    }

    /// Helper method to update tonemap parameters with current state
    fn update_tonemap_state(&self, queue: &Queue) {
        let params = TonemapParams {
            exposure: 1.0,
            gamma: 2.2,
            density_scale: self.density_scale,
            tonemap_mode: 1,  // Logarithmic
            background_color: self.background_color,
            use_curve: 0,  // Disabled
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

    /// Update tone mapping mode, curve usage, exposure, and gamma
    pub fn update_tonemap(&self, queue: &Queue, tonemap_mode: crate::scene::tonemap::ToneMapMode, use_curve: bool, exposure: f32, gamma: f32) {
        let tonemap_mode_u32 = match tonemap_mode {
            crate::scene::tonemap::ToneMapMode::Linear => 0u32,
            crate::scene::tonemap::ToneMapMode::Logarithmic => 1u32,
        };

        let params = TonemapParams {
            exposure,
            gamma,
            density_scale: self.density_scale,
            tonemap_mode: tonemap_mode_u32,
            background_color: self.background_color,
            use_curve: if use_curve { 1u32 } else { 0u32 },
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Update tone curve LUT texture
    pub fn update_curve_lut(&self, queue: &Queue, curve: &crate::scene::tonemap::ToneCurve) {
        self.buffers.update_curve_lut(queue, curve);
    }

    /// Update palette texture
    pub fn update_palette(&mut self, device: &Device, queue: &Queue, palette: &Palette) {
        self.buffers.update_palette(queue, palette);
        // Recreate compute bind group to ensure palette texture is bound
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
    }

    /// Set color mode
    pub fn set_color_mode(&mut self, queue: &Queue, color_mode: ColorMode, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, rotation: f32, camera_rotation_x: f32, camera_rotation_y: f32, speed_factor: f32) {
        self.color_mode = color_mode;

        let (projection_type, perspective_strength) = match self.current_projection {
            crate::scene::transforms::ProjectionType::Orthographic => (0u32, 2.0f32),
            crate::scene::transforms::ProjectionType::Perspective { strength } => (1u32, strength),
        };

        // Update params to reflect new color mode
        let params = GpuParams {
            num_transforms: self.buffers.transform_buffer.size() as u32 / std::mem::size_of::<GpuTransform>() as u32,
            iterations_per_thread,
            burn_in: 20,
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
            projection_type,
            perspective_strength,
            camera_rotation_x,
            camera_rotation_y,
            _pad3: 0.0,
            _pad4: 0.0,
        };
        self.buffers.update_params(queue, &params);
    }

    /// Get current color mode
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    /// Capture raw RGBA pixel data from the current frame
    /// Returns (width, height, rgba_bytes) where rgba_bytes is in standard RGBA format
    ///
    /// # Implementation Note
    /// Uses two different paths based on transparency requirement:
    /// - **Transparent**: Reads Rgba16Float accumulation buffer and applies CPU tone mapping
    ///   to preserve true alpha values (density × density_scale)
    /// - **Opaque**: Renders via tonemap shader which blends with background color
    ///
    /// Why? The tonemap shader performs `mix(background_color, fractal_color, alpha)` which
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
        let buffer_size = (self.width * self.height * bytes_per_pixel) as u64;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Accumulation Capture Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy accumulation texture to buffer
        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                texture: self.buffers.current_accumulation_texture(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyBuffer {
                buffer: &buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * bytes_per_pixel),
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
        device.poll(Maintain::Wait);

        rx.await.map_err(|_| "Failed to map buffer".to_string())?
            .map_err(|e| format!("Buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Convert Rgba16Float to Rgba8 with CPU tone mapping
        let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);

        for chunk in data.chunks_exact(8) {
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

        drop(data);
        buffer.unmap();

        Ok((self.width, self.height, rgba_data))
    }

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
            format: surface_format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let texture = device.create_texture(&texture_desc);
        let view = texture.create_view(&TextureViewDescriptor::default());

        // Render to the texture
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Screenshot Encoder"),
        });
        self.tonemap_pass(&mut encoder, &view);

        // Create buffer to copy texture data to
        let buffer_size = (self.width * self.height * 4) as u64;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Screenshot Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyBuffer {
                buffer: &buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * 4),
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
        device.poll(Maintain::Wait);

        rx.await.map_err(|_| "Failed to map buffer".to_string())?
            .map_err(|e| format!("Buffer map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();
        let rgba_data: Vec<u8> = data.to_vec();
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

    /// Capture the current rendered frame as PNG data (convenience wrapper)
    /// If transparent is true, renders without background blending (alpha channel preserved)
    pub async fn capture_png(&mut self, device: &Device, queue: &Queue, transparent: bool, surface_format: TextureFormat) -> Result<Vec<u8>, String> {
        let (width, height, rgba_data) = self.capture_pixels(device, queue, transparent, surface_format).await?;

        // Encode as PNG
        encode_png_from_rgba(width, height, rgba_data)
    }
}

/// Standalone function to encode RGBA pixel data as PNG
/// This doesn't borrow anything and can be moved into async contexts
pub fn encode_png_from_rgba(width: u32, height: u32, rgba_data: Vec<u8>) -> Result<Vec<u8>, String> {
    use image::{ImageBuffer, Rgba};

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
        width,
        height,
        rgba_data,
    ).ok_or("Failed to create image buffer")?;

    // Flip vertically (GPU textures are upside down)
    let img = image::imageops::flip_vertical(&img);

    let mut png_data = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_data), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(png_data)
}
