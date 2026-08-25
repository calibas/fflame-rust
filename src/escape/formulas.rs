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

use super::{EscapeParamDef, FormulaDef, FormulaFeature};

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
};

/// McMullen family `z ← zⁿ + c/zᵐ` (plan §5.4) — rational maps with
/// Sierpiński-carpet Julia sets.
///
/// The origin is a pole: a near-zero z is treated as escaped (a huge
/// step output trips the bailout immediately), matching the plan's
/// guard note. `SeedFromPixel` because the critical point 0 IS the
/// pole — the classic carpet pictures are Julia mode; the pixel-seeded
/// parameter plane is the exploratory map until the proper
/// critical-orbit seed lands with the perturbation work.
pub static MCMULLEN: FormulaDef = FormulaDef {
    name: "mcmullen",
    display_name: "McMullen",
    features: &[FormulaFeature::SeedFromPixel],
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
};

/// Kaliset `z ← |z| / ⟨z,z⟩ − c` (component abs; plan §5.12).
///
/// `NonEscaping`: no bailout — the loop runs the full `max_iter`
/// (classic renders use ~50–200) and ONLY the orbit-average colorings
/// show anything (escape-based colorings render black by design; the
/// panel pairing is the user's choice, per the registry-orthogonality
/// principle). Sign convention: minus, the common Kali formula; the
/// `plus_c` toggle covers the other branch. Convention pinned against
/// reference images via the visual corpus (plan §11.3).
pub static KALISET: FormulaDef = FormulaDef {
    name: "kaliset",
    display_name: "Kaliset",
    features: &[FormulaFeature::NonEscaping, FormulaFeature::SeedFromPixel],
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
};
