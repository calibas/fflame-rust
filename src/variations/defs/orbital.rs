//! `orbital` — hydrogen electron-orbital isosurface attractor (original).
//!
//! Renders the textbook shapes of hydrogen-like atomic orbitals. The
//! real-form wavefunction is
//!
//! ```text
//! ψ_nlm(r,θ,φ) = R_nl(r) · P_l^|m|(cosθ) · {cos(mφ), 1, sin(|m|φ)}
//! R_nl(ρ)      = e^(−ρ/2) · ρ^l · L_{n−l−1}^{2l+1}(ρ),   ρ = 2r/(n·a)
//! ```
//!
//! evaluated with general recurrences (associated Laguerre for the
//! radial part, associated Legendre for the angular part), so every
//! orbital up to n = 8 works uniformly — 1s, 2p, 3d_z², 4f, all of
//! them. The Bohr radius is auto-fitted (`a = size/(2n²)`) so the
//! orbital's mean radius lands around 0.75·size regardless of n.
//!
//! The familiar lobe pictures are **isosurfaces of |ψ|**, and points
//! are Newton-projected onto one — in log space:
//! `u = ln|ψ|²`, surface `u = target`. Log space matters twice over:
//! ψ's magnitude varies by orders of magnitude across (n, l), and in
//! the far field the log-gradient is nearly constant so the projection
//! walks distant points inward instead of stalling on a vanishing
//! gradient. `target` is self-normalizing: |ψ|² is probed at the
//! orbital's mean radius in four directions and the `iso` parameter
//! counts *decades below that peak* — the same slider value gives a
//! comparable shell for every orbital, and all physical normalization
//! constants cancel (no factorials anywhere).
//!
//! Gradients are central finite differences of `u` (the analytic
//! chain rule through two recurrences buys little here).
//!
//! **Slicing** (`slice_z` + `slice_thickness`): same semantics as
//! [`cymatics3d`](super::cymatics3d) — 2D render mode shows the planar
//! cross-section at `z = slice_z` (thickness > 0 superimposes nearby
//! slices); 3D mode with thickness > 0 confines the attractor to the
//! z-slab via alternating projection. Note orbitals with an angular
//! node in the z = 0 plane (e.g. p_z) have no isosurface there — slice
//! elsewhere or pick another m.
//!
//! Direct color: *Distance* and *Depth* as in the cymatics family,
//! plus *Phase* — the classic quantum-chemistry rendering, coloring
//! each lobe by the sign of ψ (opposite palette ends for opposite
//! phase).
//!
//! No JWildfire/Apophysis equivalent — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static ORBITAL: VariationDef = VariationDef {
    name: "orbital",
    aliases: &[],
    display_name: "Electron Orbital",
    // Advanced2D (not Full3D) so the 2D slice body isn't filtered out
    // of 2D shaders — same rationale as cymatics3d; AlwaysZ still
    // gives true-3D z semantics in 3D mode.
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("n", "N (Shell)", int, 3.0, 1.0, 8.0, "Principal quantum number — the shell. Higher n = larger orbital with more radial node spheres."),
        param!("l", "L (Subshell)", int, 2.0, 0.0, 7.0, "Azimuthal quantum number — the subshell shape family: 0 = s (spheres), 1 = p (dumbbells), 2 = d, 3 = f, … Clamped to n − 1."),
        param!("m", "M (Orientation)", int, 0.0, -7.0, 7.0, "Magnetic quantum number, real form — selects the lobe orientation/count within the subshell (e.g. l = 2: m = 0 is the d_z² donut-and-lobes, ±1/±2 the four-leaf clovers). Clamped to ±l."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Spatial scale. The Bohr radius is auto-fitted so the orbital's mean radius is ~0.75·size for every n."),
        param!("iso", "Iso Depth", float, 2.0, 0.2, 8.0, "Isosurface level in decades below the orbital's peak density. Small values hug the density maxima (tight lobes); larger values inflate the shell outward and merge lobes."),
        param!("slice_z", "Slice Z", float, 0.0, -2.0, 2.0, "Slice position. 2D render mode: z of the planar cross-section. 3D mode with Slice Thickness > 0: center of the z-slab. Orbitals with a nodal plane at z = 0 (odd in z, e.g. p_z) are empty exactly there — nudge the slice or pick another m."),
        param!("slice_thickness", "Slice Thickness", float, 0.0, 0.0, 2.0, "Slab thickness around Slice Z. 2D: 0 cuts an exact plane; > 0 samples each point's slice z within the slab (X-ray-style superposition). 3D: 0 disables slicing (full volume); > 0 confines the attractor to the slab."),
        param!("steps", "Steps", int, 3.0, 1.0, 6.0, "Newton iterations toward the isosurface per call. 1 is soft and cloud-like; 3+ lands points crisply on the shell."),
        param!("strength", "Strength", float, 0.9, 0.0, 1.0, "Blend between the untouched input point (0) and the fully projected point (1). Lower values give the fuzzy electron-cloud look."),
        param!("jitter", "Jitter", float, 0.0, 0.0, 0.2, "Isotropic random offset added after projection — shell thickness / probability-cloud grain. 0 keeps the isosurface razor thin."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Distance", "Phase", "Depth"], "Direct-color source, applied through the transform's Direct Color slider. Distance: palette 1 on the shell fading away from it. Phase: the classic quantum-chemistry look — lobes colored by the sign of ψ, opposite phases at opposite palette ends. Depth: colors by the output z."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 4.0, "Contrast for the direct-color modes: Distance falloff sharpness, Phase saturation."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// The helpers are duplicated into the 2D and 3D strings (only one body
// is compiled per flame). ψ is evaluated UNNORMALIZED — the probe
// reference makes the physical constants cancel in u − u_ref.
// Numerics notes:
//   * e^(−ρ/2) underflows f32 for very distant points; ψ² floors at
//     1e-30 inside the log, giving a flat far field where points stay
//     put until another transform recycles them — harmless haze.
//   * Upward Legendre/Laguerre recurrences are stable for the small
//     orders exposed here (≤ 8).

const WGSL_3D: &str = r#"
// Generalized Laguerre L_k^alpha(x) by upward recurrence.
fn orbital_laguerre(k: i32, alpha: f32, x: f32) -> f32 {
    if (k <= 0) { return 1.0; }
    var lm1 = 1.0;
    var lc = 1.0 + alpha - x;
    for (var i = 2; i <= k; i = i + 1) {
        let fi = f32(i);
        let lnext = ((2.0 * fi - 1.0 + alpha - x) * lc - (fi - 1.0 + alpha) * lm1) / fi;
        lm1 = lc;
        lc = lnext;
    }
    return lc;
}

// Associated Legendre P_l^m(x), m >= 0, l >= m — standard upward
// recurrence from the closed-form P_m^m.
fn orbital_legendre(l: i32, m: i32, x: f32) -> f32 {
    let somx2 = sqrt(max(1.0 - x * x, 0.0));
    var pmm = 1.0;
    var fact = 1.0;
    for (var i = 0; i < m; i = i + 1) {
        pmm = pmm * (-fact) * somx2;
        fact = fact + 2.0;
    }
    if (l == m) { return pmm; }
    var pmmp1 = x * (2.0 * f32(m) + 1.0) * pmm;
    var pll = pmmp1;
    for (var ll = m + 2; ll <= l; ll = ll + 1) {
        let fll = f32(ll);
        pll = (x * (2.0 * fll - 1.0) * pmmp1 - (fll + f32(m) - 1.0) * pmm) / (fll - f32(m));
        pmm = pmmp1;
        pmmp1 = pll;
    }
    return pll;
}

// Unnormalized real-form hydrogen wavefunction at pos.
fn orbital_psi(pos: vec3<f32>, n: i32, l: i32, ms: i32, size: f32) -> f32 {
    let r = max(length(pos), 1e-9);
    let ct = clamp(pos.z / r, -1.0, 1.0);
    // a = size/(2n^2)  =>  rho = 2r/(n a) = 4 n r / size.
    let rho = 4.0 * f32(n) * r / size;
    let radial = exp(-0.5 * rho) * pow(rho, f32(l)) * orbital_laguerre(n - l - 1, f32(2 * l + 1), rho);
    let ma = abs(ms);
    var ang = orbital_legendre(l, ma, ct);
    if (ms != 0) {
        let phi = atan2(pos.y, pos.x);
        if (ms > 0) { ang = ang * cos(f32(ma) * phi); }
        else { ang = ang * sin(f32(ma) * phi); }
    }
    return radial * ang;
}

// Log density u = ln(psi^2), floored against underflow.
fn orbital_u(pos: vec3<f32>, n: i32, l: i32, ms: i32, size: f32) -> f32 {
    let psi = orbital_psi(pos, n, l, ms, size);
    return log(psi * psi + 1e-30);
}

// Reference log density: max |psi|^2 probed at the orbital's mean
// radius in four spread directions (no single direction is nonzero
// for every (l, m); the max over these is within a small factor of
// the true angular max — plenty for an iso slider in decades).
fn orbital_u_ref(n: i32, l: i32, ms: i32, size: f32) -> f32 {
    let rp = (3.0 * f32(n * n) - f32(l * (l + 1))) * size / (4.0 * f32(n * n));
    let inv_sqrt2 = 0.70710678;
    var u = orbital_u(vec3<f32>(0.0, 0.0, rp), n, l, ms, size);
    u = max(u, orbital_u(vec3<f32>(rp * inv_sqrt2, 0.0, rp * inv_sqrt2), n, l, ms, size));
    u = max(u, orbital_u(vec3<f32>(rp * inv_sqrt2, rp * inv_sqrt2, 0.0), n, l, ms, size));
    u = max(u, orbital_u(vec3<f32>(0.0, rp * inv_sqrt2, rp * inv_sqrt2), n, l, ms, size));
    return u;
}

fn variation_orbital(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let n = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let l = clamp(i32(get_param(xform_id, variation_id, 1u)), 0, n - 1);
    let ms = clamp(i32(get_param(xform_id, variation_id, 2u)), -l, l);
    let size = max(get_param(xform_id, variation_id, 3u), 1e-6);
    let iso = get_param(xform_id, variation_id, 4u);
    let slice_z = get_param(xform_id, variation_id, 5u);
    let slice_thickness = get_param(xform_id, variation_id, 6u);
    let steps = i32(get_param(xform_id, variation_id, 7u));
    let strength = get_param(xform_id, variation_id, 8u);
    let jitter = get_param(xform_id, variation_id, 9u);
    let dc_mode = u32(get_param(xform_id, variation_id, 10u));
    let dc_scale = get_param(xform_id, variation_id, 11u);

    let u_target = orbital_u_ref(n, l, ms, size) - iso * 2.302585;
    let h = 0.005 * size;
    let max_step = 0.5 * size / f32(n);
    let half_t = 0.5 * slice_thickness;

    var q = p;
    for (var i = 0; i < steps; i = i + 1) {
        let f = orbital_u(q, n, l, ms, size) - u_target;
        let g = vec3<f32>(
            orbital_u(q + vec3<f32>(h, 0.0, 0.0), n, l, ms, size) - orbital_u(q - vec3<f32>(h, 0.0, 0.0), n, l, ms, size),
            orbital_u(q + vec3<f32>(0.0, h, 0.0), n, l, ms, size) - orbital_u(q - vec3<f32>(0.0, h, 0.0), n, l, ms, size),
            orbital_u(q + vec3<f32>(0.0, 0.0, h), n, l, ms, size) - orbital_u(q - vec3<f32>(0.0, 0.0, h), n, l, ms, size),
        ) / (2.0 * h);
        var step = f * g / (dot(g, g) + 1e-9);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
        if (half_t > 0.0) {
            q.z = clamp(q.z, slice_z - half_t, slice_z + half_t);
        }
    }

    var out = mix(p, q, strength);

    if (dc_mode == 1u) {
        // Distance in log space converted to world units via |grad u|.
        let f = orbital_u(p, n, l, ms, size) - u_target;
        let g = vec3<f32>(
            orbital_u(p + vec3<f32>(h, 0.0, 0.0), n, l, ms, size) - orbital_u(p - vec3<f32>(h, 0.0, 0.0), n, l, ms, size),
            orbital_u(p + vec3<f32>(0.0, h, 0.0), n, l, ms, size) - orbital_u(p - vec3<f32>(0.0, h, 0.0), n, l, ms, size),
            orbital_u(p + vec3<f32>(0.0, 0.0, h), n, l, ms, size) - orbital_u(p - vec3<f32>(0.0, 0.0, h), n, l, ms, size),
        ) / (2.0 * h);
        let dist = abs(f) / (length(g) + 1e-6);
        *vc = exp(-6.0 * dc_scale * dist / (0.25 * size));
    } else if (dc_mode == 2u) {
        // Phase: sign of psi at the landed point, saturated relative
        // to the isosurface amplitude.
        let psi_q = orbital_psi(out, n, l, ms, size);
        let psi_iso = exp(0.5 * u_target);
        *vc = 0.5 + 0.5 * tanh(dc_scale * psi_q / (psi_iso + 1e-30));
    } else if (dc_mode == 3u) {
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * out.z / size);
    }

    if (jitter > 0.0) {
        out = out + vec3<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;

const WGSL_2D: &str = r#"
// Generalized Laguerre L_k^alpha(x) by upward recurrence.
fn orbital_laguerre(k: i32, alpha: f32, x: f32) -> f32 {
    if (k <= 0) { return 1.0; }
    var lm1 = 1.0;
    var lc = 1.0 + alpha - x;
    for (var i = 2; i <= k; i = i + 1) {
        let fi = f32(i);
        let lnext = ((2.0 * fi - 1.0 + alpha - x) * lc - (fi - 1.0 + alpha) * lm1) / fi;
        lm1 = lc;
        lc = lnext;
    }
    return lc;
}

// Associated Legendre P_l^m(x), m >= 0, l >= m — standard upward
// recurrence from the closed-form P_m^m.
fn orbital_legendre(l: i32, m: i32, x: f32) -> f32 {
    let somx2 = sqrt(max(1.0 - x * x, 0.0));
    var pmm = 1.0;
    var fact = 1.0;
    for (var i = 0; i < m; i = i + 1) {
        pmm = pmm * (-fact) * somx2;
        fact = fact + 2.0;
    }
    if (l == m) { return pmm; }
    var pmmp1 = x * (2.0 * f32(m) + 1.0) * pmm;
    var pll = pmmp1;
    for (var ll = m + 2; ll <= l; ll = ll + 1) {
        let fll = f32(ll);
        pll = (x * (2.0 * fll - 1.0) * pmmp1 - (fll + f32(m) - 1.0) * pmm) / (fll - f32(m));
        pmm = pmmp1;
        pmmp1 = pll;
    }
    return pll;
}

// Unnormalized real-form hydrogen wavefunction at pos.
fn orbital_psi(pos: vec3<f32>, n: i32, l: i32, ms: i32, size: f32) -> f32 {
    let r = max(length(pos), 1e-9);
    let ct = clamp(pos.z / r, -1.0, 1.0);
    // a = size/(2n^2)  =>  rho = 2r/(n a) = 4 n r / size.
    let rho = 4.0 * f32(n) * r / size;
    let radial = exp(-0.5 * rho) * pow(rho, f32(l)) * orbital_laguerre(n - l - 1, f32(2 * l + 1), rho);
    let ma = abs(ms);
    var ang = orbital_legendre(l, ma, ct);
    if (ms != 0) {
        let phi = atan2(pos.y, pos.x);
        if (ms > 0) { ang = ang * cos(f32(ma) * phi); }
        else { ang = ang * sin(f32(ma) * phi); }
    }
    return radial * ang;
}

// Log density u = ln(psi^2), floored against underflow.
fn orbital_u(pos: vec3<f32>, n: i32, l: i32, ms: i32, size: f32) -> f32 {
    let psi = orbital_psi(pos, n, l, ms, size);
    return log(psi * psi + 1e-30);
}

// Reference log density: max |psi|^2 probed at the orbital's mean
// radius in four spread directions.
fn orbital_u_ref(n: i32, l: i32, ms: i32, size: f32) -> f32 {
    let rp = (3.0 * f32(n * n) - f32(l * (l + 1))) * size / (4.0 * f32(n * n));
    let inv_sqrt2 = 0.70710678;
    var u = orbital_u(vec3<f32>(0.0, 0.0, rp), n, l, ms, size);
    u = max(u, orbital_u(vec3<f32>(rp * inv_sqrt2, 0.0, rp * inv_sqrt2), n, l, ms, size));
    u = max(u, orbital_u(vec3<f32>(rp * inv_sqrt2, rp * inv_sqrt2, 0.0), n, l, ms, size));
    u = max(u, orbital_u(vec3<f32>(0.0, rp * inv_sqrt2, rp * inv_sqrt2), n, l, ms, size));
    return u;
}

fn variation_orbital(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let n = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let l = clamp(i32(get_param(xform_id, variation_id, 1u)), 0, n - 1);
    let ms = clamp(i32(get_param(xform_id, variation_id, 2u)), -l, l);
    let size = max(get_param(xform_id, variation_id, 3u), 1e-6);
    let iso = get_param(xform_id, variation_id, 4u);
    let slice_z = get_param(xform_id, variation_id, 5u);
    let slice_thickness = get_param(xform_id, variation_id, 6u);
    let steps = i32(get_param(xform_id, variation_id, 7u));
    let strength = get_param(xform_id, variation_id, 8u);
    let jitter = get_param(xform_id, variation_id, 9u);
    let dc_mode = u32(get_param(xform_id, variation_id, 10u));
    let dc_scale = get_param(xform_id, variation_id, 11u);

    let u_target = orbital_u_ref(n, l, ms, size) - iso * 2.302585;
    let h = 0.005 * size;
    let max_step = 0.5 * size / f32(n);

    // Slice z for this point: exact plane, or sampled within the slab.
    let z_eval = slice_z + (rng_nextf(rng) - 0.5) * slice_thickness;

    var q = p;
    for (var i = 0; i < steps; i = i + 1) {
        let q3 = vec3<f32>(q, z_eval);
        let f = orbital_u(q3, n, l, ms, size) - u_target;
        let g = vec2<f32>(
            orbital_u(q3 + vec3<f32>(h, 0.0, 0.0), n, l, ms, size) - orbital_u(q3 - vec3<f32>(h, 0.0, 0.0), n, l, ms, size),
            orbital_u(q3 + vec3<f32>(0.0, h, 0.0), n, l, ms, size) - orbital_u(q3 - vec3<f32>(0.0, h, 0.0), n, l, ms, size),
        ) / (2.0 * h);
        var step = f * g / (dot(g, g) + 1e-9);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }

    var out = mix(p, q, strength);

    if (dc_mode == 1u) {
        let p3 = vec3<f32>(p, z_eval);
        let f = orbital_u(p3, n, l, ms, size) - u_target;
        let g = vec2<f32>(
            orbital_u(p3 + vec3<f32>(h, 0.0, 0.0), n, l, ms, size) - orbital_u(p3 - vec3<f32>(h, 0.0, 0.0), n, l, ms, size),
            orbital_u(p3 + vec3<f32>(0.0, h, 0.0), n, l, ms, size) - orbital_u(p3 - vec3<f32>(0.0, h, 0.0), n, l, ms, size),
        ) / (2.0 * h);
        let dist = abs(f) / (length(g) + 1e-6);
        *vc = exp(-6.0 * dc_scale * dist / (0.25 * size));
    } else if (dc_mode == 2u) {
        let psi_q = orbital_psi(vec3<f32>(out, z_eval), n, l, ms, size);
        let psi_iso = exp(0.5 * u_target);
        *vc = 0.5 + 0.5 * tanh(dc_scale * psi_q / (psi_iso + 1e-30));
    } else if (dc_mode == 3u) {
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * z_eval / size);
    }

    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;
