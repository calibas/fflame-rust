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
//!
//! **Phase 4: the gathers too.** With the walk's indexes uploaded
//! ([`PlanGpu::attach`]), a gather -- the candidates a child takes from an
//! index over its node's cells -- runs here as well
//! (`shaders/core/plan_gather.wgsl`), in the same submission as the
//! replays, and its candidates are checked where they were gathered: they
//! never cross to the CPU, only the ones that land come back.

use crate::scene::backward::{Backward, GatherJob, Gathered, IndexTables};
#[cfg(not(target_arch = "wasm32"))]
use crate::scene::backward::{gather_on_cpu, Evaluate};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
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

/// Mirrors `GatherView` in `plan_gather.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuGatherView {
    jobs: u32,
    pairs: u32,
    slots: u32,
    row_pairs: u32,
    row_slots: u32,
    row_jobs: u32,
    cells_base: u32,
    _pad: u32,
}

/// A buffer grown to fit and reused: a power of two of u32s, at least
/// 1024.
struct Grow {
    buf: Option<wgpu::Buffer>,
    cap: usize,
    label: &'static str,
    usage: wgpu::BufferUsages,
}

impl Grow {
    fn new(label: &'static str, usage: wgpu::BufferUsages) -> Self {
        Self { buf: None, cap: 0, label, usage }
    }

    /// A buffer holding at least `words` u32s.
    fn get(&mut self, device: &wgpu::Device, words: usize) -> wgpu::Buffer {
        if self.buf.is_none() || self.cap < words {
            self.cap = words.next_power_of_two().max(1024);
            self.buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: (self.cap * 4) as u64,
                usage: self.usage,
                mapped_at_creation: false,
            }));
        }
        self.buf.clone().expect("made")
    }

    /// [`Self::get`], taken out: the caller has it alone until it gives it
    /// back with [`Self::give_back`], and if it never does, the next use
    /// makes a new one. The staging buffer is lent this way -- see
    /// [`Readback`].
    fn lend(&mut self, device: &wgpu::Device, words: usize) -> wgpu::Buffer {
        let buf = self.get(device, words);
        self.buf = None;
        buf
    }

    fn give_back(&mut self, buf: wgpu::Buffer) {
        if self.buf.is_none() && buf.size() >= (self.cap * 4) as u64 {
            self.buf = Some(buf);
        }
    }
}

/// The walk's indexes on the GPU. See [`IndexTables`].
struct IndexBuffers {
    cells: wgpu::Buffer,
    idx: wgpu::Buffer,
    offsets: Vec<u32>,
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
    /// `plan_eval`, and `plan_eval_gathered` for the gathered candidates.
    pipeline: wgpu::ComputePipeline,
    gathered: wgpu::ComputePipeline,
    layout1: wgpu::BindGroupLayout,
    group0: wgpu::BindGroup,
    points: wgpu::Buffer,
    /// The gather's passes and their bind group layout.
    g_layout: wgpu::BindGroupLayout,
    g_ranges: wgpu::ComputePipeline,
    g_scan: wgpu::ComputePipeline,
    g_fill: wgpu::ComputePipeline,
    /// The walk's indexes, once [`PlanGpu::attach`]ed.
    index: Option<IndexBuffers>,
    data: Grow,
    out: Grow,
    stage: Grow,
    gdata: Grow,
    gpairs: Grow,
    gcands: Grow,
    edata: Grow,
    eout: Grow,
    view: wgpu::Buffer,
    gview: wgpu::Buffer,
    eview: wgpu::Buffer,
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
            entries: &[storage(0, true), storage(1, true), storage(2, false), uniform(3), storage(4, true)],
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

        // **The gather** (phase 4): its own module -- it reads the walk's
        // indexes, not the flame -- and its own bind group.
        let gather_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Plan Gather"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/core/plan_gather.wgsl").into()),
        });
        let g_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Plan Gather Layout"),
            entries: &[storage(0, true), storage(1, true), storage(2, true), storage(3, false), storage(4, false), uniform(5)],
        });
        let g_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Plan Gather"),
            bind_group_layouts: &[Some(&g_layout)],
            immediate_size: 0,
        });
        let g_pipe = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&g_pl),
                module: &gather_module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let (g_ranges, g_scan, g_fill) = (g_pipe("gather_ranges"), g_pipe("gather_scan"), g_pipe("gather_fill"));
        let gathered = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Plan Eval Gathered"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("plan_eval_gathered"),
            compilation_options: Default::default(),
            cache: None,
        });
        use wgpu::BufferUsages as U;
        let fixed = |label: &str, bytes: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes as u64,
                usage: U::UNIFORM | U::COPY_DST,
                mapped_at_creation: false,
            })
        };

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            gathered,
            layout1,
            group0,
            points,
            g_layout,
            g_ranges,
            g_scan,
            g_fill,
            index: None,
            data: Grow::new("Plan Eval Data", U::STORAGE | U::COPY_DST),
            out: Grow::new("Plan Eval Out", U::STORAGE | U::COPY_SRC),
            stage: Grow::new("Plan Eval Stage", U::MAP_READ | U::COPY_DST),
            gdata: Grow::new("Plan Gather Data", U::STORAGE | U::COPY_DST),
            gpairs: Grow::new("Plan Gather Pairs", U::STORAGE),
            gcands: Grow::new("Plan Gather Candidates", U::STORAGE | U::COPY_SRC),
            edata: Grow::new("Plan Eval Gathered Data", U::STORAGE | U::COPY_DST),
            eout: Grow::new("Plan Eval Gathered Out", U::STORAGE | U::COPY_SRC),
            view: fixed("Plan Eval View", std::mem::size_of::<GpuPlanView>()),
            gview: fixed("Plan Gather View", std::mem::size_of::<GpuGatherView>()),
            eview: fixed("Plan Eval Gathered View", std::mem::size_of::<GpuPlanView>()),
            last: RunTimes::default(),
            totals: RunTimes::default(),
            batches: 0,
            points_len: sample.len(),
            _flame_buffers: vec![t_buf, v_buf, p_buf, a_buf, m_buf],
        }
    }

    /// **Upload the walk's indexes**, so gathers run here. Until this is
    /// called, `gather_lands` gathers on the CPU.
    pub fn attach(&mut self, b: &Backward) {
        crate::scene::backward::drive(self.attach_sliced(b, &crate::scene::backward::Slicer::never()));
    }

    /// [`Self::attach`], yielding at `slicer`'s ticks: the tables are
    /// tens of megabytes to build and upload, more than a web frame.
    pub async fn attach_sliced(&mut self, b: &Backward, slicer: &crate::scene::backward::Slicer) {
        let t: IndexTables = b.index_tables_sliced(slicer).await;
        slicer.tick().await;
        // Written a few megabytes at a time: the cells alone are ~20 MB,
        // and one copy of them was a web frame.
        const PIECE: usize = 4 << 20;
        let make = |label: &'static str, bytes: usize| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.max(4) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let cell_bytes: &[u8] = bytemuck::cast_slice(&t.cells);
        let idx_bytes: &[u8] = bytemuck::cast_slice(&t.idx);
        let cells = make("Plan Gather Index Cells", cell_bytes.len());
        let idx = make("Plan Gather Index Entries", idx_bytes.len());
        for (buf, bytes) in [(&cells, cell_bytes), (&idx, idx_bytes)] {
            for (k, piece) in bytes.chunks(PIECE).enumerate() {
                self.queue.write_buffer(buf, (k * PIECE) as u64, piece);
                slicer.tick().await;
            }
        }
        self.index = Some(IndexBuffers { cells, idx, offsets: t.offsets });
    }

    /// Whether gathers run here. See [`Self::attach`].
    pub fn gathers_here(&self) -> bool {
        self.index.is_some()
    }

    /// Apply each job's word to each of its points; one byte per point, in
    /// job order, 1 where the result lands in `view`. Blocks until the
    /// answer is back -- the planner calls it from its own thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn evaluate(&mut self, view: View, jobs: &[EvalJob]) -> Vec<u8> {
        self.run(view, jobs, Mode::Disc, |w| w.iter().map(|w| (*w != 0) as u8).collect())
    }

    /// Apply each job's word to each of its points; where each lands, in
    /// job order, or `None` where the word sent it to a bad value or hid
    /// it. In f32, as the render has it.
    #[cfg(not(target_arch = "wasm32"))]
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

    /// Pack plain jobs: the job table, the words, the point indices. Jobs
    /// with no points are left out: the kernel finds a thread's job by the
    /// first entry, and an empty run would shadow the next.
    fn pack_plain(jobs: &[EvalJob]) -> (Vec<u32>, u32, u32, u32) {
        let live: Vec<&EvalJob> = jobs.iter().filter(|j| !j.points.is_empty()).collect();
        let entries: usize = live.iter().map(|j| j.points.len()).sum();
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
        (data, live.len() as u32, words_base as u32, idx_base as u32)
    }

    /// The eval pipelines' group 1: points, a job table, an output, a view,
    /// and the gathered candidates (read by the gathered pass only).
    fn group1(&mut self, data: &wgpu::Buffer, out: &wgpu::Buffer, view: &wgpu::Buffer) -> wgpu::BindGroup {
        let gcands = self.gcands.get(&self.device, 2);
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Plan Eval Batch"),
            layout: &self.layout1,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.points.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: data.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: out.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: view.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: gcands.as_entire_binding() },
            ],
        })
    }

    /// One batch: pack, dispatch, and hand the raw output words to `read`
    /// while they are still mapped. Blocks.
    #[cfg(not(target_arch = "wasm32"))]
    fn run<T>(&mut self, view: View, jobs: &[EvalJob], mode: Mode, read: impl FnOnce(&[u32]) -> Vec<T>) -> Vec<T> {
        let Some((r, n)) = self.run_submit(view, jobs, mode) else { return Vec::new() };
        self.wait(&r);
        self.take(r, |w| match w {
            Some(w) => read(w),
            None => read(&vec![0; n]),
        })
    }

    /// Pack and submit one plain batch; `None` if it has no points. The
    /// readback and the number of answer words.
    fn run_submit(&mut self, view: View, jobs: &[EvalJob], mode: Mode) -> Option<(Readback, usize)> {
        let entries: usize = jobs.iter().map(|j| j.points.len()).sum();
        if entries == 0 {
            return None;
        }
        let per = if mode == Mode::Endpoint { 2 } else { 1 };
        let t_pack = web_time::Instant::now();
        let (data, n_jobs, words_base, idx_base) = Self::pack_plain(jobs);
        let (gx, gy, row) = grid(entries, 64);
        let pv = GpuPlanView {
            centre: [view.centre[0] as f32, view.centre[1] as f32],
            radius: view.radius as f32,
            entries: entries as u32,
            jobs: n_jobs,
            row,
            words_base,
            idx_base,
            mode: mode as u32,
            _pad: [0; 3],
        };
        let data_buf = self.data.get(&self.device, data.len());
        let out_buf = self.out.get(&self.device, per * entries);
        let view_buf = self.view.clone();
        self.queue.write_buffer(&data_buf, 0, bytemuck::cast_slice(&data));
        self.queue.write_buffer(&view_buf, 0, bytemuck::bytes_of(&pv));
        let group1 = self.group1(&data_buf, &out_buf, &view_buf);
        let pack = t_pack.elapsed().as_secs_f64() * 1e3;
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Plan Eval") });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Plan Eval"), timestamp_writes: None });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group0, &[]);
            pass.set_bind_group(1, &group1, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        let n = per * entries;
        Some((self.submit(enc, &[(&out_buf, n)], pack), n))
    }

    /// Copy `copies` (buffer, u32 count) into the staging buffer back to
    /// back, submit, and ask for the staging buffer to be mapped. The
    /// answer is in when [`Readback::arrived`]; the desktop waits for it
    /// with [`Self::wait`], the web's walk awaits it.
    fn submit(&mut self, mut enc: wgpu::CommandEncoder, copies: &[(&wgpu::Buffer, usize)], pack: f64) -> Readback {
        let total: usize = copies.iter().map(|c| c.1).sum();
        let stage = self.stage.lend(&self.device, total.max(1));
        let mut at = 0u64;
        for (buf, n) in copies {
            let bytes = (*n * 4) as u64;
            if bytes > 0 {
                enc.copy_buffer_to_buffer(buf, 0, &stage, at, bytes);
            }
            at += bytes;
        }
        let t_wait = web_time::Instant::now();
        let index = self.queue.submit(std::iter::once(enc.finish()));
        let state = Arc::new(AtomicU8::new(PENDING));
        let flag = state.clone();
        stage.slice(..(total.max(1) * 4) as u64).map_async(wgpu::MapMode::Read, move |r| {
            flag.store(if r.is_ok() { MAPPED } else { FAILED }, Ordering::Release);
        });
        Readback { stage: Some(stage), total, state, index, pack, t_wait }
    }

    /// Block until `r` is in. The desktop's; the web awaits instead.
    #[cfg(not(target_arch = "wasm32"))]
    fn wait(&self, r: &Readback) {
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: Some(r.index.clone()), timeout: None });
    }

    /// Hand the words of an arrived readback to `read` while they are
    /// mapped -- `None` if the map failed -- and account the time.
    fn take<T>(&mut self, mut r: Readback, read: impl FnOnce(Option<&[u32]>) -> T) -> T {
        let wait = r.t_wait.elapsed().as_secs_f64() * 1e3;
        let t_read = web_time::Instant::now();
        let stage = r.stage.take().expect("a readback is taken once");
        let out = if r.state.load(Ordering::Acquire) == MAPPED {
            let slice = stage.slice(..(r.total.max(1) * 4) as u64);
            let view = slice.get_mapped_range();
            let v = read(Some(&bytemuck::cast_slice::<u8, u32>(&view)[..r.total]));
            drop(view);
            stage.unmap();
            self.stage.give_back(stage);
            v
        } else {
            // A failed map is not given back: see `Readback`.
            read(None)
        };
        self.last = RunTimes { pack: r.pack, wait, read: t_read.elapsed().as_secs_f64() * 1e3 };
        self.totals.pack += self.last.pack;
        self.totals.wait += self.last.wait;
        self.totals.read += self.last.read;
        self.batches += 1;
        out
    }

    /// **Plain jobs and gathers in one submission** (phase 4): the plain
    /// jobs' pass, the three gather passes, and the check of the gathered
    /// candidates -- which never leave the GPU. Back come the plain
    /// answers, each gather's candidate count, and the candidates that
    /// landed.
    #[cfg(not(target_arch = "wasm32"))]
    fn fused(&mut self, view: View, jobs: &[EvalJob], gathers: &[GatherJob], b: &Backward) -> (Vec<u8>, Vec<Gathered>) {
        let (r, parse) = self.fused_submit(view, jobs, gathers, b);
        self.wait(&r);
        self.take(r, |w| parse.read(w))
    }

    /// Pack and submit a fused batch. See [`Self::fused`].
    fn fused_submit(&mut self, view: View, jobs: &[EvalJob], gathers: &[GatherJob], b: &Backward) -> (Readback, FusedParse) {
        let t_pack = web_time::Instant::now();
        let (ix_cells, ix_idx, offsets) = {
            let ix = self.index.as_ref().expect("attached");
            (ix.cells.clone(), ix.idx.clone(), ix.offsets.clone())
        };

        // The gather table, and each distinct cell list once: a node's
        // children all gather over the node's cells.
        let mut table: Vec<u32> = Vec::with_capacity(8 * gathers.len());
        let mut cells: Vec<u32> = Vec::new();
        let mut placed: std::collections::HashMap<(usize, usize), u32> = std::collections::HashMap::new();
        let (mut pairs, mut slots) = (0usize, 0usize);
        for g in gathers {
            let cell_off = *placed.entry((g.seen.as_ptr() as usize, g.seen.len())).or_insert_with(|| {
                let at = (cells.len() / 2) as u32;
                for &(x, y) in g.seen {
                    cells.push(x as u32);
                    cells.push(y as u32);
                }
                at
            });
            let k = b.index_slot(g.index);
            let (lo, hi) = (offsets[k], offsets[k + 1]);
            table.extend_from_slice(&[lo, hi, cell_off, g.seen.len() as u32, pairs as u32, g.cap as u32, slots as u32, 0]);
            pairs += g.seen.len();
            slots += g.cap;
        }
        let cells_base = table.len() as u32;
        table.extend_from_slice(&cells);
        let n_gathers = gathers.len();

        // The gathered check's table: word, and where the job's slots
        // start.
        let words_len: usize = gathers.iter().map(|g| g.word.len()).sum();
        let mut etable: Vec<u32> = Vec::with_capacity(4 * n_gathers + words_len);
        let (mut word_at, mut slot_at) = (0u32, 0u32);
        for g in gathers {
            etable.extend_from_slice(&[word_at, g.word.len() as u32, slot_at, 0]);
            word_at += g.word.len() as u32;
            slot_at += g.cap as u32;
        }
        let e_words_base = etable.len() as u32;
        for g in gathers {
            etable.extend_from_slice(g.word);
        }

        let (px, py, prow) = grid(pairs.max(1), 64);
        let (sx, sy, srow) = grid(slots.max(1), 64);
        let (jx, jy, jrow) = grid(n_gathers.max(1), 1);
        let gv = GpuGatherView {
            jobs: n_gathers as u32,
            pairs: pairs as u32,
            slots: slots as u32,
            row_pairs: prow,
            row_slots: srow,
            row_jobs: jrow,
            cells_base,
            _pad: 0,
        };
        let ev = GpuPlanView {
            centre: [view.centre[0] as f32, view.centre[1] as f32],
            radius: view.radius as f32,
            entries: slots as u32,
            jobs: n_gathers as u32,
            row: srow,
            words_base: e_words_base,
            idx_base: 0,
            mode: 0,
            _pad: [0; 3],
        };
        let gdata = self.gdata.get(&self.device, table.len());
        let gpairs = self.gpairs.get(&self.device, 3 * pairs.max(1));
        let gcands = self.gcands.get(&self.device, 2 * n_gathers + slots);
        let edata = self.edata.get(&self.device, etable.len());
        let eout = self.eout.get(&self.device, slots.max(1));
        self.queue.write_buffer(&gdata, 0, bytemuck::cast_slice(&table));
        self.queue.write_buffer(&self.gview, 0, bytemuck::bytes_of(&gv));
        self.queue.write_buffer(&edata, 0, bytemuck::cast_slice(&etable));
        self.queue.write_buffer(&self.eview, 0, bytemuck::bytes_of(&ev));
        let g_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Plan Gather"),
            layout: &self.g_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ix_cells.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: ix_idx.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: gdata.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: gpairs.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: gcands.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: self.gview.as_entire_binding() },
            ],
        });
        let eview = self.eview.clone();
        let e_group = self.group1(&edata, &eout, &eview);

        // The plain jobs, if any, in the same submission.
        let entries: usize = jobs.iter().map(|j| j.points.len()).sum();
        let plain = if entries > 0 {
            let (data, n_jobs, words_base, idx_base) = Self::pack_plain(jobs);
            let (gx, gy, row) = grid(entries, 64);
            let pv = GpuPlanView {
                centre: [view.centre[0] as f32, view.centre[1] as f32],
                radius: view.radius as f32,
                entries: entries as u32,
                jobs: n_jobs,
                row,
                words_base,
                idx_base,
                mode: Mode::Disc as u32,
                _pad: [0; 3],
            };
            let data_buf = self.data.get(&self.device, data.len());
            let out_buf = self.out.get(&self.device, entries);
            let view_buf = self.view.clone();
            self.queue.write_buffer(&data_buf, 0, bytemuck::cast_slice(&data));
            self.queue.write_buffer(&view_buf, 0, bytemuck::bytes_of(&pv));
            let group1 = self.group1(&data_buf, &out_buf, &view_buf);
            Some((group1, out_buf, gx, gy))
        } else {
            None
        };
        let pack = t_pack.elapsed().as_secs_f64() * 1e3;

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Plan Fused") });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Plan Fused"), timestamp_writes: None });
            if let Some((group1, _, gx, gy)) = &plain {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.group0, &[]);
                pass.set_bind_group(1, group1, &[]);
                pass.dispatch_workgroups(*gx, *gy, 1);
            }
            pass.set_bind_group(0, &g_group, &[]);
            if pairs > 0 {
                pass.set_pipeline(&self.g_ranges);
                pass.dispatch_workgroups(px, py, 1);
            }
            pass.set_pipeline(&self.g_scan);
            pass.dispatch_workgroups(jx, jy, 1);
            pass.set_pipeline(&self.g_fill);
            pass.dispatch_workgroups(sx, sy, 1);
            pass.set_pipeline(&self.gathered);
            pass.set_bind_group(0, &self.group0, &[]);
            pass.set_bind_group(1, &e_group, &[]);
            pass.dispatch_workgroups(sx, sy, 1);
        }
        let empty = self.out.get(&self.device, 1);
        let plain_out = plain.as_ref().map(|p| p.1.clone()).unwrap_or(empty);
        let copies = [(&plain_out, entries), (&gcands, 2 * n_gathers), (&eout, slots)];
        let parse = FusedParse { entries, caps: gathers.iter().map(|g| g.cap).collect() };
        (self.submit(enc, &copies, pack), parse)
    }
}

/// A submitted batch whose answer is coming: the staging buffer and how
/// many words of it are the answer, whether its map has come back, and
/// the times so far.
///
/// **The staging buffer is the readback's alone** ([`Grow::lend`]) until
/// [`PlanGpu::take`] reads it and gives it back. A readback can be
/// dropped instead: on the web a plan is a task, and a task dropped
/// because the view moved on drops whatever readback it was waiting for.
/// The buffer then goes with it, unmapped on the way -- which also aborts
/// a map still pending -- and the next batch makes a new one. Shared,
/// it was left mapped, and the next plan's first batch copied into a
/// mapped buffer and panicked in `map_async`. A map that failed is never
/// reused either: wgpu forgets a mapping only on `unmap`, and natively
/// unmapping a buffer whose map failed is itself an error.
struct Readback {
    stage: Option<wgpu::Buffer>,
    total: usize,
    state: Arc<AtomicU8>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    index: wgpu::SubmissionIndex,
    pack: f64,
    t_wait: web_time::Instant,
}

const PENDING: u8 = 0;
const MAPPED: u8 = 1;
const FAILED: u8 = 2;

impl Drop for Readback {
    fn drop(&mut self) {
        if let Some(stage) = self.stage.take() {
            if self.state.load(Ordering::Acquire) != FAILED {
                stage.unmap();
            }
        }
    }
}

impl Readback {
    /// The map has come back, mapped or failed. The web's walk awaits
    /// this: the browser runs the map's callback between frames. On the
    /// desktop each poll nudges the device, without blocking, so a test
    /// can run the web's path frame by frame.
    fn arrived(&self, device: &wgpu::Device) -> Arrived {
        let _ = device;
        Arrived {
            state: self.state.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            device: device.clone(),
        }
    }
}

/// See [`Readback::arrived`]. Polled once a frame by the driver; nothing
/// needs waking.
struct Arrived {
    state: Arc<AtomicU8>,
    #[cfg(not(target_arch = "wasm32"))]
    device: wgpu::Device,
}

impl std::future::Future for Arrived {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.device.poll(wgpu::PollType::Poll);
        if self.state.load(Ordering::Acquire) == PENDING {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    }
}

/// How to read a fused batch's words: the plain answers, then per gather
/// its count and step, then its `cap` slots.
struct FusedParse {
    entries: usize,
    caps: Vec<usize>,
}

impl FusedParse {
    fn read(&self, w: Option<&[u32]>) -> (Vec<u8>, Vec<Gathered>) {
        let (entries, n_gathers) = (self.entries, self.caps.len());
        let Some(w) = w else {
            return (vec![0; entries], vec![Gathered::default(); n_gathers]);
        };
        let answers: Vec<u8> = w[..entries].iter().map(|v| (*v != 0) as u8).collect();
        let header = &w[entries..entries + 2 * n_gathers];
        let out = &w[entries + 2 * n_gathers..];
        let mut at = 0usize;
        let gathered = self
            .caps
            .iter()
            .enumerate()
            .map(|(g, cap)| {
                let n = header[2 * g] as usize;
                let hits = out[at..at + n].iter().copied().filter(|v| *v != NONE).collect();
                at += cap;
                Gathered { cands: n, hits }
            })
            .collect();
        (answers, gathered)
    }
}

/// Workgroups for `threads` threads of `per` each, in two dimensions past
/// WebGPU's per-dimension limit, and the threads in one row.
fn grid(threads: usize, per: u32) -> (u32, u32, u32) {
    let groups = (threads as u32).div_ceil(per).max(1);
    let (gx, gy) = if groups <= MAX_GROUPS { (groups, 1) } else { (MAX_GROUPS, groups.div_ceil(MAX_GROUPS)) };
    (gx, gy, gx * per)
}

/// A slot or an answer that holds nothing.
const NONE: u32 = u32::MAX;

#[cfg(not(target_arch = "wasm32"))]
impl Evaluate for PlanGpu {
    fn lands(&mut self, b: &Backward, view: View, jobs: &[EvalJob]) -> Vec<u8> {
        assert_eq!(b.sample().len(), self.points_len, "a plan's GPU evaluator is for another sample");
        self.evaluate(view, jobs)
    }

    fn speculative(&self) -> bool {
        true
    }

    fn gather_lands(&mut self, b: &Backward, view: View, jobs: &[EvalJob], gathers: &[GatherJob]) -> (Vec<u8>, Vec<Gathered>) {
        if gathers.is_empty() || !self.gathers_here() {
            return gather_on_cpu(self, b, view, jobs, gathers);
        }
        assert_eq!(b.sample().len(), self.points_len, "a plan's GPU evaluator is for another sample");
        self.fused(view, jobs, gathers, b)
    }
}

/// **The GPU's answers, awaited**: a batch is submitted and the walk
/// yields until it has been mapped back, a frame or so later on the web.
/// The web's only evaluator; on the desktop the tests run the web's path
/// with it (the app's worker blocks instead, through `Evaluate`). Every
/// gather runs here -- `GpuPlanner` always attaches the indexes.
impl crate::scene::backward::AskEval for PlanGpu {
    fn speculative(&self) -> bool {
        true
    }

    fn lands<'a>(
        &'a mut self,
        _b: &'a Backward,
        view: View,
        jobs: &'a [EvalJob<'a>],
    ) -> crate::scene::backward::Ask<'a, Vec<u8>> {
        Box::pin(async move {
            let Some((r, n)) = self.run_submit(view, jobs, Mode::Disc) else { return Vec::new() };
            r.arrived(&self.device).await;
            self.take(r, |w| match w {
                Some(w) => w.iter().map(|v| (*v != 0) as u8).collect(),
                None => vec![0; n],
            })
        })
    }

    fn gather_lands<'a>(
        &'a mut self,
        b: &'a Backward,
        view: View,
        jobs: &'a [EvalJob<'a>],
        gathers: &'a [GatherJob<'a>],
    ) -> crate::scene::backward::Ask<'a, (Vec<u8>, Vec<Gathered>)> {
        Box::pin(async move {
            if gathers.is_empty() {
                let Some((r, n)) = self.run_submit(view, jobs, Mode::Disc) else { return (Vec::new(), Vec::new()) };
                r.arrived(&self.device).await;
                let a = self.take(r, |w| match w {
                    Some(w) => w.iter().map(|v| (*v != 0) as u8).collect(),
                    None => vec![0; n],
                });
                return (a, Vec::new());
            }
            assert!(self.gathers_here(), "the web's GPU planner gathers on the GPU");
            let (r, parse) = self.fused_submit(view, jobs, gathers, b);
            r.arrived(&self.device).await;
            self.take(r, |w| parse.read(w))
        })
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
        // the planner thread. (The web cannot block on the scope; a kernel
        // that failed there reports through the device's error handler,
        // and its answers come back empty.)
        #[cfg(not(target_arch = "wasm32"))]
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut gpu = PlanGpu::new(&self.device, &self.queue, flame, b.sample());
        gpu.attach(b);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(err) = pollster::block_on(scope.pop()) {
            log::warn!("the GPU planner's kernel did not build for this flame; planning on the CPU: {err}");
            self.failed = Some(key);
            return None;
        }
        self.failed = None;
        self.current = Some((key, gpu));
        self.current.as_mut().map(|(_, g)| g)
    }

    /// [`Self::for_flame`], yielding at `slicer`'s ticks between building
    /// the kernel and uploading the indexes: the web's. It cannot wait on
    /// a validation scope there; a kernel that fails reports through the
    /// device's error handler and answers nothing. (The desktop, testing
    /// the web's path, can, and does as `for_flame` does.)
    ///
    /// Building the kernel is one piece no tick can split. A native driver
    /// compiles it here, measured at 10-26 ms, and says so to
    /// [`Slicer::compiled`](crate::scene::slice::Slicer::compiled). A
    /// browser compiles it in its GPU process -- not in the page's step,
    /// but on the thread that serves the page's rendering, so the page's
    /// next frame can wait for it (`gpu-cylinder-planning.md` §17).
    pub async fn for_flame_sliced(
        &mut self,
        flame: &Flame,
        b: &Backward,
        slicer: &crate::scene::backward::Slicer,
    ) -> Option<&mut PlanGpu> {
        let key = Backward::flame_key(flame);
        if self.failed == Some(key) {
            return None;
        }
        if !self.current.as_ref().is_some_and(|(k, g)| *k == key && g.points_len == b.sample().len()) {
            self.current = None;
            #[cfg(not(target_arch = "wasm32"))]
            let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            #[cfg(not(target_arch = "wasm32"))]
            let built = web_time::Instant::now();
            let mut gpu = PlanGpu::new(&self.device, &self.queue, flame, b.sample());
            #[cfg(not(target_arch = "wasm32"))]
            slicer.add_compiled(built.elapsed());
            // Popped before the tick: a scope open across a yield would
            // catch whatever else the thread does meanwhile.
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(err) = pollster::block_on(scope.pop()) {
                log::warn!("the GPU planner's kernel did not build for this flame; planning on the CPU: {err}");
                self.failed = Some(key);
                return None;
            }
            slicer.tick().await;
            gpu.attach_sliced(b, slicer).await;
            self.failed = None;
            self.current = Some((key, gpu));
        }
        self.current.as_mut().map(|(_, g)| g)
    }
}
