//! waves2b (dark-beam, 2014)
//!
//! Generalization of waves2: per-axis sine perturbation with three
//! mode selectors based on the `pwx` / `pwy` parameter:
//!   - `pwx > 1e-4 OR pwx < -1e-4 (general)`: power-mode sine
//!     `sin(sgn(c) · |c|^pw · freq)`
//!   - `pwx ∈ [0, 1e-4)`: Jacobi `sn` mode
//!   - `pwx ∈ (-1e-4, 0)`: Bessel `J1` mode
//!
//! 10 user params (freqx, freqy, pwx, pwy, scalex, scaleinfx, scaley,
//! scaleinfy, unity, jacok) + 2 init slots
//! (_six = scalex - scaleinfx, _siy = scaley - scaleinfy).
//!
//! The Jacobi sn helper (`w2b_jac_sn`) is the same 8-iteration AGM
//! descent used in batch 65 (`jacobi_elliptic.rs`), prefixed `w2b_`.
//!
//! Bessel J1 (`w2b_bessel_j1`) implemented via Abramowitz & Stegun
//! 9.4.4 / 9.4.6 polynomial approximations:
//!   - |x| ≤ 3: 7-term Taylor-like polynomial in (x/3)²
//!   - |x| > 3: asymptotic form `f₁/√x · cos(θ₁)` with f₁ and θ₁
//!     polynomials in (3/x). Sign-preserving via `J1(-x) = -J1(x)`.
//!
//! Body factors cleanly through outer multiplier (cpp uses VVAR
//! consistently on every output line). Z passes through.
//!
//! Source: `output/jwildfire-vars/output/waves2b.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

const W2B_HELPERS: &str = r#"
// Bessel J1 via A&S 9.4.4 / 9.4.6 (max error ~1e-7)
fn w2b_bessel_j1(x: f32) -> f32 {
    let ax = abs(x);
    let s = sign(x);
    if (ax < 1e-30) {
        return 0.0;
    }
    if (ax <= 3.0) {
        let y = (x / 3.0) * (x / 3.0);
        let p = 0.5 - 0.56249985 * y + 0.21093573 * y * y
              - 0.03954289 * y * y * y + 0.00443319 * y * y * y * y
              - 0.00031761 * y * y * y * y * y + 0.00001109 * y * y * y * y * y * y;
        return x * p;
    }
    let z = 3.0 / ax;
    let f1 = 0.79788456 + 0.00000156 * z + 0.01659667 * z * z + 0.00017105 * z * z * z
           - 0.00249511 * z * z * z * z + 0.00113653 * z * z * z * z * z
           - 0.00020033 * z * z * z * z * z * z;
    let theta1 = ax - 2.35619449 + 0.12499612 * z + 0.00005650 * z * z
               - 0.00637879 * z * z * z + 0.00074348 * z * z * z * z
               + 0.00079824 * z * z * z * z * z - 0.00029166 * z * z * z * z * z * z;
    return s * f1 / sqrt(ax) * cos(theta1);
}

// Jacobi sn — 8-iter AGM descent (less precise than 13-iter but fits
// per-iteration GPU budget; ~1e-7 precision at default emmc range).
fn w2b_jac_sn(uu: f32, emmc_in: f32) -> f32 {
    let ca = 0.0003;
    var emc = emmc_in;
    var u = uu;
    if (abs(emc) < 1e-30) {
        return tanh(u);
    }
    var d: f32 = 1.0;
    var bo = false;
    if (emc < 0.0) {
        bo = true;
        d = 1.0 - emc;
        emc = -emc / d;
        d = sqrt(d);
        u = d * u;
    }
    var a: f32 = 1.0;
    var c: f32 = 0.0;
    var em: array<f32, 8>;
    var en: array<f32, 8>;
    var l = 0;
    for (var i = 0; i < 8; i = i + 1) {
        l = i;
        em[i] = a;
        emc = sqrt(emc);
        en[i] = emc;
        c = 0.5 * (a + emc);
        if (abs(a - emc) <= ca * a) {
            break;
        }
        emc = a * emc;
        a = c;
    }
    u = c * u;
    var sn = sin(u);
    let cn_init = cos(u);
    var dn: f32 = 1.0;
    if (abs(sn) > 1e-30) {
        a = cn_init / sn;
        c = a * c;
        for (var ii = l; ii >= 0; ii = ii - 1) {
            let b = em[ii];
            a = c * a;
            c = dn * c;
            dn = (en[ii] + a) / (b + a);
            a = c / b;
        }
        let inv = 1.0 / sqrt(c * c + 1.0);
        sn = select(inv, -inv, sn < 0.0);
    }
    if (bo) {
        sn = sn / d;
    }
    return sn;
}
"#;

/// Per-axis sine-perturbation displacement (generalization of `waves2`) —
/// adds a wave term to each axis driven by the *other* axis's coordinate.
/// The wave function is chosen by the per-axis power param: `pw > 1e-4` or
/// `pw < -1e-4` → power-mode sine `sin(sgn(c)·|c|^pw·freq)`; `pw ∈ [0,
/// 1e-4)` → Jacobi `sn` (with modulus `jacok`); `pw ∈ (-1e-4, 0)` → Bessel
/// `J1`. Per-axis amplitude is a smooth interpolation between `scale*` (at
/// origin) and `scaleinf*` (at infinity), controlled by `unity`.
///
/// # Authors
/// - DarkBeam
pub static WAVES2B: VariationDef = VariationDef {
    name: "waves2b",
    display_name: "Waves2 B",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("freqx", "Freq X", unlimited_float, 2.0, -10.0, 10.0, "Frequency of the wave applied to the X axis (driven by Y)."),
        param!("freqy", "Freq Y", unlimited_float, 2.0, -10.0, 10.0, "Frequency of the wave applied to the Y axis (driven by X)."),
        param!("pwx", "Power X", unlimited_float, 1.0, -10.0, 10.0, "X-axis power exponent — also selects the wave function. `> 1e-4` or `< -1e-4` = power-mode sine of `|c|^pw`; `[0, 1e-4)` = Jacobi `sn`; `(-1e-4, 0)` = Bessel `J1`."),
        param!("pwy", "Power Y", unlimited_float, 1.0, -10.0, 10.0, "Y-axis power exponent — same threshold-based mode selector as `pwx`."),
        param!("scalex", "Scale X", unlimited_float, 1.0, -10.0, 10.0, "X-axis amplitude at the origin (full damping)."),
        param!("scaleinfx", "Scale Inf X", unlimited_float, 1.0, -10.0, 10.0, "X-axis amplitude at infinity (the limiting amplitude as `x → ±∞`)."),
        param!("scaley", "Scale Y", unlimited_float, 1.0, -10.0, 10.0, "Y-axis amplitude at the origin."),
        param!("scaleinfy", "Scale Inf Y", unlimited_float, 1.0, -10.0, 10.0, "Y-axis amplitude at infinity."),
        param!("unity", "Unity", unlimited_float, 1.0, -10.0, 10.0, "Crossover scale for the amplitude interpolation between `scale*` and `scaleinf*`. Larger values keep the origin-amplitude active over a wider region."),
        param!("jacok", "Jacobi K", unlimited_float, 0.25, -10.0, 10.0, "Modulus `k` for the Jacobi `sn` mode (only used when `pwx` or `pwy` falls in `[0, 1e-4)`)."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_waves2b(user: array<f32, 10>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = user[4] - user[5];  // _six
    out[1] = user[6] - user[7];  // _siy
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn w2b_bessel_j1(x: f32) -> f32 {
    let ax = abs(x);
    let s = sign(x);
    if (ax < 1e-30) {
        return 0.0;
    }
    if (ax <= 3.0) {
        let y = (x / 3.0) * (x / 3.0);
        let p = 0.5 - 0.56249985 * y + 0.21093573 * y * y
              - 0.03954289 * y * y * y + 0.00443319 * y * y * y * y
              - 0.00031761 * y * y * y * y * y + 0.00001109 * y * y * y * y * y * y;
        return x * p;
    }
    let z = 3.0 / ax;
    let f1 = 0.79788456 + 0.00000156 * z + 0.01659667 * z * z + 0.00017105 * z * z * z
           - 0.00249511 * z * z * z * z + 0.00113653 * z * z * z * z * z
           - 0.00020033 * z * z * z * z * z * z;
    let theta1 = ax - 2.35619449 + 0.12499612 * z + 0.00005650 * z * z
               - 0.00637879 * z * z * z + 0.00074348 * z * z * z * z
               + 0.00079824 * z * z * z * z * z - 0.00029166 * z * z * z * z * z * z;
    return s * f1 / sqrt(ax) * cos(theta1);
}

fn w2b_jac_sn(uu: f32, emmc_in: f32) -> f32 {
    let ca = 0.0003;
    var emc = emmc_in;
    var u = uu;
    if (abs(emc) < 1e-30) {
        return tanh(u);
    }
    var d: f32 = 1.0;
    var bo = false;
    if (emc < 0.0) {
        bo = true;
        d = 1.0 - emc;
        emc = -emc / d;
        d = sqrt(d);
        u = d * u;
    }
    var a: f32 = 1.0;
    var c: f32 = 0.0;
    var em: array<f32, 8>;
    var en: array<f32, 8>;
    var l = 0;
    for (var i = 0; i < 8; i = i + 1) {
        l = i;
        em[i] = a;
        emc = sqrt(max(emc, 0.0));
        en[i] = emc;
        c = 0.5 * (a + emc);
        if (abs(a - emc) <= ca * a) {
            break;
        }
        emc = a * emc;
        a = c;
    }
    u = c * u;
    var sn = sin(u);
    let cn_init = cos(u);
    var dn: f32 = 1.0;
    if (abs(sn) > 1e-30) {
        a = cn_init / sn;
        c = a * c;
        for (var ii = l; ii >= 0; ii = ii - 1) {
            let b = em[ii];
            a = c * a;
            c = dn * c;
            dn = (en[ii] + a) / (b + a);
            a = c / b;
        }
        let inv = 1.0 / sqrt(c * c + 1.0);
        sn = select(inv, -inv, sn < 0.0);
    }
    if (bo) {
        sn = sn / d;
    }
    return sn;
}

fn w2b_safediv(q: f32, r: f32) -> f32 {
    if (r < 1e-10) {
        return 1.0 / max(r, 1e-30);
    }
    return q / r;
}

fn w2b_axis_term(c_in: f32, c_other: f32, freq: f32, pw: f32, jacok: f32, csx: f32) -> f32 {
    if (pw >= 0.0 && pw < 1e-4) {
        return csx * w2b_jac_sn(c_other * freq, jacok);
    }
    if (pw < 0.0 && pw > -1e-4) {
        return csx * w2b_bessel_j1(c_other * freq);
    }
    let s = select(1.0, -1.0, c_other < 0.0);
    return csx * sin(s * pow(abs(c_other) + 1e-10, pw) * freq);
}

fn variation_waves2b(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let freqx = get_param(xform_id, variation_id, 0u);
    let freqy = get_param(xform_id, variation_id, 1u);
    let pwx = get_param(xform_id, variation_id, 2u);
    let pwy = get_param(xform_id, variation_id, 3u);
    let scaleinfx = get_param(xform_id, variation_id, 5u);
    let scaleinfy = get_param(xform_id, variation_id, 7u);
    let unity = get_param(xform_id, variation_id, 8u);
    let jacok = get_param(xform_id, variation_id, 9u);
    let six = get_param(xform_id, variation_id, 10u);
    let siy = get_param(xform_id, variation_id, 11u);

    let csx = w2b_safediv(unity, unity + p.x * p.x) * six + scaleinfx;
    let csy = w2b_safediv(unity, unity + p.y * p.y) * siy + scaleinfy;
    let nx = p.x + w2b_axis_term(p.x, p.y, freqx, pwx, jacok, csx);
    let ny = p.y + w2b_axis_term(p.y, p.x, freqy, pwy, jacok, csy);
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn w2b_bessel_j1(x: f32) -> f32 {
    let ax = abs(x);
    let s = sign(x);
    if (ax < 1e-30) {
        return 0.0;
    }
    if (ax <= 3.0) {
        let y = (x / 3.0) * (x / 3.0);
        let p = 0.5 - 0.56249985 * y + 0.21093573 * y * y
              - 0.03954289 * y * y * y + 0.00443319 * y * y * y * y
              - 0.00031761 * y * y * y * y * y + 0.00001109 * y * y * y * y * y * y;
        return x * p;
    }
    let z = 3.0 / ax;
    let f1 = 0.79788456 + 0.00000156 * z + 0.01659667 * z * z + 0.00017105 * z * z * z
           - 0.00249511 * z * z * z * z + 0.00113653 * z * z * z * z * z
           - 0.00020033 * z * z * z * z * z * z;
    let theta1 = ax - 2.35619449 + 0.12499612 * z + 0.00005650 * z * z
               - 0.00637879 * z * z * z + 0.00074348 * z * z * z * z
               + 0.00079824 * z * z * z * z * z - 0.00029166 * z * z * z * z * z * z;
    return s * f1 / sqrt(ax) * cos(theta1);
}

fn w2b_jac_sn(uu: f32, emmc_in: f32) -> f32 {
    let ca = 0.0003;
    var emc = emmc_in;
    var u = uu;
    if (abs(emc) < 1e-30) {
        return tanh(u);
    }
    var d: f32 = 1.0;
    var bo = false;
    if (emc < 0.0) {
        bo = true;
        d = 1.0 - emc;
        emc = -emc / d;
        d = sqrt(d);
        u = d * u;
    }
    var a: f32 = 1.0;
    var c: f32 = 0.0;
    var em: array<f32, 8>;
    var en: array<f32, 8>;
    var l = 0;
    for (var i = 0; i < 8; i = i + 1) {
        l = i;
        em[i] = a;
        emc = sqrt(max(emc, 0.0));
        en[i] = emc;
        c = 0.5 * (a + emc);
        if (abs(a - emc) <= ca * a) {
            break;
        }
        emc = a * emc;
        a = c;
    }
    u = c * u;
    var sn = sin(u);
    let cn_init = cos(u);
    var dn: f32 = 1.0;
    if (abs(sn) > 1e-30) {
        a = cn_init / sn;
        c = a * c;
        for (var ii = l; ii >= 0; ii = ii - 1) {
            let b = em[ii];
            a = c * a;
            c = dn * c;
            dn = (en[ii] + a) / (b + a);
            a = c / b;
        }
        let inv = 1.0 / sqrt(c * c + 1.0);
        sn = select(inv, -inv, sn < 0.0);
    }
    if (bo) {
        sn = sn / d;
    }
    return sn;
}

fn w2b_safediv(q: f32, r: f32) -> f32 {
    if (r < 1e-10) {
        return 1.0 / max(r, 1e-30);
    }
    return q / r;
}

fn w2b_axis_term(c_in: f32, c_other: f32, freq: f32, pw: f32, jacok: f32, csx: f32) -> f32 {
    if (pw >= 0.0 && pw < 1e-4) {
        return csx * w2b_jac_sn(c_other * freq, jacok);
    }
    if (pw < 0.0 && pw > -1e-4) {
        return csx * w2b_bessel_j1(c_other * freq);
    }
    let s = select(1.0, -1.0, c_other < 0.0);
    return csx * sin(s * pow(abs(c_other) + 1e-10, pw) * freq);
}

fn variation_waves2b(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let freqx = get_param(xform_id, variation_id, 0u);
    let freqy = get_param(xform_id, variation_id, 1u);
    let pwx = get_param(xform_id, variation_id, 2u);
    let pwy = get_param(xform_id, variation_id, 3u);
    let scaleinfx = get_param(xform_id, variation_id, 5u);
    let scaleinfy = get_param(xform_id, variation_id, 7u);
    let unity = get_param(xform_id, variation_id, 8u);
    let jacok = get_param(xform_id, variation_id, 9u);
    let six = get_param(xform_id, variation_id, 10u);
    let siy = get_param(xform_id, variation_id, 11u);

    let csx = w2b_safediv(unity, unity + p.x * p.x) * six + scaleinfx;
    let csy = w2b_safediv(unity, unity + p.y * p.y) * siy + scaleinfy;
    let nx = p.x + w2b_axis_term(p.x, p.y, freqx, pwx, jacok, csx);
    let ny = p.y + w2b_axis_term(p.y, p.x, freqy, pwy, jacok, csy);
    return vec3<f32>(nx, ny, p.z);
}
"#),
};

#[allow(dead_code)]
const _UNUSED: &str = W2B_HELPERS;
