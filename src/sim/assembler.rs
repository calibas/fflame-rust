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
    // writes it, the step pass reads the slot `minmax_back` before it
    // -- one per layer, so with N layers a layer's previous slot is N
    // back.
    minmax_slot: u32,
    // The layer this dispatch is: the slice of the field it reads as
    // its own and writes. The colour and jump-flood passes read the
    // layer they are told to.
    layer: u32,
    // Where this layer's convolution table starts in the shared LUT.
    kernel_offset: u32,
    minmax_back: u32,
    // How many entries of the coupling table apply this frame.
    coupling_count: u32,
    // The warp stage's affine: zoom, rotation, pan x, pan y; then the
    // swirl rate and the filter (0 bilinear, 1 nearest). vec4 then
    // vec2, so the struct is 80 bytes -- `SimParamsGpu` pads to match.
    warp_a: vec4<f32>,
    warp_b: vec2<f32>,
    // The matte's edge: 1 for a distance field, 0 for a threshold.
    // Read by the colour pass and the jump flood.
    matte_b: vec2<f32>,
    // The matte: channel index, mode (0 off, 1 normal, 2 inverted),
    // cutoff, softness. Read by the colour pass and the jump flood.
    matte: vec4<f32>,
    // x: the view magnification about the grid centre (the octave
    // mode's accumulated zoom; 1 otherwise). y: fit, 0 letterbox /
    // 1 cover. z: 1 when the step freezes cells outside the visible
    // window. w: the halo around that window, in cells.
    view: vec4<f32>,
    // Which channels the warp moves, 1 or 0 each.
    warp_mask: vec4<f32>,
    // The layer map's rate (x), for the transform-warp stage.
    xform: vec4<f32>,
};

// The scale from grid cells to output pixels, by the fit: the smaller
// ratio shows the whole grid with bars, the larger fills the output
// and crops.
fn sim_fit_scale() -> f32 {
    let r = vec2<f32>(params.out_size) / vec2<f32>(sim_grid());
    return select(min(r.x, r.y), max(r.x, r.y), params.view.y >= 0.5);
}

// Half the visible window, in cells, about the grid centre: what the
// colour pass shows at the current view magnification.
fn sim_visible_halfextent() -> vec2<f32> {
    return (vec2<f32>(params.out_size) * 0.5) / (sim_fit_scale() * max(params.view.x, 1.0e-4));
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> model_params: array<f32>;
@group(0) @binding(2) var<storage, read> coloring_params: array<f32>;

// Each layer's parameters sit in their own block of the buffer;
// the block is `MODEL_PARAM_SLOTS` floats, mirrored in the renderer.
fn mparam(i: u32) -> f32 {
    return model_params[params.layer * 32u + i];
}

// The slice of the field this dispatch owns.
// The colour stack (simulation-layers plan, section 5) points each
// colouring at its source layer through this; every other pass leaves
// it at zero.
var<private> sim_layer_offset: i32 = 0;
fn sim_layer() -> i32 {
    return i32(params.layer) + sim_layer_offset;
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
    // The layer salts the stream, so two layers seeded alike draw
    // different noise; layer 0's salt is zero, so a single field's
    // stream is what it always was.
    var h = sim_pcg(idx ^ params.seed_lo ^ (params.layer * 0x9E3779B9u));
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
const SIM_PERIODIC: bool = true;
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
const SIM_PERIODIC: bool = false;
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
const SIM_PERIODIC: bool = false;
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
const SIM_PERIODIC: bool = false;
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
    return textureLoad(field_in, sim_wrap_sized(p, g), sim_layer(), 0);
}
"#;

/// The jump flood's seed pass: every cell records itself as the
/// nearest cell of its own kind. `.xy` is the nearest cell INSIDE the
/// matte's figure, `.zw` the nearest OUTSIDE, and JFA_FAR is "none
/// found yet". Which side a cell is on is the matte's own rule --
/// channel, cutoff, direction -- so the distance field agrees with
/// the threshold about where the figure is and differs only in what
/// it says about the cells around it.
const JFA_INIT_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var jfa_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d_array<f32>;

const JFA_FAR: f32 = -1.0e6;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let g = vec2<i32>(params.grid);
    let p = vec2<i32>(gid.xy);
    if (p.x >= g.x || p.y >= g.y) {
        return;
    }
    let s = textureLoad(field_in, p, sim_layer(), 0);
    let which = i32(round(clamp(params.matte.x, 0.0, 3.0)));
    var v = s.x;
    if (which == 1) { v = s.y; }
    else if (which == 2) { v = s.z; }
    else if (which == 3) { v = s.w; }
    var inside = v >= params.matte.z;
    if (params.matte.y >= 1.5) {
        inside = !inside;
    }
    let me = vec2<f32>(p);
    let far = vec2<f32>(JFA_FAR, JFA_FAR);
    textureStore(jfa_out, p, select(vec4<f32>(far, me), vec4<f32>(me, far), inside));
}
"#;

/// One jump: each cell looks at its eight neighbours `k` cells away
/// and keeps, for each of the two kinds, whichever candidate seed is
/// nearer than what it has. Run for k = N/2, N/4, ..., 1, and once
/// more at 1 (the "JFA+1" refinement), every cell ends up within a
/// cell or so of its true nearest seed.
const JFA_STEP_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var jfa_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(5) var jfa_in: texture_2d<f32>;

const JFA_NONE: f32 = -1.0e5;

fn jfa_nearer(best: vec2<f32>, cand: vec2<f32>, me: vec2<f32>) -> vec2<f32> {
    if (cand.x <= JFA_NONE) {
        return best;
    }
    if (best.x <= JFA_NONE) {
        return cand;
    }
    return select(best, cand, distance(cand, me) < distance(best, me));
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let g = vec2<i32>(params.grid);
    let p = vec2<i32>(gid.xy);
    if (p.x >= g.x || p.y >= g.y) {
        return;
    }
    // The jump for this pass rides in the kernel-radius word, which
    // nothing else in this shader reads.
    let k = i32(params.kernel_radius);
    let me = vec2<f32>(p);
    var best = textureLoad(jfa_in, p, 0);
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            if (dx == 0 && dy == 0) {
                continue;
            }
            let q = p + vec2<i32>(dx, dy) * k;
            if (q.x < 0 || q.y < 0 || q.x >= g.x || q.y >= g.y) {
                continue;
            }
            let c = textureLoad(jfa_in, q, 0);
            best = vec4<f32>(jfa_nearer(best.xy, c.xy, me), jfa_nearer(best.zw, c.zw, me));
        }
    }
    textureStore(jfa_out, p, best);
}
"#;

/// Seeds to a signed distance, in cells, positive inside the figure.
///
/// A cell is its own nearest seed of its own kind, so what carries
/// information is the distance to the nearest cell of the OTHER kind,
/// and the boundary between two cell centres lies half a cell short
/// of it: inside, d = d_out - 1/2; outside, d = 1/2 - d_in. Beside a
/// straight edge that is +1/2 and -1/2, so the resolve's interpolation
/// puts the zero exactly between the two centres, and it is the true
/// distance everywhere else -- the first version halved the
/// difference of the two, which agreed at the edge and read half the
/// distance from there on, so the distance edge could not differ from
/// the threshold one (the test caught it at the disc's centre). A
/// cell with no seed of the other kind (a grid that is all figure, or
/// all background) reads as very far inside or outside, which is
/// right.
const JFA_FINAL_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var sdf_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(5) var jfa_in: texture_2d<f32>;

const JFA_NONE: f32 = -1.0e5;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let g = vec2<i32>(params.grid);
    let p = vec2<i32>(gid.xy);
    if (p.x >= g.x || p.y >= g.y) {
        return;
    }
    let v = textureLoad(jfa_in, p, 0);
    let me = vec2<f32>(p);
    let d_in = select(1.0e6, distance(v.xy, me), v.x > JFA_NONE);
    let d_out = select(1.0e6, distance(v.zw, me), v.z > JFA_NONE);
    // Inside iff this cell is its own inside seed, which the seed pass
    // wrote and no jump replaces (nothing is nearer than 0).
    let inside = d_in <= 0.0;
    let d = select(0.5 - d_in, d_out - 0.5, inside);
    textureStore(sdf_out, p, vec4<f32>(d, 0.0, 0.0, 0.0));
}
"#;

/// The warp stage (pipeline section 4.1): the field resampled through
/// the per-step affine about the grid centre, with a swirl on top.
/// Reads through `sim_read`, so the boundary rule decides what comes
/// in from beyond the edge when the field shrinks or pans -- zeros
/// under Zero, the wrapped field under Periodic.
///
/// The INVERSE map, as a resampler must: for each destination cell,
/// where in the source did it come from. Content moves forward by
/// zoom, rotation and pan; a destination cell undoes the pan, the
/// zoom, and then the rotation the swirl adds to at its own radius.
/// The warp's samplers -- bilinear, Catmull-Rom, bicubic -- through
/// sim_read, so every tap honours the boundary. Shared by the global
/// warp and the layer map.
const WARP_SAMPLERS: &str = r#"
// Four taps, weighted -- through sim_read, so every tap honours the
// boundary.
fn warp_bilinear(src: vec2<f32>) -> vec4<f32> {
    let i0 = vec2<i32>(floor(src));
    let f = src - vec2<f32>(i0);
    let a = sim_read(i0);
    let b = sim_read(i0 + vec2<i32>(1, 0));
    let c = sim_read(i0 + vec2<i32>(0, 1));
    let d = sim_read(i0 + vec2<i32>(1, 1));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// Catmull-Rom weights for a fractional position, as the colour pass's.
fn warp_catmull_rom(t: f32) -> vec4<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return vec4<f32>(
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2,
    );
}

// Sixteen taps, through sim_read, so every tap honours the boundary.
fn warp_bicubic(src: vec2<f32>) -> vec4<f32> {
    let i0 = vec2<i32>(floor(src));
    let f = src - vec2<f32>(i0);
    let wx = warp_catmull_rom(f.x);
    let wy = warp_catmull_rom(f.y);
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var j = 0; j < 4; j = j + 1) {
        for (var i = 0; i < 4; i = i + 1) {
            acc = acc + sim_read(i0 + vec2<i32>(i - 1, j - 1)) * (wx[i] * wy[j]);
        }
    }
    return acc;
}

"#;

const WARP_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var field_out: texture_storage_2d_array<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d_array<f32>;

//__BOUNDARY__

//__SAMPLERS__

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let g = sim_grid();
    let p = vec2<i32>(gid.xy);
    if (p.x >= g.x || p.y >= g.y) {
        return;
    }
    let centre = (vec2<f32>(g) - vec2<f32>(1.0, 1.0)) * 0.5;
    let d = vec2<f32>(p) - centre;
    let zoom = max(params.warp_a.x, 1.0e-4);
    let pan = params.warp_a.zw;
    // Rotation at this radius: the uniform rate plus the swirl, which
    // is zero at the centre and `flow` at the rim.
    let rim = max(min(f32(g.x), f32(g.y)) * 0.5, 1.0);
    let theta = params.warp_a.y + params.warp_b.x * (length(d) / rim);
    // Undo the pan and the zoom, then rotate back.
    let q = (d - pan) / zoom;
    let cs = cos(theta);
    let sn = sin(theta);
    let src = centre + vec2<f32>(cs * q.x + sn * q.y, -sn * q.x + cs * q.y);
    var v: vec4<f32>;
    if (params.warp_b.y >= 1.5) {
        v = warp_bicubic(src);
    } else if (params.warp_b.y >= 0.5) {
        v = sim_read(vec2<i32>(floor(src + vec2<f32>(0.5, 0.5))));
    } else {
        v = warp_bilinear(src);
    }
    // The channels the warp does not move keep their own value: a
    // layer advected past layers that sit still.
    // select, not mix: mix(stay, v, 1) is stay + (v - stay) * 1,
    // which is not v to the last bit, and a chaotic run amplifies the
    // difference -- measured, three baselines moved.
    let stay = textureLoad(field_in, p, sim_layer(), 0);
    v = select(stay, v, params.warp_mask >= vec4<f32>(0.5, 0.5, 0.5, 0.5));
    textureStore(field_out, p, sim_layer(), v);
}
"#;

const SEED_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var field_out: texture_storage_2d_array<rgba32float, write>;

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
    textureStore(field_out, p, sim_layer(), sim_seed(inside, noise, p));
}
"#;

const STEP_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var field_out: texture_storage_2d_array<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d_array<f32>;

//__KERNEL__

//__BOUNDARY__

//__COUPLING__

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
    // Octave culling: a cell outside the visible window and its halo
    // is carried across unchanged. It is cropped away at the next
    // doubling, and the halo is wide enough that its staleness does
    // not reach the window before then.
    if (params.view.z >= 0.5) {
        let he = sim_visible_halfextent() + vec2<f32>(params.view.w, params.view.w);
        let d = abs(vec2<f32>(p) + vec2<f32>(0.5, 0.5) - vec2<f32>(g) * 0.5);
        if (d.x > he.x || d.y > he.y) {
            textureStore(field_out, p, sim_layer(), textureLoad(field_in, p, sim_layer(), 0));
            return;
        }
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
//
// THE CONTRACT: every claim is followed, in the next pass, by the
// same agent's `agent_claim_check` on the same cell. Nothing else
// clears the buffer between steps -- the winner's check does, and a
// claimed cell has exactly one winner -- so a claim that is never
// checked stays on that cell for the rest of the run, and no agent
// with a higher index can ever enter it. Decide whether the move is
// possible BEFORE claiming, not after. (Physarum's first wall fix
// decided after, and the edge column silted up with stale claims.)
// A test reads the buffer back after a run and asserts it is empty.
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
@group(0) @binding(4) var field_in: texture_2d_array<f32>;
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
@group(0) @binding(4) var field_in: texture_2d_array<f32>;
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
@group(0) @binding(3) var field_out: texture_storage_2d_array<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d_array<f32>;

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
    // A pyramid level is a one-layer array: always layer 0.
    textureStore(field_out, p, 0, acc);
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
@group(0) @binding(4) var field_in: texture_2d_array<f32>;
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
        let v = minmax_ord(textureLoad(field_in, p, sim_layer(), 0).x);
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
@group(0) @binding(4) var field_in: texture_2d_array<f32>;
@group(0) @binding(5) var palette_tex: texture_2d<f32>;
// The signed distance field, when the matte's edge is Distance; a 1x1
// dummy otherwise, which sim_sdf never reads.
@group(0) @binding(6) var sdf_tex: texture_2d<f32>;

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

// Figure or background: 1 for a cell that is part of the picture, 0
// for one the tonemap should composite the background colour into,
// and the feather in between. Multiplied into whatever coverage the
// colouring returned, so a colouring that already reports empty
// cells (label's unlabelled ones) keeps saying so.
//
// The channel select is unrolled because WGSL cannot index a vec4 by
// a runtime value -- the same shape the `channel` colouring uses.
// The signed distance at a cell, in cells, positive inside the
// figure. Only read when a distance field was built this frame -- the
// matte's edge asked for one, or a colouring did -- the branch is on
// a uniform, so it is free, and it keeps the dummy binding unread.
fn sim_sdf(p: vec2<i32>) -> f32 {
    if (params.matte_b.y < 0.5) {
        return 0.0;
    }
    let g = sim_grid();
    return textureLoad(sdf_tex, clamp(p, vec2<i32>(0, 0), g - vec2<i32>(1, 1)), 0).x;
}

fn sim_matte(s: vec4<f32>, d: f32) -> f32 {
    return sim_matte_of(s, d, params.matte, params.matte_b.x);
}

// The same, for a matte given as a value: the colour stack's layers
// each carry their own.
fn sim_matte_of(s: vec4<f32>, d: f32, matte: vec4<f32>, edge: f32) -> f32 {
    let mode = matte.y;
    if (mode < 0.5) {
        return 1.0;
    }
    if (edge >= 0.5) {
        // Distance: the jump flood already folded the channel, the
        // cutoff and the direction into which side is inside, so d is
        // signed toward the figure and the feather is a width in
        // cells, centred on the edge.
        let soft = matte.w;
        if (soft <= 0.0) {
            return select(0.0, 1.0, d >= 0.0);
        }
        return clamp(d / soft + 0.5, 0.0, 1.0);
    }
    let which = i32(round(clamp(matte.x, 0.0, 3.0)));
    var v = s.x;
    if (which == 1) { v = s.y; }
    else if (which == 2) { v = s.z; }
    else if (which == 3) { v = s.w; }

    let cutoff = matte.z;
    let soft = matte.w;
    var a: f32;
    if (soft <= 0.0) {
        a = select(0.0, 1.0, v >= cutoff);
    } else {
        // Centred on the cutoff, so widening the feather does not
        // move the edge.
        a = clamp((v - cutoff) / soft + 0.5, 0.0, 1.0);
    }
    return select(a, 1.0 - a, mode >= 1.5);
}

// Everything a colouring may read at one sample point (derived-fields
// plan, section 1). Built per CELL by sim_sample and blended by the
// resolve, so a colouring never reads a neighbour itself: every
// derived quantity is computed at the taps and interpolated exactly
// as the state is. Each is zero unless the colouring declares the
// feature that pays for it.
struct SimSample {
    // The state.
    s: vec4<f32>,
    // d/dx and d/dy of every channel, central differences. NeedsGradient.
    gx: vec4<f32>,
    gy: vec4<f32>,
    // Signed distance to the matte's edge, in cells, positive inside.
    // NeedsDistance, or the matte's own Distance edge.
    dist: f32,
    // Structure tensor of channel .x over a 3x3 binomial window:
    // (Jxx, Jxy, Jyy). NeedsStructure.
    tensor: vec3<f32>,
};

// The gradient of one channel, from a sample.
fn sim_grad_of(x: SimSample, c: i32) -> vec2<f32> {
    if (c == 1) { return vec2<f32>(x.gx.y, x.gy.y); }
    if (c == 2) { return vec2<f32>(x.gx.z, x.gy.z); }
    if (c == 3) { return vec2<f32>(x.gx.w, x.gy.w); }
    return vec2<f32>(x.gx.x, x.gy.x);
}

fn sim_sample(p: vec2<i32>) -> SimSample {
    var x: SimSample;
    x.s = sim_read(p);
//__GRADIENT__
    x.dist = sim_sdf(p);
//__TENSOR__
    return x;
}

fn sim_sample_zero() -> SimSample {
    var x: SimSample;
    x.s = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    x.gx = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    x.gy = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    x.dist = 0.0;
    x.tensor = vec3<f32>(0.0, 0.0, 0.0);
    return x;
}

// acc + x * w, for the bicubic sum.
fn sim_sample_mad(acc: SimSample, x: SimSample, w: f32) -> SimSample {
    var r = acc;
    r.s = r.s + x.s * w;
    r.gx = r.gx + x.gx * w;
    r.gy = r.gy + x.gy * w;
    r.dist = r.dist + x.dist * w;
    r.tensor = r.tensor + x.tensor * w;
    return r;
}

fn sim_sample_lerp(a: SimSample, b: SimSample, t: f32) -> SimSample {
    var r: SimSample;
    r.s = mix(a.s, b.s, t);
    r.gx = mix(a.gx, b.gx, t);
    r.gy = mix(a.gy, b.gy, t);
    r.dist = mix(a.dist, b.dist, t);
    r.tensor = mix(a.tensor, b.tensor, t);
    return r;
}

// Colour and matte ONE sample. The resolve decides what sample that
// is: a cell's own under Nearest, a blend of its neighbours' under
// Bilinear and Bicubic -- which is the whole point of interpolating
// the STATE rather than the colours (derived-fields plan, section 2):
// every isoline of the field lands where the field crosses that
// level, as a crisp curve, and the matte's cutoff on an interpolated
// occupancy is a hard sub-cell boundary at the 0.5 isoline instead of
// a cell-wide ramp of half-drawn pixels.
//
// `p` is the cell the colouring is told it is at. Under interpolation
// that is the nearest cell, which no colouring reads unless it
// declares ReadsCell -- a test in app_repro_test greps for it, so one
// that starts to will fail there rather than draw subtly wrong
// pictures at 8x.
//__SHADE__

// Catmull-Rom weights for the four taps at -1, 0, +1, +2 around a
// sample at fraction t past tap 0. They sum to 1 for every t, so a
// constant field stays constant; the spline passes through the taps
// and is C1 across them.
fn sim_catmull_rom(t: f32) -> vec4<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return 0.5 * vec4<f32>(
        -t3 + 2.0 * t2 - t,
        3.0 * t3 - 5.0 * t2 + 2.0,
        -3.0 * t3 + 4.0 * t2 + t,
        t3 - t2,
    );
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
    // ... unless the fit is COVER, which fills the output and crops
    // the grid along the axis that does not fit.
    let fit = sim_fit_scale();
    let shown = vec2<f32>(g) * fit;
    let origin = (vec2<f32>(out_size) - shown) * 0.5;
    // Cell-centre mapping: pixel centre (o + 0.5) to grid space.
    // Sampling at the pixel's corner instead shifts the image half a
    // cell, which is invisible at 1:1 and obvious at 8x.
    let gf0 = (vec2<f32>(o) + vec2<f32>(0.5, 0.5) - origin) / fit;
    // The FRAME is decided before the view: a pixel in a letterbox
    // bar stays a bar at every magnification. The first version
    // tested the magnified coordinate, and a bar pixel that maps
    // outside the grid at 1x maps inside it once the view divides
    // its distance from the centre by m -- so the picture widened
    // into the bars over an octave and snapped back at the doubling.
    if (gf0.x < 0.0 || gf0.y < 0.0 || gf0.x >= vec2<f32>(g).x || gf0.y >= vec2<f32>(g).y) {
        textureStore(out_image, o, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    // The view: the octave mode's accumulated zoom, about the grid's
    // centre. 1 otherwise, and then this is gf0 exactly.
    let gc = vec2<f32>(g) * 0.5;
    let gf = gc + (gf0 - gc) / max(params.view.x, 1.0e-4);
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
    // Interpolate the STATE of the four surrounding cells, and the
    // gradient where the colouring wants one, then colour that once.
    // Blending four coloured cells instead -- the first version --
    // smeared palette entries across a magnified boundary and turned
    // a matte edge into a cell-wide ramp; see sim_shade_from.
    // Over the four surrounding CELL CENTRES, which is why the
    // half-cell shift is subtracted first.
    let f = gf - vec2<f32>(0.5, 0.5);
    let i0 = vec2<i32>(floor(f));
    let t = f - floor(f);
    let lim = g - vec2<i32>(1, 1);
    let p00 = clamp(i0, vec2<i32>(0, 0), lim);
    let p10 = clamp(i0 + vec2<i32>(1, 0), vec2<i32>(0, 0), lim);
    let p01 = clamp(i0 + vec2<i32>(0, 1), vec2<i32>(0, 0), lim);
    let p11 = clamp(i0 + vec2<i32>(1, 1), vec2<i32>(0, 0), lim);
    let x = sim_sample_lerp(
        sim_sample_lerp(sim_sample(p00), sim_sample(p10), t.x),
        sim_sample_lerp(sim_sample(p01), sim_sample(p11), t.x),
        t.y,
    );
    let col = sim_shade_from(x, clamp(vec2<i32>(floor(gf)), vec2<i32>(0, 0), lim));
"#
            .to_string(),
            SimUpscale::Bicubic => r#"
    // Catmull-Rom over the 4x4 surrounding cell centres: the state
    // interpolated with a C1 kernel, then coloured once. Sixteen
    // reads of state, and sixteen of the gradient where the colouring
    // asks for one. The weights are the standard Catmull-Rom
    // cardinal-spline weights for taps at -1, 0, +1, +2.
    let f = gf - vec2<f32>(0.5, 0.5);
    let i0 = vec2<i32>(floor(f));
    let t = f - floor(f);
    let lim = g - vec2<i32>(1, 1);
    let wx = sim_catmull_rom(t.x);
    let wy = sim_catmull_rom(t.y);
    var x = sim_sample_zero();
    for (var j = 0; j < 4; j = j + 1) {
        for (var i = 0; i < 4; i = i + 1) {
            let q = clamp(i0 + vec2<i32>(i - 1, j - 1), vec2<i32>(0, 0), lim);
            x = sim_sample_mad(x, sim_sample(q), wx[i] * wy[j]);
        }
    }
    let col = sim_shade_from(x, clamp(vec2<i32>(floor(gf)), vec2<i32>(0, 0), lim));
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

/// The jump flood's three passes. They read the grid, not its
/// boundary rule: a distance is measured within the grid, so at a
/// periodic seam it is measured to the seam. Stated on the matte's
/// tooltip rather than hidden.
pub fn assemble_jfa_init() -> String {
    splice(JFA_INIT_TEMPLATE, SimBoundary::Clamp, &[])
}
pub fn assemble_jfa_step() -> String {
    splice(JFA_STEP_TEMPLATE, SimBoundary::Clamp, &[])
}
pub fn assemble_jfa_final() -> String {
    splice(JFA_FINAL_TEMPLATE, SimBoundary::Clamp, &[])
}

/// The warp stage. Depends on the boundary alone.
pub fn assemble_warp(boundary: SimBoundary) -> String {
    splice(WARP_TEMPLATE, boundary, &[("//__SAMPLERS__", WARP_SAMPLERS)])
}

/// The layer map (simulation-layers plan, section 4): each layer read
/// through the flame's transform of the same index, at the layer's
/// rate. The flame's definitions come from
/// `ShaderBuilder::build_layer_map` -- bound at group 1, its `params`
/// renamed -- and the simulation's own `ff_atan2` is dropped from the
/// prelude, since the flame's utilities define the same function.
const LAYER_WARP_TEMPLATE: &str = r#"
//__COMMON__
@group(0) @binding(3) var field_out: texture_storage_2d_array<rgba32float, write>;
@group(0) @binding(4) var field_in: texture_2d_array<f32>;

//__BOUNDARY__

//__SAMPLERS__

//__FLAME__

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let g = sim_grid();
    let p = vec2<i32>(gid.xy);
    if (p.x >= g.x || p.y >= g.y) {
        return;
    }
    // The grid on the unit plane: the centre at the origin, the short
    // axis spanning [-1, 1], as a flame's default view.
    let gf = vec2<f32>(g);
    let half = min(gf.x, gf.y) * 0.5;
    let centre = (gf - vec2<f32>(1.0, 1.0)) * 0.5;
    let q = vec2<f32>(p);
    let u = (q - centre) / half;
    // A stream per (cell, step, seed), for the variations that draw.
    let seed = u32(p.x) * 7919u + u32(p.y) * 104729u + params.step_index * 65537u + params.seed_lo;
    let mapped = centre + flame_map(u32(sim_layer()), u, seed) * half;
    // The rate: how much of the map is applied per step; the source
    // coordinate is a backward resample, so the map says where this
    // cell reads FROM.
    let src = mix(q, mapped, params.xform.x);
    var v: vec4<f32>;
    if (params.warp_b.y >= 1.5) {
        v = warp_bicubic(src);
    } else if (params.warp_b.y >= 0.5) {
        v = sim_read(vec2<i32>(floor(src + vec2<f32>(0.5, 0.5))));
    } else {
        v = warp_bilinear(src);
    }
    textureStore(field_out, p, sim_layer(), v);
}
"#;

/// The layer-map warp for one flame: `flame_defs` is what
/// `ShaderBuilder::build_layer_map` produced.
pub fn assemble_layer_warp(boundary: SimBoundary, flame_defs: &str) -> String {
    let src = splice(
        LAYER_WARP_TEMPLATE,
        boundary,
        &[("//__SAMPLERS__", WARP_SAMPLERS), ("//__FLAME__", flame_defs)],
    );
    // Drop the prelude's ff_atan2: the flame's utilities carry one.
    let start = src.find("fn ff_atan2(y: f32, x: f32) -> f32 {").expect("the prelude defines ff_atan2");
    let end = src[start..].find("\n}\n").expect("ff_atan2 closes") + start + 3;
    format!("{}{}", &src[..start], &src[end..])
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
/// `pass` indexes the dispatches of one step ([`ModelDef::passes`],
/// at most [`crate::sim::MAX_PASSES`]): 0 calls `sim_step`, 1 calls
/// `sim_step2`, and so on. Every module carries the model's WHOLE
/// `wgsl` -- so a helper written once is visible to all of them -- and
/// they differ only in which function the entry point calls.
pub fn assemble_step(model: &ModelDef, boundary: SimBoundary, pass: u32) -> String {
    assemble_step_coupled(model, boundary, pass, false)
}

/// The coupling accessors, spliced only into a coupled config's step
/// shaders: an uncoupled config's shader is byte-for-byte what it was.
const COUPLING_ACCESSORS: &str = r#"
// One coupling: layer `from` drives layer `to` by `form` at
// `strength`, on the channels in `mask` (simulation-layers plan,
// section 3).
struct SimCouplingGpu {
    to_layer: u32,
    from_layer: u32,
    form: u32,
    mask: u32,
    strength: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
};
@group(0) @binding(17) var<storage, read> couplings: array<SimCouplingGpu>;

// Another layer's value at this cell, through the boundary rule.
fn sim_read_layer(l: i32, p: vec2<i32>) -> vec4<f32> {
    let g = sim_grid();
    if (sim_outside(p, g)) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    return textureLoad(field_in, sim_wrap_sized(p, g), l, 0);
}

// The sum of the coupling terms aimed at this layer, per channel,
// with u this layer's value and v the driving layer's. Added to the
// rule's result times dt, after the rule's own clamp.
fn sim_coupling(u: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let n = params.coupling_count;
    for (var i = 0u; i < n; i = i + 1u) {
        let c = couplings[i];
        if (c.to_layer != params.layer) {
            continue;
        }
        let v = sim_read_layer(i32(c.from_layer), p);
        var term = v - u;
        if (c.form == 1u) {
            term = u * v * (v - u);
        } else if (c.form == 2u) {
            term = v * v - u * u;
        } else if (c.form == 3u) {
            term = u * v;
        }
        let on = vec4<bool>(
            (c.mask & 1u) != 0u, (c.mask & 2u) != 0u, (c.mask & 4u) != 0u, (c.mask & 8u) != 0u,
        );
        acc = acc + c.strength * select(vec4<f32>(0.0, 0.0, 0.0, 0.0), term, on);
    }
    return acc;
}
"#;

/// A step shader, with the coupling applied to the model's LAST pass
/// when `coupled`.
pub fn assemble_step_coupled(model: &ModelDef, boundary: SimBoundary, pass: u32, coupled: bool) -> String {
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
    return kernel_lut[params.kernel_offset + i];
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
    // sim_step, sim_step2, sim_step3, ... -- the model writes as many
    // as it declares passes, and every module carries the model's
    // whole WGSL so a helper written once is visible to all of them.
    let entry = if pass == 0 {
        "sim_step".to_string()
    } else {
        format!("sim_step{}", pass + 1)
    };
    let last = pass + 1 == model.passes;
    let call = if coupled && last {
        format!(
            "    let s_in = textureLoad(field_in, p, sim_layer(), 0);\n    textureStore(field_out, p, sim_layer(), {entry}(s_in, p) + sim_dt() * sim_coupling(s_in, p));"
        )
    } else {
        format!(
            "    textureStore(field_out, p, sim_layer(), {entry}(textureLoad(field_in, p, sim_layer(), 0), p));"
        )
    };
    let coupling = if coupled { COUPLING_ACCESSORS } else { "" };
    splice(
        STEP_TEMPLATE,
        boundary,
        &[
            ("//__MODEL__", &format!("{rng_note}{}", model.wgsl)),
            ("//__STEP_CALL__", &call),
            ("//__COUPLING__", coupling),
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
        case 0: { return textureLoad(field_in, w, sim_layer(), 0).x; }
        case 1: { return textureLoad(pyr1, w, 0).x; }
        case 2: { return textureLoad(pyr2, w, 0).x; }
        case 3: { return textureLoad(pyr3, w, 0).x; }
        case 4: { return textureLoad(pyr4, w, 0).x; }
        case 5: { return textureLoad(pyr5, w, 0).x; }
        case 6: { return textureLoad(pyr6, w, 0).x; }
        default: { return textureLoad(pyr7, w, 0).x; }
    }
}

// The same reads, all four channels: for a model whose channels are
// four fields each wanting its own averages. The pyramid is RGBA and
// its downsample already carries every channel; only the reads took
// `.x`.
fn pyr_load4_sized(l: i32, q: vec2<i32>, g: vec2<i32>) -> vec4<f32> {
    if (sim_outside(q, g)) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let w = sim_wrap_sized(q, g);
    switch l {
        case 0: { return textureLoad(field_in, w, sim_layer(), 0); }
        case 1: { return textureLoad(pyr1, w, 0); }
        case 2: { return textureLoad(pyr2, w, 0); }
        case 3: { return textureLoad(pyr3, w, 0); }
        case 4: { return textureLoad(pyr4, w, 0); }
        case 5: { return textureLoad(pyr5, w, 0); }
        case 6: { return textureLoad(pyr6, w, 0); }
        default: { return textureLoad(pyr7, w, 0); }
    }
}

fn pyr_level_avg4(l: i32, pos: vec2<f32>) -> vec4<f32> {
    let s = f32(1 << u32(l));
    let f = (pos - vec2<f32>(0.5, 0.5)) / s;
    let f0 = floor(f);
    let t = f - f0;
    let i0 = vec2<i32>(f0);
    let g = pyr_sizes[l];
    let a = pyr_load4_sized(l, i0, g);
    let b = pyr_load4_sized(l, i0 + vec2<i32>(1, 0), g);
    let c = pyr_load4_sized(l, i0 + vec2<i32>(0, 1), g);
    let d = pyr_load4_sized(l, i0 + vec2<i32>(1, 1), g);
    return mix(mix(a, b, t.x), mix(c, d, t.x), t.y);
}

fn pyr_sample4(level: f32, pos: vec2<f32>) -> vec4<f32> {
    let top = f32(pyr_top_cached);
    let lf = clamp(level, 0.0, top);
    let l0 = i32(floor(lf));
    let l1 = min(l0 + 1, i32(top));
    let t = lf - floor(lf);
    return mix(pyr_level_avg4(l0, pos), pyr_level_avg4(l1, pos), t);
}

// Bilinear within level l at a position given in BASE cells (a cell
// centre is p + 0.5). Four loads. `FLOAT32_FILTERABLE` is optional
// and never requested, so the filtering is written out.
// Texel i of level l is centred on BASE CELL i * 2^l -- the decimation
// above blurs about source cell 2p -- so its centre in base
// coordinates is i * s + 0.5, and the fractional texel index of a
// base position is (pos - 0.5) / s. The first version wrote
// pos / s - 0.5, which assumes a texel centred at (i + 0.5) * s: an
// error of (s - 1) / 2 cells along BOTH axes, growing with the level,
// so an activator read at one level and an inhibitor at the next were
// averaged about points a cell or more apart on the diagonal. Found
// by the coupled Turing lattice, whose ring amplifies any such bias
// into stripes that all run one way and travel (3, 2) cells per 50
// steps; `lattice4_ring_does_not_drift` pins it.
fn pyr_level_avg(l: i32, pos: vec2<f32>) -> f32 {
    let s = f32(1 << u32(l));
    let f = (pos - vec2<f32>(0.5, 0.5)) / s;
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
    let prev = (params.minmax_slot + 257u - params.minmax_back) % 257u;
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
    let (gradient, tensor) = sample_splices(&[coloring]);
    splice(
        COLOR_TEMPLATE,
        boundary,
        &[
            ("//__COLORING__", coloring.wgsl),
            ("//__SHADE__", SINGLE_SHADE),
            ("//__RESOLVE__", &resolve),
            ("//__GRADIENT__", gradient),
            ("//__TENSOR__", tensor),
        ],
    )
}


/// The gradient splice of `sim_sample`: four reads giving every
/// channel's gradient, or zero.
const GRADIENT_ON: &str = r#"    // Central-difference gradient of every channel: the same four
    // reads give all four.
    let gr = sim_read(p + vec2<i32>(1, 0));
    let gl = sim_read(p - vec2<i32>(1, 0));
    let gu = sim_read(p + vec2<i32>(0, 1));
    let gd = sim_read(p - vec2<i32>(0, 1));
    x.gx = (gr - gl) * 0.5;
    x.gy = (gu - gd) * 0.5;"#;
const GRADIENT_OFF: &str = r#"    x.gx = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    x.gy = vec4<f32>(0.0, 0.0, 0.0, 0.0);"#;
/// The structure-tensor splice: a 3x3 binomial window of gradient
/// outer products, or zero.
const TENSOR_ON: &str = r#"    // Structure tensor of .x: the gradient's outer product, summed
    // over a 3x3 window with binomial weights (1 2 1)/4 each way. The
    // smoothing is what makes it a tensor of the TEXTURE rather than
    // of one cell's slope.
    var jxx = 0.0;
    var jxy = 0.0;
    var jyy = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let q = p + vec2<i32>(i, j);
            let g = vec2<f32>(
                sim_read(q + vec2<i32>(1, 0)).x - sim_read(q - vec2<i32>(1, 0)).x,
                sim_read(q + vec2<i32>(0, 1)).x - sim_read(q - vec2<i32>(0, 1)).x,
            ) * 0.5;
            let w = f32((2 - abs(i)) * (2 - abs(j))) / 16.0;
            jxx = jxx + w * g.x * g.x;
            jxy = jxy + w * g.x * g.y;
            jyy = jyy + w * g.y * g.y;
        }
    }
    x.tensor = vec3<f32>(jxx, jxy, jyy);"#;
const TENSOR_OFF: &str = "    x.tensor = vec3<f32>(0.0, 0.0, 0.0);";

/// The single colouring's shade: colour, then the config's matte.
const SINGLE_SHADE: &str = r#"fn sim_shade_from(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    var col = sim_color(x, p);
    col.a = col.a * sim_matte(x.s, x.dist);
    return col;
}

// One cell, as itself.
fn sim_shade(p: vec2<i32>) -> vec4<f32> {
    return sim_shade_from(sim_sample(p), p);
}"#;

/// The gradient and tensor splices for a set of colourings: computed
/// when ANY of them declares the feature.
fn sample_splices(colorings: &[&SimColoringDef]) -> (&'static str, &'static str) {
    let gradient = if colorings.iter().any(|c| c.has(ColoringFeature::NeedsGradient)) {
        GRADIENT_ON
    } else {
        GRADIENT_OFF
    };
    let tensor = if colorings.iter().any(|c| c.has(ColoringFeature::NeedsStructure)) {
        TENSOR_ON
    } else {
        TENSOR_OFF
    };
    (gradient, tensor)
}

/// Replace every whole-word occurrence of `word` with `with` (WGSL
/// identifier characters on neither side).
fn replace_word(src: &str, word: &str, with: &str) -> String {
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 64);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(word.as_bytes())
            && (i == 0 || !is_ident(bytes[i - 1]))
            && (i + word.len() == bytes.len() || !is_ident(bytes[i + word.len()]))
        {
            out.push_str(with);
            i += word.len();
        } else {
            let ch = src[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// A colouring's WGSL with every function it defines -- and its
/// `cparam` -- suffixed `_k`, so K colourings share one shader, and
/// one colouring can appear in the stack twice.
fn suffix_coloring(wgsl: &str, k: usize) -> String {
    let mut names: Vec<String> = Vec::new();
    for line in wgsl.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("fn ") {
            if let Some(end) = rest.find('(') {
                let name = rest[..end].trim();
                if !name.is_empty() && !names.iter().any(|n| n == name) {
                    names.push(name.to_string());
                }
            }
        }
    }
    let mut out = wgsl.to_string();
    for name in &names {
        out = replace_word(&out, name, &format!("{name}_{k}"));
    }
    replace_word(&out, "cparam", &format!("cparam_{k}"))
}

/// The colour stack: K colourings, each of its own source layer, with
/// its own parameters and matte, composited bottom to top.
pub fn assemble_color_stack(
    colorings: &[&SimColoringDef],
    boundary: SimBoundary,
    up: SimUpscale,
    down: SimDownscale,
    magnifying: bool,
) -> String {
    let resolve = resolve_body(up, down, magnifying);
    let (gradient, tensor) = sample_splices(colorings);
    let mut defs = String::from(
        r#"// One colouring layer of the stack (simulation-layers plan,
// section 5): which simulation layer it reads, how it blends, its
// opacity, its matte.
struct SimColorLayerGpu {
    source: u32,
    blend: u32,
    enabled: u32,
    pad0: u32,
    opacity: f32,
    edge: f32,
    pad1: f32,
    pad2: f32,
    matte: vec4<f32>,
};
@group(0) @binding(7) var<storage, read> color_layers: array<SimColorLayerGpu>;

// Composite `top` over `base` by `mode`: separable formulas on
// straight RGB, the layer's coverage times its opacity as its alpha,
// coverage accumulating as "over". A bottom layer over nothing is
// itself, exactly -- so a stack of one Normal layer at opacity 1 is
// the single colouring's picture bit for bit.
fn sim_blend(base: vec4<f32>, top: vec4<f32>, mode: u32, opacity: f32) -> vec4<f32> {
    let a = clamp(top.a * opacity, 0.0, 1.0);
    if (base.a <= 0.0) {
        return vec4<f32>(top.rgb, a);
    }
    var f = top.rgb;
    if (mode == 1u) {
        f = max(base.rgb, top.rgb);
    } else if (mode == 2u) {
        f = min(base.rgb, top.rgb);
    } else if (mode == 3u) {
        f = base.rgb * top.rgb;
    } else if (mode == 4u) {
        f = vec3<f32>(1.0, 1.0, 1.0) - (vec3<f32>(1.0, 1.0, 1.0) - base.rgb) * (vec3<f32>(1.0, 1.0, 1.0) - top.rgb);
    } else if (mode == 5u) {
        let lo = 2.0 * base.rgb * top.rgb;
        let hi = vec3<f32>(1.0, 1.0, 1.0) - 2.0 * (vec3<f32>(1.0, 1.0, 1.0) - base.rgb) * (vec3<f32>(1.0, 1.0, 1.0) - top.rgb);
        f = select(hi, lo, base.rgb < vec3<f32>(0.5, 0.5, 0.5));
    } else if (mode == 6u) {
        f = min(base.rgb + top.rgb, vec3<f32>(1.0, 1.0, 1.0));
    }
    let blended = mix(top.rgb, f, base.a);
    let out_a = a + base.a * (1.0 - a);
    let rgb = (blended * a + base.rgb * base.a * (1.0 - a)) / max(out_a, 1.0e-6);
    return vec4<f32>(rgb, out_a);
}
"#,
    );
    for (k, c) in colorings.iter().enumerate() {
        defs.push_str(&format!(
            "
// ---- colouring layer {k}: {} ----
fn cparam_{k}(i: u32) -> f32 {{
    return coloring_params[{k}u * 16u + i];
}}
",
            c.name
        ));
        defs.push_str(&suffix_coloring(c.wgsl, k));
        defs.push_str(&format!(
            r#"
fn sim_shade_from_{k}(x: SimSample, p: vec2<i32>) -> vec4<f32> {{
    var col = sim_color_{k}(x, p);
    col.a = col.a * sim_matte_of(x.s, x.dist, color_layers[{k}].matte, color_layers[{k}].edge);
    return col;
}}
fn sim_shade_{k}(p: vec2<i32>) -> vec4<f32> {{
    return sim_shade_from_{k}(sim_sample(p), p);
}}
fn sim_resolve_{k}(gf: vec2<f32>, g: vec2<i32>, fit: f32) -> vec4<f32> {{
{}
    return col;
}}
"#,
            replace_word(&replace_word(&resolve, "sim_shade", &format!("sim_shade_{k}")), "sim_shade_from", &format!("sim_shade_from_{k}"))
        ));
    }
    let mut composite = String::from("    var col = vec4<f32>(0.0, 0.0, 0.0, 0.0);
");
    for k in 0..colorings.len() {
        composite.push_str(&format!(
            "    if (color_layers[{k}].enabled != 0u) {{
        sim_layer_offset = i32(color_layers[{k}].source);
        col = sim_blend(col, sim_resolve_{k}(gf, g, fit), color_layers[{k}].blend, color_layers[{k}].opacity);
    }}
"
        ));
    }
    splice(
        COLOR_TEMPLATE,
        boundary,
        &[
            ("//__COLORING__", &defs),
            ("//__SHADE__", ""),
            ("//__RESOLVE__", &composite),
            ("//__GRADIENT__", gradient),
            ("//__TENSOR__", tensor),
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
    fn the_jump_flood_validates() {
        validate(&assemble_jfa_init(), "jfa init");
        validate(&assemble_jfa_step(), "jfa step");
        validate(&assemble_jfa_final(), "jfa final");
    }

    #[test]
    fn the_warp_validates_under_every_boundary() {
        for b in [SimBoundary::Periodic, SimBoundary::Clamp, SimBoundary::Zero, SimBoundary::Mirror] {
            validate(&assemble_warp(b), &format!("warp {b:?}"));
        }
    }

    /// Every model's step shaders validate with the coupling spliced
    /// in (simulation-layers plan, section 3), and an uncoupled
    /// shader is byte-for-byte the shader it was.
    #[test]
    fn every_model_validates_coupled_and_uncoupled_is_unchanged() {
        for m in MODELS {
            for pass in 0..m.passes {
                validate(
                    &assemble_step_coupled(m, SimBoundary::Periodic, pass, true),
                    &format!("coupled step {}/{pass}", m.name),
                );
                assert_eq!(
                    assemble_step_coupled(m, SimBoundary::Periodic, pass, false),
                    assemble_step(m, SimBoundary::Periodic, pass),
                    "{}: the uncoupled shader must be unchanged",
                    m.name
                );
                assert!(
                    !assemble_step(m, SimBoundary::Periodic, pass).contains("sim_coupling"),
                    "{}: an uncoupled shader carries the coupling",
                    m.name
                );
            }
        }
    }


    /// Every registered variation validates in the layer-map warp:
    /// one flame per variation, its transform carrying that variation
    /// alone, through `build_layer_map` and `assemble_layer_warp`.
    #[test]
    fn every_variation_validates_in_the_layer_warp() {
        let registry = crate::variations::global_registry();
        let builder = crate::shader_builder_v2::ShaderBuilder::new(registry.clone());
        let mut count = 0;
        for name in registry.names() {
            let mut flame = crate::scene::transforms::Flame::default();
            flame.transforms.clear();
            let mut t = crate::scene::transforms::Transform::default();
            t.variations.clear();
            t.variations.insert(name.to_string(), 1.0);
            flame.transforms.push(t);
            let defs = builder.build_layer_map(&flame);
            validate(&assemble_layer_warp(SimBoundary::Periodic, &defs), &format!("layer warp {name}"));
            count += 1;
        }
        println!("{count} variations validate in the layer warp");
        assert!(count > 100);
    }


    /// The colour stack validates: every colouring stacked with itself
    /// (the renaming must let one colouring appear twice) and all of
    /// them at once.
    #[test]
    fn every_colouring_validates_in_a_stack() {
        for c in COLORINGS {
            validate(
                &assemble_color_stack(&[c, c], SimBoundary::Periodic, SimUpscale::Bicubic, SimDownscale::Box, true),
                &format!("stack {} x2", c.name),
            );
        }
        let all: Vec<&SimColoringDef> = COLORINGS.iter().copied().collect();
        validate(
            &assemble_color_stack(&all, SimBoundary::Clamp, SimUpscale::Nearest, SimDownscale::Box, false),
            "stack of every colouring",
        );
    }

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
            let has_reads = src.contains("x.gx = (gr - gl)");
            assert_eq!(
                has_reads,
                c.has(ColoringFeature::NeedsGradient),
                "{}: gradient reads present={has_reads} but NeedsGradient={}",
                c.name,
                c.has(ColoringFeature::NeedsGradient)
            );
            let has_tensor = src.contains("jxx = jxx + w * g.x * g.x");
            assert_eq!(
                has_tensor,
                c.has(ColoringFeature::NeedsStructure),
                "{}: tensor window present={has_tensor} but NeedsStructure={}",
                c.name,
                c.has(ColoringFeature::NeedsStructure)
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
