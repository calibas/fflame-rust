//! WGSL assembly: one model and one colouring spliced into a template.
//!
//! Marker-splicing, exactly as `src/escape/assembler.rs` does it: the
//! template carries `//__MARKER__` lines and this replaces each with
//! the selected definition's source. A pipeline holds ONE model and
//! ONE colouring, so the function names are fixed (`sim_step`,
//! `sim_seed`, `sim_color`) and there is no index-mapping problem like
//! the variation registry's.
//!
//! Three passes are generated:
//!
//! * [`assemble_seed`] — writes the initial field from the config's
//!   init shape. Its own shader because the init shape is a uniform
//!   choice, not a per-cell branch worth carrying in the step loop.
//! * [`assemble_step`] — the stencil. Run K times per frame, and it is
//!   the only pass whose cost matters.
//! * [`assemble_color`] — field to `Rgba32Float` in the flame
//!   accumulator's layout, plus the resolve to the output size.
//!
//! **Boundary handling lives here, not in a model.** `sim_read`
//! applies the configured wrap/clamp/zero/mirror once, so a model
//! cannot get it subtly wrong — a mistake that is invisible in the
//! middle of the grid and wrong only at its edges.

use super::{ColoringFeature, ModelDef, ModelFeature, SimColoringDef};
use crate::config::sim::{SimBoundary, SimDownscale, SimUpscale};

/// Shared prelude: bindings, parameter accessors, boundary-aware
/// reads. Spliced into every pass so the three shaders agree about
/// what a cell is.
const COMMON: &str = r#"
struct SimParams {
    grid: vec2<u32>,
    out_size: vec2<u32>,
    step_index: u32,
    seed_lo: u32,
    seed_hi: u32,
    dt: f32,
    init_p0: f32,
    init_p1: f32,
    kernel_radius: u32,
    // The min/max ring slot this dispatch belongs to: the reduce pass
    // writes it, the step pass reads the slot BEFORE it.
    minmax_slot: u32,
};

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> model_params: array<f32>;
@group(0) @binding(2) var<storage, read> coloring_params: array<f32>;

fn mparam(i: u32) -> f32 {
    return model_params[i];
}
fn cparam(i: u32) -> f32 {
    return coloring_params[i];
}
fn sim_dt() -> f32 {
    return params.dt;
}
fn sim_step_index() -> u32 {
    return params.step_index;
}
fn sim_grid() -> vec2<i32> {
    return vec2<i32>(params.grid);
}

// Metal runs shaders with fast math on, and its `atan2` is wrong at
// zero pairs in BOTH directions: same-sign zeros give pi/4 -- a
// plausible finite value that silently relocates a point -- and
// mixed-sign zeros give NaN. This is the flame path's `ff_atan2`,
// which is IEEE-exact for all four sign pairs; the sign is read
// through `bitcast` because `x < 0.0` is false for -0.0 and integer
// ops are immune to fast math. Any model taking the angle of a
// gradient reaches (0, 0) wherever its field is flat, which is most
// of the grid.
fn ff_atan2(y: f32, x: f32) -> f32 {
    if (y == 0.0 && x == 0.0) {
        let pi = 3.14159265358979;
        let mag = select(0.0, pi, (bitcast<u32>(x) & 0x80000000u) != 0u);
        return select(mag, -mag, (bitcast<u32>(y) & 0x80000000u) != 0u);
    }
    return atan2(y, x);
}

// Integer state lives in an f32 channel (exact to 2^24), so cycling a
// state is a float modulo. `%` on floats in WGSL is a remainder like
// its integer form, so the same bias-before-wrap applies as in the
// periodic boundary -- a negative state would otherwise cycle the wrong
// way rather than erroring.
fn fract_state(v: f32, n: f32) -> f32 {
    let m = v - floor(v / n) * n;
    return floor(m);
}

// PCG, the same generator the flame shaders use. Keyed by (seed, cell,
// step) so a run is reproducible from the config alone and does not
// depend on dispatch order.
fn sim_pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn sim_rand(p: vec2<i32>, salt: u32) -> f32 {
    let g = sim_grid();
    let idx = u32(p.y * g.x + p.x);
    var h = sim_pcg(idx ^ params.seed_lo);
    h = sim_pcg(h ^ params.seed_hi ^ salt);
    h = sim_pcg(h ^ params.step_index);
    // 24 bits into [0, 1): the mantissa's exact range, so the value is
    // uniform rather than merely close to it.
    return f32(h >> 8u) * (1.0 / 16777216.0);
}
"#;

/// The `sim_read` body for each boundary mode.
fn boundary_body(boundary: SimBoundary) -> &'static str {
    // Each mode defines the same two functions:
    //
    //   sim_wrap_sized(p, g)  -- the in-range coordinate to read for a
    //                            possibly out-of-range p on a grid of
    //                            size g
    //   sim_outside(p, g)     -- true when the read should be ZERO
    //                            instead (only the Zero mode)
    //
    // `sim_read` applies them at the field's own size; the pyramid
    // accessors apply the same rule at each level's size, so a Clamp
    // field is clamped at every scale rather than clamped at the base
    // and wrapped above it.
    match boundary {
        SimBoundary::Periodic => {
            r#"
fn sim_wrap_sized(p: vec2<i32>, g: vec2<i32>) -> vec2<i32> {
    // `%` is remainder, not modulo: it is negative for negative
    // operands, so a bare p % g reads out of bounds on the left and
    // top edges.
    //
    // THE OBVIOUS FORM DOES NOT WORK AT LARGE OFFSETS, and what
    // follows is measured rather than derived.
    //
    // This used to read `((p % g) + g) % g`, which is correct
    // arithmetic. Measured against a CPU mirror of a radius-7 gather
    // at 96x96, that expression is wrong at the edges by 0.228 while
    // the interior is bit-exact -- and it produces BYTE-IDENTICAL
    // output to a bare `p % g` with no bias at all, down to the same
    // 0.793078 average over the taps that leave the grid. So the bias
    // is not reaching the device. The form below, subtracting the
    // truncated quotient and correcting the sign, differs from both
    // and agrees with the mirror EXACTLY (0.0 worst, at every
    // boundary mode).
    //
    // WHAT IS NOT ESTABLISHED is why. The natural guess -- that the
    // optimiser folds the bias away on the assumption that a
    // remainder is non-negative -- does not fit the whole picture:
    // offsets of +-1 are demonstrably unaffected, since all 33
    // periodic visual baselines are byte-identical across this
    // change, and Gray-Scott's CPU mirror passed before it. Something
    // about the failure needs an offset of more than a cell or two,
    // and that has not been pinned down.
    //
    // It went unnoticed for two phases because until the large-kernel
    // models every rule read +-1. SmoothLife is what caught it: its
    // annulus carries its weight at the OUTER radius, so a wrong
    // wrap is 23% of the gather. Lenia hid it even at radius 6 --
    // its ring has almost no weight at the outermost taps, and its
    // growth term saturates exactly where the gather is wrong.
    //
    // Do NOT add an interior fast-path to skip these. Measured at
    // 1080p: Clamp 0.2618 ms/step, Periodic 0.2619. The integer
    // arithmetic is invisible under a bandwidth-bound kernel, and a
    // branch per read would cost more than it does.
    let q = p - g * (p / g);
    return select(q, q + g, q < vec2<i32>(0));
}
fn sim_outside(p: vec2<i32>, g: vec2<i32>) -> bool {
    return false;
}
"#
        }
        SimBoundary::Clamp => {
            r#"
fn sim_wrap_sized(p: vec2<i32>, g: vec2<i32>) -> vec2<i32> {
    return clamp(p, vec2<i32>(0), g - vec2<i32>(1));
}
fn sim_outside(p: vec2<i32>, g: vec2<i32>) -> bool {
    return false;
}
"#
        }
        SimBoundary::Zero => {
            r#"
fn sim_wrap_sized(p: vec2<i32>, g: vec2<i32>) -> vec2<i32> {
    return clamp(p, vec2<i32>(0), g - vec2<i32>(1));
}
fn sim_outside(p: vec2<i32>, g: vec2<i32>) -> bool {
    return p.x < 0 || p.y < 0 || p.x >= g.x || p.y >= g.y;
}
"#
        }
        SimBoundary::Mirror => {
            r#"
fn sim_mirror1(v: i32, n: i32) -> i32 {
    if (v < 0) { return -v - 1; }
    if (v >= n) { return 2 * n - v - 1; }
    return v;
}
fn sim_wrap_sized(p: vec2<i32>, g: vec2<i32>) -> vec2<i32> {
    return vec2<i32>(sim_mirror1(p.x, g.x), sim_mirror1(p.y, g.y));
}
fn sim_outside(p: vec2<i32>, g: vec2<i32>) -> bool {
    return false;
}
"#
        }
    }
}

/// The field read every template shares, on top of the boundary body.
const READ_BODY: &str = r#"
fn sim_wrap(p: vec2<i32>) -> vec2<i32> {
    return sim_wrap_sized(p, sim_grid());
}
fn sim_read(p: vec2<i32>) -> vec4<f32> {
    let g = sim_grid();
    if (sim_outside(p, g)) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    return textureLoad(field_in, sim_wrap_sized(p, g), 0);
}
"#;

const SEED_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var field_out: texture_storage_2d<rgba32float, write>;

//__MODEL_SEED__

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let g = sim_grid();
    let p = vec2<i32>(gid.xy);
    if (p.x >= g.x || p.y >= g.y) {
        return;
    }
    let inside = sim_init_mask(p);
    let noise = sim_rand(p, 0x5eedu);
    textureStore(field_out, p, sim_seed(inside, noise, p));
}
"#;

const STEP_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var field_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d<f32>;

//__KERNEL__

//__BOUNDARY__

//__PYRAMID__

//__MINMAX__

//__DEPOSIT__

//__MODEL__

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let g = sim_grid();
    let p = vec2<i32>(gid.xy);
    if (p.x >= g.x || p.y >= g.y) {
        return;
    }
//__STEP_CALL__
}
"#;

/// Shared by both agent passes: the record, the population size, and
/// the PCG stream keyed by (seed, AGENT index, step) rather than by
/// cell, since an agent is not at a cell.
const AGENT_COMMON: &str = r#"
struct SimAgent {
    pos: vec2<f32>,
    heading: f32,
    state: f32,
};

@group(0) @binding(15) var<storage, read_write> agents: array<SimAgent>;

fn agent_count() -> u32 {
    return arrayLength(&agents);
}

fn agent_rand(i: u32, salt: u32) -> f32 {
    var h = sim_pcg(i ^ params.seed_lo);
    h = sim_pcg(h ^ params.seed_hi ^ salt);
    h = sim_pcg(h ^ params.step_index);
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

// Add to a cell's deposit. Fixed-point and INTEGER: thousands of
// agents land in one cell in an order the hardware chooses, and
// atomicAdd on a u32 is associative and commutative, so the total is
// the same however they are ordered. An f32 accumulation would not
// be, and the run would not reproduce.
fn agent_deposit(p: vec2<i32>, amount: f32) {
    let g = sim_grid();
    if (sim_outside(p, g)) {
        return;
    }
    let q = sim_wrap_sized(p, g);
    let idx = u32(q.y * g.x + q.x);
    atomicAdd(&deposit[idx], u32(max(amount, 0.0) * 1024.0));
}

@group(0) @binding(16) var<storage, read_write> claim: array<atomic<u32>>;

// Stake a claim on a cell. The winner is the LOWEST agent index, not
// whoever the hardware happened to run first -- atomicMin is
// associative and commutative, so the outcome is the same however the
// dispatch is ordered, and the run reproduces.
fn agent_claim(p: vec2<i32>, i: u32) {
    let g = sim_grid();
    if (sim_outside(p, g)) {
        return;
    }
    let q = sim_wrap_sized(p, g);
    atomicMin(&claim[u32(q.y * g.x + q.x)], i);
}

// Did this agent win that cell? Clears the claim if so, which is
// what returns the buffer to its empty state for the next step: a
// claimed cell has exactly one winner, and only the winner clears.
fn agent_claim_check(p: vec2<i32>, i: u32) -> bool {
    let g = sim_grid();
    if (sim_outside(p, g)) {
        return false;
    }
    let q = sim_wrap_sized(p, g);
    let idx = u32(q.y * g.x + q.x);
    if (atomicLoad(&claim[idx]) == i) {
        atomicStore(&claim[idx], 0xFFFFFFFFu);
        return true;
    }
    return false;
}
"#;

/// The move-and-deposit pass. One thread per agent.
const AGENT_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(4) var field_in: texture_2d<f32>;
@group(0) @binding(13) var<storage, read_write> deposit: array<atomic<u32>>;

//__BOUNDARY__

//__AGENT_COMMON__

//__MINMAX__

//__MODEL_AGENT__

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= agent_count()) {
        return;
    }
//__AGENT_CALL__
}
"#;

/// The population's initial state. One thread per agent.
const AGENT_SEED_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(4) var field_in: texture_2d<f32>;
@group(0) @binding(13) var<storage, read_write> deposit: array<atomic<u32>>;

//__BOUNDARY__

//__AGENT_COMMON__

//__MINMAX__

//__MODEL_AGENT__

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= agent_count()) {
        return;
    }
    agents[i] = sim_agent_seed(i);
}
"#;

/// One pyramid level from the one below it: a separable
/// [1 4 6 4 1]/16 blur, then decimate. `params.grid` is the SOURCE
/// level's size (the renderer writes one uniform per level), and the
/// destination is half of it rounded up, which is what the dispatch
/// covers.
///
/// Gaussian, not box, and that is measured: a 2x2 box downsample
/// converges to a SQUARE kernel however many times it is applied, and
/// McCabe's texture on it showed plainly axis-aligned structure with a
/// spectrum half as peaked as the disc reference's. This converges to
/// a Gaussian, which is round.
const PYRAMID_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var field_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d<f32>;

//__BOUNDARY__

const PYR_G: array<f32, 5> = array<f32, 5>(0.0625, 0.25, 0.375, 0.25, 0.0625);

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let src = sim_grid();
    let dst = (src + vec2<i32>(1, 1)) / 2;
    let p = vec2<i32>(gid.xy);
    if (p.x >= dst.x || p.y >= dst.y) {
        return;
    }
    let c = 2 * p;
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var dy = -2; dy <= 2; dy = dy + 1) {
        for (var dx = -2; dx <= 2; dx = dx + 1) {
            let w = PYR_G[dy + 2] * PYR_G[dx + 2];
            acc = acc + w * sim_read(c + vec2<i32>(dx, dy));
        }
    }
    textureStore(field_out, p, acc);
}
"#;

/// The global min and max of channel `.x`, into one ring slot.
///
/// Each workgroup reduces its 64 cells in shared memory and then does
/// ONE atomic min and one atomic max, so a 1080p field is ~32,000
/// atomics rather than two million. Floats are ordered through an
/// integer encoding (below) because there is no atomic min/max on
/// f32; the slot is pre-cleared to the encoding's identities by the
/// renderer before the batch that will write it.
const REDUCE_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(4) var field_in: texture_2d<f32>;
@group(0) @binding(14) var<storage, read_write> minmax: array<atomic<u32>>;

var<workgroup> wg_min: array<u32, 64>;
var<workgroup> wg_max: array<u32, 64>;

// A monotone map from f32 to u32: negative floats have their bits
// inverted, non-negative ones get the sign bit set. Then integer order
// IS float order, and atomicMin/atomicMax do the job. Integer ops, so
// fast-math cannot touch it.
fn minmax_ord(f: f32) -> u32 {
    let u = bitcast<u32>(f);
    return select(u ^ 0x80000000u, ~u, (u >> 31u) != 0u);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>,
        @builtin(local_invocation_index) lid: u32) {
    let g = sim_grid();
    let p = vec2<i32>(gid.xy);
    var lo = 0xFFFFFFFFu;
    var hi = 0u;
    if (p.x < g.x && p.y < g.y) {
        let v = minmax_ord(textureLoad(field_in, p, 0).x);
        lo = v;
        hi = v;
    }
    wg_min[lid] = lo;
    wg_max[lid] = hi;
    workgroupBarrier();
    for (var s = 32u; s > 0u; s = s >> 1u) {
        if (lid < s) {
            wg_min[lid] = min(wg_min[lid], wg_min[lid + s]);
            wg_max[lid] = max(wg_max[lid], wg_max[lid + s]);
        }
        workgroupBarrier();
    }
    if (lid == 0u) {
        let slot = 2u * params.minmax_slot;
        atomicMin(&minmax[slot], wg_min[0]);
        atomicMax(&minmax[slot + 1u], wg_max[0]);
    }
}
"#;

const COLOR_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var out_image: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d<f32>;
@group(0) @binding(5) var palette_tex: texture_2d<f32>;

//__BOUNDARY__

fn sim_palette(t: f32) -> vec3<f32> {
    let w = i32(textureDimensions(palette_tex).x);
    // textureLoad, not textureSample: this runs in non-uniform control
    // flow on some paths and browsers enforce the WGSL rule strictly
    // (CLAUDE.md). Manual lerp between the two nearest entries.
    let x = clamp(t, 0.0, 1.0) * f32(w - 1);
    let i0 = i32(floor(x));
    let i1 = min(i0 + 1, w - 1);
    let f = x - f32(i0);
    let c0 = textureLoad(palette_tex, vec2<i32>(i0, 0), 0).rgb;
    let c1 = textureLoad(palette_tex, vec2<i32>(i1, 0), 0).rgb;
    return mix(c0, c1, f);
}

//__COLORING__

// Resolve: the coloured GRID mapped to the OUTPUT size. Separate from
// the colouring because the grid is its own quantity -- a 256-cell
// Gray-Scott shown at 1080p is 256 cells of information, and which
// filter presents them is a user choice, not a consequence.
fn sim_shade(p: vec2<i32>) -> vec4<f32> {
    let s = sim_read(p);
//__GRADIENT__
    return sim_color(s, grad, p);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let out_size = vec2<i32>(params.out_size);
    let o = vec2<i32>(gid.xy);
    if (o.x >= out_size.x || o.y >= out_size.y) {
        return;
    }
    let g = sim_grid();
    // LETTERBOX, do not stretch. The grid is its own quantity with its
    // own aspect ratio, so a 256x256 model shown in a 16:9 window is a
    // square picture with bars -- not an ellipse field. Stretching was
    // the first behaviour here and it was obviously wrong the moment a
    // square grid met a widescreen export.
    let fit = min(vec2<f32>(out_size).x / vec2<f32>(g).x,
                  vec2<f32>(out_size).y / vec2<f32>(g).y);
    let shown = vec2<f32>(g) * fit;
    let origin = (vec2<f32>(out_size) - shown) * 0.5;
    // Cell-centre mapping: pixel centre (o + 0.5) to grid space.
    // Sampling at the pixel's corner instead shifts the image half a
    // cell, which is invisible at 1:1 and obvious at 8x.
    let gf = (vec2<f32>(o) + vec2<f32>(0.5, 0.5) - origin) / fit;
    if (gf.x < 0.0 || gf.y < 0.0 || gf.x >= vec2<f32>(g).x || gf.y >= vec2<f32>(g).y) {
        // Outside the grid: zero coverage, so the shared tonemap
        // composites the configured background exactly as it does for
        // an empty region of a flame.
        textureStore(out_image, o, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }

//__RESOLVE__

    textureStore(out_image, o, col);
}
"#;

/// The resolve body: how output pixels sample the coloured grid.
fn resolve_body(up: SimUpscale, down: SimDownscale, magnifying: bool) -> String {
    if magnifying {
        match up {
            SimUpscale::Nearest => r#"
    let cell = clamp(vec2<i32>(floor(gf)), vec2<i32>(0, 0), g - vec2<i32>(1, 1));
    let col = sim_shade(cell);
"#
            .to_string(),
            SimUpscale::Bilinear => r#"
    // Bilinear over the four surrounding CELL CENTRES, which is why
    // the half-cell shift is subtracted first.
    let f = gf - vec2<f32>(0.5, 0.5);
    let i0 = vec2<i32>(floor(f));
    let t = f - floor(f);
    let c00 = sim_shade(clamp(i0, vec2<i32>(0, 0), g - vec2<i32>(1, 1)));
    let c10 = sim_shade(clamp(i0 + vec2<i32>(1, 0), vec2<i32>(0, 0), g - vec2<i32>(1, 1)));
    let c01 = sim_shade(clamp(i0 + vec2<i32>(0, 1), vec2<i32>(0, 0), g - vec2<i32>(1, 1)));
    let c11 = sim_shade(clamp(i0 + vec2<i32>(1, 1), vec2<i32>(0, 0), g - vec2<i32>(1, 1)));
    let col = mix(mix(c00, c10, t.x), mix(c01, c11, t.x), t.y);
"#
            .to_string(),
        }
    } else {
        match down {
            SimDownscale::Nearest => r#"
    let cell = clamp(vec2<i32>(floor(gf)), vec2<i32>(0, 0), g - vec2<i32>(1, 1));
    let col = sim_shade(cell);
"#
            .to_string(),
            SimDownscale::Box => r#"
    // Average every cell the output pixel covers. The loop is bounded
    // so a pathological ratio cannot hang the GPU: past 16x16 the
    // extra taps change nothing a viewer can see.
    // Footprint of this output pixel in grid space, derived from the
    // SAME letterboxed mapping as the point sample above -- computing
    // it independently from o/out_size silently ignored the bars.
    let half = 0.5 / fit;
    let lo = vec2<i32>(floor(gf - vec2<f32>(half, half)));
    let hi = vec2<i32>(ceil(gf + vec2<f32>(half, half)));
    let a = clamp(lo, vec2<i32>(0, 0), g - vec2<i32>(1, 1));
    let b = clamp(hi, a + vec2<i32>(1, 1), min(a + vec2<i32>(16, 16), g));
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var n = 0.0;
    for (var y = a.y; y < b.y; y = y + 1) {
        for (var x = a.x; x < b.x; x = x + 1) {
            acc = acc + sim_shade(vec2<i32>(x, y));
            n = n + 1.0;
        }
    }
    let col = acc / max(n, 1.0);
"#
            .to_string(),
        }
    }
}

/// WGSL for the init shape's coverage mask.
///
/// Its own function rather than a model's concern: every model wants
/// the same shapes, and the sizes matter (phase 0 measured 12-cell
/// blobs dying where 24-cell blobs live).
fn init_mask_body(kind: &str) -> &'static str {
    match kind {
        "noise" => {
            r#"
fn sim_init_mask(p: vec2<i32>) -> f32 {
    // init_p0 = amplitude. The mask IS the noise here, so a model's
    // `inside` argument varies per cell rather than being 0 or 1.
    return sim_rand(p, 0x11u) * params.init_p0;
}
"#
        }
        "blob" => {
            r#"
fn sim_init_mask(p: vec2<i32>) -> f32 {
    let g = sim_grid();
    let c = g / 2;
    let r = i32(params.init_p0);
    let d = abs(p - c);
    return select(0.0, 1.0, d.x <= r && d.y <= r);
}
"#
        }
        "blobs" => {
            r#"
fn sim_init_mask(p: vec2<i32>) -> f32 {
    // init_p0 = count, init_p1 = radius. Positions come from the same
    // PCG the rest of the run uses, keyed only by the blob index, so
    // they are identical at every grid size for a given seed.
    let g = sim_grid();
    let r = i32(params.init_p1);
    let n = i32(params.init_p0);
    var hit = 0.0;
    for (var i = 0; i < n; i = i + 1) {
        let h0 = sim_pcg(u32(i) * 2u + params.seed_lo);
        let h1 = sim_pcg(h0 ^ params.seed_hi);
        let cx = i32(h0 % u32(max(g.x, 1)));
        let cy = i32(h1 % u32(max(g.y, 1)));
        let d = abs(p - vec2<i32>(cx, cy));
        if (d.x <= r && d.y <= r) {
            hit = 1.0;
        }
    }
    return hit;
}
"#
        }
        "ring" => {
            r#"
fn sim_init_mask(p: vec2<i32>) -> f32 {
    let g = sim_grid();
    let c = vec2<f32>(g) * 0.5;
    let r = params.init_p0;
    let d = length(vec2<f32>(p) - c);
    return select(0.0, 1.0, abs(d - r) <= 2.0);
}
"#
        }
        "broken_wave" => {
            r#"
fn sim_init_mask(p: vec2<i32>) -> f32 {
    // A horizontal excited band across the LEFT HALF only, with a
    // refractory tail behind it. The cut end is what curls: an
    // unbroken front just annihilates on the periodic boundary.
    // Returns 1.0 for excited and 0.5 for refractory, so a model can
    // tell the two regions apart from one mask.
    let g = sim_grid();
    let cy = g.y / 2;
    if (p.x >= g.x / 2) {
        return 0.0;
    }
    if (p.y >= cy - 4 && p.y < cy + 4) {
        return 1.0;
    }
    if (p.y >= cy - 12 && p.y < cy - 4) {
        return 0.5;
    }
    return 0.0;
}
"#
        }
        "line" => {
            r#"
fn sim_init_mask(p: vec2<i32>) -> f32 {
    let g = sim_grid();
    return select(0.0, 1.0, p.y >= g.y - 2);
}
"#
        }
        _ => {
            r#"
fn sim_init_mask(p: vec2<i32>) -> f32 {
    let g = sim_grid();
    let c = g / 2;
    return select(0.0, 1.0, p.x == c.x && p.y == c.y);
}
"#
        }
    }
}

fn splice(template: &str, boundary: SimBoundary, replacements: &[(&str, &str)]) -> String {
    let mut out = Vec::new();
    for line in template.lines() {
        let trimmed = line.trim();
        if trimmed == "//__COMMON__" {
            out.push(COMMON.to_string());
        } else if trimmed == "//__BOUNDARY__" {
            out.push(boundary_body(boundary).to_string());
            out.push(READ_BODY.to_string());
        } else if let Some((_, body)) = replacements.iter().find(|(m, _)| *m == trimmed) {
            out.push((*body).to_string());
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// The seeding pass: config init shape → initial field.
pub fn assemble_seed(model: &ModelDef, init_kind: &str) -> String {
    splice(
        SEED_TEMPLATE,
        // Seeding never reads a neighbour, so the boundary is
        // irrelevant; Clamp keeps the shader free of the wrap helper.
        SimBoundary::Clamp,
        &[(
            "//__MODEL_SEED__",
            &format!("{}\n{}", init_mask_body(init_kind), model.wgsl_seed),
        )],
    )
}

/// The step pass: one application of the model's rule to every cell.
///
/// `pass` is 0 for the first dispatch of a step and 1 for the second,
/// which only a fourth-order PDE has ([`ModelDef::passes`]). Both
/// modules carry the model's WHOLE `wgsl` -- so a helper written once
/// is visible to both -- and differ only in which function the entry
/// point calls.
pub fn assemble_step(model: &ModelDef, boundary: SimBoundary, pass: u32) -> String {
    let rng_note = if model.has(ModelFeature::NeedsRng) {
        "// model draws random numbers: keyed by (seed, cell, step)\n"
    } else {
        ""
    };
    // A whole-line marker: `splice` matches markers by line, so the
    // call is spliced as a statement rather than as a name inside one.
    // The convolution table, declared only for the models that gather
    // against it. Every step pipeline shares one bind group layout, so
    // the binding is always THERE; a shader that never names it simply
    // does not read it, and the models that are stencils keep the
    // WGSL they had.
    let kernel = if model.kernel.is_some() {
        r#"@group(0) @binding(5) var<storage, read> kernel_lut: array<f32>;

// Half-width of the table, so a gather knows its bounds.
fn sim_kernel_radius() -> i32 {
    return i32(params.kernel_radius);
}

// One weight. The table is row-major from -radius to +radius in both
// axes; a model carrying two kernels stores the second block straight
// after the first and offsets into it.
fn klut(i: u32) -> f32 {
    return kernel_lut[i];
}

// Taps in one block, which is also the offset of a second one.
fn sim_kernel_taps() -> u32 {
    let w = 2u * params.kernel_radius + 1u;
    return w * w;
}"#
    } else {
        ""
    };
    // Pyramid accessors: seven sampled levels above the field, and
    // the trilinear read the wide-radius models use.
    let pyramid = if model.has(ModelFeature::NeedsPyramid) {
        PYRAMID_ACCESSORS
    } else {
        ""
    };
    // The previous step's global range, for the models that
    // renormalise.
    let minmax = if model.has(ModelFeature::NeedsMinMax) {
        MINMAX_ACCESSORS
    } else {
        ""
    };
    // What the agents left in this cell, and the means to clear it.
    let deposit = if model.has(ModelFeature::NeedsAgents) {
        DEPOSIT_ACCESSORS
    } else {
        ""
    };
    let entry = if pass == 0 { "sim_step" } else { "sim_step2" };
    let call = format!("    textureStore(field_out, p, {entry}(textureLoad(field_in, p, 0), p));");
    splice(
        STEP_TEMPLATE,
        boundary,
        &[
            ("//__MODEL__", &format!("{rng_note}{}", model.wgsl)),
            ("//__STEP_CALL__", &call),
            ("//__KERNEL__", kernel),
            ("//__PYRAMID__", pyramid),
            ("//__MINMAX__", minmax),
            ("//__DEPOSIT__", deposit),
        ],
    )
}

/// Spliced into the step shader of a model that declares
/// [`ModelFeature::NeedsAgents`]: the step pass is what folds the
/// agents' deposit into the field, and what clears it for the next
/// step. One thread per cell, so the clear needs no separate pass and
/// no barrier -- each cell owns its own entry.
const DEPOSIT_ACCESSORS: &str = r#"
@group(0) @binding(13) var<storage, read_write> deposit: array<atomic<u32>>;

// This cell's deposit since the last step, and zero it. Called ONCE
// per cell per step, by the step pass.
fn sim_take_deposit(p: vec2<i32>) -> f32 {
    let g = sim_grid();
    let idx = u32(p.y * g.x + p.x);
    let v = atomicExchange(&deposit[idx], 0u);
    return f32(v) * (1.0 / 1024.0);
}
"#;

/// Spliced into the step shader of a model that declares
/// [`ModelFeature::NeedsPyramid`].
const PYRAMID_ACCESSORS: &str = r#"
@group(0) @binding(6) var pyr1: texture_2d<f32>;
@group(0) @binding(7) var pyr2: texture_2d<f32>;
@group(0) @binding(8) var pyr3: texture_2d<f32>;
@group(0) @binding(9) var pyr4: texture_2d<f32>;
@group(0) @binding(10) var pyr5: texture_2d<f32>;
@group(0) @binding(11) var pyr6: texture_2d<f32>;
@group(0) @binding(12) var pyr7: texture_2d<f32>;

// Levels in the pyramid, level 0 included. The same rule as the
// renderer's `pyramid_levels`, and it must stay the same: this is what
// the sample level is clamped to.
fn pyr_levels() -> i32 {
    var levels = 1;
    var s = min(params.grid.x, params.grid.y);
    while (s >= 8u && levels < 8) {
        s = (s + 1u) / 2u;
        levels = levels + 1;
    }
    return levels;
}

// Size of level l: halved and rounded up, l times.
fn pyr_size(l: i32) -> vec2<i32> {
    var s = sim_grid();
    for (var i = 0; i < l; i = i + 1) {
        s = (s + vec2<i32>(1, 1)) / 2;
    }
    return s;
}

// The level count and every level's size, computed ONCE per
// invocation. A five-scale McCabe step makes twenty bilinear reads
// and each needed the size of its level; recomputing that by loop
// per read was measurable, and this table is what the reads use.
// A model calls `pyr_prepare()` at the top of its step.
var<private> pyr_top_cached: i32 = 0;
var<private> pyr_sizes: array<vec2<i32>, 8>;

fn pyr_prepare() {
    pyr_top_cached = pyr_levels() - 1;
    var s = sim_grid();
    for (var i = 0; i < 8; i = i + 1) {
        pyr_sizes[i] = s;
        s = (s + vec2<i32>(1, 1)) / 2;
    }
}

// One texel of level l, channel .x, with the configured boundary
// applied at THAT level's size. A switch rather than an array: WGSL
// has no dynamic indexing of texture bindings without an extension.
fn pyr_load(l: i32, q: vec2<i32>) -> f32 {
    return pyr_load_sized(l, q, pyr_size(l));
}

// The same, with the level's size already in hand. A bilinear read
// makes four loads at one level, and recomputing the size -- a loop
// -- for each of them was measurable: hoisting it took McCabe at
// 1080p from 7.78 to the figure recorded in the model's docs.
fn pyr_load_sized(l: i32, q: vec2<i32>, g: vec2<i32>) -> f32 {
    if (sim_outside(q, g)) {
        return 0.0;
    }
    let w = sim_wrap_sized(q, g);
    switch l {
        case 0: { return textureLoad(field_in, w, 0).x; }
        case 1: { return textureLoad(pyr1, w, 0).x; }
        case 2: { return textureLoad(pyr2, w, 0).x; }
        case 3: { return textureLoad(pyr3, w, 0).x; }
        case 4: { return textureLoad(pyr4, w, 0).x; }
        case 5: { return textureLoad(pyr5, w, 0).x; }
        case 6: { return textureLoad(pyr6, w, 0).x; }
        default: { return textureLoad(pyr7, w, 0).x; }
    }
}

// Bilinear within level l at a position given in BASE cells (a cell
// centre is p + 0.5). Four loads. `FLOAT32_FILTERABLE` is optional
// and never requested, so the filtering is written out.
fn pyr_level_avg(l: i32, pos: vec2<f32>) -> f32 {
    let s = f32(1 << u32(l));
    let f = pos / s - vec2<f32>(0.5, 0.5);
    let f0 = floor(f);
    let t = f - f0;
    let i0 = vec2<i32>(f0);
    let g = pyr_sizes[l];
    let a = pyr_load_sized(l, i0, g);
    let b = pyr_load_sized(l, i0 + vec2<i32>(1, 0), g);
    let c = pyr_load_sized(l, i0 + vec2<i32>(0, 1), g);
    let d = pyr_load_sized(l, i0 + vec2<i32>(1, 1), g);
    return mix(mix(a, b, t.x), mix(c, d, t.x), t.y);
}

// Trilinear: the two levels bracketing a fractional level, blended.
// Eight loads for an average over any radius.
fn pyr_sample(level: f32, pos: vec2<f32>) -> f32 {
    let top = f32(pyr_top_cached);
    let lf = clamp(level, 0.0, top);
    let l0 = i32(floor(lf));
    let l1 = min(l0 + 1, i32(top));
    let t = lf - floor(lf);
    return mix(pyr_level_avg(l0, pos), pyr_level_avg(l1, pos), t);
}

// The pyramid level whose Gaussian matches a DISC average of radius
// r. Calibrated, not derived: measured on McCabe's five-scale ladder,
// log2(0.55 r) reproduces the exact-disc reference's feature size
// (56.9 against 56.9 cells) and amplitude (sd 0.2695 against 0.2665);
// a plain log2(r) came out 1.8x too coarse.
fn pyr_level_for_radius(r: f32) -> f32 {
    return log2(max(0.55 * r, 1.0));
}
"#;

/// Spliced into the step shader of a model that declares
/// [`ModelFeature::NeedsMinMax`].
const MINMAX_ACCESSORS: &str = r#"
@group(0) @binding(14) var<storage, read> minmax_in: array<u32>;

// Inverse of the reduce pass's ordering map.
fn minmax_unord(e: u32) -> f32 {
    if ((e >> 31u) != 0u) {
        return bitcast<f32>(e ^ 0x80000000u);
    }
    return bitcast<f32>(~e);
}

// The PREVIOUS step's global range of channel .x. Before any reduce
// has run (the slot still holds its cleared identities) it reports
// [-1, 1], which is the range a freshly seeded McCabe field has.
fn sim_minmax() -> vec2<f32> {
    let prev = (params.minmax_slot + 256u) % 257u;
    let lo = minmax_in[2u * prev];
    let hi = minmax_in[2u * prev + 1u];
    if (lo == 0xFFFFFFFFu || hi == 0u) {
        return vec2<f32>(-1.0, 1.0);
    }
    return vec2<f32>(minmax_unord(lo), minmax_unord(hi));
}
"#;

/// The agent move-and-deposit pass.
pub fn assemble_agents(model: &ModelDef, boundary: SimBoundary, pass: u32) -> String {
    let a = model.agents.expect("only called for an agent model");
    // An agent that needs the field's global range -- DLA reads its
    // launch radius from it -- gets the same accessors the step pass
    // does.
    let minmax = if model.has(ModelFeature::NeedsMinMax) { MINMAX_ACCESSORS } else { "" };
    let call = if pass == 0 {
        "    agents[i] = sim_agent(agents[i], i);"
    } else {
        "    agents[i] = sim_agent2(agents[i], i);"
    };
    splice(
        AGENT_TEMPLATE,
        boundary,
        &[
            ("//__AGENT_COMMON__", AGENT_COMMON),
            ("//__MODEL_AGENT__", a.wgsl),
            ("//__AGENT_CALL__", call),
            ("//__MINMAX__", minmax),
        ],
    )
}

/// The agent seeding pass.
pub fn assemble_agent_seed(model: &ModelDef, boundary: SimBoundary) -> String {
    let a = model.agents.expect("only called for an agent model");
    let minmax = if model.has(ModelFeature::NeedsMinMax) { MINMAX_ACCESSORS } else { "" };
    splice(
        AGENT_SEED_TEMPLATE,
        boundary,
        &[
            ("//__AGENT_COMMON__", AGENT_COMMON),
            ("//__MODEL_AGENT__", a.wgsl),
            ("//__MINMAX__", minmax),
        ],
    )
}

/// One pyramid level from the one below it.
pub fn assemble_pyramid(boundary: SimBoundary) -> String {
    splice(PYRAMID_TEMPLATE, boundary, &[])
}

/// The global min/max of the field into a ring slot.
pub fn assemble_reduce() -> String {
    splice(REDUCE_TEMPLATE, SimBoundary::Clamp, &[])
}

/// The colour + resolve pass: field → `Rgba32Float` at the output size.
pub fn assemble_color(
    coloring: &SimColoringDef,
    boundary: SimBoundary,
    up: SimUpscale,
    down: SimDownscale,
    magnifying: bool,
) -> String {
    let resolve = resolve_body(up, down, magnifying);
    // The gradient is four neighbour reads per output pixel, and the
    // bilinear resolve calls sim_shade four times. A colouring that
    // never reads `grad` gets a constant instead; the compiler then
    // has nothing to keep.
    let gradient = if coloring.has(ColoringFeature::NeedsGradient) {
        r#"    // Central-difference gradient of .x for this colouring.
    let gx = sim_read(p + vec2<i32>(1, 0)).x - sim_read(p - vec2<i32>(1, 0)).x;
    let gy = sim_read(p + vec2<i32>(0, 1)).x - sim_read(p - vec2<i32>(0, 1)).x;
    let grad = vec2<f32>(gx, gy) * 0.5;"#
    } else {
        "    let grad = vec2<f32>(0.0, 0.0);"
    };
    splice(
        COLOR_TEMPLATE,
        boundary,
        &[
            ("//__COLORING__", coloring.wgsl),
            ("//__RESOLVE__", &resolve),
            ("//__GRADIENT__", gradient),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{COLORINGS, MODELS};

    use egui_wgpu::wgpu::naga;

    fn validate(src: &str, what: &str) {
        assert!(!src.contains("//__"), "{what} left an unspliced marker");
        let module = naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("{what} parse: {e}
--- source ---
{src}"));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{what} validation: {e:?}
--- source ---
{src}"));
        // The Metal fast-math rules in CLAUDE.md apply to generated
        // source as much as to hand-written shaders: a self-compare is
        // not a NaN test there and a self-division folds to 1.0. This is
        // the same lint every variation and shader file is held to.
        use crate::variations::shader_lint;
        assert!(
            shader_lint::self_operations(src).is_empty(),
            "{what}: fast-math self-op -- {:?}",
            shader_lint::self_operations(src)
        );
        assert!(
            shader_lint::subnormal_literals(src).is_empty(),
            "{what}: subnormal literal -- {:?}",
            shader_lint::subnormal_literals(src)
        );
    }

    /// Every model x colouring x boundary x resolve combination has to
    /// compile. This is the assembler's whole safety net: a model is
    /// text until something parses it, and a typo in an unused
    /// combination would otherwise surface as a black viewport.
    #[test]
    fn every_combination_validates() {
        let boundaries = [
            SimBoundary::Periodic,
            SimBoundary::Clamp,
            SimBoundary::Zero,
            SimBoundary::Mirror,
        ];
        for m in MODELS {
            for b in boundaries {
                for pass in 0..m.passes {
                    validate(
                        &assemble_step(m, b, pass),
                        &format!("step {}/{:?}/pass {pass}", m.name, b),
                    );
                }
            }
            for kind in crate::config::sim::SimInit::KINDS {
                validate(&assemble_seed(m, kind), &format!("seed {}/{kind}", m.name));
            }
        }
        for b in boundaries {
            validate(&assemble_pyramid(b), &format!("pyramid {b:?}"));
            for m in MODELS {
                if let Some(a) = m.agents {
                    for pass in 0..a.passes {
                        validate(
                            &assemble_agents(m, b, pass),
                            &format!("agents {}/{b:?}/pass {pass}", m.name),
                        );
                    }
                    validate(
                        &assemble_agent_seed(m, b),
                        &format!("agent seed {}/{b:?}", m.name),
                    );
                }
            }
        }
        validate(&assemble_reduce(), "reduce");
        for c in COLORINGS {
            for b in boundaries {
                for mag in [true, false] {
                    for up in [SimUpscale::Nearest, SimUpscale::Bilinear] {
                        for down in [SimDownscale::Box, SimDownscale::Nearest] {
                            validate(
                                &assemble_color(c, b, up, down, mag),
                                &format!("color {}/{:?}/{mag}/{:?}/{:?}", c.name, b, up, down),
                            );
                        }
                    }
                }
            }
        }
    }

    /// The periodic read must handle negative coordinates. `%` is
    /// remainder in WGSL, so `p % g` is negative on the left and top
    /// edges and reads out of bounds -- the bug is invisible except at
    /// two edges of the grid.
    #[test]
    fn the_periodic_wrap_avoids_the_idiom_the_optimiser_deletes() {
        let src = boundary_body(SimBoundary::Periodic);
        // This test used to assert the OPPOSITE -- that the source
        // contained `((p.x % g.x) + g.x) % g.x`. It passed for two
        // phases while that expression was measurably behaving like a
        // bare `p % g` on the device, because asserting on source text
        // cannot see what happens to it afterwards.
        //
        // What actually guards the behaviour is
        // `the_large_kernel_gathers_match_a_cpu_mirror`, which
        // compares a radius-7 gather against a CPU mirror at the
        // EDGES, where a wrap that does not wrap is 23% of the field.
        // This one only keeps the deleted idiom from coming back.
        // Comments are stripped first: the note above the wrap QUOTES
        // the deleted idiom in order to explain it.
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("+ g.x) % g.x") && !code.contains("+ g) % g"),
            "the periodic wrap is using the bias-then-remainder idiom, which the \
             optimiser folds away -- see the note in `boundary_body`"
        );
        assert!(
            code.contains("p - g * (p / g)"),
            "the periodic wrap should subtract the truncated quotient"
        );
    }

    /// A colouring that does not declare NeedsGradient must not pay
    /// for one: the generated shader has to contain no gradient reads
    /// at all, or the saving is a comment rather than a fact.
    #[test]
    fn a_colouring_without_needs_gradient_reads_no_neighbours() {
        for c in COLORINGS {
            let src = assemble_color(
                c,
                SimBoundary::Periodic,
                SimUpscale::Nearest,
                SimDownscale::Box,
                true,
            );
            let has_reads = src.contains("let gx = sim_read(");
            assert_eq!(
                has_reads,
                c.has(ColoringFeature::NeedsGradient),
                "{}: gradient reads present={has_reads} but NeedsGradient={}",
                c.name,
                c.has(ColoringFeature::NeedsGradient)
            );
        }
    }

    /// A model must not reach around the boundary helper: the whole
    /// point of `sim_read` is that edge behaviour is decided in one
    /// place.
    #[test]
    fn no_model_calls_texture_load_directly() {
        for m in MODELS {
            assert!(
                !m.wgsl.contains("textureLoad"),
                "{} reads the field directly; use sim_read so the boundary applies",
                m.name
            );
        }
    }
}
