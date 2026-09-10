//! `SimRenderer` — the stateful grid, its passes, and the step loop.
//!
//! Owns a **ping-pong pair** of `Rgba32Float` textures at the
//! simulation grid size, plus one output image at the display size in
//! the flame accumulator's layout, so the shared tonemap → effects →
//! readback tail consumes it exactly as it consumes escape's.
//!
//! Why a pair rather than one texture: wgpu rejects read-write storage
//! access on `rgba32float`, so a step reads `field[i]` as a sampled
//! texture and writes `field[1 - i]` as a write-only storage texture.
//! That is the only portable shape, and everything else here follows
//! from it.
//!
//! Three things this renderer is responsible for that the escape one
//! is not:
//!
//! * **State across frames.** `step_index` and the field survive the
//!   frame; a still is "the state at step N from this seed". Reseeding
//!   is explicit.
//! * **The grid is not the output.** Colouring happens at grid
//!   resolution and the same pass resolves to the output size, so a
//!   256-cell model shown at 1080p stays 256 cells of information.
//! * **Batching against the watchdog.** A 10,000-step export cannot be
//!   one submission. [`Self::run_steps`] splits by a measured budget;
//!   phase 0 established that per-submit overhead is 0.8% across a
//!   256x range, so batching is purely a watchdog and pacing device.

use crate::config::sim::{SimConfig, SimGrid, MAX_COUPLINGS, MAX_LAYERS};

/// One colour-stack layer as the colour shader reads it; mirrored in
/// the assembler's `SimColorLayerGpu`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SimColorLayerGpu {
    source: u32,
    blend: u32,
    enabled: u32,
    /// 1 to read four layers' first channels from `source` onward.
    gather: u32,
    opacity: f32,
    edge: f32,
    pad1: f32,
    pad2: f32,
    matte: [f32; 4],
}

/// One coupling as the step shader reads it; mirrored in the
/// assembler's `SimCouplingGpu`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SimCouplingGpu {
    to: u32,
    from: u32,
    form: u32,
    mask: u32,
    strength: f32,
    /// The driving layer's kernel table, for the Signal form: offset
    /// into the shared LUT, half-width, length in floats. Length 0
    /// with `signal_channel` below 4: the driver publishes its signal
    /// in that channel (`ModelFeature::PublishesSignal`), read
    /// instead of convolved.
    k_offset: u32,
    k_radius: u32,
    k_len: u32,
    signal_channel: u32,
    pad: [u32; 3],
}

/// Floats in the model-parameter buffer. Sixteen was every model until
/// the coupled Turing lattice, whose coupling matrix alone is sixteen;
/// `every_model_fits_the_parameter_buffer` keeps this honest.
pub const MODEL_PARAM_SLOTS: usize = 32;
use crate::sim::{assembler, coloring_or_default, model_or_default, pyramid_levels, ModelDef, ModelFeature, SimColoringDef, MAX_KERNEL_RADIUS, MAX_PYRAMID_LEVELS, MINMAX_RING, MAX_AGENTS};
#[allow(unused_imports)]
use crate::sim::ColoringFeature;
use wgpu::util::DeviceExt;
use wgpu::*;

/// Most steps in one submission, and the size of the uniform ring.
///
/// This used to be THE batch size, justified by phase 0's numbers
/// (a 1080p stencil step at 0.5 ms, so 256 steps is ~0.1 s). Those
/// numbers were for a 3×3 stencil. Cyclic CA at range 5 is 121 reads
/// a cell and 9.7 ms a step at 1080p, so one 256-step submit is 2.5 s
/// — past Windows' 2 s GPU watchdog, which resets the device. The
/// fence signals anyway, the run reports a fictional cost, and the
/// process aborts at teardown; the shipped binary's `export` of that
/// config failed with "Parent device is lost". Pinned between 192
/// steps (1.8 s, clean) and 224 (2.3 s, lost). So the batch is now
/// sized from measured cost, and this is only its ceiling.
const MAX_STEPS_PER_SUBMIT: u32 = 256;

/// Steps in the first submission after a pipeline or grid change,
/// before anything has been measured. Small enough that even a kernel
/// an order of magnitude slower than range-5 cyclic CA (phase 3's
/// large-kernel models) stays well inside the watchdog: 8 steps at
/// 60 ms is half a second.
///
/// That arithmetic assumed a step is one or two dispatches. A model
/// with a repeated pass can be two hundred, so `run_steps` scales this
/// down by the dispatches in a step before the first, blind submit --
/// the dielectric breakdown model at 4K with 200 relaxation sweeps
/// would otherwise put about 1.6 s in one submission against a 2 s
/// watchdog.
const FIRST_SUBMIT: u32 = 8;

/// Jump-flood passes a frame can need: ceil(log2 N) + 1, so sixteen is
/// a 32768-cell grid, past the grid cap.
const JFA_SLOTS: usize = 16;

/// Wall-clock budget for one submission. An eighth of the watchdog,
/// so a card half as fast as the one measured on, or a frame that
/// shares the GPU with something else, still has margin; and long
/// enough that at 0.3 ms a step the ceiling is what binds.
const SUBMIT_BUDGET_MS: f64 = 250.0;

/// Cap on a single dimension of the grid. The real limit is the
/// device's `max_texture_dimension_2d`, which [`SimRenderer::allocation_error`]
/// checks; this keeps arithmetic sane before a device is in scope.
const MAX_GRID_DIM: u32 = 8192;

/// Ring slots for per-step uniforms. One per step in a submission, so
/// each step reads its own step index.
/// Ring slots: one per (step, layer, variant), where the variant is
/// the layer's own step or the copy-through a layer makes in a stage
/// it has no pass for. The batch divides by layers x 2 to fit.
const RING_SLOTS: u32 = MAX_STEPS_PER_SUBMIT * 2;
/// The second slot of a (step, layer) pair: the warp with an all-zero
/// channel mask, which carries the layer across unchanged.
const VARIANT_COPY: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SimParamsGpu {
    grid: [u32; 2],
    out_size: [u32; 2],
    step_index: u32,
    seed_lo: u32,
    seed_hi: u32,
    dt: f32,
    init_p0: f32,
    init_p1: f32,
    /// Half-width of the convolution kernel in the LUT, so the step
    /// shader can bound its gather. 0 for a model with no kernel.
    kernel_radius: u32,
    /// Min/max ring slot for this dispatch: the reduce pass writes it,
    /// the step pass reads the slot `minmax_back` before it.
    minmax_slot: u32,
    /// The layer this dispatch is (simulation-layers plan, section 2):
    /// the slice it reads as its own and writes.
    layer: u32,
    /// Where this layer's convolution table starts in the shared LUT.
    kernel_offset: u32,
    /// How far back the previous min/max slot of the same layer is:
    /// the layer count.
    minmax_back: u32,
    /// Entries of the coupling table in force.
    coupling_count: u32,
    /// The warp stage's per-step affine: zoom, rotation, pan x, pan y.
    /// A vec4 in WGSL, so 16-aligned: the ten words above end at 48.
    warp_a: [f32; 4],
    /// Swirl rate and the filter (0 bilinear, 1 nearest), then padding
    /// to the struct's 16-byte alignment. Mirrored in `SimParams` in
    /// the assembler; the sizes must agree.
    warp_b: [f32; 2],
    /// The matte's edge -- 1 for a distance field, 0 for a threshold
    /// -- and a spare word. Fills what was padding.
    matte_b: [f32; 2],
    /// The matte: channel index, mode (0 off, 1 normal, 2 inverted),
    /// cutoff, softness. `SimMatte::packed` builds it, and that
    /// function's mode word is what the shader branches on.
    matte: [f32; 4],
    /// x: the view magnification the colour pass applies about the
    /// grid's centre -- the octave mode's accumulated zoom, 1
    /// otherwise. y: the fit, 0 letterbox, 1 cover. z: 1 when the step
    /// freezes cells outside the visible window (octave mode with
    /// `cull`). w: the halo around that window, in cells.
    view: [f32; 4],
    /// Which channels the warp moves, 1 or 0 per channel. Continuous
    /// mode only; all four in octave mode.
    warp_mask: [f32; 4],
    /// x: the layer map's rate for this layer (simulation-layers plan,
    /// section 4); 0 when the config does not use transforms.
    xform: [f32; 4],
}

/// The flame's transforms as the layers' maps: the definitions the
/// shader builder produced for this flame, the buffers the flame
/// kernel would bind, and each layer's rate.
struct LayerMap {
    /// What the definitions were built from: the active variation
    /// names in order, the transform count, the post-affine and
    /// attachment flags.
    key: LayerMapKey,
    defs: String,
    transforms: Buffer,
    variation_params: Buffer,
    flame_params: Buffer,
    attachments: Buffer,
    subflame_meta: Buffer,
    /// Per transform, its weight clamped to [0, 1].
    rates: Vec<f32>,
    bind_group: Option<BindGroup>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct LayerMapKey {
    variations: Vec<String>,
    transforms: usize,
    post_affine: bool,
    attachments: bool,
    attachment_cap: usize,
}

impl LayerMapKey {
    fn of(flame: &crate::scene::transforms::Flame) -> Self {
        LayerMapKey {
            variations: flame.active_variation_names_ordered(&crate::variations::global_registry()),
            transforms: flame.transforms.len(),
            post_affine: flame.has_post_affine(),
            attachments: flame.has_attachments(),
            attachment_cap: flame.attachment_cap(),
        }
    }

    fn hash64(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }
}

/// The part of a `SimConfig` the FIELD's meaning depends on.
///
/// A field is only ever the state of the run that produced it. Change
/// the rule, what a step reads past the edges, the shape it started
/// from or the RNG stream, and the cells sitting in the texture are
/// the answer to a question nobody asked -- so the run restarts.
///
/// This exists because a whole-config replacement does not come
/// through the delta path that computes `UpdateType::SimReseed`:
/// loading a file, applying a preset, a script writing a config, the
/// animation exporter building one per frame. Before it, loading a
/// file left the previous simulation's field on screen.
///
/// Deliberately NOT included: model and colouring parameters. Turning
/// Gray-Scott's feed rate while it runs is what the slider is for, and
/// reseeding on it would make every model unusable. The grid is absent
/// for a different reason -- `resize` already reseeds when it changes.
#[derive(Clone, PartialEq)]
struct SeedIdentity {
    /// The model of every layer, layer 0 first.
    layers: Vec<&'static str>,
    boundary: crate::config::sim::SimBoundary,
    init: crate::config::sim::SimInit,
    seed: u64,
}

impl SeedIdentity {
    fn of(cfg: &SimConfig) -> Self {
        SeedIdentity {
            layers: layer_models(cfg).iter().map(|m| m.name).collect(),
            boundary: cfg.boundary,
            init: cfg.init,
            seed: cfg.seed,
        }
    }
}

/// The model of each layer, layer 0 first; one entry for a config
/// without layers.
fn layer_models(cfg: &SimConfig) -> Vec<&'static ModelDef> {
    (0..cfg.layer_count()).map(|l| model_or_default(cfg.layer_model_name(l))).collect()
}

/// The shader set for one (model, colouring, boundary, resolve)
/// combination. Rebuilt when any of those change, which is rare —
/// parameter edits do not touch it.
struct Pipelines {
    seed: ComputePipeline,
    /// Per layer, the dispatches of one step in order: `sim_step`,
    /// then `sim_step2`, and so on for as many as that layer's model
    /// declares. Layers sharing a model share the compiled pipelines.
    layer_steps: Vec<Vec<ComputePipeline>>,
    /// The layer whose model has agents, if one does. One population
    /// and one deposit buffer serve the grid, so one layer.
    agent_layer: Option<usize>,
    /// Per layer, its model's seed pipeline.
    layer_seeds: Vec<ComputePipeline>,
    /// The layer-map warp -- group 0 the step layout, group 1 the
    /// flame's buffers -- built when the config uses transforms and a
    /// map has been set.
    layer_warp: Option<ComputePipeline>,
    flame_layout: Option<BindGroupLayout>,
    /// The warp stage: a resample of the field through the per-step
    /// affine, first in the step. Built with every set (it depends on
    /// the boundary alone) and dispatched only when the config's warp
    /// is not the identity.
    warp: ComputePipeline,
    color: ComputePipeline,
    /// One pyramid level from the one below it; built only for a
    /// model that declares `NeedsPyramid`.
    pyramid: Option<ComputePipeline>,
    /// The global min/max reduce; built only for `NeedsMinMax`.
    reduce: Option<ComputePipeline>,
    /// The jump flood -- seed, jump, and seeds-to-distance -- for a
    /// matte whose edge is Distance. Always built; they depend on
    /// nothing in the key.
    jfa_init: ComputePipeline,
    jfa_step: ComputePipeline,
    jfa_final: ComputePipeline,
    jfa_layout: BindGroupLayout,
    /// The agent passes and their seeding, for `NeedsAgents`.
    agents: Vec<ComputePipeline>,
    agent_seed: Option<ComputePipeline>,
    agent_layout: BindGroupLayout,
    seed_layout: BindGroupLayout,
    step_layout: BindGroupLayout,
    reduce_layout: BindGroupLayout,
    color_layout: BindGroupLayout,
    /// What the pipelines were built for, so the renderer knows when
    /// they are stale.
    key: PipelineKey,
}

#[derive(Clone, PartialEq, Eq)]
struct PipelineKey {
    layers: Vec<&'static str>,
    /// Whether the step shaders carry the coupling.
    coupled: bool,
    /// The layer map's definitions, or 0 when transforms are not used.
    layer_map: u64,
    coloring: &'static str,
    /// The colour stack's colourings, in order; empty for the single
    /// colouring.
    stack: Vec<&'static str>,
    boundary: crate::config::sim::SimBoundary,
    upscale: crate::config::sim::SimUpscale,
    downscale: crate::config::sim::SimDownscale,
    // `&'static`: `SimInit::kind_name` returns one, and this key is
    // rebuilt three times per frame -- a String here was three
    // allocations per frame for nothing.
    init_kind: &'static str,
    magnifying: bool,
}

pub struct SimRenderer {
    /// Simulation grid, in cells.
    grid_w: u32,
    grid_h: u32,
    /// Output image, in pixels.
    out_w: u32,
    out_h: u32,

    field: [Texture; 2],
    field_view: [TextureView; 2],
    /// Which of the pair currently holds the live state.
    current: usize,

    output_texture: Texture,
    output_view: TextureView,

    /// A RING of per-step uniforms, addressed by dynamic offset.
    ///
    /// Not one uniform rewritten per step: `Queue::write_buffer` is
    /// staged and applied before the command buffer executes, so every
    /// step in a submission would read the LAST step's values. Measured,
    /// that silently corrupted the age channel in every batched run
    /// (93 texels of 4,096 differed between one batch of 300 and 300
    /// batches of one) while the concentration channels looked right --
    /// and it cost a `write_buffer` per step, which is what made 4
    /// steps at 1080p take 39.8 ms instead of ~2.
    params_buffer: Buffer,
    /// Distance between ring slots, `min_uniform_buffer_offset_alignment`
    /// rounded up from the struct size.
    params_stride: u64,
    model_params_buffer: Buffer,
    coloring_params_buffer: Buffer,

    pipelines: Option<Pipelines>,
    /// One bind group per ping-pong direction, built with the
    /// pipelines and reused for every step in every batch.
    step_bind_groups: Option<[BindGroup; 2]>,

    /// Steps applied since the last reseed. The state's identity.
    step_index: u32,
    /// Per layer: the convolution table's radius (0 = none) and where
    /// its block starts in `kernel_buffer`.
    kernel_radii: Vec<u32>,
    kernel_offsets: Vec<u32>,
    /// Each layer's table length in floats; 0 for a model without one.
    kernel_lens: Vec<u32>,
    /// How many slices the field arrays carry.
    layers: u32,
    /// The coupling table (`SimCouplingGpu` x MAX_COUPLINGS).
    coupling_buffer: Buffer,
    /// The flame's transforms as layer maps, once `set_layer_transforms`
    /// has been called.
    layer_map: Option<LayerMap>,
    /// The colour stack's records (`SimColorLayerGpu` x MAX_COLOR_LAYERS).
    color_layers_buffer: Buffer,
    /// The convolution table for the large-kernel models, rebuilt and
    /// uploaded with the parameters. Sized once for the largest
    /// kernel the engine allows, so it never resizes.
    kernel_buffer: Buffer,
    /// Pyramid levels 1.., each half the previous (rounded up). Level 0
    /// is the field itself. Allocated by `ensure_pyramid` for a model
    /// that declares `NeedsPyramid` and rebuilt every step; empty
    /// otherwise, and freed again when the model changes to one that
    /// does not read it.
    pyramid: Vec<(Texture, TextureView, TextureView)>,
    /// A 1x1 texture bound to every pyramid slot a model does not use:
    /// the layout carries seven, and a bind group must fill them.
    pyramid_dummy: (Texture, TextureView),
    /// The jump flood's ping-pong pair and its result, the signed
    /// distance field, all at grid size. Allocated when the matte's
    /// edge is Distance and freed when it is not -- three grid-sized
    /// textures are 800 MB at 4K and nothing reads them otherwise.
    jfa: Option<[(Texture, TextureView); 2]>,
    sdf: Option<(Texture, TextureView)>,
    /// Bound at the colour pass's distance slot when there is no
    /// distance field; the shader never reads it then.
    sdf_dummy: (Texture, TextureView),
    /// One `SimParamsGpu` per jump-flood pass, its jump in the
    /// kernel-radius word. Sixteen slots covers a 32768-cell grid.
    jfa_params_buffer: Buffer,
    /// One uniform per pyramid level, holding that level's SOURCE size
    /// in `grid` so the shared boundary wrap applies at the right
    /// scale. Same layout as the step ring; selected by dynamic offset.
    level_params_buffer: Buffer,
    /// The min/max ring: `MINMAX_RING` slots of two ordered u32s.
    minmax_buffer: Buffer,
    /// The agent population, 16 bytes each. Allocated to the count the
    /// model asks for and reallocated when that changes.
    agent_buffer: Option<Buffer>,
    agent_capacity: u32,
    /// One u32 per cell: what the agents deposited since the last
    /// step, fixed-point, folded and cleared by the step pass.
    deposit_buffer: Buffer,
    /// One u32 per cell: the lowest index of an agent claiming it this
    /// step, or `u32::MAX`. Only a two-pass population uses it.
    claim_buffer: Buffer,
    /// One per ping-pong side: agents SENSE the live field.
    agent_bind_groups: Option<[BindGroup; 2]>,
    /// Pyramid bind groups: `[level][src]` for level 0 (which reads the
    /// field, so it depends on the ping-pong side), a single group for
    /// every level above. Rebuilt with the step bind groups.
    pyramid_bind_groups: Option<Vec<Vec<BindGroup>>>,
    /// Reduce bind groups per ping-pong side.
    reduce_bind_groups: Option<[BindGroup; 2]>,
    /// Steps in the next submission: adapted from the measured cost
    /// of the previous one, reset to [`FIRST_SUBMIT`] whenever the
    /// pipeline or the grid changes and the old measurement no longer
    /// describes the kernel.
    steps_per_submit: u32,
    /// Measured cost of one step, in milliseconds, from the same
    /// timing that sizes `steps_per_submit`. Zero until a batch has
    /// run. Used by the interactive driver to size a per-display-frame
    /// budget so catching up to a timeline target never blocks the UI.
    ms_per_step: f64,
    /// Set when the field has not been seeded yet, or the config
    /// changed in a way that invalidates it.
    needs_seed: bool,
    /// What the live field was seeded from, or `None` before the
    /// first seed. Compared against the config every frame, so a
    /// config that arrives by any route at all cannot inherit the
    /// previous run's field.
    seeded_as: Option<SeedIdentity>,
}

impl SimRenderer {
    pub fn new(device: &Device, cfg: &SimConfig, out_w: u32, out_h: u32) -> Self {
        let (grid_w, grid_h) = Self::allocatable_grid(cfg, out_w, out_h);
        let layers = cfg.layer_count();
        let (field, field_view) = Self::create_field_pair(device, grid_w, grid_h, layers as u32);
        let (output_texture, output_view) = Self::create_output(device, out_w, out_h);

        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let params_stride = (std::mem::size_of::<SimParamsGpu>() as u64).div_ceil(align) * align;
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sim Params Ring"),
            size: params_stride * RING_SLOTS as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Storage buffers, not uniforms: the arrays are runtime-sized
        // and a uniform would need a fixed maximum. They are tiny and
        // read once per invocation.
        let model_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Sim Model Params"),
            contents: bytemuck::cast_slice(&[0.0f32; MODEL_PARAM_SLOTS * MAX_LAYERS]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });
        let coloring_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Sim Coloring Params"),
            contents: bytemuck::cast_slice(&[0.0f32; 16 * crate::config::sim::MAX_COLOR_LAYERS]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });
        let color_layers_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Sim Colour Layers"),
            contents: bytemuck::cast_slice(
                &[<SimColorLayerGpu as bytemuck::Zeroable>::zeroed(); crate::config::sim::MAX_COLOR_LAYERS],
            ),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // Allocated lazily by `ensure_pyramid`: it is a third of a
        // field texture again (44 MB at 4K), and only one model reads
        // it.
        let pyramid = Vec::new();
        let pyramid_dummy = Self::create_level(device, 1, 1, "Sim Pyramid Dummy");
        let sdf_dummy = Self::create_level(device, 1, 1, "Sim SDF Dummy");
        let jfa_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sim JFA Params"),
            size: params_stride * JFA_SLOTS as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let level_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sim Level Params"),
            size: params_stride * MAX_PYRAMID_LEVELS as u64 * MAX_LAYERS as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (deposit_buffer, claim_buffer) = Self::create_cell_buffers(device, grid_w, grid_h);
        let minmax_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sim MinMax Ring"),
            size: (MINMAX_RING as u64) * 2 * 4,
            // COPY_SRC so a test can read a slot back against a CPU
            // min/max of the same field.
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Two blocks of (2R+1)^2 at the maximum radius: a model may
        // carry a pair of kernels (SmoothLife's disc and annulus).
        let kernel_floats = 2 * (2 * MAX_KERNEL_RADIUS as usize + 1).pow(2) * MAX_LAYERS;
        let coupling_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Sim Couplings"),
            contents: bytemuck::cast_slice(&[<SimCouplingGpu as bytemuck::Zeroable>::zeroed(); MAX_COUPLINGS]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });
        let kernel_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sim Kernel LUT"),
            size: (kernel_floats * std::mem::size_of::<f32>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            grid_w,
            grid_h,
            out_w,
            out_h,
            field,
            field_view,
            current: 0,
            output_texture,
            output_view,
            params_buffer,
            params_stride,
            model_params_buffer,
            coloring_params_buffer,
            pipelines: None,
            step_bind_groups: None,
            step_index: 0,
            kernel_buffer,
            pyramid,
            pyramid_dummy,
            jfa: None,
            sdf: None,
            sdf_dummy,
            jfa_params_buffer,
            level_params_buffer,
            minmax_buffer,
            agent_buffer: None,
            agent_capacity: 0,
            deposit_buffer,
            claim_buffer,
            agent_bind_groups: None,
            pyramid_bind_groups: None,
            reduce_bind_groups: None,
            kernel_radii: Vec::new(),
            kernel_offsets: Vec::new(),
            kernel_lens: Vec::new(),
            layers: layers as u32,
            coupling_buffer,
            layer_map: None,
            color_layers_buffer,
            steps_per_submit: FIRST_SUBMIT,
            ms_per_step: 0.0,
            needs_seed: true,
            seeded_as: None,
        }
    }

    /// Grid size for a config at a given output size, as REQUESTED.
    ///
    /// Deliberately not clamped to anything a device can hold: an
    /// earlier version clamped here, which silently shrank an
    /// over-large `Fixed` grid to 8192 and made
    /// [`Self::allocation_error`]'s dimension check unreachable — the
    /// user typed a size and got a different one with no message. The
    /// clamp belongs at the two ends instead: the config manager
    /// bounds what can be entered, and the constructor bounds what can
    /// be allocated if a caller skipped the check.
    pub fn grid_for(cfg: &SimConfig, out_w: u32, out_h: u32) -> (u32, u32) {
        cfg.grid.cells_for(out_w.max(1), out_h.max(1))
    }

    /// What will actually be allocated: the requested grid, bounded so
    /// a caller that never asked [`Self::allocation_error`] gets a
    /// smaller texture rather than an aborted process.
    fn allocatable_grid(cfg: &SimConfig, out_w: u32, out_h: u32) -> (u32, u32) {
        let (w, h) = Self::grid_for(cfg, out_w, out_h);
        (w.clamp(1, MAX_GRID_DIM), h.clamp(1, MAX_GRID_DIM))
    }

    /// Whether this config can be rendered on this device, and why not
    /// if it cannot. Mirrors `EscapeRenderer::allocation_error`: the
    /// caller refuses the job rather than letting wgpu abort the
    /// process on an allocation failure.
    pub fn allocation_error(
        device: &Device,
        cfg: &SimConfig,
        out_w: u32,
        out_h: u32,
    ) -> Option<String> {
        let limits = device.limits();
        let (gw, gh) = Self::grid_for(cfg, out_w, out_h);
        let max_dim = limits.max_texture_dimension_2d;
        if gw > max_dim || gh > max_dim {
            return Some(format!(
                "simulation grid {gw}x{gh} exceeds this device's maximum texture size ({max_dim}). \
                 Use a Fixed grid, or a viewport scale below 1."
            ));
        }
        if out_w > max_dim || out_h > max_dim {
            return Some(format!(
                "output {out_w}x{out_h} exceeds this device's maximum texture size ({max_dim})."
            ));
        }
        // Two field textures plus the output, 16 bytes per texel.
        let field_bytes = 2u64 * gw as u64 * gh as u64 * 16;
        let out_bytes = out_w as u64 * out_h as u64 * 16;
        let total = field_bytes + out_bytes;
        // Not a device limit but a sanity bound: past this the machine
        // is thrashing rather than rendering, and refusing with a
        // message beats an out-of-memory abort.
        const BUDGET: u64 = 4 << 30;
        if total > BUDGET {
            return Some(format!(
                "simulation needs {:.1} GiB of GPU memory ({gw}x{gh} grid, {out_w}x{out_h} output) \
                 which is over the {:.0} GiB budget. Reduce the grid or the output size.",
                total as f64 / (1 << 30) as f64,
                BUDGET as f64 / (1 << 30) as f64
            ));
        }
        None
    }

    fn create_level(device: &Device, w: u32, h: u32, label: &str) -> (Texture, TextureView) {
        let t = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let v = t.create_view(&TextureViewDescriptor::default());
        (t, v)
    }

    /// Levels 1.. of the pyramid for a grid: separate textures rather
    /// than mip levels of one, because a level is written as a storage
    /// texture and read as a sampled one, and the two views of one
    /// texture's mips do not mix cleanly.
    fn create_pyramid(device: &Device, w: u32, h: u32) -> Vec<(Texture, TextureView, TextureView)> {
        let levels = pyramid_levels(w, h);
        let (mut lw, mut lh) = (w, h);
        (1..levels)
            .map(|l| {
                lw = lw.div_ceil(2);
                lh = lh.div_ceil(2);
                let (t, v) = Self::create_level(device, lw, lh, &format!("Sim Pyramid L{l}"));
                // The 2D view for the step's level bindings, the array
                // view for the pyramid pass, which shares the step
                // layout and so binds levels as one-layer arrays.
                let va = t.create_view(&Self::array_view());
                (t, v, va)
            })
            .collect()
    }

    /// The min/max ring, for a test to read a slot back. Two ordered
    /// u32s per slot; `step_index % MINMAX_RING` is the slot a step's
    /// reduce wrote.
    pub fn minmax_buffer(&self) -> &Buffer {
        &self.minmax_buffer
    }

    /// The two per-cell integer buffers the agent stage uses.
    fn create_cell_buffers(device: &Device, w: u32, h: u32) -> (Buffer, Buffer) {
        let cells = (w as u64) * (h as u64) * 4;
        let mk = |label: &str| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: cells.max(4),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        (mk("Sim Deposit"), mk("Sim Claim"))
    }

    /// Allocate the agent population when the model has one, at the
    /// count its parameters ask for. A change of count reallocates and
    /// forces a reseed, because a population is state and half of a
    /// new one is not a state.
    fn ensure_agents(&mut self, device: &Device, cfg: &SimConfig) {
        // The first layer with agents, if any: one population serves
        // the grid.
        let models = layer_models(cfg);
        let want = match models.iter().enumerate().find_map(|(l, m)| m.agents.map(|a| (l, m, a))) {
            Some((l, model, a)) => (a.count)(
                &model.params_view(cfg.layer_model_params(l)),
                self.grid_w,
                self.grid_h,
            )
            .clamp(1, MAX_AGENTS),
            None => 0,
        };
        if want == self.agent_capacity {
            return;
        }
        self.agent_buffer = (want > 0).then(|| {
            device.create_buffer(&BufferDescriptor {
                label: Some("Sim Agents"),
                size: (want as u64) * 16,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        });
        self.agent_capacity = want;
        self.agent_bind_groups = None;
        self.needs_seed = true;
    }

    /// The agent population, for a test to read back.
    pub fn agent_buffer(&self) -> Option<&Buffer> {
        self.agent_buffer.as_ref()
    }

    /// The per-cell claim buffer, for a test to check it is empty
    /// between steps (see `agent_claim`'s contract in the assembler).
    pub fn claim_buffer(&self) -> &Buffer {
        &self.claim_buffer
    }

    /// Allocate the pyramid when the model reads one, free it when the
    /// model does not. Called wherever `ensure_pipelines` is, so a
    /// model or grid change is caught before the next dispatch.
    fn ensure_pyramid(&mut self, device: &Device, cfg: &SimConfig) {
        let wants = layer_models(cfg).iter().any(|m| m.has(ModelFeature::NeedsPyramid));
        let expected = if wants { pyramid_levels(self.grid_w, self.grid_h) as usize - 1 } else { 0 };
        if self.pyramid.len() == expected {
            return;
        }
        self.pyramid = if wants {
            Self::create_pyramid(device, self.grid_w, self.grid_h)
        } else {
            Vec::new()
        };
        // The step groups bind the levels at 6..12, so they go too.
        self.step_bind_groups = None;
        self.pyramid_bind_groups = None;
    }

    /// Whether this frame builds a distance field: the matte's edge
    /// asked for one, or the colouring reads one. Either way the matte
    /// must be on -- it is what says which cells are the figure.
    /// A config with a different number of layers needs field arrays
    /// with that many slices; the state cannot survive, so it reseeds.
    fn ensure_layers(&mut self, device: &Device, cfg: &SimConfig) {
        let want = cfg.layer_count() as u32;
        if want == self.layers {
            return;
        }
        let (f, fv) = Self::create_field_pair(device, self.grid_w, self.grid_h, want);
        self.field = f;
        self.field_view = fv;
        self.layers = want;
        self.current = 0;
        self.step_bind_groups = None;
        self.pyramid_bind_groups = None;
        self.reduce_bind_groups = None;
        self.agent_bind_groups = None;
        self.needs_seed = true;
    }

    fn wants_sdf(cfg: &SimConfig) -> bool {
        if !cfg.color_layers.is_empty() {
            return Self::sdf_layer(cfg).is_some();
        }
        !cfg.matte.is_off()
            && (cfg.matte.uses_distance()
                || coloring_or_default(&cfg.coloring).has(ColoringFeature::NeedsDistance))
    }

    /// Which colour layer this frame's distance field belongs to: the
    /// FIRST whose matte is on and either has a Distance edge or whose
    /// colouring reads the distance. One field per frame; the plan
    /// says so and the other layers read it as it is.
    fn sdf_layer(cfg: &SimConfig) -> Option<usize> {
        cfg.color_layers.iter().position(|l| {
            !l.matte.is_off()
                && (l.matte.uses_distance()
                    || coloring_or_default(&l.coloring).has(ColoringFeature::NeedsDistance))
        })
    }

    /// The matte and source the jump flood seeds from: the single
    /// colouring's, or the stack's distance layer's.
    fn sdf_matte(cfg: &SimConfig) -> (crate::config::sim::SimMatte, usize) {
        match Self::sdf_layer(cfg).and_then(|k| cfg.color_layers.get(k)) {
            Some(l) => (l.matte, l.source.min(cfg.layer_count().saturating_sub(1))),
            None => (cfg.matte, 0),
        }
    }

    /// Allocate the jump flood's textures when the matte asks for a
    /// distance field, free them when it stops asking.
    fn ensure_sdf(&mut self, device: &Device, wants: bool) {
        if wants == self.sdf.is_some() {
            return;
        }
        if wants {
            let (w, h) = (self.grid_w, self.grid_h);
            self.jfa = Some([
                Self::create_level(device, w, h, "Sim JFA A"),
                Self::create_level(device, w, h, "Sim JFA B"),
            ]);
            self.sdf = Some(Self::create_level(device, w, h, "Sim SDF"));
        } else {
            self.jfa = None;
            self.sdf = None;
        }
    }

    /// The signed distance field, for a test to read back.
    pub fn sdf_texture(&self) -> Option<&Texture> {
        self.sdf.as_ref().map(|(t, _)| t)
    }

    /// The jump flood over the live field, into `self.sdf`: seed, then
    /// jumps of N/2 down to 1 and one more at 1, then seeds to
    /// distance. Each pass reads its own uniform slot, whose
    /// kernel-radius word is the jump.
    fn encode_jump_flood(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        enc: &mut CommandEncoder,
    ) {
        let n = self.grid_w.max(self.grid_h).next_power_of_two();
        let mut jumps: Vec<u32> = Vec::new();
        let mut k = (n / 2).max(1);
        loop {
            jumps.push(k);
            if k == 1 {
                break;
            }
            k /= 2;
        }
        jumps.push(1);
        assert!(jumps.len() + 1 <= JFA_SLOTS, "grid too large for the jump-flood slots");

        // Slot 0 seeds; slot 1 + i is jump i.
        let stride = self.params_stride as usize;
        let mut bytes = vec![0u8; stride * (jumps.len() + 1)];
        for (i, slot) in std::iter::once(0u32).chain(jumps.iter().copied()).enumerate() {
            let (matte, source) = Self::sdf_matte(cfg);
            let mut p = self.params_for_layer(cfg, self.step_index, source);
            p.matte = matte.packed();
            p.matte_b[0] = if matte.uses_distance() { 1.0 } else { 0.0 };
            p.kernel_radius = slot;
            let at = i * stride;
            bytes[at..at + std::mem::size_of::<SimParamsGpu>()]
                .copy_from_slice(bytemuck::bytes_of(&p));
        }
        queue.write_buffer(&self.jfa_params_buffer, 0, &bytes);

        let p = self.pipelines.as_ref().expect("pipelines built above");
        let jfa = self.jfa.as_ref().expect("ensure_sdf allocated it");
        let sdf = self.sdf.as_ref().expect("ensure_sdf allocated it");
        let group = |out: &TextureView, ping: &TextureView| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Sim JFA BG"),
                layout: &p.jfa_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.jfa_params_buffer,
                            offset: 0,
                            size: std::num::NonZeroU64::new(
                                std::mem::size_of::<SimParamsGpu>() as u64,
                            ),
                        }),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: self.model_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: self.coloring_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry { binding: 3, resource: BindingResource::TextureView(out) },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::TextureView(&self.field_view[self.current]),
                    },
                    BindGroupEntry { binding: 5, resource: BindingResource::TextureView(ping) },
                ],
            })
        };
        let (gx, gy) = Self::dispatch_size(self.grid_w, self.grid_h);
        let stride32 = self.params_stride as u32;
        let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Sim Jump Flood"),
            timestamp_writes: None,
        });
        // Seed into A. The ping slot is unused here; bind B.
        let init_bg = group(&jfa[0].1, &jfa[1].1);
        pass.set_pipeline(&p.jfa_init);
        pass.set_bind_group(0, &init_bg, &[0]);
        pass.dispatch_workgroups(gx, gy, 1);
        // Jump A -> B -> A ...
        let mut src = 0usize;
        let mut groups = Vec::with_capacity(jumps.len());
        for i in 0..jumps.len() {
            groups.push(group(&jfa[1 - src].1, &jfa[src].1));
            pass.set_pipeline(&p.jfa_step);
            pass.set_bind_group(0, &groups[i], &[(i as u32 + 1) * stride32]);
            pass.dispatch_workgroups(gx, gy, 1);
            src = 1 - src;
        }
        // Seeds to distance, from whichever holds the last jump.
        let final_bg = group(&sdf.1, &jfa[src].1);
        pass.set_pipeline(&p.jfa_final);
        pass.set_bind_group(0, &final_bg, &[0]);
        pass.dispatch_workgroups(gx, gy, 1);
    }

    /// Level `l` of the pyramid (1..), for a test to read back.
    pub fn pyramid_texture(&self, level: usize) -> Option<&Texture> {
        self.pyramid.get(level.checked_sub(1)?).map(|(t, _, _)| t)
    }

    fn create_field_pair(device: &Device, w: u32, h: u32, layers: u32) -> ([Texture; 2], [TextureView; 2]) {
        let desc = TextureDescriptor {
            label: Some("Sim Field"),
            // One slice per layer (simulation-layers plan, section 2);
            // a single system is an array of one.
            size: Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: layers.max(1),
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            // Both usages on both textures: they swap roles every step.
            // COPY_SRC so a test can read the field back and compare
            // against a CPU mirror of the rule.
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC
                // COPY_DST too: a test writes an analytic field straight
                // in to measure the resolve against it, and the
                // bound-grid resize resampler (plan section 7) will
                // copy between pairs. Costs nothing on the render path.
                | TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let a = device.create_texture(&desc);
        let b = device.create_texture(&desc);
        let va = a.create_view(&Self::array_view());
        let vb = b.create_view(&Self::array_view());
        ([a, b], [va, vb])
    }

    /// A view that binds a texture as a 2D array, which is how every
    /// pass declares the field -- and how the pyramid pass declares a
    /// level, whose texture has one layer.
    fn array_view() -> TextureViewDescriptor<'static> {
        TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        }
    }

    fn create_output(device: &Device, w: u32, h: u32) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Sim Output"),
            size: Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    /// The rendered image, in the flame-accumulator format the tonemap
    /// pass expects.
    pub fn output_view(&self) -> &TextureView {
        &self.output_view
    }

    pub fn output_texture(&self) -> &Texture {
        &self.output_texture
    }

    /// The texture currently holding live state. Test-only: reading the
    /// field back is how the CPU-mirror and determinism tests check the
    /// rule rather than the picture.
    #[cfg(test)]
    pub(crate) fn field_texture(&self) -> &Texture {
        &self.field[self.current]
    }

    pub fn step_index(&self) -> u32 {
        self.step_index
    }

    /// Whether the next frame will restart the run before stepping.
    /// The caller needs this to know that a step index it is looking
    /// at is about to become 0.
    ///
    /// True either because something asked for a reseed, or because
    /// the config no longer describes the field that is loaded --
    /// see `SeedIdentity`.
    pub fn will_reseed(&self, cfg: &SimConfig) -> bool {
        self.needs_seed || self.seeded_as.as_ref() != Some(&SeedIdentity::of(cfg))
    }

    /// How many steps are left before `cfg.steps`, or `None` when
    /// nothing is holding the run back.
    ///
    /// `None` covers two cases deliberately: an uncapped run
    /// (`steps == 0`), and a run that is already AT or past the cap.
    /// The second is what lets Run resume after the auto-pause —
    /// the limit is a place the run stops once, not a wall it can
    /// never cross.
    pub fn steps_remaining(&self, cfg: &SimConfig) -> Option<u32> {
        if cfg.steps == 0 || self.step_index >= cfg.steps {
            None
        } else {
            Some(cfg.steps - self.step_index)
        }
    }

    /// How many steps fit in `budget_ms`, from the measured cost of a
    /// step.
    ///
    /// The interactive driver's answer to "the timeline wants 2,000
    /// steps and this display frame has 8 ms": take what fits, return,
    /// and come back next frame. Blocking until the target is reached
    /// would freeze the UI for as long as the jump takes -- and a
    /// scrub can ask for one on every slider event.
    ///
    /// Before any batch has been timed this returns [`FIRST_SUBMIT`],
    /// the same blind, conservative first step count `run_steps` uses:
    /// enough to get a measurement, small enough that an expensive
    /// model's first frame is not a stall. Never zero, so a caller
    /// looping on it cannot spin.
    pub fn steps_in(&self, budget_ms: f64) -> u32 {
        if self.ms_per_step <= 0.0 {
            return FIRST_SUBMIT;
        }
        ((budget_ms / self.ms_per_step).floor()).clamp(1.0, u32::MAX as f64) as u32
    }

    pub fn grid_size(&self) -> (u32, u32) {
        (self.grid_w, self.grid_h)
    }

    /// Mark the field stale so the next render reseeds it.
    pub fn request_seed(&mut self) {
        self.needs_seed = true;
    }

    /// Resize the OUTPUT, and the grid with it when the grid is bound
    /// to the viewport. Returns whether anything was recreated.
    ///
    /// A bound grid changing size discards the run — the field cannot
    /// be carried across a resolution change without interpolating
    /// state, which is a phase-3 concern (`resample_into`). Saying so
    /// by reseeding is honest; silently continuing on a stretched
    /// field would not be.
    pub fn resize(&mut self, device: &Device, cfg: &SimConfig, out_w: u32, out_h: u32) -> bool {
        let (gw, gh) = Self::allocatable_grid(cfg, out_w, out_h);
        let out_changed = out_w != self.out_w || out_h != self.out_h;
        let grid_changed = gw != self.grid_w || gh != self.grid_h;
        if !out_changed && !grid_changed {
            return false;
        }
        if out_changed {
            let (t, v) = Self::create_output(device, out_w, out_h);
            self.output_texture = t;
            self.output_view = v;
            self.out_w = out_w;
            self.out_h = out_h;
        }
        if grid_changed {
            let (f, fv) = Self::create_field_pair(device, gw, gh, self.layers);
            self.field = f;
            self.field_view = fv;
            // Re-created lazily at the new size, if the model reads it.
            self.pyramid.clear();
            self.jfa = None;
            self.sdf = None;
            let (d, c) = Self::create_cell_buffers(device, gw, gh);
            self.deposit_buffer = d;
            self.claim_buffer = c;
            self.agent_bind_groups = None;
            self.grid_w = gw;
            self.grid_h = gh;
            self.current = 0;
            self.needs_seed = true;
        }
        // Do NOT clear `pipelines` here. The resolve direction is part
        // of the pipeline key, so `ensure_pipelines` already rebuilds
        // exactly when a resize flips it -- clearing unconditionally
        // recompiled all three shaders on EVERY resize event, which is
        // once per frame during a window drag. The step bind groups do
        // reference the field views, so they go with the field.
        if grid_changed {
            self.step_bind_groups = None;
            self.pyramid_bind_groups = None;
            self.reduce_bind_groups = None;
            self.steps_per_submit = FIRST_SUBMIT;
            // The measured step cost describes the OLD kernel or grid; the
            // interactive budget must re-measure with the new one.
            self.ms_per_step = 0.0;
        }
        true
    }

    fn pipeline_key(&self, cfg: &SimConfig) -> PipelineKey {
        PipelineKey {
            layers: layer_models(cfg).iter().map(|m| m.name).collect(),
            coupled: !cfg.couplings.is_empty(),
            layer_map: if cfg.use_transforms {
                self.layer_map.as_ref().map(|m| m.key.hash64()).unwrap_or(0)
            } else {
                0
            },
            coloring: coloring_or_default(&cfg.coloring).name,
            stack: cfg.color_layers.iter().map(|l| coloring_or_default(&l.coloring).name).collect(),
            boundary: cfg.boundary,
            upscale: cfg.upscale,
            downscale: cfg.downscale,
            init_kind: cfg.init.kind_name(),
            magnifying: self.magnifying(),
        }
    }

    /// Whether the output is larger than the grid, which decides which
    /// resolve filter the colour pass compiles.
    fn magnifying(&self) -> bool {
        self.out_w >= self.grid_w && self.out_h >= self.grid_h
    }

    fn ensure_pipelines(&mut self, device: &Device, cfg: &SimConfig) {
        let key = self.pipeline_key(cfg);
        if self.pipelines.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let models = layer_models(cfg);
        let model = models[0];
        let coloring = coloring_or_default(&cfg.coloring);

        // The seed is layer 0's model's -- every layer of a layered
        // config is seeded by its own model below, through one shader
        // per distinct model.
        let seed_srcs: Vec<(&'static str, String)> = {
            let mut v: Vec<(&'static str, String)> = Vec::new();
            for m in &models {
                if !v.iter().any(|(n, _)| *n == m.name) {
                    v.push((m.name, assembler::assemble_seed(m, cfg.init.kind_name())));
                }
            }
            v
        };
        let _ = model;
        // Step shaders per distinct model; a layer looks its model up.
        let step_srcs: Vec<(&'static str, Vec<String>)> = {
            let mut v: Vec<(&'static str, Vec<String>)> = Vec::new();
            for m in &models {
                if !v.iter().any(|(n, _)| *n == m.name) {
                    v.push((
                        m.name,
                        (0..m.passes)
                            .map(|pass| assembler::assemble_step_coupled(m, cfg.boundary, pass, key.coupled))
                            .collect(),
                    ));
                }
            }
            v
        };
        let any_pyramid = models.iter().any(|m| m.has(ModelFeature::NeedsPyramid));
        let any_minmax = models.iter().any(|m| m.has(ModelFeature::NeedsMinMax));
        let agent_layer = models.iter().position(|m| m.agents.is_some());
        let agent_model = agent_layer.map(|l| models[l]);
        let warp_src = assembler::assemble_warp(cfg.boundary);
        let color_src = if cfg.color_layers.is_empty() {
            assembler::assemble_color(coloring, cfg.boundary, cfg.upscale, cfg.downscale, key.magnifying)
        } else {
            let stack: Vec<&'static SimColoringDef> = cfg
                .color_layers
                .iter()
                .take(crate::config::sim::MAX_COLOR_LAYERS)
                .map(|l| coloring_or_default(&l.coloring))
                .collect();
            assembler::assemble_color_stack(&stack, cfg.boundary, cfg.upscale, cfg.downscale, key.magnifying)
        };
        let pyramid_src = any_pyramid.then(|| assembler::assemble_pyramid(cfg.boundary));
        let reduce_src = any_minmax.then(assembler::assemble_reduce);
        let agent_srcs: Vec<String> = match agent_model.and_then(|m| m.agents.map(|a| (m, a))) {
            Some((m, a)) => (0..a.passes)
                .map(|p| assembler::assemble_agents(m, cfg.boundary, p))
                .collect(),
            None => Vec::new(),
        };
        let agent_seed_src = agent_model.map(|m| assembler::assemble_agent_seed(m, cfg.boundary));

        let make = |label: &str, src: &str| {
            device.create_shader_module(ShaderModuleDescriptor {
                label: Some(label),
                source: ShaderSource::Wgsl(src.into()),
            })
        };
        let seed_mods: Vec<(&'static str, ShaderModule)> =
            seed_srcs.iter().map(|(n, src)| (*n, make("Sim Seed", src))).collect();
        let step_mods: Vec<(&'static str, Vec<ShaderModule>)> = step_srcs
            .iter()
            .map(|(n, srcs)| (*n, srcs.iter().map(|src| make("Sim Step", src)).collect()))
            .collect();
        let warp_mod = make("Sim Warp", &warp_src);
        let color_mod = make("Sim Color", &color_src);
        let pyramid_mod = pyramid_src.as_ref().map(|src| make("Sim Pyramid", src));
        let reduce_mod = reduce_src.as_ref().map(|src| make("Sim Reduce", src));
        let jfa_init_mod = make("Sim JFA Init", &assembler::assemble_jfa_init());
        let jfa_step_mod = make("Sim JFA Step", &assembler::assemble_jfa_step());
        let jfa_final_mod = make("Sim JFA Final", &assembler::assemble_jfa_final());
        let agent_mods: Vec<ShaderModule> =
            agent_srcs.iter().map(|src| make("Sim Agents", src)).collect();
        let agent_seed_mod = agent_seed_src.as_ref().map(|src| make("Sim Agent Seed", src));

        // Bind group layouts, written out rather than derived from the
        // shader: `layout: None` would infer a fresh layout per module
        // and the bind groups could not be shared between passes.
        let uniform_entry = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                // Dynamic: the step loop selects its ring slot by
                // offset rather than rewriting the buffer.
                has_dynamic_offset: true,
                min_binding_size: None,
            },
            count: None,
        };
        let storage_ro = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage_rw = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        // The FIELD is a texture array, one slice per layer; the other
        // textures (pyramid levels, jump flood, distance, output,
        // palette) are plain 2D. Binding 4 is the field in every pass;
        // binding 3 is the field in seed, step and warp and a 2D
        // target elsewhere -- the pyramid writes its level through a
        // one-layer array view, since it shares the step layout.
        let storage_tex_dim = |binding: u32, dim: TextureViewDimension| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: TextureFormat::Rgba32Float,
                view_dimension: dim,
            },
            count: None,
        };
        let storage_tex = |binding: u32| storage_tex_dim(binding, TextureViewDimension::D2);
        let storage_tex_array = |binding: u32| storage_tex_dim(binding, TextureViewDimension::D2Array);
        let sampled_tex_dim = |binding: u32, float32: bool, dim: TextureViewDimension| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Texture {
                // Non-filterable: FLOAT32_FILTERABLE is an optional
                // feature, and every read here is a textureLoad anyway.
                sample_type: TextureSampleType::Float { filterable: !float32 },
                view_dimension: dim,
                multisampled: false,
            },
            count: None,
        };
        let sampled_tex = |binding: u32, float32: bool| sampled_tex_dim(binding, float32, TextureViewDimension::D2);
        let sampled_field = |binding: u32| sampled_tex_dim(binding, true, TextureViewDimension::D2Array);

        let seed_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Seed Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                storage_tex_array(3),
            ],
        });
        let step_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Step Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                storage_tex_array(3),
                sampled_field(4),
                // The convolution table. Always bound, declared in the
                // WGSL only by the models that gather against it -- a
                // layout may carry an entry the shader does not use.
                storage_ro(5),
                // Pyramid levels 1..7, and the min/max ring. Likewise
                // always bound (a 1x1 dummy where unused) and declared
                // only by the models that read them.
                sampled_tex(6, true),
                sampled_tex(7, true),
                sampled_tex(8, true),
                sampled_tex(9, true),
                sampled_tex(10, true),
                sampled_tex(11, true),
                sampled_tex(12, true),
                storage_ro(14),
                // The agents' deposit: read and cleared by the step.
                storage_rw(13),
                // The coupling table, declared only by a coupled
                // config's step shaders.
                storage_ro(17),
            ],
        });
        let agent_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Agent Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                sampled_field(4),
                storage_rw(13),
                storage_rw(15),
                storage_rw(16),
                // The min/max ring, for an agent that needs the
                // field's global range (DLA's launch radius).
                storage_ro(14),
            ],
        });
        let reduce_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Reduce Layout"),
            entries: &[
                uniform_entry(0),
                sampled_field(4),
                BindGroupLayoutEntry {
                    binding: 14,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let color_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Color Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                storage_tex(3),
                sampled_field(4),
                sampled_tex(5, false),
                // The distance field, or its dummy.
                sampled_tex(6, true),
                // The colour stack's layer records.
                storage_ro(7),
            ],
        });
        // The jump flood: the shared uniform and param buffers (its
        // shaders carry the common header), one storage target, the
        // field for the seed pass, and the ping texture for the rest.
        let jfa_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim JFA Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                storage_tex(3),
                sampled_field(4),
                sampled_tex(5, true),
            ],
        });

        let pipeline = |label: &str, layout: &BindGroupLayout, module: &ShaderModule| {
            let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };

        // New layouts and modules: any cached bind group refers to the
        // old ones.
        self.step_bind_groups = None;
        self.pyramid_bind_groups = None;
        self.reduce_bind_groups = None;
        self.agent_bind_groups = None;
        self.steps_per_submit = FIRST_SUBMIT;
        // The measured step cost describes the OLD kernel or grid; the
        // interactive budget must re-measure with the new one.
        self.ms_per_step = 0.0;
        let seed_pipelines: Vec<(&'static str, ComputePipeline)> = seed_mods
            .iter()
            .map(|(n, m)| (*n, pipeline("Sim Seed", &seed_layout, m)))
            .collect();
        let step_pipelines: Vec<(&'static str, Vec<ComputePipeline>)> = step_mods
            .iter()
            .map(|(n, ms)| (*n, ms.iter().map(|m| pipeline("Sim Step", &step_layout, m)).collect()))
            .collect();
        let lookup_steps = |name: &str| -> Vec<ComputePipeline> {
            step_pipelines.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).unwrap_or_default()
        };
        // The layer map: the flame's definitions in the warp template,
        // with the flame's bindings at group 1.
        let (layer_warp, flame_layout) = match (&self.layer_map, key.layer_map != 0) {
            (Some(map), true) => {
                let src = assembler::assemble_layer_warp(cfg.boundary, &map.defs);
                let module = make("Sim Layer Warp", &src);
                let flame_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: Some("Sim Flame Layout"),
                    entries: &[
                        storage_ro(0),
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
                        storage_ro(5),
                        storage_ro(10),
                        storage_ro(12),
                    ],
                });
                let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Sim Layer Warp"),
                    bind_group_layouts: &[Some(&step_layout), Some(&flame_layout)],
                    immediate_size: 0,
                });
                let pipe = device.create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some("Sim Layer Warp"),
                    layout: Some(&pl),
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
                (Some(pipe), Some(flame_layout))
            }
            _ => (None, None),
        };
        if let Some(map) = self.layer_map.as_mut() {
            map.bind_group = None;
        }
        self.pipelines = Some(Pipelines {
            layer_warp,
            flame_layout,
            seed: seed_pipelines[0].1.clone(),
            layer_seeds: models.iter().map(|m| {
                seed_pipelines.iter().find(|(n, _)| *n == m.name).map(|(_, p)| p.clone()).expect("built above")
            }).collect(),
            layer_steps: models.iter().map(|m| lookup_steps(m.name)).collect(),
            agent_layer,
            // Same layout as a step: it reads binding 4 and writes 3,
            // and ignores the rest.
            warp: pipeline("Sim Warp", &step_layout, &warp_mod),
            color: pipeline("Sim Color", &color_layout, &color_mod),
            jfa_init: pipeline("Sim JFA Init", &jfa_layout, &jfa_init_mod),
            jfa_step: pipeline("Sim JFA Step", &jfa_layout, &jfa_step_mod),
            jfa_final: pipeline("Sim JFA Final", &jfa_layout, &jfa_final_mod),
            jfa_layout,
            pyramid: pyramid_mod
                .as_ref()
                .map(|m| pipeline("Sim Pyramid", &step_layout, m)),
            reduce: reduce_mod
                .as_ref()
                .map(|m| pipeline("Sim Reduce", &reduce_layout, m)),
            agents: agent_mods
                .iter()
                .map(|m| pipeline("Sim Agents", &agent_layout, m))
                .collect(),
            agent_seed: agent_seed_mod
                .as_ref()
                .map(|m| pipeline("Sim Agent Seed", &agent_layout, m)),
            agent_layout,
            seed_layout,
            step_layout,
            reduce_layout,
            color_layout,
            key,
        });
    }

    /// The uniform for one step index, for layer 0.
    fn params_for(&self, cfg: &SimConfig, step_index: u32) -> SimParamsGpu {
        self.params_for_layer(cfg, step_index, 0)
    }

    /// The uniform for one step index and one layer.
    fn params_for_layer(&self, cfg: &SimConfig, step_index: u32, layer: usize) -> SimParamsGpu {
        let layers = cfg.layer_count() as u32;
        let (p0, p1) = match cfg.init {
            crate::config::sim::SimInit::Noise { amplitude } => (amplitude, 0.0),
            crate::config::sim::SimInit::Blob { radius } => (radius as f32, 0.0),
            crate::config::sim::SimInit::Blobs { count, radius } => {
                (count.min(64) as f32, radius as f32)
            }
            crate::config::sim::SimInit::Ring { radius } => (radius as f32, 0.0),
            crate::config::sim::SimInit::Line
            | crate::config::sim::SimInit::Center
            | crate::config::sim::SimInit::BrokenWave => (0.0, 0.0),
        };
        SimParamsGpu {
            grid: [self.grid_w, self.grid_h],
            out_size: [self.out_w, self.out_h],
            step_index,
            seed_lo: cfg.seed as u32,
            seed_hi: (cfg.seed >> 32) as u32,
            // Capped at the model's stability bound here as well as at
            // the config manager: a hand-edited file can carry any value,
            // and past the bound the [0,1] clamp turns divergence into
            // plausible-looking garbage rather than NaN. The bound
            // depends on the diffusion rates in force, not just the
            // model, which is why it is computed from the params.
            dt: {
                // One dt for every layer: the tightest cap wins.
                let max_dt = (0..cfg.layer_count())
                    .map(|l| model_or_default(cfg.layer_model_name(l)).max_dt_for(cfg.layer_model_params(l)))
                    .fold(f32::MAX, f32::min);
                if cfg.dt.is_finite() { cfg.dt.clamp(1e-4, max_dt) } else { 1.0 }
            },
            init_p0: p0,
            init_p1: p1,
            kernel_radius: self.kernel_radii.get(layer).copied().unwrap_or(0),
            minmax_slot: (step_index * layers + layer as u32) % MINMAX_RING,
            layer: layer as u32,
            kernel_offset: self.kernel_offsets.get(layer).copied().unwrap_or(0),
            minmax_back: layers,
            coupling_count: cfg.couplings.len().min(crate::config::sim::MAX_COUPLINGS) as u32,
            warp_a: match cfg.warp.mode {
                crate::config::sim::SimWarpMode::Continuous => {
                    [cfg.warp.zoom, cfg.warp.rotation, cfg.warp.pan_x, cfg.warp.pan_y]
                }
                // Zoom only, and only on a doubling step; the warp is
                // not dispatched on the others, so the value there is
                // moot.
                crate::config::sim::SimWarpMode::Octaves => {
                    [cfg.warp.octave(step_index).warp.unwrap_or(1.0), 0.0, 0.0, 0.0]
                }
            },
            warp_b: [
                match cfg.warp.mode {
                    crate::config::sim::SimWarpMode::Continuous => cfg.warp.flow,
                    crate::config::sim::SimWarpMode::Octaves => 0.0,
                },
                match cfg.warp.filter {
                    crate::config::sim::SimWarpFilter::Bilinear => 0.0,
                    crate::config::sim::SimWarpFilter::Nearest => 1.0,
                    crate::config::sim::SimWarpFilter::Bicubic => 2.0,
                },
            ],
            matte_b: [
                if cfg.matte.uses_distance() { 1.0 } else { 0.0 },
                if Self::wants_sdf(cfg) { 1.0 } else { 0.0 },
            ],
            matte: cfg.matte.packed(),
            view: [
                match cfg.warp.mode {
                    crate::config::sim::SimWarpMode::Continuous => 1.0,
                    crate::config::sim::SimWarpMode::Octaves => cfg.warp.octave(step_index).view,
                },
                match cfg.fit {
                    crate::config::sim::SimFit::Letterbox => 0.0,
                    crate::config::sim::SimFit::Cover => 1.0,
                },
                if cfg.warp.cull && cfg.warp.mode == crate::config::sim::SimWarpMode::Octaves {
                    1.0
                } else {
                    0.0
                },
                // The halo: the kernel's reach plus a pattern's worth
                // of cells, so the frozen ring's staleness cannot
                // reach the window within an octave.
                (self.kernel_radii.get(layer).copied().unwrap_or(0) + 24) as f32,
            ],
            warp_mask: {
                let m = match cfg.warp.mode {
                    crate::config::sim::SimWarpMode::Continuous => cfg.warp.layers,
                    crate::config::sim::SimWarpMode::Octaves => 15,
                };
                [0, 1, 2, 3].map(|b| if m & (1 << b) != 0 { 1.0 } else { 0.0 })
            },
            xform: [self.layer_rate(cfg, layer), 0.0, 0.0, 0.0],
        }
    }

    /// The layer map's rate for a layer: its transform's weight, when
    /// the config uses transforms and a map is set; 0 otherwise.
    fn layer_rate(&self, cfg: &SimConfig, layer: usize) -> f32 {
        if !cfg.use_transforms {
            return 0.0;
        }
        self.layer_map
            .as_ref()
            .and_then(|m| m.rates.get(layer).copied())
            .unwrap_or(0.0)
    }

    /// Give the renderer the flame whose transforms are the layers'
    /// maps (simulation-layers plan, section 4). Called by whoever
    /// drives a frame -- the app, the headless renderer, the animation
    /// export -- before the step. Cheap when nothing changed: the
    /// definitions rebuild only when the flame's active variation set,
    /// transform count or flags change; the buffers are rewritten each
    /// call, and they are small (the normal transforms only).
    pub fn set_layer_transforms(&mut self, device: &Device, queue: &Queue, flame: &crate::scene::transforms::Flame) {
        use crate::gpu::buffers::{pack_gpu_transforms, pack_gpu_variation_params};
        let key = LayerMapKey::of(flame);
        let n = flame.transforms.len().max(1);
        let transforms = pack_gpu_transforms(flame, crate::scene::transforms::RenderMode::TwoD);
        let vparams = pack_gpu_variation_params(flame);
        let t_bytes: &[u8] = bytemuck::cast_slice(&transforms[..n.min(transforms.len())]);
        let v_bytes: &[u8] = bytemuck::cast_slice(&vparams[..n.min(vparams.len())]);
        let rates: Vec<f32> = flame.transforms.iter().map(|t| t.weight.clamp(0.0, 1.0)).collect();
        let rebuild = match &self.layer_map {
            Some(m) => m.key != key,
            None => true,
        };
        if rebuild {
            let builder = crate::shader_builder_v2::ShaderBuilder::new(crate::variations::global_registry().clone());
            let defs = builder.build_layer_map(flame);
            let make = |label: &str, bytes: &[u8]| {
                device.create_buffer_init(&util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytes,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                })
            };
            let flame_params = device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some("Sim Flame Params"),
                contents: bytemuck::bytes_of(&<crate::gpu::buffers::GpuParams as bytemuck::Zeroable>::zeroed()),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });
            let attachments = vec![
                <crate::gpu::buffers::GpuAttachmentList as bytemuck::Zeroable>::zeroed();
                crate::gpu::buffers::MAX_TRANSFORMS
            ];
            let metas = crate::gpu::buffers::build_subflame_metas(&[]).unwrap_or_else(|_| {
                [<crate::gpu::buffers::SubflameMeta as bytemuck::Zeroable>::zeroed(); crate::gpu::buffers::MAX_SUBFLAMES]
            });
            self.layer_map = Some(LayerMap {
                key,
                defs,
                transforms: make("Sim Flame Transforms", t_bytes),
                variation_params: make("Sim Flame Variation Params", v_bytes),
                flame_params,
                attachments: make("Sim Flame Attachments", bytemuck::cast_slice(&attachments)),
                subflame_meta: make("Sim Flame Subflame Meta", bytemuck::cast_slice(&metas)),
                rates,
                bind_group: None,
            });
            // The pipeline key carries the map; the next ensure rebuilds.
            return;
        }
        let map = self.layer_map.as_mut().expect("set above");
        map.rates = rates;
        // Same shape (the key matched), so the buffers hold: rewrite.
        if (t_bytes.len() as u64) <= map.transforms.size() && (v_bytes.len() as u64) <= map.variation_params.size() {
            queue.write_buffer(&map.transforms, 0, t_bytes);
            queue.write_buffer(&map.variation_params, 0, v_bytes);
        }
    }

    /// The group-1 bind group for the layer warp, built once per
    /// pipeline set.
    fn ensure_flame_bind_group(&mut self, device: &Device) {
        let Some(p) = self.pipelines.as_ref() else { return };
        let Some(layout) = p.flame_layout.as_ref() else { return };
        let Some(map) = self.layer_map.as_mut() else { return };
        if map.bind_group.is_some() {
            return;
        }
        map.bind_group = Some(device.create_bind_group(&BindGroupDescriptor {
            label: Some("Sim Flame BG"),
            layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: map.transforms.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: map.flame_params.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: map.variation_params.as_entire_binding() },
                BindGroupEntry { binding: 10, resource: map.attachments.as_entire_binding() },
                BindGroupEntry { binding: 12, resource: map.subflame_meta.as_entire_binding() },
            ],
        }));
    }

    /// Write ring slot 0, for the passes that run once (seed, colour).
    fn write_params_slot0(&self, queue: &Queue, cfg: &SimConfig) {
        let p = self.params_for(cfg, self.step_index);
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&p));
    }

    /// Fill `count` ring slots for steps `start..start + count`, in ONE
    /// write. Each step then reads its own slot by dynamic offset,
    /// which is the only way a batched submission can see per-step
    /// values at all -- `write_buffer` is staged before the command
    /// buffer runs, so rewriting one uniform per step gives every step
    /// in the batch the last step's index.
    fn write_params_ring(&self, queue: &Queue, cfg: &SimConfig, start: u32, count: u32) {
        let stride = self.params_stride as usize;
        let layers = cfg.layer_count();
        let mut bytes = vec![0u8; stride * count as usize * layers * 2];
        for i in 0..count {
            for l in 0..layers {
                let p = self.params_for_layer(cfg, start + i, l);
                let at = self.ring_slot(i, l, 0, layers) as usize * stride;
                bytes[at..at + std::mem::size_of::<SimParamsGpu>()]
                    .copy_from_slice(bytemuck::bytes_of(&p));
                // The copy variant: the warp with nothing moved, which
                // carries this layer through a stage it has no pass for.
                let mut c = p;
                c.warp_a = [1.0, 0.0, 0.0, 0.0];
                c.warp_b = [0.0, 1.0];
                c.warp_mask = [0.0; 4];
                let at = self.ring_slot(i, l, VARIANT_COPY, layers) as usize * stride;
                bytes[at..at + std::mem::size_of::<SimParamsGpu>()]
                    .copy_from_slice(bytemuck::bytes_of(&c));
            }
        }
        queue.write_buffer(&self.params_buffer, 0, &bytes);
    }

    /// The ring slot of (step in batch, layer, variant).
    fn ring_slot(&self, i: u32, layer: usize, variant: u32, layers: usize) -> u32 {
        (i * layers as u32 + layer as u32) * 2 + variant
    }

    /// Build the two step bind groups once. They depend only on the
    /// pipeline layout and the field views, both of which are replaced
    /// together, so a rebuild is driven by `pipelines` being cleared.
    fn ensure_step_bind_groups(&mut self, device: &Device) {
        if self.step_bind_groups.is_some() {
            return;
        }
        let p = self.pipelines.as_ref().expect("pipelines built before bind groups");
        let make = |src: usize| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Sim Step BG"),
                layout: &p.step_layout,
                // groups[src] reads field[src] and writes field[1 - src].
                entries: &self.step_entries(
                    &self.params_buffer,
                    &self.field_view[1 - src],
                    &self.field_view[src],
                    true,
                ),
            })
        };
        self.step_bind_groups = Some([make(0), make(1)]);
    }

    fn write_param_arrays(&mut self, queue: &Queue, model: &ModelDef, coloring: &SimColoringDef, cfg: &SimConfig) {
        let _ = model;
        // One block of MODEL_PARAM_SLOTS per layer, padded so the
        // buffer never needs resizing; the shader indexes its own
        // layer's block.
        let layers = cfg.layer_count();
        let mut mp: Vec<f32> = Vec::with_capacity(MODEL_PARAM_SLOTS * MAX_LAYERS);
        for l in 0..layers {
            let m = model_or_default(cfg.layer_model_name(l));
            let mut block = m.pack_params_from(cfg.layer_model_params(l));
            assert!(
                block.len() <= MODEL_PARAM_SLOTS,
                "{} declares {} parameters; the buffer holds {MODEL_PARAM_SLOTS}",
                m.name,
                block.len()
            );
            block.resize(MODEL_PARAM_SLOTS, 0.0);
            mp.extend_from_slice(&block);
        }
        mp.resize(MODEL_PARAM_SLOTS * MAX_LAYERS, 0.0);
        // The colour stack: each layer's parameters in its own block
        // of 16, and its record. Without a stack, the single
        // colouring's block 0, as before.
        let mut cp: Vec<f32> = Vec::new();
        if cfg.color_layers.is_empty() {
            cp = coloring.pack_params(cfg);
            cp.resize(16, 0.0);
        } else {
            let sdf_of = Self::sdf_layer(cfg);
            let mut records: Vec<SimColorLayerGpu> = Vec::new();
            for (k, l) in cfg.color_layers.iter().take(crate::config::sim::MAX_COLOR_LAYERS).enumerate() {
                let c = coloring_or_default(&l.coloring);
                let mut block: Vec<f32> =
                    c.parameters.iter().map(|p| l.coloring_params.get(p.name).copied().filter(|v| v.is_finite()).unwrap_or(p.default)).collect();
                block.resize(16, 0.0);
                cp.extend_from_slice(&block);
                records.push(SimColorLayerGpu {
                    source: l.source.min(cfg.layer_count().saturating_sub(1)) as u32,
                    blend: l.blend.code(),
                    enabled: u32::from(l.enabled),
                    gather: u32::from(l.gather),
                    opacity: l.opacity.clamp(0.0, 1.0),
                    // Its matte's edge: distance only when this frame's
                    // distance field is this layer's.
                    edge: if l.matte.uses_distance() && sdf_of == Some(k) { 1.0 } else { 0.0 },
                    pad1: 0.0,
                    pad2: 0.0,
                    matte: l.matte.packed(),
                });
            }
            queue.write_buffer(&self.color_layers_buffer, 0, bytemuck::cast_slice(&records));
            cp.resize(16 * crate::config::sim::MAX_COLOR_LAYERS, 0.0);
        }
        queue.write_buffer(&self.model_params_buffer, 0, bytemuck::cast_slice(&mp));
        queue.write_buffer(&self.coloring_params_buffer, 0, bytemuck::cast_slice(&cp));

        // The convolution table, for the models whose rule is a
        // gather. Rebuilt unconditionally: it is a few thousand floats
        // and this runs once per batch, not per step, so tracking
        // staleness would cost more than it saves and could get it
        // wrong.
        // Every layer's table, one after another; each layer reads
        // its own through `kernel_offset`.
        let mut lut: Vec<f32> = Vec::new();
        self.kernel_radii.clear();
        self.kernel_offsets.clear();
        self.kernel_lens.clear();
        for l in 0..layers {
            let m = model_or_default(cfg.layer_model_name(l));
            self.kernel_offsets.push(lut.len() as u32);
            match m.kernel_for(cfg.layer_model_params(l)) {
                Some(k) => {
                    self.kernel_radii.push(k.radius);
                    self.kernel_lens.push(k.weights.len() as u32);
                    lut.extend_from_slice(&k.weights);
                }
                None => {
                    self.kernel_radii.push(0);
                    self.kernel_lens.push(0);
                }
            }
        }
        if !lut.is_empty() {
            queue.write_buffer(&self.kernel_buffer, 0, bytemuck::cast_slice(&lut));
        }

        // The coupling table, in config order, capped at the table.
        if !cfg.couplings.is_empty() {
            let table: Vec<SimCouplingGpu> = cfg
                .couplings
                .iter()
                .take(MAX_COUPLINGS)
                .map(|c| {
                    let from = c.from.min(layers.saturating_sub(1));
                    let publishes = model_or_default(cfg.layer_model_name(from)).has(ModelFeature::PublishesSignal);
                    SimCouplingGpu {
                        to: c.to.min(layers.saturating_sub(1)) as u32,
                        from: from as u32,
                        form: c.form.code(),
                        mask: c.channels & 15,
                        strength: if c.strength.is_finite() { c.strength } else { 0.0 },
                        k_offset: self.kernel_offsets.get(from).copied().unwrap_or(0),
                        k_radius: self.kernel_radii.get(from).copied().unwrap_or(0),
                        k_len: if publishes { 0 } else { self.kernel_lens.get(from).copied().unwrap_or(0) },
                        signal_channel: if publishes { 1 } else { 4 },
                        pad: [0; 3],
                    }
                })
                .collect();
            queue.write_buffer(&self.coupling_buffer, 0, bytemuck::cast_slice(&table));
        }

        // One uniform per pyramid level, carrying the SOURCE level's
        // size: the pyramid pass reads its input through the shared
        // boundary wrap, which sizes itself from `grid`.
        if layer_models(cfg).iter().any(|m| m.has(ModelFeature::NeedsPyramid)) {
            let stride = self.params_stride as usize;
            let levels = pyramid_levels(self.grid_w, self.grid_h) as usize;
            // Slots (layer, level): level 0 reads the layer's own slice
            // of the field, every level above reads a one-layer level
            // texture, so its slot says layer 0.
            let mut bytes = vec![0u8; stride * levels * layers];
            for layer in 0..layers {
            let (mut w, mut h) = (self.grid_w, self.grid_h);
            for l in 0..levels {
                let mut p = self.params_for_layer(cfg, self.step_index, if l == 0 { layer } else { 0 });
                p.grid = [w, h];
                let at = (layer * levels + l) * stride;
                bytes[at..at + std::mem::size_of::<SimParamsGpu>()]
                    .copy_from_slice(bytemuck::bytes_of(&p));
                w = w.div_ceil(2);
                h = h.div_ceil(2);
            }
            }
            queue.write_buffer(&self.level_params_buffer, 0, &bytes);
        }
    }

    /// Reset min/max ring slots `start..start + count` (mod the ring)
    /// to the ordering's identities, so the reduce passes that write
    /// them start from nothing. ONE write per batch; the slot a step
    /// reads is the one before the batch, which is never among these
    /// because the ring has one more slot than the largest batch.
    fn clear_minmax_slots(&self, queue: &Queue, start: u32, count: u32) {
        let identity = [u32::MAX, 0u32];
        let first = start % MINMAX_RING;
        let end = first + count;
        if end <= MINMAX_RING {
            let bytes: Vec<u32> = identity.repeat(count as usize);
            queue.write_buffer(&self.minmax_buffer, (first as u64) * 8, bytemuck::cast_slice(&bytes));
        } else {
            let head = MINMAX_RING - first;
            let a: Vec<u32> = identity.repeat(head as usize);
            let b: Vec<u32> = identity.repeat((count - head) as usize);
            queue.write_buffer(&self.minmax_buffer, (first as u64) * 8, bytemuck::cast_slice(&a));
            queue.write_buffer(&self.minmax_buffer, 0, bytemuck::cast_slice(&b));
        }
    }

    /// Build the per-level pyramid bind groups and the reduce groups
    /// once, alongside the step groups.
    fn ensure_stage_bind_groups(&mut self, device: &Device) {
        let p = self.pipelines.as_ref().expect("pipelines built before bind groups");
        if p.pyramid.is_some() && self.pyramid_bind_groups.is_none() {
            let mut groups: Vec<Vec<BindGroup>> = Vec::new();
            let levels = self.pyramid.len();
            for l in 0..levels {
                // Level l+1 is written from level l. Level 0 is the
                // field, so it has one group per ping-pong side.
                let sides: Vec<usize> = if l == 0 { vec![0, 1] } else { vec![0] };
                let mut per_side = Vec::new();
                for src in sides {
                    let input = if l == 0 { &self.field_view[src] } else { &self.pyramid[l - 1].2 };
                    let output = &self.pyramid[l].2;
                    per_side.push(device.create_bind_group(&BindGroupDescriptor {
                        label: Some("Sim Pyramid BG"),
                        layout: &p.step_layout,
                        entries: &self.step_entries(&self.level_params_buffer, output, input, false),
                    }));
                }
                groups.push(per_side);
            }
            self.pyramid_bind_groups = Some(groups);
        }
        if !p.agents.is_empty() && self.agent_bind_groups.is_none() {
            if let Some(buf) = self.agent_buffer.as_ref() {
                let make = |src: usize| device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Sim Agent BG"),
                    layout: &p.agent_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::Buffer(BufferBinding {
                                buffer: &self.params_buffer,
                                offset: 0,
                                size: std::num::NonZeroU64::new(
                                    std::mem::size_of::<SimParamsGpu>() as u64,
                                ),
                            }),
                        },
                        BindGroupEntry { binding: 1, resource: self.model_params_buffer.as_entire_binding() },
                        BindGroupEntry { binding: 2, resource: self.coloring_params_buffer.as_entire_binding() },
                        // Agents SENSE the live field, which is why
                        // there is one group per ping-pong side.
                        BindGroupEntry {
                            binding: 4,
                            resource: BindingResource::TextureView(&self.field_view[src]),
                        },
                        BindGroupEntry { binding: 13, resource: self.deposit_buffer.as_entire_binding() },
                        BindGroupEntry { binding: 15, resource: buf.as_entire_binding() },
                        BindGroupEntry { binding: 16, resource: self.claim_buffer.as_entire_binding() },
                        BindGroupEntry { binding: 14, resource: self.minmax_buffer.as_entire_binding() },
                    ],
                });
                self.agent_bind_groups = Some([make(0), make(1)]);
            }
        }
        if p.reduce.is_some() && self.reduce_bind_groups.is_none() {
            let make = |src: usize| {
                device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Sim Reduce BG"),
                    layout: &p.reduce_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::Buffer(BufferBinding {
                                buffer: &self.params_buffer,
                                offset: 0,
                                size: std::num::NonZeroU64::new(
                                    std::mem::size_of::<SimParamsGpu>() as u64,
                                ),
                            }),
                        },
                        BindGroupEntry {
                            binding: 4,
                            resource: BindingResource::TextureView(&self.field_view[src]),
                        },
                        BindGroupEntry { binding: 14, resource: self.minmax_buffer.as_entire_binding() },
                    ],
                })
            };
            self.reduce_bind_groups = Some([make(0), make(1)]);
        }
    }

    /// The full entry list of the step layout, for a given uniform
    /// buffer, output texture and input texture. Shared by the step
    /// groups and the pyramid groups, which use the same layout.
    ///
    /// `with_pyramid` binds the real pyramid levels at 6..12; the
    /// pyramid pass itself passes `false`, because a dispatch that
    /// WRITES level l cannot also have level l bound as a sampled
    /// texture -- wgpu rejects the two usages in one scope -- and it
    /// reads its input through binding 4 anyway.
    fn step_entries<'a>(
        &'a self,
        uniform: &'a Buffer,
        output: &'a TextureView,
        input: &'a TextureView,
        with_pyramid: bool,
    ) -> Vec<BindGroupEntry<'a>> {
        let mut entries = vec![
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: uniform,
                    offset: 0,
                    size: std::num::NonZeroU64::new(std::mem::size_of::<SimParamsGpu>() as u64),
                }),
            },
            BindGroupEntry { binding: 1, resource: self.model_params_buffer.as_entire_binding() },
            BindGroupEntry { binding: 2, resource: self.coloring_params_buffer.as_entire_binding() },
            BindGroupEntry { binding: 3, resource: BindingResource::TextureView(output) },
            BindGroupEntry { binding: 4, resource: BindingResource::TextureView(input) },
            BindGroupEntry { binding: 5, resource: self.kernel_buffer.as_entire_binding() },
        ];
        // Pyramid levels 1..7 at bindings 6..12; the dummy where the
        // pyramid is shorter (or absent).
        for i in 0..(MAX_PYRAMID_LEVELS as usize - 1) {
            let view = if with_pyramid {
                self.pyramid.get(i).map(|(_, v, _)| v).unwrap_or(&self.pyramid_dummy.1)
            } else {
                &self.pyramid_dummy.1
            };
            entries.push(BindGroupEntry {
                binding: 6 + i as u32,
                resource: BindingResource::TextureView(view),
            });
        }
        entries.push(BindGroupEntry { binding: 14, resource: self.minmax_buffer.as_entire_binding() });
        entries.push(BindGroupEntry { binding: 13, resource: self.deposit_buffer.as_entire_binding() });
        entries.push(BindGroupEntry { binding: 17, resource: self.coupling_buffer.as_entire_binding() });
        entries
    }

    fn dispatch_size(w: u32, h: u32) -> (u32, u32) {
        (w.div_ceil(8), h.div_ceil(8))
    }

    /// Seed the field from the config. Resets the step counter: the
    /// pair (seed, step_index) is the state's identity, and a reseed
    /// starts a new run.
    pub fn seed(&mut self, device: &Device, queue: &Queue, cfg: &SimConfig) {
        self.ensure_layers(device, cfg);
        self.ensure_pipelines(device, cfg);
        self.ensure_pyramid(device, cfg);
        // `ensure_agents` may set `needs_seed`; this IS the seed, so
        // the flag is cleared at the end regardless.
        self.ensure_agents(device, cfg);
        self.step_index = 0;
        let model = model_or_default(&cfg.model);
        let coloring = coloring_or_default(&cfg.coloring);
        // Parameter arrays FIRST: building the kernel is what sets
        // `kernel_radius`, and the uniform written next carries it.
        // The other order shipped for a wave, and Lenia's seed -- which
        // sizes its noise patches by that radius -- read whatever the
        // previous model had left there (1 on a fresh renderer). The
        // phase-3 review found it; the soup baseline was regenerated.
        self.write_param_arrays(queue, model, coloring, cfg);
        // Ring slots (0, layer, 0): each layer's seed reads its own.
        self.write_params_ring(queue, cfg, 0, 1);
        let layers = cfg.layer_count();
        let stride = self.params_stride as u32;

        let p = self.pipelines.as_ref().expect("pipelines built above");
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Sim Seed BG"),
            layout: &p.seed_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &self.params_buffer,
                        offset: 0,
                        size: std::num::NonZeroU64::new(
                            std::mem::size_of::<SimParamsGpu>() as u64,
                        ),
                    }),
                },
                BindGroupEntry { binding: 1, resource: self.model_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.coloring_params_buffer.as_entire_binding() },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&self.field_view[self.current]),
                },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Sim Seed"),
        });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Sim Seed"),
                timestamp_writes: None,
            });
            let (gx, gy) = Self::dispatch_size(self.grid_w, self.grid_h);
            for l in 0..layers {
                pass.set_pipeline(&p.layer_seeds[l]);
                pass.set_bind_group(0, &bg, &[self.ring_slot(0, l, 0, layers) * stride]);
                pass.dispatch_workgroups(gx, gy, 1);
            }
        }
        queue.submit(std::iter::once(enc.finish()));
        self.needs_seed = false;
        self.seeded_as = Some(SeedIdentity::of(cfg));

        // A model that renormalises needs the seed's range before its
        // first step. Step 0 reads slot (0 - 1) mod RING, so the seed's
        // reduce writes THAT slot; the uniform is rewritten for it,
        // after the seed's own submission has consumed slot 0.
        let models = layer_models(cfg);
        if models.iter().any(|m| m.has(ModelFeature::NeedsMinMax)) {
            self.ensure_step_bind_groups(device);
            self.ensure_stage_bind_groups(device);
            // Step 0 of layer l reads slot (0 * N + l - N) mod RING:
            // each layer's seed reduce writes that slot. Its uniform
            // goes in the layer's ring slot, rewritten for the purpose.
            let n = layers as u32;
            let stride_b = self.params_stride as usize;
            let mut bytes = vec![0u8; stride_b * layers * 2];
            for l in 0..layers {
                let mut p = self.params_for_layer(cfg, 0, l);
                p.minmax_slot = (MINMAX_RING + l as u32 - n) % MINMAX_RING;
                let at = self.ring_slot(0, l, 0, layers) as usize * stride_b;
                bytes[at..at + std::mem::size_of::<SimParamsGpu>()]
                    .copy_from_slice(bytemuck::bytes_of(&p));
            }
            queue.write_buffer(&self.params_buffer, 0, &bytes);
            self.clear_minmax_slots(queue, MINMAX_RING - n, n);
            let pipes = self.pipelines.as_ref().expect("built above");
            let groups = self.reduce_bind_groups.as_ref().expect("built above");
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Sim Seed Reduce"),
            });
            {
                let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Sim Seed Reduce"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipes.reduce.as_ref().expect("NeedsMinMax builds it"));
                let (gx, gy) = Self::dispatch_size(self.grid_w, self.grid_h);
                for l in 0..layers {
                    if !models[l].has(ModelFeature::NeedsMinMax) {
                        continue;
                    }
                    pass.set_bind_group(0, &groups[self.current], &[self.ring_slot(0, l, 0, layers) * stride]);
                    pass.dispatch_workgroups(gx, gy, 1);
                }
            }
            queue.submit(std::iter::once(enc.finish()));
        }

        // The agent population and the two per-cell integer buffers.
        // After the reduce above, not before: an agent seed that reads
        // the field's range -- DLA launches just outside the cluster
        // -- has to see the seeded field's, not whatever the previous
        // run left in that slot.
        // Deposit starts empty; claim starts at "unclaimed", which is
        // u32::MAX because the claim is an atomic MINIMUM.
        if model.agents.is_some() {
            let cells = (self.grid_w as usize) * (self.grid_h as usize);
            queue.write_buffer(&self.deposit_buffer, 0, bytemuck::cast_slice(&vec![0u32; cells]));
            queue.write_buffer(
                &self.claim_buffer,
                0,
                bytemuck::cast_slice(&vec![u32::MAX; cells]),
            );
            self.ensure_stage_bind_groups(device);
            let pipes = self.pipelines.as_ref().expect("built above");
            let groups = self.agent_bind_groups.as_ref().expect("built above");
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Sim Agent Seed"),
            });
            {
                let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Sim Agent Seed"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipes.agent_seed.as_ref().expect("an agent model builds it"));
                pass.set_bind_group(0, &groups[self.current], &[0]);
                pass.dispatch_workgroups(self.agent_capacity.div_ceil(64), 1, 1);
            }
            queue.submit(std::iter::once(enc.finish()));
        }
    }

    /// Advance exactly `count` steps, in watchdog-sized submissions.
    ///
    /// Batching does not change the sequence: each step reads the
    /// previous step's output, so a batch of 256 and 256 batches of one
    /// produce identical fields. There is a test for that, because it
    /// is the property that lets an export batch freely while a still
    /// stays reproducible.
    ///
    /// "Watchdog-sized" is measured, not assumed: each submission is
    /// waited on and timed, and the next is sized to
    /// [`SUBMIT_BUDGET_MS`] from that cost. The wait means the CPU
    /// stalls for the simulation's GPU time inside this call -- for an
    /// interactive frame that is a few milliseconds and for an export
    /// it was already the case. On wasm the wait returns at once (no
    /// blocking poll in WebGPU), the measured cost is ~0, and the
    /// batch sits at the ceiling; browsers have no 2 s reset to avoid.
    pub fn run_steps(&mut self, device: &Device, queue: &Queue, cfg: &SimConfig, count: u32) {
        if count == 0 {
            return;
        }
        self.ensure_pipelines(device, cfg);
        self.ensure_pyramid(device, cfg);
        self.ensure_layers(device, cfg);
        let model = model_or_default(cfg.layer_model_name(0));
        let coloring = coloring_or_default(&cfg.coloring);
        self.write_param_arrays(queue, model, coloring, cfg);

        self.ensure_agents(device, cfg);
        self.ensure_step_bind_groups(device);
        self.ensure_stage_bind_groups(device);
        self.ensure_flame_bind_group(device);
        let agent_groups = self.agent_capacity.div_ceil(64);
        let (gx, gy) = Self::dispatch_size(self.grid_w, self.grid_h);
        let stride = self.params_stride as u32;
        let layers = cfg.layer_count();
        let models = layer_models(cfg);
        let wants_minmax = models.iter().any(|m| m.has(ModelFeature::NeedsMinMax));
        // Per layer, its passes in order with their repeats unrolled:
        // one entry per dispatch, naming the pass. A relaxation whose
        // sweep count is a slider cannot be compiled in.
        let layer_stages: Vec<Vec<usize>> = models
            .iter()
            .enumerate()
            .map(|(l, m)| {
                let params = cfg.layer_model_params(l);
                let mut v = Vec::new();
                for n in 0..m.passes {
                    let rep = match m.repeat {
                        Some((idx, name)) if idx == n => m
                            .parameters
                            .iter()
                            .find(|p| p.name == name)
                            .map(|p| params.get(p.name).copied().filter(|v| v.is_finite()).unwrap_or(p.default))
                            .unwrap_or(1.0)
                            .round()
                            .clamp(1.0, crate::sim::MAX_INNER_ITERATIONS as f32)
                            as u32,
                        _ => 1,
                    };
                    for _ in 0..rep {
                        v.push(n as usize);
                    }
                }
                v
            })
            .collect();
        // Every stage writes every layer -- the layer's own pass, or a
        // copy-through where it has none -- and then the pair flips,
        // so no dispatch ever reads a slice another layer's dispatch
        // has not written this stage (simulation-layers plan, section
        // 2). With one layer this is exactly the old sequence.
        let max_stages = layer_stages.iter().map(|v| v.len()).max().unwrap_or(1).max(1);
        // Per-level dispatch sizes, level 1 upward.
        let level_dispatch: Vec<(u32, u32)> = {
            let (mut w, mut h) = (self.grid_w, self.grid_h);
            (0..self.pyramid.len())
                .map(|_| {
                    w = w.div_ceil(2);
                    h = h.div_ceil(2);
                    Self::dispatch_size(w, h)
                })
                .collect()
        };
        let levels = pyramid_levels(self.grid_w, self.grid_h) as usize;

        // The blind first submit is sized in DISPATCHES, so a step that
        // is two hundred of them starts at one step rather than eight.
        // `== FIRST_SUBMIT` is a sentinel for "nothing measured yet"; a
        // calibrated batch that happens to equal it is shrunk once and
        // re-measured, which costs one small submission and nothing
        // else.
        let warping = !cfg.warp.is_identity();
        let octaves = cfg.warp.mode == crate::config::sim::SimWarpMode::Octaves;
        if self.steps_per_submit == FIRST_SUBMIT {
            let dispatches: u32 =
                (max_stages as u32 + u32::from(wants_minmax) + u32::from(warping)) * layers as u32;
            self.steps_per_submit = (FIRST_SUBMIT * 2 / dispatches.max(1)).clamp(1, FIRST_SUBMIT);
        }
        // The ring holds (step, layer, variant) slots.
        let per_submit_cap = (MAX_STEPS_PER_SUBMIT / layers as u32).max(1);
        let mut done = 0;
        while done < count {
            let batch = self.steps_per_submit.clamp(1, per_submit_cap).min(count - done);
            // One write for the whole batch, and one compute pass: the
            // dispatches inside it are ordered against each other, and
            // each reads its own ring slot by dynamic offset.
            self.write_params_ring(queue, cfg, self.step_index, batch);
            if wants_minmax {
                self.clear_minmax_slots(queue, self.step_index * layers as u32, batch * layers as u32);
            }
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Sim Steps"),
            });
            {
                let p = self.pipelines.as_ref().expect("pipelines built above");
                let groups = self
                    .step_bind_groups
                    .as_ref()
                    .expect("built by ensure_step_bind_groups");
                let flame_bg = if p.layer_warp.is_some() && self.layer_map.iter().any(|m| m.rates.iter().any(|r| *r > 0.0)) {
                    self.layer_map.as_ref().and_then(|m| m.bind_group.as_ref())
                } else {
                    None
                };
                let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Sim Steps"),
                    timestamp_writes: None,
                });
                for i in 0..batch {
                    let slot = |l: usize, variant: u32| (i * layers as u32 + l as u32) * 2 + variant;
                    // The warp goes first, before anything reads the
                    // field: it moves the FIELD, through the boundary
                    // rule, and nothing else -- an agent population's
                    // positions stay where they are. Every layer is
                    // written, so the pair flips once. In octave mode
                    // the field is resampled only on the steps where
                    // the accumulated zoom crosses a power of two.
                    let warp_now = if octaves {
                        cfg.warp.octave(self.step_index).warp.is_some()
                    } else {
                        warping
                    };
                    if warp_now {
                        pass.set_pipeline(&p.warp);
                        for l in 0..layers {
                            pass.set_bind_group(0, &groups[self.current], &[slot(l, 0) * stride]);
                            pass.dispatch_workgroups(gx, gy, 1);
                        }
                        self.current = 1 - self.current;
                    }
                    // The layer map: each layer read through its own
                    // transform at its rate; a layer at rate 0 is
                    // carried across. One stage, one flip.
                    if let (Some(lw), Some(fbg)) = (p.layer_warp.as_ref(), flame_bg) {
                        for l in 0..layers {
                            if self.layer_rate(cfg, l) > 0.0 {
                                pass.set_pipeline(lw);
                                pass.set_bind_group(0, &groups[self.current], &[slot(l, 0) * stride]);
                                pass.set_bind_group(1, fbg, &[]);
                            } else {
                                pass.set_pipeline(&p.warp);
                                pass.set_bind_group(0, &groups[self.current], &[slot(l, VARIANT_COPY) * stride]);
                            }
                            pass.dispatch_workgroups(gx, gy, 1);
                        }
                        self.current = 1 - self.current;
                    }
                    // The agents move, sense and deposit next, from
                    // the field as the last step left it -- Jones'
                    // order. The agent layer's step then folds what
                    // they deposited and clears it.
                    if let (Some(al), Some(agroups)) = (p.agent_layer, self.agent_bind_groups.as_ref()) {
                        for ap in p.agents.iter() {
                            pass.set_pipeline(ap);
                            pass.set_bind_group(0, &agroups[self.current], &[slot(al, 0) * stride]);
                            pass.dispatch_workgroups(agent_groups, 1, 1);
                        }
                    }
                    // The pyramid of each layer that reads one, from
                    // the current field, level by level: each dispatch
                    // reads the level below through its own uniform.
                    if let Some(pyr) = p.pyramid.as_ref() {
                        let pgroups = self
                            .pyramid_bind_groups
                            .as_ref()
                            .expect("built by ensure_stage_bind_groups");
                        pass.set_pipeline(pyr);
                        for l in 0..layers {
                            if !models[l].has(ModelFeature::NeedsPyramid) {
                                continue;
                            }
                            for (lv, per_side) in pgroups.iter().enumerate() {
                                let bg = if lv == 0 { &per_side[self.current] } else { &per_side[0] };
                                pass.set_bind_group(0, bg, &[((l * levels + lv) as u32) * stride]);
                                let (lx, ly) = level_dispatch[lv];
                                pass.dispatch_workgroups(lx, ly, 1);
                            }
                        }
                    }
                    // groups[src] reads field[src] and writes
                    // field[1 - src], so alternating the index IS the
                    // ping-pong. Every stage writes every layer.
                    for stage in 0..max_stages {
                        for l in 0..layers {
                            match layer_stages[l].get(stage) {
                                Some(&n) if cfg.layer_enabled(l) => {
                                    pass.set_pipeline(&p.layer_steps[l][n]);
                                    pass.set_bind_group(0, &groups[self.current], &[slot(l, 0) * stride]);
                                }
                                _ => {
                                    pass.set_pipeline(&p.warp);
                                    pass.set_bind_group(0, &groups[self.current], &[slot(l, VARIANT_COPY) * stride]);
                                }
                            }
                            pass.dispatch_workgroups(gx, gy, 1);
                        }
                        self.current = 1 - self.current;
                    }
                    // The new field's range, into this step's slot, for
                    // the next step to normalise by.
                    if let Some(red) = p.reduce.as_ref() {
                        let rgroups = self
                            .reduce_bind_groups
                            .as_ref()
                            .expect("built by ensure_stage_bind_groups");
                        pass.set_pipeline(red);
                        for l in 0..layers {
                            if !models[l].has(ModelFeature::NeedsMinMax) {
                                continue;
                            }
                            pass.set_bind_group(0, &rgroups[self.current], &[slot(l, 0) * stride]);
                            pass.dispatch_workgroups(gx, gy, 1);
                        }
                    }
                    self.step_index += 1;
                }
            }
            queue.submit(std::iter::once(enc.finish()));
            done += batch;

            // Time this submission and size the next one from it. The
            // measurement can include unrelated work still in flight
            // (the seed pass, a previous frame's tonemap), which only
            // overestimates the cost and shrinks the next batch: the
            // safe direction.
            let started = web_time::Instant::now();
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let ms = started.elapsed().as_secs_f64() * 1e3;
            let per_step = ms / batch as f64;
            // Kept for the interactive budget. Smoothed, because a
            // single submission can be timed against unrelated work
            // still in flight; an exponential average settles on the
            // real cost within a few batches without a spike moving it
            // far.
            self.ms_per_step = if self.ms_per_step > 0.0 {
                self.ms_per_step * 0.7 + per_step * 0.3
            } else {
                per_step
            };
            self.steps_per_submit = if per_step > 0.0 {
                (SUBMIT_BUDGET_MS / per_step).floor().clamp(1.0, MAX_STEPS_PER_SUBMIT as f64) as u32
            } else {
                MAX_STEPS_PER_SUBMIT
            };
        }
    }

    /// Colour the live field into the output image at the display size.
    ///
    /// Always run, even on a frame that took no steps: a parameter or
    /// palette edit has to be visible without advancing the
    /// simulation, and stepping to show an edit would make the picture
    /// depend on how long the user looked at it.
    pub fn color(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        palette_view: &TextureView,
    ) {
        self.ensure_pipelines(device, cfg);
        self.write_params_slot0(queue, cfg);
        let model = model_or_default(&cfg.model);
        let coloring = coloring_or_default(&cfg.coloring);
        self.write_param_arrays(queue, model, coloring, cfg);

        let sdf = Self::wants_sdf(cfg);
        self.ensure_sdf(device, sdf);
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Sim Color"),
        });
        if sdf {
            self.encode_jump_flood(device, queue, cfg, &mut enc);
        }
        let p = self.pipelines.as_ref().expect("pipelines built above");
        let sdf_view = self.sdf.as_ref().map(|(_, v)| v).unwrap_or(&self.sdf_dummy.1);
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Sim Color BG"),
            layout: &p.color_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &self.params_buffer,
                        offset: 0,
                        size: std::num::NonZeroU64::new(
                            std::mem::size_of::<SimParamsGpu>() as u64,
                        ),
                    }),
                },
                BindGroupEntry { binding: 1, resource: self.model_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.coloring_params_buffer.as_entire_binding() },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&self.output_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&self.field_view[self.current]),
                },
                BindGroupEntry { binding: 5, resource: BindingResource::TextureView(palette_view) },
                BindGroupEntry { binding: 6, resource: BindingResource::TextureView(sdf_view) },
                BindGroupEntry { binding: 7, resource: self.color_layers_buffer.as_entire_binding() },
            ],
        });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Sim Color"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&p.color);
            pass.set_bind_group(0, &bg, &[0]);
            let (gx, gy) = Self::dispatch_size(self.out_w, self.out_h);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    /// One frame of interactive rendering: seed if needed, advance if
    /// running, and always colour.
    pub fn render_frame(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        palette_view: &TextureView,
        steps: u32,
    ) {
        if self.will_reseed(cfg) {
            self.seed(device, queue, cfg);
        }
        // Never overshoot the cap, even by part of a frame's batch:
        // the whole point of `steps` is that stopping there gives the
        // picture an export of the same config gives, and 2,010 steps
        // is not 2,000. Asked AFTER the seed, so a run that has just
        // restarted measures from 0.
        let steps = match self.steps_remaining(cfg) {
            Some(left) => steps.min(left),
            None => steps,
        };
        if steps > 0 {
            self.run_steps(device, queue, cfg, steps);
        }
        self.color(device, queue, cfg, palette_view);
    }


    /// Advance the field to `target` steps and colour it.
    ///
    /// The timeline's contract, and the exporter's: the picture at a
    /// given time is the state at that step count, whatever happened
    /// before. Going BACKWARDS means restarting and re-running, since
    /// the rule is not invertible.
    ///
    /// `budget` caps the steps this call may take. The exporter passes
    /// `None` -- a video frame IS the state at its target, so it runs
    /// as long as it must. The interactive driver passes a
    /// per-display-frame allowance so a big jump is walked over
    /// several frames instead of blocking the UI; call again next
    /// frame until it returns true.
    ///
    /// Returns whether `target` has been reached. The field is
    /// recoloured either way, so a partial catch-up still shows
    /// progress rather than a frozen picture.
    ///
    /// One caveat, inherent to a non-invertible rule: a restart re-runs
    /// with the parameters as they are NOW. If a model parameter is
    /// animating too, the original run's history had it changing along
    /// the way and the re-run's does not, so a reversed step track
    /// retraces the forward pictures exactly only when the parameters
    /// are constant.
    pub fn advance_to(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        palette_view: &TextureView,
        target: u32,
        budget: Option<u32>,
    ) -> bool {
        let reached = self.advance_steps(device, queue, cfg, target, budget);
        self.color(device, queue, cfg, palette_view);
        reached
    }

    /// `advance_to` without the colouring.
    ///
    /// Split out for the exporter, which walks a long run in batches
    /// to report progress and wants ONE colour pass at the end rather
    /// than one per batch. The interactive driver uses `advance_to`,
    /// which recolours every frame -- a parameter or palette edit has
    /// to be visible without advancing the simulation.
    pub fn advance_steps(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        target: u32,
        budget: Option<u32>,
    ) -> bool {
        // A config that no longer describes the loaded field reseeds
        // anyway (`SeedIdentity`), and then the index to measure from
        // is 0 rather than whatever the old run had reached.
        let index = if self.will_reseed(cfg) { 0 } else { self.step_index };
        let plan = crate::sim::plan_steps(index, target, budget);
        if plan.reseed {
            self.request_seed();
        }
        if self.will_reseed(cfg) {
            self.seed(device, queue, cfg);
        }
        if plan.steps > 0 {
            self.run_steps(device, queue, cfg, plan.steps);
        }
        plan.reached
    }

    /// A complete still: seed, run exactly `cfg.steps`, colour. The
    /// export contract.
    pub fn render_still(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        palette_view: &TextureView,
    ) {
        self.seed(device, queue, cfg);
        self.run_steps(device, queue, cfg, cfg.steps);
        self.color(device, queue, cfg, palette_view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bound_grid_follows_the_output_and_a_fixed_one_does_not() {
        let mut cfg = SimConfig::default();
        cfg.grid = SimGrid::Viewport { scale: 0.5 };
        assert_eq!(SimRenderer::grid_for(&cfg, 1920, 1080), (960, 540));
        cfg.grid = SimGrid::Fixed { width: 256, height: 256 };
        assert_eq!(SimRenderer::grid_for(&cfg, 1920, 1080), (256, 256));
    }

    /// `grid_for` must report what was ASKED for, so the refusal path
    /// can see it; only the allocation is bounded. Collapsing these two
    /// silently resized the user's grid and made the refusal
    /// unreachable.
    #[test]
    fn an_absurd_grid_is_reported_honestly_and_allocated_safely() {
        let mut cfg = SimConfig::default();
        cfg.grid = SimGrid::Fixed { width: 100_000, height: 100_000 };
        assert_eq!(
            SimRenderer::grid_for(&cfg, 1920, 1080),
            (100_000, 100_000),
            "the requested size must survive so allocation_error can refuse it"
        );
        assert_eq!(
            SimRenderer::allocatable_grid(&cfg, 1920, 1080),
            (MAX_GRID_DIM, MAX_GRID_DIM),
            "but nothing that large is ever handed to wgpu"
        );
    }

    #[test]
    fn dispatch_covers_every_cell_including_a_partial_workgroup() {
        assert_eq!(SimRenderer::dispatch_size(256, 256), (32, 32));
        // 257 needs a 33rd group whose upper rows the shader's bounds
        // check discards.
        assert_eq!(SimRenderer::dispatch_size(257, 1), (33, 1));
        assert_eq!(SimRenderer::dispatch_size(1, 1), (1, 1));
    }
}
