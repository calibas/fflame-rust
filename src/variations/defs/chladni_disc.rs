//! `chladni_disc` — circular-plate Chladni figure attractor (original).
//!
//! Companion to [`chladni`](super::chladni): same Newton-projection
//! idea, but for a vibrating circular plate / water dish (tonoscope).
//! The mode shape of a circular membrane is
//!
//! ```text
//! f(r, θ) = J_n(k·r) · cos(n·(θ − phase))
//! ```
//!
//! where `J_n` is the Bessel function of the first kind, `n` counts
//! the diametric nodal lines and `k = j_{n,m} / radius` places the
//! m-th radial node ring exactly on the plate rim. A single pure mode
//! is always just spokes crossing rings, so the variation projects
//! onto the nodal set of a **two-mode superposition**
//! `a·mode(n,m) + b·mode(n2,m2)` — the interference between two
//! modes of different symmetry is what bends the nodal set into the
//! wavy flower and star figures seen on real plates (which are driven
//! at frequencies exciting near-degenerate mode pairs). Both modes
//! put a nodal circle on the rim, so the plate boundary stays a node.
//! With `b = 0` you recover the pure figure (`n = 0`: concentric
//! water-dish rings); the field keeps oscillating beyond the rim, so
//! ripple rings continue outward like waves past the plate edge.
//!
//! Bessel evaluation in WGSL: `J0`/`J1` via the standard Numerical
//! Recipes rational approximations, `J_n` via upward recurrence for
//! `x > n` and Miller's normalized downward recurrence otherwise
//! (upward is unstable below the turning point). The derivative uses
//! `J_n'(x) = J_{n−1}(x) − (n/x)·J_n(x)` (and `J_0' = −J_1`). Inputs
//! are always `x = k·r ≥ 0`, so the negative-x odd-order sign flip of
//! the textbook routine is deliberately omitted. The mode zero
//! `j_{n,m}` comes from McMahon's asymptotic expansion — within ~1e-4
//! for every mode in the exposed parameter range, far below visual
//! relevance.
//!
//! No JWildfire/Apophysis equivalent — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static CHLADNI_DISC: VariationDef = VariationDef {
    name: "chladni_disc",
    aliases: &[],
    display_name: "Chladni Disc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("n", "Diametric Lines", int, 3.0, 0.0, 8.0, "Angular mode number of the first mode — its count of straight nodal lines through the center. 0 contributes pure concentric rings."),
        param!("m", "Radial Circles", int, 2.0, 1.0, 8.0, "Radial mode number of the first mode — its count of nodal circles inside the plate (the rim is the m-th)."),
        param!("n2", "Diametric Lines 2", int, 5.0, 0.0, 8.0, "Angular mode number of the second mode. The figure comes from the interference of the two modes — a single pure mode is always just spokes crossing rings; two modes with different symmetry bend the nodal set into the wavy flower and star figures of real plates."),
        param!("m2", "Radial Circles 2", int, 1.0, 1.0, 8.0, "Radial mode number of the second mode."),
        param!("a", "Mix A", float, 1.0, -2.0, 2.0, "Amplitude of the first mode."),
        param!("b", "Mix B", float, 0.6, -2.0, 2.0, "Amplitude of the second mode. 0 disables the interference (pure first mode); the a : b ratio morphs continuously between the two pure figures through every hybrid in between."),
        param!("radius", "Radius", float, 1.0, 0.1, 4.0, "Plate radius: both modes place a nodal circle exactly at this distance, so the rim stays a node while the interior interferes. The ripple field continues beyond it."),
        param!("phase", "Phase", angle, 0.0, "Rotates the first mode's diametric-line pattern, in degrees."),
        param!("phase2", "Phase 2", angle, 0.0, "Rotates the second mode independently — misaligning the two symmetry axes breaks mirror symmetry and gives spiral-flavored figures."),
        param!("steps", "Steps", int, 3.0, 1.0, 6.0, "Newton iterations toward the nodal set per call. 1 is soft and halo-like; 3+ lands points crisply on the figure."),
        param!("strength", "Strength", float, 0.9, 0.0, 1.0, "Blend between the untouched input point (0) and the fully projected point (1)."),
        param!("jitter", "Jitter", float, 0.0, 0.0, 0.2, "Isotropic random offset added after projection — a sand-grain look. 0 keeps the nodal curves razor thin."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// The Bessel helpers + projection loop are duplicated into the 2D and
// 3D strings (only one body is compiled per flame, so nothing is
// defined twice in a shader). Same stability guards as `chladni`:
// epsilon in |∇f|² and a step clamp of half the ring spacing (π/k).

const WGSL_2D: &str = r#"
// J0(x) — Numerical Recipes rational approximation (|err| < 1e-7).
fn chladni_disc_bessj0(x: f32) -> f32 {
    let ax = abs(x);
    if (ax < 8.0) {
        let y = x * x;
        let p1 = -2957821389.0 + y * (7062834065.0 + y * (-512359803.6 + y * (10879881.29 + y * (-86327.92757 + y * 228.4622733))));
        let p2 = 40076544269.0 + y * (745249964.8 + y * (7189466.438 + y * (47447.26470 + y * (226.1030244 + y))));
        return p1 / p2;
    }
    let z = 8.0 / ax;
    let y = z * z;
    let xx = ax - 0.785398164;
    let p1 = 1.0 + y * (-0.1098628627e-2 + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
    let p2 = -0.1562499995e-1 + y * (0.1430488765e-3 + y * (-0.6911147651e-5 + y * (0.7621095161e-6 + y * (-0.934935152e-7))));
    return sqrt(0.636619772 / ax) * (cos(xx) * p1 - z * sin(xx) * p2);
}

// J1(x) for x >= 0 — Numerical Recipes rational approximation.
fn chladni_disc_bessj1(x: f32) -> f32 {
    let ax = abs(x);
    if (ax < 8.0) {
        let y = x * x;
        let p1 = x * (72362614232.0 + y * (-7895059235.0 + y * (242396853.1 + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
        let p2 = 144725228442.0 + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        return p1 / p2;
    }
    let z = 8.0 / ax;
    let y = z * z;
    let xx = ax - 2.356194491;
    let p1 = 1.0 + y * (0.183105e-2 + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
    let p2 = 0.04687499995 + y * (-0.2002690873e-3 + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
    return sqrt(0.636619772 / ax) * (cos(xx) * p1 - z * sin(xx) * p2);
}

// J_n(x) for n >= 0, x >= 0 — upward recurrence above the turning
// point, Miller's normalized downward recurrence below it.
fn chladni_disc_bessjn(n: i32, x: f32) -> f32 {
    if (n == 0) { return chladni_disc_bessj0(x); }
    if (n == 1) { return chladni_disc_bessj1(x); }
    let ax = abs(x);
    if (ax < 1e-8) { return 0.0; }
    let tox = 2.0 / ax;
    var ans: f32 = 0.0;
    if (ax > f32(n)) {
        var bjm = chladni_disc_bessj0(ax);
        var bj = chladni_disc_bessj1(ax);
        for (var j = 1; j < n; j = j + 1) {
            let bjp = f32(j) * tox * bj - bjm;
            bjm = bj;
            bj = bjp;
        }
        ans = bj;
    } else {
        let m_start = 2 * ((n + i32(sqrt(40.0 * f32(n)))) / 2);
        var jsum = false;
        var sum: f32 = 0.0;
        var bjp: f32 = 0.0;
        var bj: f32 = 1.0;
        for (var j = m_start; j > 0; j = j - 1) {
            let bjm = f32(j) * tox * bj - bjp;
            bjp = bj;
            bj = bjm;
            if (abs(bj) > 1.0e10) {
                bj = bj * 1.0e-10;
                bjp = bjp * 1.0e-10;
                ans = ans * 1.0e-10;
                sum = sum * 1.0e-10;
            }
            if (jsum) { sum = sum + bj; }
            jsum = !jsum;
            if (j == n) { ans = bjp; }
        }
        sum = 2.0 * sum - bj;
        ans = ans / sum;
    }
    return ans;
}

// (J_n(x), J_n'(x)) with the x -> 0 guard.
fn chladni_disc_jn_deriv(n: i32, x: f32) -> vec2<f32> {
    let xs = max(x, 1e-6);
    let jn = chladni_disc_bessjn(n, xs);
    var jnp: f32;
    if (n == 0) {
        jnp = -chladni_disc_bessj1(xs);
    } else {
        jnp = chladni_disc_bessjn(n - 1, xs) - f32(n) / xs * jn;
    }
    return vec2<f32>(jn, jnp);
}

// k = j_{n,m} / radius via McMahon's asymptotic expansion (m-th
// positive zero of J_n, ~1e-4 accurate over the exposed range).
fn chladni_disc_mode_k(n: i32, m: f32, radius: f32) -> f32 {
    let mu = 4.0 * f32(n) * f32(n);
    let beta = (m + f32(n) * 0.5 - 0.25) * 3.14159265359;
    let b8 = 8.0 * beta;
    let jnm = beta - (mu - 1.0) / b8 - 4.0 * (mu - 1.0) * (7.0 * mu - 31.0) / (3.0 * b8 * b8 * b8);
    return jnm / radius;
}

// One mode's contribution at (r, θ): vec3(f, ∂f/∂r, ∂f/∂θ) of
// J_n(k r)·cos(n(θ − phase)).
fn chladni_disc_mode(r: f32, theta: f32, n: i32, k: f32, phase: f32) -> vec3<f32> {
    let jd = chladni_disc_jn_deriv(n, k * r);
    let ang = f32(n) * (theta - phase);
    let ca = cos(ang);
    return vec3<f32>(jd.x * ca, k * jd.y * ca, -f32(n) * jd.x * sin(ang));
}

// Newton-project `p` onto the nodal set of the two-mode superposition
// a·mode(n1,k1,ph1) + b·mode(n2,k2,ph2). The interference between the
// modes is what makes the figures — a single pure circular-membrane
// mode is always just spokes crossing rings.
fn chladni_disc_project(p: vec2<f32>, n1: i32, k1: f32, ph1: f32, a: f32, n2: i32, k2: f32, ph2: f32, b: f32, steps: i32) -> vec2<f32> {
    let max_step = 0.5 * 3.14159265359 / max(k1, k2);
    var q = p;
    for (var i = 0; i < steps; i = i + 1) {
        let r = max(length(q), 1e-6);
        let theta = atan2(q.y, q.x);
        let fd = a * chladni_disc_mode(r, theta, n1, k1, ph1)
               + b * chladni_disc_mode(r, theta, n2, k2, ph2);
        let ct = q.x / r;
        let st = q.y / r;
        let g = vec2<f32>(fd.y * ct - fd.z / r * st, fd.y * st + fd.z / r * ct);
        var step = fd.x * g / (dot(g, g) + 1e-6);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }
    return q;
}

fn variation_chladni_disc(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let n1 = i32(get_param(xform_id, variation_id, 0u));
    let m1 = get_param(xform_id, variation_id, 1u);
    let n2 = i32(get_param(xform_id, variation_id, 2u));
    let m2 = get_param(xform_id, variation_id, 3u);
    let a = get_param(xform_id, variation_id, 4u);
    let b = get_param(xform_id, variation_id, 5u);
    let radius = max(get_param(xform_id, variation_id, 6u), 1e-3);
    let ph1 = get_param(xform_id, variation_id, 7u) * 0.01745329252;
    let ph2 = get_param(xform_id, variation_id, 8u) * 0.01745329252;
    let steps = i32(get_param(xform_id, variation_id, 9u));
    let strength = get_param(xform_id, variation_id, 10u);
    let jitter = get_param(xform_id, variation_id, 11u);

    let k1 = chladni_disc_mode_k(n1, m1, radius);
    let k2 = chladni_disc_mode_k(n2, m2, radius);

    let q = chladni_disc_project(p, n1, k1, ph1, a, n2, k2, ph2, b, steps);
    var out = mix(p, q, strength);
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
// J0(x) — Numerical Recipes rational approximation (|err| < 1e-7).
fn chladni_disc_bessj0(x: f32) -> f32 {
    let ax = abs(x);
    if (ax < 8.0) {
        let y = x * x;
        let p1 = -2957821389.0 + y * (7062834065.0 + y * (-512359803.6 + y * (10879881.29 + y * (-86327.92757 + y * 228.4622733))));
        let p2 = 40076544269.0 + y * (745249964.8 + y * (7189466.438 + y * (47447.26470 + y * (226.1030244 + y))));
        return p1 / p2;
    }
    let z = 8.0 / ax;
    let y = z * z;
    let xx = ax - 0.785398164;
    let p1 = 1.0 + y * (-0.1098628627e-2 + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
    let p2 = -0.1562499995e-1 + y * (0.1430488765e-3 + y * (-0.6911147651e-5 + y * (0.7621095161e-6 + y * (-0.934935152e-7))));
    return sqrt(0.636619772 / ax) * (cos(xx) * p1 - z * sin(xx) * p2);
}

// J1(x) for x >= 0 — Numerical Recipes rational approximation.
fn chladni_disc_bessj1(x: f32) -> f32 {
    let ax = abs(x);
    if (ax < 8.0) {
        let y = x * x;
        let p1 = x * (72362614232.0 + y * (-7895059235.0 + y * (242396853.1 + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
        let p2 = 144725228442.0 + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        return p1 / p2;
    }
    let z = 8.0 / ax;
    let y = z * z;
    let xx = ax - 2.356194491;
    let p1 = 1.0 + y * (0.183105e-2 + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
    let p2 = 0.04687499995 + y * (-0.2002690873e-3 + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
    return sqrt(0.636619772 / ax) * (cos(xx) * p1 - z * sin(xx) * p2);
}

// J_n(x) for n >= 0, x >= 0 — upward recurrence above the turning
// point, Miller's normalized downward recurrence below it.
fn chladni_disc_bessjn(n: i32, x: f32) -> f32 {
    if (n == 0) { return chladni_disc_bessj0(x); }
    if (n == 1) { return chladni_disc_bessj1(x); }
    let ax = abs(x);
    if (ax < 1e-8) { return 0.0; }
    let tox = 2.0 / ax;
    var ans: f32 = 0.0;
    if (ax > f32(n)) {
        var bjm = chladni_disc_bessj0(ax);
        var bj = chladni_disc_bessj1(ax);
        for (var j = 1; j < n; j = j + 1) {
            let bjp = f32(j) * tox * bj - bjm;
            bjm = bj;
            bj = bjp;
        }
        ans = bj;
    } else {
        let m_start = 2 * ((n + i32(sqrt(40.0 * f32(n)))) / 2);
        var jsum = false;
        var sum: f32 = 0.0;
        var bjp: f32 = 0.0;
        var bj: f32 = 1.0;
        for (var j = m_start; j > 0; j = j - 1) {
            let bjm = f32(j) * tox * bj - bjp;
            bjp = bj;
            bj = bjm;
            if (abs(bj) > 1.0e10) {
                bj = bj * 1.0e-10;
                bjp = bjp * 1.0e-10;
                ans = ans * 1.0e-10;
                sum = sum * 1.0e-10;
            }
            if (jsum) { sum = sum + bj; }
            jsum = !jsum;
            if (j == n) { ans = bjp; }
        }
        sum = 2.0 * sum - bj;
        ans = ans / sum;
    }
    return ans;
}

// (J_n(x), J_n'(x)) with the x -> 0 guard.
fn chladni_disc_jn_deriv(n: i32, x: f32) -> vec2<f32> {
    let xs = max(x, 1e-6);
    let jn = chladni_disc_bessjn(n, xs);
    var jnp: f32;
    if (n == 0) {
        jnp = -chladni_disc_bessj1(xs);
    } else {
        jnp = chladni_disc_bessjn(n - 1, xs) - f32(n) / xs * jn;
    }
    return vec2<f32>(jn, jnp);
}

// k = j_{n,m} / radius via McMahon's asymptotic expansion (m-th
// positive zero of J_n, ~1e-4 accurate over the exposed range).
fn chladni_disc_mode_k(n: i32, m: f32, radius: f32) -> f32 {
    let mu = 4.0 * f32(n) * f32(n);
    let beta = (m + f32(n) * 0.5 - 0.25) * 3.14159265359;
    let b8 = 8.0 * beta;
    let jnm = beta - (mu - 1.0) / b8 - 4.0 * (mu - 1.0) * (7.0 * mu - 31.0) / (3.0 * b8 * b8 * b8);
    return jnm / radius;
}

// One mode's contribution at (r, θ): vec3(f, ∂f/∂r, ∂f/∂θ) of
// J_n(k r)·cos(n(θ − phase)).
fn chladni_disc_mode(r: f32, theta: f32, n: i32, k: f32, phase: f32) -> vec3<f32> {
    let jd = chladni_disc_jn_deriv(n, k * r);
    let ang = f32(n) * (theta - phase);
    let ca = cos(ang);
    return vec3<f32>(jd.x * ca, k * jd.y * ca, -f32(n) * jd.x * sin(ang));
}

// Newton-project `p` onto the nodal set of the two-mode superposition
// a·mode(n1,k1,ph1) + b·mode(n2,k2,ph2). The interference between the
// modes is what makes the figures — a single pure circular-membrane
// mode is always just spokes crossing rings.
fn chladni_disc_project(p: vec2<f32>, n1: i32, k1: f32, ph1: f32, a: f32, n2: i32, k2: f32, ph2: f32, b: f32, steps: i32) -> vec2<f32> {
    let max_step = 0.5 * 3.14159265359 / max(k1, k2);
    var q = p;
    for (var i = 0; i < steps; i = i + 1) {
        let r = max(length(q), 1e-6);
        let theta = atan2(q.y, q.x);
        let fd = a * chladni_disc_mode(r, theta, n1, k1, ph1)
               + b * chladni_disc_mode(r, theta, n2, k2, ph2);
        let ct = q.x / r;
        let st = q.y / r;
        let g = vec2<f32>(fd.y * ct - fd.z / r * st, fd.y * st + fd.z / r * ct);
        var step = fd.x * g / (dot(g, g) + 1e-6);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }
    return q;
}

fn variation_chladni_disc(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let n1 = i32(get_param(xform_id, variation_id, 0u));
    let m1 = get_param(xform_id, variation_id, 1u);
    let n2 = i32(get_param(xform_id, variation_id, 2u));
    let m2 = get_param(xform_id, variation_id, 3u);
    let a = get_param(xform_id, variation_id, 4u);
    let b = get_param(xform_id, variation_id, 5u);
    let radius = max(get_param(xform_id, variation_id, 6u), 1e-3);
    let ph1 = get_param(xform_id, variation_id, 7u) * 0.01745329252;
    let ph2 = get_param(xform_id, variation_id, 8u) * 0.01745329252;
    let steps = i32(get_param(xform_id, variation_id, 9u));
    let strength = get_param(xform_id, variation_id, 10u);
    let jitter = get_param(xform_id, variation_id, 11u);

    let k1 = chladni_disc_mode_k(n1, m1, radius);
    let k2 = chladni_disc_mode_k(n2, m2, radius);

    let q = chladni_disc_project(p.xy, n1, k1, ph1, a, n2, k2, ph2, b, steps);
    var out = mix(p.xy, q, strength);
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return vec3<f32>(out, p.z);
}
"#;
