//! GPU accumulate pass — consumes the sample stream produced by the
//! unified iteration shader (`OUTPUT_HISTOGRAM_DIRECT=false`) and writes
//! atomic-add updates into a histogram buffer.
//!
//! This is the host-side companion to `shaders/core/accumulate_samples.wgsl`.
//! Phase 5 wires `Strategy::ParallelTiles` to call into this; Phase 6 wires
//! `Strategy::SerialTiles` to do the same per-tile in a loop.
//!
//! The pipeline is cheap to construct (one shader module, one compute
//! pipeline, one bind-group layout) so we build it once per
//! `HighResExporter` lifetime and reuse it across iterate→accumulate
//! cycles.
//!
//! See `docs/projects/unified-render-pipeline.md`.

use egui_wgpu::wgpu::*;

/// Parameters uniform consumed by `accumulate_samples.wgsl`. Layout
/// must match the WGSL `AccumulateParams` struct exactly — ordered for
/// std140 with explicit padding to round to 32 bytes (a multiple of 16,
/// the std140 minimum stride for uniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AccumulateParams {
    /// X origin of the histogram region this dispatch is bound to.
    pub bound_x: u32,
    /// Y origin of the histogram region this dispatch is bound to.
    pub bound_y: u32,
    /// Width of the histogram region (always equals full image width
    /// today — tiles are horizontal slices, see `pick_strategy`).
    pub bound_width: u32,
    /// Height of the histogram region (= tile_height for ParallelTiles
    /// / SerialTiles, = full height for Direct).
    pub bound_height: u32,
    /// Number of valid samples in the sample buffer; readback this
    /// value from the atomic counter on host before dispatching so the
    /// accumulate shader can early-out for unwritten slots.
    pub sample_count: u32,
    /// Mirrors the iteration shader's `params.histogram_color_scale`
    /// — must be the same value the iterate dispatch wrote with so
    /// post-tonemap densities are proportional.
    pub color_scale: f32,
    pub _pad0: u32,
    pub _pad1: u32,
}

/// Threads per workgroup along x. Must match the `@workgroup_size` in
/// `accumulate_samples.wgsl`.
pub const ACCUMULATE_WORKGROUP_SIZE: u32 = 64;

/// Compute the dispatch group count for a given sample count.
pub fn accumulate_dispatch_groups(sample_count: u32) -> u32 {
    (sample_count + ACCUMULATE_WORKGROUP_SIZE - 1) / ACCUMULATE_WORKGROUP_SIZE
}

/// Bind-group layout for the accumulate pipeline. Three bindings:
///   - 0: sample stream (storage, read)
///   - 1: AccumulateParams (uniform)
///   - 2: histogram region (storage, read_write atomic u32)
pub fn create_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Accumulate Bind Group Layout"),
        entries: &[
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
        ],
    })
}

/// Build the accumulate compute pipeline. Shader source is embedded at
/// compile time via `include_str!` so deployments don't need the
/// shaders/ directory at runtime.
pub fn create_pipeline(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
) -> ComputePipeline {
    let shader_source = include_str!("../../shaders/core/accumulate_samples.wgsl");
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Accumulate Samples Shader"),
        source: ShaderSource::Wgsl(shader_source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Accumulate Pipeline Layout"),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("Accumulate Samples Pipeline"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("accumulate_main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulate_params_size_matches_wgsl() {
        // WGSL struct AccumulateParams: 4 u32 + 1 u32 + 1 f32 + 2 u32 padding = 32 bytes.
        // std140 uniform layout pads to a multiple of 16; 32 satisfies.
        assert_eq!(std::mem::size_of::<AccumulateParams>(), 32);
    }

    #[test]
    fn dispatch_group_arithmetic() {
        assert_eq!(accumulate_dispatch_groups(0), 0);
        assert_eq!(accumulate_dispatch_groups(1), 1);
        assert_eq!(accumulate_dispatch_groups(64), 1);
        assert_eq!(accumulate_dispatch_groups(65), 2);
        assert_eq!(accumulate_dispatch_groups(128), 2);
        assert_eq!(accumulate_dispatch_groups(1_000_000), 15625);
    }
}
