//! WGSL assembly for escape-time pipelines.
//!
//! Far simpler than `shader_builder_v2`: one formula + one coloring
//! spliced into a fixed template. Marker lines (`//__FORMULA__` etc.)
//! are replaced whole-line — the same directive-on-its-own-line
//! discipline the effect chain's include splicer uses. Feature-gated
//! code (bailout test, orbit accumulator) is spliced or omitted at
//! assembly time, so a Mandelbrot + smooth pipeline carries no
//! accumulator and a Kaliset pipeline carries no bailout test.
//!
//! ## Output contract
//!
//! The compute pass writes an `Rgba32Float` image shaped like the
//! flame accumulator: `rgb` = color in linear space, `a` = density
//! (1.0 everywhere — every pixel is "hit" exactly once). The tonemap
//! shader's Linear mode then applies exposure and its `pow(1/gamma)`
//! display encode; palette texels are decoded `pow(2.2)` here so the
//! round trip is identity at the default gamma — the same
//! `srgb_to_linear` convention the flame plot uses.

use super::fields::{FieldColoringDef, FieldDef};
use super::{ColoringDef, ColoringFeature, FormulaDef, FormulaFeature};

/// Number of vec4 slots for each param block in the uniform — 16 float
/// params per formula / per coloring. Mirrored in the Rust-side
/// `EscapeParamsGpu`; a def exceeding it logs in `pack_params`.
pub const PARAM_VEC4S: usize = 4;

const TEMPLATE: &str = r#"
// Escape-time compute pass (assembled per formula x coloring).
// See src/escape/assembler.rs for the output contract.

struct EscapeParams {
    center: vec2<f32>,     // view center (f64 cast -- phase-1 ceiling)
    julia_c: vec2<f32>,    // fixed c in Julia mode
    span: vec2<f32>,       // complex-plane extent of the viewport (x, y)
    rot_cs: vec2<f32>,     // (cos, sin) of view rotation
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,            // bit 0 = Julia mode; bits 1-2 = biomorph (0 off, 1 |Re|, 2 |Im|)
    bailout: f32,          // escape test threshold, SQUARED metric
    // First row this dispatch covers. The perturbed templates
    // chunk by ITERATION and leave this zero; the direct and field
    // templates have no per-pixel resume state, so they chunk by
    // ROW BAND instead - see EscapeRenderer::direct_rows_per_dispatch.
    tile_y0: u32,
    damping: vec2<f32>,    // Mann alpha (re, im); read only when compiled damped
    shade_flags: u32,      // 1 = relief slopes the WRAPPED value
    // Three scalars, NOT vec3<u32>: a vec3 aligns to 16 in std140 but
    // [u32; 3] aligns to 4 in Rust, and the two structs would size
    // differently (1248 vs 1232 -- caught as a bind-group validation
    // error the first time this ran).
    _pad_shade0: u32,
    _pad_shade1: u32,
    _pad_shade2: u32,
    fparams: array<vec4<f32>, 4>,  // formula params, slot-ordered
    cparams: array<vec4<f32>, 4>,  // coloring params, slot-ordered
    // CPU-derived formula data (FormulaDef::derived_data), vec4-packed.
    // Zero for formulas without the hook. Origami's fold-line table
    // lives here: identical for every pixel, so computing it per
    // thread was pure waste — and the var<private> line cache it
    // replaced cost enough occupancy to trip the TDR watchdog under
    // supersampling.
    fdata: array<vec4<f32>, 64>,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
// The coloring's scalar value, kept for the relief pass to
// finite-difference. Bound to a 1x1 dummy when shading is off, where
// every store but one falls out of bounds and WGSL discards it -- so
// the cost of always writing is a single dead store per pixel, and
// there is no second shader variant to keep in step.
@group(0) @binding(4) var height_tex: texture_storage_2d<r32float, write>;

// Terminal per-pixel iteration record (32 B/px), written on a pass
// that completes the pixel's iteration when params.flags bit 3 is
// set. The recolor pass re-runs coloring_map + palette lookup from
// this WITHOUT re-iterating -- the cache that makes palette, coloring
// and relief edits cheap. Field order mirrors OrbitSummary; the two
// bools and the period share one word.
struct IterResult {
    z: vec2<f32>,
    dz: vec2<f32>,
    accum: vec2<f32>,
    n: u32,
    // bit 0 escaped, bit 1 converged, bits 2.. detected period.
    tags: u32,
}
@group(0) @binding(5) var<storage, read_write> results: array<IterResult>;

fn fparam(i: u32) -> f32 {
    return params.fparams[i / 4u][i % 4u];
}

// One vec4 of CPU-derived formula data (FormulaDef::derived_data).
fn fdata4(i: u32) -> vec4<f32> {
    return params.fdata[i];
}

fn cparam(i: u32) -> f32 {
    return params.cparams[i / 4u][i % 4u];
}

// What the colorings read: the orbit's terminal state.
struct OrbitSummary {
    z: vec2<f32>,
    n: u32,
    escaped: bool,
    converged: bool,
    period: u32,       // detected cycle length, 0 = none
    dz: vec2<f32>,     // derivative orbit (seed value if not compiled)
}

// Complex multiply.
fn esc_cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// Complex power via polar form. Returns 0 at the origin — the correct
// limit for p > 0, and it keeps atan2 from ever seeing a zero pair
// (Metal fast-math gives garbage there; see CLAUDE.md). Formulas with
// a pole (negative p) must guard the origin themselves BEFORE calling.
fn esc_cpow(z: vec2<f32>, p: f32) -> vec2<f32> {
    let r2 = dot(z, z);
    if (r2 < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let theta = atan2(z.y, z.x) * p;
    let r = pow(r2, 0.5 * p);
    return r * vec2<f32>(cos(theta), sin(theta));
}

// Complex exponential. Re is clamped before exp (the plan's
// tetration overflow guard); the escaping families trip their
// escape test far below the clamp.
fn esc_cexp(z: vec2<f32>) -> vec2<f32> {
    let ex = exp(min(z.x, 80.0));
    return ex * vec2<f32>(cos(z.y), sin(z.y));
}

// Complex log. The origin (a log singularity) returns a large
// negative real deterministically instead of evaluating atan2 at a
// zero pair (the Metal fast-math hazard).
fn esc_clog(z: vec2<f32>) -> vec2<f32> {
    let r2 = dot(z, z);
    if (r2 < 1e-30) {
        return vec2<f32>(-34.5, 0.0);
    }
    return vec2<f32>(0.5 * log(r2), atan2(z.y, z.x));
}

// Complex sine / cosine. sinh/cosh overflow to inf for |Im| ≳ 89;
// the trig family's |Im| escape test fires long before.
fn esc_csin(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(z.x) * cosh(z.y), cos(z.x) * sinh(z.y));
}

fn esc_ccos(z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(cos(z.x) * cosh(z.y), -sin(z.x) * sinh(z.y));
}

// Complex division, pole-guarded: a near-zero denominator returns a
// huge value (trips the escape test next) instead of dividing toward
// inf/NaN.
fn esc_cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let d = dot(b, b);
    if (d < 1e-30) {
        return vec2<f32>(1e20, 0.0);
    }
    return vec2<f32>(a.x * b.x + a.y * b.y, a.y * b.x - a.x * b.y) / d;
}

//__FORMULA__

//__COLORING__

//__COLORING_ACCUM__

@compute @workgroup_size(8, 8, 1)
fn escape_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Row band: this dispatch covers [tile_y0, tile_y0 + dispatched).
    let py = gid.y + params.tile_y0;
    if (gid.x >= params.width || py >= params.height) {
        return;
    }

    // Pixel center -> complex plane: offset from view center, y flipped
    // (texture y grows down, Im grows up), then view rotation.
    let uv = (vec2<f32>(f32(gid.x), f32(py)) + vec2<f32>(0.5, 0.5))
        / vec2<f32>(f32(params.width), f32(params.height));
    var d = (uv - vec2<f32>(0.5, 0.5)) * params.span;
    d.y = -d.y;
    let rot = params.rot_cs;
    let pixel = params.center + vec2<f32>(
        d.x * rot.x - d.y * rot.y,
        d.x * rot.y + d.y * rot.x,
    );

    // Julia toggle: parameter plane iterates z from the formula's
    // critical-point seed with c = pixel; dynamical plane iterates z
    // from the pixel with c fixed. One flag, not a formula-list entry
    // (plan section 3).
    let is_julia = (params.flags & 1u) != 0u;
    var z = select(PARAM_PLANE_SEED, pixel, is_julia);
    //__C_DECL__

    //__ACCUM_DECL__
    //__PREV_DECL__

    var escaped = false;
    var converged = false;
    var period = 0u;
    var n = 0u;
    // Derivative-orbit seed: d z0/dz0 = 1 on the dynamical plane. On
    // the PARAMETER plane it is d z0/dc, which is 0 for a formula
    // seeded at a constant (Mandelbrot's z0 = 0) and 1 for one seeded
    // AT THE PIXEL, since there z0 IS c. Getting this wrong is not
    // subtle: a pixel-seeded formula would carry dz = 0 forever and
    // distance estimation would divide by 1e-30. Unread and
    // dead-code-eliminated unless the coloring uses it.
    var dz = select(DZ0_PARAM, vec2<f32>(1.0, 0.0), is_julia);
    //__PERIOD_DECL__
    //__INTERIOR_DECL__
    for (var i = 0u; i < params.max_iter; i = i + 1u) {
        //__STEP__
        //__ACCUM_UPDATE__
        //__CONVERGE_TEST__
        //__PERIOD_TEST__
        //__ESCAPE_TEST__
        //__INTERIOR_TEST__
    }
    if (!escaped) {
        n = params.max_iter;
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    // COVERAGE, not colour. A pixel that never escaped has no value
    // to show, so it is left ABSENT rather than painted black, and
    // the tonemap's background blend fills it -- which is what makes
    // the background colour apply to the interior, and what makes a
    // transparent export leave it transparent. Averaged by the
    // downsample like everything else, so the boundary antialiases
    // against the background instead of against black.
    var coverage = 0.0;
    // The relief pass slopes THIS, not the rendered colour: the value
    // before the palette, so a cycling palette's band edges are not
    // mistaken for cliffs. Interior pixels keep 0 -- flat, which puts
    // the rim light exactly on the set boundary.
    var height = 0.0;
    if (escaped || COLORING_COLORS_INTERIOR) {
        let summary = OrbitSummary(z, n, escaped, converged, period, dz);
        // `fract` so unbounded colorings cycle as they grow; a
        // bounded one clamps instead, or its top value (1.0) wraps to
        // the palette's bottom and the brightest points render
        // darkest. See ColoringFeature::Bounded.
        let raw = coloring_map(summary, accum_state);
        let t = select(fract(raw), clamp(raw, 0.0, 1.0), COLORING_IS_BOUNDED);
        // SHADING_BANDED picks the wrapped coordinate instead, which
        // turns every palette band into a step (the engraved look).
        height = select(raw, t, params.shade_flags == 1u);
        // textureSampleLevel: explicit LOD, legal in non-uniform
        // control flow (unlike textureSample) -- WASM-safe.
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
        coverage = 1.0;
    }

    if ((params.flags & 8u) != 0u) {
        results[py * params.width + gid.x] = IterResult(
            z, dz, accum_state, n,
            select(0u, 1u, escaped) | select(0u, 2u, converged) | (period << 2u),
        );
    }
    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(py)), vec4<f32>(rgb, coverage));
    textureStore(height_tex, vec2<i32>(i32(gid.x), i32(py)), vec4<f32>(height, 0.0, 0.0, 0.0));
}
"#;

/// The escape test spliced into the loop for escaping formulas.
/// The base metric is per-formula (plan §5.9): squared norm for the
/// polynomial families, RAW Re z for exp/tetration, RAW |Im z| for
/// trig/Collatz. Biomorph (Pickover) still overrides either at
/// runtime — a switch on every formula, not a formula (plan §3).
fn escape_test(metric: crate::escape::EscapeMetric) -> String {
    let base = match metric {
        crate::escape::EscapeMetric::NormSq => "dot(z, z)",
        crate::escape::EscapeMetric::Re => "z.x",
        crate::escape::EscapeMetric::AbsIm => "abs(z.y)",
    };
    format!(
        "        let bio = (params.flags >> 1u) & 3u;\n\
         \x20       var esc_metric = {base};\n\
         \x20       if (bio == 1u) {{ esc_metric = z.x * z.x; }}\n\
         \x20       if (bio == 2u) {{ esc_metric = z.y * z.y; }}\n\
         \x20       if (esc_metric > params.bailout) {{\n\
         \x20           escaped = true;\n\
         \x20           n = i + 1u;\n\
         \x20           break;\n\
         \x20       }}"
    )
}

const PERTURBED_TEMPLATE: &str = r#"
// Perturbation compute pass (Mandelbrot, scaled-f32 deltas + Zhuoran
// rebasing). See src/escape/assembler.rs and the phase-4 plan.
//
// The reference orbit Z_n (computed on the CPU in fixed-point) rides
// a storage buffer; each pixel iterates its DELTA from the reference
// in f32, scaled so w is in pixel units: delta = S*w with S = the
// complex-plane pixel spacing. The arbitrary-precision terms cancel
// (delta' = 2 Z w S + S^2 w^2 + S d0  =>  w' = 2 Z w + S w^2 + d0),
// so one CPU orbit serves every pixel.
//
// Rebasing (Zhuoran 2021): whenever |Z_m + delta| < |delta|, restart
// the reference index with delta <- Z_m + delta. This REPLACES glitch
// detection, and also handles the reference ending early (escape):
// wrap-to-zero is a mandatory rebase. Full z = Z_m + delta is
// reconstructed every iteration for this test - which is exactly what
// the colorings need, so the whole coloring registry works under
// perturbation unchanged.

struct EscapeParams {
    center: vec2<f32>,
    julia_c: vec2<f32>,
    span: vec2<f32>,
    rot_cs: vec2<f32>,
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,
    bailout: f32,
    // First row this dispatch covers. The perturbed templates
    // chunk by ITERATION and leave this zero; the direct and field
    // templates have no per-pixel resume state, so they chunk by
    // ROW BAND instead - see EscapeRenderer::direct_rows_per_dispatch.
    tile_y0: u32,
    damping: vec2<f32>,
    shade_flags: u32,
    _pad_shade0: u32,
    _pad_shade1: u32,
    _pad_shade2: u32,
    fparams: array<vec4<f32>, 4>,
    cparams: array<vec4<f32>, 4>,
    // CPU-derived formula data — see the direct template's header.
    fdata: array<vec4<f32>, 64>,
}

struct PerturbParams {
    // Pixel spacing S as an f32 (scaled-f32 rung; normal down to
    // 2^-126) and its reciprocal.
    s: f32,
    inv_s: f32,
    orbit_len: u32,   // usable entries in the reference orbit
    // bit 0: skip the S*w*w term (deep-linear regime, hoisted per the
    // plan). bit 1: Julia mode - c is constant, so the +d0 term drops
    // from the recurrence and seeds the delta instead (delta_0 =
    // pixel - center).
    flags: u32,
    // Pixel spacing as mantissa * 2^exponent for the floatexp rung -
    // computed symbolically from zoom_log2, valid at ANY depth (no
    // f32/f64 underflow anywhere).
    s_m: f32,
    s_e: i32,
    // (view center - reference center) in pixel-spacing units:
    // nonzero when the reference was relocated to a minibrot nucleus.
    ref_offset: vec2<f32>,
    // Chunked iteration: this dispatch covers [iter_start, iter_end).
    // A single unbounded dispatch at high max_iter is a Windows TDR
    // (driver reset kills the device; observed in the field as a
    // 0xc0000409 abort at 200k iterations deep). Chunks bound every
    // dispatch; per-pixel state rides binding 6 between them.
    iter_start: u32,
    iter_end: u32,
    // The reference orbit's own c, as plain f32. Only the maps whose
    // parameter MULTIPLIES read it -- Lambda's delta step carries a
    // factor of c, where z^2 + c's carries none, which is why every
    // earlier tier could ignore it. It lands in what used to be two
    // words of padding, so the uniform's size and layout are
    // unchanged. f32 is enough because it is a MULTIPLIER: only its
    // relative error matters, and that is the same 2^-24 the scaled
    // rung already accepts for the reference itself.
    ref_c: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
@group(0) @binding(4) var<storage, read> ref_orbit: array<vec2<f32>>;
@group(0) @binding(5) var<uniform> perturb: PerturbParams;
// Per-pixel iteration state for chunked dispatches (48 bytes/px).
struct IterState {
    w: vec2<f32>,       // scaled: w | floatexp: DF mantissa hi
    z: vec2<f32>,       // last full orbit value
    accum: vec2<f32>,   // coloring accumulator
    w_lo: vec2<f32>,    // floatexp DF mantissa lo (zero on the scaled rung)
    w_e: i32,           // floatexp exponent (unused by the scaled rung)
    m: u32,             // reference index
    // Iterations recorded at termination, with the escaped flag in the
    // high bit (max_iter is capped far below 2^31). Packing it here
    // frees a word for `i_at` and keeps the struct at 48 B/px.
    n_done: u32,
    // The iteration index this pixel ACTUALLY reached. A BLA skip is
    // allowed to run past the chunk's nominal end - see the skip
    // guard - so the next chunk resumes from here, not from the CPU's
    // iter_start. Without this the legal skip length would depend on
    // where chunk boundaries fell, and the rendered image with it.
    i_at: u32,
}
const ITER_ESCAPED_BIT: u32 = 0x80000000u;
@group(0) @binding(6) var<storage, read_write> iter_state: array<IterState>;
// BLA table (binding 7): iteration skipping. Level l holds entries
// each collapsing 2^(l+1) delta iterations to one affine application
// delta' = A*delta + B*delta_c, valid while |delta| < r. Extended
// range: mantissa pairs + i32 exponents (A overflows f32 across a
// long merged run). n_levels == 0 disables skipping (dummy buffer).
struct BlaEntry {
    a_m: vec2<f32>,
    b_m: vec2<f32>,
    a_e: i32,
    b_e: i32,
    r_m: f32,
    r_e: i32,
}
struct BlaBuf {
    // Start index of each level's entries, 32 levels max (a 2^33-step
    // skip ceiling -- far past any orbit).
    offsets: array<vec4<u32>, 8>,
    n_levels: u32,
    // Steps the table covers: the orbit prefix BEFORE the reference's
    // own escape (skips must never ride the escaped tail - n would
    // overshoot by the span).
    n_steps: u32,
    _p1: u32,
    _p2: u32,
    entries: array<BlaEntry>,
}
@group(0) @binding(7) var<storage, read> bla: BlaBuf;
// DF residuals of the reference orbit, parallel to binding 4: each
// entry's f64 tail below its f32 hi (~2^-48 relative reference
// values — the multiplier and rebase read both halves on the
// floatexp rung; the scaled rung ignores this binding).
@group(0) @binding(8) var<storage, read> ref_orbit_lo: array<vec2<f32>>;
// Per-entry binary exponent of the reference orbit, parallel to
// binding 4: entry m is (ref_orbit[m] + ref_orbit_lo[m]) * 2^e. It is
// 0 for every iterate above 2^-90 (the overwhelming majority), and
// nonzero only where the reference passes close to a nucleus - the
// iterates plain f32 flushes to zero, which would delete 2*Z*delta
// from that step and permanently halve delta growth from there on.
@group(0) @binding(9) var<storage, read> ref_orbit_e: array<i32>;
// See the direct template: the coloring's scalar value for the relief
// pass, bound to a 1x1 dummy when shading is off.
@group(0) @binding(10) var height_tex: texture_storage_2d<r32float, write>;
// |Z|² per reference entry as a DF pair (hi, lo), CPU-computed in f64.
// The escape margin needs (|Z|² - bailout) to better than f32 ulp --
// see the margin comment at the escape test.
@group(0) @binding(11) var<storage, read> ref_r2_buf: array<vec2<f32>>;

// Terminal per-pixel iteration record (32 B/px), written on a pass
// that completes the pixel's iteration when params.flags bit 3 is
// set. The recolor pass re-runs coloring_map + palette lookup from
// this WITHOUT re-iterating -- the cache that makes palette, coloring
// and relief edits cheap. Field order mirrors OrbitSummary; the two
// bools and the period share one word.
struct IterResult {
    z: vec2<f32>,
    dz: vec2<f32>,
    accum: vec2<f32>,
    n: u32,
    // bit 0 escaped, bit 1 converged, bits 2.. detected period.
    tags: u32,
}
@group(0) @binding(12) var<storage, read_write> results: array<IterResult>;

// The reference iterate as a plain f32 value (zero below f32's normal
// range - the pre-exponent behaviour, which is all the rebase test
// and the z_full reconstruction need).
fn ref_r2(m: u32) -> vec2<f32> {
    return ref_r2_buf[m];
}

fn ref_z(m: u32) -> vec2<f32> {
    let e = ref_orbit_e[m];
    if (e == 0) {
        return ref_orbit[m];
    }
    if (e < -126) {
        return vec2<f32>(0.0, 0.0);
    }
    return ref_orbit[m] * exp2(f32(e));
}

// The scaled DF tail of the same iterate.
fn ref_z_lo(m: u32) -> vec2<f32> {
    let e = ref_orbit_e[m];
    if (e == 0) {
        return ref_orbit_lo[m];
    }
    if (e < -126) {
        return vec2<f32>(0.0, 0.0);
    }
    return ref_orbit_lo[m] * exp2(f32(e));
}

// (a_m * 2^a_e) < (b_m * 2^b_e) on magnitudes; mantissas need not be
// pre-normalized. b_m <= 0 encodes "radius zero, never valid".
fn bla_mag_lt(a_m: f32, a_e: i32, b_m: f32, b_e: i32) -> bool {
    if (b_m <= 0.0) {
        return false;
    }
    if (a_m <= 0.0) {
        return true;
    }
    let fa = frexp(a_m);
    let fb = frexp(b_m);
    let ea = a_e + fa.exp;
    let eb = b_e + fb.exp;
    if (ea != eb) {
        return ea < eb;
    }
    return fa.fract < fb.fract;
}

fn cparam(i: u32) -> f32 {
    return params.cparams[i / 4u][i % 4u];
}

struct OrbitSummary {
    z: vec2<f32>,
    n: u32,
    escaped: bool,
    converged: bool,
    period: u32,
    dz: vec2<f32>,
}

// |X + x| - |X| computed exactly by sign case (laser blaster's
// diffabs; Kalles Fraktaler lineage): piecewise linear in the
// perturbation, no cancellation. Positively homogeneous, which is
// what lets the scaled form pass X/S and keep S factored out.
fn diffabs(X: f32, x: f32) -> f32 {
    if (X >= 0.0) {
        if (X + x >= 0.0) {
            return x;
        }
        return -(2.0 * X + x);
    }
    if (X + x > 0.0) {
        return 2.0 * X + x;
    }
    return -x;
}

//__COLORING__

//__COLORING_ACCUM__

@compute @workgroup_size(8, 8, 1)
fn escape_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }

    // Pixel offset from the view center in PIXEL units (order
    // +-height/2), rotated - this is d0, the c-perturbation in units
    // of S. Same y-flip and rotation as the direct template.
    let centered = vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5, 0.5)
        - 0.5 * vec2<f32>(f32(params.width), f32(params.height));
    var dpx = centered;
    dpx.y = -dpx.y;
    let rot = params.rot_cs;
    let d0 = vec2<f32>(
        dpx.x * rot.x - dpx.y * rot.y,
        dpx.x * rot.y + dpx.y * rot.x,
    ) + perturb.ref_offset;

    // Delta iteration state: w = delta / S, m = reference index.
    // Parameter plane: delta_0 = 0 (both orbits start at z = 0) and
    // the +d0 term generates delta_1 = delta_c. Julia: c is constant
    // (the term drops) and the SEED differs by the pixel offset.
    let is_julia_perturb = (perturb.flags & 2u) != 0u;
    var w = select(vec2<f32>(0.0, 0.0), d0, is_julia_perturb);
    let d0_term = select(d0, vec2<f32>(0.0, 0.0), is_julia_perturb);
    let px_index = gid.y * params.width + gid.x;
    var m = 0u;
    var z = vec2<f32>(0.0, 0.0);
    var escaped = false;
    var n = 0u;
    // Mutable on the perturbed path too: a Convergent formula
    // terminates on a settled orbit here exactly as it does on the
    // direct one. Non-convergent formulas splice nothing below, so
    // their shaders stay byte-identical and the compiler folds this
    // straight back to a constant.
    var converged = false;
    let period = 0u;
    let dz = vec2<f32>(1.0, 0.0);
    // The f32 value of c for the accumulator colorings (trap geometry
    // lives at O(1) scale, where f32 c is exact enough).
    let c_f32 = params.center;

    //__ACCUM_DECL__

    // Chunk resume: after the first chunk every register comes from
    // the state buffer; an already-escaped pixel just rewrites its
    // (final) color.
    // Previous-iteration delta, for two-term recurrences.
    // Unused, and dead-code-eliminated, for every other tier.
    //__W_PREV_INIT__
    var i = perturb.iter_start;
    if (perturb.iter_start > 0u) {
        let st = iter_state[px_index];
        w = st.w;
        // See the declaration: w_lo is the deep rung's field and is
        // free here, so the two-term history resumes through it.
        w_prev = st.w_lo;
        z = st.z;
        m = st.m;
        accum_state = st.accum;
        i = max(i, st.i_at);
        if ((st.n_done & ITER_ESCAPED_BIT) != 0u) {
            escaped = true;
            n = st.n_done & ~ITER_ESCAPED_BIT;
        }
    }

    while (i < perturb.iter_end && !escaped) {
        // BLA skip: from an aligned reference index, collapse the
        // longest valid run of iterations to one affine application.
        // m == 0 never validates (the Z_0 = 0 step has no linear
        // part, so its entry radius is 0 by construction).
        var did_skip = false;
        if (bla.n_levels > 0u && m > 0u) {
            // |delta| = |w| * S, held symbolically (s_m * 2^s_e).
            let aw = length(w);
            var d_m = 0.0;
            var d_e = -1000000;
            if (aw > 0.0) {
                let fw = frexp(aw);
                d_m = fw.fract * perturb.s_m;
                d_e = fw.exp + perturb.s_e;
            }
            var pick_a = vec2<f32>(0.0, 0.0);
            var pick_b = vec2<f32>(0.0, 0.0);
            var pick_ae = 0;
            var pick_be = 0;
            var pick_span = 0u;
            let m_left = perturb.orbit_len - 1u - m;
            // Bounded by the RENDER's end, not the chunk's: a skip
            // that crosses a chunk boundary is fine (i_at carries the
            // overshoot), and bounding it by the chunk would make the
            // image depend on the chunk size.
            let steps_left = params.max_iter - i;
            for (var l = 0u; l < bla.n_levels; l = l + 1u) {
                let span = 2u << l;
                if ((m & (span - 1u)) != 0u || span > m_left || span > steps_left
                    || m + span > bla.n_steps) {
                    break;
                }
                let ent = bla.entries[bla.offsets[l >> 2u][l & 3u] + (m >> (l + 1u))];
                if (aw > 0.0 && !bla_mag_lt(d_m, d_e, ent.r_m, ent.r_e)) {
                    break;
                }
                if (aw == 0.0 && ent.r_m <= 0.0) {
                    break;
                }
                pick_a = ent.a_m;
                pick_ae = ent.a_e;
                pick_b = ent.b_m;
                pick_be = ent.b_e;
                pick_span = span;
            }
            if (pick_span > 0u) {
                // w' = A*w + B*d0 (both dimensionless: the S factors
                // cancel exactly as in the per-step recurrence).
                // Exponents clamp at f32's edge; validity keeps the
                // PRODUCTS representable long before A alone is.
                let ta = vec2<f32>(
                    pick_a.x * w.x - pick_a.y * w.y,
                    pick_a.x * w.y + pick_a.y * w.x,
                ) * exp2(f32(clamp(pick_ae, -126, 126)));
                let tb = vec2<f32>(
                    pick_b.x * d0_term.x - pick_b.y * d0_term.y,
                    pick_b.x * d0_term.y + pick_b.y * d0_term.x,
                ) * exp2(f32(clamp(pick_be, -126, 126)));
                w = ta + tb;
                m = m + pick_span;
                i = i + pick_span;
                did_skip = true;
            }
        }
        if (!did_skip) {
            let z_ref = ref_z(m);
            //__DELTA_STEP__
            w = w_new;
            m = m + 1u;
            i = i + 1u;
        }
        let z_before = z;

        // Full orbit value: z = Z_m + S*w. S*w underflows to zero
        // while the delta is far below f32 - exactly when z == Z_m to
        // f32 precision anyway.
        let delta = perturb.s * w;
        let z_full = ref_z(min(m, perturb.orbit_len - 1u)) + delta;
        z = z_full;

        //__ACCUM_UPDATE__
        //__CONVERGE_TEST__

        // DELTA-AWARE escape test (biomorph is gated off on the
        // perturbed path). |z_full|² in plain f32 quantizes away the
        // per-pixel delta once 2·Z·δ drops below one ulp of the
        // bailout -- past zoom ~22 every pixel then inherits the
        // reference's rounded fate, which is what broke Feather (its
        // slow-growth boundary is DECIDED by those sub-ulp
        // differences; a chaos-amplified boundary never is, which is
        // why no earlier tier hit this). So the margin is formed in
        // parts that are each small or exact: r2.x - bailout is exact
        // near the threshold (both f32, within a factor of two --
        // Sterbenz), and r2.y + 2·Z·δ + |δ|² are all tiny.
        let mr = ref_r2(min(m, perturb.orbit_len - 1u));
        let zi_m = ref_z(min(m, perturb.orbit_len - 1u));
        let margin = (mr.x - params.bailout)
            + (mr.y + 2.0 * dot(zi_m, delta) + dot(delta, delta));
        if (margin > 0.0) {
            escaped = true;
            n = i;
            break;
        }

        // Zhuoran rebase: restart the reference index when the new
        // delta AGAINST THE ORBIT'S START would be smaller than the
        // current one, or when the reference orbit ended. The new
        // delta is z_full - Z_0: zero-start references (parameter
        // plane) reduce to the textbook z_full form, Julia references
        // start at Z_0 = center and MUST subtract it (found the hard
        // way: without the subtraction a rebase teleports z to
        // center + z_full and every pixel escapes instantly).
        //__REBASE__
    }
    if (perturb.iter_end < params.max_iter) {
        // More chunks follow: persist the registers.
        iter_state[px_index] = IterState(
            w, z, accum_state, w_prev, 0, m,
            select(0u, n | ITER_ESCAPED_BIT, escaped),
            i,
        );
    }
    if (!escaped) {
        n = params.max_iter;
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    // COVERAGE, not colour. A pixel that never escaped has no value
    // to show, so it is left ABSENT rather than painted black, and
    // the tonemap's background blend fills it -- which is what makes
    // the background colour apply to the interior, and what makes a
    // transparent export leave it transparent. Averaged by the
    // downsample like everything else, so the boundary antialiases
    // against the background instead of against black.
    var coverage = 0.0;
    // The relief pass slopes THIS, not the rendered colour: the value
    // before the palette, so a cycling palette's band edges are not
    // mistaken for cliffs. Interior pixels keep 0 -- flat, which puts
    // the rim light exactly on the set boundary.
    var height = 0.0;
    if (escaped || COLORING_COLORS_INTERIOR) {
        let summary = OrbitSummary(z, n, escaped, converged, period, dz);
        // `fract` so unbounded colorings cycle as they grow; a
        // bounded one clamps instead, or its top value (1.0) wraps to
        // the palette's bottom and the brightest points render
        // darkest. See ColoringFeature::Bounded.
        let raw = coloring_map(summary, accum_state);
        let t = select(fract(raw), clamp(raw, 0.0, 1.0), COLORING_IS_BOUNDED);
        // SHADING_BANDED picks the wrapped coordinate instead, which
        // turns every palette band into a step (the engraved look).
        height = select(raw, t, params.shade_flags == 1u);
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
        coverage = 1.0;
    }

    if ((params.flags & 8u) != 0u && perturb.iter_end >= params.max_iter) {
        results[px_index] = IterResult(
            z, dz, accum_state, n,
            select(0u, 1u, escaped) | select(0u, 2u, converged) | (period << 2u),
        );
    }
    // Mid-render, an UNFINISHED pixel keeps the previous frame's
    // content instead of painting itself black: a chunked render's
    // early frames used to flash black wherever iterations had not
    // finished (the whole frame, during a pan past the floatexp
    // threshold, where the TDR-safe chunk is far smaller than
    // max_iter). Escaped pixels rewrite their final colour every
    // chunk as before, and the LAST chunk writes everyone -- the
    // settled image is byte-identical.
    if (escaped || perturb.iter_end >= params.max_iter) {
        textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(rgb, coverage));
        textureStore(height_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(height, 0.0, 0.0, 0.0));
    }
}
"#;

const PERTURBED_FE_TEMPLATE: &str = r#"
// Perturbation compute pass, floatexp rung: deltas carried as
// SHARED-EXPONENT complex floatexp (vec2 mantissa + one i32
// exponent). Mantissa precision is that of f32 complex arithmetic -
// the accepted delta precision model - while the exponent range is
// unbounded, lifting the scaled-f32 rung's ~zoom-54 ceiling.
// Iteration cost is a few times the scaled rung; this template only
// runs past PERTURB_FLOATEXP_ZOOM.
//
// Same structure as the scaled template otherwise: Zhuoran rebasing
// against Z_0 (delta <- z_full - Z_0), full z reconstructed every
// iteration, the whole coloring registry spliced in unchanged.

struct EscapeParams {
    center: vec2<f32>,
    julia_c: vec2<f32>,
    span: vec2<f32>,
    rot_cs: vec2<f32>,
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,
    bailout: f32,
    // First row this dispatch covers. The perturbed templates
    // chunk by ITERATION and leave this zero; the direct and field
    // templates have no per-pixel resume state, so they chunk by
    // ROW BAND instead - see EscapeRenderer::direct_rows_per_dispatch.
    tile_y0: u32,
    damping: vec2<f32>,
    shade_flags: u32,
    _pad_shade0: u32,
    _pad_shade1: u32,
    _pad_shade2: u32,
    fparams: array<vec4<f32>, 4>,
    cparams: array<vec4<f32>, 4>,
    // CPU-derived formula data — see the direct template's header.
    fdata: array<vec4<f32>, 64>,
}

struct PerturbParams {
    s: f32,
    inv_s: f32,
    orbit_len: u32,
    flags: u32,
    s_m: f32,
    s_e: i32,
    // (view center - reference center) in pixel-spacing units:
    // nonzero when the reference was relocated to a minibrot nucleus.
    ref_offset: vec2<f32>,
    // Chunked iteration: this dispatch covers [iter_start, iter_end).
    // A single unbounded dispatch at high max_iter is a Windows TDR
    // (driver reset kills the device; observed in the field as a
    // 0xc0000409 abort at 200k iterations deep). Chunks bound every
    // dispatch; per-pixel state rides binding 6 between them.
    iter_start: u32,
    iter_end: u32,
    // The reference orbit's own c, as plain f32. Only the maps whose
    // parameter MULTIPLIES read it -- Lambda's delta step carries a
    // factor of c, where z^2 + c's carries none, which is why every
    // earlier tier could ignore it. It lands in what used to be two
    // words of padding, so the uniform's size and layout are
    // unchanged. f32 is enough because it is a MULTIPLIER: only its
    // relative error matters, and that is the same 2^-24 the scaled
    // rung already accepts for the reference itself.
    ref_c: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
@group(0) @binding(4) var<storage, read> ref_orbit: array<vec2<f32>>;
@group(0) @binding(5) var<uniform> perturb: PerturbParams;
// Per-pixel iteration state for chunked dispatches (48 bytes/px).
struct IterState {
    w: vec2<f32>,       // scaled: w | floatexp: DF mantissa hi
    z: vec2<f32>,       // last full orbit value
    accum: vec2<f32>,   // coloring accumulator
    w_lo: vec2<f32>,    // floatexp DF mantissa lo (zero on the scaled rung)
    w_e: i32,           // floatexp exponent (unused by the scaled rung)
    m: u32,             // reference index
    // Iterations recorded at termination, with the escaped flag in the
    // high bit (max_iter is capped far below 2^31). Packing it here
    // frees a word for `i_at` and keeps the struct at 48 B/px.
    n_done: u32,
    // The iteration index this pixel ACTUALLY reached. A BLA skip is
    // allowed to run past the chunk's nominal end - see the skip
    // guard - so the next chunk resumes from here, not from the CPU's
    // iter_start. Without this the legal skip length would depend on
    // where chunk boundaries fell, and the rendered image with it.
    i_at: u32,
    //__ITER_STATE_TAIL__
}
const ITER_ESCAPED_BIT: u32 = 0x80000000u;
@group(0) @binding(6) var<storage, read_write> iter_state: array<IterState>;
// BLA table (binding 7): iteration skipping. Level l holds entries
// each collapsing 2^(l+1) delta iterations to one affine application
// delta' = A*delta + B*delta_c, valid while |delta| < r. Extended
// range: mantissa pairs + i32 exponents (A overflows f32 across a
// long merged run). n_levels == 0 disables skipping (dummy buffer).
struct BlaEntry {
    a_m: vec2<f32>,
    b_m: vec2<f32>,
    a_e: i32,
    b_e: i32,
    r_m: f32,
    r_e: i32,
}
struct BlaBuf {
    // Start index of each level's entries, 32 levels max (a 2^33-step
    // skip ceiling -- far past any orbit).
    offsets: array<vec4<u32>, 8>,
    n_levels: u32,
    // Steps the table covers: the orbit prefix BEFORE the reference's
    // own escape (skips must never ride the escaped tail - n would
    // overshoot by the span).
    n_steps: u32,
    _p1: u32,
    _p2: u32,
    entries: array<BlaEntry>,
}
@group(0) @binding(7) var<storage, read> bla: BlaBuf;
// DF residuals of the reference orbit, parallel to binding 4: each
// entry's f64 tail below its f32 hi (~2^-48 relative reference
// values — the multiplier and rebase read both halves on the
// floatexp rung; the scaled rung ignores this binding).
@group(0) @binding(8) var<storage, read> ref_orbit_lo: array<vec2<f32>>;
// Per-entry binary exponent of the reference orbit, parallel to
// binding 4: entry m is (ref_orbit[m] + ref_orbit_lo[m]) * 2^e. It is
// 0 for every iterate above 2^-90 (the overwhelming majority), and
// nonzero only where the reference passes close to a nucleus - the
// iterates plain f32 flushes to zero, which would delete 2*Z*delta
// from that step and permanently halve delta growth from there on.
@group(0) @binding(9) var<storage, read> ref_orbit_e: array<i32>;
// See the direct template: the coloring's scalar value for the relief
// pass, bound to a 1x1 dummy when shading is off.
@group(0) @binding(10) var height_tex: texture_storage_2d<r32float, write>;
// |Z|² per reference entry as a DF pair (hi, lo), CPU-computed in f64.
// The escape margin needs (|Z|² - bailout) to better than f32 ulp --
// see the margin comment at the escape test.
@group(0) @binding(11) var<storage, read> ref_r2_buf: array<vec2<f32>>;

// Terminal per-pixel iteration record (32 B/px), written on a pass
// that completes the pixel's iteration when params.flags bit 3 is
// set. The recolor pass re-runs coloring_map + palette lookup from
// this WITHOUT re-iterating -- the cache that makes palette, coloring
// and relief edits cheap. Field order mirrors OrbitSummary; the two
// bools and the period share one word.
struct IterResult {
    z: vec2<f32>,
    dz: vec2<f32>,
    accum: vec2<f32>,
    n: u32,
    // bit 0 escaped, bit 1 converged, bits 2.. detected period.
    tags: u32,
}
@group(0) @binding(12) var<storage, read_write> results: array<IterResult>;

// The reference iterate as a plain f32 value (zero below f32's normal
// range - the pre-exponent behaviour, which is all the rebase test
// and the z_full reconstruction need).
fn ref_r2(m: u32) -> vec2<f32> {
    return ref_r2_buf[m];
}

fn ref_z(m: u32) -> vec2<f32> {
    let e = ref_orbit_e[m];
    if (e == 0) {
        return ref_orbit[m];
    }
    if (e < -126) {
        return vec2<f32>(0.0, 0.0);
    }
    return ref_orbit[m] * exp2(f32(e));
}

// The scaled DF tail of the same iterate.
fn ref_z_lo(m: u32) -> vec2<f32> {
    let e = ref_orbit_e[m];
    if (e == 0) {
        return ref_orbit_lo[m];
    }
    if (e < -126) {
        return vec2<f32>(0.0, 0.0);
    }
    return ref_orbit_lo[m] * exp2(f32(e));
}

// (a_m * 2^a_e) < (b_m * 2^b_e) on magnitudes; mantissas need not be
// pre-normalized. b_m <= 0 encodes "radius zero, never valid".
fn bla_mag_lt(a_m: f32, a_e: i32, b_m: f32, b_e: i32) -> bool {
    if (b_m <= 0.0) {
        return false;
    }
    if (a_m <= 0.0) {
        return true;
    }
    let fa = frexp(a_m);
    let fb = frexp(b_m);
    let ea = a_e + fa.exp;
    let eb = b_e + fb.exp;
    if (ea != eb) {
        return ea < eb;
    }
    return fa.fract < fb.fract;
}

fn cparam(i: u32) -> f32 {
    return params.cparams[i / 4u][i % 4u];
}

// ---- shared-exponent complex floatexp ----
// value = m * 2^e; normalized so max(|m.x|, |m.y|) is in [0.5, 1),
// or m == (0,0) with the ZERO_E sentinel (large negative, but far
// from i32::MIN so exponent arithmetic cannot wrap).
const CFE_ZERO_E: i32 = -1000000000;

struct CFe {
    m: vec2<f32>,
    e: i32,
}

fn cfe_norm(v: CFe) -> CFe {
    let a = max(abs(v.m.x), abs(v.m.y));
    if (a == 0.0) {
        return CFe(vec2<f32>(0.0, 0.0), CFE_ZERO_E);
    }
    let f = frexp(a);
    // a = f.fract * 2^f.exp with f.fract in [0.5, 1).
    return CFe(v.m * exp2(f32(-f.exp)), v.e + f.exp);
}

fn cfe_from_f32(v: vec2<f32>) -> CFe {
    return cfe_norm(CFe(v, 0));
}

fn cfe_to_f32(v: CFe) -> vec2<f32> {
    if (v.e < -126 || v.e == CFE_ZERO_E) {
        return vec2<f32>(0.0, 0.0);
    }
    if (v.e > 127) {
        // Far past any escape threshold; a huge finite stands in.
        return v.m * 3.0e38;
    }
    return v.m * exp2(f32(v.e));
}

// Multiply by an ordinary f32 complex (|b| bounded ~4: the 2Z term).
fn cfe_mul_c32(a: CFe, b: vec2<f32>) -> CFe {
    return cfe_norm(CFe(
        vec2<f32>(a.m.x * b.x - a.m.y * b.y, a.m.x * b.y + a.m.y * b.x),
        a.e,
    ));
}

fn cfe_mul(a: CFe, b: CFe) -> CFe {
    if (a.e == CFE_ZERO_E || b.e == CFE_ZERO_E) {
        return CFe(vec2<f32>(0.0, 0.0), CFE_ZERO_E);
    }
    return cfe_norm(CFe(
        vec2<f32>(a.m.x * b.m.x - a.m.y * b.m.y, a.m.x * b.m.y + a.m.y * b.m.x),
        a.e + b.e,
    ));
}

fn cfe_sqr(a: CFe) -> CFe {
    if (a.e == CFE_ZERO_E) {
        return a;
    }
    return cfe_norm(CFe(
        vec2<f32>(a.m.x * a.m.x - a.m.y * a.m.y, 2.0 * a.m.x * a.m.y),
        a.e * 2,
    ));
}

fn cfe_add(a: CFe, b: CFe) -> CFe {
    let d = a.e - b.e;
    // Past 26 octaves the smaller addend is below the mantissa's
    // resolution entirely.
    if (d > 26) {
        return a;
    }
    if (d < -26) {
        return b;
    }
    if (d >= 0) {
        return cfe_norm(CFe(a.m + b.m * exp2(f32(-d)), a.e));
    }
    return cfe_norm(CFe(a.m * exp2(f32(d)) + b.m, b.e));
}

// ---- extended-range scalars: the Ship family's per-component
// algebra (abs-folds break complex structure, so its floatexp step
// runs on scalar mantissa+exponent pairs). ----
struct SFe {
    m: f32,
    e: i32,
}

fn sfe_norm(v: SFe) -> SFe {
    if (v.m == 0.0) {
        return SFe(0.0, CFE_ZERO_E);
    }
    let f = frexp(v.m);
    return SFe(f.fract, v.e + f.exp);
}

fn sfe_from_f32(v: f32) -> SFe {
    return sfe_norm(SFe(v, 0));
}

fn sfe_mul(a: SFe, b: SFe) -> SFe {
    if (a.e == CFE_ZERO_E || b.e == CFE_ZERO_E) {
        return SFe(0.0, CFE_ZERO_E);
    }
    return sfe_norm(SFe(a.m * b.m, a.e + b.e));
}

fn sfe_scale(a: SFe, k: f32) -> SFe {
    if (a.e == CFE_ZERO_E || k == 0.0) {
        return SFe(0.0, CFE_ZERO_E);
    }
    return sfe_norm(SFe(a.m * k, a.e));
}

fn sfe_neg(a: SFe) -> SFe {
    return SFe(-a.m, a.e);
}

fn sfe_abs(a: SFe) -> SFe {
    return SFe(abs(a.m), a.e);
}

fn sfe_add(a: SFe, b: SFe) -> SFe {
    if (a.e == CFE_ZERO_E) {
        return b;
    }
    if (b.e == CFE_ZERO_E) {
        return a;
    }
    let d = a.e - b.e;
    if (d > 26) {
        return a;
    }
    if (d < -26) {
        return b;
    }
    if (d >= 0) {
        return sfe_norm(SFe(a.m + b.m * exp2(f32(-d)), a.e));
    }
    return sfe_norm(SFe(a.m * exp2(f32(d)) + b.m, b.e));
}

// |X + x| - |X| exactly, X an ordinary f32 (a reference component),
// x extended-range: the floatexp diffabs. The sum's mantissa sign IS
// its sign, so the three-branch analysis is exact.
fn sfe_diffabs(X: f32, x: SFe) -> SFe {
    let xf = sfe_from_f32(X);
    let sum = sfe_add(xf, x);
    if (X >= 0.0) {
        if (sum.m >= 0.0) {
            return x;
        }
        return sfe_neg(sfe_add(sfe_add(xf, xf), x));
    }
    if (sum.m > 0.0) {
        return sfe_add(sfe_add(xf, xf), x);
    }
    return sfe_neg(x);
}

// Reassemble a shared-exponent complex from two scalars.
fn cfe_from_sfe(re: SFe, im: SFe) -> CFe {
    if (re.e == CFE_ZERO_E && im.e == CFE_ZERO_E) {
        return CFe(vec2<f32>(0.0, 0.0), CFE_ZERO_E);
    }
    if (re.e == CFE_ZERO_E) {
        return cfe_norm(CFe(vec2<f32>(0.0, im.m), im.e));
    }
    if (im.e == CFE_ZERO_E) {
        return cfe_norm(CFe(vec2<f32>(re.m, 0.0), re.e));
    }
    let e = max(re.e, im.e);
    let rm = re.m * exp2(f32(clamp(re.e - e, -60, 0)));
    let im2 = im.m * exp2(f32(clamp(im.e - e, -60, 0)));
    return cfe_norm(CFe(vec2<f32>(rm, im2), e));
}

// ---- double-f32 ("DF") arithmetic: error-free transforms with
// BITMASK splits (integer ops — immune to fast-math folding on
// Metal). A value is hi + lo with |lo| <= ulp(hi)/2: ~2^-48 relative
// precision, doubling the crush-survival depth of the delta
// iteration (each near-nucleus pass reseeds deltas toward d0 scale
// and truncates pixel history to the mantissa width). ----
fn df_two_sum(a: f32, b: f32) -> vec2<f32> {
    let s = a + b;
    let bb = s - a;
    let err = (a - (s - bb)) + (b - bb);
    return vec2<f32>(s, err);
}

fn df_quick_sum(a: f32, b: f32) -> vec2<f32> {
    // Requires |a| >= |b| (true for a value + its rounding error).
    let s = a + b;
    return vec2<f32>(s, b - (s - a));
}

fn df_split(a: f32) -> vec2<f32> {
    let hi = bitcast<f32>(bitcast<u32>(a) & 0xFFFFF000u);
    return vec2<f32>(hi, a - hi);
}

fn df_two_prod(a: f32, b: f32) -> vec2<f32> {
    let p = a * b;
    let aa = df_split(a);
    let bb = df_split(b);
    let err = ((aa.x * bb.x - p) + aa.x * bb.y + aa.y * bb.x) + aa.y * bb.y;
    return vec2<f32>(p, err);
}

fn df_add(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let t = df_two_sum(a.x, b.x);
    let e = t.y + a.y + b.y;
    return df_quick_sum(t.x, e);
}

fn df_mul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let t = df_two_prod(a.x, b.x);
    let e = t.y + (a.x * b.y + a.y * b.x);
    return df_quick_sum(t.x, e);
}

fn df_muls(a: vec2<f32>, b: f32) -> vec2<f32> {
    let t = df_two_prod(a.x, b);
    let e = t.y + a.y * b;
    return df_quick_sum(t.x, e);
}

fn df_neg(a: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(-a.x, -a.y);
}

// Shared-exponent complex with DF mantissas: value = (hi + lo)·2^e,
// normalized so max(|hi.x|, |hi.y|) is in [0.5, 1).
struct CFe2 {
    hi: vec2<f32>,
    lo: vec2<f32>,
    e: i32,
}

fn cfe2_zero() -> CFe2 {
    return CFe2(vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), CFE_ZERO_E);
}

fn cfe2_norm(v: CFe2) -> CFe2 {
    let a = max(abs(v.hi.x), abs(v.hi.y));
    if (a == 0.0) {
        let b = max(abs(v.lo.x), abs(v.lo.y));
        if (b == 0.0) {
            return cfe2_zero();
        }
        // hi vanished: promote lo (keeps the information).
        let f = frexp(b);
        let sc = exp2(f32(-f.exp));
        return CFe2(v.lo * sc, vec2<f32>(0.0, 0.0), v.e + f.exp);
    }
    let f = frexp(a);
    // Power-of-two rescale: exact on both halves.
    let sc = exp2(f32(-f.exp));
    return CFe2(v.hi * sc, v.lo * sc, v.e + f.exp);
}

fn cfe2_from_f32(v: vec2<f32>) -> CFe2 {
    return cfe2_norm(CFe2(v, vec2<f32>(0.0, 0.0), 0));
}

fn cfe2_to_f32(v: CFe2) -> vec2<f32> {
    if (v.e < -126 || v.e == CFE_ZERO_E) {
        return vec2<f32>(0.0, 0.0);
    }
    if (v.e > 127) {
        return v.hi * 3.0e38;
    }
    return v.hi * exp2(f32(v.e));
}

// w × b where b is a plain f32 complex (the 2Z term, BLA mantissas).
fn cfe2_mul_c32(a: CFe2, b: vec2<f32>) -> CFe2 {
    if (a.e == CFE_ZERO_E) {
        return a;
    }
    let ax = vec2<f32>(a.hi.x, a.lo.x);
    let ay = vec2<f32>(a.hi.y, a.lo.y);
    let re = df_add(df_muls(ax, b.x), df_neg(df_muls(ay, b.y)));
    let im = df_add(df_muls(ax, b.y), df_muls(ay, b.x));
    return cfe2_norm(CFe2(vec2<f32>(re.x, im.x), vec2<f32>(re.y, im.y), a.e));
}

// w × Z where Z itself is DF (hi + lo): the 2Z multiplier at ~2^-48
// relative — the last 2^-24 reference-noise source on this rung.
fn cfe2_mul_zdf(a: CFe2, zh: vec2<f32>, zl: vec2<f32>) -> CFe2 {
    if (a.e == CFE_ZERO_E) {
        return a;
    }
    let ax = vec2<f32>(a.hi.x, a.lo.x);
    let ay = vec2<f32>(a.hi.y, a.lo.y);
    let bx = vec2<f32>(zh.x, zl.x);
    let by = vec2<f32>(zh.y, zl.y);
    let re = df_add(df_mul(ax, bx), df_neg(df_mul(ay, by)));
    let im = df_add(df_mul(ax, by), df_mul(ay, bx));
    return cfe2_norm(CFe2(vec2<f32>(re.x, im.x), vec2<f32>(re.y, im.y), a.e));
}

// As above but the f32 complex carries its own exponent (BLA A/B).
// w x Z where Z is DF mantissa + its own binary exponent: the deep
// rung's 2Z multiplier. The exponent is what keeps a near-nucleus
// reference iterate (|Z| far below f32's 2^-126) in the recurrence
// instead of silently reading as zero.
fn cfe2_mul_zdfe(a: CFe2, zh: vec2<f32>, zl: vec2<f32>, ze: i32) -> CFe2 {
    var r = cfe2_mul_zdf(a, zh, zl);
    if (r.e != CFE_ZERO_E) {
        r.e = r.e + ze;
    }
    return r;
}

// As above but the f32 complex carries its own exponent (BLA A/B).
fn cfe2_mul_cfe32(a: CFe2, m: vec2<f32>, e: i32) -> CFe2 {
    var r = cfe2_mul_c32(a, m);
    if (r.e != CFE_ZERO_E) {
        r.e = r.e + e;
    }
    return r;
}

fn cfe2_mul(a: CFe2, b: CFe2) -> CFe2 {
    if (a.e == CFE_ZERO_E || b.e == CFE_ZERO_E) {
        return cfe2_zero();
    }
    let ax = vec2<f32>(a.hi.x, a.lo.x);
    let ay = vec2<f32>(a.hi.y, a.lo.y);
    let bx = vec2<f32>(b.hi.x, b.lo.x);
    let by = vec2<f32>(b.hi.y, b.lo.y);
    let re = df_add(df_mul(ax, bx), df_neg(df_mul(ay, by)));
    let im = df_add(df_mul(ax, by), df_mul(ay, bx));
    return cfe2_norm(CFe2(vec2<f32>(re.x, im.x), vec2<f32>(re.y, im.y), a.e + b.e));
}

fn cfe2_sqr(a: CFe2) -> CFe2 {
    if (a.e == CFE_ZERO_E) {
        return a;
    }
    let ax = vec2<f32>(a.hi.x, a.lo.x);
    let ay = vec2<f32>(a.hi.y, a.lo.y);
    let re = df_add(df_mul(ax, ax), df_neg(df_mul(ay, ay)));
    let xy = df_mul(ax, ay);
    let im = df_add(xy, xy);
    return cfe2_norm(CFe2(vec2<f32>(re.x, im.x), vec2<f32>(re.y, im.y), a.e * 2));
}

fn cfe2_add(a: CFe2, b: CFe2) -> CFe2 {
    if (a.e == CFE_ZERO_E) {
        return b;
    }
    if (b.e == CFE_ZERO_E) {
        return a;
    }
    let d = a.e - b.e;
    // DF carries ~49 mantissa bits: the octave cutoff widens to match
    // (this is what preserves pixel history through delta-crush
    // reseeds that the f32 rung truncated).
    if (d > 49) {
        return a;
    }
    if (d < -49) {
        return b;
    }
    if (d >= 0) {
        let sc = exp2(f32(-d));
        let re = df_add(vec2<f32>(a.hi.x, a.lo.x), vec2<f32>(b.hi.x, b.lo.x) * sc);
        let im = df_add(vec2<f32>(a.hi.y, a.lo.y), vec2<f32>(b.hi.y, b.lo.y) * sc);
        return cfe2_norm(CFe2(vec2<f32>(re.x, im.x), vec2<f32>(re.y, im.y), a.e));
    }
    let sc = exp2(f32(d));
    let re = df_add(vec2<f32>(a.hi.x, a.lo.x) * sc, vec2<f32>(b.hi.x, b.lo.x));
    let im = df_add(vec2<f32>(a.hi.y, a.lo.y) * sc, vec2<f32>(b.hi.y, b.lo.y));
    return cfe2_norm(CFe2(vec2<f32>(re.x, im.x), vec2<f32>(re.y, im.y), b.e));
}

struct OrbitSummary {
    z: vec2<f32>,
    n: u32,
    escaped: bool,
    converged: bool,
    period: u32,
    dz: vec2<f32>,
}

// Rebuild a delta against a DIFFERENT reference index, in double
// float. The rebase target is the orbit start, so the new delta is
// (Z_from - Z_0) + delta -- and computing that difference through the
// reference's own DF entries is the point: an f32 subtraction of two
// O(1) iterates would truncate the pixel's history to f32, which is
// the reseed-precision loss this rung exists to fix.
fn fe_rebase_delta(w: CFe2, at: u32) -> CFe2 {
    let zi = ref_z(at);
    let z0 = ref_z(0u);
    let zi_lo = ref_z_lo(at);
    let z0_lo = ref_z_lo(0u);
    var dxr = vec2<f32>(0.0, 0.0);
    var dyr = vec2<f32>(0.0, 0.0);
    if (w.e != CFE_ZERO_E && w.e >= -126 && w.e <= 127) {
        let sc_w = exp2(f32(w.e));
        dxr = vec2<f32>(w.hi.x, w.lo.x) * sc_w;
        dyr = vec2<f32>(w.hi.y, w.lo.y) * sc_w;
    }
    let rx = df_add(df_add(vec2<f32>(zi.x, zi_lo.x), df_neg(vec2<f32>(z0.x, z0_lo.x))), dxr);
    let ry = df_add(df_add(vec2<f32>(zi.y, zi_lo.y), df_neg(vec2<f32>(z0.y, z0_lo.y))), dyr);
    return cfe2_norm(CFe2(vec2<f32>(rx.x, ry.x), vec2<f32>(rx.y, ry.y), 0));
}

// The same, against a reference of ZERO: a two-term recurrence's
// history rebases onto Z_-1, which is zero for both the reference and
// the pixel, so the whole previous iterate becomes the new delta.
fn fe_rebase_from_zero(w: CFe2, at: u32) -> CFe2 {
    let zi = ref_z(at);
    let zi_lo = ref_z_lo(at);
    var dxr = vec2<f32>(0.0, 0.0);
    var dyr = vec2<f32>(0.0, 0.0);
    if (w.e != CFE_ZERO_E && w.e >= -126 && w.e <= 127) {
        let sc_w = exp2(f32(w.e));
        dxr = vec2<f32>(w.hi.x, w.lo.x) * sc_w;
        dyr = vec2<f32>(w.hi.y, w.lo.y) * sc_w;
    }
    let rx = df_add(vec2<f32>(zi.x, zi_lo.x), dxr);
    let ry = df_add(vec2<f32>(zi.y, zi_lo.y), dyr);
    return cfe2_norm(CFe2(vec2<f32>(rx.x, ry.x), vec2<f32>(rx.y, ry.y), 0));
}

//__COLORING__

//__COLORING_ACCUM__

@compute @workgroup_size(8, 8, 1)
fn escape_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }

    // Pixel offset in pixel units, rotated (same mapping as the
    // scaled rung); delta_c = offset * S with S = s_m * 2^s_e.
    let centered = vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5, 0.5)
        - 0.5 * vec2<f32>(f32(params.width), f32(params.height));
    var dpx = centered;
    dpx.y = -dpx.y;
    let rot = params.rot_cs;
    let d0px = vec2<f32>(
        dpx.x * rot.x - dpx.y * rot.y,
        dpx.x * rot.y + dpx.y * rot.x,
    );
    // d0 in DF: the (pixel + relocation-offset) sum and the s_m
    // product both keep their low halves — pixel positions resolve to
    // ~2^-48 relative even against a large offset.
    let d0xs = df_muls(df_two_sum(d0px.x, perturb.ref_offset.x), perturb.s_m);
    let d0ys = df_muls(df_two_sum(d0px.y, perturb.ref_offset.y), perturb.s_m);
    let d0 = cfe2_norm(CFe2(
        vec2<f32>(d0xs.x, d0ys.x),
        vec2<f32>(d0xs.y, d0ys.y),
        perturb.s_e,
    ));
    let px_index = gid.y * params.width + gid.x;

    let is_julia_perturb = (perturb.flags & 2u) != 0u;
    var w: CFe2;
    //__W_INIT__

    var m = 0u;
    var z = vec2<f32>(0.0, 0.0);
    var escaped = false;
    var n = 0u;
    // Mutable on the perturbed path too: a Convergent formula
    // terminates on a settled orbit here exactly as it does on the
    // direct one. Non-convergent formulas splice nothing below, so
    // their shaders stay byte-identical and the compiler folds this
    // straight back to a constant.
    var converged = false;
    let period = 0u;
    let dz = vec2<f32>(1.0, 0.0);
    let c_f32 = params.center;

    //__ACCUM_DECL__

    // Previous-iteration delta, for two-term recurrences.
    //__W_PREV_INIT__
    var i = perturb.iter_start;
    if (perturb.iter_start > 0u) {
        let st = iter_state[px_index];
        w = CFe2(st.w, st.w_lo, st.w_e);
        //__STATE_RESUME_TAIL__
        z = st.z;
        m = st.m;
        accum_state = st.accum;
        i = max(i, st.i_at);
        if ((st.n_done & ITER_ESCAPED_BIT) != 0u) {
            escaped = true;
            n = st.n_done & ~ITER_ESCAPED_BIT;
        }
    }

    while (i < perturb.iter_end && !escaped) {
        // BLA skip (see the scaled template). Here w IS the absolute
        // delta as a CFe, so validity compares and the affine
        // application run in full extended range -- no clamps.
        var did_skip = false;
        if (bla.n_levels > 0u && m > 0u) {
            let aw = length(w.hi);
            var pick_a = vec2<f32>(0.0, 0.0);
            var pick_b = vec2<f32>(0.0, 0.0);
            var pick_ae = 0;
            var pick_be = 0;
            var pick_span = 0u;
            let w_zero = aw == 0.0 || w.e == CFE_ZERO_E;
            let m_left = perturb.orbit_len - 1u - m;
            // Bounded by the RENDER's end, not the chunk's: a skip
            // that crosses a chunk boundary is fine (i_at carries the
            // overshoot), and bounding it by the chunk would make the
            // image depend on the chunk size.
            let steps_left = params.max_iter - i;
            for (var l = 0u; l < bla.n_levels; l = l + 1u) {
                let span = 2u << l;
                if ((m & (span - 1u)) != 0u || span > m_left || span > steps_left
                    || m + span > bla.n_steps) {
                    break;
                }
                let ent = bla.entries[bla.offsets[l >> 2u][l & 3u] + (m >> (l + 1u))];
                if (w_zero) {
                    if (ent.r_m <= 0.0) {
                        break;
                    }
                } else if (!bla_mag_lt(aw, w.e, ent.r_m, ent.r_e)) {
                    break;
                }
                pick_a = ent.a_m;
                pick_ae = ent.a_e;
                pick_b = ent.b_m;
                pick_be = ent.b_e;
                pick_span = span;
            }
            if (pick_span > 0u) {
                let ta = cfe2_mul_cfe32(w, pick_a, pick_ae);
                let tb = cfe2_mul_cfe32(d0, pick_b, pick_be);
                w = cfe2_add(ta, tb);
                m = m + pick_span;
                i = i + pick_span;
                did_skip = true;
            }
        }
        if (!did_skip) {
            // Two views of the same iterate: the plain value (Ship
            // and anything that needs an f32) and the raw mantissa
            // with its exponent (the exact multiplier, valid at any
            // depth - see ReferenceOrbit::orbit_e).
            let z_ref = ref_z(m);
            let z_ref_m = ref_orbit[m];
            let z_ref_lo_m = ref_orbit_lo[m];
            let z_ref_e = ref_orbit_e[m];
            //__DELTA_STEP_FE__
            w = w_new;
            m = m + 1u;
            i = i + 1u;
        }
        let z_before = z;

        let delta = cfe2_to_f32(w);
        let zi = ref_z(min(m, perturb.orbit_len - 1u));
        let z_full = zi + delta;
        z = z_full;

        //__ACCUM_UPDATE__
        //__CONVERGE_TEST__

        // Delta-aware escape margin -- see the scaled template. The
        // f32 delta underflows to zero exactly when it is too small to
        // move the margin, so the deep rung needs no extended-range
        // cross term.
        let mr = ref_r2(min(m, perturb.orbit_len - 1u));
        let margin = (mr.x - params.bailout)
            + (mr.y + 2.0 * dot(zi, delta) + dot(delta, delta));
        if (margin > 0.0) {
            escaped = true;
            n = i;
            break;
        }

        //__REBASE__
    }
    if (perturb.iter_end < params.max_iter) {
        iter_state[px_index] = IterState(
            w.hi, z, accum_state, w.lo, w.e, m,
            select(0u, n | ITER_ESCAPED_BIT, escaped),
            i,
            //__STATE_SAVE_TAIL__
        );
    }
    if (!escaped) {
        n = params.max_iter;
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    // COVERAGE, not colour. A pixel that never escaped has no value
    // to show, so it is left ABSENT rather than painted black, and
    // the tonemap's background blend fills it -- which is what makes
    // the background colour apply to the interior, and what makes a
    // transparent export leave it transparent. Averaged by the
    // downsample like everything else, so the boundary antialiases
    // against the background instead of against black.
    var coverage = 0.0;
    // The relief pass slopes THIS, not the rendered colour: the value
    // before the palette, so a cycling palette's band edges are not
    // mistaken for cliffs. Interior pixels keep 0 -- flat, which puts
    // the rim light exactly on the set boundary.
    var height = 0.0;
    if (escaped || COLORING_COLORS_INTERIOR) {
        let summary = OrbitSummary(z, n, escaped, converged, period, dz);
        // `fract` so unbounded colorings cycle as they grow; a
        // bounded one clamps instead, or its top value (1.0) wraps to
        // the palette's bottom and the brightest points render
        // darkest. See ColoringFeature::Bounded.
        let raw = coloring_map(summary, accum_state);
        let t = select(fract(raw), clamp(raw, 0.0, 1.0), COLORING_IS_BOUNDED);
        // SHADING_BANDED picks the wrapped coordinate instead, which
        // turns every palette band into a step (the engraved look).
        height = select(raw, t, params.shade_flags == 1u);
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
        coverage = 1.0;
    }

    if ((params.flags & 8u) != 0u && perturb.iter_end >= params.max_iter) {
        results[px_index] = IterResult(
            z, dz, accum_state, n,
            select(0u, 1u, escaped) | select(0u, 2u, converged) | (period << 2u),
        );
    }
    // Mid-render, an UNFINISHED pixel keeps the previous frame's
    // content instead of painting itself black: a chunked render's
    // early frames used to flash black wherever iterations had not
    // finished (the whole frame, during a pan past the floatexp
    // threshold, where the TDR-safe chunk is far smaller than
    // max_iter). Escaped pixels rewrite their final colour every
    // chunk as before, and the LAST chunk writes everyone -- the
    // settled image is byte-identical.
    if (escaped || perturb.iter_end >= params.max_iter) {
        textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(rgb, coverage));
        textureStore(height_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(height, 0.0, 0.0, 0.0));
    }
}
"#;

/// Which delta algebra the perturbed pipeline compiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerturbTier {
    /// z^p + c, integer p — the clean binomial expansion.
    Power(u32),
    /// Tricorn / Multicorn: `conj(z)^p + c`, the power binomial over
    /// conjugated operands -- on BOTH rungs, since conjugation is only
    /// a sign flip in either representation. BLA stays unavailable:
    /// the map is anti-holomorphic, so the linear A*delta + B*delta_c
    /// model has no matching derivation.
    Tricorn(u32),
    /// Phoenix: `z^2 + c + p*z_prev`. A TWO-TERM recurrence, so both
    /// rungs carry a second delta and rebase the pair together. The
    /// scaled rung hides the history in `w_lo`, which is dead there;
    /// the deep rung has no spare field, so its IterState genuinely
    /// grows (see [`iter_state_bytes`]). Carries the map's continuous
    /// parameter, which is part of the reference orbit's identity.
    Phoenix,
    /// Manowar: `z^2 + z_prev + c`, seeded at z_0 = z_-1 = c. The
    /// same two-term recurrence as Phoenix with p = 1, so it shares
    /// the second delta, the step and the pair rebase. It differs in
    /// the SEED -- both deltas start at d0, not zero -- and in what
    /// the history rebases against: Z_-1 = Z_0 = c here, zero there.
    Manowar,
    /// Lambda (logistic): `c*z*(1-z)`.
    ///
    /// The first tier whose PARAMETER MULTIPLIES the map. Every
    /// earlier one has c entering additively, so the delta step never
    /// needed to know the reference's c at all; here it is a factor,
    /// and the parameter-plane term picks up the reference's own
    /// z-product as well:
    ///
    ///   dP = d*(1 - 2Z - d)          (the z-part, cancellation-free)
    ///   d' = C*dP + dc*(Z(1-Z) + dP)
    ///
    /// Derived by expanding `(C+dc)(Z+d)(1-Z-d) - C*Z(1-Z)` and
    /// collecting; every term is a product of available quantities
    /// with no subtraction of nearly-equal values, which is the whole
    /// requirement for a delta form.
    ///
    /// Seeded at the CRITICAL POINT 1/2 on the parameter plane (zero
    /// is a fixed point of this map for every c). The delta still
    /// starts at zero there, because both orbits share that seed.
    ///
    /// BLA is available in principle -- the map is holomorphic and
    /// `A = C(1-2Z)`, `B = Z(1-Z)` -- but is not built yet, so this
    /// tier iterates per step.
    Lambda,
    /// Feather: `z^p / (1 + x^2 - i*y^2) + c`, integer p.
    ///
    /// The first RATIONAL tier, and the one that introduces the
    /// quotient delta form the other rationals will reuse:
    ///
    ///   dq = (dN - q*dD) / (D + dD)     with q = N/D the REFERENCE
    ///                                   quotient, N, D its numerator
    ///                                   and denominator
    ///
    /// Both `dN` and `dD` are small and `D + dD` is full size, so
    /// nothing subtracts nearly-equal values -- which is the entire
    /// requirement. Writing it the obvious way instead, as
    /// `(dN*D - N*dD)/(D*(D+dD))`, differences two full-size products
    /// and loses the delta to cancellation.
    ///
    /// `dN` is the ordinary power binomial. `dD` is component-wise,
    /// because this denominator is NOT holomorphic -- it reads x and y
    /// separately -- but each component is its own cancellation-free
    /// binomial: `d(x^2) = 2X*dx + dx^2`. Non-holomorphic also means
    /// no BLA, for the same reason the Tricorn family has none.
    ///
    /// The divisor is the one quantity here that is O(1) rather than
    /// small, so it is formed in plain f32 on BOTH rungs. On the deep
    /// rung `dD` converts down first: it flushes to zero exactly when
    /// it is too small to change an f32 O(1) value, which is the
    /// correct answer rather than an approximation.
    Feather(u32),
    /// McMullen: `z^n + c/z^m`, integer n and m — the first tier with
    /// a genuine POLE, and the first that is JULIA-ONLY.
    ///
    /// The delta form splits the two terms and never divides a small
    /// number by a small number:
    ///
    ///   dA = (Z+d)^n - Z^n                    (binomial, exact)
    ///   dM = (Z+d)^m - Z^m                    (binomial, exact)
    ///   d' = dA - C*dM / [ (Z+d)^m * Z^m ]    (+ dc/(Z+d)^m on the
    ///                                          parameter plane)
    ///
    /// The pole term's delta is written as
    /// `1/(Z+d)^m - 1/Z^m = -dM / ((Z+d)^m Z^m)`: numerator small and
    /// cancellation-free, denominator a product of FULL values. Formed
    /// the direct way instead, it would subtract two large nearly-equal
    /// reciprocals and lose the delta entirely.
    ///
    /// JULIA-ONLY, and that is a statement about the formula rather
    /// than the tier. Our McMullen seeds its parameter plane at
    /// `z_0 = c`, which is not a critical point of this map (`z = 0`
    /// is the POLE, and the real critical points sit at
    /// `z^(n+m) = (m/n)c`) — measured, 0 of 4000 sampled parameters
    /// have a bounded orbit, so that plane has no interior to zoom
    /// into and perturbing it would be machinery for nothing. The
    /// classic Sierpinski-carpet pictures are Julia sets, which is
    /// where this works. Seeding the parameter plane at a proper
    /// critical point is a separate, visible formula change.
    McMullen(u32, u32),
    /// Magnet: `((z^2 + c - 1)/(2z + c - 2))^2` (variant 0) and its
    /// cubic sibling (variant 1) — the quotient delta form composed
    /// with a square, and the first CONVERGENT tier.
    ///
    ///   dN, dD     the numerator's and denominator's own deltas,
    ///              each a cancellation-free expansion
    ///   dq = (dN - q*dD)/(D + dD)      q = N/D, the REFERENCE quotient
    ///   df = 2*q*dq + dq^2             from f = q^2
    ///
    /// `c` appears in BOTH numerator and denominator here, which no
    /// earlier tier had: the parameter-plane term is not a bare `+dc`
    /// but enters `dN` and `dD` separately and then partially cancels
    /// inside `dN - q*dD`. That cancellation is the map's own — it is
    /// what makes the derivative small near the attractor — and it
    /// happens between two SMALL quantities, so no significance is
    /// lost.
    ///
    /// CONVERGENT, and that is load-bearing: these orbits settle at
    /// z = 1 rather than escaping. The perturbed loop needs the settle
    /// test, or every converging pixel runs to `max_iter` and the
    /// perturbed image differs from the direct one. See
    /// [`PerturbTier::is_convergent`].
    Magnet(u32),
    /// Burning Ship family: abs-folds via diffabs case analysis, on
    /// both rungs (the floatexp rung runs extended-range scalar
    /// diffabs). The u32 is the variant enum (0..=5) — each fold
    /// arrangement has its own delta algebra.
    Ship(u32),
}

impl PerturbTier {
    /// Whether this tier's map CONVERGES, so the perturbed loop needs
    /// the settle test.
    ///
    /// `assemble_perturbed` has no FormulaDef in scope -- on that path
    /// the TIER is the map's identity -- so the feature has to be
    /// restated here. A test checks it against the registry's own
    /// `FormulaFeature::Convergent`, so the two cannot drift.
    pub fn is_convergent(self) -> bool {
        matches!(self, PerturbTier::Magnet(_))
    }
}

/// The scaled-f32 Burning Ship-family delta step for one variant.
///
/// Derivations (delta = S*w; diffabs is positively homogeneous, so
/// diffabs(A, S*t)/S = diffabs(A/S, t), and A/S stays inside f32 for
/// the scaled rung's zoom range < 2^54):
///   re plain   (v0,1,2): d(x^2-y^2)/S = 2Xwx - 2Ywy + S(wx^2-wy^2)
///   re folded  (v3,4,5): diffabs((X^2-Y^2)/S, plain-re)
///   im ship    (v0):  2*diffabs(XY/S, cross), cross = Xwy+Ywx+Swxwy
///   im perp-M  (v1,5): -2[diffabs(X/S, wx)*(Y+Swy) + |X|*wy]
///   im perp-S  (v2):  -2[wx*|Y+Swy| + X*diffabs(Y/S, wy)]
///   im celtic  (v3):  2*cross
///   im buffalo (v4):  -2*diffabs(XY/S, cross)
fn delta_step_ship(variant: u32) -> String {
    let mut out = String::new();
    out.push_str("        let sx = z_ref.x;\n");
    out.push_str("        let sy = z_ref.y;\n");
    let re_plain = "2.0 * (sx * w.x - sy * w.y) + perturb.s * (w.x * w.x - w.y * w.y)";
    match variant {
        0 | 1 | 2 => {
            out.push_str(&format!("        let re_new = {re_plain} + d0_term.x;\n"));
        }
        _ => {
            out.push_str(&format!(
                "        let re_new = diffabs((sx * sx - sy * sy) * perturb.inv_s, {re_plain}) + d0_term.x;\n"
            ));
        }
    }
    match variant {
        0 => {
            out.push_str("        let cross = sx * w.y + sy * w.x + perturb.s * w.x * w.y;\n");
            out.push_str("        let im_new = 2.0 * diffabs(sx * sy * perturb.inv_s, cross) + d0_term.y;\n");
        }
        1 | 5 => {
            out.push_str("        let im_new = -2.0 * (diffabs(sx * perturb.inv_s, w.x) * (sy + perturb.s * w.y) + abs(sx) * w.y) + d0_term.y;\n");
        }
        2 => {
            out.push_str("        let im_new = -2.0 * (w.x * abs(sy + perturb.s * w.y) + sx * diffabs(sy * perturb.inv_s, w.y)) + d0_term.y;\n");
        }
        3 => {
            out.push_str("        let im_new = 2.0 * (sx * w.y + sy * w.x + perturb.s * w.x * w.y) + d0_term.y;\n");
        }
        _ => {
            out.push_str("        let cross = sx * w.y + sy * w.x + perturb.s * w.x * w.y;\n");
            out.push_str("        let im_new = -2.0 * diffabs(sx * sy * perturb.inv_s, cross) + d0_term.y;\n");
        }
    }
    out.push_str("        var w_new = vec2<f32>(re_new, im_new);");
    out
}

/// The floatexp Burning Ship-family delta step: the same algebra on
/// ABSOLUTE deltas carried as extended-range scalars (SFe), with
/// sfe_diffabs doing the exact three-branch case analysis against
/// the f32 reference components. This is the "floatexp diffabs" the
/// plan deferred — and the deep-needle case (either component of Z
/// small, full-range iteration required) is handled by construction:
/// every quantity here is full-range.
fn delta_step_ship_fe(variant: u32) -> String {
    let mut out = String::new();
    out.push_str("        let sx = z_ref.x;\n");
    out.push_str("        let sy = z_ref.y;\n");
    out.push_str("        let dx = sfe_norm(SFe(w.hi.x, w.e));\n");
    out.push_str("        let dy = sfe_norm(SFe(w.hi.y, w.e));\n");
    out.push_str("        let d0x = sfe_norm(SFe(d0.hi.x, d0.e));\n");
    out.push_str("        let d0y = sfe_norm(SFe(d0.hi.y, d0.e));\n");
    out.push_str("        let du = sfe_add(sfe_add(sfe_scale(dx, 2.0 * sx), sfe_scale(dy, -2.0 * sy)), sfe_add(sfe_mul(dx, dx), sfe_neg(sfe_mul(dy, dy))));\n");
    match variant {
        0 | 1 | 2 => {
            out.push_str("        let re_s = sfe_add(du, d0x);\n");
        }
        _ => {
            out.push_str("        let re_s = sfe_add(sfe_diffabs(sx * sx - sy * sy, du), d0x);\n");
        }
    }
    match variant {
        0 | 4 => {
            out.push_str("        let cross = sfe_add(sfe_add(sfe_scale(dy, sx), sfe_scale(dx, sy)), sfe_mul(dx, dy));\n");
            let sign = if variant == 0 { "2.0" } else { "-2.0" };
            out.push_str(&format!(
                "        let im_s = sfe_add(sfe_scale(sfe_diffabs(sx * sy, cross), {sign}), d0y);\n"
            ));
        }
        1 | 5 => {
            out.push_str("        let yprime = sfe_add(sfe_from_f32(sy), dy);\n");
            out.push_str("        let im_s = sfe_add(sfe_scale(sfe_add(sfe_mul(sfe_diffabs(sx, dx), yprime), sfe_scale(dy, abs(sx))), -2.0), d0y);\n");
        }
        2 => {
            out.push_str("        let yabs = sfe_abs(sfe_add(sfe_from_f32(sy), dy));\n");
            out.push_str("        let im_s = sfe_add(sfe_scale(sfe_add(sfe_mul(dx, yabs), sfe_scale(sfe_diffabs(sy, dy), sx)), -2.0), d0y);\n");
        }
        _ => {
            out.push_str("        let cross = sfe_add(sfe_add(sfe_scale(dy, sx), sfe_scale(dx, sy)), sfe_mul(dx, dy));\n");
            out.push_str("        let im_s = sfe_add(sfe_scale(cross, 2.0), d0y);\n");
        }
    }
    out.push_str("        let w_new_c = cfe_from_sfe(re_s, im_s);\n");
    out.push_str("        var w_new = CFe2(w_new_c.m, vec2<f32>(0.0, 0.0), w_new_c.e);");
    out
}

/// Binomial coefficient (small arguments; the power cap is 12).
fn binomial(p: u32, k: u32) -> u64 {
    let mut acc = 1u64;
    for i in 0..k as u64 {
        acc = acc * (p as u64 - i) / (i + 1);
    }
    acc
}

/// Emit the scaled-f32 delta step for `z^p + c`:
/// `w' = sum_k C(p,k) Z^(p-k) S^(k-1) w^k + d0`. For p = 2 this is
/// byte-for-byte the original hand-written Mandelbrot step (pipeline
/// stability); higher powers unroll the binomial chain with the
/// S-folded w powers (`u_k = S^(k-1) w^k`, underflow of S legitimately
/// zeroing the high terms in the deep-linear regime).
fn delta_step_scaled(p: u32) -> String {
    delta_step_scaled_on(p, "z_ref", "w")
}

/// The binomial delta step, over NAMED operands.
///
/// Tricorn's map is `conj(z)^p + c`, whose delta expansion is
/// this same binomial with Z and w replaced by their conjugates
/// (conj is an involution and distributes over products, so
/// `conj(Z+d)^p - conj(Z)^p` expands exactly like the plain
/// power in conj(Z), conj(d)). Naming the operands is all it
/// takes to share the derivation instead of copying it.
fn delta_step_scaled_on(p: u32, zr: &str, w: &str) -> String {
    if p == 2 {
        return format!(
            "        // w' = 2 Z w + S w^2 + d0 (quadratic term hoisted out in the\n\
             \x20       // deep-linear regime).\n\
             \x20       var w_new = 2.0 * vec2<f32>(\n\
             \x20           {zr}.x * {w}.x - {zr}.y * {w}.y,\n\
             \x20           {zr}.x * {w}.y + {zr}.y * {w}.x,\n\
             \x20       ) + d0_term;\n\
             \x20       if ((perturb.flags & 1u) == 0u) {{\n\
             \x20           w_new = w_new + perturb.s * vec2<f32>({w}.x * {w}.x - {w}.y * {w}.y, 2.0 * {w}.x * {w}.y);\n\
             \x20       }}"
        );
    }
    let mut out = String::new();
    out.push_str(&format!("        // Binomial delta step for z^{p} + c.\n"));
    // Powers of the reference Z up to p-1.
    out.push_str(&format!("        let zr1 = {zr};\n"));
    for k in 2..p {
        out.push_str(&format!(
            "        let zr{k} = vec2<f32>(zr{}.x * {zr}.x - zr{}.y * {zr}.y, zr{}.x * {zr}.y + zr{}.y * {zr}.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    // S-folded powers of w: u1 = w, u(k) = S * u(k-1) * w.
    out.push_str(&format!("        let u1 = {w};\n"));
    for k in 2..=p {
        out.push_str(&format!(
            "        let u{k} = perturb.s * vec2<f32>(u{}.x * {w}.x - u{}.y * {w}.y, u{}.x * {w}.y + u{}.y * {w}.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str("        var w_new = d0_term;\n");
    for k in 1..=p {
        let coeff = binomial(p, k);
        let term = if k == p {
            format!("{coeff}.0 * u{k}")
        } else {
            let zp = p - k;
            format!(
                "{coeff}.0 * vec2<f32>(zr{zp}.x * u{k}.x - zr{zp}.y * u{k}.y, zr{zp}.x * u{k}.y + zr{zp}.y * u{k}.x)"
            )
        };
        out.push_str(&format!("        w_new = w_new + {term};\n"));
    }
    out
}

/// Lambda's scaled-rung delta step.
///
/// With `d = S*w` and `dc = S*d0`, dividing the derivation in
/// [`PerturbTier::Lambda`] through by S gives
///
///   q  = w*(1 - 2Z - S*w)        (= dP/S)
///   w' = C*q + d0*(Z(1-Z) + S*q)
///
/// The `S*w` and `S*q` terms are the second-order ones, dropped under
/// the deep-linear flag exactly as the power tier drops its `S*w^2`.
fn delta_step_lambda() -> String {
    r#"        // Lambda: w' = C*q + d0*(P + S*q), q = w*(1 - 2Z - S*w).
        var t = vec2<f32>(1.0, 0.0) - 2.0 * z_ref;
        if ((perturb.flags & 1u) == 0u) {
            t = t - perturb.s * w;
        }
        let q = vec2<f32>(w.x * t.x - w.y * t.y, w.x * t.y + w.y * t.x);
        let cref = perturb.ref_c;
        var w_new = vec2<f32>(q.x * cref.x - q.y * cref.y, q.x * cref.y + q.y * cref.x);
        if (!is_julia_perturb) {
            // P = Z(1 - Z), the reference's own z-product: what the
            // pixel's c-offset acts on even where the delta is zero.
            let om = vec2<f32>(1.0, 0.0) - z_ref;
            var p_ref = vec2<f32>(
                z_ref.x * om.x - z_ref.y * om.y,
                z_ref.x * om.y + z_ref.y * om.x,
            );
            if ((perturb.flags & 1u) == 0u) {
                p_ref = p_ref + perturb.s * q;
            }
            w_new = w_new + vec2<f32>(
                d0_term.x * p_ref.x - d0_term.y * p_ref.y,
                d0_term.x * p_ref.y + d0_term.y * p_ref.x,
            );
        }"#
    .to_string()
}

/// Feather's scaled-rung delta step.
///
/// With `d = S*w`, every delta divides through by S: `n_s = dN/S`,
/// `d_s = dD/S`, and `w' = (n_s - q*d_s)/(D + S*d_s) + d0`.
fn delta_step_feather(p: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "        // Feather: w' = (dN/S - q*dD/S)/(D + S*dD/S) + d0, for z^{p}/(1+x^2-i y^2)+c.\n"
    ));
    // dN/S by the binomial, over the reference Z and the scaled delta.
    out.push_str("        let fzr1 = z_ref;\n");
    for k in 2..p {
        out.push_str(&format!(
            "        let fzr{k} = vec2<f32>(fzr{}.x * z_ref.x - fzr{}.y * z_ref.y, fzr{}.x * z_ref.y + fzr{}.y * z_ref.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str("        let fu1 = w;\n");
    for k in 2..=p {
        out.push_str(&format!(
            "        let fu{k} = perturb.s * vec2<f32>(fu{}.x * w.x - fu{}.y * w.y, fu{}.x * w.y + fu{}.y * w.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str("        var n_s = vec2<f32>(0.0, 0.0);\n");
    for k in 1..=p {
        let coeff = binomial(p, k);
        let term = if k == p {
            format!("{coeff}.0 * fu{k}")
        } else {
            let zp = p - k;
            format!(
                "{coeff}.0 * vec2<f32>(fzr{zp}.x * fu{k}.x - fzr{zp}.y * fu{k}.y, fzr{zp}.x * fu{k}.y + fzr{zp}.y * fu{k}.x)"
            )
        };
        out.push_str(&format!("        n_s = n_s + {term};\n"));
    }
    out.push_str(
        r#"        // dD/S, component-wise: d(x^2)/S = 2X*wx + S*wx^2,
        // and the imaginary part carries the formula's minus sign.
        var d_s = vec2<f32>(2.0 * z_ref.x * w.x, -2.0 * z_ref.y * w.y);
        if ((perturb.flags & 1u) == 0u) {
            d_s = d_s + perturb.s * vec2<f32>(w.x * w.x, -(w.y * w.y));
        }
        // The reference's own N, D and quotient q = N/D. |D| >= 1 by
        // construction (Re D = 1 + X^2), so this division is safe.
        var f_num = z_ref;
"#,
    );
    for _ in 1..p {
        out.push_str("        f_num = vec2<f32>(f_num.x * z_ref.x - f_num.y * z_ref.y, f_num.x * z_ref.y + f_num.y * z_ref.x);\n");
    }
    out.push_str(
        r#"        let f_den = vec2<f32>(1.0 + z_ref.x * z_ref.x, -(z_ref.y * z_ref.y));
        let f_dd = dot(f_den, f_den);
        let q = vec2<f32>(
            (f_num.x * f_den.x + f_num.y * f_den.y) / f_dd,
            (f_num.y * f_den.x - f_num.x * f_den.y) / f_dd,
        );
        // Divisor D + dD, formed at full size in f32.
        var div = f_den;
        if ((perturb.flags & 1u) == 0u) {
            div = div + perturb.s * d_s;
        }
        let dv2 = dot(div, div);
        let top = n_s - vec2<f32>(q.x * d_s.x - q.y * d_s.y, q.x * d_s.y + q.y * d_s.x);
        var w_new = vec2<f32>(
            (top.x * div.x + top.y * div.y) / dv2,
            (top.y * div.x - top.x * div.y) / dv2,
        ) + d0_term;
"#,
    );
    out
}

/// `((Z+d)^p - Z^p)/S` with `d = S*w`, over named operands and with a
/// name prefix, so a formula needing TWO different binomials in one
/// step (McMullen's `z^n` and its pole's `z^m`) can emit both without
/// colliding. Same derivation as [`delta_step_scaled_on`]; that one
/// writes straight into `w_new` for the single-binomial tiers.
fn binom_scaled(p: u32, zvar: &str, wvar: &str, prefix: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("        let {prefix}z1 = {zvar};\n"));
    for k in 2..p {
        out.push_str(&format!(
            "        let {prefix}z{k} = vec2<f32>({prefix}z{}.x * {zvar}.x - {prefix}z{}.y * {zvar}.y, {prefix}z{}.x * {zvar}.y + {prefix}z{}.y * {zvar}.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str(&format!("        let {prefix}u1 = {wvar};\n"));
    for k in 2..=p {
        out.push_str(&format!(
            "        let {prefix}u{k} = perturb.s * vec2<f32>({prefix}u{}.x * {wvar}.x - {prefix}u{}.y * {wvar}.y, {prefix}u{}.x * {wvar}.y + {prefix}u{}.y * {wvar}.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str(&format!("        var {prefix}d = vec2<f32>(0.0, 0.0);\n"));
    for k in 1..=p {
        let c = binomial(p, k);
        let term = if k == p {
            format!("{c}.0 * {prefix}u{k}")
        } else {
            let zp = p - k;
            format!(
                "{c}.0 * vec2<f32>({prefix}z{zp}.x * {prefix}u{k}.x - {prefix}z{zp}.y * {prefix}u{k}.y, {prefix}z{zp}.x * {prefix}u{k}.y + {prefix}z{zp}.y * {prefix}u{k}.x)"
            )
        };
        out.push_str(&format!("        {prefix}d = {prefix}d + {term};\n"));
    }
    out
}

/// McMullen's scaled-rung delta step.
fn delta_step_mcmullen(n: u32, m: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "        // McMullen z^{n} + c/z^{m}: w' = dA/S - C*(dM/S)/((Z+d)^{m} Z^{m}).\n"
    ));
    out.push_str(&binom_scaled(n, "z_ref", "w", "a"));
    out.push_str(&binom_scaled(m, "z_ref", "w", "b"));
    out.push_str(
        r#"        // Full pixel value and the two full-size powers the
        // pole term divides by. Computing (Z+d) in f32 is fine: it is
        // a COEFFICIENT here, not the delta.
        let zf = z_ref + perturb.s * w;
        var zf_m = zf;
"#,
    );
    for _ in 1..m {
        out.push_str("        zf_m = vec2<f32>(zf_m.x * zf.x - zf_m.y * zf.y, zf_m.x * zf.y + zf_m.y * zf.x);\n");
    }
    out.push_str("        var zr_m = z_ref;\n");
    for _ in 1..m {
        out.push_str("        zr_m = vec2<f32>(zr_m.x * z_ref.x - zr_m.y * z_ref.y, zr_m.x * z_ref.y + zr_m.y * z_ref.x);\n");
    }
    out.push_str(
        r#"        let den = vec2<f32>(zf_m.x * zr_m.x - zf_m.y * zr_m.y, zf_m.x * zr_m.y + zf_m.y * zr_m.x);
        let den2 = max(dot(den, den), 1e-30);
        let cref = perturb.ref_c;
        let cb = vec2<f32>(cref.x * bd.x - cref.y * bd.y, cref.x * bd.y + cref.y * bd.x);
        // -C*dM/S divided by the full-size denominator.
        var w_new = ad - vec2<f32>(
            (cb.x * den.x + cb.y * den.y) / den2,
            (cb.y * den.x - cb.x * den.y) / den2,
        );
        if (!is_julia_perturb) {
            // Parameter plane: + dc/(Z+d)^m. Unused today -- the tier
            // is Julia-only -- but the term belongs to the derivation.
            let zf2 = max(dot(zf_m, zf_m), 1e-30);
            w_new = w_new + vec2<f32>(
                (d0_term.x * zf_m.x + d0_term.y * zf_m.y) / zf2,
                (d0_term.y * zf_m.x - d0_term.x * zf_m.y) / zf2,
            );
        }
"#,
    );
    out
}

/// Magnet's scaled-rung delta step.
fn delta_step_magnet(variant: u32) -> String {
    let head = if variant == 0 {
        r#"        // Magnet I: N = z^2+c-1, D = 2z+c-2, f = (N/D)^2.
        let cref = perturb.ref_c;
        let zr2 = vec2<f32>(z_ref.x * z_ref.x - z_ref.y * z_ref.y, 2.0 * z_ref.x * z_ref.y);
        let n_ref = zr2 + cref - vec2<f32>(1.0, 0.0);
        let d_ref = 2.0 * z_ref + cref - vec2<f32>(2.0, 0.0);
        // dN/S and dD/S.
        var ns = 2.0 * vec2<f32>(
            z_ref.x * w.x - z_ref.y * w.y,
            z_ref.x * w.y + z_ref.y * w.x,
        ) + d0_term;
        ns = ns + perturb.s * vec2<f32>(w.x * w.x - w.y * w.y, 2.0 * w.x * w.y);
        let ds = 2.0 * w + d0_term;
"#
    } else {
        r#"        // Magnet II: the cubic numerator and quadratic
        // denominator, both carrying c.
        let cref = perturb.ref_c;
        let cm1 = cref - vec2<f32>(1.0, 0.0);
        let cm2 = cref - vec2<f32>(2.0, 0.0);
        let c12 = vec2<f32>(cm1.x * cm2.x - cm1.y * cm2.y, cm1.x * cm2.y + cm1.y * cm2.x);
        let zr2 = vec2<f32>(z_ref.x * z_ref.x - z_ref.y * z_ref.y, 2.0 * z_ref.x * z_ref.y);
        let zr3 = vec2<f32>(zr2.x * z_ref.x - zr2.y * z_ref.y, zr2.x * z_ref.y + zr2.y * z_ref.x);
        let n_ref = zr3 + 3.0 * vec2<f32>(
            cm1.x * z_ref.x - cm1.y * z_ref.y,
            cm1.x * z_ref.y + cm1.y * z_ref.x,
        ) + c12;
        let d_ref = 3.0 * zr2 + 3.0 * vec2<f32>(
            cm2.x * z_ref.x - cm2.y * z_ref.y,
            cm2.x * z_ref.y + cm2.y * z_ref.x,
        ) + c12 + vec2<f32>(1.0, 0.0);
        // Shared pieces of dN/S and dD/S.
        let zw = vec2<f32>(z_ref.x * w.x - z_ref.y * w.y, z_ref.x * w.y + z_ref.y * w.x);
        let w2 = vec2<f32>(w.x * w.x - w.y * w.y, 2.0 * w.x * w.y);
        let zpw = z_ref + perturb.s * w;
        let dcz = vec2<f32>(
            d0_term.x * zpw.x - d0_term.y * zpw.y,
            d0_term.x * zpw.y + d0_term.y * zpw.x,
        );
        // dc*(2C - 3 + S*dc), the (c-1)(c-2) term's delta.
        let tail_b = 2.0 * cref - vec2<f32>(3.0, 0.0) + perturb.s * d0_term;
        let tail = vec2<f32>(
            d0_term.x * tail_b.x - d0_term.y * tail_b.y,
            d0_term.x * tail_b.y + d0_term.y * tail_b.x,
        );
        let z2w = vec2<f32>(zr2.x * w.x - zr2.y * w.y, zr2.x * w.y + zr2.y * w.x);
        let zw2 = vec2<f32>(zw.x * w.x - zw.y * w.y, zw.x * w.y + zw.y * w.x);
        let w3 = vec2<f32>(w2.x * w.x - w2.y * w.y, w2.x * w.y + w2.y * w.x);
        var ns = 3.0 * z2w + 3.0 * perturb.s * zw2 + perturb.s * perturb.s * w3;
        ns = ns + 3.0 * vec2<f32>(cm1.x * w.x - cm1.y * w.y, cm1.x * w.y + cm1.y * w.x);
        ns = ns + 3.0 * dcz + tail;
        var ds = 3.0 * (2.0 * zw + perturb.s * w2);
        ds = ds + 3.0 * vec2<f32>(cm2.x * w.x - cm2.y * w.y, cm2.x * w.y + cm2.y * w.x);
        ds = ds + 3.0 * dcz + tail;
"#
    };
    let tail = r#"        // q = N/D, the REFERENCE quotient, at O(1) in f32.
        let dr2 = max(dot(d_ref, d_ref), 1e-30);
        let q = vec2<f32>(
            (n_ref.x * d_ref.x + n_ref.y * d_ref.y) / dr2,
            (n_ref.y * d_ref.x - n_ref.x * d_ref.y) / dr2,
        );
        // dq/S = (dN/S - q*dD/S) / (D + S*dD/S).
        let top = ns - vec2<f32>(q.x * ds.x - q.y * ds.y, q.x * ds.y + q.y * ds.x);
        let div = d_ref + perturb.s * ds;
        let dv2 = max(dot(div, div), 1e-30);
        let qs = vec2<f32>(
            (top.x * div.x + top.y * div.y) / dv2,
            (top.y * div.x - top.x * div.y) / dv2,
        );
        // f = q^2, so df/S = 2*q*(dq/S) + S*(dq/S)^2.
        var w_new = 2.0 * vec2<f32>(q.x * qs.x - q.y * qs.y, q.x * qs.y + q.y * qs.x);
        w_new = w_new + perturb.s * vec2<f32>(qs.x * qs.x - qs.y * qs.y, 2.0 * qs.x * qs.y);
"#;
    format!("{head}{tail}")
}

/// The floatexp flavor of the same step.
/// The default Zhuoran rebase: restart the reference at index 0.
fn rebase_default() -> String {
    "        let rebase_delta = z_full - ref_z(0u);\n\
     \x20       if (m >= perturb.orbit_len - 1u\n\
     \x20           || dot(rebase_delta, rebase_delta) < dot(delta, delta)) {\n\
     \x20           w = rebase_delta * perturb.inv_s;\n\
     \x20           m = 0u;\n\
     \x20       }"
        .to_string()
}

/// Bytes of per-pixel iteration state the assembled deep-rung shader
/// declares, per tier.
///
/// The renderer allocates `iter_state` from this, so it is the single
/// place the Rust buffer and the WGSL struct agree -- and
/// `iter_state_stride_matches_the_shader` measures the assembled
/// struct against it rather than trusting the arithmetic.
pub const ITER_STATE_BYTES: u64 = 48;

/// Phoenix carries a second delta in full floatexp: +hi +lo +e, and
/// a pad word to keep the struct 8-byte aligned.
pub const ITER_STATE_BYTES_PHOENIX: u64 = 72;

/// The widest state any tier declares. The render-pixel cap uses this
/// so a tier switch can never need a buffer the device will not bind.
pub const ITER_STATE_BYTES_MAX: u64 = ITER_STATE_BYTES_PHOENIX;

/// Per-pixel state a given tier needs.
pub fn iter_state_bytes(tier: PerturbTier, floatexp: bool) -> u64 {
    match (tier, floatexp) {
        // The scaled rung hides Phoenix's history in `w_lo`, which is
        // dead there, so only the deep rung actually grows.
        (PerturbTier::Phoenix, true) | (PerturbTier::Manowar, true) => ITER_STATE_BYTES_PHOENIX,
        _ => ITER_STATE_BYTES,
    }
}

/// The struct tail, resume and save splices for a tier. They are
/// returned together because they are only ever correct together.
fn state_tail(tier: PerturbTier) -> (String, String, String) {
    match tier {
        PerturbTier::Phoenix | PerturbTier::Manowar => state_tail_phoenix(),
        _ => state_tail_default(),
    }
}

/// How the pixel's delta STARTS, per tier and rung.
///
/// The parameter plane starts a pixel at z_0 = 0, which is the
/// reference's own start, so its delta is zero. The Julia plane
/// starts it at the pixel itself, so its delta is d0. Manowar seeds
/// z_0 = z_-1 = c -- the pixel again -- so BOTH of its deltas start
/// at d0 even though it is a parameter-plane map.
fn w_init(tier: PerturbTier, floatexp: bool) -> String {
    match (tier, floatexp) {
        (PerturbTier::Manowar, false) => "    var w = d0;".to_string(),
        (PerturbTier::Manowar, true) => "    w = d0;".to_string(),
        (_, false) => "    var w = select(vec2<f32>(0.0, 0.0), d0, is_julia_perturb);".to_string(),
        (_, true) => "    if (is_julia_perturb) {
        w = d0;
    } else {
        w = cfe2_zero();
    }"
            .to_string(),
    }
}

/// The history's initial value. Manowar seeds z_-1 = c, so its
/// previous delta starts at d0 exactly as its current one does;
/// every other tier starts with no history at all.
fn w_prev_init(tier: PerturbTier, floatexp: bool) -> String {
    match (tier, floatexp) {
        (PerturbTier::Manowar, false) => "    var w_prev = d0;".to_string(),
        (PerturbTier::Manowar, true) => "    var w_prev = d0;".to_string(),
        (_, false) => "    var w_prev = vec2<f32>(0.0, 0.0);".to_string(),
        (_, true) => "    var w_prev = cfe2_zero();".to_string(),
    }
}

/// The deep rung's default state tail: nothing.
///
/// Only a two-term recurrence needs more per-pixel state than the
/// 48 B every tier carries, and the buffer is sized to match (see
/// [`iter_state_bytes`]), so these three splices move together.
fn state_tail_default() -> (String, String, String) {
    (String::new(), String::new(), String::new())
}

/// Phoenix's deep state tail: the previous delta, in full floatexp.
///
/// The scaled rung hides this in `w_lo`, which is dead there. On the
/// deep rung every field is live, so the struct genuinely grows --
/// 48 B/px to 72 B/px, for Phoenix renders only. The mantissa keeps
/// its LOW half: the history feeds `p * w_prev` straight into the
/// next delta, and truncating it to single f32 would inject exactly
/// the 2^-24 reseed error the DF rung exists to avoid.
fn state_tail_phoenix() -> (String, String, String) {
    (
        "    wp: vec2<f32>,     // Phoenix: previous delta, DF mantissa hi
             wp_lo: vec2<f32>,  // ... mantissa lo
             wp_e: i32,         // ... exponent
             wp_pad: u32,       // keeps the struct 8-byte aligned at 72 B"
            .to_string(),
        "        w_prev = CFe2(st.wp, st.wp_lo, st.wp_e);".to_string(),
        "            w_prev.hi, w_prev.lo, w_prev.e, 0u,".to_string(),
    )
}

/// Phoenix, floatexp rung: the scaled step's two-term recurrence with
/// every delta in extended range.
///
/// `p` is a plain f32 pair from the formula uniform -- an O(1)
/// number, so it multiplies the extended-range history directly.
/// Manowar on the deep rung: the same two-term step with p = 1.
fn delta_step_manowar_fe() -> String {
    delta_step_two_term_fe("vec2<f32>(1.0, 0.0)")
}

fn delta_step_phoenix_fe() -> String {
    delta_step_two_term_fe("vec2<f32>(params.fparams[0][0], params.fparams[0][1])")
}

/// The shared deep-rung two-term step: the canonical p = 2
/// floatexp step, plus a term in the previous delta.
fn delta_step_two_term_fe(p_expr: &str) -> String {
    // The quadratic part is the CANONICAL p = 2 floatexp step, reused
    // rather than restated: it owns the reference's variable names and
    // the with-its-own-exponent multiply, and a copy here drifted from
    // them within the hour.
    let mut out = delta_step_floatexp(2);
    out.push_str(&format!(
        "
        // ... plus the history term, p * w_prev. p is an O(1)
                 // number from the formula uniform, so it multiplies the
                 // extended-range history directly.
                 let pp = {p_expr};
                 w_new = cfe2_add(w_new, cfe2_mul_c32(w_prev, pp));
                 // The history advances to the delta being left behind;
                 // the template assigns w = w_new right after this.
                 w_prev = w;"));

    out
}

/// The deep rung's default rebase: restart the reference at index 0.
///
/// The CONDITION uses the f32 view; the ASSIGNMENT rebuilds the delta
/// in double-float from the reference's own DF entries, so the wrap
/// does not truncate a pixel's history to f32 -- the reseed-precision
/// loss this rung exists to fix.
/// Lambda on the deep rung. Same algebra as [`delta_step_lambda`],
/// but `w` is the absolute delta here rather than `delta/S`, so the
/// scale factors drop out entirely:
///
///   dP = d*(1 - 2Z - d)
///   d' = C*dP + dc*(Z(1-Z) + dP)
///
/// The reference enters through `cfe2_mul_zdfe`, which applies the DF
/// mantissa with its OWN exponent -- so a near-nucleus iterate cannot
/// underflow out of the multiplier, the same care the power tier takes.
fn delta_step_lambda_fe() -> String {
    r#"        // Lambda (floatexp): d' = C*dP + dc*(P + dP).
        let one_fe = cfe2_from_f32(vec2<f32>(1.0, 0.0));
        let z_fe = cfe2_mul_zdfe(one_fe, z_ref_m, z_ref_lo_m, z_ref_e);
        let two_z_fe = cfe2_mul_zdfe(one_fe, 2.0 * z_ref_m, 2.0 * z_ref_lo_m, z_ref_e);
        // t = 1 - 2Z - d. Near Z = 1/2 this cancels, but that is the
        // map's own vanishing multiplier rather than a lost delta:
        // the absolute error stays at the DF rounding level and the
        // product below is correspondingly small.
        let t_fe = cfe2_add(one_fe, cfe2_mul_c32(cfe2_add(two_z_fe, w), vec2<f32>(-1.0, 0.0)));
        let dp = cfe2_mul(w, t_fe);
        var w_new = cfe2_mul_c32(dp, perturb.ref_c);
        if (!is_julia_perturb) {
            let om_fe = cfe2_add(one_fe, cfe2_mul_c32(z_fe, vec2<f32>(-1.0, 0.0)));
            let p_ref = cfe2_mul(z_fe, om_fe);
            w_new = cfe2_add(w_new, cfe2_mul(d0, cfe2_add(p_ref, dp)));
        }"#
    .to_string()
}

/// Feather on the deep rung. Same quotient form; `w` is the absolute
/// delta, so the scale factors drop out and the divisor is built by
/// converting `dD` DOWN to f32 -- which flushes to zero exactly when
/// it could not have changed an O(1) f32 value anyway.
fn delta_step_feather_fe(p: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!("        // Feather (floatexp), z^{p}/(1+x^2-i y^2)+c.\n"));
    out.push_str("        let fzr1 = z_ref_m;\n");
    for k in 2..p {
        out.push_str(&format!(
            "        let fzr{k} = vec2<f32>(fzr{}.x * z_ref_m.x - fzr{}.y * z_ref_m.y, fzr{}.x * z_ref_m.y + fzr{}.y * z_ref_m.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str("        let fu1 = w;\n");
    for k in 2..=p {
        out.push_str(&format!(
            "        let fu{k} = cfe2_mul(fu{}, w);\n", k - 1
        ));
    }
    out.push_str("        var n_s = cfe2_zero();\n");
    for k in 1..=p {
        let coeff = binomial(p, k);
        if k == p {
            out.push_str(&format!(
                "        n_s = cfe2_add(n_s, cfe2_mul_c32(fu{k}, vec2<f32>({coeff}.0, 0.0)));\n"
            ));
        } else {
            let zp = p - k;
            out.push_str(&format!(
                "        n_s = cfe2_add(n_s, cfe2_mul_cfe32(cfe2_mul_c32(fu{k}, vec2<f32>({coeff}.0, 0.0)), fzr{zp}, {zp} * z_ref_e));\n"
            ));
        }
    }
    out.push_str(
        r#"        // dD component-wise, in floatexp. The reference
        // component enters with its own exponent so a tiny iterate
        // cannot underflow out of the multiplier.
        let zx = cfe2_mul_cfe32(cfe2_from_f32(vec2<f32>(1.0, 0.0)), vec2<f32>(z_ref_m.x, 0.0), z_ref_e);
        let zy = cfe2_mul_cfe32(cfe2_from_f32(vec2<f32>(1.0, 0.0)), vec2<f32>(z_ref_m.y, 0.0), z_ref_e);
        let wx = cfe2_from_f32(vec2<f32>(cfe2_to_f32(w).x, 0.0));
        let wy = cfe2_from_f32(vec2<f32>(cfe2_to_f32(w).y, 0.0));
        let dsx = cfe2_add(cfe2_mul_c32(cfe2_mul(zx, wx), vec2<f32>(2.0, 0.0)), cfe2_mul(wx, wx));
        let dsy = cfe2_add(cfe2_mul_c32(cfe2_mul(zy, wy), vec2<f32>(2.0, 0.0)), cfe2_mul(wy, wy));
        let d_s32 = vec2<f32>(cfe2_to_f32(dsx).x, -cfe2_to_f32(dsy).x);
        // The reference's N, D, q at O(1), in f32.
        let zr32 = z_ref_m * exp2(f32(z_ref_e));
        var f_num = zr32;
"#,
    );
    for _ in 1..p {
        out.push_str("        f_num = vec2<f32>(f_num.x * zr32.x - f_num.y * zr32.y, f_num.x * zr32.y + f_num.y * zr32.x);\n");
    }
    out.push_str(
        r#"        let f_den = vec2<f32>(1.0 + zr32.x * zr32.x, -(zr32.y * zr32.y));
        let f_dd = dot(f_den, f_den);
        let q = vec2<f32>(
            (f_num.x * f_den.x + f_num.y * f_den.y) / f_dd,
            (f_num.y * f_den.x - f_num.x * f_den.y) / f_dd,
        );
        let div = f_den + d_s32;
        let dv2 = dot(div, div);
        let inv = vec2<f32>(div.x / dv2, -div.y / dv2);
        let d_s_fe = cfe2_from_f32(d_s32);
        let top = cfe2_add(n_s, cfe2_mul_c32(d_s_fe, vec2<f32>(-q.x, -q.y)));
        var w_new = cfe2_mul_c32(top, inv);
        if (!is_julia_perturb) {
            w_new = cfe2_add(w_new, d0);
        }
"#,
    );
    out
}

/// McMullen on the deep rung.
///
/// The binomials run in floatexp; the two full-size powers and the
/// division stay in f32, because they are O(1) COEFFICIENTS and the
/// delta never passes through them.
fn delta_step_mcmullen_fe(n: u32, m: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!("        // McMullen z^{n} + c/z^{m} (floatexp).\n"));
    for (p, prefix) in [(n, "a"), (m, "b")] {
        out.push_str(&format!("        let {prefix}z1 = z_ref_m;\n"));
        for k in 2..p {
            out.push_str(&format!(
                "        let {prefix}z{k} = vec2<f32>({prefix}z{}.x * z_ref_m.x - {prefix}z{}.y * z_ref_m.y, {prefix}z{}.x * z_ref_m.y + {prefix}z{}.y * z_ref_m.x);\n",
                k - 1, k - 1, k - 1, k - 1
            ));
        }
        out.push_str(&format!("        let {prefix}u1 = w;\n"));
        for k in 2..=p {
            out.push_str(&format!("        let {prefix}u{k} = cfe2_mul({prefix}u{}, w);\n", k - 1));
        }
        out.push_str(&format!("        var {prefix}d = cfe2_zero();\n"));
        for k in 1..=p {
            let c = binomial(p, k);
            if k == p {
                out.push_str(&format!(
                    "        {prefix}d = cfe2_add({prefix}d, cfe2_mul_c32({prefix}u{k}, vec2<f32>({c}.0, 0.0)));\n"
                ));
            } else {
                let zp = p - k;
                out.push_str(&format!(
                    "        {prefix}d = cfe2_add({prefix}d, cfe2_mul_cfe32(cfe2_mul_c32({prefix}u{k}, vec2<f32>({c}.0, 0.0)), {prefix}z{zp}, {zp} * z_ref_e));\n"
                ));
            }
        }
    }
    out.push_str(
        r#"        let zr32 = z_ref_m * exp2(f32(z_ref_e));
        let zf = zr32 + cfe2_to_f32(w);
        var zf_m = zf;
"#,
    );
    for _ in 1..m {
        out.push_str("        zf_m = vec2<f32>(zf_m.x * zf.x - zf_m.y * zf.y, zf_m.x * zf.y + zf_m.y * zf.x);\n");
    }
    out.push_str("        var zr_m = zr32;\n");
    for _ in 1..m {
        out.push_str("        zr_m = vec2<f32>(zr_m.x * zr32.x - zr_m.y * zr32.y, zr_m.x * zr32.y + zr_m.y * zr32.x);\n");
    }
    out.push_str(
        r#"        let den = vec2<f32>(zf_m.x * zr_m.x - zf_m.y * zr_m.y, zf_m.x * zr_m.y + zf_m.y * zr_m.x);
        let den2 = max(dot(den, den), 1e-30);
        let cref = perturb.ref_c;
        // Multiply the floatexp dM by the f32 factor -C/den, formed at
        // O(1) where an f32 reciprocal is exact enough.
        let inv = vec2<f32>(den.x / den2, -den.y / den2);
        let fac = vec2<f32>(
            -(cref.x * inv.x - cref.y * inv.y),
            -(cref.x * inv.y + cref.y * inv.x),
        );
        var w_new = cfe2_add(ad, cfe2_mul_c32(bd, fac));
        if (!is_julia_perturb) {
            let zf2 = max(dot(zf_m, zf_m), 1e-30);
            let invm = vec2<f32>(zf_m.x / zf2, -zf_m.y / zf2);
            w_new = cfe2_add(w_new, cfe2_mul_c32(d0, invm));
        }
"#,
    );
    out
}

/// Magnet on the deep rung.
///
/// Same algebra as [`delta_step_magnet`] with `w` the ABSOLUTE delta,
/// so the scale factors drop out. The deltas run in floatexp; the
/// reference's N, D and quotient are O(1) and stay in f32, as does the
/// divisor — `dD` converts down first, flushing to zero exactly when
/// it could not have moved an O(1) f32 value.
fn delta_step_magnet_fe(variant: u32) -> String {
    let head = if variant == 0 {
        r#"        // Magnet I (floatexp): N = z^2+c-1, D = 2z+c-2.
        let cref = perturb.ref_c;
        let zr32 = z_ref_m * exp2(f32(z_ref_e));
        let zr2 = vec2<f32>(zr32.x * zr32.x - zr32.y * zr32.y, 2.0 * zr32.x * zr32.y);
        let n_ref = zr2 + cref - vec2<f32>(1.0, 0.0);
        let d_ref = 2.0 * zr32 + cref - vec2<f32>(2.0, 0.0);
        var ns = cfe2_add(cfe2_mul_c32(w, 2.0 * zr32), cfe2_sqr(w));
        var ds = cfe2_mul_c32(w, vec2<f32>(2.0, 0.0));
        if (!is_julia_perturb) {
            ns = cfe2_add(ns, d0);
            ds = cfe2_add(ds, d0);
        }
"#
    } else {
        r#"        // Magnet II (floatexp).
        let cref = perturb.ref_c;
        let zr32 = z_ref_m * exp2(f32(z_ref_e));
        let cm1 = cref - vec2<f32>(1.0, 0.0);
        let cm2 = cref - vec2<f32>(2.0, 0.0);
        let c12 = vec2<f32>(cm1.x * cm2.x - cm1.y * cm2.y, cm1.x * cm2.y + cm1.y * cm2.x);
        let zr2 = vec2<f32>(zr32.x * zr32.x - zr32.y * zr32.y, 2.0 * zr32.x * zr32.y);
        let zr3 = vec2<f32>(zr2.x * zr32.x - zr2.y * zr32.y, zr2.x * zr32.y + zr2.y * zr32.x);
        let n_ref = zr3 + 3.0 * vec2<f32>(
            cm1.x * zr32.x - cm1.y * zr32.y,
            cm1.x * zr32.y + cm1.y * zr32.x,
        ) + c12;
        let d_ref = 3.0 * zr2 + 3.0 * vec2<f32>(
            cm2.x * zr32.x - cm2.y * zr32.y,
            cm2.x * zr32.y + cm2.y * zr32.x,
        ) + c12 + vec2<f32>(1.0, 0.0);
        let zw = cfe2_mul_c32(w, zr32);
        let w2 = cfe2_sqr(w);
        let z2w = cfe2_mul_c32(w, zr2);
        let zw2 = cfe2_mul(zw, w);
        let w3 = cfe2_mul(w2, w);
        var ns = cfe2_add(
            cfe2_add(cfe2_mul_c32(z2w, vec2<f32>(3.0, 0.0)), cfe2_mul_c32(zw2, vec2<f32>(3.0, 0.0))),
            w3,
        );
        ns = cfe2_add(ns, cfe2_mul_c32(w, 3.0 * cm1));
        var ds = cfe2_mul_c32(cfe2_add(cfe2_mul_c32(zw, vec2<f32>(2.0, 0.0)), w2), vec2<f32>(3.0, 0.0));
        ds = cfe2_add(ds, cfe2_mul_c32(w, 3.0 * cm2));
        if (!is_julia_perturb) {
            // dc*(Z+d) and dc*(2C-3+dc): the c-carrying terms, which
            // enter the numerator and denominator identically.
            let zpw = zr32 + cfe2_to_f32(w);
            let dcz = cfe2_mul_c32(d0, 3.0 * zpw);
            let tail_b = 2.0 * cref - vec2<f32>(3.0, 0.0) + cfe2_to_f32(d0);
            let tail = cfe2_mul_c32(d0, tail_b);
            ns = cfe2_add(cfe2_add(ns, dcz), tail);
            ds = cfe2_add(cfe2_add(ds, dcz), tail);
        }
"#
    };
    let tail = r#"        let dr2 = max(dot(d_ref, d_ref), 1e-30);
        let q = vec2<f32>(
            (n_ref.x * d_ref.x + n_ref.y * d_ref.y) / dr2,
            (n_ref.y * d_ref.x - n_ref.x * d_ref.y) / dr2,
        );
        let top = cfe2_add(ns, cfe2_mul_c32(ds, vec2<f32>(-q.x, -q.y)));
        let div = d_ref + cfe2_to_f32(ds);
        let dv2 = max(dot(div, div), 1e-30);
        let inv = vec2<f32>(div.x / dv2, -div.y / dv2);
        let qs = cfe2_mul_c32(top, inv);
        var w_new = cfe2_add(cfe2_mul_c32(qs, 2.0 * q), cfe2_mul(qs, qs));
"#;
    format!("{head}{tail}")
}

fn rebase_default_fe() -> String {
    "        let rebase_delta = z_full - ref_z(0u);
             if (m >= perturb.orbit_len - 1u
                 || dot(rebase_delta, rebase_delta) < dot(delta, delta)) {
                 w = fe_rebase_delta(w, min(m, perturb.orbit_len - 1u));
                 m = 0u;
             }"
        .to_string()
}

/// Phoenix, floatexp rung: rebase the PAIR onto the orbit start.
///
/// Same target and same gate as the scaled rung (see
/// [`rebase_phoenix`]) -- index 0, whose state is the pair
/// (Z_0, Z_-1) = (Z_0, 0) -- with both deltas rebuilt in double-float
/// rather than through an f32 subtraction.
fn rebase_phoenix_fe() -> String {
    rebase_two_term_fe("vec2<f32>(0.0, 0.0)", "fe_rebase_from_zero(w_prev, mp)")
}

/// Manowar: the history rebases against Z_0, so its delta is
/// rebuilt against the orbit start exactly as the current one is.
fn rebase_manowar_fe() -> String {
    rebase_two_term_fe("ref_z(0u)", "fe_rebase_delta(w_prev, mp)")
}

/// The shared deep-rung pair rebase. Both deltas are rebuilt in
/// double-float from the reference's own DF entries, never by an
/// f32 subtraction of two O(1) iterates.
fn rebase_two_term_fe(prev_target: &str, prev_rebase: &str) -> String {
    format!(
    "        let delta_prev = cfe2_to_f32(w_prev);
             let mp = min(max(m, 1u) - 1u, perturb.orbit_len - 1u);
             let z_prev_full = select(vec2<f32>(0.0, 0.0), ref_z(mp), m > 0u)
                 + delta_prev;
             let rebase_delta = z_full - ref_z(0u);
             // Z_-1 is zero: the previous iterate rebases against
             // nothing at all, so it keeps its full value.
             if (m >= perturb.orbit_len - 1u
                 || dot(rebase_delta, rebase_delta) + dot(z_prev_full, z_prev_full)
                     < dot(delta, delta) + dot(delta_prev, delta_prev)) {{
                 // (select takes no struct arguments in WGSL.)
                 var wp_new = w_prev;
                 if (m > 0u) {{
                     wp_new = {prev_rebase};
                 }}
                 w = fe_rebase_delta(w, min(m, perturb.orbit_len - 1u));
                 w_prev = wp_new;
                 m = 0u;
             }}")
}

/// Phoenix rebases onto the orbit's START, as a PAIR.
///
/// A two-term recurrence's state is (z, z_prev), so a rebase has to
/// move both deltas onto the same reference index or the history is
/// measured against a different iterate -- a wrong orbit, not a noisy
/// one. The index to move them onto is 0, whose state is the pair
/// (Z_0, Z_-1) = (Z_0, 0): the reference began with no history, and
/// so did the pixel.
///
/// Index 1 is the trap. It looks like the natural choice -- the
/// earliest index whose PREDECESSOR exists in the array -- but
/// Z_1 = c, an O(1) number, and `z_full - Z_1` in f32 is
/// catastrophic cancellation. The surviving delta quantises to
/// ulp(c), which at zoom 22 is about EIGHTY PIXELS across, and the
/// image breaks into displaced rectangular blocks (twice as wide as
/// tall whenever the two centre components straddle a binade). Index
/// 0 subtracts zero on the parameter plane, exactly like every
/// one-term tier.
///
/// The gate is the pair's norm, not the current delta's alone. A
/// rebase that shrinks z while leaving z_prev far from the reference
/// would blow up the p*w_prev term on the very next step.
fn rebase_phoenix() -> String {
    // Phoenix's reference begins with no history: Z_-1 = 0.
    rebase_two_term("vec2<f32>(0.0, 0.0)")
}

/// Manowar seeds z_0 = z_-1 = c, so its history rebases against
/// Z_0 rather than against zero.
fn rebase_manowar() -> String {
    rebase_two_term("ref_z(0u)")
}

/// The shared two-term rebase: both deltas move onto reference
/// index 0 together, gated on the PAIR's norm.
fn rebase_two_term(prev_target: &str) -> String {
    format!(
    "        // Previous full iterate: Z_(m-1) + S*w_prev, against the target below.
             let zp_ref = select(vec2<f32>(0.0, 0.0),
                 ref_z(min(max(m, 1u) - 1u, perturb.orbit_len - 1u)), m > 0u);
             let z_prev_full = zp_ref + perturb.s * w_prev;
             let delta_prev = perturb.s * w_prev;
             let rebase_delta = z_full - ref_z(0u);
             let rebase_prev = z_prev_full - {prev_target};
             // At m == 0 both sides are equal by construction, so
             // this cannot loop.
             if (m >= perturb.orbit_len - 1u
                 || dot(rebase_delta, rebase_delta) + dot(rebase_prev, rebase_prev)
                     < dot(delta, delta) + dot(delta_prev, delta_prev)) {{
                 w = rebase_delta * perturb.inv_s;
                 w_prev = rebase_prev * perturb.inv_s;
                 m = 0u;
             }}")
}

/// Phoenix: z' = z^2 + c + p*z_prev, so the delta step is the
/// quadratic one plus a term in the PREVIOUS delta, and the history
/// advances with it.
///
/// `p` is read straight from the formula-parameter uniform, which the
/// perturbed templates already carry; it is also part of the
/// reference orbit's identity (see MapId), so the reference this runs
/// against was built with the same value.
fn delta_step_phoenix() -> String {
    delta_step_two_term("vec2<f32>(params.fparams[0][0], params.fparams[0][1])")
}

/// Manowar is the same step with p pinned to 1 -- a literal,
/// because the formula has no parameters and there is no uniform
/// slot to read it from.
fn delta_step_manowar() -> String {
    delta_step_two_term("vec2<f32>(1.0, 0.0)")
}

/// The shared two-term delta step: the quadratic part, plus a
/// term in the PREVIOUS delta, plus the history advancing.
fn delta_step_two_term(p_expr: &str) -> String {
    format!(
    "        // w' = 2 Z w + S w^2 + p w_prev + d0\n\
     \x20       let pp = {p_expr};\n\
     \x20       var w_new = 2.0 * vec2<f32>(\n\
     \x20           z_ref.x * w.x - z_ref.y * w.y,\n\
     \x20           z_ref.x * w.y + z_ref.y * w.x,\n\
     \x20       ) + d0_term + vec2<f32>(\n\
     \x20           pp.x * w_prev.x - pp.y * w_prev.y,\n\
     \x20           pp.x * w_prev.y + pp.y * w_prev.x,\n\
     \x20       );\n\
     \x20       if ((perturb.flags & 1u) == 0u) {{\n\
     \x20           w_new = w_new + perturb.s * vec2<f32>(w.x * w.x - w.y * w.y, 2.0 * w.x * w.y);\n\
     \x20       }}\n\
     \x20       // History advances to the delta we are leaving; `w` is\n\
     \x20       // still the old value here, and the template assigns\n\
     \x20       // w = w_new immediately after this splice.\n\
     \x20       w_prev = w;")
}

/// Tricorn / Multicorn, floatexp rung: the same conjugation, applied
/// to the DF mantissa pair and to the delta's hi/lo halves. The
/// exponent is untouched -- conjugation only flips a sign.
fn delta_step_conj_fe(p: u32) -> String {
    format!(
        "        // Anti-holomorphic: conj(z)^p + c, deep rung.\n\
         \x20       let zcm = vec2<f32>(z_ref_m.x, -z_ref_m.y);\n\
         \x20       let zclo = vec2<f32>(z_ref_lo_m.x, -z_ref_lo_m.y);\n\
         \x20       let wc = CFe2(vec2<f32>(w.hi.x, -w.hi.y), vec2<f32>(w.lo.x, -w.lo.y), w.e);\n{}",
        delta_step_floatexp_on(p, "zcm", "zclo", "wc")
    )
}

/// Tricorn / Multicorn, scaled rung: the power step over conjugated
/// operands. The reference and the delta are conjugated on entry; the
/// RESULT is not, because `conj(Z+d)^p - conj(Z)^p` is already the
/// new delta rather than its conjugate.
fn delta_step_conj(p: u32) -> String {
    format!(
        "        // Anti-holomorphic: conj(z)^p + c. Same binomial as the\n\
         \x20       // power tier, over conj(Z) and conj(w).\n\
         \x20       let zc = vec2<f32>(z_ref.x, -z_ref.y);\n\
         \x20       let wc = vec2<f32>(w.x, -w.y);\n{}",
        delta_step_scaled_on(p, "zc", "wc")
    )
}

fn delta_step_floatexp(p: u32) -> String {
    delta_step_floatexp_on(p, "z_ref_m", "z_ref_lo_m", "w")
}

/// The floatexp binomial step over NAMED operands -- the deep-rung
/// twin of [`delta_step_scaled_on`], so the Tricorn family can feed
/// it conjugates instead of duplicating the derivation.
fn delta_step_floatexp_on(p: u32, zm: &str, zlo: &str, w: &str) -> String {
    if p == 2 {
        return format!(
            "        // delta' = 2 Z delta + delta^2 (+ delta_c on the parameter\n\
             \x20       // plane) - DF mantissas AND DF reference (~2^-48),\n\
             \x20       // the reference read with its own exponent so a\n\
             \x20       // near-nucleus iterate cannot underflow out of the\n\
             \x20       // multiplier.\n\
             \x20       var w_new = cfe2_add(\n\
             \x20           cfe2_mul_zdfe({w}, 2.0 * {zm}, 2.0 * {zlo}, z_ref_e),\n\
             \x20           cfe2_sqr({w}),\n\
             \x20       );\n\
             \x20       if (!is_julia_perturb) {{\n\
             \x20           w_new = cfe2_add(w_new, d0);\n\
             \x20       }}"
        );
    }
    let mut out = String::new();
    out.push_str(&format!("        // Binomial delta step for z^{p} + c (floatexp).\n"));
    // zr{k} is the k-th power of the reference MANTISSA; the matching
    // exponent k*z_ref_e is applied at the term multiply below. The
    // mantissa stays in [1,4), so no power of it underflows even when
    // the iterate itself is far below f32's range.
    out.push_str(&format!("        let zr1 = {zm};\n"));
    for k in 2..p {
        out.push_str(&format!(
            "        let zr{k} = vec2<f32>(zr{}.x * {zm}.x - zr{}.y * {zm}.y, zr{}.x * {zm}.y + zr{}.y * {zm}.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str(&format!("        let u1 = {w};\n"));
    for k in 2..=p {
        out.push_str(&format!("        let u{k} = cfe2_mul(u{}, {w});\n", k - 1));
    }
    out.push_str("        var w_new = cfe2_zero();\n");
    out.push_str("        if (!is_julia_perturb) {\n            w_new = d0;\n        }\n");
    for k in 1..=p {
        let coeff = binomial(p, k);
        if k == p {
            out.push_str(&format!(
                "        w_new = cfe2_add(w_new, cfe2_mul_c32(u{k}, vec2<f32>({coeff}.0, 0.0)));\n"
            ));
        } else {
            let zp = p - k;
            out.push_str(&format!(
                "        w_new = cfe2_add(w_new, cfe2_mul_cfe32(u{k}, {coeff}.0 * zr{zp}, {zp} * z_ref_e));\n"
            ));
        }
    }
    out
}

/// Assemble the perturbed (deep-zoom) WGSL for one coloring at a
/// given integer power (`z^p + c` — the "clean" binomial tier; p = 2
/// is Mandelbrot and emits the original hand-written step). The
/// coloring axis is fully preserved - the loop reconstructs the full
/// orbit value every iteration for the rebase test, which is exactly
/// the summary the colorings consume. `floatexp` picks the deep rung.
pub fn assemble_perturbed(coloring: &ColoringDef, floatexp: bool, tier: PerturbTier) -> String {
    let needs_accum = coloring.has_feature(ColoringFeature::NeedsOrbitAccum);
    let colors_interior = coloring.has_feature(ColoringFeature::ColorsInterior);
    let bounded = coloring.has_feature(ColoringFeature::Bounded);
    // On this path the TIER is the map's identity -- there is no
    // FormulaDef in scope -- so convergence is a property of the tier.
    let convergent = tier.is_convergent();
    let template = if floatexp {
        PERTURBED_FE_TEMPLATE
    } else {
        PERTURBED_TEMPLATE
    };

    let mut out = Vec::new();
    for line in template.lines() {
        match line.trim() {
            "//__DELTA_STEP__" => out.push(match tier {
                PerturbTier::Power(p) => delta_step_scaled(p.clamp(2, 12)),
                PerturbTier::Ship(v) => delta_step_ship(v.min(5)),
                PerturbTier::Tricorn(p) => delta_step_conj(p.clamp(2, 12)),
                PerturbTier::Phoenix => delta_step_phoenix(),
                PerturbTier::Manowar => delta_step_manowar(),
                PerturbTier::Lambda => delta_step_lambda(),
                PerturbTier::Feather(p) => delta_step_feather(p.clamp(2, 8)),
                PerturbTier::McMullen(n, m) => {
                    delta_step_mcmullen(n.clamp(2, 8), m.clamp(1, 8))
                }
                PerturbTier::Magnet(v) => delta_step_magnet(v.min(1)),
            }),
            "//__DELTA_STEP_FE__" => out.push(match tier {
                PerturbTier::Power(p) => delta_step_floatexp(p.clamp(2, 12)),
                PerturbTier::Ship(v) => delta_step_ship_fe(v.min(5)),
                PerturbTier::Tricorn(p) => delta_step_conj_fe(p.clamp(2, 12)),
                PerturbTier::Phoenix => delta_step_phoenix_fe(),
                PerturbTier::Manowar => delta_step_manowar_fe(),
                PerturbTier::Lambda => delta_step_lambda_fe(),
                PerturbTier::Feather(p) => delta_step_feather_fe(p.clamp(2, 8)),
                PerturbTier::McMullen(n, m) => {
                    delta_step_mcmullen_fe(n.clamp(2, 8), m.clamp(1, 8))
                }
                PerturbTier::Magnet(v) => delta_step_magnet_fe(v.min(1)),
            }),
            "//__REBASE__" => out.push(match (tier, floatexp) {
                (PerturbTier::Phoenix, false) => rebase_phoenix(),
                (PerturbTier::Phoenix, true) => rebase_phoenix_fe(),
                (PerturbTier::Manowar, false) => rebase_manowar(),
                (PerturbTier::Manowar, true) => rebase_manowar_fe(),
                (_, false) => rebase_default(),
                (_, true) => rebase_default_fe(),
            }),
            // The deep rung's per-pixel state grows for a two-term
            // recurrence. These three move together, and the buffer
            // the renderer allocates must match: see
            // `iter_state_bytes`, and the test that measures the
            // assembled struct against it.
            "//__W_INIT__" => out.push(w_init(tier, floatexp)),
            "//__W_PREV_INIT__" => out.push(w_prev_init(tier, floatexp)),
            "//__ITER_STATE_TAIL__" => out.push(state_tail(tier).0),
            "//__STATE_RESUME_TAIL__" => out.push(state_tail(tier).1),
            "//__STATE_SAVE_TAIL__" => out.push(state_tail(tier).2),
            "//__COLORING__" => {
                out.push(format!(
                    "const COLORING_COLORS_INTERIOR: bool = {colors_interior};"
                ));
                // The perturbed rungs never iterate a derivative: `dz`
                // is a constant seed there. A coloring built on dz has
                // to KNOW that, or it renders something plausible from
                // a derivative that is really the number 1.
                out.push("const HAS_DERIVATIVE: bool = false;".to_string());
                out.push(format!("const COLORING_IS_BOUNDED: bool = {bounded};"));
                out.push(format!("// coloring: {}", coloring.name));
                out.push(coloring.wgsl.to_string());
            }
            "//__COLORING_ACCUM__" => {
                if needs_accum {
                    out.push(coloring.wgsl_accum.to_string());
                }
            }
            "//__ACCUM_DECL__" => {
                if needs_accum {
                    out.push(format!(
                        "    var accum_state: vec2<f32> = {};",
                        coloring.accum_init
                    ));
                } else {
                    // `var`, not `let`: the perturbed templates' chunk
                    // resume assigns it even when no accumulator runs.
                    out.push("    var accum_state = vec2<f32>(0.0, 0.0);".to_string());
                }
            }
            "//__CONVERGE_TEST__" => {
                // Same semantics as the direct template: a settled
                // orbit ends the loop, sets `escaped` so escape-count
                // and smooth colorings shade convergence SPEED, and
                // records `converged` for the basin colorings. Without
                // this a perturbed Convergent formula would run every
                // converging pixel to max_iter and report a different
                // image from the direct path -- which is why Magnet
                // could not perturb until it was here.
                if convergent {
                    out.push("        let conv_dz = z - z_before;".to_string());
                    out.push("        if (dot(conv_dz, conv_dz) < 1e-12) {".to_string());
                    out.push("            converged = true;".to_string());
                    out.push("            escaped = true;".to_string());
                    out.push("            n = i;".to_string());
                    out.push("            break;".to_string());
                    out.push("        }".to_string());
                }
            }
            "//__ACCUM_UPDATE__" => {
                if needs_accum {
                    out.push(
                        "        accum_state = coloring_accum(z_full, z_before, c_f32, accum_state);"
                            .to_string(),
                    );
                }
            }
            _ => out.push(line.to_string()),
        }
    }
    out.join("
")
}

/// Recolor pass: re-run the coloring + palette lookup from the
/// per-pixel [`IterResult`] records a completed iterate pass left
/// behind, without iterating anything. One dispatch over the full
/// image; the tail is a transcription of the iterate templates' tail
/// (kept bit-identical -- the cache-equivalence GPU test pins that a
/// recolor of stored results matches a full re-render exactly).
///
/// Assembled per COLORING (plus the derivative flag), not per
/// formula: the formula's work is all inside the records.
const RECOLOR_TEMPLATE: &str = r#"
// Recolor compute pass: coloring + palette from cached IterResult
// records. See assemble_recolor.

struct EscapeParams {
    center: vec2<f32>,
    julia_c: vec2<f32>,
    span: vec2<f32>,
    rot_cs: vec2<f32>,
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,
    bailout: f32,
    tile_y0: u32,
    damping: vec2<f32>,
    shade_flags: u32,
    _pad_shade0: u32,
    _pad_shade1: u32,
    _pad_shade2: u32,
    fparams: array<vec4<f32>, 4>,
    cparams: array<vec4<f32>, 4>,
    fdata: array<vec4<f32>, 64>,
}

struct IterResult {
    z: vec2<f32>,
    dz: vec2<f32>,
    accum: vec2<f32>,
    n: u32,
    tags: u32,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
@group(0) @binding(4) var height_tex: texture_storage_2d<r32float, write>;
@group(0) @binding(5) var<storage, read> results: array<IterResult>;

fn cparam(i: u32) -> f32 {
    return params.cparams[i / 4u][i % 4u];
}

struct OrbitSummary {
    z: vec2<f32>,
    n: u32,
    escaped: bool,
    converged: bool,
    period: u32,
    dz: vec2<f32>,
}

//__COLORING__

@compute @workgroup_size(8, 8, 1)
fn escape_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }
    let r = results[gid.y * params.width + gid.x];
    let escaped = (r.tags & 1u) != 0u;
    let converged = (r.tags & 2u) != 0u;
    let period = r.tags >> 2u;

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    // COVERAGE, not colour. A pixel that never escaped has no value
    // to show, so it is left ABSENT rather than painted black, and
    // the tonemap's background blend fills it -- which is what makes
    // the background colour apply to the interior, and what makes a
    // transparent export leave it transparent. Averaged by the
    // downsample like everything else, so the boundary antialiases
    // against the background instead of against black.
    var coverage = 0.0;
    var height = 0.0;
    if (escaped || COLORING_COLORS_INTERIOR) {
        let summary = OrbitSummary(r.z, r.n, escaped, converged, period, r.dz);
        let raw = coloring_map(summary, r.accum);
        let t = select(fract(raw), clamp(raw, 0.0, 1.0), COLORING_IS_BOUNDED);
        height = select(raw, t, params.shade_flags == 1u);
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
        coverage = 1.0;
    }
    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(rgb, coverage));
    textureStore(height_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(height, 0.0, 0.0, 0.0));
}
"#;

/// Assemble the recolor pass for one coloring.
///
/// `has_derivative` must match the iterate pass that produced the
/// records (the direct path compiles a real derivative orbit when the
/// formula has one and the coloring wants it; the perturbed rungs
/// never do) -- a derivative-reading coloring behaves differently
/// under each, and the renderer folds the flag into both the cache
/// key and the pipeline key so the two always agree.
pub fn assemble_recolor(coloring: &ColoringDef, has_derivative: bool) -> String {
    let colors_interior = coloring.has_feature(ColoringFeature::ColorsInterior);
    let bounded = coloring.has_feature(ColoringFeature::Bounded);
    let mut out = Vec::new();
    for line in RECOLOR_TEMPLATE.lines() {
        match line.trim() {
            "//__COLORING__" => {
                out.push(format!(
                    "const COLORING_COLORS_INTERIOR: bool = {colors_interior};"
                ));
                out.push(format!("const HAS_DERIVATIVE: bool = {has_derivative};"));
                out.push(format!("const COLORING_IS_BOUNDED: bool = {bounded};"));
                out.push(format!("// coloring: {}", coloring.name));
                out.push(coloring.wgsl.to_string());
            }
            _ => out.push(line.to_string()),
        }
    }
    out.join("\n")
}

/// Assemble the WGSL for one (formula, coloring) pair.

const FIELD_TEMPLATE: &str = r#"
// Field-evaluation compute pass (mode B, plan phase 3): no
// classification, no escape - each pixel runs a fixed-count loop
// accumulating a scalar field value (and its analytic gradient, when
// the field has one), then colors the scalar. `max_iter` is the TERM
// COUNT here; the same uniform block as the escape template keeps the
// renderer's params packing shared.

struct EscapeParams {
    center: vec2<f32>,
    julia_c: vec2<f32>,
    span: vec2<f32>,
    rot_cs: vec2<f32>,
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,
    bailout: f32,
    // First row this dispatch covers. The perturbed templates
    // chunk by ITERATION and leave this zero; the direct and field
    // templates have no per-pixel resume state, so they chunk by
    // ROW BAND instead - see EscapeRenderer::direct_rows_per_dispatch.
    tile_y0: u32,
    damping: vec2<f32>,
    shade_flags: u32,
    _pad_shade0: u32,
    _pad_shade1: u32,
    _pad_shade2: u32,
    fparams: array<vec4<f32>, 4>,
    cparams: array<vec4<f32>, 4>,
    // CPU-derived formula data — see the direct template's header.
    fdata: array<vec4<f32>, 64>,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
// The coloring's scalar value, kept for the relief pass to
// finite-difference. Bound to a 1x1 dummy when shading is off, where
// every store but one falls out of bounds and WGSL discards it -- so
// the cost of always writing is a single dead store per pixel, and
// there is no second shader variant to keep in step.
@group(0) @binding(4) var height_tex: texture_storage_2d<r32float, write>;

fn fparam(i: u32) -> f32 {
    return params.fparams[i / 4u][i % 4u];
}

// One vec4 of CPU-derived formula data (FormulaDef::derived_data).
fn fdata4(i: u32) -> vec4<f32> {
    return params.fdata[i];
}

fn cparam(i: u32) -> f32 {
    return params.cparams[i / 4u][i % 4u];
}

// Eight floats of per-pixel loop state (the FTLE tangent matrix plus
// the map's own point needs more than a vec4).
struct FieldState {
    a: vec4<f32>,
    b: vec4<f32>,
}

struct FieldStep {
    state: FieldState,
    value: f32,
    grad: vec2<f32>,
}

// What a field coloring returns: palette position + luminance (the
// hillshade channel; 1.0 elsewhere).
struct FieldShade {
    t: f32,
    lum: f32,
}

//__FIELD__

//__FIELD_COLORING__

@compute @workgroup_size(8, 8, 1)
fn escape_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Row band: this dispatch covers [tile_y0, tile_y0 + dispatched).
    let py = gid.y + params.tile_y0;
    if (gid.x >= params.width || py >= params.height) {
        return;
    }

    // Pixel center -> plane: same mapping as the escape template.
    let uv = (vec2<f32>(f32(gid.x), f32(py)) + vec2<f32>(0.5, 0.5))
        / vec2<f32>(f32(params.width), f32(params.height));
    var d = (uv - vec2<f32>(0.5, 0.5)) * params.span;
    d.y = -d.y;
    let rot = params.rot_cs;
    let pixel = params.center + vec2<f32>(
        d.x * rot.x - d.y * rot.y,
        d.x * rot.y + d.y * rot.x,
    );

    var state = field_init(pixel);
    var sum = 0.0;
    var grad = vec2<f32>(0.0, 0.0);
    // Term count: max_iter, clamped - mode B sums converge in tens of
    // terms and statistics in thousands; a runaway count is a TDR.
    let terms = min(params.max_iter, 20000u);
    for (var i = 0u; i < terms; i = i + 1u) {
        let step = field_step(i, pixel, state);
        state = step.state;
        sum = sum + step.value;
        grad = grad + step.grad;
    }

    let shade = field_color(sum, grad, terms);
    let t = fract(shade.t);
    // Relief source, as in the escape templates.
    let height = select(shade.t, t, params.shade_flags == 1u);
    let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
    let rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2)) * clamp(shade.lum, 0.0, 4.0);

    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(py)), vec4<f32>(rgb, 1.0));
    textureStore(height_tex, vec2<i32>(i32(gid.x), i32(py)), vec4<f32>(height, 0.0, 0.0, 0.0));
}
"#;

/// Assemble a mode-B field shader: splice one field def and one field
/// coloring into [`FIELD_TEMPLATE`]. Same marker discipline as
/// [`assemble`].
pub fn assemble_field(field: &FieldDef, coloring: &FieldColoringDef) -> String {
    let mut out = Vec::new();
    for line in FIELD_TEMPLATE.lines() {
        match line.trim() {
            "//__FIELD__" => out.push(field.wgsl.trim().to_string()),
            "//__FIELD_COLORING__" => out.push(coloring.wgsl.trim().to_string()),
            _ => out.push(line.to_string()),
        }
    }
    out.join("\n")
}

/// Assemble the direct template, with interior detection enabled --
/// the production entry point.
pub fn assemble(formula: &FormulaDef, coloring: &ColoringDef, damped: bool) -> String {
    assemble_with(formula, coloring, damped, true)
}

/// As [`assemble`], with interior detection switchable. Disabling it
/// exists for the agreement test (which asserts the two produce the
/// SAME image); production always enables it.
pub fn assemble_with(
    formula: &FormulaDef,
    coloring: &ColoringDef,
    damped: bool,
    interior_detect: bool,
) -> String {
    let needs_accum = coloring.has_feature(ColoringFeature::NeedsOrbitAccum);
    let colors_interior = coloring.has_feature(ColoringFeature::ColorsInterior);
    let bounded = coloring.has_feature(ColoringFeature::Bounded);
    let non_escaping = formula.has_feature(FormulaFeature::NonEscaping);
    let needs_prev = formula.has_feature(FormulaFeature::NeedsPrevZ);
    let needs_index = formula.has_feature(FormulaFeature::NeedsIndex);
    let mutates_c = formula.has_feature(FormulaFeature::MutatesC);
    let convergent = formula.has_feature(FormulaFeature::Convergent);
    let needs_period = coloring.has_feature(ColoringFeature::NeedsPeriod);
    // Interior detection may only stop an orbit where stopping is
    // INVISIBLE. A coloring that draws the interior reads the final z
    // (and its accumulator, and its derivative) for exactly those
    // pixels, so an early stop would change their colour; a period
    // coloring already terminates on its own cycle test and wants the
    // period it measured. Where none of that holds, a non-escaping
    // pixel renders the background no matter which iteration it
    // stopped on.
    let interior_ok = interior_detect
        && !colors_interior
        && !needs_accum
        && !needs_period;
    let needs_derivative = coloring.has_feature(ColoringFeature::NeedsDerivative)
        && !formula.wgsl_derivative.is_empty();
    let param_seed = if formula.wgsl_param_seed.is_empty() {
        "vec2<f32>(0.0, 0.0)"
    } else {
        formula.wgsl_param_seed
    };
    // See the seed comment in the template: z0 = pixel means z0 = c,
    // hence dz0/dc = 1. Any other seed is a constant, hence 0.
    let dz0_param = if formula.wgsl_param_seed == "pixel" {
        "vec2<f32>(1.0, 0.0)"
    } else {
        "vec2<f32>(0.0, 0.0)"
    };

    let mut out = Vec::new();
    for line in TEMPLATE.lines() {
        match line.trim() {
            "//__FORMULA__" => {
                out.push(format!("// formula: {}", formula.name));
                out.push(formula.wgsl.to_string());
                if needs_derivative {
                    out.push(formula.wgsl_derivative.to_string());
                }
            }
            "//__COLORING__" => {
                out.push(format!(
                    "const COLORING_COLORS_INTERIOR: bool = {colors_interior};"
                ));
                out.push(format!("const HAS_DERIVATIVE: bool = {needs_derivative};"));
                out.push(format!("const COLORING_IS_BOUNDED: bool = {bounded};"));
                out.push(format!("// coloring: {}", coloring.name));
                out.push(coloring.wgsl.to_string());
            }
            "//__COLORING_ACCUM__" => {
                if needs_accum {
                    out.push(coloring.wgsl_accum.to_string());
                }
            }
            "//__ACCUM_DECL__" => {
                if needs_accum {
                    out.push(format!(
                        "    var accum_state: vec2<f32> = {};",
                        coloring.accum_init
                    ));
                } else {
                    // `var`, not `let`: the perturbed templates' chunk
                    // resume assigns it even when no accumulator runs.
                    out.push("    var accum_state = vec2<f32>(0.0, 0.0);".to_string());
                }
            }
            "//__ACCUM_UPDATE__" => {
                if needs_accum {
                    out.push("        accum_state = coloring_accum(z, z_before, c, accum_state);".to_string());
                }
            }
            "//__CONVERGE_TEST__" => {
                if convergent {
                    // Terminates on a settled orbit (a root, Magnet's
                    // fixed point at 1, an attracting cycle of period
                    // 1). Sets `escaped` too so escape-count/smooth
                    // shade convergence speed; `converged`
                    // distinguishes basins for root colorings.
                    out.push("        let conv_dz = z - z_before;".to_string());
                    out.push("        if (dot(conv_dz, conv_dz) < 1e-12) {".to_string());
                    out.push("            converged = true;".to_string());
                    out.push("            escaped = true;".to_string());
                    out.push("            n = i + 1u;".to_string());
                    out.push("            break;".to_string());
                    out.push("        }".to_string());
                }
            }
            "//__PERIOD_DECL__" => {
                if needs_period {
                    out.push("    var chk_z = z;".to_string());
                    out.push("    var chk_i = 0u;".to_string());
                }
            }
            "//__PERIOD_TEST__" => {
                if needs_period {
                    // Brent-style: compare against a checkpoint that
                    // advances at powers of two; a match means the
                    // orbit revisited itself — cycle length is the
                    // distance from the checkpoint.
                    out.push("        let pd = z - chk_z;".to_string());
                    out.push("        if (dot(pd, pd) < 1e-12) {".to_string());
                    out.push("            period = max(i - chk_i, 1u);".to_string());
                    out.push("            n = i + 1u;".to_string());
                    out.push("            break;".to_string());
                    out.push("        }".to_string());
                    out.push("        if (((i + 1u) & i) == 0u) {".to_string());
                    out.push("            chk_z = z;".to_string());
                    out.push("            chk_i = i + 1u;".to_string());
                    out.push("        }".to_string());
                }
            }
            "//__INTERIOR_DECL__" => {
                if interior_ok {
                    out.push("    var isnap = z;".to_string());
                }
            }
            "//__INTERIOR_TEST__" => {
                if interior_ok {
                    // Exact-repeat interior detection (Brent epochs).
                    //
                    // The direct path's whole iteration state IS z, and
                    // the arithmetic is deterministic, so a BIT-EXACT
                    // repeat proves the f32 orbit is periodic from here
                    // on: it can never escape, and every later iterate
                    // is a value already seen. Stopping is therefore
                    // not an approximation of running to max_iter -- it
                    // is the same render, which is why this needs no
                    // tolerance, no confirmation count, and no user
                    // toggle. (A tolerance-based check would have to
                    // defend against slow escapes that merely LOOK
                    // periodic; an exact one cannot see them.)
                    //
                    // Compared through bitcast, not float ==, for two
                    // reasons: +0.0 == -0.0 is true while the two can
                    // continue differently under maps that divide or
                    // take logs, and integer comparison is immune to
                    // Metal's fast-math (CLAUDE.md).
                    //
                    // Only spliced when the coloring cannot draw the
                    // interior, so a stopped pixel renders exactly what
                    // an exhausted one would: the background.
                    out.push(
                        "        if (bitcast<u32>(z.x) == bitcast<u32>(isnap.x)".to_string(),
                    );
                    out.push(
                        "            && bitcast<u32>(z.y) == bitcast<u32>(isnap.y)) {".to_string(),
                    );
                    out.push("            break;".to_string());
                    out.push("        }".to_string());
                    // Brent: advance the snapshot at powers of two, so
                    // any cycle is caught within one period of entering
                    // it, with a single stored value.
                    out.push("        if (((i + 1u) & i) == 0u) {".to_string());
                    out.push("            isnap = z;".to_string());
                    out.push("        }".to_string());
                }
            }
            "//__ESCAPE_TEST__" => {
                if !non_escaping {
                    out.push(escape_test(formula.escape_metric));
                }
            }
            "//__C_DECL__" => {
                // MutatesC formulas write c back each step (Spider).
                let kw = if mutates_c { "var" } else { "let" };
                out.push(format!(
                    "    {kw} c = select(pixel, params.julia_c, is_julia);"
                ));
            }
            "//__PREV_DECL__" => {
                if needs_prev {
                    // Default: history starts at 0 (Phoenix
                    // convention). Manowar overrides with "z" —
                    // evaluated here, after z is seeded.
                    let init = if formula.wgsl_prev_init.is_empty() {
                        "vec2<f32>(0.0, 0.0)"
                    } else {
                        formula.wgsl_prev_init
                    };
                    out.push(format!("    var z_prev = {init};"));
                }
            }
            "//__STEP__" => {
                // Damped (Mann) wrap: z <- z + alpha*(f(z) - z), with
                // COMPLEX alpha. Compiled in only when alpha != 1, so
                // undamped pipelines stay byte-identical (a runtime
                // mix() at alpha = 1 is not bit-exact).
                let c_arg = if mutates_c { "&c" } else { "c" };
                // `i` is the loop counter, in scope here. A formula
                // whose RULE changes per step (Origami's fold line)
                // takes it; everything else keeps the two-argument
                // signature byte-for-byte.
                let call = match (needs_prev, needs_index) {
                    (true, true) => format!("formula_step(z, {c_arg}, z_prev, i)"),
                    (true, false) => format!("formula_step(z, {c_arg}, z_prev)"),
                    (false, true) => format!("formula_step(z, {c_arg}, i)"),
                    (false, false) => format!("formula_step(z, {c_arg})"),
                };
                if convergent || needs_accum {
                    // The pre-step iterate: the convergence register,
                    // and the accumulator's z_prev argument. Kept
                    // independently of the formula's own history.
                    out.push("        let z_before = z;".to_string());
                }
                if needs_derivative {
                    // Chain rule at the PRE-step iterate.
                    out.push("        dz = formula_derivative(z, c, dz, is_julia);".to_string());
                }
                if damped {
                    out.push(format!("        let z_raw = {call};"));
                    if needs_prev {
                        out.push("        z_prev = z;".to_string());
                    }
                    out.push("        z = z + esc_cmul(params.damping, z_raw - z);".to_string());
                } else if needs_prev {
                    out.push(format!("        let z_next = {call};"));
                    out.push("        z_prev = z;".to_string());
                    out.push("        z = z_next;".to_string());
                } else {
                    out.push(format!("        z = {call};"));
                }
            }
            _ => out.push(
                line.replace("PARAM_PLANE_SEED", param_seed)
                    .replace("DZ0_PARAM", dz0_param),
            ),
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escape::{COLORINGS, FORMULAS};
    use egui_wgpu::wgpu::naga;

    #[test]
    fn every_field_coloring_combination_validates() {
        // The whole mode-B matrix through naga, same discipline as
        // the escape matrix below (plus the fast-math lints).
        use crate::escape::fields;
        use crate::variations::shader_lint;
        for f in fields::FIELDS {
            for c in fields::FIELD_COLORINGS {
                let src = assemble_field(f, c);
                assert!(!src.contains("//__"), "{}x{} left a marker", f.name, c.name);
                let module = naga::front::wgsl::parse_str(&src)
                    .unwrap_or_else(|e| panic!("{}x{} parse: {e}", f.name, c.name));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|e| panic!("{}x{} validation: {e:?}", f.name, c.name));
                assert!(
                    shader_lint::self_operations(&src).is_empty(),
                    "{}x{} fast-math self-op",
                    f.name,
                    c.name
                );
                assert!(
                    shader_lint::subnormal_literals(&src).is_empty(),
                    "{}x{} subnormal literal",
                    f.name,
                    c.name
                );
            }
        }
    }

    #[test]
    fn assembly_splices_both_bodies_and_leaves_no_markers() {
        for f in FORMULAS {
            for c in COLORINGS {
                let src = assemble(f, c, false);
                assert!(src.contains("fn formula_step"), "{}x{}", f.name, c.name);
                assert!(src.contains("fn coloring_map"), "{}x{}", f.name, c.name);
                assert!(!src.contains("//__"), "{}x{} left a marker", f.name, c.name);
                assert!(!src.contains("PARAM_PLANE_SEED"), "{}x{}", f.name, c.name);
            }
        }
    }

    /// `PerturbTier::is_convergent` must agree with the registry.
    ///
    /// `assemble_perturbed` has no FormulaDef in scope, so the feature
    /// is restated on the tier -- and a restatement can drift. This
    /// walks every formula, asks the renderer which tier it would use,
    /// and checks the two answers match. A Convergent formula that
    /// gained a tier without gaining the settle test would render
    /// every converging pixel at max_iter.
    #[test]
    fn tier_convergence_matches_the_registry() {
        for f in crate::escape::FORMULAS {
            let declared = f.has_feature(crate::escape::FormulaFeature::Convergent);
            for julia in [false, true] {
                let mut esc = crate::config::escape::EscapeConfig::default();
                esc.formula = f.name.to_string();
                esc.julia = julia;
                let Some(tier) = crate::escape::EscapeRenderer::perturb_tier(&esc) else {
                    continue;
                };
                assert_eq!(
                    tier.is_convergent(),
                    declared,
                    "{}: the registry says Convergent={declared} but its tier {tier:?}                      says {} -- the perturbed loop would omit (or add) the settle test",
                    f.name,
                    tier.is_convergent()
                );
            }
        }
    }

    #[test]
    fn every_combination_parses_as_wgsl() {
        // naga front-end parse + validation, no GPU needed — the same
        // guarantee the flame shader dumps get from their tests. This
        // is the compile half of the "formula x coloring compile
        // probe" the plan calls for; the render half needs a device.
        for f in FORMULAS {
            for c in COLORINGS {
              for damped in [false, true] {
                let src = assemble(f, c, damped);
                let module = naga::front::wgsl::parse_str(&src)
                    .unwrap_or_else(|e| panic!("{}x{} failed to parse: {e}", f.name, c.name));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|e| panic!("{}x{} failed validation: {e:?}", f.name, c.name));
              }
            }
        }
    }

    #[test]
    fn non_escaping_formulas_compile_out_the_bailout() {
        let src = assemble(&crate::escape::formulas::KALISET, &crate::escape::colorings::ORBIT_AVERAGE, false);
        assert!(!src.contains("esc_metric"), "Kaliset should carry no escape test");
        assert!(src.contains("coloring_accum"), "orbit average should carry its accumulator");
    }

    #[test]
    fn escaping_formulas_keep_the_bailout_and_skip_the_accum() {
        let src = assemble(&crate::escape::formulas::MANDELBROT, &crate::escape::colorings::SMOOTH, false);
        assert!(src.contains("esc_metric"));
        assert!(!src.contains("fn coloring_accum"));
    }

    #[test]
    fn escape_wgsl_passes_the_fast_math_lints() {
        // Same discipline the variation WGSL is held to (see
        // shader_lint in src/variations/mod.rs): no NaN self-compares,
        // no self-divisions (Metal fast-math folds both), no
        // subnormal f32 literals (FTZ flushes them). Scanning the
        // ASSEMBLED source for every combination covers the template,
        // every formula, every coloring and every accum snippet.
        use crate::variations::shader_lint;
        for f in FORMULAS {
            for c in COLORINGS {
                let src = assemble(f, c, true);
                let self_ops = shader_lint::self_operations(&src);
                assert!(
                    self_ops.is_empty(),
                    "{}x{}: fast-math-unsafe self-operations: {:?}",
                    f.name,
                    c.name,
                    self_ops
                );
                let subnormals = shader_lint::subnormal_literals(&src);
                assert!(
                    subnormals.is_empty(),
                    "{}x{}: subnormal f32 literals: {:?}",
                    f.name,
                    c.name,
                    subnormals
                );
            }
        }
    }

    #[test]
    fn damping_compiles_in_only_when_asked() {
        use crate::escape::{colorings, formulas};
        let plain = assemble(&formulas::MANDELBROT, &colorings::SMOOTH, false);
        assert!(!plain.contains("params.damping"), "undamped shader must not read damping");
        let damped = assemble(&formulas::MANDELBROT, &colorings::SMOOTH, true);
        assert!(damped.contains("esc_cmul(params.damping"));
        // NeedsPrevZ + damped: history records the PRE-damping z.
        let phoenix = assemble(&formulas::PHOENIX, &colorings::SMOOTH, true);
        assert!(phoenix.contains("z_prev = z;"));
    }

    #[test]
    fn mutates_c_compiles_a_var_and_a_pointer_call() {
        use crate::escape::{colorings, formulas};
        let spider = assemble(&formulas::SPIDER, &colorings::SMOOTH, false);
        assert!(spider.contains("var c = select"));
        assert!(spider.contains("formula_step(z, &c)"));
        let plain = assemble(&formulas::MANDELBROT, &colorings::SMOOTH, false);
        assert!(plain.contains("let c = select"));
    }

    /// The Tricorn tier compiles on both rungs, for every integer
    /// power, and actually carries the conjugation (a wrapper that
    /// silently emitted the plain power would render the Multibrot
    /// while claiming to be the Tricorn).
    #[test]
    fn tricorn_tier_compiles_and_conjugates() {
        use crate::escape::colorings;
        for p in 2..=12u32 {
            for floatexp in [false, true] {
                let src = assemble_perturbed(
                    &colorings::SMOOTH, floatexp, PerturbTier::Tricorn(p));
                let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| {
                    panic!("Tricorn p={p} floatexp={floatexp} failed to parse: {e}")
                });
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|e| {
                    panic!("Tricorn p={p} floatexp={floatexp} failed validation: {e}")
                });
                // The conjugation must be present, and the step must
                // consume it rather than the raw operands.
                if floatexp {
                    assert!(src.contains("let zcm = vec2<f32>(z_ref_m.x, -z_ref_m.y);"));
                    if p == 2 {
                        // The p=2 fast path has no zr1; it consumes the
                        // conjugated mantissa and delta directly.
                        assert!(
                            src.contains("cfe2_mul_zdfe(wc, 2.0 * zcm"),
                            "p=2 deep fast path must use the conjugates"
                        );
                    } else {
                        assert!(src.contains("let zr1 = zcm;"));
                    }
                } else {
                    assert!(src.contains("let zc = vec2<f32>(z_ref.x, -z_ref.y);"));
                    assert!(src.contains("let wc = vec2<f32>(w.x, -w.y);"));
                    if p == 2 {
                        assert!(src.contains("zc.x * wc.x"), "p=2 fast path must use conjugates");
                    } else {
                        assert!(src.contains("let zr1 = zc;"));
                    }
                }
            }
        }
    }

    /// The Phoenix tier compiles on the scaled rung and carries the
    /// pieces its recurrence needs: the parameter, the history term,
    /// the history advance, and a rebase to index ONE.
    /// The buffer the renderer allocates must be exactly as wide as
    /// the struct the shader declares.
    ///
    /// These live in different languages and are wired together by
    /// hand, so this measures the ASSEMBLED struct with naga's own
    /// layout rules rather than re-deriving the arithmetic. A tier
    /// that grows its state and forgets `iter_state_bytes` would
    /// otherwise read and write past its slot -- silent corruption of
    /// a neighbouring pixel's history, which no image comparison
    /// attributes to the right cause.
    /// A bounded coloring must CLAMP, not wrap.
    ///
    /// The template wraps every coloring's value with `fract` so that
    /// unbounded ones cycle. For a bounded one that is a bug at
    /// exactly one input: 1.0 wraps to 0.0, so the brightest points
    /// render as the palette's darkest colour. It showed up as a thin
    /// black seam through the highlight of a normal-map render, along
    /// the curve where the surface normal aims straight at the light
    /// — one value out of a continuum, which is why it survived the
    /// numerical check that said the shading matched its reference to
    /// 3.18/255.
    #[test]
    fn a_bounded_coloring_clamps_where_an_unbounded_one_wraps() {
        use crate::escape::colorings;
        for (coloring, expect_bounded) in [
            (&colorings::NORMAL_MAP, true),
            (&colorings::SMOOTH, false),
            (&colorings::ESCAPE_COUNT, false),
        ] {
            for floatexp in [false, true] {
                let src = assemble_perturbed(coloring, floatexp, PerturbTier::Power(2));
                assert!(
                    src.contains(&format!("const COLORING_IS_BOUNDED: bool = {expect_bounded};")),
                    "{} (fe={floatexp}) did not declare bounded={expect_bounded}",
                    coloring.name
                );
            }
            let direct = assemble_with(
                crate::escape::get_formula("mandelbrot"),
                coloring,
                false,
                false,
            );
            assert!(
                direct.contains(&format!("const COLORING_IS_BOUNDED: bool = {expect_bounded};")),
                "{} (direct) did not declare bounded={expect_bounded}",
                coloring.name
            );
            // And the template must actually consult it.
            assert!(
                direct.contains("clamp(raw, 0.0, 1.0), COLORING_IS_BOUNDED"),
                "the palette lookup ignores the bounded flag"
            );
        }
    }

    /// The perturbed rungs never iterate a derivative, and a coloring
    /// built on one has to know.
    ///
    /// `dz` is a constant seed there, so `z/dz` is `z` and the shading
    /// would be a smooth function of arg(z) — convincing relief that
    /// encodes nothing. The flag lets the coloring return flat light
    /// instead, which is visibly unshaded rather than plausibly wrong.
    #[test]
    fn the_perturbed_rungs_declare_no_derivative() {
        use crate::escape::colorings;
        for floatexp in [false, true] {
            let src = assemble_perturbed(&colorings::NORMAL_MAP, floatexp, PerturbTier::Power(2));
            assert!(
                src.contains("const HAS_DERIVATIVE: bool = false;"),
                "perturbed (fe={floatexp}) must declare no derivative"
            );
        }
        // Direct + a formula that HAS one: true.
        let with = assemble_with(
            crate::escape::get_formula("mandelbrot"),
            &colorings::NORMAL_MAP,
            false,
            false,
        );
        assert!(with.contains("const HAS_DERIVATIVE: bool = true;"));
        // Direct + a formula that does NOT: false.
        let without = assemble_with(
            crate::escape::get_formula("kaliset"),
            &colorings::NORMAL_MAP,
            false,
            false,
        );
        assert!(
            without.contains("const HAS_DERIVATIVE: bool = false;"),
            "a formula with no derivative must not claim one"
        );
    }

    #[test]
    fn iter_state_stride_matches_the_shader() {
        use crate::escape::colorings;
        for (tier, floatexp) in [
            (PerturbTier::Power(2), false),
            (PerturbTier::Power(2), true),
            (PerturbTier::Ship(0), true),
            (PerturbTier::Tricorn(2), true),
            (PerturbTier::Phoenix, false),
            (PerturbTier::Phoenix, true),
            (PerturbTier::Lambda, false),
            (PerturbTier::Lambda, true),
            (PerturbTier::Feather(3), false),
            (PerturbTier::Feather(3), true),
            (PerturbTier::McMullen(2, 3), false),
            (PerturbTier::McMullen(2, 3), true),
            (PerturbTier::Magnet(0), false),
            (PerturbTier::Magnet(0), true),
            (PerturbTier::Magnet(1), false),
            (PerturbTier::Magnet(1), true),
        ] {
            let src = assemble_perturbed(&colorings::SMOOTH, floatexp, tier);
            let module = naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("{tier:?} fe={floatexp} parse: {e}"));
            let mut layouter = naga::proc::Layouter::default();
            layouter
                .update(module.to_ctx())
                .unwrap_or_else(|e| panic!("{tier:?} fe={floatexp} layout: {e}"));
            let handle = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some("IterState"))
                .map(|(h, _)| h)
                .expect("IterState must exist");
            let shader_bytes = layouter[handle].size as u64;
            assert_eq!(
                shader_bytes,
                iter_state_bytes(tier, floatexp),
                "{tier:?} fe={floatexp}: the shader's IterState is {shader_bytes} B but the                  renderer allocates {} B per pixel",
                iter_state_bytes(tier, floatexp)
            );
        }
    }

    #[test]
    fn phoenix_tier_compiles_with_history_and_rebases_onto_the_orbit_start() {
        use crate::escape::colorings;
        let src = assemble_perturbed(&colorings::SMOOTH, false, PerturbTier::Phoenix);
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("Phoenix failed to parse: {e}"));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("Phoenix failed validation: {e}"));
        assert!(src.contains("params.fparams[0][0]"), "p must reach the step");
        assert!(src.contains("pp.x * w_prev.x"), "the history term must be applied");
        assert!(src.contains("w_prev = w;"), "the history must advance");
        // The rebase target is index 0, whose state is the pair
        // (Z_0, Z_-1) = (Z_0, 0). Rebasing onto index 1 subtracts
        // Z_1 = c from an O(1) z_full in f32 and quantises the delta
        // to ulp(c) -- ~80 pixels wide at zoom 22, which showed up as
        // displaced rectangular blocks.
        assert!(src.contains("m = 0u;"), "Phoenix rebases onto the orbit start");
        assert!(
            !src.contains("ref_z(1u)"),
            "rebasing against Z_1 is catastrophic cancellation in f32"
        );
        assert!(src.contains("let rebase_delta = z_full - ref_z(0u);"));
        // Both deltas move together, gated on the PAIR's norm.
        assert!(src.contains("w_prev = rebase_prev * perturb.inv_s;"), "history rebases too");
        assert!(
            src.contains("dot(rebase_prev, rebase_prev)"),
            "the gate must weigh the history, or the p*w_prev term explodes"
        );
        // Every OTHER tier keeps the plain rebase, with no history.
        let plain = assemble_perturbed(&colorings::SMOOTH, false, PerturbTier::Power(2));
        assert!(plain.contains("let rebase_delta = z_full - ref_z(0u);"));
        assert!(!plain.contains("w_prev.x"), "one-term tiers must not gain a history term");
    }

    #[test]
    fn prev_init_expression_is_spliced() {
        use crate::escape::{colorings, formulas};
        let manowar = assemble(&formulas::MANOWAR, &colorings::SMOOTH, false);
        assert!(manowar.contains("var z_prev = z;"));
        let phoenix = assemble(&formulas::PHOENIX, &colorings::SMOOTH, false);
        assert!(phoenix.contains("var z_prev = vec2<f32>(0.0, 0.0);"));
    }

    #[test]
    fn escape_metric_is_spliced_per_formula() {
        use crate::escape::{colorings, formulas};
        let exp = assemble(&formulas::EXPONENTIAL, &colorings::SMOOTH, false);
        assert!(exp.contains("var esc_metric = z.x;"));
        let trig = assemble(&formulas::TRIG, &colorings::SMOOTH, false);
        assert!(trig.contains("var esc_metric = abs(z.y);"));
        let mandel = assemble(&formulas::MANDELBROT, &colorings::SMOOTH, false);
        assert!(mandel.contains("var esc_metric = dot(z, z);"));
    }

    #[test]
    fn convergent_formulas_get_the_convergence_test() {
        use crate::escape::{colorings, formulas};
        let newton = assemble(&formulas::NEWTON, &colorings::ROOT_BASIN, false);
        assert!(newton.contains("conv_dz"));
        assert!(newton.contains("converged = true;"));
        let mandel = assemble(&formulas::MANDELBROT, &colorings::SMOOTH, false);
        assert!(!mandel.contains("conv_dz"));
        // Novaretti: convergence test WITHOUT an escape test.
        let nov = assemble(&formulas::NOVARETTI, &colorings::ROOT_BASIN, false);
        assert!(nov.contains("conv_dz"));
        assert!(!nov.contains("esc_metric"));
    }

    #[test]
    fn period_detection_compiles_in_only_for_the_period_coloring() {
        use crate::escape::{colorings, formulas};
        let with = assemble(&formulas::NOVARETTI, &colorings::PERIOD, false);
        assert!(with.contains("chk_z"));
        let without = assemble(&formulas::NOVARETTI, &colorings::SMOOTH, false);
        assert!(!without.contains("chk_z"));
    }

    #[test]
    fn derivative_compiles_only_when_both_sides_agree() {
        use crate::escape::{colorings, formulas};
        let de = assemble(&formulas::MANDELBROT, &colorings::DISTANCE_ESTIMATE, false);
        assert!(de.contains("formula_derivative"));
        // Formula without a derivative: DE compiles, register stays seeded.
        let ship = assemble(&formulas::BURNING_SHIP, &colorings::DISTANCE_ESTIMATE, false);
        assert!(!ship.contains("formula_derivative"));
        // Coloring that doesn't ask: no derivative code at all.
        let plain = assemble(&formulas::MANDELBROT, &colorings::SMOOTH, false);
        assert!(!plain.contains("formula_derivative"));
    }

    #[test]
    fn perturbed_ship_step_validates() {
        use crate::escape::colorings;
        for v in 0..=5u32 {
            for floatexp in [false, true] {
                let src =
                    assemble_perturbed(&colorings::SMOOTH, floatexp, PerturbTier::Ship(v));
                assert!(src.contains("diffabs("), "v{v} fe={floatexp}");
                let module = naga::front::wgsl::parse_str(&src)
                    .unwrap_or_else(|e| panic!("ship v{v} fe={floatexp} parse: {e}"));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|e| panic!("ship v{v} fe={floatexp} validation: {e:?}"));
                use crate::variations::shader_lint;
                assert!(shader_lint::self_operations(&src).is_empty(), "v{v}");
                assert!(shader_lint::subnormal_literals(&src).is_empty(), "v{v}");
            }
        }
    }

    #[test]
    fn perturbed_binomial_step_validates_at_higher_powers() {
        use crate::escape::colorings;
        for floatexp in [false, true] {
            for p in [3u32, 4, 7, 12] {
                let src = assemble_perturbed(&colorings::SMOOTH, floatexp, PerturbTier::Power(p));
                let module = naga::front::wgsl::parse_str(&src)
                    .unwrap_or_else(|e| panic!("p={p} fe={floatexp} parse: {e}"));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|e| panic!("p={p} fe={floatexp} validation: {e:?}"));
            }
        }
        // p = 2 must emit the original hand-written step, byte-stable.
        let src = assemble_perturbed(&colorings::SMOOTH, false, PerturbTier::Power(2));
        assert!(src.contains("var w_new = 2.0 * vec2<f32>("));
    }

    #[test]
    fn perturbed_template_validates_for_every_coloring() {
      for floatexp in [false, true] {
        for c in COLORINGS {
            let src = assemble_perturbed(c, floatexp, PerturbTier::Power(2));
            assert!(!src.contains("//__"), "{} left a marker", c.name);
            let module = naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("perturbed x {} failed to parse: {e}", c.name));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("perturbed x {} failed validation: {e:?}", c.name));
            // Fast-math lints hold here too.
            use crate::variations::shader_lint;
            assert!(shader_lint::self_operations(&src).is_empty(), "{}", c.name);
            assert!(shader_lint::subnormal_literals(&src).is_empty(), "{}", c.name);
        }
      }
    }

    #[test]
    fn defs_fit_the_param_slot_budget() {
        for f in FORMULAS {
            assert!(f.parameters.len() <= PARAM_VEC4S * 4, "{}", f.name);
        }
        for c in COLORINGS {
            assert!(c.parameters.len() <= PARAM_VEC4S * 4, "{}", c.name);
        }
    }
}
