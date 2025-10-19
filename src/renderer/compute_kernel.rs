use wgpu::*;
use crate::gpu::{buffers::*, pipelines::FlamePipelines};
use crate::scene::transforms::Flame;
use crate::scene::palette::{Palette, ColorMode};

/// Manages fractal flame rendering via GPU compute shaders
pub struct FlameRenderer {
    pipelines: FlamePipelines,
    buffers: FlameBuffers,
    compute_bind_group: BindGroup,
    accumulate_bind_group: BindGroup,
    tonemap_bind_group: BindGroup,
    width: u32,
    height: u32,
    samples_accumulated: u32,
    total_iterations: u64,
    color_mode: ColorMode,
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
        let pipelines = FlamePipelines::new(device, surface_format);
        let buffers = FlameBuffers::new(device, queue, width, height, flame);

        let compute_bind_group = pipelines.create_compute_bind_group(device, &buffers);
        let accumulate_bind_group = pipelines.create_accumulate_bind_group(device, &buffers);
        let tonemap_bind_group = pipelines.create_tonemap_bind_group(device, &buffers);

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
        }
    }

    /// Resize the accumulation buffer
    pub fn resize(&mut self, device: &Device, encoder: &mut CommandEncoder, queue: &Queue, width: u32, height: u32, flame: &Flame, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32) {
        self.width = width;
        self.height = height;

        // Recreate buffers with new size
        self.buffers = FlameBuffers::new(device, queue, width, height, flame);

        // Recreate bind groups
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.accumulate_bind_group = self.pipelines.create_accumulate_bind_group(device, &self.buffers);
        self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);

        // Clear accumulation counter
        self.reset(encoder, queue, iterations_per_thread, zoom, pan_x, pan_y);
    }

    /// Reset accumulation buffer and sample count
    pub fn reset(&mut self, encoder: &mut CommandEncoder, queue: &Queue, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32) {
        self.samples_accumulated = 0;
        self.total_iterations = 0;

        // Clear accumulation buffers
        self.buffers.clear_all(encoder);

        // Update seed to generate different random samples
        let params = GpuParams {
            num_transforms: 2, // Will be updated when flame changes
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: rand::random::<u32>(),
            color_mode: self.color_mode as u32,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            speed_factor: 0.5,
        };

        self.buffers.update_params(queue, &params);
    }

    /// Run compute pass to generate flame samples
    pub fn compute_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, num_workgroups: u32, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, speed_factor: f32) {
        // Update seed for new random samples each frame
        let params = GpuParams {
            num_transforms: self.buffers.transform_buffer.size() as u32 / std::mem::size_of::<GpuTransform>() as u32,
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: rand::random::<u32>(),
            color_mode: self.color_mode as u32,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            speed_factor,
        };
        self.buffers.update_params(queue, &params);

        // Track total iterations: workgroups * threads_per_workgroup * iterations_per_thread
        // Each workgroup has 64 threads (8x8)
        let threads_per_workgroup = 64u64;
        self.total_iterations += num_workgroups as u64 * threads_per_workgroup * iterations_per_thread as u64;

        // Clear temp samples texture before rendering new samples
        self.buffers.clear_temp(encoder);

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Flame Compute Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.pipelines.compute_pipeline);
        compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
        compute_pass.dispatch_workgroups(num_workgroups, 1, 1);

        drop(compute_pass);
    }

    /// Run accumulation pass to blend new samples with previous accumulation
    pub fn accumulate_pass(&mut self, encoder: &mut CommandEncoder, queue: &Queue, device: &Device) {
        self.samples_accumulated += 1;

        // Calculate blend factor for exponential moving average
        // blend_factor = 1 / samples_accumulated gives equal weight to all samples
        let blend_factor = 1.0 / self.samples_accumulated as f32;

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

    /// Update the flame being rendered
    pub fn update_flame(&mut self, queue: &Queue, flame: &Flame, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, speed_factor: f32) {
        self.buffers.update_transforms(queue, flame);

        let params = GpuParams {
            num_transforms: flame.transforms.len() as u32,
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: rand::random::<u32>(),
            color_mode: self.color_mode as u32,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            speed_factor,
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
            _pad0: 0.0,
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    pub fn samples_accumulated(&self) -> u32 {
        self.samples_accumulated
    }

    pub fn total_iterations(&self) -> u64 {
        self.total_iterations
    }

    /// Update iterations per thread
    pub fn update_iterations(&self, queue: &Queue, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, speed_factor: f32) {
        let params = GpuParams {
            num_transforms: self.buffers.transform_buffer.size() as u32 / std::mem::size_of::<GpuTransform>() as u32,
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: rand::random::<u32>(),
            color_mode: self.color_mode as u32,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            speed_factor,
        };
        self.buffers.update_params(queue, &params);
    }

    /// Update density scale for alpha blending
    pub fn update_density_scale(&self, queue: &Queue, density_scale: f32) {
        let params = TonemapParams {
            exposure: 1.0,
            gamma: 2.2,
            density_scale,
            _pad0: 0.0,
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    /// Update palette texture
    pub fn update_palette(&mut self, device: &Device, queue: &Queue, palette: &Palette) {
        self.buffers.update_palette(queue, palette);
        // Recreate compute bind group to ensure palette texture is bound
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
    }

    /// Set color mode
    pub fn set_color_mode(&mut self, queue: &Queue, color_mode: ColorMode, iterations_per_thread: u32, zoom: f32, pan_x: f32, pan_y: f32, speed_factor: f32) {
        self.color_mode = color_mode;
        // Update params to reflect new color mode
        let params = GpuParams {
            num_transforms: self.buffers.transform_buffer.size() as u32 / std::mem::size_of::<GpuTransform>() as u32,
            iterations_per_thread,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: rand::random::<u32>(),
            color_mode: self.color_mode as u32,
            splat_size: 1.0,
            zoom,
            pan_x,
            pan_y,
            speed_factor,
        };
        self.buffers.update_params(queue, &params);
    }

    /// Get current color mode
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }
}
