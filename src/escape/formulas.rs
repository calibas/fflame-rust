//! Formula definitions — the iterated step `z ← f(z, c)`.
//!
//! One `static FormulaDef` per formula, WGSL inline, mirroring
//! `src/variations/defs/*`. Registered in `super::FORMULAS`
//! (append-only). This is the phase-1 set from the plan's catalog
//! (docs/projects/escape-time-fractals.md §5): Mandelbrot/Multibrot,
//! Tricorn/Multicorn, the Burning Ship family, McMullen, Kaliset.
//!
//! Complex helpers `esc_cmul(a, b)` and `esc_cpow(z, p)` come from the
//! assembler template. `esc_cpow` returns 0 at the origin (the correct
//! limit for p > 0) so the polar `atan2` is never evaluated at a zero
//! pair — the Metal fast-math hazard documented in CLAUDE.md.

use super::{EscapeMetric, EscapeParamDef, FormulaDef, FormulaFeature};


/// The classic quadratic map `z ← z² + c` (plan §5.1).
///
/// No parameters: Multibrot (arbitrary power) is a separate def so
/// that the common case compiles the two-multiply special form rather
/// than a polar pow, and so the panel doesn't show a power slider on
/// the formula everyone starts with.
pub static MANDELBROT: FormulaDef = FormulaDef {
    name: "mandelbrot",
    display_name: "Mandelbrot",
    features: &[],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    let d = 2.0 * esc_cmul(z, dz);
    return select(d + vec2<f32>(1.0, 0.0), d, is_julia);
}
"#,
};

/// Multibrot `z ← zᵖ + c` (plan §5.1). Non-integer powers render too
/// (the polar form doesn't care); integer powers are the classic sets.
pub static MULTIBROT: FormulaDef = FormulaDef {
    name: "multibrot",
    display_name: "Multibrot",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "power",
        display_name: "Power",
        default: 3.0,
        min: 2.0,
        max: 12.0,
        tooltip: "Exponent p in z^p + c. 2 is the Mandelbrot set; higher powers grow more lobes.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return esc_cpow(z, fparam(0u)) + c;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    let p = fparam(0u);
    let d = p * esc_cmul(esc_cpow(z, p - 1.0), dz);
    return select(d + vec2<f32>(1.0, 0.0), d, is_julia);
}
"#,
};

/// Tricorn / Multicorn `z ← z̄ᵖ + c` (plan §5.2): conjugate first,
/// then the Multibrot step. p = 2 is the Tricorn (Mandelbar).
pub static TRICORN: FormulaDef = FormulaDef {
    name: "tricorn",
    display_name: "Tricorn",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "power",
        display_name: "Power",
        default: 2.0,
        min: 2.0,
        max: 12.0,
        tooltip: "Exponent p in conj(z)^p + c. 2 is the Tricorn; higher powers are the multicorns.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return esc_cpow(vec2<f32>(z.x, -z.y), fparam(0u)) + c;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // conj(z)^p is ANTI-holomorphic: the chain rule carries the
    // conjugate of dz, not dz. (So this is the standard
    // anti-holomorphic distance estimate, not a complex derivative --
    // |dz| is still the growth factor the estimate needs.)
    let p = fparam(0u);
    let zc = vec2<f32>(z.x, -z.y);
    let dzc = vec2<f32>(dz.x, -dz.y);
    let d = p * esc_cmul(esc_cpow(zc, p - 1.0), dzc);
    return select(d + vec2<f32>(1.0, 0.0), d, is_julia);
}
"#,
};

/// Burning Ship family (plan §5.3): one formula, a `variant` enum of
/// component-fold placements around the quadratic step.
///
/// Variant table follows the widely-reproduced Fractal Forums
/// perpendicular-family chart (Burning Ship, Perpendicular
/// Mandelbrot/Ship, Celtic, Buffalo, Perpendicular Celtic). The plan's
/// standing instruction to pin conventions against reference images
/// applies — these match the common chart, and the visual corpus
/// baselines are the pin.
///
/// Note the classic "ship" appears inverted in our Im-up viewport (the
/// famous renders use Im-down); the view `rotation` handles taste.
pub static BURNING_SHIP: FormulaDef = FormulaDef {
    name: "burning_ship",
    display_name: "Burning Ship",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "variant",
        display_name: "Variant",
        default: 0.0,
        min: 0.0,
        max: 5.0,
        tooltip: "0 Burning Ship, 1 Perpendicular Mandelbrot, 2 Perpendicular Ship, 3 Celtic, 4 Buffalo, 5 Perpendicular Celtic.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let v = u32(clamp(fparam(0u), 0.0, 5.0));
    let x = z.x;
    let y = z.y;
    var re: f32;
    var im: f32;
    switch v {
        case 0u: { // Burning Ship: fold both components before squaring
            let ax = abs(x);
            let ay = abs(y);
            re = ax * ax - ay * ay;
            im = 2.0 * ax * ay;
        }
        case 1u: { // Perpendicular Mandelbrot
            re = x * x - y * y;
            im = -2.0 * abs(x) * y;
        }
        case 2u: { // Perpendicular Burning Ship
            re = x * x - y * y;
            im = -2.0 * x * abs(y);
        }
        case 3u: { // Celtic
            re = abs(x * x - y * y);
            im = 2.0 * x * y;
        }
        case 4u: { // Buffalo
            re = abs(x * x - y * y);
            im = -2.0 * abs(x * y);
        }
        default: { // 5: Perpendicular Celtic
            re = abs(x * x - y * y);
            im = -2.0 * abs(x) * y;
        }
    }
    return vec2<f32>(re, im) + c;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// McMullen family `z ← zⁿ + c/zᵐ` (plan §5.4) — rational maps with
/// Sierpiński-carpet Julia sets.
///
/// The origin is a pole: a near-zero z is treated as escaped (a huge
/// step output trips the bailout immediately), matching the plan's
/// guard note. seeded from the pixel because the critical point 0 IS the
/// pole — the classic carpet pictures are Julia mode; the pixel-seeded
/// parameter plane is the exploratory map until the proper
/// critical-orbit seed lands with the perturbation work.
pub static MCMULLEN: FormulaDef = FormulaDef {
    name: "mcmullen",
    display_name: "McMullen",
    features: &[],
    parameters: &[
        EscapeParamDef {
            name: "n",
            display_name: "n (numerator power)",
            default: 2.0,
            min: 2.0,
            max: 8.0,
            tooltip: "Power of the polynomial term z^n.",
        },
        EscapeParamDef {
            name: "m",
            display_name: "m (pole power)",
            default: 3.0,
            min: 1.0,
            max: 8.0,
            tooltip: "Power of the pole term c / z^m. n=2, m=3 is the classic Sierpinski-carpet family.",
        },
    ],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let r2 = dot(z, z);
    if (r2 < 1e-20) {
        // Pole: treat as escaped (huge output trips the bailout).
        return vec2<f32>(1e20, 0.0);
    }
    return esc_cpow(z, fparam(0u)) + esc_cmul(c, esc_cpow(z, -fparam(1u)));
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // d/dz [z^n + c z^-m] = n z^(n-1) - m c z^(-m-1);  d/dc = z^-m.
    let n = fparam(0u);
    let m = fparam(1u);
    if (dot(z, z) < 1e-20) {
        return vec2<f32>(0.0, 0.0);   // at the pole the estimate is void
    }
    let dfz = n * esc_cpow(z, n - 1.0)
            - m * esc_cmul(c, esc_cpow(z, -m - 1.0));
    let d = esc_cmul(dfz, dz);
    return select(d + esc_cpow(z, -m), d, is_julia);
}
"#,
};

/// Kaliset `z ← |z| / ⟨z,z⟩ − c` (component abs; plan §5.12).
///
/// `NonEscaping` (no bailout), seeded from the pixel — the loop runs the full `max_iter`
/// (classic renders use ~50–200) and ONLY the orbit-average colorings
/// show anything (escape-based colorings render black by design; the
/// panel pairing is the user's choice, per the registry-orthogonality
/// principle). Sign convention: minus, the common Kali formula; the
/// `plus_c` toggle covers the other branch. Convention pinned against
/// reference images via the visual corpus (plan §11.3).
pub static KALISET: FormulaDef = FormulaDef {
    name: "kaliset",
    display_name: "Kaliset",
    features: &[FormulaFeature::NonEscaping],
    parameters: &[EscapeParamDef {
        name: "plus_c",
        display_name: "Add c (instead of subtract)",
        default: 0.0,
        min: 0.0,
        max: 1.0,
        tooltip: "0: z <- |z|/<z,z> - c (classic). 1: the + c branch.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let r2 = dot(z, z);
    // 0/0 guard: from exactly zero, the fold stays at zero and only
    // the c term acts (deterministic, no NaN enters the orbit).
    var folded = vec2<f32>(0.0, 0.0);
    if (r2 > 1e-30) {
        folded = abs(z) / r2;
    }
    let sign_c = select(-1.0, 1.0, fparam(0u) > 0.5);
    return folded + sign_c * c;
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Phoenix — `z ← z² + c + p·z_prev` (plan §5.6). The first
/// `NeedsPrevZ` formula: the loop carries a one-iterate history
/// register. Classic gallery view: Julia mode, `c = 0.5667`,
/// `p = −0.5`.
pub static PHOENIX: FormulaDef = FormulaDef {
    name: "phoenix",
    display_name: "Phoenix",
    features: &[FormulaFeature::NeedsPrevZ],
    parameters: &[
        EscapeParamDef {
            name: "p_re",
            display_name: "p (re)",
            default: -0.5,
            min: -2.0,
            max: 2.0,
            tooltip: "Real part of the previous-iterate coefficient. The classic Phoenix Julia uses c = 0.5667, p = -0.5.",
        },
        EscapeParamDef {
            name: "p_im",
            display_name: "p (im)",
            default: 0.0,
            min: -2.0,
            max: 2.0,
            tooltip: "Imaginary part of the previous-iterate coefficient.",
        },
    ],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>, z_prev: vec2<f32>) -> vec2<f32> {
    let p = vec2<f32>(fparam(0u), fparam(1u));
    return vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c + esc_cmul(p, z_prev);
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Lambda / logistic plane — `z ← λ·z·(1−z)` (plan §5.5).
/// Conformally conjugate to Mandelbrot, but the λ-plane layout is its
/// own classic. Pixel = λ (the map's `c` slot); the critical point of
/// the logistic map is z = 1/2, so the parameter plane seeds there.
pub static LAMBDA: FormulaDef = FormulaDef {
    name: "lambda",
    display_name: "Lambda (Logistic)",
    features: &[],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return esc_cmul(c, esc_cmul(z, vec2<f32>(1.0, 0.0) - z));
}
"#,
    wgsl_param_seed: "vec2<f32>(0.5, 0.0)",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    let d = esc_cmul(c, esc_cmul(vec2<f32>(1.0, 0.0) - 2.0 * z, dz));
    let fc = esc_cmul(z, vec2<f32>(1.0, 0.0) - z);
    return select(d + fc, d, is_julia);
}
"#,
};

/// Fractint Spider — `z ← z² + c; c ← c/2 + z` (plan §5.16). The
/// first `MutatesC` formula: c drifts toward the orbit, half-life one
/// iteration. Per Fractint's fractals.doc the c update uses the NEW z.
pub static SPIDER: FormulaDef = FormulaDef {
    name: "spider",
    display_name: "Spider",
    features: &[FormulaFeature::MutatesC],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: ptr<function, vec2<f32>>) -> vec2<f32> {
    let cc = *c;
    let z_new = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + cc;
    *c = cc * 0.5 + z_new;
    return z_new;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // f = c z (1 - z):  df/dz = c (1 - 2z),  df/dc = z (1 - z).
    let one = vec2<f32>(1.0, 0.0);
    let d = esc_cmul(esc_cmul(c, one - 2.0 * z), dz);
    return select(d + esc_cmul(z, one - z), d, is_julia);
}
"#,
};

/// Fractint Manowar — `z ← z² + m + c; m ← z(old)` (plan §5.16),
/// i.e. Phoenix with p = 1 but with the auxiliary seeded at z₀
/// rather than 0 (Fractint starts z = m = pixel).
pub static MANOWAR: FormulaDef = FormulaDef {
    name: "manowar",
    display_name: "Manowar",
    features: &[FormulaFeature::NeedsPrevZ],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>, z_prev: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + z_prev + c;
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "z",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Fractint Barnsley M1–M3 — conditional affine/quadratic maps
/// (plan §5.16: "escape-time renderings of IFS-like maps, ancestors
/// of mode C"). One formula, `variant` enum, per fractals.doc:
///   M1: z ← (z∓1)·c by sign of Re z
///   M2: z ← (z∓1)·c by sign of Re(z)·Im(c) + Re(c)·Im(z)
///   M3: z ← z²−1 (+ c·Re z on the Re z ≤ 0 branch)
/// Convention pinned by the visual corpus (Feather policy).
pub static BARNSLEY: FormulaDef = FormulaDef {
    name: "barnsley",
    display_name: "Barnsley",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "variant",
        display_name: "Variant",
        default: 0.0,
        min: 0.0,
        max: 2.0,
        tooltip: "0: M1 (fold on Re z), 1: M2 (fold on a bilinear test), 2: M3 (quadratic with conditional c term).",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let v = u32(clamp(fparam(0u), 0.0, 2.0));
    switch v {
        case 0u: {
            if (z.x >= 0.0) {
                return esc_cmul(z - vec2<f32>(1.0, 0.0), c);
            }
            return esc_cmul(z + vec2<f32>(1.0, 0.0), c);
        }
        case 1u: {
            if (z.x * c.y + c.x * z.y >= 0.0) {
                return esc_cmul(z - vec2<f32>(1.0, 0.0), c);
            }
            return esc_cmul(z + vec2<f32>(1.0, 0.0), c);
        }
        default: {
            let sq = vec2<f32>(z.x * z.x - z.y * z.y - 1.0, 2.0 * z.x * z.y);
            if (z.x > 0.0) {
                return sq;
            }
            return sq + c * z.x;
        }
    }
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // Piecewise-linear in z, so the derivative follows the SAME branch
    // the step took (the branch boundary itself is measure-zero).
    let one = vec2<f32>(1.0, 0.0);
    let v = u32(clamp(fparam(0u), 0.0, 2.0));
    if (v == 2u) {
        // Type 3 is z^2 - 1, plus c*Re(z) on the left half-plane.
        // Re(z) is NOT holomorphic, so that branch has no complex
        // derivative; we take the real-direction one (d Re(z)/dz -> 1),
        // which is the usual choice for distance estimation on
        // fold-type maps and is exact wherever the branch is inactive.
        var dfz = 2.0 * z;
        var dfc = vec2<f32>(0.0, 0.0);
        if (z.x <= 0.0) {
            dfz = dfz + c;
            dfc = vec2<f32>(z.x, 0.0);
        }
        let d = esc_cmul(dfz, dz);
        return select(d + dfc, d, is_julia);
    }
    // Types 1 and 2: f = (z -/+ 1) * c, so df/dz = c and df/dc = z -/+ 1.
    var shifted = z - one;
    if (v == 0u) {
        if (z.x < 0.0) { shifted = z + one; }
    } else {
        if (z.x * c.y + c.x * z.y < 0.0) { shifted = z + one; }
    }
    let d = esc_cmul(c, dz);
    return select(d + shifted, d, is_julia);
}
"#,
};

/// Fractint Cactus — `z ← z³ + (c−1)·z − c` (plan §5.16). Fractint
/// seeds z₀ = pixel on the parameter plane.
pub static CACTUS: FormulaDef = FormulaDef {
    name: "cactus",
    display_name: "Cactus",
    features: &[],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let z2 = esc_cmul(z, z);
    let z3 = esc_cmul(z2, z);
    return z3 + esc_cmul(c - vec2<f32>(1.0, 0.0), z) - c;
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // f = z^3 + (c-1) z - c:  df/dz = 3z^2 + (c-1),  df/dc = z - 1.
    let one = vec2<f32>(1.0, 0.0);
    let dfz = 3.0 * esc_cmul(z, z) + (c - one);
    let d = esc_cmul(dfz, dz);
    return select(d + (z - one), d, is_julia);
}
"#,
};

/// Exponential map `z ← e^z + c` (plan §5.9): Cantor bouquets.
/// |e^z| = e^(Re z), so escape tests Re z RAW — set bailout ~50.
/// esc_cexp clamps Re before exp (overflow guard).
pub static EXPONENTIAL: FormulaDef = FormulaDef {
    name: "exponential",
    display_name: "Exponential",
    features: &[],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return esc_cexp(z) + c;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::Re,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // f = e^z + c:  df/dz = e^z,  df/dc = 1.
    let d = esc_cmul(esc_cexp(z), dz);
    return select(d + vec2<f32>(1.0, 0.0), d, is_julia);
}
"#,
};

/// Trig family `z ← sin z + c` / `cos z + c` (plan §5.9). Escape
/// tests |Im z| RAW (sinh/cosh growth) — set bailout ~50.
pub static TRIG: FormulaDef = FormulaDef {
    name: "trig",
    display_name: "Sine / Cosine",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "variant",
        display_name: "Function",
        default: 0.0,
        min: 0.0,
        max: 1.0,
        tooltip: "0: sin z + c, 1: cos z + c.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    if (fparam(0u) < 0.5) {
        return esc_csin(z) + c;
    }
    return esc_ccos(z) + c;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::AbsIm,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // sin z + c -> cos z;  cos z + c -> -sin z.  df/dc = 1.
    var dfz = esc_ccos(z);
    if (fparam(0u) >= 0.5) {
        dfz = -esc_csin(z);
    }
    let d = esc_cmul(dfz, dz);
    return select(d + vec2<f32>(1.0, 0.0), d, is_julia);
}
"#,
};

/// Lambda-sine `z ← λ·sin z` — the Cantor bouquet family.
///
/// NOT `sin z + c` (that is [`TRIG`]). The parameter MULTIPLIES, and
/// the difference is the whole point: the dynamics literature is
/// specific that *"the Julia set of any λ sin(z) with λ ∈ (0,1) … is
/// a Cantor bouquet"* (Pardo-Simón, arXiv:2209.03284, following
/// Devaney–Tangerman). A Cantor bouquet is a Cantor set of disjoint
/// HAIRS, each an arc to infinity, on which every point escapes
/// except the endpoint. Julia mode with a real λ in (0,1) is where
/// they live; try λ ≈ 0.5 and a high iteration cap.
///
/// PARAMETER PLANE SEEDS AT THE CRITICAL POINT, π/2, not at zero.
/// `sin 0 = 0`, so a zero seed is a fixed point and the whole plane
/// would render one colour — the same reason [`LAMBDA`] seeds the
/// logistic map at 1/2. `cos z = 0` at π/2, so that is the critical
/// point, and its critical value is λ.
///
/// Escape is `|Im z| > bailout` RAW, as for the rest of the trig
/// family: sin grows like sinh in the imaginary direction, so orbits
/// leave through ±i∞ rather than outward in every direction. Set the
/// bailout around 50; the default 4 works but crops the hairs early.
pub static LAMBDA_SINE: FormulaDef = FormulaDef {
    name: "lambda_sine",
    display_name: "Lambda Sine (λ·sin z)",
    features: &[],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return esc_cmul(c, esc_csin(z));
}
"#,
    wgsl_param_seed: "vec2<f32>(1.5707964, 0.0)",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::AbsIm,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // f = c·sin z, so df/dz = c·cos z. On the parameter plane the
    // pixel IS c, which contributes df/dc = sin z; the seed is a
    // constant there, so dz starts at 0 and this term is what gets it
    // moving.
    let d = esc_cmul(esc_cmul(c, esc_ccos(z)), dz);
    return select(d + esc_csin(z), d, is_julia);
}
"#,
};

/// Origami — McCabe's Butterfly Origami: fold the plane along a
/// sequence of lines and colour by where each point lands.
///
/// [algorithmic-worlds](https://www.algorithmic-worlds.net/expo/work.php?work=20110204-ds7)
/// records the algorithm: *"Take a square, and choose a number of
/// random lines cutting the square and order them. For each point of
/// the square, compute its images under the sequence of mirror
/// symmetries about the sequence of random lines. Then color the
/// result by performing an average over all the image points."* The
/// only public implementation found is Kyle McDonald's Processing
/// port (OpenProcessing sketch 1185 — source member-locked today,
/// embedded in full in the 2012 Wayback snapshot:
/// <https://web.archive.org/web/20120209174422/http://www.openprocessing.org/visuals/?visualID=1185>),
/// and it contains the detail neither prose description mentions:
///
/// **EACH FOLD LINE'S ENDPOINTS ARE THEMSELVES FOLDED THROUGH ALL
/// PREVIOUS FOLDS** — `randomPosition(i)` returns
/// `foldPoint(random(w), random(h), i)`. Every new crease therefore
/// lands on the current folded wad, the way real paper is folded: you
/// fold the wad you are holding, not the original flat sheet. This is
/// load-bearing. With lines fixed in the plane (the first version
/// here) the wad shrinks away from them, later folds go dead —
/// measured, 0% of pixels moving — and the crease count stays O(F).
/// With wad-relative lines every fold stays active (measured 8%–92%
/// of pixels reflected on every one of 32 folds) and the crease count
/// compounds toward 2^F: the folds-on-folds-on-folds look of McCabe's
/// published images.
///
/// The fold itself is a CONDITIONAL reflection — side chosen by the
/// orientation determinant against the directed segment, reflect
/// about the closest point on the line — exactly McDonald's
/// `foldPoint`. One fold per iteration, so `max_iter` is the fold
/// count; 32 is McCabe's number, and more is SMOOTHER, not more
/// detailed (measured: 96 folds reads softer than 32, because the wad
/// keeps shrinking). Iterations past 64 return the point unchanged.
///
/// **ZOOM STILL BOTTOMS OUT, and now we know the true reason** (the
/// first version's doc blamed an O(F) piece count, which was wrong —
/// the piece count is exponential). Every fold is an isometry, so the
/// final-position field is Lipschitz-1: within a window of size w the
/// landing positions vary by at most w, and creases are DERIVATIVE
/// kinks, not value jumps. Any fixed smooth colour source therefore
/// washes to a constant as the window shrinks, regardless of the fold
/// geometry. What survives zooming is the discrete part — WHICH folds
/// reflected the point — which is piecewise-constant with a jump at
/// every crease. `position_map`'s `address_mix` mixes that channel
/// in; with it, structure was still visible at zoom 22 (measured,
/// following crease intersections), versus zoom ~5 without.
///
/// The lines come from `seed` through an in-shader integer hash
/// (precision-exact and fast-math-immune — the usual
/// `fract(sin(x)*43758.5)` idiom would make the arrangement
/// device-dependent). They are cached in a `var<private>` array,
/// built incrementally: line j needs lines 0..j-1 to fold its
/// endpoints, so construction is O(F²) once per pixel per dispatch,
/// which is trivial next to the iteration loop. WGSL zero-initializes
/// private variables, so the cache is correctly empty at the start of
/// every invocation, including resumed chunks.
///
/// Non-escaping. Pair with `position_map` (projects a colour source
/// onto the folded paper — McDonald's "project an image, unfold" —
/// with the address channel for zoom) or `position_average`
/// (McCabe's stated mean-of-positions).
pub static ORIGAMI: FormulaDef = FormulaDef {
    name: "origami",
    display_name: "Origami (Butterfly)",
    features: &[FormulaFeature::NonEscaping, FormulaFeature::NeedsIndex],
    parameters: &[
        EscapeParamDef {
            name: "seed",
            display_name: "Line seed",
            default: 7.0,
            min: 0.0,
            max: 512.0,
            tooltip: "Chooses the random fold arrangement. Every value is a \
                      different folding; scrub it to explore the family.",
        },
        EscapeParamDef {
            name: "spread",
            display_name: "Line spread",
            default: 2.0,
            min: 0.05,
            max: 4.0,
            tooltip: "Half-size of the square the fold endpoints are drawn \
                      from, before being folded onto the wad. 2.0 matches the \
                      default view.",
        },
    ],
    wgsl: r#"
// An INTEGER hash, deliberately. The usual `fract(sin(x) * 43758.5)`
// trick multiplies a sine by a large constant, which amplifies
// rounding: f32 and f64 disagree, and so can two GPUs. That would
// make the line arrangement — and therefore the whole image —
// device-dependent. Integer ops are exact and immune to fast-math
// (CLAUDE.md), and 24 bits is precisely what an f32 mantissa holds.
fn origami_hash(x: u32) -> u32 {
    var h = x;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return h;
}

fn origami_unit(x: u32) -> f32 {
    // Top 24 bits -> [0,1), exactly representable.
    return f32(origami_hash(x) >> 8u) * (1.0 / 16777216.0);
}

// The fold: reflect p about the line through a-b, but ONLY from the
// negative-determinant side — the other side is the part of the paper
// that does not move. A degenerate segment (endpoints folded onto
// each other) folds nothing.
fn origami_fold(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let ab = b - a;
    let d2 = dot(ab, ab);
    if (ab.x * (p.y - a.y) - ab.y * (p.x - a.x) > 0.0 || d2 < 1e-12) {
        return p;
    }
    let t = dot(p - a, ab) / d2;
    return (a + ab * t) * 2.0 - p;
}

// Fold lines, cached per invocation. WGSL zero-initializes private
// variables, so origami_built starts at 0 in every dispatch —
// including resumed iteration chunks, which rebuild from scratch.
var<private> origami_lines: array<vec4<f32>, 64>;
var<private> origami_built: u32;

// Line j's endpoints are two seeded random points FOLDED THROUGH
// LINES 0..j-1, so every new crease lands on the current wad. This is
// the detail that separates folding paper from slicing the plane:
// without it, later folds miss the shrinking wad and go dead.
fn origami_ensure(upto: u32) {
    let s = u32(clamp(fparam(0u), 0.0, 4096.0)) * 2654435761u;
    let spread = fparam(1u);
    while (origami_built <= upto) {
        let j = origami_built;
        var a = vec2<f32>(
            (origami_unit(j * 4u + 0u + s) * 2.0 - 1.0) * spread,
            (origami_unit(j * 4u + 1u + s) * 2.0 - 1.0) * spread,
        );
        var b = vec2<f32>(
            (origami_unit(j * 4u + 2u + s) * 2.0 - 1.0) * spread,
            (origami_unit(j * 4u + 3u + s) * 2.0 - 1.0) * spread,
        );
        for (var k = 0u; k < j; k = k + 1u) {
            a = origami_fold(a, origami_lines[k].xy, origami_lines[k].zw);
            b = origami_fold(b, origami_lines[k].xy, origami_lines[k].zw);
        }
        origami_lines[j] = vec4<f32>(a, b);
        origami_built = j + 1u;
    }
}

fn formula_step(z: vec2<f32>, c: vec2<f32>, i: u32) -> vec2<f32> {
    if (i >= 64u) {
        return z;
    }
    origami_ensure(i);
    return origami_fold(z, origami_lines[i].xy, origami_lines[i].zw);
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Ducks / Kali-log (Monnier) — `z ← log(Iabs(z) + c)` where
/// Iabs folds the lower half-plane up (Im ← |Im|), i.e. the ADDITION
/// OF C SITS INSIDE THE LOG (plan §5.13; Monnier's post 2011-02-27
/// gives `z = log(Iabs(z)+p)` and Softology's variations post shows
/// fold, add c, then log). We shipped `log(Iabs(z)) + c` at first —
/// a different map that a user caught against the references.
/// Monnier seeds z₀ = 0 on the parameter plane. Softology's
/// pseudocode folds DOWN (Im ← −|Im|), the mirror image; Monnier is
/// the author, so the upper fold is canonical. Non-escaping: the
/// characteristic scaly look is the mean-|z| coloring
/// (`magnitude_average`), per the reference.
/// The variant set is Softology's (2011-04-06 post), ported onto the
/// canonical upper fold: 0 = classic `log(f + c)`; 1 = `log(sin(f + c))`;
/// 2 = `log((f+c) − sec(f+c))`; 3 = sin BEFORE the add,
/// `log(sin(f) + c)`; 4 = `log((f+c)²)` — where f = fold(z).
/// (Softology's pseudocode folds DOWN; the mirror image. Monnier's
/// upper fold stays canonical across all variants.)
pub static DUCKS: FormulaDef = FormulaDef {
    name: "ducks",
    display_name: "Ducks (Kali-log)",
    features: &[FormulaFeature::NonEscaping],
    parameters: &[EscapeParamDef {
        name: "variant",
        display_name: "Variant",
        default: 0.0,
        min: 0.0,
        max: 4.0,
        tooltip: "0 classic log(f+c); 1 log(sin(f+c)); 2 log(f+c - sec(f+c)); \
                  3 log(sin(f)+c); 4 log((f+c)^2). Softology's variation set.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let f = vec2<f32>(z.x, abs(z.y));
    let v = u32(clamp(fparam(0u), 0.0, 4.0));
    if (v == 3u) {
        // Variation 3 reorders: sin BEFORE the constant.
        return esc_clog(esc_csin(f) + c);
    }
    let t = f + c;
    switch v {
        case 1u: {
            return esc_clog(esc_csin(t));
        }
        case 2u: {
            let sec = esc_cdiv(vec2<f32>(1.0, 0.0), esc_ccos(t));
            return esc_clog(t - sec);
        }
        case 4u: {
            return esc_clog(esc_cmul(t, t));
        }
        default: {
            return esc_clog(t);
        }
    }
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Tetration `w ← c^w = e^(w·log c)` (plan §5.8). Parameter plane
/// seeds w₀ = c (the power-tower convention); Julia mode is "Tower
/// Julia". Escape tests Re w. log c is loop-invariant — the compiler
/// hoists it (trust CSE); esc_cexp's clamp is the plan's overflow
/// guard. Converge/period classification lands with the convergent
/// axis; escape-only already draws the tetration star.
pub static TETRATION: FormulaDef = FormulaDef {
    name: "tetration",
    display_name: "Tetration",
    features: &[],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return esc_cexp(esc_cmul(z, esc_clog(c)));
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::Re,
    wgsl_derivative: r#"
fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32> {
    // f = c^z = exp(z log c):  df/dz = log(c) * f,
    // df/dc = z * c^(z-1) = z * f / c.
    let logc = esc_clog(c);
    let f = esc_cexp(esc_cmul(z, logc));
    let d = esc_cmul(esc_cmul(logc, f), dz);
    if (is_julia) {
        return d;
    }
    return d + esc_cdiv(esc_cmul(z, f), c);
}
"#,
};

/// Collatz — the standard interpolation
/// `z ← ¼(2 + 7z − (2+5z)·cos πz)` (plan §5.15). Escape on |Im z|.
pub static COLLATZ: FormulaDef = FormulaDef {
    name: "collatz",
    display_name: "Collatz",
    features: &[],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let cz = esc_ccos(vec2<f32>(pi * z.x, pi * z.y));
    let seven_z = 7.0 * z;
    let two_plus_5z = vec2<f32>(2.0 + 5.0 * z.x, 5.0 * z.y);
    return 0.25 * (vec2<f32>(2.0, 0.0) + seven_z - esc_cmul(two_plus_5z, cz));
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::AbsIm,
    wgsl_derivative: "",
};

/// Feather — `z ← z^p / (1 + (Re²z − i·Im²z)) + c` (plan §5.14).
/// MandelBrowser's code form (`z^p / (1 + complex(z.x², −z.y²)) + c`);
/// the Fractal Art Wiki's ASCII rendering omits the minus on the
/// imaginary part — we follow the code form, and the visual corpus
/// pins our convention (the plan's Feather policy). Denominator real
/// part is ≥ 1, so no pole guard is needed.
pub static FEATHER: FormulaDef = FormulaDef {
    name: "feather",
    display_name: "Feather",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "power",
        display_name: "Power",
        default: 3.0,
        min: 2.0,
        max: 8.0,
        tooltip: "Exponent p in the numerator z^p. The classic feather uses 3.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let num = esc_cpow(z, fparam(0u));
    let den = vec2<f32>(1.0 + z.x * z.x, -(z.y * z.y));
    let d2 = dot(den, den); // >= 1: den.x >= 1 always
    return vec2<f32>(
        (num.x * den.x + num.y * den.y) / d2,
        (num.y * den.x - num.x * den.y) / d2,
    ) + c;
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Newton / root-finder plane over `zᵖ − 1` (plan §5.7): the scheme
/// axis with complex relaxation R — the generalized-relaxation
/// "a-plane" galleries. Schemes shipped: Newton, Halley, Chebyshev
/// (closed forms over f = zᵖ−1, f′ = p·zᵖ⁻¹, f″ = p(p−1)zᵖ⁻²);
/// Schröder/Householder-3/König are noted follow-ups. `c` is unused —
/// the Newton fractal is the dynamical plane, so the parameter plane
/// seeds the pixel too.
pub static NEWTON: FormulaDef = FormulaDef {
    name: "newton",
    display_name: "Newton",
    features: &[FormulaFeature::Convergent],
    parameters: &[
        EscapeParamDef {
            name: "power",
            display_name: "Power",
            default: 3.0,
            min: 2.0,
            max: 12.0,
            tooltip: "Roots of z^p - 1: p basins of attraction.",
        },
        EscapeParamDef {
            name: "scheme",
            display_name: "Scheme",
            default: 0.0,
            min: 0.0,
            max: 2.0,
            tooltip: "0: Newton, 1: Halley, 2: Chebyshev.",
        },
        EscapeParamDef {
            name: "relax_re",
            display_name: "Relaxation (re)",
            default: 1.0,
            min: -3.0,
            max: 3.0,
            tooltip: "Complex relaxation R multiplying the step. 1+0i is the plain scheme; the interesting galleries live away from it.",
        },
        EscapeParamDef {
            name: "relax_im",
            display_name: "Relaxation (im)",
            default: 0.0,
            min: -3.0,
            max: 3.0,
            tooltip: "Imaginary part of the relaxation.",
        },
    ],
    wgsl: r#"
fn newton_delta(z: vec2<f32>) -> vec2<f32> {
    let p = fparam(0u);
    let f = esc_cpow(z, p) - vec2<f32>(1.0, 0.0);
    let fp = p * esc_cpow(z, p - 1.0);
    let scheme = u32(clamp(fparam(1u), 0.0, 2.0));
    if (scheme == 0u) {
        return esc_cdiv(f, fp);
    }
    let fpp = p * (p - 1.0) * esc_cpow(z, p - 2.0);
    if (scheme == 1u) {
        // Halley: 2 f f' / (2 f'^2 - f f'')
        let num = 2.0 * esc_cmul(f, fp);
        let den = 2.0 * esc_cmul(fp, fp) - esc_cmul(f, fpp);
        return esc_cdiv(num, den);
    }
    // Chebyshev: (f/f') * (1 + f f'' / (2 f'^2))
    let nf = esc_cdiv(f, fp);
    let corr = esc_cdiv(esc_cmul(f, fpp), 2.0 * esc_cmul(fp, fp));
    return esc_cmul(nf, vec2<f32>(1.0, 0.0) + corr);
}

fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let relax = vec2<f32>(fparam(2u), fparam(3u));
    return z - esc_cmul(relax, newton_delta(z));
}
"#,
    wgsl_param_seed: "pixel",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Nova (plan §5.7): the Newton step plus `c` — a Mandelbrot-like
/// parameter plane over the convergent core. Seeds the critical
/// point z₀ = 1 on the parameter plane.
pub static NOVA: FormulaDef = FormulaDef {
    name: "nova",
    display_name: "Nova",
    features: &[FormulaFeature::Convergent],
    parameters: &[
        EscapeParamDef {
            name: "power",
            display_name: "Power",
            default: 3.0,
            min: 2.0,
            max: 12.0,
            tooltip: "Power of the underlying z^p - 1.",
        },
        EscapeParamDef {
            name: "relax_re",
            display_name: "Relaxation (re)",
            default: 1.0,
            min: -3.0,
            max: 3.0,
            tooltip: "Complex relaxation R multiplying the Newton step.",
        },
        EscapeParamDef {
            name: "relax_im",
            display_name: "Relaxation (im)",
            default: 0.0,
            min: -3.0,
            max: 3.0,
            tooltip: "Imaginary part of the relaxation.",
        },
    ],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let p = fparam(0u);
    let f = esc_cpow(z, p) - vec2<f32>(1.0, 0.0);
    let fp = p * esc_cpow(z, p - 1.0);
    let relax = vec2<f32>(fparam(1u), fparam(2u));
    return z - esc_cmul(relax, esc_cdiv(f, fp)) + c;
}
"#,
    wgsl_param_seed: "vec2<f32>(1.0, 0.0)",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Magnet I / II (plan §5.10): rational maps from statistical
/// mechanics; escape AND convergence to the fixed point 1 both
/// terminate (the generic |z − z_prev| test catches the latter).
/// Denominator zeros are guarded as escaped.
pub static MAGNET: FormulaDef = FormulaDef {
    name: "magnet",
    display_name: "Magnet",
    features: &[FormulaFeature::Convergent],
    parameters: &[EscapeParamDef {
        name: "variant",
        display_name: "Variant",
        default: 0.0,
        min: 0.0,
        max: 1.0,
        tooltip: "0: Magnet I, 1: Magnet II.",
    }],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    if (fparam(0u) < 0.5) {
        // I: ((z^2 + c - 1) / (2z + c - 2))^2
        let num = esc_cmul(z, z) + c - vec2<f32>(1.0, 0.0);
        let den = 2.0 * z + c - vec2<f32>(2.0, 0.0);
        let q = esc_cdiv(num, den);
        return esc_cmul(q, q);
    }
    // II: ((z^3 + 3(c-1)z + (c-1)(c-2)) / (3z^2 + 3(c-2)z + (c-1)(c-2) + 1))^2
    let cm1 = c - vec2<f32>(1.0, 0.0);
    let cm2 = c - vec2<f32>(2.0, 0.0);
    let c12 = esc_cmul(cm1, cm2);
    let z2 = esc_cmul(z, z);
    let num = esc_cmul(z2, z) + 3.0 * esc_cmul(cm1, z) + c12;
    let den = 3.0 * z2 + 3.0 * esc_cmul(cm2, z) + c12 + vec2<f32>(1.0, 0.0);
    let q = esc_cdiv(num, den);
    return esc_cmul(q, q);
}
"#,
    wgsl_param_seed: "",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Novaretti — `z ← −6z(z³ + c) / (2z³ − c)²` (plan §5.17; community
/// formula credited to Elena Novaretti, ZoneXplorer). Degree-6
/// rational; NOTHING escapes (∞ ↦ 0), so classification is
/// convergence — z = 0 attracts iff |c| > 6, other cycles carry
/// |c| < 6. The parameter plane seeds one of the two closed-form
/// critical orbits, z³ = c·(−7+3√5)/4 (the second, and true period
/// detection for the cycle territory, are noted follow-ups). Poles
/// 2z³ = c are guarded like McMullen's.
pub static NOVARETTI: FormulaDef = FormulaDef {
    name: "novaretti",
    display_name: "Novaretti",
    features: &[FormulaFeature::NonEscaping, FormulaFeature::Convergent],
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let z3 = esc_cmul(esc_cmul(z, z), z);
    let den_root = 2.0 * z3 - c;
    let den = esc_cmul(den_root, den_root);
    if (dot(den, den) < 1e-24) {
        // Double pole: feed a large value, which the next step maps
        // back toward 0 (-6z*z^3 / (2z^3)^2 ~ -1.5/z^3).
        //
        // The sentinel must SURVIVE that next step in f32, which
        // 1e10 does not: (2*(1e10)^3)^2 = 4e60 overflows f32's 3.4e38
        // ceiling, so den and num both become inf and the step
        // returns inf/inf. On Vulkan that is NaN, which then poisons
        // the pixel for every remaining iteration; on Metal, whose
        // fast-math folds inf/inf to 1.0 (see CLAUDE.md), it silently
        // returns a plausible finite number instead -- the same
        // formula rendering differently on two platforms. Novaretti is
        // NON-ESCAPING, so unlike McMullen's identical-looking guard
        // there is no bailout to catch the huge value first: it has to
        // stay representable. 1e6 keeps den at 4e36, two orders inside
        // the ceiling, and reaches ~1.5e-12 next step. Found by the
        // formula audit: 14% of pixels disagreed with an exact oracle
        // at four iterations, and an f32 oracle disagreed identically,
        // which ruled out precision and pointed here.
        return vec2<f32>(1e6, 0.0);
    }
    let num = -6.0 * esc_cmul(z, z3 + c);
    return esc_cdiv(num, den);
}
"#,
    wgsl_param_seed: "esc_cpow(pixel * -0.0729490168, 0.3333333333)",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

/// Littlewood parameter space (plan §5.11): pixel λ is in the root
/// cloud iff the greedy sign-choice map `w ← λ·w + d` (d from the
/// digit set, chosen to minimize |w|) stays bounded — the
/// parameter-space twin of the chaos-game `littlewood` variation.
/// Cross-check against the variation's attractor at landmark λ is a
/// noted follow-up; the corpus pins the annulus structure meanwhile.
pub static LITTLEWOOD: FormulaDef = FormulaDef {
    name: "littlewood",
    display_name: "Littlewood",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "digit_set",
        display_name: "Digit set",
        default: 0.0,
        min: 0.0,
        max: 2.0,
        tooltip: "0: {+1,-1} (Littlewood), 1: {0,+1,-1}, 2: {+1,-1,+i,-i}.",
    }],
    wgsl: r#"
fn littlewood_pick(w: vec2<f32>, ds: u32) -> vec2<f32> {
    // Greedy: the digit minimizing |w + d|.
    var best = w + vec2<f32>(1.0, 0.0);
    var best_d = dot(best, best);
    let cand_m = w - vec2<f32>(1.0, 0.0);
    if (dot(cand_m, cand_m) < best_d) {
        best = cand_m;
        best_d = dot(cand_m, cand_m);
    }
    if (ds >= 1u && dot(w, w) < best_d) {
        // {0,...}: keeping w unchanged is allowed
        best = w;
        best_d = dot(w, w);
    }
    if (ds == 2u) {
        let cand_i = w + vec2<f32>(0.0, 1.0);
        if (dot(cand_i, cand_i) < best_d) {
            best = cand_i;
            best_d = dot(cand_i, cand_i);
        }
        let cand_mi = w - vec2<f32>(0.0, 1.0);
        if (dot(cand_mi, cand_mi) < best_d) {
            best = cand_mi;
        }
    }
    return best;
}

fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    let ds = u32(clamp(fparam(0u), 0.0, 2.0));
    return littlewood_pick(esc_cmul(c, z), ds);
}
"#,
    wgsl_param_seed: "vec2<f32>(1.0, 0.0)",
    wgsl_prev_init: "",
    escape_metric: EscapeMetric::NormSq,
    wgsl_derivative: "",
};

#[cfg(test)]
mod tests {
    /// `lambda_sine` must seed its parameter plane at the CRITICAL
    /// POINT, not at zero.
    ///
    /// `sin 0 = 0`, so zero is a fixed point of `λ·sin z` for EVERY
    /// λ: a zero-seeded parameter plane is one flat colour, with no
    /// error anywhere to say so. This test iterates the map from both
    /// seeds and shows the difference, so a later tidy-up that
    /// "normalises" the seed field fails here with the reason
    /// attached rather than silently flattening the plane.
    #[test]
    fn lambda_sine_seeds_at_the_critical_point() {
        let def = &super::LAMBDA_SINE;
        assert!(
            def.wgsl_param_seed.contains("1.5707964"),
            "the parameter seed must be pi/2 (cos = 0 there, so it is the \
             critical point); found {:?}",
            def.wgsl_param_seed
        );

        // From zero, every lambda stays at zero.
        for (lr, li) in [(0.3f64, 0.2f64), (0.9, -0.4), (2.5, 1.0), (-1.7, 0.6)] {
            let (mut zr, mut zi) = (0.0f64, 0.0f64);
            for _ in 0..50 {
                // lambda * sin(z), complex.
                let (sr, si) = (zr.sin() * zi.cosh(), zr.cos() * zi.sinh());
                let nr = lr * sr - li * si;
                let ni = lr * si + li * sr;
                zr = nr;
                zi = ni;
            }
            assert_eq!(
                (zr, zi),
                (0.0, 0.0),
                "zero is a fixed point for lambda = {lr}+{li}i, which is why the \
                 parameter plane cannot seed there"
            );
        }

        // From pi/2 the orbits actually differ from one another.
        let mut finals = Vec::new();
        for (lr, li) in [(0.3f64, 0.2f64), (0.9, -0.4)] {
            let (mut zr, mut zi) = (std::f64::consts::FRAC_PI_2, 0.0f64);
            for _ in 0..50 {
                let (sr, si) = (zr.sin() * zi.cosh(), zr.cos() * zi.sinh());
                let nr = lr * sr - li * si;
                let ni = lr * si + li * sr;
                zr = nr;
                zi = ni;
            }
            finals.push((zr, zi));
        }
        assert_ne!(
            finals[0], finals[1],
            "seeded at the critical point, different lambdas must give different orbits"
        );
    }
}
