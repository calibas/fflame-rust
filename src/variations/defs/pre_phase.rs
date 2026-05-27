//! Pre-phase variations
//!
//! Pre-phase variations directly modify the input coordinates before normal variations.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Scales the Z coordinate before the rest of the variations run. The
/// variation's weight is the scale factor — weight 2.0 doubles depth, 0.5
/// halves it.
pub static PRE_ZSCALE: VariationDef = VariationDef {
    name: "pre_zscale",
    display_name: "Pre-ZScale",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_zscale(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Pre_ZScale only affects Z (3D mode), pass through in 2D
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_zscale(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis: Pre-phase Z scaling. Variation weight is the scale factor.
    let weight = transforms[xform_id].variations[variation_id];
    return vec3<f32>(p.x, p.y, p.z * weight);
}
"#),
};

/// Shifts the Z coordinate up or down before the rest of the variations
/// run. The variation's weight is the offset.
pub static PRE_ZTRANSLATE: VariationDef = VariationDef {
    name: "pre_ztranslate",
    display_name: "Pre-ZTranslate",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_ztranslate(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Pre_ZTranslate only affects Z (3D mode), pass through in 2D
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_ztranslate(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis: Pre-phase Z translation. Variation weight is the offset.
    let weight = transforms[xform_id].variations[variation_id];
    return vec3<f32>(p.x, p.y, p.z + weight);
}
"#),
};

/// Same math as Spherical (inverts through the unit circle) but runs before
/// the rest of the variations instead of contributing to the weighted sum.
///
/// # Authors
/// - Scott Draves
pub static PRE_SPHERICAL: VariationDef = VariationDef {
    name: "pre_spherical",
    display_name: "Pre-Spherical",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
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
fn variation_pre_spherical(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Pre-Spherical: Pre-phase spherical distortion
    let r = 1.0 / (dot(p, p) + 1e-5);
    return p * r;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_spherical(p: vec3<f32>) -> vec3<f32> {
    // Apophysis Pre-Spherical: Pre-phase spherical distortion (3D)
    let r = 1.0 / (dot(p.xy, p.xy) + 1e-5);
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#),
};

/// Same math as Sinusoidal (sine on each axis) but runs before the rest of
/// the variations. The variation's weight scales the output.
///
/// # Authors
/// - Scott Draves
pub static PRE_SINUSOIDAL: VariationDef = VariationDef {
    name: "pre_sinusoidal",
    display_name: "Pre-Sinusoidal",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_sinusoidal(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Pre-Sinusoidal: Pre-phase sinusoidal wave
    let weight = transforms[xform_id].variations[variation_id];
    return vec2<f32>(weight * sin(p.x), weight * sin(p.y));
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_sinusoidal(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Pre-Sinusoidal: Pre-phase sinusoidal wave (3D)
    let weight = transforms[xform_id].variations[variation_id];
    return vec3<f32>(weight * sin(p.x), weight * sin(p.y), weight * p.z);
}
"#),
};

/// Same math as Disc (wraps the plane onto a disc) but runs before the rest
/// of the variations. The variation's weight scales the result.
///
/// # Authors
/// - Scott Draves
pub static PRE_DISC: VariationDef = VariationDef {
    name: "pre_disc",
    display_name: "Pre-Disc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_disc(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Pre-Disc: Pre-phase disc transformation
    const PI: f32 = 3.14159265359;
    let weight = transforms[xform_id].variations[variation_id];
    let rad = sqrt(dot(p, p));
    let r = (weight / PI) * atan2(p.x, p.y);
    return vec2<f32>(sin(PI * rad) * r, cos(PI * rad) * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_disc(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Pre-Disc: Pre-phase disc transformation (3D)
    const PI: f32 = 3.14159265359;
    let weight = transforms[xform_id].variations[variation_id];
    let rad = sqrt(dot(p.xy, p.xy));
    let r = (weight / PI) * atan2(p.x, p.y);
    return vec3<f32>(sin(PI * rad) * r, cos(PI * rad) * r, weight * p.z);
}
"#),
};

/// Wraps the plane into a grid of soft bubbles, each with its own internal
/// twist. Runs before the rest of the variations so the rest see the
/// bubbled coordinates.
pub static PRE_BWRAPS: VariationDef = VariationDef {
    name: "pre_bwraps",
    display_name: "Pre Bwraps",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, -10.0, 10.0, "Width of each grid cell — the plane is divided into cells of this size, each becoming a bubble."),
        param!("space", "Space", unlimited_float, 0.0, -1.0, 1.0, "Gap between cells. 0 = no gap; positive values push the bubbles apart."),
        param!("gain", "Gain", unlimited_float, 1.0, -5.0, 5.0, "How strongly each bubble wraps its contents inward."),
        param!("inner_twist", "Inner Twist", unlimited_float, 0.0, -10.0, 10.0, "Rotation (in degrees) applied at the center of each bubble."),
        param!("outer_twist", "Outer Twist", unlimited_float, 0.0, -10.0, 10.0, "Rotation (in degrees) applied at the edge of each bubble."),
    ],
    // 5 derived values at slots 5..10:
    //   5: g2                  (gain² / (radius + ε) + ε)
    //   6: r2                  (radius²)
    //   7: rfactor             (radius / max_bubble)
    //   8: inner_twist_rad     (inner_twist · π/180)
    //   9: outer_twist_rad     (outer_twist · π/180)
    needs_transform: false,
    writes_color: false,
    init_param_count: 5,
    wgsl_init: Some(r#"
fn init_pre_bwraps(user: array<f32, 5>) -> array<f32, 5> {
    let cellsize = user[0];
    let space = user[1];
    let gain = user[2];
    let inner_twist = user[3];
    let outer_twist = user[4];
    var out: array<f32, 5>;
    if (cellsize == 0.0) {
        out[0] = 0.0; out[1] = 0.0; out[2] = 0.0;
        out[3] = inner_twist * 3.14159265358979 / 180.0;
        out[4] = outer_twist * 3.14159265358979 / 180.0;
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
    out[3] = inner_twist * 3.14159265358979 / 180.0;
    out[4] = outer_twist * 3.14159265358979 / 180.0;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_bwraps(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let inner_twist = get_param(xform_id, variation_id, 8u);
    let outer_twist = get_param(xform_id, variation_id, 9u);

    if cellsize == 0.0 {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if (lx * lx + ly * ly) <= r2 {
        lx = lx * g2;
        ly = ly * g2;

        var r = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);

        lx = lx * r;
        ly = ly * r;

        r = (lx * lx + ly * ly) / r2;
        let theta = inner_twist * (1.0 - r) + outer_twist * r;

        let s = sin(theta);
        let c = cos(theta);

        return vec2<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly
        );
    }

    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_bwraps(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let inner_twist = get_param(xform_id, variation_id, 8u);
    let outer_twist = get_param(xform_id, variation_id, 9u);

    if cellsize == 0.0 {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if (lx * lx + ly * ly) <= r2 {
        lx = lx * g2;
        ly = ly * g2;

        var r = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);

        lx = lx * r;
        ly = ly * r;

        r = (lx * lx + ly * ly) / r2;
        let theta = inner_twist * (1.0 - r) + outer_twist * r;

        let s = sin(theta);
        let c = cos(theta);

        return vec3<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly,
            p.z
        );
    }

    return p;
}
"#),
};

/// Constrains points to a rectangle before the rest of the variations run.
/// Points outside the rectangle either collapse to zero or get scattered
/// along the nearest edge.
/// 
/// # Authors
/// - Xyrus02
pub static PRE_CROP: VariationDef = VariationDef {
    name: "pre_crop",
    display_name: "Pre Crop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: true,
    parameters: &[
        param!("left", "Left", unlimited_float, -1.0, -5.0, 5.0, "Left edge of the rectangle the points are constrained to."),
        param!("top", "Top", unlimited_float, -1.0, -5.0, 5.0, "Top edge of the rectangle."),
        param!("right", "Right", unlimited_float, 1.0, -5.0, 5.0, "Right edge of the rectangle."),
        param!("bottom", "Bottom", unlimited_float, 1.0, -5.0, 5.0, "Bottom edge of the rectangle."),
        param!("scatter_area", "Scatter Area", float, 0.0, -1.0, 1.0, "Width of the random scatter band along the rectangle's edges. 0 = points snap exactly to the edge."),
        param!("zero", "Zero", bool, false, "When on, points outside the rectangle collapse to the origin. When off, they scatter back to the nearest edge."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_crop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Pre_Crop - same as crop but applied before variations
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
    wgsl_3d: Some(r#"
fn variation_pre_crop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Pre_Crop - 3D (Z passes through)
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
"#),
};

/// Adds random scatter that varies with distance from a chosen center
/// point. Closer points get less scatter (or more, with `invert`); the
/// random distribution shape is selectable. Runs before the rest of the
/// variations.
pub static PRE_FALLOFF2: VariationDef = VariationDef {
    name: "pre_falloff2",
    display_name: "Pre Falloff2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: true,
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
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_falloff2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Pre_Falloff2 - Distance-based scatter with multiple blur modes
    const PI: f32 = 3.14159265359;

    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blurtype = get_param(xform_id, variation_id, 10u);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let dist = sqrt(dx * dx + dy * dy);

    var factor: f32;
    if invert > 0.5 {
        factor = select(1.0, dist / mindist, dist < mindist);
    } else {
        factor = select(1.0, mindist / dist, dist > mindist);
    }

    var sx: f32;
    var sy: f32;

    let blurtype_int = i32(blurtype + 0.5);

    if blurtype_int == 0 {
        let r = scatter * factor;
        let angle = rng_nextf(rng) * 2.0 * PI;
        let d = rng_nextf(rng) * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    } else if blurtype_int == 1 {
        let r = scatter * factor;
        let angle = rng_nextf(rng) * 2.0 * PI;
        let d = (rng_nextf(rng) + rng_nextf(rng)) * 0.5 * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    } else {
        let r = scatter * factor;
        let angle = rng_nextf(rng) * 2.0 * PI;
        let d = sqrt(-log(rng_nextf(rng) + 1e-10)) * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    }

    return vec2<f32>(p.x + sx * mul_x, p.y + sy * mul_y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_falloff2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Pre_Falloff2 - Distance-based scatter with multiple blur modes (3D)
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
    let blurtype = get_param(xform_id, variation_id, 10u);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let dz = p.z - z0;
    let dist = sqrt(dx * dx + dy * dy + dz * dz);

    var factor: f32;
    if invert > 0.5 {
        factor = select(1.0, dist / mindist, dist < mindist);
    } else {
        factor = select(1.0, mindist / dist, dist > mindist);
    }

    var sx: f32;
    var sy: f32;
    var sz: f32;

    let blurtype_int = i32(blurtype + 0.5);

    if blurtype_int == 0 {
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = rng_nextf(rng) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else if blurtype_int == 1 {
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = (rng_nextf(rng) + rng_nextf(rng)) * 0.5 * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else {
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = sqrt(-log(rng_nextf(rng) + 1e-10)) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    }

    return vec3<f32>(p.x + sx * mul_x, p.y + sy * mul_y, p.z + sz * mul_z);
}
"#),
};
