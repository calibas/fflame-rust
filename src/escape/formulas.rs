//! Formula definitions — the iterated step `z ← f(z, c)`.
//!
//! One `static FormulaDef` per formula, WGSL inline, mirroring
//! `src/variations/defs/*`. Registered in `super::FORMULAS`
//! (append-only). Phase-1 set lands here: Mandelbrot/Multibrot,
//! Tricorn/Multicorn, Burning Ship family, McMullen, Kaliset.

use super::FormulaDef;

/// The classic quadratic map `z ← z² + c`.
///
/// No parameters: Multibrot (arbitrary power) is a separate def so
/// that the common case compiles the two-multiply special form rather
/// than a polar pow, and so the panel doesn't show a power slider on
/// the formula everyone starts with.
pub static MANDELBROT: FormulaDef = FormulaDef {
    name: "mandelbrot",
    display_name: "Mandelbrot",
    parameters: &[],
    wgsl: r#"
fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
}
"#,
};
