//! `quaternion_julia` — 4D quaternion Julia map `q' = q² + c`.
//!
//! A starter/experimental 4D variation. The running 3D point supplies the
//! quaternion's vector part `(x, y, z)`; the scalar part `w` lives in the
//! per-thread `point_w` (`Feature::NeedsW`), so the full 4D quaternion
//! survives transform switches across the walk. Each iteration squares the
//! quaternion (Hamilton product) and adds the constant `c = (cx, cy, cz, cw)`.
//!
//! Two directions, via the `inverse` parameter:
//!
//! * **Forward** (`inverse=0`): `q' = q² + c`. Under the chaos game (which
//!   iterates *forward* and plots every point) this does NOT trace a Julia set
//!   — forward iteration escapes to ∞ or falls into an attracting cycle, fleeing
//!   the repelling Julia boundary. It's an IFS attractor of the forward map.
//! * **Inverse** (`inverse=1`): `q' = ±√(q − c)` with the branch chosen at
//!   random — the **Inverse Iteration Method**. The Julia set is the *attractor*
//!   of the inverse map, so the chaos game converges onto it. This is how you
//!   render an actual Julia set; use it alone on a transform with an identity
//!   affine at weight 1.0.
//!
//! Intended to be used **alone** on a transform at weight 1.0: the flame
//! dispatcher does `result += weight · body(p)`, so at weight 1 the output *is*
//! the map. 2D mode is the complex Julia `z² + (cx + i·cy)` / its inverse.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUATERNION_JULIA: VariationDef = VariationDef {
    name: "quaternion_julia",
    aliases: &["qjulia"],
    display_name: "Quaternion Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // NeedsW: emits the per-thread `point_w` 4th coordinate + resets it on
    // bad-value respawn. NeedsRng: the inverse map picks one of the two square
    // roots at random each step (the Inverse Iteration Method).
    features: &[Feature::NeedsW, Feature::NeedsRng],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cx", "Constant X", float, -0.2, -2.0, 2.0, "Vector-i component of the Julia constant quaternion c."),
        param!("cy", "Constant Y", float, 0.4, -2.0, 2.0, "Vector-j component of c."),
        param!("cz", "Constant Z", float, 0.0, -2.0, 2.0, "Vector-k component of c. 3D only."),
        param!("cw", "Constant W", float, 0.0, -2.0, 2.0, "Scalar component of c (drives the 4th dimension). 3D only."),
        param!("projection", "Projection", unlimited_int, 0.0, 0.0, 2.0, "How the 4D result maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z; z and w swap), 2 = Perspective (divide xyz by 1-w). The return both plots AND feeds forward, so each mode is effectively a different attractor, not just a different view."),
        param!("inverse", "Inverse (Julia set)", unlimited_int, 0.0, 0.0, 1.0, "0 = forward q^2 + c (an IFS attractor of the forward map — NOT a Julia set under the chaos game). 1 = inverse ±sqrt(q - c): the Inverse Iteration Method, which converges to the actual Julia set. Use inverse=1 with an identity affine and weight 1.0 (alone on the transform) to render a real Julia."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_julia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Complex Julia (the 2D case). `point_w` rides unused here.
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    if (get_param(xform_id, variation_id, 5u) > 0.5) {
        // Inverse Iteration Method: z -> ±sqrt(z - c). Picking the branch at
        // random, the chaos game converges to the Julia set itself.
        let d = p - vec2<f32>(cx, cy);
        let half = atan2(d.y, d.x) * 0.5;
        var root = sqrt(length(d)) * vec2<f32>(cos(half), sin(half));
        if (rng_nextf(rng) < 0.5) { root = -root; }
        return root;
    }
    // Forward: z^2 + c — the IFS attractor of the forward map, not the Julia set.
    return vec2<f32>(p.x * p.x - p.y * p.y + cx, 2.0 * p.x * p.y + cy);
}
"#;

const WGSL_3D: &str = r#"
// Hamilton product. Quaternion convention here: q = (x, y, z, w) where w is
// the scalar part (stored in `point_w`) and (x,y,z) is the vector part (the
// running point).
fn qjulia_qmul(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z
    );
}

// Principal quaternion square root via the half-angle identities (no acos).
// q = |q|·(cos θ + n̂·sin θ) with cos θ = q.w/|q|, n̂ = vec/|vec|; the root is
// sqrt(|q|)·(cos(θ/2) + n̂·sin(θ/2)). The other root is its negation.
fn qjulia_qsqrt(q: vec4<f32>) -> vec4<f32> {
    let mag = length(q) + 1e-12;
    let smag = sqrt(mag);
    let cos_half = sqrt(max(0.0, 0.5 * (1.0 + q.w / mag)));
    let sin_half = sqrt(max(0.0, 0.5 * (1.0 - q.w / mag)));
    let vlen = length(q.xyz);
    let n = select(vec3<f32>(1.0, 0.0, 0.0), q.xyz / vlen, vlen > 1e-9);
    return vec4<f32>(smag * sin_half * n, smag * cos_half);
}

fn variation_quaternion_julia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let q = vec4<f32>(p, point_w);          // 4D point: vector = p, scalar = w
    let c = vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );

    var r: vec4<f32>;
    if (get_param(xform_id, variation_id, 5u) > 0.5) {
        // Inverse Iteration Method: q -> ±sqrt(q - c), random branch.
        // Converges the chaos game to the (quaternion) Julia set.
        r = qjulia_qsqrt(q - c);
        if (rng_nextf(rng) < 0.5) { r = -r; }
    } else {
        r = qjulia_qmul(q, q) + c;          // forward q^2 + c
    }

    // Project the 4D result (x,y,z,w) to the plotted/fed-forward 3D point.
    // The flame plots `current` (= this return) AND feeds it to the next
    // iteration, so the projection shapes the attractor, not just the view.
    let mode = u32(get_param(xform_id, variation_id, 4u) + 0.5);
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
