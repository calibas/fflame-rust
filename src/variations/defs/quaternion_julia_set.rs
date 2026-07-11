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
//! `sample_range`-radius ball with one quaternion component (`slice_axis`)
//! pinned to `w_slice`, iterates `q → qⁿ + c`, and **hides** the sample if it
//! escapes (`|q| > bailout` within `escape_iters`). Surviving samples fill the
//! interior of that 3D cross-section of the 4D solid. Sweep `w_slice` for
//! Bourke's series of slices. Use alone, identity affine, weight 1.0.
//!
//! **Choosing the slice axis matters enormously for complex constants.** When
//! `c` has only real + i components (e.g. Bourke's `-1 + 0.2i`), `q² + c`
//! commutes with rotations of the `(j,k)` plane, so the 4D set is a solid of
//! revolution — and the **real-axis slice is always a smooth, featureless
//! revolved ball**, no matter the `c`. All the 2D-Julia fractal detail lives in
//! the `(real, i)` plane, which the real-axis slice is everywhere transverse
//! to. Slice **j or k instead** (`slice_axis` 1 or 2, value 0): that slice
//! *contains* the complex plane, so you get the classic detailed Bourke solid —
//! the 2D Julia set revolved through the remaining vector direction.
//!
//! Convention `q = (x,y,z,w)`: vector part `(x,y,z) = (i,j,k)`, scalar `w` =
//! real. For Bourke's `c = (-1, 0.2, 0, 0)` (scalar-first) set `cw=-1, cx=0.2`.
//! For vector-axis slices the **real axis is plotted as screen Y** so the
//! complex-plane detail faces the default camera. 2D mode is the flat filled
//! Julia for `c = cx + i·cy`.

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
    // Crawl mode's per-thread walk: current surface point (x,y,z), its distance
    // estimate, and a seeded flag. Zero-init is fine (flag 0 = "seed me").
    state_count: 5,
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
        param!("w_slice", "Slice Value", float, 0.0, -2.0, 2.0, "Position of the cutting hyperplane along the Slice Axis (3D only). Sweep it (e.g. -0.6..0.6) to walk Bourke's slice series through the 4D solid."),
        param!("surface", "Surface Shell", float, 0.03, 0.0, 0.5, "0 = fill the SOLID interior (shows the object's form). >0 = plot only the boundary SHELL of this WORLD-SPACE thickness, found with a distance estimator (the exterior points within `surface` of the set), hiding the interior and deep exterior. This reveals Bourke's fractal surface detail at uniform density; ~0.02-0.05 is a crisp skin, larger = thicker/fuzzier."),
        param!("w_color", "Color Scale", unlimited_float, 2.0, 0.0, 8.0, "0 = off. >0 = write a palette index per Color Mode: escape-time bands (mode 0, scale = band frequency) or depth-into-shell (mode 1, scale = contrast exponent). Needs the transform's direct_color > 0. (Crawl + mode 0 colors by radius instead — the escape count isn't retained across rejected steps.)"),
        param!("crawl", "Surface Crawl", bool, false, "OFF = uniform Monte-Carlo sampling (simple, but ~97% of samples miss a thin surface shell). ON = a per-thread walk that stays pinned to the boundary via the distance estimator, so nearly every step plots a useful surface point — a big speedup for SURFACE renders. Needs Surface Shell > 0 (uses 0.02 if left at 0). Leave OFF for solid fill (uniform is already efficient there)."),
        param!("crawl_step", "Crawl Step", float, 0.04, 0.005, 0.3, "Crawl mode only: size of each random step along the surface. Smaller = hugs the surface tighter but traverses slower; larger = faster coverage but looser. ~0.02-0.06 works well; scale it down for finer detail."),
        param!("slice_axis", "Slice Axis", unlimited_int, 3.0, 0.0, 3.0, "Which quaternion component the cutting hyperplane pins to Slice Value: 0 = x/i, 1 = y/j, 2 = z/k, 3 = w/real. CRITICAL for a complex c (cy=cz=0, e.g. Bourke's -1+0.2i): the real-axis slice (3) is ALWAYS a smooth featureless ball of revolution; slice j or k (1 or 2) at value 0 instead — that slice contains the complex plane, giving the classic detailed Bourke solid. For vector-axis slices the real axis is plotted as screen Y so the Julia detail faces the camera."),
        param!("color_mode", "Color Mode", unlimited_int, 0.0, 0.0, 1.0, "What Color Scale writes to the palette index. 0 = escape-time bands: fract(escape_fraction * scale) — wraps, so the palette needs black at position 0 to keep the outer shell dark. 1 = depth-into-shell: (1 - DE/surface)^scale — monotone, true surface at the palette's TOP end, shell edge at 0, no wrapping; any dark-at-0 palette just works, and Color Scale acts as a contrast exponent (>1 concentrates brightness at the surface). Shell mode only (solid interiors keep the base color)."),
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

// Uniform point inside a ball of radius `range` (a ball, not a cube: no clip).
fn qjset_sample_ball(rng: ptr<function, RngState>, range: f32) -> vec3<f32> {
    let costheta = 2.0 * rng_nextf(rng) - 1.0;
    let phi = 6.28318530718 * rng_nextf(rng);
    let rr = range * pow(rng_nextf(rng), 0.33333333);
    let sintheta = sqrt(max(0.0, 1.0 - costheta * costheta));
    return vec3<f32>(rr * sintheta * cos(phi), rr * sintheta * sin(phi), rr * costheta);
}

// Uniform direction on the unit sphere (for the crawl step).
fn qjset_rand_dir(rng: ptr<function, RngState>) -> vec3<f32> {
    let costheta = 2.0 * rng_nextf(rng) - 1.0;
    let phi = 6.28318530718 * rng_nextf(rng);
    let sintheta = sqrt(max(0.0, 1.0 - costheta * costheta));
    return vec3<f32>(sintheta * cos(phi), sintheta * sin(phi), costheta);
}

// Assemble the 4D quaternion from the plotted 3D point, pinning `axis`
// (0=x/i, 1=y/j, 2=z/k, 3=w/real) to the slice value. For vector-axis slices
// the plotted Y carries the REAL component, so the complex-plane (real, i)
// detail faces the default camera; the two free vector components take
// plotted X and Z in slot order.
fn qjset_q4(pos: vec3<f32>, sval: f32, axis: u32) -> vec4<f32> {
    switch (axis) {
        case 0u: { return vec4<f32>(sval, pos.x, pos.z, pos.y); }
        case 1u: { return vec4<f32>(pos.x, sval, pos.z, pos.y); }
        case 2u: { return vec4<f32>(pos.x, pos.z, sval, pos.y); }
        default: { return vec4<f32>(pos, sval); }
    }
}

// Escape-time membership + distance estimate for the plotted point on the
// `axis` slice at value `sval`. Returns (de, escaped01, esc_frac): de is the
// exterior distance estimate DE = 0.5*|q|*ln|q|/|q'|, or the sentinel 1e3 for
// interior points (so a walk treats the interior as "very far" and flees it
// toward the surface).
fn qjset_eval(pos: vec3<f32>, sval: f32, axis: u32, c: vec4<f32>, n: f32, is_sq: bool, bail2: f32, maxit: i32) -> vec3<f32> {
    var q = qjset_q4(pos, sval, axis);
    var dr = 1.0;
    var esc_i = maxit;
    var escaped = false;
    for (var i = 0; i < maxit; i = i + 1) {
        let ql = length(q);
        dr = n * pow(ql, n - 1.0) * dr;      // Julia: dr *= n*|q|^(n-1)
        if (is_sq) { q = qjset_qmul(q, q) + c; } else { q = qjset_qpow(q, n) + c; }
        if (dot(q, q) > bail2) { escaped = true; esc_i = i; break; }
    }
    let frac = f32(esc_i) / f32(maxit);
    if (!escaped) { return vec3<f32>(1e3, 0.0, frac); }
    let ql = length(q);
    return vec3<f32>(0.5 * ql * log(ql) / max(dr, 1e-9), 1.0, frac);
}

fn variation_quaternion_julia_set(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>, hide: ptr<function, bool>) -> vec3<f32> {
    let c = vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );
    let n = get_param(xform_id, variation_id, 4u);
    let is_sq = abs(n - 2.0) < 0.5;
    let maxit = i32(get_param(xform_id, variation_id, 5u) + 0.5);
    let bail2 = pow(get_param(xform_id, variation_id, 6u), 2.0);
    let range = get_param(xform_id, variation_id, 7u);
    let wsl = get_param(xform_id, variation_id, 8u);
    let surface = get_param(xform_id, variation_id, 9u);
    let wcol = get_param(xform_id, variation_id, 10u);
    let crawl = get_param(xform_id, variation_id, 11u) > 0.5;
    let axis = min(u32(get_param(xform_id, variation_id, 13u) + 0.5), 3u);
    let cmode = u32(get_param(xform_id, variation_id, 14u) + 0.5);

    if (!crawl) {
        // UNIFORM sampling: a fresh independent sample each call. Solid
        // (surface=0) keeps the interior; shell (surface>0) keeps the exterior
        // points within `surface` of the set. Most samples miss a thin shell.
        let pos = qjset_sample_ball(rng, range);
        let ev = qjset_eval(pos, wsl, axis, c, n, is_sq, bail2, maxit);
        let de = ev.x;
        let escaped = ev.y > 0.5;
        if (surface <= 0.0) {
            if (escaped) { *hide = true; }
        } else {
            if (!escaped || de > surface) { *hide = true; }
        }
        if (wcol > 1e-6 && escaped) {
            if (cmode == 1u && surface > 0.0) {
                // Depth-into-shell: monotone, surface = 1, shell edge = 0;
                // wcol is a contrast exponent (no fract wrapping).
                *vc = pow(clamp(1.0 - de / surface, 0.0, 1.0), wcol);
            } else {
                *vc = fract(ev.z * wcol);
            }
        }
        return pos;
    }

    // CRAWL sampling: a per-thread walk pinned to the surface. State slots
    // 0..2 = current point, 3 = its distance estimate, 4 = seeded flag. Because
    // each thread runs only a few hundred iterations then a fresh thread starts,
    // the walk is re-seeded millions of times over — natural coverage.
    let surf = select(surface, 0.02, surface <= 0.0);   // crawl needs a target thickness
    let step = get_param(xform_id, variation_id, 12u);
    var pc = vec3<f32>(
        get_state(xform_id, variation_id, 0u),
        get_state(xform_id, variation_id, 1u),
        get_state(xform_id, variation_id, 2u)
    );
    var dec = get_state(xform_id, variation_id, 3u);

    if (get_state(xform_id, variation_id, 4u) < 0.5) {
        // First step in this thread: drop a random seed and measure its DE.
        pc = qjset_sample_ball(rng, range);
        dec = qjset_eval(pc, wsl, axis, c, n, is_sq, bail2, maxit).x;
        set_state(xform_id, variation_id, 4u, 1.0);
    } else if (rng_nextf(rng) < 0.01) {
        // Occasional random jump so one thread can reach several lobes.
        pc = qjset_sample_ball(rng, range);
        dec = qjset_eval(pc, wsl, axis, c, n, is_sq, bail2, maxit).x;
    } else {
        // Propose a small step; accept it if it descends toward the surface
        // (DE decreases) or stays within the shell. Interior points carry the
        // 1e3 sentinel, so the walk is pushed out toward the boundary.
        let pn = pc + step * qjset_rand_dir(rng);
        if (dot(pn, pn) <= range * range) {
            let den = qjset_eval(pn, wsl, axis, c, n, is_sq, bail2, maxit).x;
            if (den <= dec || den < surf) { pc = pn; dec = den; }
        }
    }

    set_state(xform_id, variation_id, 0u, pc.x);
    set_state(xform_id, variation_id, 1u, pc.y);
    set_state(xform_id, variation_id, 2u, pc.z);
    set_state(xform_id, variation_id, 3u, dec);

    // Plot only once the walk is actually pinned to the surface (still
    // descending / interior → suppressed as burn-in). Color by radius (the
    // escape count isn't retained across a rejected step).
    if (dec >= surf) { *hide = true; }
    if (wcol > 1e-6) {
        if (cmode == 1u) {
            *vc = pow(clamp(1.0 - dec / surf, 0.0, 1.0), wcol);
        } else {
            *vc = fract(length(pc) * wcol);
        }
    }
    return pc;
}
"#;
