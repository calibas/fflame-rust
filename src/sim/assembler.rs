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
    _pad1: f32,
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
    match boundary {
        SimBoundary::Periodic => {
            r#"
fn sim_wrap(p: vec2<i32>) -> vec2<i32> {
    let g = sim_grid();
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
fn sim_read(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(field_in, sim_wrap(p), 0);
}
"#
        }
        SimBoundary::Clamp => {
            r#"
fn sim_read(p: vec2<i32>) -> vec4<f32> {
    let g = sim_grid();
    return textureLoad(field_in, clamp(p, vec2<i32>(0, 0), g - vec2<i32>(1, 1)), 0);
}
"#
        }
        SimBoundary::Zero => {
            r#"
fn sim_read(p: vec2<i32>) -> vec4<f32> {
    let g = sim_grid();
    if (p.x < 0 || p.y < 0 || p.x >= g.x || p.y >= g.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    return textureLoad(field_in, p, 0);
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
fn sim_read(p: vec2<i32>) -> vec4<f32> {
    let g = sim_grid();
    return textureLoad(field_in, vec2<i32>(sim_mirror1(p.x, g.x), sim_mirror1(p.y, g.y)), 0);
}
"#
        }
    }
}

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
    let entry = if pass == 0 { "sim_step" } else { "sim_step2" };
    let call = format!("    textureStore(field_out, p, {entry}(textureLoad(field_in, p, 0), p));");
    splice(
        STEP_TEMPLATE,
        boundary,
        &[
            ("//__MODEL__", &format!("{rng_note}{}", model.wgsl)),
            ("//__STEP_CALL__", &call),
            ("//__KERNEL__", kernel),
        ],
    )
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
