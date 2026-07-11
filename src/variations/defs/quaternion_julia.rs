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
//! **4D caveat:** the inverse root inherits the *axis* of `q − c`, so the walk
//! collapses onto the 2D complex plane through `1` and `c` (verified: the
//! transverse `j,k` magnitude decays geometrically to zero). Inverse mode is
//! therefore a faithful *2D* Julia renderer embedded in 4D, but it does **not**
//! fill the genuine 4D quaternion Julia set — for that (Bourke's solids and
//! their slices) use [`quaternion_julia_set`], which tests membership directly.
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
    // of the n roots at random. WritesColor: optional color-by-w. WritesRgb:
    // optional w-driven brightness/saturation of the palette color. AlwaysZ:
    // the 3D body writes z unconditionally (it's the quaternion's k component)
    // — without it, preserve_z = false would flatten k every iteration and the
    // 4D map silently degenerates to the (x, y, w) subspace.
    features: &[Feature::NeedsW, Feature::NeedsRng, Feature::WritesColor, Feature::WritesRgb, Feature::AlwaysZ],
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
        param!("w_bright", "Brightness by W", unlimited_float, 0.0, -2.0, 2.0, "0 = off. Scales the sample's palette color by (1 + w_bright*w): positive = high-w structure glows brighter (feeds the Glow post-effect nicely), negative = it dims. Hue-preserving; 3D only. Needs the transform's direct_color > 0."),
        param!("w_sat", "Saturation by W", unlimited_float, 0.0, -2.0, 2.0, "0 = off. Shifts the sample's color saturation by (1 + w_sat*w) around its luminance: negative w_sat washes high-w structure toward gray, >1 total over-saturates. Hue-preserving; 3D only. Needs the transform's direct_color > 0."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_julia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    // vrc unused in 2D: w-shading (w_bright/w_sat) is a 4D-only effect.
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

fn variation_quaternion_julia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
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

    // w-shading: scale brightness / saturation of the palette color by w.
    // Samples the palette at the CURRENT color coordinate (post any w_color
    // write above) and hands the shaded RGB to the direct-color mix at plot.
    let wb = get_param(xform_id, variation_id, 9u);
    let ws = get_param(xform_id, variation_id, 10u);
    if (abs(wb) > 1e-6 || abs(ws) > 1e-6) {
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(clamp(*vc, 0.0, 1.0), 0.5), 0.0).rgb;
        var col = srgb_to_linear(srgb) * clamp(1.0 + wb * r.w, 0.0, 4.0);
        let luma = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
        col = mix(vec3<f32>(luma), col, clamp(1.0 + ws * r.w, 0.0, 2.0));
        *vrc = col;
    }

    // Project the 4D result (x,y,z,w) to the plotted/fed-forward 3D point.
    // The flame plots `current` (= this return) AND feeds it to the next
    // iteration, so the projection shapes the attractor, not just the view.
    let mode = u32(get_param(xform_id, variation_id, 6u) + 0.5);
    switch (mode) {
        case 1u: {
            // Depth: surface w as the z axis; z and w swap roles each step.
            point_w_out = r.z;
            return vec3<f32>(r.x, r.y, r.w);
        }
        case 2u: {
            // Perspective: fold w into a depth-like foreshortening by
            // dividing xyz by (1 - w). Guard the singularity at w = 1.
            point_w_out = r.w;
            let denom = 1.0 - r.w;
            let safe = select(denom, 1e-3, abs(denom) < 1e-3);
            return r.xyz / safe;
        }
        default: {
            // Vector: drop w (it evolves independently, hidden from the plot).
            point_w_out = r.w;
            return r.xyz;
        }
    }
}
"#;

/// Algebraic-identity verification for the quaternion helpers. These mirror the
/// WGSL bodies (qmul from quaternion_rotation, qpow/qroot above) line-for-line
/// in Rust and assert the relations that MUST hold if the math is correct — no
/// reference render needed:
///   * `qmul` anchored by identity and `i·j = k`,
///   * `qpow(q, 2) == qmul(q, q)` and `qpow(q, 3) == q·q·q` (polar == Hamilton),
///   * `qpow(qroot(q, n, k, 1), n) == q` for every branch (root inverts power) —
///     the correctness of the inverse-iteration Julia math,
///   * `|â·q| == |q|` (the rotation is an isometry).
#[cfg(test)]
mod quaternion_identity_tests {
    type Q = [f32; 4]; // (x, y, z, w) — vector part (x,y,z), scalar w.

    fn norm(q: Q) -> f32 {
        (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt()
    }

    /// Hamilton product — mirrors `qrot_qmul` (quaternion_rotation) /
    /// `qjset_qmul` (quaternion_julia_set), which are byte-identical.
    fn qmul(a: Q, b: Q) -> Q {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    }

    /// Polar power — mirrors WGSL `qjulia_qpow`.
    fn qpow(q: Q, n: f32) -> Q {
        let mag = norm(q) + 1e-12;
        let rad = mag.powf(n);
        let ang = (q[3] / mag).clamp(-1.0, 1.0).acos() * n;
        qpolar(q, rad, ang)
    }

    /// Polar n-th root, branch `k` — mirrors WGSL `qjulia_qroot`.
    fn qroot(q: Q, n: f32, branch: f32, dist: f32) -> Q {
        let mag = norm(q) + 1e-12;
        let rad = mag.powf(dist / n);
        let ang = ((q[3] / mag).clamp(-1.0, 1.0).acos() + branch * std::f32::consts::TAU) / n;
        qpolar(q, rad, ang)
    }

    fn qpolar(q: Q, rad: f32, ang: f32) -> Q {
        let vlen = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
        let nhat = if vlen > 1e-9 {
            [q[0] / vlen, q[1] / vlen, q[2] / vlen]
        } else {
            [1.0, 0.0, 0.0]
        };
        [rad * ang.sin() * nhat[0], rad * ang.sin() * nhat[1], rad * ang.sin() * nhat[2], rad * ang.cos()]
    }

    fn approx(a: Q, b: Q, eps: f32) -> bool {
        (0..4).all(|i| (a[i] - b[i]).abs() < eps)
    }

    // General + edge cases (pure real ±, pure vector).
    const SAMPLES: [Q; 7] = [
        [0.3, -0.5, 0.7, 0.2],
        [-0.6, 0.2, 0.1, 0.8],
        [0.5, 0.5, 0.5, 0.5],
        [1.2, -0.3, 0.4, -0.9],
        [0.0, 0.0, 0.0, 0.7],
        [0.0, 0.0, 0.0, -0.7],
        [0.8, 0.0, 0.0, 0.0],
    ];

    #[test]
    fn qmul_anchored() {
        let id = [0.0, 0.0, 0.0, 1.0];
        let q = [0.3, -0.5, 0.7, 0.2];
        assert!(approx(qmul(q, id), q, 1e-6), "q·1 = q");
        assert!(approx(qmul(id, q), q, 1e-6), "1·q = q");
        // i·j = k
        assert!(approx(qmul([1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]), [0.0, 0.0, 1.0, 0.0], 1e-6));
    }

    #[test]
    fn qpow_matches_hamilton() {
        for &q in &SAMPLES {
            assert!(approx(qpow(q, 2.0), qmul(q, q), 3e-3), "q^2 != q·q for {:?}", q);
            assert!(approx(qpow(q, 3.0), qmul(qmul(q, q), q), 3e-3), "q^3 != q·q·q for {:?}", q);
        }
    }

    #[test]
    fn qroot_inverts_qpow() {
        for &q in &SAMPLES {
            for n in [2i32, 3, 4] {
                for k in 0..n {
                    let back = qpow(qroot(q, n as f32, k as f32, 1.0), n as f32);
                    assert!(
                        approx(back, q, 3e-3),
                        "qpow(qroot(q,{n},{k}),{n}) != q; q={q:?} back={back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn rotation_is_isometry() {
        let raw = [0.3, 0.1, -0.2, 0.9];
        let m = norm(raw);
        let a = [raw[0] / m, raw[1] / m, raw[2] / m, raw[3] / m]; // unit
        let conj_a = [-a[0], -a[1], -a[2], a[3]];
        for &q in &SAMPLES {
            // Left, right, and sandwich (â·q·b̂) multiplications by unit
            // quaternions are all rotations of R⁴ — norms preserved.
            let left = qmul(a, q);
            assert!((norm(left) - norm(q)).abs() < 1e-4, "|â·q| != |q| for {:?}", q);
            let right = qmul(q, a);
            assert!((norm(right) - norm(q)).abs() < 1e-4, "|q·â| != |q| for {:?}", q);
            let sandwich = qmul(qmul(a, q), conj_a);
            assert!((norm(sandwich) - norm(q)).abs() < 1e-4, "|â·q·ā| != |q| for {:?}", q);
            // b̂ = conjugate(â) is the ordinary 3D rotation: the scalar part
            // (our point_w) must pass through untouched.
            assert!(
                (sandwich[3] - q[3]).abs() < 1e-4,
                "â·q·ā must fix the scalar part; q={q:?} got {sandwich:?}"
            );
        }
    }
}
