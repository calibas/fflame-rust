//! Fly mode's clearance (docs/archive/projects/3d-look-and-tools.md, P6): how far
//! the eye is from a solid's surface -- the solid's own distance estimate
//! there, `ifs_distance_at` at the eye, for every solid kind and a Rhai
//! one alike, at the render's precision -- measured by one GPU thread and
//! read back without waiting, as the terrain's nearest-ground probe is
//! (`TerrainRenderer::probe_nearest`). Fly mode steps inside it and, as an
//! option, sizes the step and the zoom by it (`camera::fly::FlyLimits`).
//!
//! The probe is the walk's own shader up to its entry point -- the
//! distance function, its parameters, the maps and the lens -- with an
//! entry of one thread writing one float at binding 7, which the walk
//! leaves free. Its pipeline layout holds only what that entry reads: the
//! escape uniform and the output (group 0), the IFS rows (1), the lens's
//! maps (2).

use crate::camera::fly::Clearance;
use crate::config::escape::EscapeConfig;
use egui_wgpu::wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType,
    Buffer, BufferBindingType, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
    ComputePipelineDescriptor, Device, MapMode, PipelineLayoutDescriptor, PollType, Queue, ShaderModuleDescriptor,
    ShaderSource, ShaderStages,
};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use super::{assembler, EscapeParamsGpu, EscapeRenderer};

/// The probe's entry, after the walk's text.
const PROBE_ENTRY: &str = r#"
// Fly mode's clearance (3d-look-and-tools P6): the solid's distance at
// the eye -- its offset from the target, as every position here is --
// asked at a ten-thousandth of the eye's distance.
@group(0) @binding(7) var<storage, read_write> probe_out: array<f32>;

@compute @workgroup_size(1, 1, 1)
fn probe_main() {
    let eye = params.fdata[2].xyz;
    probe_out[0] = ifs_distance_at(eye, max(length(eye) * 1.0e-4, 1.0e-30));
}
"#;

/// The probe's buffers and its last measure.
pub(super) struct ClearanceProbe {
    layout: BindGroupLayout,
    params: Buffer,
    out: Buffer,
    stage: Buffer,
    /// A probe in flight: 0 waiting, 1 landed, 2 failed.
    landed: Option<Arc<AtomicU8>>,
    /// The eye (the world's) the probe in flight measures from.
    pending_eye: [f64; 3],
    last: Option<Clearance>,
}

impl ClearanceProbe {
    fn new(device: &Device) -> Self {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Escape Clearance Probe"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let buffer = |label: &str, size: u64, usage: BufferUsages| device.create_buffer(&BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        ClearanceProbe {
            layout,
            params: buffer("Escape Clearance Params", std::mem::size_of::<EscapeParamsGpu>() as u64, BufferUsages::UNIFORM | BufferUsages::COPY_DST),
            out: buffer("Escape Clearance", 16, BufferUsages::STORAGE | BufferUsages::COPY_SRC),
            stage: buffer("Escape Clearance Readback", 16, BufferUsages::MAP_READ | BufferUsages::COPY_DST),
            landed: None,
            pending_eye: [0.0; 3],
            last: None,
        }
    }

    pub(super) fn destroy(&self) {
        self.params.destroy();
        self.out.destroy();
        self.stage.destroy();
    }
}

impl EscapeRenderer {
    /// Measure the solid's distance at the eye `escape`'s camera stands
    /// at, without waiting: collected by [`Self::poll_clearance`]. Nothing
    /// while a probe is in flight, or when the config draws no solid.
    /// Submits its own work.
    pub fn probe_clearance(&mut self, device: &Device, queue: &Queue, escape: &EscapeConfig) {
        if self.clearance.as_ref().is_some_and(|p| p.landed.is_some()) {
            return;
        }
        let Some(def) = crate::escape::ifs::get_ifs(&escape.formula).filter(|d| d.solid) else {
            return;
        };
        let Some(eye) = self.ifs.as_ref().and_then(|p| p.solid.as_ref()).map(|(ifs3, _)| crate::escape::ifs::solid_camera(escape, ifs3).eye) else {
            return;
        };
        if self.clearance.is_none() {
            self.clearance = Some(ClearanceProbe::new(device));
        }
        let (key, lens) = self.ensure_probe_pipeline(device, escape, def);
        // A lens the frame has not made yet: its group is not there to bind.
        if lens && self.lens_gpu.is_none() {
            return;
        }
        let params = self.params_for(escape);
        let ifs_group = self.ifs_bind_group(device);
        let probe = self.clearance.as_ref().expect("made above");
        queue.write_buffer(&probe.params, 0, bytemuck::bytes_of(&params));
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Clearance Probe"),
            layout: &probe.layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: probe.params.as_entire_binding() },
                BindGroupEntry { binding: 7, resource: probe.out.as_entire_binding() },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Escape Clearance Probe") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Escape Clearance Probe"), timestamp_writes: None });
            pass.set_pipeline(&self.pipelines[&key]);
            pass.set_bind_group(0, &group, &[]);
            pass.set_bind_group(1, &ifs_group, &[]);
            if let Some(l) = self.lens_gpu.as_ref() {
                pass.set_bind_group(2, l.bind_group(), &[]);
            }
            pass.dispatch_workgroups(1, 1, 1);
        }
        enc.copy_buffer_to_buffer(&probe.out, 0, &probe.stage, 0, 4);
        queue.submit(std::iter::once(enc.finish()));
        let landed = Arc::new(AtomicU8::new(0));
        let flag = landed.clone();
        probe.stage.slice(..4).map_async(MapMode::Read, move |r| {
            flag.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
        });
        let probe = self.clearance.as_mut().expect("made above");
        probe.landed = Some(landed);
        probe.pending_eye = eye;
    }

    /// Collect a probe that has landed ([`Self::probe_clearance`]),
    /// polling without waiting. A distance that is not a finite positive
    /// number leaves the last measure standing.
    pub fn poll_clearance(&mut self, device: &Device) {
        let Some(probe) = self.clearance.as_mut() else {
            return;
        };
        let Some(landed) = probe.landed.as_ref() else {
            return;
        };
        let _ = device.poll(PollType::Poll);
        let state = landed.load(Ordering::Acquire);
        if state == 0 {
            return;
        }
        if state == 1 {
            let bits = {
                let data = probe.stage.slice(..4).get_mapped_range();
                u32::from_le_bytes(data[0..4].try_into().expect("four bytes"))
            };
            probe.stage.unmap();
            let d = f32::from_bits(bits);
            if d.is_finite() && d >= 0.0 {
                probe.last = Some(Clearance { eye: probe.pending_eye, distance: d as f64 });
            }
        }
        probe.landed = None;
    }

    /// The solid's distance from the eye at the last probe that landed,
    /// and where that eye stood.
    pub fn clearance(&self) -> Option<Clearance> {
        self.clearance.as_ref().and_then(|p| p.last)
    }

    /// The probe's pipeline for this solid: the walk's text to its entry
    /// point and [`PROBE_ENTRY`], keyed as the walk's is by what changes
    /// the distance function -- the solid, the lens, the sums.
    /// With it, whether it binds the lens's maps.
    fn ensure_probe_pipeline(&mut self, device: &Device, escape: &EscapeConfig, def: &crate::escape::ifs::IfsDef) -> (String, bool) {
        let coloring = crate::escape::ifs::get_ifs_coloring(&escape.coloring, def);
        let beam = escape
            .formula_params
            .get("beam")
            .copied()
            .unwrap_or_else(|| def.parameters.iter().find(|p| p.name == "beam").map_or(1.0, |p| p.default))
            .clamp(1.0, assembler::IFS_MAX_BEAM as f32) as u32;
        let registry = crate::variations::global_registry();
        let lens_src = crate::escape::lens::lens_source(escape, &registry);
        let lens_id = crate::escape::lens::lens_key(escape, &registry);
        drop(registry);
        let sums = self.ifs.as_ref().is_some_and(|p| p.rows.iter().any(|r| r.kind == 7.0));
        let key = format!("ifs_probe|{}|{}|b{beam}|{lens_id}|{sums}", def.name, coloring.name);
        if !self.pipelines.contains_key(&key) {
            let walk = assembler::assemble_ifs_rig(def, coloring, beam, lens_src.as_deref(), false, sums, false);
            let cut = walk.find("@compute @workgroup_size(8, 8, 1)\nfn escape_main").expect("the solid template ends in the walk's entry point");
            let source = format!("{}\n{}", &walk[..cut], PROBE_ENTRY);
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&format!("Escape Shader {key}")),
                source: ShaderSource::Wgsl(source.into()),
            });
            let probe = self.clearance.as_ref().expect("made before the pipeline");
            let mut groups = vec![Some(&probe.layout), Some(&self.ifs_bind_group_layout)];
            if lens_src.is_some() {
                groups.push(Some(&self.lens_bind_group_layout));
            }
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Clearance Probe Layout"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(&format!("Escape Pipeline {key}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("probe_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline);
        }
        (key, lens_src.is_some())
    }
}
