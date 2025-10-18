use wgpu::*;
use crate::gpu::{buffers::*, pipelines::FlamePipelines};
use crate::scene::transforms::Flame;

/// Manages fractal flame rendering via GPU compute shaders
pub struct FlameRenderer {
    pipelines: FlamePipelines,
    buffers: FlameBuffers,
    compute_bind_group: BindGroup,
    tonemap_bind_group: BindGroup,
    width: u32,
    height: u32,
    samples_accumulated: u32,
}

impl FlameRenderer {
    pub fn new(
        device: &Device,
        surface_format: TextureFormat,
        width: u32,
        height: u32,
        flame: &Flame,
    ) -> Self {
        let pipelines = FlamePipelines::new(device, surface_format);
        let buffers = FlameBuffers::new(device, width, height, flame);

        let compute_bind_group = pipelines.create_compute_bind_group(device, &buffers);
        let tonemap_bind_group = pipelines.create_tonemap_bind_group(device, &buffers);

        Self {
            pipelines,
            buffers,
            compute_bind_group,
            tonemap_bind_group,
            width,
            height,
            samples_accumulated: 0,
        }
    }

    /// Resize the accumulation buffer
    pub fn resize(&mut self, device: &Device, queue: &Queue, width: u32, height: u32, flame: &Flame) {
        self.width = width;
        self.height = height;

        // Recreate buffers with new size
        self.buffers = FlameBuffers::new(device, width, height, flame);

        // Recreate bind groups
        self.compute_bind_group = self.pipelines.create_compute_bind_group(device, &self.buffers);
        self.tonemap_bind_group = self.pipelines.create_tonemap_bind_group(device, &self.buffers);

        // Clear accumulation counter
        self.reset(queue);
    }

    /// Reset accumulation buffer and sample count
    pub fn reset(&mut self, queue: &Queue) {
        self.samples_accumulated = 0;

        // Update seed to generate different random samples
        let params = GpuParams {
            num_transforms: 2, // Will be updated when flame changes
            iterations_per_thread: 256,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: rand::random::<u32>(),
            splat_size: 1.0,
            _pad0: 0.0,
        };

        self.buffers.update_params(queue, &params);
    }

    /// Run compute pass to generate flame samples
    pub fn compute_pass(&mut self, encoder: &mut CommandEncoder, num_workgroups: u32) {
        // Always clear - we're not accumulating across frames with write-only texture
        // TODO: Implement proper accumulation with atomic operations or dual textures
        self.buffers.clear(encoder);

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Flame Compute Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.pipelines.compute_pipeline);
        compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
        compute_pass.dispatch_workgroups(num_workgroups, 1, 1);

        drop(compute_pass);

        self.samples_accumulated += 1;
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
    pub fn update_flame(&mut self, queue: &Queue, flame: &Flame) {
        self.buffers.update_transforms(queue, flame);

        let params = GpuParams {
            num_transforms: flame.transforms.len() as u32,
            iterations_per_thread: 256,
            burn_in: 20,
            width: self.width,
            height: self.height,
            seed: rand::random::<u32>(),
            splat_size: 1.0,
            _pad0: 0.0,
        };

        self.buffers.update_params(queue, &params);
        self.samples_accumulated = 0;
    }

    /// Update tonemap parameters (exposure, gamma)
    pub fn update_tonemap_params(&self, queue: &Queue, exposure: f32, gamma: f32) {
        let params = TonemapParams {
            exposure,
            gamma,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        self.buffers.update_tonemap_params(queue, &params);
    }

    pub fn samples_accumulated(&self) -> u32 {
        self.samples_accumulated
    }
}
