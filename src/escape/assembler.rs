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
    // Pixel spacing S as an f32 (normal down to 2^-126; the v1
    // ceiling, far past the UI zoom range) and its reciprocal.
    s: f32,
    inv_s: f32,
    orbit_len: u32,   // usable entries in the reference orbit
    // bit 0: skip the S*w*w term (deep-linear regime, hoisted per the
    // plan). bit 1: Julia mode - c is constant, so the +d0 term drops
    // from the recurrence and seeds the delta instead (delta_0 =
    // pixel - center).
    flags: u32,
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;
@group(0) @binding(4) var<storage, read> ref_orbit: array<vec2<f32>>;
@group(0) @binding(5) var<uniform> perturb: PerturbParams;

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
    );

    // Delta iteration state: w = delta / S, m = reference index.
    // Parameter plane: delta_0 = 0 (both orbits start at z = 0) and
    // the +d0 term generates delta_1 = delta_c. Julia: c is constant
    // (the term drops) and the SEED differs by the pixel offset.
    let is_julia_perturb = (perturb.flags & 2u) != 0u;
    var w = select(vec2<f32>(0.0, 0.0), d0, is_julia_perturb);
    let d0_term = select(d0, vec2<f32>(0.0, 0.0), is_julia_perturb);
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

    for (var i = 0u; i < params.max_iter; i = i + 1u) {
        let z_ref = ref_orbit[m];
        // w' = 2 Z w + S w^2 + d0 (quadratic term hoisted out in the
        // deep-linear regime).
        var w_new = 2.0 * vec2<f32>(
            z_ref.x * w.x - z_ref.y * w.y,
            z_ref.x * w.y + z_ref.y * w.x,
        ) + d0_term;
        if ((perturb.flags & 1u) == 0u) {
            w_new = w_new + perturb.s * vec2<f32>(w.x * w.x - w.y * w.y, 2.0 * w.x * w.y);
        }
        let z_before = z;
        w = w_new;
        m = m + 1u;

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
            n = i + 1u;
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

/// Assemble the perturbed (deep-zoom) Mandelbrot WGSL for one
/// coloring. The formula axis is fixed to Mandelbrot in v1 (the
/// "clean" perturbation tier); the coloring axis is fully preserved -
/// the loop reconstructs the full orbit value every iteration for the
/// rebase test, which is exactly the summary the colorings consume.
pub fn assemble_perturbed(coloring: &ColoringDef) -> String {
    let needs_accum = coloring.has_feature(ColoringFeature::NeedsOrbitAccum);
    let colors_interior = coloring.has_feature(ColoringFeature::ColorsInterior);

    let mut out = Vec::new();
    for line in PERTURBED_TEMPLATE.lines() {
        match line.trim() {
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
                    out.push("    let accum_state = vec2<f32>(0.0, 0.0);".to_string());
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
                    out.push("    let accum_state = vec2<f32>(0.0, 0.0);".to_string());
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
    fn perturbed_template_validates_for_every_coloring() {
        for c in COLORINGS {
            let src = assemble_perturbed(c);
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
