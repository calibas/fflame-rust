//! The planner's arithmetic on the GPU.
//!
//! `docs/projects/gpu-cylinder-planning.md`. The inverse-walk planner
//! (`backward.rs`) spends three quarters of its time on one operation:
//! apply a chosen word to chosen sample points and ask which land in the
//! view. This does that operation in batches on the GPU, with the
//! flame's own WGSL -- the kernel is the flame's definitions plus
//! `shaders/core/plan_eval.wgsl` (`ShaderBuilder::build_plan_eval`), so a
//! word is applied exactly as the render's replay arm applies it.
//!
//! The planner keeps every decision; this only answers questions. A
//! batch is a list of [`EvalJob`]s -- a word and the sample indices to
//! apply it to -- and the answer is one byte per point, in job order.

use crate::scene::backward::{Backward, Evaluate};
use crate::scene::cylinder::View;
use crate::scene::transforms::Flame;
use wgpu::util::DeviceExt;

pub use crate::scene::backward::EvalJob;

/// Mirrors `PlanView` in `plan_eval.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuPlanView {
    centre: [f32; 2],
    radius: f32,
    entries: u32,
    jobs: u32,
    row: u32,
    words_base: u32,
    idx_base: u32,
    mode: u32,
    _pad: [u32; 3],
}

/// What a batch writes per entry. Mirrors `PlanView::mode`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// One u32: 1 where the point lands in the disc.
    Disc = 0,
    /// Two u32s: the end point's f32 bits.
    Endpoint = 1,
}

/// The per-batch buffers, grown to fit and reused.
struct Batch {
    data: wgpu::Buffer,
    out: wgpu::Buffer,
    stage: wgpu::Buffer,
    view: wgpu::Buffer,
    group1: wgpu::BindGroup,
    /// Capacities, in u32s.
    cap_data: usize,
    cap_out: usize,
}

/// Where the last batch spent its time, in milliseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunTimes {
    /// Packing the jobs and writing them to the GPU.
    pub pack: f64,
    /// Submit to the answer being mapped: the GPU's work and the copy.
    pub wait: f64,
    /// Reading the mapped answer out.
    pub read: f64,
}

/// The planner's GPU evaluator for one flame and one sample.
pub struct PlanGpu {
    /// Where the last batch spent its time.
    pub last: RunTimes,
    /// Every batch's times added up, and how many batches, since the
    /// caller last reset them.
    pub totals: RunTimes,
    pub batches: usize,
    /// How many sample points were uploaded: the walk's indices must be
    /// into the same sample.
    points_len: usize,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout1: wgpu::BindGroupLayout,
    group0: wgpu::BindGroup,
    points: wgpu::Buffer,
    batch: Option<Batch>,
    /// The flame's group-0 buffers, held so the bind group stays valid.
    _flame_buffers: Vec<wgpu::Buffer>,
}

/// The most workgroups in one dispatch dimension, WebGPU's default.
const MAX_GROUPS: u32 = 65535;

impl PlanGpu {
    /// Build the kernel for `flame` and upload `sample` (in f32).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, flame: &Flame, sample: &[[f64; 2]]) -> Self {
        use crate::gpu::buffers as gb;
        let builder = crate::shader_builder_v2::ShaderBuilder::new(crate::variations::global_registry().clone());
        let src = builder.build_plan_eval(flame);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Plan Eval"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        // Group 0 as the simulation's layer map binds the flame.
        let layout0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Plan Eval Flame Layout"),
            entries: &[storage(0, true), uniform(1), storage(5, true), storage(10, true), storage(12, true)],
        });
        let layout1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Plan Eval Batch Layout"),
            entries: &[storage(0, true), storage(1, true), storage(2, false), uniform(3)],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Plan Eval"),
            bind_group_layouts: &[Some(&layout0), Some(&layout1)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Plan Eval"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("plan_eval"),
            compilation_options: Default::default(),
            cache: None,
        });

        let n = flame.transforms.len().max(1);
        let transforms = gb::pack_gpu_transforms(flame, crate::scene::transforms::RenderMode::TwoD);
        let vparams = gb::pack_gpu_variation_params(flame);
        let init = |label: &str, bytes: &[u8], usage: wgpu::BufferUsages| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: bytes, usage })
        };
        let st = wgpu::BufferUsages::STORAGE;
        let t_buf = init("Plan Eval Transforms", bytemuck::cast_slice(&transforms[..n.min(transforms.len())]), st);
        let v_buf = init("Plan Eval Variation Params", bytemuck::cast_slice(&vparams[..n.min(vparams.len())]), st);
        let p_buf = init(
            "Plan Eval Params",
            bytemuck::bytes_of(&<gb::GpuParams as bytemuck::Zeroable>::zeroed()),
            wgpu::BufferUsages::UNIFORM,
        );
        let attachments = vec![<gb::GpuAttachmentList as bytemuck::Zeroable>::zeroed(); gb::MAX_TRANSFORMS];
        let a_buf = init("Plan Eval Attachments", bytemuck::cast_slice(&attachments), st);
        let metas = gb::build_subflame_metas(&[]).unwrap_or_else(|_| {
            [<gb::SubflameMeta as bytemuck::Zeroable>::zeroed(); gb::MAX_SUBFLAMES]
        });
        let m_buf = init("Plan Eval Subflame Meta", bytemuck::cast_slice(&metas), st);
        let group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Plan Eval Flame"),
            layout: &layout0,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: t_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: p_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: v_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 10, resource: a_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 12, resource: m_buf.as_entire_binding() },
            ],
        });

        // **The derived parameter slots.** A variation with a `wgsl_init`
        // (julian's `cpower`, among many) has slots the render fills on
        // the GPU with an init pass before its first frame. Without it
        // they read zero -- measured, every word sent every point to
        // radius one and the kernel reported no hits at all. The same
        // pass the render and the exporter run, once, here.
        let active: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
        if let Some(init_src) = builder.build_init_shader(flame, &active) {
            let pairs = init_src
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    t.starts_with("case ") && t.contains("u: {")
                })
                .count() as u32;
            let init_layout = crate::shader_cache::ShaderCache::create_init_bind_group_layout(device);
            let init_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Plan Eval Init"),
                source: wgpu::ShaderSource::Wgsl(init_src.into()),
            });
            let init_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Plan Eval Init"),
                bind_group_layouts: &[Some(&init_layout)],
                immediate_size: 0,
            });
            let init_pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Plan Eval Init"),
                layout: Some(&init_pl),
                module: &init_module,
                entry_point: Some("init_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            let init_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Plan Eval Init"),
                layout: &init_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: v_buf.as_entire_binding() }],
            });
            if pairs > 0 {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Plan Eval Init") });
                {
                    let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Plan Eval Init"), timestamp_writes: None });
                    pass.set_pipeline(&init_pipe);
                    pass.set_bind_group(0, &init_group, &[]);
                    pass.dispatch_workgroups(pairs.div_ceil(64), 1, 1);
                }
                queue.submit(std::iter::once(enc.finish()));
            }
        }

        let pts: Vec<[f32; 2]> = sample.iter().map(|p| [p[0] as f32, p[1] as f32]).collect();
        let points = init("Plan Eval Points", bytemuck::cast_slice(&pts), st);

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout1,
            group0,
            points,
            batch: None,
            last: RunTimes::default(),
            totals: RunTimes::default(),
            batches: 0,
            points_len: sample.len(),
            _flame_buffers: vec![t_buf, v_buf, p_buf, a_buf, m_buf],
        }
    }

    /// Make sure the batch buffers hold `data` and `out` u32s.
    fn ensure_batch(&mut self, data: usize, out: usize) {
        if self.batch.as_ref().is_some_and(|b| b.cap_data >= data && b.cap_out >= out) {
            return;
        }
        let cap_data = data.next_power_of_two().max(1024);
        let cap_out = out.next_power_of_two().max(1024);
        let buf = |label: &str, words: usize, usage: wgpu::BufferUsages| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (words * 4) as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        use wgpu::BufferUsages as U;
        let data_buf = buf("Plan Eval Data", cap_data, U::STORAGE | U::COPY_DST);
        let out_buf = buf("Plan Eval Out", cap_out, U::STORAGE | U::COPY_SRC);
        let stage = buf("Plan Eval Stage", cap_out, U::MAP_READ | U::COPY_DST);
        let view = buf("Plan Eval View", std::mem::size_of::<GpuPlanView>() / 4, U::UNIFORM | U::COPY_DST);
        let group1 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Plan Eval Batch"),
            layout: &self.layout1,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.points.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: data_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: out_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: view.as_entire_binding() },
            ],
        });
        self.batch = Some(Batch { data: data_buf, out: out_buf, stage, view, group1, cap_data, cap_out });
    }

    /// Apply each job's word to each of its points; one byte per point, in
    /// job order, 1 where the result lands in `view`. Blocks until the
    /// answer is back -- the planner calls it from its own thread.
    pub fn evaluate(&mut self, view: View, jobs: &[EvalJob]) -> Vec<u8> {
        self.run(view, jobs, Mode::Disc, |w| w.iter().map(|w| (*w != 0) as u8).collect())
    }

    /// Apply each job's word to each of its points; where each lands, in
    /// job order, or `None` where the word sent it to a bad value or hid
    /// it. In f32, as the render has it.
    pub fn endpoints(&mut self, jobs: &[EvalJob]) -> Vec<Option<[f32; 2]>> {
        let view = View { centre: [0.0, 0.0], radius: 0.0 };
        self.run(view, jobs, Mode::Endpoint, |w| {
            w.chunks_exact(2)
                .map(|c| {
                    let (x, y) = (f32::from_bits(c[0]), f32::from_bits(c[1]));
                    (c[0] != 0x7fc0_0000 && x.is_finite() && y.is_finite()).then_some([x, y])
                })
                .collect()
        })
    }

    /// One batch: pack, dispatch, and hand the raw output words to `read`
    /// while they are still mapped.
    fn run<T>(&mut self, view: View, jobs: &[EvalJob], mode: Mode, read: impl FnOnce(&[u32]) -> Vec<T>) -> Vec<T> {
        let entries: usize = jobs.iter().map(|j| j.points.len()).sum();
        if entries == 0 {
            return Vec::new();
        }
        let per = if mode == Mode::Endpoint { 2 } else { 1 };
        let t_pack = web_time::Instant::now();
        // Jobs with no points are left out: the kernel finds a thread's
        // job by the first entry, and an empty run would shadow the next.
        let live: Vec<&EvalJob> = jobs.iter().filter(|j| !j.points.is_empty()).collect();
        let words_len: usize = live.iter().map(|j| j.word.len()).sum();
        let words_base = 4 * live.len();
        let idx_base = words_base + words_len;
        let mut data: Vec<u32> = Vec::with_capacity(idx_base + entries);
        let (mut word_at, mut entry_at) = (0u32, 0u32);
        for j in &live {
            data.extend_from_slice(&[word_at, j.word.len() as u32, entry_at, j.points.len() as u32]);
            word_at += j.word.len() as u32;
            entry_at += j.points.len() as u32;
        }
        for j in &live {
            data.extend_from_slice(j.word);
        }
        for j in &live {
            data.extend_from_slice(j.points);
        }

        self.ensure_batch(data.len(), per * entries);
        let groups = entries.div_ceil(64) as u32;
        let (gx, gy) = if groups <= MAX_GROUPS { (groups, 1) } else { (MAX_GROUPS, groups.div_ceil(MAX_GROUPS)) };
        let pv = GpuPlanView {
            centre: [view.centre[0] as f32, view.centre[1] as f32],
            radius: view.radius as f32,
            entries: entries as u32,
            jobs: live.len() as u32,
            row: gx * 64,
            words_base: words_base as u32,
            idx_base: idx_base as u32,
            mode: mode as u32,
            _pad: [0; 3],
        };
        let b = self.batch.as_ref().expect("ensured");
        self.queue.write_buffer(&b.data, 0, bytemuck::cast_slice(&data));
        self.queue.write_buffer(&b.view, 0, bytemuck::bytes_of(&pv));
        let pack = t_pack.elapsed().as_secs_f64() * 1e3;
        let t_wait = web_time::Instant::now();
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Plan Eval") });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Plan Eval"), timestamp_writes: None });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group0, &[]);
            pass.set_bind_group(1, &b.group1, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        let bytes = (per * entries * 4) as u64;
        enc.copy_buffer_to_buffer(&b.out, 0, &b.stage, 0, bytes);
        self.queue.submit(std::iter::once(enc.finish()));
        let slice = b.stage.slice(..bytes);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let ok = rx.recv().map(|r| r.is_ok()).unwrap_or(false);
        let wait = t_wait.elapsed().as_secs_f64() * 1e3;
        let t_read = web_time::Instant::now();
        let out = if ok {
            let view = slice.get_mapped_range();
            read(bytemuck::cast_slice::<u8, u32>(&view))
        } else {
            read(&vec![0; per * entries])
        };
        if ok {
            b.stage.unmap();
        }
        self.last = RunTimes { pack, wait, read: t_read.elapsed().as_secs_f64() * 1e3 };
        self.totals.pack += self.last.pack;
        self.totals.wait += self.last.wait;
        self.totals.read += self.last.read;
        self.batches += 1;
        out
    }
}

impl Evaluate for PlanGpu {
    fn lands(&mut self, b: &Backward, view: View, jobs: &[EvalJob]) -> Vec<u8> {
        assert_eq!(b.sample().len(), self.points_len, "a plan's GPU evaluator is for another sample");
        self.evaluate(view, jobs)
    }

    fn speculative(&self) -> bool {
        true
    }
}

/// **The app's GPU planner**: a device, and the evaluator for the flame
/// planned last -- the kernel is compiled per flame (~12 ms), so a flame
/// panned and zoomed keeps its own.
pub struct GpuPlanner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    current: Option<(u64, PlanGpu)>,
    /// A flame whose kernel failed to build plans on the CPU, and is not
    /// retried until another flame has been planned.
    failed: Option<u64>,
}

impl GpuPlanner {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self { device: device.clone(), queue: queue.clone(), current: None, failed: None }
    }

    /// The evaluator for `flame`, whose walk is `b`: built on first use,
    /// `None` if its kernel does not build here.
    pub fn for_flame(&mut self, flame: &Flame, b: &Backward) -> Option<&mut PlanGpu> {
        let key = Backward::flame_key(flame);
        if self.failed == Some(key) {
            return None;
        }
        if self.current.as_ref().is_some_and(|(k, g)| *k == key && g.points_len == b.sample().len()) {
            return self.current.as_mut().map(|(_, g)| g);
        }
        self.current = None;
        // A kernel that fails validation is a CPU plan, not a panic in
        // the planner thread.
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let gpu = PlanGpu::new(&self.device, &self.queue, flame, b.sample());
        if let Some(err) = pollster::block_on(scope.pop()) {
            log::warn!("the GPU planner's kernel did not build for this flame; planning on the CPU: {err}");
            self.failed = Some(key);
            return None;
        }
        self.failed = None;
        self.current = Some((key, gpu));
        self.current.as_mut().map(|(_, g)| g)
    }
}
