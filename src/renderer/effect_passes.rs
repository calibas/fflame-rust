//! The effect chain's multi-pass effects (docs/projects/multipass-effects.md).
//!
//! An effect whose `EffectInfo::passes` is set runs each of its passes in
//! order, each repeat with its own parameter slot (the pass fields of
//! `EffectParams`), each writing one of its buffers or, the last, the
//! chain's output. What the passes are is the effect's (`effects::passes`);
//! what they hold between passes -- and, for a persistent buffer, between
//! frames -- is here, one [`State`] per effect in a chain.
//!
//! Buffers are `Rgba32Float`. A plain one is a ping-pong pair: a pass that
//! reads the buffer it writes reads the version before. A pyramid is one
//! texture with mips, bound a level at a time: a pass over its levels
//! writes one and reads its target at the level it came from (a zero
//! texture at the first), every other pyramid at the level it writes. A
//! level and its neighbour are distinct subresources, so writing one while
//! reading the other is legal in WebGPU.
//!
//! A persistent buffer outlives the frame. What a run does with it is the
//! clock's ([`super::effect_chain::EffectClock`]) against the clock the
//! state last ran at: the same frame again -- the viewport converging,
//! an edit while paused -- re-reads the history it read before; the next
//! frame makes the last write the history; anything else -- a seek, a
//! rewind, a loop's wrap, a Reset, a still -- starts from zero.

use std::sync::Arc;

use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer,
    BufferBinding, BufferBindingType, Color, ColorTargetState, ColorWrites, CommandEncoder, Device, Extent3d, FragmentState, LoadOp, MultisampleState,
    Operations, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension, VertexState,
};

use super::effect_chain::{EffectClock, EffectParams};
use crate::effects::passes::{Order, Passes, Repeat};

/// A buffer's texel format: four f32s, any numbers (a jump flood's
/// coordinates past 2048 need f32). Renderable, not blendable: no pass
/// blends into a buffer.
pub const BUFFER_FORMAT: TextureFormat = TextureFormat::Rgba32Float;

/// An effect's passes compiled: one pipeline a pass, over one layout.
pub(super) struct Compiled {
    /// The registry's revision it was built from.
    pub revision: u64,
    pub plan: Arc<Passes>,
    layout: BindGroupLayout,
    pipelines: Vec<RenderPipeline>,
}

impl Compiled {
    /// `module` (the effect's WGSL, includes spliced) for `plan`, its last
    /// pass writing `output` (the chain's format) with `blend`.
    pub fn new(device: &Device, name: &str, module: &str, plan: Arc<Passes>, revision: u64, output: TextureFormat, blend: Option<BlendState>) -> Self {
        let module = device.create_shader_module(ShaderModuleDescriptor { label: Some(&format!("{name} Shader")), source: ShaderSource::Wgsl(module.into()) });
        let texture = |binding: u32, filterable: bool| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture { sample_type: TextureSampleType::Float { filterable }, view_dimension: TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        // The image (filterable, as one pass's is), its sampler, the
        // parameter slot; then each buffer, unfilterable (every read a
        // `textureLoad`).
        let mut entries = vec![
            texture(0, true),
            BindGroupLayoutEntry { binding: 1, visibility: ShaderStages::FRAGMENT, ty: BindingType::Sampler(SamplerBindingType::Filtering), count: None },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
        ];
        for k in 0..plan.buffers.len() {
            entries.push(texture(3 + k as u32, false));
        }
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor { label: Some(&format!("{name} Bind Group Layout")), entries: &entries });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{name} Pipeline Layout")),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = plan
            .passes
            .iter()
            .map(|pass| {
                let (format, blend) = if pass.to.is_some() { (BUFFER_FORMAT, None) } else { (output, blend) };
                device.create_render_pipeline(&RenderPipelineDescriptor {
                    label: Some(&format!("{name} {} Pipeline", pass.entry)),
                    layout: Some(&pipeline_layout),
                    vertex: VertexState { module: &module, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
                    fragment: Some(FragmentState {
                        module: &module,
                        entry_point: Some(&pass.entry),
                        targets: &[Some(ColorTargetState { format, blend, write_mask: ColorWrites::ALL })],
                        compilation_options: Default::default(),
                    }),
                    primitive: PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
            })
            .collect();
        Self { revision, plan, layout, pipelines }
    }
}

/// One buffer's textures.
enum Buf {
    /// A ping-pong pair; `current` the version a read sees. A persistent
    /// one's `held` is the version the last frame left, which a run of the
    /// same frame again reads.
    Plain { textures: [Texture; 2], views: [TextureView; 2], current: usize, size: (u32, u32), persist: bool, held: usize },
    /// One texture with a mip a level: a view of each level, and of all.
    Pyramid { texture: Texture, all: TextureView, levels: Vec<TextureView>, sizes: Vec<(u32, u32)> },
}

impl Buf {
    fn destroy(&self) {
        match self {
            Buf::Plain { textures, .. } => textures.iter().for_each(Texture::destroy),
            Buf::Pyramid { texture, .. } => texture.destroy(),
        }
    }
}

/// What one effect in a chain holds between its passes: its buffers, for
/// one size, effect and revision.
pub(super) struct State {
    name: String,
    revision: u64,
    size: (u32, u32),
    buffers: Vec<Buf>,
    /// One zero texel: what a pyramid pass reads of its target where there
    /// is no level to come from.
    zero: (Texture, TextureView),
    /// The clock's epoch and frame at the last run, `None` before one (or
    /// after a still): what tells a run of the same frame from the next.
    last: Option<(u64, u64)>,
}

/// What a run does with the persistent buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tick {
    /// The same frame again: read the history the last run read.
    Repeat,
    /// The next frame: the last write is the history.
    Advance,
    /// No history: zero.
    Fresh,
}

/// What a run at `clock` does, after one at `last`.
pub(super) fn tick(last: Option<(u64, u64)>, clock: &EffectClock) -> Tick {
    match last {
        _ if clock.still => Tick::Fresh,
        Some((e, f)) if e == clock.epoch && f == clock.frame => Tick::Repeat,
        Some((e, f)) if e == clock.epoch && clock.frame == f + 1 => Tick::Advance,
        _ => Tick::Fresh,
    }
}

impl State {
    /// Buffers for `compiled`'s plan at a frame of `w` x `h`.
    pub fn new(device: &Device, name: &str, compiled: &Compiled, (w, h): (u32, u32)) -> Self {
        let texture = |label: &str, (tw, th): (u32, u32), mips: u32| {
            device.create_texture(&TextureDescriptor {
                label: Some(label),
                size: Extent3d { width: tw, height: th, depth_or_array_layers: 1 },
                mip_level_count: mips,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: BUFFER_FORMAT,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let buffers = compiled
            .plan
            .buffers
            .iter()
            .map(|b| {
                let size = b.size(w, h);
                if b.pyramid() {
                    let n = b.levels_at(w, h);
                    let t = texture(&format!("{name} {}", b.name), size, n);
                    let levels = (0..n)
                        .map(|l| t.create_view(&TextureViewDescriptor { base_mip_level: l, mip_level_count: Some(1), ..Default::default() }))
                        .collect();
                    let sizes = (0..n).map(|l| ((size.0 >> l).max(1), (size.1 >> l).max(1))).collect();
                    Buf::Pyramid { all: t.create_view(&TextureViewDescriptor::default()), texture: t, levels, sizes }
                } else {
                    let a = texture(&format!("{name} {} A", b.name), size, 1);
                    let bb = texture(&format!("{name} {} B", b.name), size, 1);
                    let views = [a.create_view(&TextureViewDescriptor::default()), bb.create_view(&TextureViewDescriptor::default())];
                    Buf::Plain { textures: [a, bb], views, current: 0, size, persist: b.persist, held: 0 }
                }
            })
            .collect();
        let zero = texture(&format!("{name} zero"), (1, 1), 1);
        let zero_view = zero.create_view(&TextureViewDescriptor::default());
        Self { name: name.to_string(), revision: compiled.revision, size: (w, h), buffers, zero: (zero, zero_view), last: None }
    }

    /// Move the persistent buffers to `clock`'s frame (the module's docs):
    /// the version to read chosen, or both versions cleared to zero.
    fn tick(&mut self, encoder: &mut CommandEncoder, clock: &EffectClock) {
        let t = tick(self.last, clock);
        for b in &mut self.buffers {
            let Buf::Plain { views, current, persist: true, held, .. } = b else { continue };
            match t {
                Tick::Repeat => *current = *held,
                Tick::Advance => *held = *current,
                Tick::Fresh => {
                    for v in views.iter() {
                        clear(encoder, v);
                    }
                    *current = 0;
                    *held = 0;
                }
            }
        }
        self.last = (!clock.still).then_some((clock.epoch, clock.frame));
    }

    /// Whether it holds buffers for this effect, revision and size.
    pub fn fits(&self, name: &str, compiled: &Compiled, size: (u32, u32)) -> bool {
        self.name == name && self.revision == compiled.revision && self.size == size
    }

    pub fn destroy(&self) {
        self.buffers.iter().for_each(Buf::destroy);
        self.zero.0.destroy();
    }
}

/// What a run of the effect reads besides its buffers.
pub(super) struct Inputs<'a> {
    pub input: &'a TextureView,
    pub output: &'a TextureView,
    pub sampler: &'a Sampler,
    pub params_buffer: &'a Buffer,
    /// One parameter slot a pass run, in order (`Passes::runs`).
    pub slots: &'a [u64],
    /// The effect's parameters and the frame's fields, which each pass
    /// run's slot copies and gives its pass fields.
    pub base: EffectParams,
}

/// A texture cleared to zero: a render pass that only clears.
fn clear(encoder: &mut CommandEncoder, view: &TextureView) {
    encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("effect buffer clear"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: Operations { load: LoadOp::Clear(Color::TRANSPARENT), store: StoreOp::Store },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });
}

/// Run every pass of `compiled` on `inputs.input` into `inputs.output`, at
/// `clock`'s frame.
pub(super) fn run(device: &Device, queue: &Queue, encoder: &mut CommandEncoder, compiled: &Compiled, state: &mut State, inputs: &Inputs<'_>, clock: &EffectClock) {
    if compiled.plan.temporal() {
        state.tick(encoder, clock);
    }
    let plan = &compiled.plan;
    let (w, h) = state.size;
    let total = plan.runs(w, h);
    if inputs.slots.len() < total {
        log::warn!("{}: not enough parameter slots ({} < {total})", state.name, inputs.slots.len());
        return;
    }
    let params_size = std::num::NonZeroU64::new(std::mem::size_of::<EffectParams>() as u64).expect("not zero");
    let mut slot = 0;
    for (k, pass) in plan.passes.iter().enumerate() {
        let n = pass.runs(&plan.buffers, w, h);
        for r in 0..n {
            // The level a pass over a pyramid writes, and the level of its
            // target it reads, where there is one.
            let (level, from) = if pass.repeat == Repeat::Levels {
                match pass.order {
                    Order::Down => (r, r.checked_sub(1)),
                    Order::Up => {
                        let l = n - 1 - r;
                        (l, (l + 1 < n).then_some(l + 1))
                    }
                }
            } else {
                (0, None)
            };
            let (target, size) = match pass.to.map(|t| &state.buffers[t]) {
                None => (inputs.output, (w, h)),
                Some(Buf::Plain { views, current, size, .. }) => (&views[1 - current], *size),
                Some(Buf::Pyramid { levels, sizes, .. }) => (&levels[level as usize], sizes[level as usize]),
            };
            let mut buffer_levels = 0u32;
            let mut entries = vec![
                BindGroupEntry { binding: 0, resource: BindingResource::TextureView(inputs.input) },
                BindGroupEntry { binding: 1, resource: BindingResource::Sampler(inputs.sampler) },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding { buffer: inputs.params_buffer, offset: inputs.slots[slot], size: Some(params_size) }),
                },
            ];
            for (j, b) in state.buffers.iter().enumerate() {
                let view = match b {
                    // The version before, when it is the target: the
                    // write goes to the other.
                    Buf::Plain { views, current, .. } => &views[*current],
                    Buf::Pyramid { levels, .. } if pass.to == Some(j) => match from {
                        Some(l) => &levels[l as usize],
                        None => &state.zero.1,
                    },
                    Buf::Pyramid { all, levels, .. } => {
                        let l = level.min(levels.len() as u32 - 1);
                        buffer_levels |= l << (4 * j);
                        all
                    }
                };
                entries.push(BindGroupEntry { binding: 3 + j as u32, resource: BindingResource::TextureView(view) });
            }
            let mut params = inputs.base;
            params.width = size.0;
            params.height = size.1;
            params.pass_index = r;
            params.passes = n;
            params.step = 1u32.checked_shl(n - 1 - r).unwrap_or(0);
            params.level = level;
            params.levels = match pass.to.map(|t| &state.buffers[t]) {
                Some(Buf::Pyramid { levels, .. }) => levels.len() as u32,
                _ => 1,
            };
            params.buffer_levels = buffer_levels;
            queue.write_buffer(inputs.params_buffer, inputs.slots[slot], bytemuck::bytes_of(&params));
            slot += 1;
            let bind_group = device.create_bind_group(&BindGroupDescriptor { label: Some(&format!("{} pass {k}", state.name)), layout: &compiled.layout, entries: &entries });
            {
                let mut rp = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some(&format!("{} {}", state.name, pass.entry)),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: target,
                        resolve_target: None,
                        ops: Operations { load: LoadOp::Clear(Color::BLACK), store: StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                });
                rp.set_pipeline(&compiled.pipelines[k]);
                rp.set_bind_group(0, &bind_group, &[]);
                rp.draw(0..3, 0..1);
            }
            if let Some(Buf::Plain { current, .. }) = pass.to.map(|t| &mut state.buffers[t]) {
                *current = 1 - *current;
            }
        }
    }
}
