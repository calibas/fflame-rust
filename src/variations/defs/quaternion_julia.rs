//! `quaternion_julia` — 4D quaternion **Julia-N** map `q' = qⁿ + c`.
//!
//! The running 3D point is the quaternion's vector part `(x, y, z)`; the scalar
//! part `w` lives in the per-thread `point_w` (`Feature::NeedsW`), so the full
//! 4D quaternion survives transform switches across the walk.
//!
//! Generalized over an integer **`power` (n)** and a radial **`dist`** exponent
//! (the flam3 `julian` parameterization), so n=2 is the quadratic Julia
//! (rabbit), n=3 the cubic, etc. Two directions via `inverse`:
//!
//! * **Forward** (`inverse=0`): `q' = qⁿ + c` via the polar form
//!   `qⁿ = |q|ⁿ·(cos nθ + n̂·sin nθ)`. Under the (forward-iterating) chaos game
//!   this is an IFS attractor of the forward map, NOT the Julia set.
//! * **Inverse** (`inverse=1`): `q' = (q − c)^{1/n}` picking one of the **n**
//!   roots at random, with radial exponent `dist/n` — the Inverse Iteration
//!   Method, which converges to the Julia set. `dist=1` gives the true root;
//!   other values distort it radially (the `julian` look). Use alone, identity
//!   affine, weight 1.0.
//!
//! 2D mode is the complex Julia-N `zⁿ + (cx + i·cy)` / its inverse.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUATERNION_JULIA: VariationDef = VariationDef {
    name: "quaternion_julia",
    aliases: &["qjulia", "qjulian"],
    display_name: "Quaternion Julia (N)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // NeedsW: the persistent 4th coordinate. NeedsRng: the inverse map picks one
    // of the n roots at random. WritesColor: optional color-by-w.
    features: &[Feature::NeedsW, Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cx", "Constant X", float, -0.2, -2.0, 2.0, "Vector-i component of the Julia constant quaternion c."),
        param!("cy", "Constant Y", float, 0.4, -2.0, 2.0, "Vector-j component of c."),
        param!("cz", "Constant Z", float, 0.0, -2.0, 2.0, "Vector-k component of c. 3D only."),
        param!("cw", "Constant W", float, 0.0, -2.0, 2.0, "Scalar component of c (drives the 4th dimension). 3D only."),
        param!("power", "Power (N)", int, 2.0, -16.0, 16.0, "The Julia power n: q^n + c forward, or the n-branch root (q-c)^(1/n) inverse. n=2 quadratic (rabbit), n=3 cubic, etc. |n| sets the number of inverse root branches."),
        param!("dist", "Distance", unlimited_float, 1.0, -4.0, 4.0, "Radial exponent multiplier for the inverse root: magnitude = |q-c|^(dist/n). 1.0 = the true n-th root (real Julia set); other values distort radially (the flam3 `julian` effect). No effect on the forward map."),
        param!("projection", "Projection", unlimited_int, 0.0, 0.0, 2.0, "How the 4D result maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z; z and w swap), 2 = Perspective (divide xyz by 1-w). The return both plots AND feeds forward, so each mode is effectively a different attractor."),
        param!("inverse", "Inverse (Julia set)", unlimited_int, 0.0, 0.0, 1.0, "0 = forward q^n + c (an IFS attractor, NOT a Julia set under the chaos game). 1 = inverse (q - c)^(1/n) with a random branch: the Inverse Iteration Method, which converges to the actual Julia set. Use inverse=1 with an identity affine and weight 1.0."),
        param!("w_color", "Color by W", float, 0.0, 0.0, 8.0, "0 = off. >0 = write a palette index from the 4th coordinate (3D: fract(w * scale); 2D: fract(|z| * scale)), revealing the hidden dimension as COLOR without altering the attractor. Needs the transform's direct_color > 0."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_julia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let n = get_param(xform_id, variation_id, 4u);
    let dist = get_param(xform_id, variation_id, 5u);
    let safe_n = select(n, 1e-6, abs(n) < 1e-6);
    var out: vec2<f32>;
    if (get_param(xform_id, variation_id, 7u) > 0.5) {
        // Inverse: one of the n complex n-th roots of (z - c), at random.
        let d = p - vec2<f32>(cx, cy);
        let branch = floor(rng_nextf(rng) * max(1.0, abs(n)));
        let theta = (atan2(d.y, d.x) + branch * 6.28318530718) / safe_n;
        out = pow(length(d), dist / safe_n) * vec2<f32>(cos(theta), sin(theta));
    } else {
        // Forward: z^n + c via polar form.
        let theta = atan2(p.y, p.x) * n;
        out = pow(length(p), n) * vec2<f32>(cos(theta), sin(theta)) + vec2<f32>(cx, cy);
    }
    let wcol = get_param(xform_id, variation_id, 8u);
    if (wcol > 1e-6) { *vc = fract(length(out) * wcol); }
    return out;
}
"#;

const WGSL_3D: &str = r#"
// q^n via polar form: q = |q|·(cos θ + n̂·sin θ), q^n = |q|^n·(cos nθ + n̂·sin nθ).
// Convention: q = (x, y, z, w), scalar part w, vector part (x,y,z).
fn qjulia_qpow(q: vec4<f32>, n: f32) -> vec4<f32> {
    let mag = length(q) + 1e-12;
    let rad = pow(mag, n);
    let ang = acos(clamp(q.w / mag, -1.0, 1.0)) * n;
    let vlen = length(q.xyz);
    let nhat = select(vec3<f32>(1.0, 0.0, 0.0), q.xyz / vlen, vlen > 1e-9);
    return vec4<f32>(rad * sin(ang) * nhat, rad * cos(ang));
}

// One of the n quaternion n-th roots of q, selected by `branch` (0..n-1), with
// radial exponent dist/n. Generalizes √ (n=2) and ∛ (n=3).
fn qjulia_qroot(q: vec4<f32>, n: f32, branch: f32, dist: f32) -> vec4<f32> {
    let mag = length(q) + 1e-12;
    let rad = pow(mag, dist / n);
    let ang = (acos(clamp(q.w / mag, -1.0, 1.0)) + branch * 6.28318530718) / n;
    let vlen = length(q.xyz);
    let nhat = select(vec3<f32>(1.0, 0.0, 0.0), q.xyz / vlen, vlen > 1e-9);
    return vec4<f32>(rad * sin(ang) * nhat, rad * cos(ang));
}

fn variation_quaternion_julia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let q = vec4<f32>(p, point_w);          // 4D point: vector = p, scalar = w
    let c = vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );
    let n = get_param(xform_id, variation_id, 4u);
    let dist = get_param(xform_id, variation_id, 5u);
    let safe_n = select(n, 1e-6, abs(n) < 1e-6);

    var r: vec4<f32>;
    if (get_param(xform_id, variation_id, 7u) > 0.5) {
        // Inverse Iteration Method: q -> (q - c)^(1/n), random branch of |n|.
        let branch = floor(rng_nextf(rng) * max(1.0, abs(n)));
        r = qjulia_qroot(q - c, safe_n, branch, dist);
    } else {
        r = qjulia_qpow(q, n) + c;          // forward q^n + c
    }

    // Color by the 4th coordinate w (dynamics untouched) when enabled.
    let wcol = get_param(xform_id, variation_id, 8u);
    if (wcol > 1e-6) { *vc = fract(r.w * wcol); }

    // Project the 4D result (x,y,z,w) to the plotted/fed-forward 3D point.
    // The flame plots `current` (= this return) AND feeds it to the next
    // iteration, so the projection shapes the attractor, not just the view.
    let mode = u32(get_param(xform_id, variation_id, 6u) + 0.5);
    switch (mode) {
        case 1u: {
            // Depth: surface w as the z axis; z and w swap roles each step.
            point_w = r.z;
            return vec3<f32>(r.x, r.y, r.w);
        }
        case 2u: {
            // Perspective: fold w into a depth-like foreshortening by
            // dividing xyz by (1 - w). Guard the singularity at w = 1.
            point_w = r.w;
            let denom = 1.0 - r.w;
            let safe = select(denom, 1e-3, abs(denom) < 1e-3);
            return r.xyz / safe;
        }
        default: {
            // Vector: drop w (it evolves independently, hidden from the plot).
            point_w = r.w;
            return r.xyz;
        }
    }
}
"#;
