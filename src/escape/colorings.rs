//! Coloring definitions — orbit summary → palette position.
//!
//! One `static ColoringDef` per coloring, WGSL inline. The template
//! wraps the returned coordinate with `fract()` so colorings can
//! return unbounded ramps and let the palette cycle.

use super::{ColoringDef, EscapeParamDef};

/// Discrete escape count: the classic banded look. `t = n · scale`.
pub static ESCAPE_COUNT: ColoringDef = ColoringDef {
    name: "escape_count",
    display_name: "Escape Count",
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 0.05,
        min: 0.001,
        max: 1.0,
        tooltip: "Palette distance per iteration band. Smaller = broader bands.",
    }],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool) -> f32 {
    return f32(n) * cparam(0u);
}
"#,
};

/// Smooth (continuous) iteration count — the standard fractional
/// escape-time formula `mu = n + 1 - log2(log2 |z|)`, which cancels the
/// banding of the discrete count for any quadratic-growth formula.
pub static SMOOTH: ColoringDef = ColoringDef {
    name: "smooth",
    display_name: "Smooth Iteration",
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 0.05,
        min: 0.001,
        max: 1.0,
        tooltip: "Palette distance per iteration. Smaller = broader gradient.",
    }],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool) -> f32 {
    // |z|^2 at escape is > bailout >= 1, so log2 is safe; the max()
    // guards the first-iteration corner (bailout < 1 configs) without
    // any fast-math-hazard idiom (no self-compare, no self-divide).
    let r2 = max(dot(z, z), 1.0000001);
    let mu = f32(n) + 1.0 - log2(0.5 * log2(r2));
    return mu * cparam(0u);
}
"#,
};
