//! `quaternion_julia_set` — the **filled** quaternion Julia set by escape-time
//! membership (Paul Bourke's method), rendered as a Monte-Carlo point cloud.
//!
//! Unlike [`quaternion_julia`]'s inverse mode — which iterates `q → (q−c)^{1/n}`
//! and, for any `c`, **collapses onto the 2D complex plane through `1` and `c`**
//! (the inverse map inherits the axis of `q−c`, so the transverse `j,k`
//! components decay to zero and you only ever get the 2D Julia embedded in 4D) —
//! this tests membership directly and so reaches the genuine 4D set.
//!
//! Each call ignores the incoming point, draws a fresh uniform sample in a
//! `sample_range`-radius ball with the **real** part pinned to `w_slice`, iterates
//! `q → qⁿ + c`, and **hides** the sample if it escapes (`|q| > bailout` within
//! `escape_iters`). Surviving samples fill the interior of the 3D cross-section
//! `{ (i,j,k) : (i,j,k,w_slice) ∈ filled-Julia }`. Sweep `w_slice` for Bourke's
//! series of slices. Use alone, identity affine, weight 1.0.
//!
//! Convention `q = (x,y,z,w)`: vector part `(x,y,z) = (i,j,k)`, scalar `w` =
//! real. For Bourke's `c = (-1, 0.2, 0, 0)` (scalar-first) set `cw=-1, cx=0.2`.
//! 2D mode is the flat filled Julia for `c = cx + i·cy`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUATERNION_JULIA_SET: VariationDef = VariationDef {
    name: "quaternion_julia_set",
    aliases: &["qjuliaset", "qjulia_solid"],
    display_name: "Quaternion Julia (Solid)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // NeedsRng: Monte-Carlo domain sampling. CanHide: escaped samples are
    // suppressed (only the interior plots). WritesColor: optional shading.
    features: &[Feature::NeedsRng, Feature::CanHide, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cx", "Constant X", float, 0.2, -2.0, 2.0, "Vector-i component of the Julia constant c."),
        param!("cy", "Constant Y", float, 0.0, -2.0, 2.0, "Vector-j component of c. 3D only."),
        param!("cz", "Constant Z", float, 0.0, -2.0, 2.0, "Vector-k component of c. 3D only."),
        param!("cw", "Constant W", float, -1.0, -2.0, 2.0, "Scalar (real) component of c. Bourke writes c scalar-first, so his c=(-1,0.2,0,0) is cw=-1, cx=0.2. 2D mode ignores this (uses cx+i*cy)."),
        param!("power", "Power (N)", int, 2.0, 2.0, 8.0, "The Julia power n in q^n + c. 2 = the classic quadratic set; 3+ = higher-order. n=2 uses the exact Hamilton product."),
        param!("escape_iters", "Escape Iterations", int, 16.0, 2.0, 64.0, "Membership-test depth: how many times to apply q -> q^n + c before declaring a sample interior. Higher = sharper boundary (fewer false-interior points) but thinner surviving interior and slower."),
        param!("bailout", "Bailout", float, 2.0, 1.0, 8.0, "Escape radius: a sample escapes (and is hidden) once |q| exceeds this. ~2 is standard for |c|~1."),
        param!("sample_range", "Sample Range", float, 2.0, 0.2, 4.0, "Radius of the BALL the samples are drawn from (a ball, not a cube — so no box-face clipping). The whole set lies within |q| <= bailout, so leaving this at the bailout radius (2) guarantees the entire cross-section is covered. Shrink it ONLY to trade coverage for density on a set you know is smaller — too small clips (now as a sphere, not a cube)."),
        param!("w_slice", "Slice (real)", float, 0.0, -2.0, 2.0, "The fixed real (w) coordinate of the 3D cross-section (3D only). 0 is the widest slice for a complex c; sweep it (e.g. -0.6..0.6) to walk Bourke's slice series through the 4D solid."),
        param!("surface", "Surface Shell", float, 0.03, 0.0, 0.5, "0 = fill the SOLID interior (shows the object's form). >0 = plot only the boundary SHELL of this WORLD-SPACE thickness, found with a distance estimator (the exterior points within `surface` of the set), hiding the interior and deep exterior. This reveals Bourke's fractal surface detail at uniform density; ~0.02-0.05 is a crisp skin, larger = thicker/fuzzier."),
        param!("w_color", "Color by Escape", float, 2.0, 0.0, 8.0, "0 = off. >0 = write a palette index from the escape speed: fract((escape_iter/limit) * scale), giving the classic escape-time bands across the surface. Needs the transform's direct_color > 0."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_julia_set(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>, hide: ptr<function, bool>) -> vec2<f32> {
    // 2D: uniformly sample the box, test the complex map z -> z^n + c, hide escapees.
    let range = get_param(xform_id, variation_id, 7u);
    // Uniform sample inside a disk of radius `range` (a square would clip).
    let rr = range * sqrt(rng_nextf(rng));
    let phi = 6.28318530718 * rng_nextf(rng);
    let sx = rr * cos(phi);
    let sy = rr * sin(phi);
    let cx = get_param(xform_id, variation_id, 0u);   // real part of c (2D reading)
    let cy = get_param(xform_id, variation_id, 1u);   // imag part of c (2D reading)
    let n = get_param(xform_id, variation_id, 4u);
    let bail2 = pow(get_param(xform_id, variation_id, 6u), 2.0);
    let maxit = i32(get_param(xform_id, variation_id, 5u) + 0.5);
    var zx = sx;
    var zy = sy;
    var dr = 1.0;
    var escaped = false;
    var esc_i = maxit;
    for (var i = 0; i < maxit; i = i + 1) {
        // z^n via polar form, + c; track derivative magnitude for the DE.
        let zl = sqrt(zx * zx + zy * zy);
        dr = n * pow(zl, n - 1.0) * dr;
        let r = pow(zx * zx + zy * zy, n * 0.5);
        let th = atan2(zy, zx) * n;
        zx = r * cos(th) + cx;
        zy = r * sin(th) + cy;
        if (zx * zx + zy * zy > bail2) { escaped = true; esc_i = i; break; }
    }
    let surface = get_param(xform_id, variation_id, 9u);
    var de = 1e9;
    if (escaped) {
        let zl = sqrt(zx * zx + zy * zy);
        de = 0.5 * zl * log(zl) / max(dr, 1e-9);
    }
    if (surface <= 0.0) {
        if (escaped) { *hide = true; }
    } else {
        if (!escaped || de > surface) { *hide = true; }
    }
    let wcol = get_param(xform_id, variation_id, 10u);
    if (wcol > 1e-6 && escaped) { *vc = fract((f32(esc_i) / f32(maxit)) * wcol); }
    return vec2<f32>(sx, sy);
}
"#;

const WGSL_3D: &str = r#"
fn qjset_qmul(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z
    );
}

// q^n via polar form (for n != 2). q = (x,y,z,w), scalar w, vector (x,y,z).
fn qjset_qpow(q: vec4<f32>, n: f32) -> vec4<f32> {
    let mag = length(q) + 1e-12;
    let rad = pow(mag, n);
    let ang = acos(clamp(q.w / mag, -1.0, 1.0)) * n;
    let vlen = length(q.xyz);
    let nhat = select(vec3<f32>(1.0, 0.0, 0.0), q.xyz / vlen, vlen > 1e-9);
    return vec4<f32>(rad * sin(ang) * nhat, rad * cos(ang));
}

fn variation_quaternion_julia_set(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>, hide: ptr<function, bool>) -> vec3<f32> {
    // Fresh uniform sample of the (i,j,k) box; the real part is pinned to the slice.
    let range = get_param(xform_id, variation_id, 7u);
    // Uniform sample INSIDE a ball of radius `range` — a cube would clip the set
    // at hard box faces. The whole set lies within |q| <= bailout, so
    // range = bailout guarantees full coverage with no clipping.
    let costheta = 2.0 * rng_nextf(rng) - 1.0;
    let phi = 6.28318530718 * rng_nextf(rng);
    let rr = range * pow(rng_nextf(rng), 0.33333333);
    let sintheta = sqrt(max(0.0, 1.0 - costheta * costheta));
    let sx = rr * sintheta * cos(phi);
    let sy = rr * sintheta * sin(phi);
    let sz = rr * costheta;
    let wsl = get_param(xform_id, variation_id, 8u);
    let c = vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );
    let n = get_param(xform_id, variation_id, 4u);
    let is_sq = abs(n - 2.0) < 0.5;
    let bail2 = pow(get_param(xform_id, variation_id, 6u), 2.0);
    let maxit = i32(get_param(xform_id, variation_id, 5u) + 0.5);

    // Iterate q -> q^n + c, tracking the scalar derivative magnitude `dr` for
    // the distance estimator (Julia: c constant, so dr *= n*|q|^(n-1)).
    var q = vec4<f32>(sx, sy, sz, wsl);
    var dr = 1.0;
    var escaped = false;
    var esc_i = maxit;
    for (var i = 0; i < maxit; i = i + 1) {
        let ql = length(q);
        dr = n * pow(ql, n - 1.0) * dr;
        if (is_sq) { q = qjset_qmul(q, q) + c; } else { q = qjset_qpow(q, n) + c; }
        if (dot(q, q) > bail2) { escaped = true; esc_i = i; break; }
    }

    // Solid (surface=0): keep the interior, hide escapees. Shell (surface>0):
    // keep exterior points whose distance estimate DE = 0.5*|q|*ln|q|/|q'| is
    // within `surface` of the set — a uniform-thickness fractal skin.
    let surface = get_param(xform_id, variation_id, 9u);
    var de = 1e9;
    if (escaped) {
        let ql = length(q);
        de = 0.5 * ql * log(ql) / max(dr, 1e-9);
    }
    if (surface <= 0.0) {
        if (escaped) { *hide = true; }
    } else {
        if (!escaped || de > surface) { *hide = true; }
    }

    // Escape-time banding across the surface when enabled. Only escaped points
    // carry a meaningful escape count; solid-interior points keep the
    // transform's base color rather than collapsing to one band.
    let wcol = get_param(xform_id, variation_id, 10u);
    if (wcol > 1e-6 && escaped) { *vc = fract((f32(esc_i) / f32(maxit)) * wcol); }

    // Plot the sampled (i,j,k). The map ignores the incoming point, so the affine
    // and feed-forward are irrelevant — each iteration is an independent sample.
    return vec3<f32>(sx, sy, sz);
}
"#;
