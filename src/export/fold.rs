//! **Whole-export sums folded into f64 before a u32 wraps**
//! (docs/archive/projects/exact-accumulator.md, part 2).
//!
//! `HighResExporter` keeps three sums for the whole export in u32: the
//! tile histogram (a pixel's density word is 100 x its hits, so it wraps
//! past ~43M hits -- every in-app export above 250M iterations takes this
//! path, as one tile), the analytic-blur splat buffer (a low-res cell
//! collects many pixels' hits) and the solid occlusion counters (16 a
//! subsampled sample, wrapping past ~2.7e11 samples). Every
//! `CHECK_EVERY` dispatches a small pass finds the largest density word
//! (`shaders/export_max_density.wgsl`) and reads it back with the
//! occlusion counters; when the next check could pass `limit`, the
//! exporter reads the histogram and the splat buffer into its f64 running
//! sums and clears them. The occlusion counters are folded at every
//! check. An export that never gets near the limit never folds.
//!
//! The running sums hold raw words (whole numbers, exact in f64 to 2^53)
//! and are divided by the colour scale once, at the end, as an unfolded
//! export divides its words: a folded export is bit-identical to one
//! that did not need to fold.

use wgpu::*;

/// Dispatches between checks.
pub const CHECK_EVERY: u64 = 8;

/// A density word past which a fold is due: half the u32 range, so a
/// check's projection has the other half to be wrong in.
pub const LIMIT: u32 = 1 << 31;

/// Workgroups the max pass dispatches; its loop strides over the rest.
const GROUPS: u32 = 256;

/// The max pass and the decision.
pub struct Fold {
    pipeline: ComputePipeline,
    layout: BindGroupLayout,
    /// The pass's atomic maximum, 4 bytes.
    result: Buffer,
    /// Where a check reads to: the maximum, then the two occlusion words.
    pub staging: Buffer,
    /// When to fold.
    pub watch: Watch,
}

/// The decision: the largest word each check saw, and the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watch {
    /// The largest word the last check saw (0 after a fold).
    last_max: u32,
    /// The fold limit: `LIMIT`, lowered by tests.
    pub limit: u32,
    /// Folds so far this export.
    pub folds: u32,
}

impl Default for Watch {
    fn default() -> Self {
        Self { last_max: 0, limit: LIMIT, folds: 0 }
    }
}

impl Watch {
    /// Whether to fold, given the largest word now: when it, plus twice
    /// what it grew since the last check, reaches the limit. Right after
    /// a fold the growth is the whole of it.
    pub fn due(&mut self, max: u32) -> bool {
        let growth = max.saturating_sub(self.last_max);
        let projected = u64::from(max) + 2 * u64::from(growth);
        if projected >= u64::from(self.limit) {
            self.last_max = 0;
            self.folds += 1;
            true
        } else {
            self.last_max = max;
            false
        }
    }

    /// A new export: nothing seen, nothing folded; the limit kept.
    pub fn reset(&mut self) {
        *self = Self { limit: self.limit, ..Self::default() };
    }
}

impl Fold {
    pub fn new(device: &Device) -> Self {
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Export Max Density"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/export_max_density.wgsl").into()),
        });
        let storage = |binding, read_only| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Export Max Density Layout"),
            entries: &[storage(0, true), storage(1, false)],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Export Max Density Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Export Max Density"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let result = device.create_buffer(&BufferDescriptor {
            label: Some("Export Max Density Result"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("Export Fold Check Staging"),
            size: 12,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self { pipeline, layout, result, staging, watch: Watch::default() }
    }

    /// Encode the max pass over `spans` (buffer, byte offset, byte size,
    /// each a whole number of RGBD pixels) and copy the maximum into
    /// `staging` at 0.
    pub fn encode_max(&self, device: &Device, encoder: &mut CommandEncoder, spans: &[(&Buffer, u64, u64)]) {
        encoder.clear_buffer(&self.result, 0, None);
        for &(buffer, offset, size) in spans {
            if size < 16 {
                continue;
            }
            let bind = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Export Max Density Span"),
                layout: &self.layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding { buffer, offset, size: std::num::NonZeroU64::new(size) }),
                    },
                    BindGroupEntry { binding: 1, resource: self.result.as_entire_binding() },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("Export Max Density"), timestamp_writes: None });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(GROUPS, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.result, 0, &self.staging, 0, 4);
    }
}

#[cfg(test)]
mod tests {
    use super::Watch;

    /// Folds before the projection reaches the limit, and counts from
    /// zero after a fold.
    #[test]
    fn a_fold_is_due_before_the_limit() {
        let mut w = Watch { limit: 1000, ..Watch::default() };
        assert!(!w.due(100), "100, grown 100: projects 300");
        assert!(!w.due(300), "300, grown 200: projects 700");
        assert!(!w.due(500), "500, grown 200: projects 900");
        assert!(w.due(700), "700, grown 200: projects 1100");
        assert_eq!(w.folds, 1);
        assert!(!w.due(200), "after the fold, 200 is all growth: projects 600");
        assert!(!w.due(400), "400, grown 200: projects 800");
        assert!(w.due(600), "600, grown 200: projects 1000, the limit itself");
        assert_eq!(w.folds, 2);
    }
}
