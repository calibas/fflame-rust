//! Advanced 2D variations (indices 5-15, 24+)
//!
//! More complex 2D variations including polar coordinates, Julia sets, etc.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

/// Switches to polar coordinates: the X output becomes the angle (scaled to
/// [-1, 1]), the Y output becomes the radius minus 1. Unwraps circular
/// patterns into horizontal stripes.
///
/// # Authors
/// - Scott Draves
pub static POLAR: VariationDef = VariationDef {
    name: "polar",
    aliases: &[],
    display_name: "Polar",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_polar(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p);
    return vec2<f32>(theta / 3.14159265359, r - 1.0);
}
"#,
    wgsl_3d: r#"
fn variation_polar(p: vec3<f32>) -> vec3<f32> {
    let theta = atan2(p.x, p.y);
    let r = length(p.xy);
    return vec3<f32>(theta / 3.14159265359, r - 1.0, p.z);
}
"#,
};

/// Twists radial waves so the pattern looks like a knotted handkerchief -
/// concentric folds that ripple in toward the center.
///
/// # Authors
/// - Scott Draves
pub static HANDKERCHIEF: VariationDef = VariationDef {
    name: "handkerchief",
    aliases: &[],
    display_name: "Handkerchief",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_handkerchief(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2<f32>(r * sin(theta + r), r * cos(theta - r));
}
"#,
    wgsl_3d: r#"
fn variation_handkerchief(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);
    return vec3<f32>(r * sin(theta + r), r * cos(theta - r), p.z);
}
"#,
};

/// Folds the plane along a heart-shaped curve. The output silhouette traces
/// a cardioid; classic Apophysis effect.
///
/// # Authors
/// - Scott Draves
pub static HEART: VariationDef = VariationDef {
    name: "heart",
    aliases: &[],
    display_name: "Heart",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_heart(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_theta = r * theta;
    return vec2<f32>(r * sin(r_theta), -r * cos(r_theta));
}
"#,
    wgsl_3d: r#"
fn variation_heart(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.x, p.y);
    let r_theta = r * theta;
    return vec3<f32>(r * sin(r_theta), -r * cos(r_theta), p.z);
}
"#,
};

/// Wraps the plane onto a disc, with the angle controlling the radial
/// position and the radius controlling the ripples. Creates a hypnotic
/// sunburst pattern.
///
/// # Authors
/// - Scott Draves
pub static DISC: VariationDef = VariationDef {
    name: "disc",
    aliases: &[],
    display_name: "Disc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_disc(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p);
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec2<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r));
}
"#,
    wgsl_3d: r#"
fn variation_disc(p: vec3<f32>) -> vec3<f32> {
    let theta = atan2(p.x, p.y);
    let r = length(p.xy);
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec3<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r), p.z);
}
"#,
};

/// Combines an inverse-radius scaling with sine/cosine of both angle and
/// radius. The result spirals inward in a logarithmic pattern.
///
/// # Authors
/// - Scott Draves
pub static SPIRAL: VariationDef = VariationDef {
    name: "spiral",
    aliases: &[],
    display_name: "Spiral",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_spiral(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_inv = 1.0 / r;
    return vec2<f32>(
        r_inv * (cos(theta) + sin(r)),
        r_inv * (sin(theta) - cos(r))
    );
}
"#,
    wgsl_3d: r#"
fn variation_spiral(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy) + 1e-6;
    let theta = atan2(p.x, p.y);
    let r_inv = 1.0 / r;
    return vec3<f32>(
        r_inv * (cos(theta) + sin(r)),
        r_inv * (sin(theta) - cos(r)),
        p.z
    );
}
"#,
};

/// Inverts X by the squared radius while leaving Y alone. Stretches things
/// horizontally near the origin and squashes them outside.
///
/// # Authors
/// - Scott Draves
pub static HYPERBOLIC: VariationDef = VariationDef {
    name: "hyperbolic",
    aliases: &[],
    display_name: "Hyperbolic",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hyperbolic(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;
    return vec2<f32>(p.x / r2, p.y);
}
"#,
    wgsl_3d: r#"
fn variation_hyperbolic(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy) + 1e-6;
    return vec3<f32>(p.x / r2, p.y, p.z);
}
"#,
};

/// Maps the plane into a rotated diamond shape using sine and cosine of the
/// polar angle and radius. Produces sharp diagonal symmetry.
///
/// # Authors
/// - Scott Draves
pub static DIAMOND: VariationDef = VariationDef {
    name: "diamond",
    aliases: &[],
    display_name: "Diamond",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_diamond(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    return vec2<f32>(sin(theta) * cos(r), cos(theta) * sin(r));
}
"#,
    wgsl_3d: r#"
fn variation_diamond(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.x, p.y);
    return vec3<f32>(sin(theta) * cos(r), cos(theta) * sin(r), p.z);
}
"#,
};

/// Cubes two sinusoidal functions of angle and radius and blends them.
/// Spreads points into pointed lobes
///
/// # Authors
/// - Scott Draves
pub static EX: VariationDef = VariationDef {
    name: "ex",
    aliases: &[],
    display_name: "Ex",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_ex(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let n0 = sin(theta + r);
    let n1 = cos(theta - r);
    let m0 = n0 * n0 * n0;  // n0³
    let m1 = n1 * n1 * n1;  // n1³
    return vec2<f32>(r * (m0 + m1), r * (m0 - m1));
}
"#,
    wgsl_3d: r#"
fn variation_ex(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);
    let n0 = sin(theta + r);
    let n1 = cos(theta - r);
    let m0 = n0 * n0 * n0;
    let m1 = n1 * n1 * n1;
    return vec3<f32>(r * (m0 + m1), r * (m0 - m1), p.z);
}
"#,
};

/// Randomly picks one of the two branches of the complex square root, then
/// applies it. Each iteration jumps to one half of the Julia-set folding;
/// over time the attractor fills out.
///
/// # Authors
/// - Scott Draves
pub static JULIA: VariationDef = VariationDef {
    name: "julia",
    aliases: &[],
    display_name: "Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn julia(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let sqrt_r = sqrt(r);
    let omega = select(0.0, 3.14159265359, rng_nextf(rng) < 0.5);
    let half_theta = theta / 2.0 + omega;
    return vec2<f32>(sqrt_r * cos(half_theta), sqrt_r * sin(half_theta));
}
"#,
    wgsl_3d: r#"
fn julia(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);
    let sqrt_r = sqrt(r);
    let omega = select(0.0, 3.14159265359, rng_nextf(rng) < 0.5);
    let half_theta = theta / 2.0 + omega;
    return vec3<f32>(sqrt_r * cos(half_theta), sqrt_r * sin(half_theta), p.z);
}
"#,
};

/// Doubles the X coordinate when X is negative, halves the Y coordinate
/// when Y is negative. A simple asymmetric pinch.
///
/// # Authors
/// - Scott Draves
pub static BENT: VariationDef = VariationDef {
    name: "bent",
    aliases: &[],
    display_name: "Bent",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bent(p: vec2<f32>) -> vec2<f32> {
    let nx = select(2.0 * p.x, p.x, p.x >= 0.0);
    let ny = select(p.y / 2.0, p.y, p.y >= 0.0);
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_bent(p: vec3<f32>) -> vec3<f32> {
    let nx = select(2.0 * p.x, p.x, p.x >= 0.0);
    let ny = select(p.y / 2.0, p.y, p.y >= 0.0);
    return vec3<f32>(nx, ny, p.z);
}
"#,
};

// waves: Scott Draves's classic waves variation, using affine matrix fields.
//   x' = x + b · sin(y / (c² + ε))
//   y' = y + e · sin(x / (f² + ε))
// In the standard Apophysis affine notation, b/c/e/f are read directly from
// the transform's affine matrix:
//   our `xform.b`  (= XFORM_COEFF_10) → X amplitude
//   our `xform.e`  (= XFORM_COEFF_20, X translation) → X wavelength (squared)
//   our `xform.d`  (= XFORM_COEFF_11) → Y amplitude
//   our `xform.f`  (= XFORM_COEFF_21, Y translation) → Y wavelength (squared)
//
// First variation to use `needs_transform: true` — the body reads from the
// `transforms` storage buffer via xform_id.
/// Adds sine-wave displacement to each coordinate, using the affine
/// matrix's own b/c/d/f fields as wave parameters. Inherits its frequency
/// and amplitude from the transform itself rather than from extra sliders.
///
/// # Authors
/// - Scott Draves
pub static WAVES: VariationDef = VariationDef {
    name: "waves",
    aliases: &[],
    display_name: "Waves",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waves(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xf = transforms[xform_id];
    return vec2<f32>(
        p.x + xf.b * sin(p.y / (xf.e * xf.e + 1e-6)),
        p.y + xf.d * sin(p.x / (xf.f * xf.f + 1e-6))
    );
}
"#,
    wgsl_3d: r#"
fn variation_waves(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xf = transforms[xform_id];
    return vec3<f32>(
        p.x + xf.b * sin(p.y / (xf.e * xf.e + 1e-6)),
        p.y + xf.d * sin(p.x / (xf.f * xf.f + 1e-6)),
        p.z
    );
}
"#,
};

// Parameterized variations

/// Generalized Julia variation with a chosen integer power. Splits the
/// angle into `power` equally-spaced branches and randomly picks one each
/// iteration. With power = 2 it reduces to the classic Julia.
///
/// # Authors
/// - Scott Draves
pub static JULIAN: VariationDef = VariationDef {
    name: "julian",
    aliases: &[],
    display_name: "JuliaN",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        VariationParamDef {
            name: "power",
            display_name: "Power",
            param_type: ParamType::UnlimitedInteger,
            default_value: 2.0,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: Some("Number of branches the output is split into. Higher = more arms; negative values flip the rotation direction."),
        },
        VariationParamDef {
            name: "dist",
            display_name: "Distance",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: Some("Stretches or compresses each arm radially. 1.0 is balanced; larger pushes arms outward, smaller pulls them in."),
        },
    ],
    // 1 derived value at slot 2:
    //   2: cpower  (dist / |power| / 2)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_julian(user: array<f32, 2>) -> array<f32, 1> {
    let power = user[0];
    let dist = user[1];
    var out: array<f32, 1>;
    out[0] = dist / max(abs(power), 1e-30) / 2.0;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_julian(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let cpower = get_param(xform_id, variation_id, 2u);

    let r2 = dot(p, p);
    let r = pow(r2, cpower);
    let theta = atan2(p.y, p.x);

    let trunc_val = floor(abs(power) * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec2<f32>(r * cos(t), r * sin(t));
}
"#,
    wgsl_3d: r#"
fn variation_julian(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let cpower = get_param(xform_id, variation_id, 2u);

    let r2 = dot(p.xy, p.xy);
    let r = pow(r2, cpower);
    let theta = atan2(p.y, p.x);

    let trunc_val = floor(abs(power) * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec3<f32>(r * cos(t), r * sin(t), p.z);
}
"#,
};

/// Wraps the plane around the origin, with the radius pulsing between a
/// high and low value as the angle rotates. Produces a wavy, bumpy
/// boundary.
///
/// # Authors
/// - Scott Draves
pub static BLOB: VariationDef = VariationDef {
    name: "blob",
    aliases: &[],
    display_name: "Blob",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "high",
            display_name: "High",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-3.0),
            max_value: Some(3.0),
            description: Some("Outer radius - how far the bumps reach at their peaks."),
        },
        VariationParamDef {
            name: "low",
            display_name: "Low",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-3.0),
            max_value: Some(3.0),
            description: Some("Inner radius - how close the bumps recede in the troughs."),
        },
        VariationParamDef {
            name: "waves",
            display_name: "Waves",
            param_type: ParamType::UnlimitedInteger,
            default_value: 6.0,
            min_value: Some(1.0),
            max_value: Some(20.0),
            description: Some("How many bumps go around the perimeter. More waves = finer-grained edge."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blob(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let p1 = get_param(xform_id, variation_id, 0u);  // high
    let p2 = get_param(xform_id, variation_id, 1u);  // low
    let p3 = get_param(xform_id, variation_id, 2u);  // waves

    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)

    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec2<f32>(scale * cos(theta), scale * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_blob(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let p1 = get_param(xform_id, variation_id, 0u);
    let p2 = get_param(xform_id, variation_id, 1u);
    let p3 = get_param(xform_id, variation_id, 2u);

    let r = length(p.xy);
    let theta = atan2(p.x, p.y);

    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec3<f32>(scale * cos(theta), scale * sin(theta), p.z);
}
"#,
};

/// An anti-fisheye that pulls everything toward the unit circle. Inverse of
/// the classic fisheye warp.
///
/// # Authors
/// - Scott Draves
pub static EYEFISH: VariationDef = VariationDef {
    name: "eyefish",
    aliases: &[],
    display_name: "Eyefish",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_eyefish(p: vec2<f32>) -> vec2<f32> {
    let r_xy = length(p) + 1.0;
    let scale = 2.0 / r_xy;
    return vec2<f32>(scale * p.x, scale * p.y);
}
"#,
    wgsl_3d: r#"
fn variation_eyefish(p: vec3<f32>) -> vec3<f32> {
    let r_xy = length(p.xy) + 1.0;
    let scale = 2.0 / r_xy;
    return vec3<f32>(scale * p.x, scale * p.y, p.z);
}
"#,
};

/// Maps the plane onto a sphere - far points shrink toward the equator,
/// near points spread across the surface.
///
/// # Authors
/// - Scott Draves
pub static BUBBLE: VariationDef = VariationDef {
    name: "bubble",
    aliases: &[],
    display_name: "Bubble",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bubble(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let r = r2 / 4.0 + 1.0;
    let scale = 1.0 / r;
    return vec2<f32>(scale * p.x, scale * p.y);
}
"#,
    wgsl_3d: r#"
fn variation_bubble(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: r = (x² + y²)/4 + 1
    // z' = 2/r - 1, scale = 1/r
    let r2_xy = dot(p.xy, p.xy);
    let r = r2_xy / 4.0 + 1.0;
    let scale = 1.0 / r;
    let new_z = 2.0 / r - 1.0;
    return vec3<f32>(scale * p.x, scale * p.y, new_z);
}
"#,
};

/// Wraps the X coordinate around a cylinder (sine), passes Y through
/// unchanged. In 3D, adds a cosine of X as the Z coordinate so the plane
/// really wraps into a cylindrical sheet.
///
/// # Authors
/// - Scott Draves
pub static CYLINDER: VariationDef = VariationDef {
    name: "cylinder",
    // JWildfire ships `cylinder_apo` with an identical body (the
    // Apophysis-rebranded version of the same Scott Draves
    // variation). Accept the JWF token on import.
    aliases: &["cylinder_apo"],
    display_name: "Cylinder",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cylinder(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(p.x), p.y);
}
"#,
    wgsl_3d: r#"
fn variation_cylinder(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: result = (sin(x), y, cos(x))
    return vec3<f32>(sin(p.x), p.y, cos(p.x));
}
"#,
};

/// Multiplies the point by a random radius in a random direction. Adds a
/// textured, noisy spray to the rendered shape.
///
/// # Authors
/// - Scott Draves
pub static NOISE: VariationDef = VariationDef {
    name: "noise",
    aliases: &[],
    display_name: "Noise",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_noise(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec2<f32>(p.x * r * cos(theta), p.y * r * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_noise(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng);
    return vec3<f32>(p.x * r * cos(theta), p.y * r * sin(theta), p.z);
}
"#,
};

/// Replaces the input with a uniformly random point inside the unit disc -
/// the position is ignored. Useful for adding a soft glow or particle haze.
///
/// # Authors
/// - Scott Draves
pub static BLUR: VariationDef = VariationDef {
    name: "blur",
    aliases: &[],
    display_name: "Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec2<f32>(r * cos(theta), r * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng);
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}
"#,
};

/// Like Blur but the random radius follows a bell curve (sum of four
/// uniforms). Produces a softer, more concentrated haze.
///
/// # Authors
/// - Scott Draves
pub static GAUSSIAN_BLUR: VariationDef = VariationDef {
    name: "gaussian_blur",
    aliases: &[],
    display_name: "Gaussian Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_gaussian_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_gaussian_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}
"#,
};

// Extended variations

/// Variant of Polar with log-radius output. Compresses large distances and
/// expands small ones; good for revealing distant structure.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static POLAR2: VariationDef = VariationDef {
    name: "polar2",
    aliases: &[],
    display_name: "Polar2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_polar2(p: vec2<f32>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let r2 = dot(p, p);
    let new_x = atan2(p.x, p.y) / PI;
    let new_y = 0.5 * log(r2) / PI;
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: r#"
fn variation_polar2(p: vec3<f32>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let r2 = dot(p.xy, p.xy);
    let new_x = atan2(p.x, p.y) / PI;
    let new_y = 0.5 * log(r2) / PI;
    return vec3<f32>(new_x, new_y, p.z);
}
"#,
};

/// Divides each coordinate by the absolute difference of squared
/// coordinates. Produces a sharp diagonal cross pattern.
///
/// # Authors
/// - Scott Draves
pub static CROSS: VariationDef = VariationDef {
    name: "cross",
    aliases: &[],
    display_name: "Cross",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cross(p: vec2<f32>) -> vec2<f32> {
    var r = abs((p.x - p.y) * (p.x + p.y) + 1e-6);
    if (r < 0.0) { r = r * -1.0; }
    r = 1.0 / r;
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: r#"
fn variation_cross(p: vec3<f32>) -> vec3<f32> {
    var r = abs((p.x - p.y) * (p.x + p.y) + 1e-6);
    if (r < 0.0) { r = r * -1.0; }
    r = 1.0 / r;
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#,
};

/// Inside the unit circle, inflates points outward; outside, leaves them
/// alone. Creates a coin shape with a sharp edge at radius 1.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static LOONIE: VariationDef = VariationDef {
    name: "loonie",
    aliases: &[],
    display_name: "Loonie",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_loonie(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    if (r2 < 1.0 && r2 != 0.0) {
        let r = sqrt(1.0 / r2 - 1.0);
        return vec2<f32>(p.x * r, p.y * r);
    } else {
        return p;
    }
}
"#,
    wgsl_3d: r#"
fn variation_loonie(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy);
    if (r2 < 1.0 && r2 != 0.0) {
        let r = sqrt(1.0 / r2 - 1.0);
        return vec3<f32>(p.x * r, p.y * r, p.z);
    } else {
        return p;
    }
}
"#,
};

/// Pulls every point toward the origin with a strength that drops off with
/// distance. Produces a magnifying-glass / scrying-orb effect.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static SCRY: VariationDef = VariationDef {
    name: "scry",
    aliases: &[],
    display_name: "Scry",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_scry(p: vec2<f32>) -> vec2<f32> {
    let t = dot(p, p);
    var r = 1.0 / (sqrt(t) * (t + 1.0));
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: r#"
fn variation_scry(p: vec3<f32>) -> vec3<f32> {
    let t = dot(p.xy, p.xy);
    var r = 1.0 / (sqrt(t) * (t + 1.0));
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#,
};

/// 3D-aware companion to [`SCRY`] (Larry Berlin, Sep 2009). Distinct
/// variation, not an alias: the inversion radius is weight-dependent
/// (`r = 1 / (sqrt(t) × (t + 1/w))`), signed by the variation amount;
/// a second factor `u` handles the Z component over a different
/// denominator (`s = sqrt(t) + z²`). Z falls back to
/// `cos(sqrt(x²+y²))` whenever the input or accumulator Z is zero,
/// so the variation lands on a cylinder for the first iteration
/// rather than collapsing flat. For |weight| ≤ 1 the cpp's
/// `smooth = 1 − w` blend collapses to pure additive on Z; for
/// |weight| > 1 the prior accumulator is wiped and replaced by
/// `w × (accumZ + Z·u)` — both branches are handled exactly.
///
/// # Authors
/// - Apophysis Plugin Pack (original `scry`)
/// - Larry Berlin (3D extension)
pub static SCRY_3D: VariationDef = VariationDef {
    name: "scry_3D",
    aliases: &[],
    display_name: "Scry 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform, Feature::NeedsAccum, Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_scry_3D(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // 2D mode: no Z register, so the cpp's Foopzee/Footzee branches
    // are inert. Body is scry's 2D inversion with the cpp's
    // weight-dependent `r = 1 / (sqrt(t) × (t + 1/w))` rather than
    // scry's hard-coded `+1`. Lets a flame saved with scry_3D still
    // render meaningfully in 2D mode.
    let w = transforms[xform_id].variations[variation_id];
    let vvar_inv = 1.0 / (w + select(1e-12, -1e-12, w < 0.0));
    let abs_inv = abs(vvar_inv);
    let sign_factor = select(1.0, -1.0, vvar_inv < 0.0);
    let t = p.x * p.x + p.y * p.y;
    let safe_f = max(sqrt(t), 1e-30);
    let r = sign_factor / (safe_f * (t + abs_inv));
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: r#"
fn variation_scry_3D(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    // EPS sign-matches `w` so 1/(w + EPS) doesn't flip sign near 0.
    let vvar_inv = 1.0 / (w + select(1e-12, -1e-12, w < 0.0));

    let t = p.x * p.x + p.y * p.y;
    let f = sqrt(t);
    let kikr = cos(f);  // cos(sqrt(t)) — Z fallback

    // Footzee = input Z, or kikr if input Z is exactly 0.
    // Foopzee = accumulator Z, or kikr if accumulator Z is exactly 0.
    // Exact-zero comparison matches the cpp; the typical
    // first-variation-in-an-iteration case has accum.z == 0.0.
    let footzee = select(p.z, kikr, p.z == 0.0);
    let foopzee = select(accum.z, kikr, accum.z == 0.0);

    let s = f + footzee * footzee;

    let abs_inv = abs(vvar_inv);
    let sign_factor = select(1.0, -1.0, vvar_inv < 0.0);
    let safe_f = max(f, 1e-30);
    let safe_s = max(s, 1e-30);
    let r = sign_factor / (safe_f * (t + abs_inv));
    let u = sign_factor / (sqrt(safe_s) * (s + abs_inv));

    // The cpp's `smooth × FPz` Z blend: 1 - w when |w| <= 1, else 0.
    // For |w| <= 1 the additive case cancels and we just contribute
    // `w × footzee × u` to Z; for |w| > 1 the prior accumulator gets
    // wiped (smooth = 0) and replaced with `w × (foopzee + footzee × u)`.
    let smoothed = select(0.0, 1.0 - w, abs(w) <= 1.0);

    // Map cpp's `FPz_new = FPz × smooth + w × (foopzee + footzee × u)`
    // into our additive dispatcher `result += w × return_value`:
    //   return_z = accum.z × (smooth − 1) / w + (foopzee + footzee × u)
    // Verifies algebraically against both branches above.
    let return_z = accum.z * (smoothed - 1.0) * inv_w + (foopzee + footzee * u);

    return vec3<f32>(p.x * r, p.y * r, return_z);
}
"#,
};

/// Maps the plane through a hyperbolic curve based on exponentials.
/// Produces two focal points that warp the surrounding space.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static FOCI: VariationDef = VariationDef {
    name: "foci",
    aliases: &[],
    display_name: "Foci",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_foci(p: vec2<f32>) -> vec2<f32> {
    let expx = exp(p.x) * 0.5;
    let expnx = 0.5 * exp(-p.x);
    var tmp = expx + expnx - cos(p.y);
    if (tmp == 0.0) { tmp = 1e-6; }
    tmp = 1.0 / tmp;
    let new_x = (expx - expnx) * tmp;
    let new_y = sin(p.y) * tmp;
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: r#"
fn variation_foci(p: vec3<f32>) -> vec3<f32> {
    let expx = exp(p.x) * 0.5;
    let expnx = 0.5 * exp(-p.x);
    var tmp = expx + expnx - cos(p.y);
    if (tmp == 0.0) { tmp = 1e-6; }
    tmp = 1.0 / tmp;
    let new_x = (expx - expnx) * tmp;
    let new_y = sin(p.y) * tmp;
    return vec3<f32>(new_x, new_y, p.z);
}
"#,
};

/// Conformal map onto an elliptic-coordinate grid. Useful for mathematical-
/// looking, symmetric patterns.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static ELLIPTIC: VariationDef = VariationDef {
    name: "elliptic",
    aliases: &[],
    display_name: "Elliptic",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_elliptic(p: vec2<f32>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let v = 2.0 / PI;
    let tmp = dot(p, p) + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(tmp + x2) + sqrt(tmp - x2));
    let a = p.x / xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));
    let new_x = v * atan2(a, b);
    var new_y = v * log(xmax + sqrt(max(0.0, xmax - 1.0)));
    if (p.y < 0.0) { new_y = -new_y; }
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: r#"
fn variation_elliptic(p: vec3<f32>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let v = 2.0 / PI;
    let tmp = dot(p.xy, p.xy) + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(tmp + x2) + sqrt(tmp - x2));
    let a = p.x / xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));
    let new_x = v * atan2(a, b);
    var new_y = v * log(xmax + sqrt(max(0.0, xmax - 1.0)));
    if (p.y < 0.0) { new_y = -new_y; }
    return vec3<f32>(new_x, new_y, p.z);
}
"#,
};

// Parameterized extended variations

/// Like Waves, but the sine wave frequencies and amplitudes are exposed as
/// sliders instead of being baked into the affine. Independent control over
/// each axis.
///
/// # Authors
/// - Joel Faber
pub static WAVES2: VariationDef = VariationDef {
    name: "waves2",
    aliases: &[],
    display_name: "Waves2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "freqx",
            display_name: "Freq X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.0,
            min_value: Some(0.0),
            max_value: Some(20.0),
            description: Some("Horizontal ripple frequency. More = tighter waves across the X axis."),
        },
        VariationParamDef {
            name: "scalex",
            display_name: "Scale X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Horizontal ripple amplitude. How far points get pushed sideways."),
        },
        VariationParamDef {
            name: "freqy",
            display_name: "Freq Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.0,
            min_value: Some(0.0),
            max_value: Some(20.0),
            description: Some("Vertical ripple frequency."),
        },
        VariationParamDef {
            name: "scaley",
            display_name: "Scale Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Vertical ripple amplitude."),
        },
        VariationParamDef {
            name: "freqz",
            display_name: "Freq Z",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(0.0),
            max_value: Some(20.0),
            description: Some("Depth ripple frequency (3D mode only)."),
        },
        VariationParamDef {
            name: "scalez",
            display_name: "Scale Z",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Depth ripple amplitude (3D mode only)."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waves2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let freqx = get_param(xform_id, variation_id, 0u);
    let scalex = get_param(xform_id, variation_id, 1u);
    let freqy = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);
    let new_x = p.x + scalex * sin(p.y * freqx);
    let new_y = p.y + scaley * sin(p.x * freqy);
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: r#"
fn variation_waves2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Waves2: Sine wave distortion with 6 parameters (3D version)
    let freqx = get_param(xform_id, variation_id, 0u);
    let scalex = get_param(xform_id, variation_id, 1u);
    let freqy = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);
    let freqz = get_param(xform_id, variation_id, 4u);
    let scalez = get_param(xform_id, variation_id, 5u);
    let r_xy = length(p.xy);
    let new_x = p.x + scalex * sin(p.y * freqx);
    let new_y = p.y + scaley * sin(p.x * freqy);
    let new_z = p.z + scalez * sin(r_xy * freqz);
    return vec3<f32>(new_x, new_y, new_z);
}
"#,
};

/// Polar log transform - the output X is the logarithm of the squared
/// distance, the output Y is the angle. Produces a spiral log-scale view.
///
pub static LOG: VariationDef = VariationDef {
    name: "log",
    aliases: &[],
    display_name: "Log",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "base",
            display_name: "Base",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.718281828,
            min_value: Some(1.01),
            max_value: Some(100.0),
            description: Some("Logarithm base. Default `e` (natural log); larger compresses the output, smaller stretches it out."),
        },
    ],
    // 1 derived value at slot 1:
    //   1: denom  (0.5 / log(base))
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_log(user: array<f32, 1>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 0.5 / log(max(user[0], 1.000001));
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_log(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let denom = get_param(xform_id, variation_id, 1u);
    let r2 = dot(p, p);
    return vec2<f32>(log(r2) * denom, atan2(p.y, p.x));
}
"#,
    wgsl_3d: r#"
fn variation_log(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let denom = get_param(xform_id, variation_id, 1u);
    let r2 = dot(p.xy, p.xy);
    return vec3<f32>(log(r2) * denom, atan2(p.y, p.x), p.z);
}
"#,
};

/// Conformal log-spiral mapping inspired by M. C. Escher's prints. Tunes
/// between pure scaling and pure rotation via the beta angle.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static ESCHER: VariationDef = VariationDef {
    name: "escher",
    aliases: &[],
    display_name: "Escher",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "beta",
            display_name: "Beta",
            param_type: ParamType::Angle,
            default_value: 0.0,
            min_value: Some(-180.0),
            max_value: Some(180.0),
            description: Some("Balance between scaling and rotation. At 0 degrees the map is pure scaling; near +/-90 degrees it's pure rotation. Sweep this to get spiraling effects."),
        },
    ],
    // 2 derived values at slots 1..3:
    //   1: c  (0.5 · (1 + cos(beta·π/180)))
    //   2: d  (0.5 · sin(beta·π/180))
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_escher(user: array<f32, 1>) -> array<f32, 2> {
    let beta = user[0] * 3.14159265358979 / 180.0;
    var out: array<f32, 2>;
    out[0] = 0.5 * (1.0 + cos(beta));  // c
    out[1] = 0.5 * sin(beta);          // d
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_escher(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let c = get_param(xform_id, variation_id, 1u);
    let d = get_param(xform_id, variation_id, 2u);
    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p, p));
    let m = exp(c * lnr - d * a);
    let angle = c * a + d * lnr;
    return vec2<f32>(m * cos(angle), m * sin(angle));
}
"#,
    wgsl_3d: r#"
fn variation_escher(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let c = get_param(xform_id, variation_id, 1u);
    let d = get_param(xform_id, variation_id, 2u);
    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p.xy, p.xy));
    let m = exp(c * lnr - d * a);
    let angle = c * a + d * lnr;
    return vec3<f32>(m * cos(angle), m * sin(angle), p.z);
}
"#,
};

/// Maps to bipolar coordinates (a pair of orthogonal coordinate systems
/// centered on two points). Good for two-focus symmetric flames.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static BIPOLAR: VariationDef = VariationDef {
    name: "bipolar",
    aliases: &[],
    display_name: "Bipolar",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "shift",
            display_name: "Shift",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Vertical offset on the output. Slides the bipolar pattern up or down."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bipolar(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    const HALF_PI: f32 = 1.57079632679;
    let shift = get_param(xform_id, variation_id, 0u);
    let x2y2 = dot(p, p);
    var y = 0.5 * atan2(2.0 * p.y, x2y2 - 1.0) + (-HALF_PI * shift);
    if (y > HALF_PI) {
        y = -HALF_PI + (y + HALF_PI) % PI;
    } else if (y < -HALF_PI) {
        y = HALF_PI - (HALF_PI - y) % PI;
    }
    let t = x2y2 + 1.0;
    let x2 = 2.0 * p.x;
    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return vec2<f32>(0.0, 0.0);
    }
    let new_x = (1.0 / (2.0 * PI)) * log(f / g);
    let new_y = (2.0 / PI) * y;
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: r#"
fn variation_bipolar(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    const HALF_PI: f32 = 1.57079632679;
    let shift = get_param(xform_id, variation_id, 0u);
    let x2y2 = dot(p.xy, p.xy);
    var y = 0.5 * atan2(2.0 * p.y, x2y2 - 1.0) + (-HALF_PI * shift);
    if (y > HALF_PI) {
        y = -HALF_PI + (y + HALF_PI) % PI;
    } else if (y < -HALF_PI) {
        y = HALF_PI - (HALF_PI - y) % PI;
    }
    let t = x2y2 + 1.0;
    let x2 = 2.0 * p.x;
    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let new_x = (1.0 / (2.0 * PI)) * log(f / g);
    let new_y = (2.0 / PI) * y;
    return vec3<f32>(new_x, new_y, p.z);
}
"#,
};

/// Inside a unit disc the points rotate and twist; outside the disc they're
/// pushed away from the center. Produces a layered, plate-like swirl.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static LAZYSUSAN: VariationDef = VariationDef {
    name: "lazysusan",
    aliases: &[],
    display_name: "LazySusan",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "spin",
            display_name: "Spin",
            param_type: ParamType::Angle,
            default_value: 0.0,
            min_value: Some(-360.0),
            max_value: Some(360.0),
            description: Some("How far points inside the unit disc rotate."),
        },
        VariationParamDef {
            name: "space",
            display_name: "Space",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Gap added to points outside the disc - pushes the outer region away from center."),
        },
        VariationParamDef {
            name: "twist",
            display_name: "Twist",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-5.0),
            max_value: Some(5.0),
            description: Some("Extra rotation that fades with distance. Adds a twisting motion to the inside."),
        },
        VariationParamDef {
            name: "x",
            display_name: "X Offset",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Horizontal offset of the rotation center."),
        },
        VariationParamDef {
            name: "y",
            display_name: "Y Offset",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Vertical offset of the rotation center."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_lazysusan(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let spin = get_param(xform_id, variation_id, 0u) * PI / 180.0;
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_offset = get_param(xform_id, variation_id, 3u);
    let y_offset = get_param(xform_id, variation_id, 4u);
    let x = p.x - x_offset;
    let y = p.y + y_offset;
    let r = sqrt(x * x + y * y);
    if (r < 1.0) {
        let a = atan2(y, x) + spin + twist * (1.0 - r);
        return vec2<f32>(r * cos(a) + x_offset, r * sin(a) - y_offset);
    } else {
        let r_scale = 1.0 + space / (r + 1e-6);
        return vec2<f32>(r_scale * x + x_offset, r_scale * y - y_offset);
    }
}
"#,
    wgsl_3d: r#"
fn variation_lazysusan(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let spin = get_param(xform_id, variation_id, 0u) * PI / 180.0;
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_offset = get_param(xform_id, variation_id, 3u);
    let y_offset = get_param(xform_id, variation_id, 4u);
    let x = p.x - x_offset;
    let y = p.y + y_offset;
    let r = sqrt(x * x + y * y);
    if (r < 1.0) {
        let a = atan2(y, x) + spin + twist * (1.0 - r);
        return vec3<f32>(r * cos(a) + x_offset, r * sin(a) - y_offset, p.z);
    } else {
        let r_scale = 1.0 + space / (r + 1e-6);
        return vec3<f32>(r_scale * x + x_offset, r_scale * y - y_offset, p.z);
    }
}
"#,
};

/// Carves the plane into concentric ring bands at the chosen spacing. Each
/// ring inverts the radial position within its band.
///
pub static RINGS2: VariationDef = VariationDef {
    name: "rings2",
    aliases: &[],
    display_name: "Rings2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "val",
            display_name: "Value",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: Some("Ring spacing. Smaller packs more rings closer together; larger spreads them out."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_rings2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let val = get_param(xform_id, variation_id, 0u);
    let dx = val * val + 1e-10;
    let length = sqrt(dot(p, p));
    let r = 2.0 - dx * (floor((length / dx + 1.0) / 2.0) * 2.0 / length + 1.0);
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: r#"
fn variation_rings2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let val = get_param(xform_id, variation_id, 0u);
    let dx = val * val + 1e-10;
    let length = sqrt(dot(p.xy, p.xy));
    let r = 2.0 - dx * (floor((length / dx + 1.0) / 2.0) * 2.0 / length + 1.0);
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#,
};

/// Slices the plane into pie wedges and offsets each wedge alternately -
/// even wedges go one way, odd wedges the other. Configurable wedge width
/// and rotation.
///
/// # Authors
/// - Scott Draves
pub static FAN2: VariationDef = VariationDef {
    name: "fan2",
    aliases: &[],
    display_name: "Fan2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "x",
            display_name: "X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: Some("Wedge width. Controls how many sectors the fan is split into."),
        },
        VariationParamDef {
            name: "y",
            display_name: "Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-5.0),
            max_value: Some(5.0),
            description: Some("Rotation offset. Spins the whole fan around the origin."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_fan2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let x_param = get_param(xform_id, variation_id, 0u);
    let y_param = get_param(xform_id, variation_id, 1u);
    let dx = PI * (x_param * x_param + 1e-10);
    let dx2 = dx / 2.0;
    let angle = atan2(p.x, p.y);
    var a: f32;
    if (fract((angle + y_param) / dx) > 0.5) {
        a = angle - dx2;
    } else {
        a = angle + dx2;
    }
    let r = sqrt(dot(p, p));
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_fan2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let x_param = get_param(xform_id, variation_id, 0u);
    let y_param = get_param(xform_id, variation_id, 1u);
    let dx = PI * (x_param * x_param + 1e-10);
    let dx2 = dx / 2.0;
    let angle = atan2(p.x, p.y);
    var a: f32;
    if (fract((angle + y_param) / dx) > 0.5) {
        a = angle - dx2;
    } else {
        a = angle + dx2;
    }
    let r = sqrt(dot(p.xy, p.xy));
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#,
};

/// Peter de Jong attractor - four sine/cosine coefficients drive the
/// output. Famous for producing intricate chaotic attractor shapes.
///
/// # Authors
/// - Scott Draves
pub static PDJ: VariationDef = VariationDef {
    name: "pdj",
    aliases: &[],
    display_name: "PDJ",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "a",
            display_name: "A",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: Some("Coefficient on the first sine - shapes the X output curve."),
        },
        VariationParamDef {
            name: "b",
            display_name: "B",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: Some("Coefficient on the first cosine - shapes the X output curve."),
        },
        VariationParamDef {
            name: "c",
            display_name: "C",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: Some("Coefficient on the second sine - shapes the Y output curve."),
        },
        VariationParamDef {
            name: "d",
            display_name: "D",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: Some("Coefficient on the second cosine - shapes the Y output curve."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_pdj(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    return vec2<f32>(
        sin(a * p.y) - cos(b * p.x),
        sin(c * p.x) - cos(d * p.y)
    );
}
"#,
    wgsl_3d: r#"
fn variation_pdj(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    return vec3<f32>(
        sin(a * p.y) - cos(b * p.x),
        sin(c * p.x) - cos(d * p.y),
        p.z
    );
}
"#,
};

/// Multiplies the input by a complex polynomial (1 + c1*z + c2*z^2) and
/// normalises. Adds a soft swirling distortion.
///
/// # Authors
/// - Scott Draves
pub static CURL: VariationDef = VariationDef {
    name: "curl",
    aliases: &[],
    display_name: "Curl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "c1",
            display_name: "C1",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Linear twist strength. Stronger = tighter curl around the center."),
        },
        VariationParamDef {
            name: "c2",
            display_name: "C2",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Quadratic twist strength. Adds a second-order curl that grows away from the origin."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_curl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);
    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;
    let r = 1.0 / (re * re + im * im);
    return vec2<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r
    );
}
"#,
    wgsl_3d: r#"
fn variation_curl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);
    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;
    let r = 1.0 / (re * re + im * im);
    return vec3<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r,
        p.z
    );
}
"#,
};

/// Tiles the plane into rectangles, mirroring the coordinates within each
/// tile. Produces a checkered, blocky output.
///
/// # Authors
/// - Scott Draves
pub static RECTANGLES: VariationDef = VariationDef {
    name: "rectangles",
    aliases: &[],
    display_name: "Rectangles",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "x",
            display_name: "X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: Some("Width of each rectangular tile."),
        },
        VariationParamDef {
            name: "y",
            display_name: "Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: Some("Height of each rectangular tile."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_rectangles(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let rect_x = get_param(xform_id, variation_id, 0u);
    let rect_y = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(
        (2.0 * floor(p.x / rect_x) + 1.0) * rect_x - p.x,
        (2.0 * floor(p.y / rect_y) + 1.0) * rect_y - p.y
    );
}
"#,
    wgsl_3d: r#"
fn variation_rectangles(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let rect_x = get_param(xform_id, variation_id, 0u);
    let rect_y = get_param(xform_id, variation_id, 1u);
    return vec3<f32>(
        (2.0 * floor(p.x / rect_x) + 1.0) * rect_x - p.x,
        (2.0 * floor(p.y / rect_y) + 1.0) * rect_y - p.y,
        p.z
    );
}
"#,
};

/// Pushes positive-X points and negative-X points apart by `x`, and same
/// for Y. Creates a gap down the middle along each axis.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static SPLITS: VariationDef = VariationDef {
    name: "splits",
    aliases: &[],
    display_name: "Splits",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "x",
            display_name: "X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.4,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Horizontal gap. Pushes positive-X and negative-X points apart by this amount."),
        },
        VariationParamDef {
            name: "y",
            display_name: "Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.4,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Vertical gap. Pushes positive-Y and negative-Y points apart by this amount."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_splits(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let splits_x = get_param(xform_id, variation_id, 0u);
    let splits_y = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(
        select(p.x - splits_x, p.x + splits_x, p.x >= 0.0),
        select(p.y - splits_y, p.y + splits_y, p.y >= 0.0)
    );
}
"#,
    wgsl_3d: r#"
fn variation_splits(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let splits_x = get_param(xform_id, variation_id, 0u);
    let splits_y = get_param(xform_id, variation_id, 1u);
    return vec3<f32>(
        select(p.x - splits_x, p.x + splits_x, p.x >= 0.0),
        select(p.y - splits_y, p.y + splits_y, p.y >= 0.0),
        p.z
    );
}
"#,
};

/// Bends the plane into an N-sided polygon outline. Configurable side
/// count, corner sharpness, and how circle-vs-polygon the shape feels.
///
/// # Authors
/// - Scott Draves
pub static NGON: VariationDef = VariationDef {
    name: "ngon",
    aliases: &[],
    display_name: "Ngon",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "sides",
            display_name: "Sides",
            param_type: ParamType::UnlimitedInteger,
            default_value: 5.0,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: Some("Number of sides of the polygon (e.g. 5 = pentagon, 6 = hexagon)."),
        },
        VariationParamDef {
            name: "power",
            display_name: "Power",
            param_type: ParamType::UnlimitedFloat,
            default_value: 3.0,
            min_value: Some(-2.0),
            max_value: Some(20.0),
            description: Some("Radial exponent. Stretches or compresses the polygon shape outward."),
        },
        VariationParamDef {
            name: "circle",
            display_name: "Circle",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: Some("Blend between polygon and circle. 0 = pure circle, higher = sharper corners."),
        },
        VariationParamDef {
            name: "corners",
            display_name: "Corners",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: Some("Horizontal output offset. Useful for tiling the polygon outward."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_ngon(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let sides = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let circle = get_param(xform_id, variation_id, 2u);
    let corners = get_param(xform_id, variation_id, 3u);
    let theta = atan2(p.y, p.x);
    let phi = theta - (PI * 2.0 / sides) * floor(sides * theta / (PI * 2.0));
    let phi_adj = select(phi, phi - 2.0 * PI / sides, phi > PI / sides);
    let amp = cos(phi_adj) * pow(1.0 / (cos(phi_adj * sides / 2.0) + 1e-10), circle);
    let r = pow(dot(p, p), power * 0.5);
    return vec2<f32>(
        amp * r * cos(theta) + corners,
        amp * r * sin(theta)
    );
}
"#,
    wgsl_3d: r#"
fn variation_ngon(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let sides = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let circle = get_param(xform_id, variation_id, 2u);
    let corners = get_param(xform_id, variation_id, 3u);
    let theta = atan2(p.y, p.x);
    let phi = theta - (PI * 2.0 / sides) * floor(sides * theta / (PI * 2.0));
    let phi_adj = select(phi, phi - 2.0 * PI / sides, phi > PI / sides);
    let amp = cos(phi_adj) * pow(1.0 / (cos(phi_adj * sides / 2.0) + 1e-10), circle);
    let r = pow(dot(p.xy, p.xy), power * 0.5);
    return vec3<f32>(
        amp * r * cos(theta) + corners,
        amp * r * sin(theta),
        p.z
    );
}
"#,
};

/// Drills a corkscrew distortion into the plane - sine waves on both axes
/// coupled together. Produces twisting, augur-like patterns.
///
/// # Authors
/// - Xyrus02
pub static AUGER: VariationDef = VariationDef {
    name: "auger",
    aliases: &[],
    display_name: "Auger",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef {
            name: "freq",
            display_name: "Frequency",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.0),
            max_value: Some(10.0),
            description: Some("Ripple frequency. How many waves go across the surface."),
        },
        VariationParamDef {
            name: "weight",
            display_name: "Weight",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.5,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("How strongly the waves displace points. 0 = no effect."),
        },
        VariationParamDef {
            name: "scale",
            display_name: "Scale",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.5,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Cross-coupling between X and Y waves. Tunes the diagonal texture."),
        },
        VariationParamDef {
            name: "sym",
            display_name: "Symmetry",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: Some("Blend back toward the input. 0 = full displacement, 1 = no displacement."),
        },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_auger(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let freq = get_param(xform_id, variation_id, 0u);
    let weight = get_param(xform_id, variation_id, 1u);
    let scale = get_param(xform_id, variation_id, 2u);
    let sym = get_param(xform_id, variation_id, 3u);
    let s = sin(freq * p.x);
    let t = sin(freq * p.y);
    let dx = p.x + weight * (0.5 * scale * t + abs(p.x) * t);
    let dy = p.y + weight * (0.5 * scale * s + abs(p.y) * s);
    return vec2<f32>(
        p.x + sym * (dx - p.x),
        dy
    );
}
"#,
    wgsl_3d: r#"
fn variation_auger(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let freq = get_param(xform_id, variation_id, 0u);
    let weight = get_param(xform_id, variation_id, 1u);
    let scale = get_param(xform_id, variation_id, 2u);
    let sym = get_param(xform_id, variation_id, 3u);
    let s = sin(freq * p.x);
    let t = sin(freq * p.y);
    let dx = p.x + weight * (0.5 * scale * t + abs(p.x) * t);
    let dy = p.y + weight * (0.5 * scale * s + abs(p.y) * s);
    return vec3<f32>(
        p.x + sym * (dx - p.x),
        dy,
        p.z
    );
}
"#,
};

/// Raises the complex point to a complex power (real + imaginary parts of
/// the exponent both adjustable). Produces logarithmic spirals with `power`
/// arms.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static CPOW: VariationDef = VariationDef {
    name: "cpow",
    aliases: &[],
    display_name: "CPow",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        VariationParamDef {
            name: "r",
            display_name: "R",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: Some("Real component of the complex exponent. Controls scaling and how tightly the spiral winds."),
        },
        VariationParamDef {
            name: "i",
            display_name: "I",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.1,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: Some("Imaginary component of the complex exponent. Controls how much the spiral rotates."),
        },
        VariationParamDef {
            name: "power",
            display_name: "Power",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.5,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: Some("Number of branches in the result. Like JuliaN's `power` - more = more arms."),
        },
    ],
    // 3 derived values at slots 3..6:
    //   3: va  (2π / power)
    //   4: vc  (r / power)
    //   5: vd  (i / power)
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_cpow(user: array<f32, 3>) -> array<f32, 3> {
    let r_param = user[0];
    let i_param = user[1];
    let power = user[2];
    let safe_power = select(power, 1e-30, power == 0.0);
    var out: array<f32, 3>;
    out[0] = 6.28318530717959 / safe_power;  // va
    out[1] = r_param / safe_power;           // vc
    out[2] = i_param / safe_power;           // vd
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cpow(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 2u);
    let va = get_param(xform_id, variation_id, 3u);
    let vc = get_param(xform_id, variation_id, 4u);
    let vd = get_param(xform_id, variation_id, 5u);

    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p, p) + 1e-20);

    let ang = vc * a + vd * lnr + va * floor(power * rng_nextf(rng));
    let m = exp(vc * lnr - vd * a);

    return vec2<f32>(m * cos(ang), m * sin(ang));
}
"#,
    wgsl_3d: r#"
fn variation_cpow(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 2u);
    let va = get_param(xform_id, variation_id, 3u);
    let vc = get_param(xform_id, variation_id, 4u);
    let vd = get_param(xform_id, variation_id, 5u);

    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p.xy, p.xy) + 1e-20);

    let ang = vc * a + vd * lnr + va * floor(power * rng_nextf(rng));
    let m = exp(vc * lnr - vd * a);

    return vec3<f32>(m * cos(ang), m * sin(ang), p.z);
}
"#,
};
