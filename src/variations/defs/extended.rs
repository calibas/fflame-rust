//! Extended normal-phase variations
//!
//! Additional variations beyond the basic and advanced sets.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Shifts the Z coordinate during the variation pass. The variation's
/// weight is the offset — set the weight to control how far each point
/// moves up or down.
pub static ZTRANSLATE: VariationDef = VariationDef {
    name: "ztranslate",
    aliases: &[],
    display_name: "ZTranslate",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_ztranslate(p: vec2<f32>) -> vec2<f32> {
    // 2D stub: ztranslate only writes Z. Returning `p` here would
    // inject `weight × p.xy` into the variation accumulator (additive
    // normal-phase dispatch) — same phantom-linear bug as zblur had.
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_ztranslate(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: Normal-phase Z translation
    // FPz += vars[variation_id] (added during weighted sum)
    // Return (0, 0, 1) so weighted sum adds weight to Z: result.z += weight * 1
    return vec3<f32>(0.0, 0.0, 1.0);
}
"#,
};

/// 3D version of Julia — splits the output into `power` randomly-chosen
/// branches in both XY and Z. Generates intricate 3D Julia-set fractals.
///
/// # Authors
/// - Joel Faber
pub static JULIA3D: VariationDef = VariationDef {
    name: "julia3D",
    aliases: &["julia3d"],
    display_name: "Julia3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("power", "Power", unlimited_int, 2.0, -10.0, 10.0, "Number of branches in the 3D Julia output. Higher = more arms; negative values flip the rotation."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_julia3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Julia3D in 2D mode (Z = 0). Mirrors cpp transformFunction;
    // N == 0 guarded because the cpp formula divides by absPower.
    let power_f = get_param(xform_id, variation_id, 0u);
    let N = i32(power_f);
    if (N == 0) { return p; }

    let absN_f = f32(abs(N));
    let r2d = dot(p, p);
    let cN = (1.0 / power_f - 1.0) * 0.5;
    let r = pow(r2d, cN);
    let random_idx = i32(rng_nextf(rng) * absN_f);
    let angle = (atan2(p.y, p.x) + 6.28318530718 * f32(random_idx)) / power_f;
    let tmp = r * sqrt(r2d);
    return vec2<f32>(tmp * cos(angle), tmp * sin(angle));
}
"#,
    wgsl_3d: r#"
fn variation_julia3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Julia3D — Joel Faber, ported from julia3D.cpp transformFunction.
    // The cpp file has special branches for power = ±1, ±2 but they are
    // commented out of PluginVarCalc, so upstream always takes this path.
    // The sign of `power` flows naturally through `angle = .../power_f`;
    // do NOT re-negate sin(angle) for negative powers.
    let power_f = get_param(xform_id, variation_id, 0u);
    let N = i32(power_f);
    if (N == 0) { return p; }

    let absN_f = f32(abs(N));
    let z = p.z / absN_f;
    let r2d = dot(p.xy, p.xy);
    let cN = (1.0 / power_f - 1.0) * 0.5;
    let r = pow(r2d + z * z, cN);
    let random_idx = i32(rng_nextf(rng) * absN_f);
    let angle = (atan2(p.y, p.x) + 6.28318530718 * f32(random_idx)) / power_f;
    let tmp = r * sqrt(r2d);
    return vec3<f32>(tmp * cos(angle), tmp * sin(angle), r * z);
}
"#,
};

/// Adds random scatter that varies with distance from a chosen center
/// point. Closer points get less scatter (or more, with `invert`); the
/// random distribution shape is selectable.
pub static FALLOFF2: VariationDef = VariationDef {
    name: "falloff2",
    aliases: &[],
    display_name: "Falloff2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("scatter", "Scatter", unlimited_float, 1.0, 0.000001, 10.0, "Maximum random scatter applied at full strength."),
        param!("mindist", "Min Distance", unlimited_float, 0.5, 0.0, 10.0, "Distance from the center where the falloff kicks in. Points inside this radius get full strength scatter."),
        param!("mul_x", "Multiply X", float, 1.0, 0.0, 1.0, "How strongly the scatter affects the X axis (0 = ignore, 1 = full)."),
        param!("mul_y", "Multiply Y", float, 1.0, 0.0, 1.0, "How strongly the scatter affects the Y axis (0 = ignore, 1 = full)."),
        param!("mul_z", "Multiply Z", float, 0.0, 0.0, 1.0, "How strongly the scatter affects the Z axis (0 = ignore, 1 = full). 3D mode only."),
        param!("mul_c", "Multiply Color", float, 0.0, 0.0, 1.0, "Color-channel scatter strength. Currently unused — direct color writing is not wired up for this variation."),
        param!("x0", "X Center", unlimited_float, 0.0, -10.0, 10.0, "X coordinate of the falloff center."),
        param!("y0", "Y Center", unlimited_float, 0.0, -10.0, 10.0, "Y coordinate of the falloff center."),
        param!("z0", "Z Center", unlimited_float, 0.0, -10.0, 10.0, "Z coordinate of the falloff center."),
        param!("invert", "Invert", bool, false, "When on, flips the falloff direction — full scatter applies far from the center, nothing near it."),
        param!("type", "Blur Type", enum, 0, &["Uniform", "Triangular", "Gaussian"],
            "Random distribution shape. Triangular is smoother; Gaussian concentrates near zero."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_falloff2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Falloff2: Distance-based blur effect with 3 modes
    const PI: f32 = 3.14159265359;
    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blur_type = get_param(xform_id, variation_id, 10u);

    let rmax = 0.04 * scatter;
    var d = sqrt((p.x - x0) * (p.x - x0) + (p.y - y0) * (p.y - y0));

    if (invert > 0.5) {
        d = 1.0 - d;
    }
    if (d < 0.0) {
        d = 0.0;
    }

    d = (d - mindist) * rmax;
    if (d < 0.0) {
        d = 0.0;
    }

    if (blur_type < 0.5) {
        let rand_x = rng_nextf(rng);
        let rand_y = rng_nextf(rng);
        return vec2<f32>(
            p.x + mul_x * rand_x * d,
            p.y + mul_y * rand_y * d
        );
    } else if (blur_type < 1.5) {
        let r_in = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
        let phi = atan2(p.y, p.x) + mul_y * rng_nextf(rng) * d;
        let r = r_in + mul_x * rng_nextf(rng) * d;
        return vec2<f32>(r * cos(phi), r * sin(phi));
    } else {
        let phi = d * rng_nextf(rng) * PI;
        let r = d * rng_nextf(rng);
        return vec2<f32>(
            p.x + mul_x * r * cos(phi),
            p.y + mul_y * r * sin(phi)
        );
    }
}
"#,
    wgsl_3d: r#"
fn variation_falloff2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Falloff2: Distance-based blur effect with 3 modes (3D)
    const PI: f32 = 3.14159265359;
    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let mul_z = get_param(xform_id, variation_id, 4u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let z0 = get_param(xform_id, variation_id, 8u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blur_type = get_param(xform_id, variation_id, 10u);

    let rmax = 0.04 * scatter;
    var d = sqrt((p.x - x0) * (p.x - x0) + (p.y - y0) * (p.y - y0) + (p.z - z0) * (p.z - z0));

    if (invert > 0.5) {
        d = 1.0 - d;
    }
    if (d < 0.0) {
        d = 0.0;
    }

    d = (d - mindist) * rmax;
    if (d < 0.0) {
        d = 0.0;
    }

    if (blur_type < 0.5) {
        let rand_x = rng_nextf(rng);
        let rand_y = rng_nextf(rng);
        let rand_z = rng_nextf(rng);
        return vec3<f32>(
            p.x + mul_x * rand_x * d,
            p.y + mul_y * rand_y * d,
            p.z + mul_z * rand_z * d
        );
    } else if (blur_type < 1.5) {
        let r_in = sqrt(p.x * p.x + p.y * p.y + p.z * p.z) + 1e-6;
        let sigma = asin(p.z / r_in) + mul_z * rng_nextf(rng) * d;
        let phi = atan2(p.y, p.x) + mul_y * rng_nextf(rng) * d;
        let r = r_in + mul_x * rng_nextf(rng) * d;
        return vec3<f32>(
            r * cos(sigma) * cos(phi),
            r * cos(sigma) * sin(phi),
            r * sin(sigma)
        );
    } else {
        let sigma = d * rng_nextf(rng) * 2.0 * PI;
        let phi = d * rng_nextf(rng) * PI;
        let r = d * rng_nextf(rng);
        return vec3<f32>(
            p.x + mul_x * r * cos(sigma) * cos(phi),
            p.y + mul_y * r * cos(sigma) * sin(phi),
            p.z + mul_z * r * sin(sigma)
        );
    }
}
"#,
};

/// Slices the plane into N pie wedges, each compressed and offset by the
/// chosen angle. Adds an optional swirl that increases with distance and a
/// `hole` radial offset.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static WEDGE: VariationDef = VariationDef {
    name: "wedge",
    aliases: &[],
    display_name: "Wedge",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("angle", "Angle", angle, 90.0, "Wedge angle in degrees — how wide each pie slice is before compression."),
        param!("hole", "Hole", unlimited_float, 0.0, -5.0, 5.0, "Radial offset added to the output. Positive pushes the pattern outward, negative pulls it inward."),
        param!("count", "Count", int, 2.0, 1.0, 20.0, "Number of pie wedges arranged around the center."),
        param!("swirl", "Swirl", unlimited_float, 0.0, -30.0, 30.0, "Extra rotation that grows with distance. 0 = no swirl, positive = curves arms outward."),
    ],
    // 2 derived values at slots 4..6:
    //   4: angle_rad  (angle_deg · π/180)
    //   5: comp_fac   (1 − angle_rad · count / (2π))
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_wedge(user: array<f32, 4>) -> array<f32, 2> {
    let angle_deg = user[0];
    let count = user[2];
    let angle_rad = angle_deg * 3.14159265358979 / 180.0;
    var out: array<f32, 2>;
    out[0] = angle_rad;
    out[1] = 1.0 - angle_rad * count * 0.15915494309189534;  // 1/(2π)
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_wedge(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    const C1_2PI: f32 = 0.15915494309189534;

    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    let angle_rad = get_param(xform_id, variation_id, 4u);
    let comp_fac = get_param(xform_id, variation_id, 5u);

    let r = sqrt(dot(p, p));
    var a = atan2(p.y, p.x) + swirl * r;
    let c = floor((count * a + PI) * C1_2PI);
    a = a * comp_fac + c * angle_rad;

    let r_out = r + hole;
    return vec2<f32>(r_out * cos(a), r_out * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_wedge(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    const C1_2PI: f32 = 0.15915494309189534;

    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    let angle_rad = get_param(xform_id, variation_id, 4u);
    let comp_fac = get_param(xform_id, variation_id, 5u);

    let r = sqrt(dot(p.xy, p.xy));
    var a = atan2(p.y, p.x) + swirl * r;
    let c = floor((count * a + PI) * C1_2PI);
    a = a * comp_fac + c * angle_rad;

    let r_out = r + hole;
    return vec3<f32>(r_out * cos(a), r_out * sin(a), p.z);
}
"#,
};

/// Maps the plane onto an epicycloid spiral pattern. `n` controls how many
/// lobes, `thickness` adds randomness, `holes` punches gaps in the pattern.
/// 
/// # Authors
/// - Joel Faber
/// - cyberxaos
pub static EPISPIRAL: VariationDef = VariationDef {
    name: "epispiral",
    aliases: &[],
    display_name: "Epispiral",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("n", "N", unlimited_float, 6.0, -20.0, 20.0, "Number of lobes in the spiral pattern."),
        param!("thickness", "Thickness", unlimited_float, 0.0, -2.0, 2.0, "Random thickness of each lobe. 0 = razor-thin curves, higher = wider bands."),
        param!("holes", "Holes", unlimited_float, 1.0, -10.0, 10.0, "Radial offset that punches gaps in the pattern."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_epispiral(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Epispiral: Epicycloid spiral pattern with random thickness
    let n = get_param(xform_id, variation_id, 0u);
    let thickness = get_param(xform_id, variation_id, 1u);
    let holes = get_param(xform_id, variation_id, 2u);

    let theta = atan2(p.y, p.x);
    let t = rng_nextf(rng) * thickness / cos(n * theta) - holes;

    if (abs(t) < 1e-6) {
        return vec2<f32>(0.0, 0.0);
    }

    return vec2<f32>(t * cos(theta), t * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_epispiral(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Epispiral: 3D (Z passes through)
    let n = get_param(xform_id, variation_id, 0u);
    let thickness = get_param(xform_id, variation_id, 1u);
    let holes = get_param(xform_id, variation_id, 2u);

    let theta = atan2(p.y, p.x);
    let t = rng_nextf(rng) * thickness / cos(n * theta) - holes;

    if (abs(t) < 1e-6) {
        return vec3<f32>(0.0, 0.0, p.z);
    }

    return vec3<f32>(t * cos(theta), t * sin(theta), p.z);
}
"#,
};

/// Wraps the plane into a grid of soft bubbles, each with its own internal
/// twist. Same shape as Pre Bwraps and Post Bwraps but applied in the
/// normal weighted-sum phase.
pub static BWRAPS: VariationDef = VariationDef {
    name: "bwraps",
    aliases: &[],
    display_name: "BWraps",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, -10.0, 10.0, "Width of each grid cell — the plane is divided into cells of this size, each becoming a bubble."),
        param!("space", "Space", unlimited_float, 0.0, -1.0, 1.0, "Gap between cells. 0 = no gap; positive values push the bubbles apart."),
        param!("gain", "Gain", unlimited_float, 1.0, -5.0, 5.0, "How strongly each bubble wraps its contents inward."),
        param!("inner_twist", "Inner Twist", unlimited_float, 0.0, -10.0, 10.0, "Rotation (in degrees) applied at the center of each bubble."),
        param!("outer_twist", "Outer Twist", unlimited_float, 0.0, -10.0, 10.0, "Rotation (in degrees) applied at the edge of each bubble."),
    ],
    // 3 derived values at slots 5..8:
    //   5: g2        (gain² / (radius + ε) + ε)
    //   6: r2        (radius²)
    //   7: rfactor   (radius / max_bubble, where max_bubble = clamp(g2·radius))
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_bwraps(user: array<f32, 5>) -> array<f32, 3> {
    let cellsize = user[0];
    let space = user[1];
    let gain = user[2];
    var out: array<f32, 3>;
    if (cellsize == 0.0) {
        // Body short-circuits when cellsize == 0; init values irrelevant.
        out[0] = 0.0; out[1] = 0.0; out[2] = 0.0;
        return out;
    }
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = (gain * gain) / (radius + 1e-6) + 1e-6;
    var max_bubble = g2 * radius;
    if (max_bubble > 2.0) {
        max_bubble = 1.0;
    } else {
        max_bubble = max_bubble * (1.0 / ((max_bubble * max_bubble) / 4.0 + 1.0));
    }
    out[0] = g2;
    out[1] = radius * radius;
    out[2] = radius / max_bubble;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bwraps(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);

    if (cellsize == 0.0) {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if ((lx * lx + ly * ly) > r2) {
        return p;
    }

    lx = lx * g2;
    ly = ly * g2;

    let r_dist = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);
    lx = lx * r_dist;
    ly = ly * r_dist;

    let r_ratio = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - r_ratio) + outer_twist * r_ratio;

    let vx = cx + cos(theta) * lx + sin(theta) * ly;
    let vy = cy - sin(theta) * lx + cos(theta) * ly;

    return vec2<f32>(vx, vy);
}
"#,
    wgsl_3d: r#"
fn variation_bwraps(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);

    if (cellsize == 0.0) {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if ((lx * lx + ly * ly) > r2) {
        return p;
    }

    lx = lx * g2;
    ly = ly * g2;

    let r_dist = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);
    lx = lx * r_dist;
    ly = ly * r_dist;

    let r_ratio = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - r_ratio) + outer_twist * r_ratio;

    let vx = cx + cos(theta) * lx + sin(theta) * ly;
    let vy = cy - sin(theta) * lx + cos(theta) * ly;

    return vec3<f32>(vx, vy, p.z);
}
"#,
};

/// Kaleidoscope variant of Julia — splits the angle into `power` branches
/// with random sign-flipping, producing symmetric mirror-like patterns.
///
/// # Authors
/// - Scott Draves
pub static JULIASCOPE: VariationDef = VariationDef {
    name: "juliascope",
    aliases: &[],
    display_name: "JuliaScope",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("power", "Power", unlimited_int, 2.0, -20.0, 20.0, "Number of mirror branches. Higher = more reflections; negative values invert the rotation."),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0, "Radial scaling factor. 1.0 is balanced; larger values push arms outward."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_juliascope(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // JWildfire JuliaScopeFunc.transformFunction (the only path JWF uses;
    // the power=±1/±2 special cases are commented out in the source).
    // One random branch index `rnd` in [0, |power|): even branches ADD
    // atan2, odd branches SUBTRACT it (the "scope" mirror — this is what
    // distinguishes juliascope from julian). The SAME `rnd` supplies both
    // the 2*pi*rnd offset and its parity, so only ONE draw is taken.
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let dist = get_param(xform_id, variation_id, 1u);

    let r2 = dot(p, p);
    let absp = abs(power);
    let rnd = i32(f32(absp) * rng_nextf(rng));
    let phi = atan2(p.y, p.x);
    // even rnd -> +phi, odd rnd -> -phi
    let a = (2.0 * PI * f32(rnd) + select(-phi, phi, (rnd & 1) == 0)) / f32(power);
    let cn = dist / f32(power) * 0.5;
    let r = pow(r2, cn);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_juliascope(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // JuliaScope 3D — Z passes through; XY identical to the 2D body. See
    // that one for the JWF transformFunction derivation.
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let dist = get_param(xform_id, variation_id, 1u);

    let r2 = p.x * p.x + p.y * p.y;
    let absp = abs(power);
    let rnd = i32(f32(absp) * rng_nextf(rng));
    let phi = atan2(p.y, p.x);
    // even rnd -> +phi, odd rnd -> -phi
    let a = (2.0 * PI * f32(rnd) + select(-phi, phi, (rnd & 1) == 0)) / f32(power);
    let cn = dist / f32(power) * 0.5;
    let r = pow(r2, cn);
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#,
};

/// 3D extension of [`JULIASCOPE`] with Brad Stefanov's added Z-warp
/// controls. Inherits the kaleidoscope's mirror-branch dispatch
/// (random branch index ∈ [0, |power|), sign of the angle flipped
/// on odd branches) but layers a configurable Z output on top:
///   - `za`, `zb` rescale X² and Y² in the 2D radius term;
///   - `warp` offsets the input Z before participating;
///   - `zamount`, `zdist` scale the radial Z power (`pow(r2d + z²,
///     cPower × zdist)`);
///   - `mode = 1` multiplies the Z output by `sin(wave1) × cos(wave2)`
///     for a per-iteration depth ripple;
///   - `type = 1` switches the XY radial power to use the full 3D
///     length `sqrt(x²+y²+z²)` instead of just `sqrt(x²+y²)`.
///
/// `power` is intentionally unconstrained; values close to 0 NaN
/// the trig branch, so the body guards with `abs_power = max(|int
/// power|, 1)` for the random pick and an `|power| < EPS` early-out.
///
/// # Authors
/// - Brad Stefanov
/// - Scott Draves
pub static JULIASCOPE_3DB: VariationDef = VariationDef {
    name: "juliascope3Db",
    aliases: &[],
    display_name: "JuliaScope 3Db",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("power", "Power", unlimited_int, 3.0, -20.0, 20.0, "Number of mirror branches. Higher = more reflections; negative values invert the rotation. `0` no-ops the variation to avoid divide-by-zero in the angle dispatch."),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0, "Radial scaling factor. Combined with `power` into `cPower = dist / power × 0.5`, which sets the exponent on the radial term."),
        param!("type", "Type", bool, false, "When on, the XY radial term uses the full 3D length `sqrt(x²+y²+z²)` instead of just the 2D `sqrt(x²+y²)`. Affects the XY scale only; Z output is unchanged."),
        param!("warp", "Warp", unlimited_float, 0.0, -10.0, 10.0, "Pre-offset added to the input Z (after dividing by `abs(power)`) before it participates in the Z radial term and output multiplier."),
        param!("za", "za", unlimited_float, 1.0, -10.0, 10.0, "X² scale in the 2D radius `r2d = x²·za + y²·zb` used by the Z radial term."),
        param!("zb", "zb", unlimited_float, 1.0, -10.0, 10.0, "Y² scale in the 2D radius (see `za`)."),
        param!("zamount", "Z Amount", unlimited_float, 1.0, -10.0, 10.0, "Outer multiplier on the Z output — scales `rz` directly."),
        param!("zdist", "Z Distance", unlimited_float, 1.0, -10.0, 10.0, "Exponent multiplier on the Z radial term — `rz = zamount × pow(r2d + z², cPower × zdist)`."),
        param!("mode", "Mode", bool, false, "When on, multiplies the Z output by `sin(wave1) × cos(wave2)`. Lets `wave1` / `wave2` modulate the depth contribution per iteration."),
        param!("wave1", "Wave 1", unlimited_float, 1.0, -10.0, 10.0, "Argument to `sin()` in the mode-on Z multiplier. Ignored when `mode` is off."),
        param!("wave2", "Wave 2", unlimited_float, 1.0, -10.0, 10.0, "Argument to `cos()` in the mode-on Z multiplier. Ignored when `mode` is off."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_juliascope3Db(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // 2D mode: drop Z entirely. The `type` switch becomes a no-op
    // (z² = 0 in either branch). Output is just the XY part of the
    // 3D body — kaleidoscope dispatch + radial scaling.
    const PI: f32 = 3.14159265359;
    let power = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);
    if (abs(power) < 1e-6) { return vec2<f32>(0.0, 0.0); }

    let c_power = dist / power * 0.5;
    let abs_power = max(i32(abs(power)), 1);

    let rnd = i32(rng_nextf(rng) * f32(abs_power));
    let theta = atan2(p.y, p.x);
    let a = select(
        (2.0 * PI * f32(rnd) - theta) / power,
        (2.0 * PI * f32(rnd) + theta) / power,
        (rnd & 1) == 0,
    );

    let r2 = p.x * p.x + p.y * p.y;
    let r = pow(r2, c_power);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_juliascope3Db(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let power = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);
    let type_ = i32(get_param(xform_id, variation_id, 2u));
    let warp = get_param(xform_id, variation_id, 3u);
    let za = get_param(xform_id, variation_id, 4u);
    let zb = get_param(xform_id, variation_id, 5u);
    let zamount = get_param(xform_id, variation_id, 6u);
    let zdist = get_param(xform_id, variation_id, 7u);
    let mode = i32(get_param(xform_id, variation_id, 8u));
    let wave1 = get_param(xform_id, variation_id, 9u);
    let wave2 = get_param(xform_id, variation_id, 10u);

    if (abs(power) < 1e-6) { return vec3<f32>(0.0, 0.0, 0.0); }

    let c_power = dist / power * 0.5;
    let abs_power = max(i32(abs(power)), 1);
    let abs_power_f = f32(abs_power);

    // Z pre-scale + warp offset. The /abs_power keeps z bounded
    // across the radial pow() so it doesn't explode for high powers.
    let z = p.z / abs_power_f + warp;

    // 2D radius with per-axis scaling, then the Z radial term.
    let r2d = p.x * p.x * za + p.y * p.y * zb;
    let rz = zamount * pow(max(r2d + z * z, 1e-30), c_power * zdist);

    // Random kaleidoscope branch: rnd ∈ [0, abs_power). Even branches
    // add the polar angle, odd branches subtract it (mirror effect).
    let rnd = i32(rng_nextf(rng) * abs_power_f);
    let theta = atan2(p.y, p.x);
    let a = select(
        (2.0 * PI * f32(rnd) - theta) / power,
        (2.0 * PI * f32(rnd) + theta) / power,
        (rnd & 1) == 0,
    );

    // XY radial scale. type=1 uses the full 3D length; type=0 stays
    // 2D. Either way the exponent is c_power.
    let xy2 = p.x * p.x + p.y * p.y;
    let r2 = select(xy2, xy2 + p.z * p.z, type_ != 0);
    let r = pow(max(r2, 1e-30), c_power);

    // Z output. mode=1 multiplies by sin(wave1)·cos(wave2) — lets
    // the wave params modulate depth per iteration.
    let z_factor = select(1.0, sin(wave1) * cos(wave2), mode != 0);
    return vec3<f32>(r * cos(a), r * sin(a), rz * z * z_factor);
}
"#,
};

/// 3D variant of Julia where the Z coordinate gets folded along with the
/// XY. Produces 3D fractals stretched along the depth axis.
pub static JULIA3DZ: VariationDef = VariationDef {
    name: "julia3Dz",
    aliases: &["julia3dz"],
    display_name: "Julia3Dz",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("power", "Power", unlimited_int, 2.0, -20.0, 20.0, "Number of Julia branches in the 3D output. Higher = more arms."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_julia3Dz(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // 2D stub: julia3Dz's XY contribution depends on z-derived
    // coefficients, so it doesn't have a meaningful 2D fallback.
    // Returning `p` would inject `weight × p.xy` (phantom-linear bug
    // class). Better to no-op; for a 2D Julia effect, use `julia` or
    // `julian` directly.
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_julia3Dz(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Julia3Dz - Full 3D Julia set with Z modification
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let abs_power = abs(power);
    let power_f = f32(power);

    let r2d = p.x * p.x + p.y * p.y;

    if (power == 1) {
        return p;
    }

    if (power == -1) {
        let r = 1.0 / r2d;
        return vec3<f32>(r * p.x, -r * p.y, r * p.z);
    }

    let rnd_int = i32(rng_nextf(rng) * f32(abs_power));
    // ff_atan2: zero pairs are reachable (preset:Bubbles, measured) and
    // Metal's fast atan2 NaNs the mixed-sign ones; the z-lane 0/0 NaN
    // that remains is shared with Windows and zeroed by recovery.
    let angle = (ff_atan2(p.y, p.x) + 2.0 * PI * f32(rnd_int)) / power_f;

    if (power == 2) {
        let r2d_sqrt = sqrt(r2d);
        let r = sqrt(r2d_sqrt);
        let z_out = r * p.z / r2d_sqrt / 2.0;
        return vec3<f32>(r * cos(angle), r * sin(angle), z_out);
    }

    if (power == -2) {
        let r2d_sqrt = sqrt(r2d);
        let r = 1.0 / sqrt(r2d_sqrt);
        let z_out = r * p.z / r2d_sqrt / 2.0;
        return vec3<f32>(r * cos(angle), -r * sin(angle), z_out);
    }

    let cN = 1.0 / power_f / 2.0;
    let r = pow(r2d, cN);
    let r2d_sqrt = sqrt(r2d);
    let z_out = r * p.z / (r2d_sqrt * f32(abs_power));

    return vec3<f32>(r * cos(angle), r * sin(angle), z_out);
}
"#,
};

/// 3D version of Curl — applies a complex polynomial twist along all three
/// axes. Each axis has its own twist coefficient.
pub static CURL3D: VariationDef = VariationDef {
    name: "curl3D",
    aliases: &["curl3d"],
    display_name: "Curl3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[
        param!("cx", "CX", unlimited_float, 0.0, -5.0, 5.0, "Twist strength along the X axis."),
        param!("cy", "CY", unlimited_float, 0.0, -5.0, 5.0, "Twist strength along the Y axis."),
        param!("cz", "CZ", unlimited_float, 0.0, -5.0, 5.0, "Twist strength along the Z axis."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_curl3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Curl3D is a 3D variation, apply XY curl in 2D
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);

    let r2 = p.x * p.x + p.y * p.y;
    let c2 = cx * cx + cy * cy;
    let denom = r2 * c2 + 2.0 * cx * p.x - 2.0 * cy * p.y + 1.0;
    let r = 1.0 / denom;

    return vec2<f32>(
        r * (p.x + cx * r2),
        r * (p.y - cy * r2)
    );
}
"#,
    wgsl_3d: r#"
fn variation_curl3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Curl3D - Full 3D curl transformation
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let cz = get_param(xform_id, variation_id, 2u);

    let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
    let c2 = cx * cx + cy * cy + cz * cz;
    let denom = r2 * c2 + 2.0 * cx * p.x - 2.0 * cy * p.y + 2.0 * cz * p.z + 1.0;
    let r = 1.0 / denom;

    return vec3<f32>(
        r * (p.x + cx * r2),
        r * (p.y - cy * r2),
        r * (p.z + cz * r2)
    );
}
"#,
};

/// Adds randomness in both rotation (spin) and scale (zoom) around the
/// origin. The angle slider controls the mix — 0° = pure zoom blur,
/// 90° = balanced, 180° = pure rotational blur.
///
/// flam3 / JWildfire store this parameter in half-turn units
/// (`spin = sin(angle · π/2)`, so 1.0 = pure spin, JWF default 0.5 =
/// balanced) — our degrees relate by `degrees = jwf_value × 180`,
/// converted at the .flame XML boundary (see `flame_xml.rs`).
///
/// Weight semantics follow JWF's `RadialBlurFunc.transform`: `pAmount`
/// scales spin/zoom only, while the `ra` rotation term and the `−1` in
/// `rz` stay unscaled. The WGSL bakes the weight in and pre-divides by
/// it to cancel the dispatcher's outer multiply (idisc pattern).
///
/// # Authors
/// - Scott Draves
/// - Andreas Maschke
pub static RADIAL_BLUR: VariationDef = VariationDef {
    name: "radial_blur",
    aliases: &[],
    display_name: "Radial Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("angle", "Angle", angle, 0.0, "Spin/zoom balance. 0 degrees = pure zoom blur, 180 degrees = pure rotational blur, 90 degrees = balanced mix."),
    ],
    // 2 derived values at slots 1..3:
    //   1: spin_var  (sin(angle_deg · π/360))
    //   2: zoom_var  (cos(angle_deg · π/360))
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_radial_blur(user: array<f32, 1>) -> array<f32, 2> {
    // deg·π/360 = (deg/180)·(π/2) — JWF's angle·π/2 with angle = deg/180.
    let half_angle_rad = user[0] * 3.14159265358979 / 360.0;
    var out: array<f32, 2>;
    out[0] = sin(half_angle_rad);  // spin_var
    out[1] = cos(half_angle_rad);  // zoom_var
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_radial_blur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // JWF bakes the variation weight INTO spin/zoom while ra and the
    // −1 in rz stay unscaled; the dispatch site multiplies our return
    // value by the weight, so compute the JWF result and pre-divide.
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let spin = w * get_param(xform_id, variation_id, 1u);
    let zoom = w * get_param(xform_id, variation_id, 2u);

    let rnd_g = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;

    let ra = sqrt(p.x * p.x + p.y * p.y);
    let alpha = atan2(p.y, p.x) + spin * rnd_g;
    let rz = zoom * rnd_g - 1.0;

    return vec2<f32>(
        (ra * cos(alpha) + rz * p.x) * inv_w,
        (ra * sin(alpha) + rz * p.y) * inv_w
    );
}
"#,
    wgsl_3d: r#"
fn variation_radial_blur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let spin = w * get_param(xform_id, variation_id, 1u);
    let zoom = w * get_param(xform_id, variation_id, 2u);

    let rnd_g = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;

    let ra = sqrt(p.x * p.x + p.y * p.y);
    let alpha = atan2(p.y, p.x) + spin * rnd_g;
    let rz = zoom * rnd_g - 1.0;

    // JWF preserve-z adds pAmount·z; the dispatcher's outer weight
    // supplies that on the unscaled p.z.
    return vec3<f32>(
        (ra * cos(alpha) + rz * p.x) * inv_w,
        (ra * sin(alpha) + rz * p.y) * inv_w,
        p.z
    );
}
"#,
};

/// Replaces the input with a uniformly random point inside the unit circle.
/// Like Blur, but with a sharp circular boundary instead of a soft
/// gradient.
pub static BLUR_CIRCLE: VariationDef = VariationDef {
    name: "blur_circle",
    aliases: &[],
    display_name: "Blur Circle",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blur_circle(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Circle - uniform blur in a circle
    const PI: f32 = 3.14159265359;
    const PI_4: f32 = 0.78539816339;

    let x = 2.0 * rng_nextf(rng) - 1.0;
    let y = 2.0 * rng_nextf(rng) - 1.0;

    let absx = abs(x);
    let absy = abs(y);

    var perimeter: f32;
    var side: f32;

    if (absx >= absy) {
        if (x >= absy) {
            perimeter = absx + y;
        } else {
            perimeter = 5.0 * absx - y;
        }
        side = absx;
    } else {
        if (y >= absx) {
            perimeter = 3.0 * absy - x;
        } else {
            perimeter = 7.0 * absy + x;
        }
        side = absy;
    }

    let r = side;
    let angle = PI_4 * perimeter / side - PI_4;

    return vec2<f32>(r * cos(angle), r * sin(angle));
}
"#,
    wgsl_3d: r#"
fn variation_blur_circle(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Circle - 3D (Z passes through)
    const PI: f32 = 3.14159265359;
    const PI_4: f32 = 0.78539816339;

    let x = 2.0 * rng_nextf(rng) - 1.0;
    let y = 2.0 * rng_nextf(rng) - 1.0;

    let absx = abs(x);
    let absy = abs(y);

    var perimeter: f32;
    var side: f32;

    if (absx >= absy) {
        if (x >= absy) {
            perimeter = absx + y;
        } else {
            perimeter = 5.0 * absx - y;
        }
        side = absx;
    } else {
        if (y >= absx) {
            perimeter = 3.0 * absy - x;
        } else {
            perimeter = 7.0 * absy + x;
        }
        side = absy;
    }

    let r = side;
    let angle = PI_4 * perimeter / side - PI_4;

    return vec3<f32>(r * cos(angle), r * sin(angle), p.z);
}
"#,
};

/// Random zoom blur radiating from a chosen center point. The `length`
/// slider controls how far points get pushed; `x` and `y` set the center.
pub static BLUR_ZOOM: VariationDef = VariationDef {
    name: "blur_zoom",
    aliases: &[],
    display_name: "Blur Zoom",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("length", "Length", unlimited_float, 0.0, -5.0, 5.0, "Maximum zoom distance. Larger values streak points further outward from the center."),
        param!("x", "X", unlimited_float, 0.0, -20.0, 20.0, "X coordinate of the zoom center."),
        param!("y", "Y", unlimited_float, 0.0, -20.0, 20.0, "Y coordinate of the zoom center."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blur_zoom(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Zoom - zoom blur from a center point
    let length = get_param(xform_id, variation_id, 0u);
    let zoom_x = get_param(xform_id, variation_id, 1u);
    let zoom_y = get_param(xform_id, variation_id, 2u);

    let z = 1.0 + length * rng_nextf(rng);

    return vec2<f32>(
        (p.x - zoom_x) * z + zoom_x,
        (p.y - zoom_y) * z + zoom_y
    );
}
"#,
    wgsl_3d: r#"
fn variation_blur_zoom(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Zoom - 3D (Z passes through)
    let length = get_param(xform_id, variation_id, 0u);
    let zoom_x = get_param(xform_id, variation_id, 1u);
    let zoom_y = get_param(xform_id, variation_id, 2u);

    let z = 1.0 + length * rng_nextf(rng);

    return vec3<f32>(
        (p.x - zoom_x) * z + zoom_x,
        (p.y - zoom_y) * z + zoom_y,
        p.z
    );
}
"#,
};

/// Snaps points to a grid of pixel-sized cells, then adds random offset
/// within each cell. Produces a mosaic effect.
pub static BLUR_PIXELIZE: VariationDef = VariationDef {
    name: "blur_pixelize",
    aliases: &[],
    display_name: "Blur Pixelize",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("size", "Size", unlimited_float, 0.1, 0.0000001, 10.0, "Pixel cell size. Smaller = finer grid; larger = chunkier pixels."),
        param!("scale", "Scale", unlimited_float, 1.0, -20.0, 20.0, "How much each point can jitter within its cell. 0 = points snap to cell centers; 1 = points scatter across the cell."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blur_pixelize(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Pixelize - pixelated/mosaic blur effect
    let size = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);

    let inv_size = 1.0 / size;

    let x = floor(p.x * inv_size);
    let y = floor(p.y * inv_size);

    return vec2<f32>(
        size * (x + scale * (rng_nextf(rng) - 0.5) + 0.5),
        size * (y + scale * (rng_nextf(rng) - 0.5) + 0.5)
    );
}
"#,
    wgsl_3d: r#"
fn variation_blur_pixelize(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Pixelize - 3D (Z passes through)
    let size = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);

    let inv_size = 1.0 / size;

    let x = floor(p.x * inv_size);
    let y = floor(p.y * inv_size);

    return vec3<f32>(
        size * (x + scale * (rng_nextf(rng) - 0.5) + 0.5),
        size * (y + scale * (rng_nextf(rng) - 0.5) + 0.5),
        p.z
    );
}
"#,
};

/// Pushes points away from the X and Y axes by configurable amounts, with
/// separate inside/outside offsets. Creates a split, mirrored look.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static SEPARATION: VariationDef = VariationDef {
    name: "separation",
    aliases: &[],
    display_name: "Separation",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("x", "X", unlimited_float, 1.0, -20.0, 20.0, "How far to push points away from the X axis on either side."),
        param!("y", "Y", unlimited_float, 1.0, -20.0, 20.0, "How far to push points away from the Y axis on either side."),
        param!("xinside", "X Inside", unlimited_float, 0.0, -20.0, 20.0, "Inside offset along X — adjusts how the separation looks near the axis."),
        param!("yinside", "Y Inside", unlimited_float, 0.0, -20.0, 20.0, "Inside offset along Y — adjusts how the separation looks near the axis."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_separation(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Separation - separates positive/negative quadrants
    let sep_x = get_param(xform_id, variation_id, 0u);
    let sep_y = get_param(xform_id, variation_id, 1u);
    let xinside = get_param(xform_id, variation_id, 2u);
    let yinside = get_param(xform_id, variation_id, 3u);

    let x_out = select(
        -(sqrt(p.x * p.x + sep_x * sep_x) + p.x * xinside),
        sqrt(p.x * p.x + sep_x * sep_x) - p.x * xinside,
        p.x > 0.0
    );

    let y_out = select(
        -(sqrt(p.y * p.y + sep_y * sep_y) + p.y * yinside),
        sqrt(p.y * p.y + sep_y * sep_y) - p.y * yinside,
        p.y > 0.0
    );

    return vec2<f32>(x_out, y_out);
}
"#,
    wgsl_3d: r#"
fn variation_separation(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Separation - 3D (Z passes through)
    let sep_x = get_param(xform_id, variation_id, 0u);
    let sep_y = get_param(xform_id, variation_id, 1u);
    let xinside = get_param(xform_id, variation_id, 2u);
    let yinside = get_param(xform_id, variation_id, 3u);

    let x_out = select(
        -(sqrt(p.x * p.x + sep_x * sep_x) + p.x * xinside),
        sqrt(p.x * p.x + sep_x * sep_x) - p.x * xinside,
        p.x > 0.0
    );

    let y_out = select(
        -(sqrt(p.y * p.y + sep_y * sep_y) + p.y * yinside),
        sqrt(p.y * p.y + sep_y * sep_y) - p.y * yinside,
        p.y > 0.0
    );

    return vec3<f32>(x_out, y_out, p.z);
}
"#,
};

/// Möbius transformation in the complex plane — `(Az + B) / (Cz + D)`. The
/// eight real/imaginary coefficients (A, B, C, D) control the conformal
/// warping; classic hyperbolic-geometry effect.
/// 
/// # Authors
/// - eralex61
pub static MOBIUS: VariationDef = VariationDef {
    name: "mobius",
    aliases: &[],
    display_name: "Mobius",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("re_a", "Re A", unlimited_float, 0.1, -1.0, 1.0, "Real component of complex coefficient A in `(Az + B)/(Cz + D)`."),
        param!("im_a", "Im A", unlimited_float, 0.2, -1.0, 1.0, "Imaginary component of complex coefficient A."),
        param!("re_b", "Re B", unlimited_float, 0.2, -1.0, 1.0, "Real component of complex coefficient B."),
        param!("im_b", "Im B", unlimited_float, -0.12, -1.0, 1.0, "Imaginary component of complex coefficient B."),
        param!("re_c", "Re C", unlimited_float, -0.15, -1.0, 1.0, "Real component of complex coefficient C."),
        param!("im_c", "Im C", unlimited_float, -0.15, -1.0, 1.0, "Imaginary component of complex coefficient C."),
        param!("re_d", "Re D", unlimited_float, 0.21, -1.0, 1.0, "Real component of complex coefficient D."),
        param!("im_d", "Im D", unlimited_float, 0.1, -1.0, 1.0, "Imaginary component of complex coefficient D."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_mobius(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Mobius - Möbius transformation f(z) = (Az + B)/(Cz + D)
    let re_a = get_param(xform_id, variation_id, 0u);
    let im_a = get_param(xform_id, variation_id, 1u);
    let re_b = get_param(xform_id, variation_id, 2u);
    let im_b = get_param(xform_id, variation_id, 3u);
    let re_c = get_param(xform_id, variation_id, 4u);
    let im_c = get_param(xform_id, variation_id, 5u);
    let re_d = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;

    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;

    let v_denom = re_v * re_v + im_v * im_v + 1e-10;

    return vec2<f32>(
        (re_u * re_v + im_u * im_v) / v_denom,
        (im_u * re_v - re_u * im_v) / v_denom
    );
}
"#,
    wgsl_3d: r#"
fn variation_mobius(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Mobius - 3D (Z passes through)
    let re_a = get_param(xform_id, variation_id, 0u);
    let im_a = get_param(xform_id, variation_id, 1u);
    let re_b = get_param(xform_id, variation_id, 2u);
    let im_b = get_param(xform_id, variation_id, 3u);
    let re_c = get_param(xform_id, variation_id, 4u);
    let im_c = get_param(xform_id, variation_id, 5u);
    let re_d = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;

    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;

    let v_denom = re_v * re_v + im_v * im_v + 1e-10;

    return vec3<f32>(
        (re_u * re_v + im_u * im_v) / v_denom,
        (im_u * re_v - re_u * im_v) / v_denom,
        p.z
    );
}
"#,
};

/// Constrains points to a rectangle. Points outside either collapse to zero
/// or get scattered along the nearest edge, depending on `zero`.
/// 
/// # Authors
/// - Xyrus02
pub static CROP: VariationDef = VariationDef {
    name: "crop",
    aliases: &[],
    display_name: "Crop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace],
    parameters: &[
        param!("left", "Left", unlimited_float, -1.0, -5.0, 5.0, "Left edge of the rectangle the points are constrained to."),
        param!("top", "Top", unlimited_float, -1.0, -5.0, 5.0, "Top edge of the rectangle."),
        param!("right", "Right", unlimited_float, 1.0, -5.0, 5.0, "Right edge of the rectangle."),
        param!("bottom", "Bottom", unlimited_float, 1.0, -5.0, 5.0, "Bottom edge of the rectangle."),
        param!("scatter_area", "Scatter Area", float, 0.0, -1.0, 1.0, "Width of the random scatter band along the rectangle's edges. 0 = points snap exactly to the edge."),
        param!("zero", "Zero", bool, false, "When on, points outside the rectangle collapse to the origin. When off, they scatter back to the nearest edge."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_crop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Crop - crops to a rectangular region with optional scatter
    let x0 = get_param(xform_id, variation_id, 0u);
    let y0 = get_param(xform_id, variation_id, 1u);
    let x1 = get_param(xform_id, variation_id, 2u);
    let y1 = get_param(xform_id, variation_id, 3u);
    let scatter = get_param(xform_id, variation_id, 4u);
    let zero = get_param(xform_id, variation_id, 5u);

    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec2<f32>(0.0, 0.0);
    }

    if x < _x0 {
        x = _x0 + rng_nextf(rng) * w;
    } else if x > _x1 {
        x = _x1 - rng_nextf(rng) * w;
    }

    if y < _y0 {
        y = _y0 + rng_nextf(rng) * h;
    } else if y > _y1 {
        y = _y1 - rng_nextf(rng) * h;
    }

    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_crop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Crop - 3D (Z passes through)
    let x0 = get_param(xform_id, variation_id, 0u);
    let y0 = get_param(xform_id, variation_id, 1u);
    let x1 = get_param(xform_id, variation_id, 2u);
    let y1 = get_param(xform_id, variation_id, 3u);
    let scatter = get_param(xform_id, variation_id, 4u);
    let zero = get_param(xform_id, variation_id, 5u);

    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec3<f32>(0.0, 0.0, p.z);
    }

    if x < _x0 {
        x = _x0 + rng_nextf(rng) * w;
    } else if x > _x1 {
        x = _x1 - rng_nextf(rng) * w;
    }

    if y < _y0 {
        y = _y0 + rng_nextf(rng) * h;
    } else if y > _y1 {
        y = _y1 - rng_nextf(rng) * h;
    }

    return vec3<f32>(x, y, p.z);
}
"#,
};
