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
    _pad0: f32,
    damping: vec2<f32>,    // Mann alpha (re, im); read only when compiled damped
    fparams: array<vec4<f32>, 4>,  // formula params, slot-ordered
    cparams: array<vec4<f32>, 4>,  // coloring params, slot-ordered
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;

fn fparam(i: u32) -> f32 {
    return params.fparams[i / 4u][i % 4u];
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
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }

    // Pixel center -> complex plane: offset from view center, y flipped
    // (texture y grows down, Im grows up), then view rotation.
    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5, 0.5))
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
    // Derivative-orbit seed: d z0/dz0 = 1 on the dynamical plane,
    // d z0/dc = 0 on the parameter plane (z0 is c-independent for
    // every formula that supplies a derivative). Unread and
    // dead-code-eliminated unless the coloring uses it.
    var dz = select(vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), is_julia);
    //__PERIOD_DECL__
    for (var i = 0u; i < params.max_iter; i = i + 1u) {
        //__STEP__
        //__ACCUM_UPDATE__
        //__CONVERGE_TEST__
        //__PERIOD_TEST__
        //__ESCAPE_TEST__
    }
    if (!escaped) {
        n = params.max_iter;
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    if (escaped || COLORING_COLORS_INTERIOR) {
        let summary = OrbitSummary(z, n, escaped, converged, period, dz);
        let t = fract(coloring_map(summary, accum_state));
        // textureSampleLevel: explicit LOD, legal in non-uniform
        // control flow (unlike textureSample) -- WASM-safe.
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    }

    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(rgb, 1.0));
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
    _pad0: f32,
    damping: vec2<f32>,
    fparams: array<vec4<f32>, 4>,
    cparams: array<vec4<f32>, 4>,
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
    _pad_c0: u32,
    _pad_c1: u32,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
@group(0) @binding(4) var<storage, read> ref_orbit: array<vec2<f32>>;
@group(0) @binding(5) var<uniform> perturb: PerturbParams;
// Per-pixel iteration state for chunked dispatches (48 bytes/px).
struct IterState {
    w: vec2<f32>,       // scaled: w | floatexp: CFe mantissa
    z: vec2<f32>,       // last full orbit value
    accum: vec2<f32>,   // coloring accumulator
    w_e: i32,           // floatexp exponent (unused by the scaled rung)
    m: u32,             // reference index
    n_done: u32,        // iterations recorded at termination
    status: u32,        // 0 = iterating, 1 = escaped
}
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
    let converged = false;
    let period = 0u;
    let dz = vec2<f32>(1.0, 0.0);
    // The f32 value of c for the accumulator colorings (trap geometry
    // lives at O(1) scale, where f32 c is exact enough).
    let c_f32 = params.center;

    //__ACCUM_DECL__

    // Chunk resume: after the first chunk every register comes from
    // the state buffer; an already-escaped pixel just rewrites its
    // (final) color.
    if (perturb.iter_start > 0u) {
        let st = iter_state[px_index];
        w = st.w;
        z = st.z;
        m = st.m;
        accum_state = st.accum;
        if (st.status == 1u) {
            escaped = true;
            n = st.n_done;
        }
    }

    var i = perturb.iter_start;
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
            let steps_left = perturb.iter_end - i;
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
            let z_ref = ref_orbit[m];
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
        let z_full = ref_orbit[min(m, perturb.orbit_len - 1u)] + delta;
        z = z_full;

        //__ACCUM_UPDATE__

        // Escape test (biomorph is gated off on the perturbed path).
        if (dot(z_full, z_full) > params.bailout) {
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
        let rebase_delta = z_full - ref_orbit[0];
        if (m >= perturb.orbit_len - 1u
            || dot(rebase_delta, rebase_delta) < dot(delta, delta)) {
            w = rebase_delta * perturb.inv_s;
            m = 0u;
        }
    }
    if (perturb.iter_end < params.max_iter) {
        // More chunks follow: persist the registers.
        iter_state[px_index] = IterState(
            w, z, accum_state, 0, m,
            select(0u, n, escaped),
            select(0u, 1u, escaped),
        );
    }
    if (!escaped) {
        n = params.max_iter;
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    if (escaped || COLORING_COLORS_INTERIOR) {
        let summary = OrbitSummary(z, n, escaped, converged, period, dz);
        let t = fract(coloring_map(summary, accum_state));
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    }

    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(rgb, 1.0));
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
    _pad0: f32,
    damping: vec2<f32>,
    fparams: array<vec4<f32>, 4>,
    cparams: array<vec4<f32>, 4>,
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
    _pad_c0: u32,
    _pad_c1: u32,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
@group(0) @binding(4) var<storage, read> ref_orbit: array<vec2<f32>>;
@group(0) @binding(5) var<uniform> perturb: PerturbParams;
// Per-pixel iteration state for chunked dispatches (48 bytes/px).
struct IterState {
    w: vec2<f32>,       // scaled: w | floatexp: CFe mantissa
    z: vec2<f32>,       // last full orbit value
    accum: vec2<f32>,   // coloring accumulator
    w_e: i32,           // floatexp exponent (unused by the scaled rung)
    m: u32,             // reference index
    n_done: u32,        // iterations recorded at termination
    status: u32,        // 0 = iterating, 1 = escaped
}
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

struct OrbitSummary {
    z: vec2<f32>,
    n: u32,
    escaped: bool,
    converged: bool,
    period: u32,
    dz: vec2<f32>,
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
    let d0 = cfe_norm(CFe((d0px + perturb.ref_offset) * perturb.s_m, perturb.s_e));
    let px_index = gid.y * params.width + gid.x;

    let is_julia_perturb = (perturb.flags & 2u) != 0u;
    var w: CFe;
    if (is_julia_perturb) {
        w = d0;
    } else {
        w = CFe(vec2<f32>(0.0, 0.0), CFE_ZERO_E);
    }

    var m = 0u;
    var z = vec2<f32>(0.0, 0.0);
    var escaped = false;
    var n = 0u;
    let converged = false;
    let period = 0u;
    let dz = vec2<f32>(1.0, 0.0);
    let c_f32 = params.center;

    //__ACCUM_DECL__

    if (perturb.iter_start > 0u) {
        let st = iter_state[px_index];
        w = CFe(st.w, st.w_e);
        z = st.z;
        m = st.m;
        accum_state = st.accum;
        if (st.status == 1u) {
            escaped = true;
            n = st.n_done;
        }
    }

    var i = perturb.iter_start;
    while (i < perturb.iter_end && !escaped) {
        // BLA skip (see the scaled template). Here w IS the absolute
        // delta as a CFe, so validity compares and the affine
        // application run in full extended range -- no clamps.
        var did_skip = false;
        if (bla.n_levels > 0u && m > 0u) {
            let aw = length(w.m);
            var pick_a = vec2<f32>(0.0, 0.0);
            var pick_b = vec2<f32>(0.0, 0.0);
            var pick_ae = 0;
            var pick_be = 0;
            var pick_span = 0u;
            let w_zero = aw == 0.0 || w.e == CFE_ZERO_E;
            let m_left = perturb.orbit_len - 1u - m;
            let steps_left = perturb.iter_end - i;
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
                let ta = cfe_mul(CFe(pick_a, pick_ae), w);
                let tb = cfe_mul(CFe(pick_b, pick_be), d0);
                w = cfe_add(ta, tb);
                m = m + pick_span;
                i = i + pick_span;
                did_skip = true;
            }
        }
        if (!did_skip) {
            let z_ref = ref_orbit[m];
            //__DELTA_STEP_FE__
            w = w_new;
            m = m + 1u;
            i = i + 1u;
        }
        let z_before = z;

        let delta = cfe_to_f32(w);
        let z_full = ref_orbit[min(m, perturb.orbit_len - 1u)] + delta;
        z = z_full;

        //__ACCUM_UPDATE__

        if (dot(z_full, z_full) > params.bailout) {
            escaped = true;
            n = i;
            break;
        }

        // Zhuoran rebase against the orbit start (Z_0 = 0 on the
        // parameter plane, the center on the Julia plane).
        let rebase_delta = z_full - ref_orbit[0];
        if (m >= perturb.orbit_len - 1u
            || dot(rebase_delta, rebase_delta) < dot(delta, delta)) {
            w = cfe_from_f32(rebase_delta);
            m = 0u;
        }
    }
    if (perturb.iter_end < params.max_iter) {
        iter_state[px_index] = IterState(
            w.m, z, accum_state, w.e, m,
            select(0u, n, escaped),
            select(0u, 1u, escaped),
        );
    }
    if (!escaped) {
        n = params.max_iter;
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    if (escaped || COLORING_COLORS_INTERIOR) {
        let summary = OrbitSummary(z, n, escaped, converged, period, dz);
        let t = fract(coloring_map(summary, accum_state));
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    }

    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(rgb, 1.0));
}
"#;

/// Which delta algebra the perturbed pipeline compiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerturbTier {
    /// z^p + c, integer p — the clean binomial expansion.
    Power(u32),
    /// Burning Ship family: abs-folds via diffabs case analysis, on
    /// both rungs (the floatexp rung runs extended-range scalar
    /// diffabs). The u32 is the variant enum (0..=5) — each fold
    /// arrangement has its own delta algebra.
    Ship(u32),
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
    out.push_str("        let dx = sfe_norm(SFe(w.m.x, w.e));\n");
    out.push_str("        let dy = sfe_norm(SFe(w.m.y, w.e));\n");
    out.push_str("        let d0x = sfe_norm(SFe(d0.m.x, d0.e));\n");
    out.push_str("        let d0y = sfe_norm(SFe(d0.m.y, d0.e));\n");
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
    out.push_str("        var w_new = cfe_from_sfe(re_s, im_s);");
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
    if p == 2 {
        return "        // w' = 2 Z w + S w^2 + d0 (quadratic term hoisted out in the\n\
                \x20       // deep-linear regime).\n\
                \x20       var w_new = 2.0 * vec2<f32>(\n\
                \x20           z_ref.x * w.x - z_ref.y * w.y,\n\
                \x20           z_ref.x * w.y + z_ref.y * w.x,\n\
                \x20       ) + d0_term;\n\
                \x20       if ((perturb.flags & 1u) == 0u) {\n\
                \x20           w_new = w_new + perturb.s * vec2<f32>(w.x * w.x - w.y * w.y, 2.0 * w.x * w.y);\n\
                \x20       }"
            .to_string();
    }
    let mut out = String::new();
    out.push_str(&format!("        // Binomial delta step for z^{p} + c.\n"));
    // Powers of the reference Z up to p-1.
    out.push_str("        let zr1 = z_ref;\n");
    for k in 2..p {
        out.push_str(&format!(
            "        let zr{k} = vec2<f32>(zr{}.x * z_ref.x - zr{}.y * z_ref.y, zr{}.x * z_ref.y + zr{}.y * z_ref.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    // S-folded powers of w: u1 = w, u(k) = S * u(k-1) * w.
    out.push_str("        let u1 = w;\n");
    for k in 2..=p {
        out.push_str(&format!(
            "        let u{k} = perturb.s * vec2<f32>(u{}.x * w.x - u{}.y * w.y, u{}.x * w.y + u{}.y * w.x);\n",
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

/// The floatexp flavor of the same step.
fn delta_step_floatexp(p: u32) -> String {
    if p == 2 {
        return "        // delta' = 2 Z delta + delta^2 (+ delta_c on the parameter\n\
                \x20       // plane) - all in floatexp.\n\
                \x20       var w_new = cfe_add(cfe_mul_c32(w, 2.0 * z_ref), cfe_sqr(w));\n\
                \x20       if (!is_julia_perturb) {\n\
                \x20           w_new = cfe_add(w_new, d0);\n\
                \x20       }"
            .to_string();
    }
    let mut out = String::new();
    out.push_str(&format!("        // Binomial delta step for z^{p} + c (floatexp).\n"));
    out.push_str("        let zr1 = z_ref;\n");
    for k in 2..p {
        out.push_str(&format!(
            "        let zr{k} = vec2<f32>(zr{}.x * z_ref.x - zr{}.y * z_ref.y, zr{}.x * z_ref.y + zr{}.y * z_ref.x);\n",
            k - 1, k - 1, k - 1, k - 1
        ));
    }
    out.push_str("        let u1 = w;\n");
    for k in 2..=p {
        out.push_str(&format!("        let u{k} = cfe_mul(u{}, w);\n", k - 1));
    }
    out.push_str("        var w_new = CFe(vec2<f32>(0.0, 0.0), CFE_ZERO_E);\n");
    out.push_str("        if (!is_julia_perturb) {\n            w_new = d0;\n        }\n");
    for k in 1..=p {
        let coeff = binomial(p, k);
        if k == p {
            out.push_str(&format!(
                "        w_new = cfe_add(w_new, cfe_mul_c32(u{k}, vec2<f32>({coeff}.0, 0.0)));\n"
            ));
        } else {
            let zp = p - k;
            out.push_str(&format!(
                "        w_new = cfe_add(w_new, cfe_mul_c32(u{k}, {coeff}.0 * zr{zp}));\n"
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
            }),
            "//__DELTA_STEP_FE__" => out.push(match tier {
                PerturbTier::Power(p) => delta_step_floatexp(p.clamp(2, 12)),
                PerturbTier::Ship(v) => delta_step_ship_fe(v.min(5)),
            }),
            "//__COLORING__" => {
                out.push(format!(
                    "const COLORING_COLORS_INTERIOR: bool = {colors_interior};"
                ));
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

/// Assemble the WGSL for one (formula, coloring) pair.
pub fn assemble(formula: &FormulaDef, coloring: &ColoringDef, damped: bool) -> String {
    let needs_accum = coloring.has_feature(ColoringFeature::NeedsOrbitAccum);
    let colors_interior = coloring.has_feature(ColoringFeature::ColorsInterior);
    let non_escaping = formula.has_feature(FormulaFeature::NonEscaping);
    let needs_prev = formula.has_feature(FormulaFeature::NeedsPrevZ);
    let mutates_c = formula.has_feature(FormulaFeature::MutatesC);
    let convergent = formula.has_feature(FormulaFeature::Convergent);
    let needs_period = coloring.has_feature(ColoringFeature::NeedsPeriod);
    let needs_derivative = coloring.has_feature(ColoringFeature::NeedsDerivative)
        && !formula.wgsl_derivative.is_empty();
    let param_seed = if formula.wgsl_param_seed.is_empty() {
        "vec2<f32>(0.0, 0.0)"
    } else {
        formula.wgsl_param_seed
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
                let call = if needs_prev {
                    format!("formula_step(z, {c_arg}, z_prev)")
                } else {
                    format!("formula_step(z, {c_arg})")
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
            _ => out.push(line.replace("PARAM_PLANE_SEED", param_seed)),
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
