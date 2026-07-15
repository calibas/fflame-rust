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
/// std140 with explicit padding to round to 48 bytes (a multiple of 16,
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
    /// Solid rendering occlusion strength (0 = off). When active the
    /// bound histogram region carries one extra u32 per pixel at offset
    /// bound_width*bound_height*4 (the nearest-depth region) and the
    /// scatter gates each sample against it.
    pub solid_strength: f32,
    /// Solid rendering: world-space thickness of the accepted depth shell.
    pub surface_thickness: f32,
    /// 1 = depth-priming dispatch (record depth only, plot nothing).
    pub depth_prime: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    // ── Light-space shadow maps (Stage 2, export path) ──
    /// Full-image dimensions (sample coords are full-image pixels; the
    /// world reconstruction needs the whole view transform).
    pub full_width: u32,
    pub full_height: u32,
    /// 0 disables the shadow splat entirely.
    pub shadow_count: u32,
    pub _pad3: u32,
    pub zoom: f32,
    pub rotation: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub persp: f32,
    pub _pad4: f32,
    pub _pad5: f32,
    pub _pad6: f32,
    /// Effective world→camera rotation rows + camera position
    /// (shade_pass::effective_camera_rows).
    pub cam_row0: [f32; 4],
    pub cam_row1: [f32; 4],
    pub cam_row2: [f32; 4],
    pub cam_pos: [f32; 4],
    /// xyz = map center, w = bounding radius.
    pub shadow_fit: [f32; 4],
    /// xyz = world direction TO each light, w = enabled.
    pub shadow_dirs: [[f32; 4]; 4],
}

/// Threads per workgroup along x. Must match the `@workgroup_size` in
/// `accumulate_samples.wgsl`.
pub const ACCUMULATE_WORKGROUP_SIZE: u32 = 64;

/// Compute the dispatch group count for a given sample count.
pub fn accumulate_dispatch_groups(sample_count: u32) -> u32 {
    (sample_count + ACCUMULATE_WORKGROUP_SIZE - 1) / ACCUMULATE_WORKGROUP_SIZE
}

/// Bind-group layout for the accumulate pipeline. Four bindings:
///   - 0: sample stream (storage, read)
///   - 1: AccumulateParams (uniform)
///   - 2: histogram region (storage, read_write atomic u32)
///   - 3: light-space shadow maps (storage, read_write atomic u32;
///        16-byte dummy when shadow_count == 0)
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
            BindGroupLayoutEntry {
                binding: 3,
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
        // 48 bytes of original fields + 48 bytes of shadow-map scalars
        // + 5 vec4 (camera rows/pos + fit) + 4 vec4 light dirs
        // = 48 + 48 + 80 + 64 = 240 bytes (multiple of 16 for std140).
        assert_eq!(std::mem::size_of::<AccumulateParams>(), 240);
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
