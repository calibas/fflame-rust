//! `quaternion_cubic` — 4D quaternion **cubic** Julia map `q' = q³ + c`.
//!
//! The cubic sibling of [`quaternion_julia`](super::quaternion_julia). Same
//! machinery (the running 3D point is the quaternion vector part, `point_w`
//! the scalar part via `Feature::NeedsW`), but the map is `q³ + c` instead of
//! `q² + c` — giving 3-fold (rather than 2-fold) lobe structure.
//!
//! * **Forward** (`inverse=0`): `q' = q³ + c`. An IFS attractor of the forward
//!   map, not the Julia set (the chaos game iterates forward — see
//!   `quaternion_julia`).
//! * **Inverse** (`inverse=1`): `q' = (q − c)^{1/3}` choosing one of the **three**
//!   cube-root branches at random (the cubic map has three preimages). The
//!   Inverse Iteration Method — converges the chaos game onto the cubic Julia
//!   set. Use alone, identity affine, weight 1.0.
//!
//! 2D mode is the complex cubic Julia `z³ + (cx + i·cy)` / its inverse.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUATERNION_CUBIC: VariationDef = VariationDef {
    name: "quaternion_cubic",
    aliases: &["qcubic"],
    display_name: "Quaternion Cubic Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsW, Feature::NeedsRng],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cx", "Constant X", float, 0.4, -2.0, 2.0, "Vector-i component of the Julia constant quaternion c."),
        param!("cy", "Constant Y", float, 0.0, -2.0, 2.0, "Vector-j component of c."),
        param!("cz", "Constant Z", float, 0.0, -2.0, 2.0, "Vector-k component of c. 3D only."),
        param!("cw", "Constant W", float, 0.0, -2.0, 2.0, "Scalar component of c (drives the 4th dimension). 3D only."),
        param!("projection", "Projection", unlimited_int, 0.0, 0.0, 2.0, "How the 4D result maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z; z and w swap), 2 = Perspective (divide xyz by 1-w). The return both plots AND feeds forward, so each mode is effectively a different attractor."),
        param!("inverse", "Inverse (Julia set)", unlimited_int, 0.0, 0.0, 1.0, "0 = forward q^3 + c (IFS attractor, NOT a Julia set under the chaos game). 1 = inverse (q - c)^(1/3) with a random one of the three branches: the Inverse Iteration Method, which converges to the cubic Julia set. Use alone, identity affine, weight 1.0."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_cubic(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    if (get_param(xform_id, variation_id, 5u) > 0.5) {
        // Inverse: one of the three complex cube roots of (z - c), at random.
        let d = p - vec2<f32>(cx, cy);
        let k = floor(rng_nextf(rng) * 3.0);                 // branch 0, 1, or 2
        let theta = (atan2(d.y, d.x) + k * 6.28318530718) / 3.0;
        let m = pow(length(d), 1.0 / 3.0);
        return m * vec2<f32>(cos(theta), sin(theta));
    }
    // Forward: z^3 + c.  z^3 = (x^3 - 3xy^2,  3x^2 y - y^3).
    let x = p.x;
    let y = p.y;
    return vec2<f32>(x * x * x - 3.0 * x * y * y + cx, 3.0 * x * x * y - y * y * y + cy);
}
"#;

const WGSL_3D: &str = r#"
fn qcubic_qmul(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z
    );
}

// One of the three quaternion cube roots of `p`, selected by `branch` (0,1,2).
// q = |q|·(cos θ + n̂·sin θ), θ ∈ [0, π]; the cube roots take the angle to
// (θ + 2πk)/3, scaling magnitude by |q|^{1/3}.
fn qcubic_qcbrt(p: vec4<f32>, branch: f32) -> vec4<f32> {
    let mag = length(p) + 1e-12;
    let cmag = pow(mag, 1.0 / 3.0);
    let theta = (acos(clamp(p.w / mag, -1.0, 1.0)) + branch * 6.28318530718) / 3.0;
    let vlen = length(p.xyz);
    let n = select(vec3<f32>(1.0, 0.0, 0.0), p.xyz / vlen, vlen > 1e-9);
    return vec4<f32>(cmag * sin(theta) * n, cmag * cos(theta));
}

fn variation_quaternion_cubic(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let q = vec4<f32>(p, point_w);
    let c = vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );

    var r: vec4<f32>;
    if (get_param(xform_id, variation_id, 5u) > 0.5) {
        // Inverse Iteration Method: q -> (q - c)^(1/3), random branch of 3.
        r = qcubic_qcbrt(q - c, floor(rng_nextf(rng) * 3.0));
    } else {
        r = qcubic_qmul(qcubic_qmul(q, q), q) + c;   // forward q^3 + c
    }

    // Project the 4D result to the plotted/fed-forward 3D point (see
    // quaternion_julia for the projection-shapes-the-attractor caveat).
    let mode = u32(get_param(xform_id, variation_id, 4u) + 0.5);
    switch (mode) {
        case 1u: {
            point_w = r.z;
            return vec3<f32>(r.x, r.y, r.w);
        }
        case 2u: {
            point_w = r.w;
            let denom = 1.0 - r.w;
            let safe = select(denom, 1e-3, abs(denom) < 1e-3);
            return r.xyz / safe;
        }
        default: {
            point_w = r.w;
            return r.xyz;
        }
    }
}
"#;
