//! Basic 2D variations (indices 0-4)
//!
//! These are the fundamental variations from the original fractal flame algorithm.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

/// The identity transform — passes the point through unchanged. Acts as
/// a baseline shape; mix it with other variations to soften their
/// effect, or use a small weight as a placeholder.
///
/// # Authors
/// - Scott Draves
pub static LINEAR: VariationDef = VariationDef {
    name: "linear",
    // Apophysis 7X and JWildfire have a separate `linear3D` variation;
    // our `linear` handles both 2D and 3D from the same definition.
    // Without this alias, `linear3D="…"` attributes get silently dropped
    // on .flame XML import.
    aliases: &["linear3D"],
    display_name: "Linear",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_linear(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_linear(p: vec3<f32>) -> vec3<f32> {
    return p;
}
"#,
};

/// Maps each coordinate through `sin`, folding the plane into a
/// [-1, 1] tile. Produces wavy, ribbon-like patterns that repeat
/// outward from the origin.
///
/// # Authors
/// - Scott Draves
pub static SINUSOIDAL: VariationDef = VariationDef {
    name: "sinusoidal",
    aliases: &[],
    display_name: "Sinusoidal",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_sinusoidal(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(p.x), sin(p.y));
}
"#,
    wgsl_3d: r#"
fn variation_sinusoidal(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(sin(p.x), sin(p.y), p.z);
}
"#,
};

/// Inverts every point through the unit circle — a point at distance
/// r is mapped to distance 1/r. Far points fold inward, near points
/// fly outward; an inside-out fisheye on the plane.
///
/// # Authors
/// - Scott Draves
pub static SPHERICAL: VariationDef = VariationDef {
    name: "spherical",
    aliases: &[],
    display_name: "Spherical",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_spherical(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;
    return p / r2;
}
"#,
    wgsl_3d: r#"
fn variation_spherical(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy) + 1e-6;
    return vec3(p.xy / r2, p.z);
}
"#,
};

/// Rotates each point around the origin by an angle proportional to
/// its distance — the further out, the more it twists. Produces
/// whirlpool and pinwheel patterns.
///
/// # Authors
/// - Scott Draves
pub static SWIRL: VariationDef = VariationDef {
    name: "swirl",
    aliases: &[],
    display_name: "Swirl",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_swirl(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let s = sin(r2);
    let c = cos(r2);
    return vec2<f32>(p.x * s - p.y * c, p.x * c + p.y * s);
}
"#,
    wgsl_3d: r#"
fn variation_swirl(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy);
    let s = sin(r2);
    let c = cos(r2);
    return vec3<f32>(p.x * s - p.y * c, p.x * c + p.y * s, p.z);
}
"#,
};

/// Bends the plane into a horseshoe — opposite quadrants get folded
/// into the same region of the output. The result has a distinctive
/// U-shaped silhouette.
///
/// # Authors
/// - Scott Draves
pub static HORSESHOE: VariationDef = VariationDef {
    name: "horseshoe",
    aliases: &[],
    display_name: "Horseshoe",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_horseshoe(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let r_inv = 1.0 / r;
    return vec2<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv
    );
}
"#,
    wgsl_3d: r#"
fn variation_horseshoe(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy) + 1e-6;
    let r_inv = 1.0 / r;
    return vec3<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv,
        p.z
    );
}
"#,
};
