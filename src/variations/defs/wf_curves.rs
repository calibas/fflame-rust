//! WF-suffix simple curves: epispiral_wf, cloverleaf_wf, rose_wf, bubble_wf, plane_wf
//!
//! Five polar-curve / plane-blur variations from JWildfire's WF
//! (Wildfire) family. All are simple, clean ports — body factors
//! cleanly through the outer multiplier.
//!
//!   - epispiral_wf: epispiral curve `r = 0.5 / cos(waves·a)`.
//!     1 user param `waves` (default 4). Returns (0, 0) when the cosine
//!     hits zero (cpp early-returns leaving FPx/FPy unchanged; closest
//!     equivalent in our outer-multiplier convention is to add nothing).
//!     Preserves cpp's `atan2(x, y)` swap.
//!
//!   - cloverleaf_wf: cloverleaf curve `r = sin(2a) +
//!     0.25·sin(6a)`. 1 user param `filled` (int, default 1). RNG when
//!     filled==1. Preserves cpp's `atan2(x, y)` swap.
//!
//!   - rose_wf: rose curve `r = amp · cos(waves·a)`. 3 user
//!     params (amp default 0.5, waves int default 4, filled int default 0).
//!     RNG when filled==1. Preserves cpp's `atan2(x, y)` swap.
//!
//!   - bubble_wf: standard bubble inversion plus a random
//!     ±z bump (`±(2/r − 1)`). 0 user params. RNG (1 call/iter for
//!     z-sign). Full3D.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/epispiral_wf.cpp`
//!   - `output/jwildfire-vars/output/cloverleaf_wf.cpp`
//!   - `output/jwildfire-vars/output/rose_wf.cpp`
//!   - `output/jwildfire-vars/output/bubble_wf.cpp`
//!   - `output/jwildfire-vars/output/plane_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// epispiral_wf
// ---------------------------------------------------------------------------

/// Epispiral curve — emits a point on the epispiral `r = 0.5 / cos(waves ·
/// a)`. The curve produces `waves` symmetric petals radiating from the
/// origin. Returns `(0, 0)` at the cos-zero singularities.
pub static EPISPIRAL_WF: VariationDef = VariationDef {
    name: "epispiral_wf",
    aliases: &[],
    display_name: "Epispiral WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("waves", "Waves", unlimited_float, 4.0, -50.0, 50.0, "Number of petals (frequency of the cosine denominator)."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_epispiral_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let waves = get_param(xform_id, variation_id, 0u);
    let a = atan2(p.x, p.y);
    let d = cos(waves * a);
    if (abs(d) < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let r = 0.5 / d;
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: r#"
fn variation_epispiral_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let waves = get_param(xform_id, variation_id, 0u);
    let a = atan2(p.x, p.y);
    let d = cos(waves * a);
    if (abs(d) < 1e-30) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let r = 0.5 / d;
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// cloverleaf_wf
// ---------------------------------------------------------------------------

/// Cloverleaf curve — emits a point on `r = sin(2a) + 0.25 · sin(6a)`.
/// Produces a 4-leaf clover shape. With `filled = 1`, randomizes the radius
/// to fill the interior.
pub static CLOVERLEAF_WF: VariationDef = VariationDef {
    name: "cloverleaf_wf",
    aliases: &[],
    display_name: "Cloverleaf WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("filled", "Filled", bool, true, "1 = fill the curve interior by randomizing the radius per iteration; 0 = trace only the curve outline."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cloverleaf_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    let a = atan2(p.x, p.y);
    var r = sin(2.0 * a) + 0.25 * sin(6.0 * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: r#"
fn variation_cloverleaf_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    let a = atan2(p.x, p.y);
    var r = sin(2.0 * a) + 0.25 * sin(6.0 * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// rose_wf
// ---------------------------------------------------------------------------

/// Rose curve — emits a point on the classic rose `r = amp · cos(waves ·
/// a)`. Produces `waves` petals (or `2·waves` if `waves` is even). With
/// `filled = 1`, randomizes the radius to fill the interior.
pub static ROSE_WF: VariationDef = VariationDef {
    name: "rose_wf",
    aliases: &[],
    display_name: "Rose WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("amp", "Amplitude", unlimited_float, 0.5, -10.0, 10.0, "Radial amplitude (multiplies the cosine)."),
        param!("waves", "Waves", int, 4.0, -50.0, 50.0, "Number of petals (`waves` petals if odd, `2·waves` if even)."),
        param!("filled", "Filled", bool, false, "1 = fill the curve interior; 0 = trace only the outline."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_rose_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let amp = get_param(xform_id, variation_id, 0u);
    let waves = get_param(xform_id, variation_id, 1u);
    let filled = i32(get_param(xform_id, variation_id, 2u));
    let a = atan2(p.x, p.y);
    var r = amp * cos(waves * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: r#"
fn variation_rose_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let amp = get_param(xform_id, variation_id, 0u);
    let waves = get_param(xform_id, variation_id, 1u);
    let filled = i32(get_param(xform_id, variation_id, 2u));
    let a = atan2(p.x, p.y);
    var r = amp * cos(waves * a);
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// bubble_wf
// ---------------------------------------------------------------------------

/// Bubble inversion with random Z bump — applies the standard bubble
/// inversion `(x, y) / (1 + r²/4)` to XY, plus a random `±(2/r − 1)` Z bump
/// (sign chosen by coin flip). The XY portion matches the classic `bubble`
/// variation; Z gets per-iteration random spheres above and below the
/// bubble.
pub static BUBBLE_WF: VariationDef = VariationDef {
    name: "bubble_wf",
    aliases: &[],
    display_name: "Bubble WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bubble_wf(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = (p.x * p.x + p.y * p.y) * 0.25 + 1.0;
    let safe_r = select(r, 1e-30, abs(r) < 1e-30);
    let t = 1.0 / safe_r;
    return vec2<f32>(t * p.x, t * p.y);
}
"#,
    wgsl_3d: r#"
fn variation_bubble_wf(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = (p.x * p.x + p.y * p.y) * 0.25 + 1.0;
    let safe_r = select(r, 1e-30, abs(r) < 1e-30);
    let t = 1.0 / safe_r;
    let z_bump = 2.0 / safe_r - 1.0;
    let z = select(z_bump, -z_bump, rng_nextf(rng) < 0.5);
    return vec3<f32>(t * p.x, t * p.y, z);
}
"#,
};

/// Plane scatter — blur-style variation that ignores its input `p`
/// and draws a random point on an axis-aligned plane. Two of the
/// three coordinates come from `(rand − 0.5) × size`; the third is
/// fixed at `position` (which axis is the "fixed" one is controlled
/// by the `axis` param: XY plane → fixed Z, YZ plane → fixed X, ZX
/// plane → fixed Y, default ZX). Optionally writes a direct color
/// scalar to the iteration's `vc` register derived from the two
/// random coordinates (modes U / V / UV map to the first random,
/// second random, or their product); `direct_color = 0` disables
/// the write.
///
/// JWildfire's image-related modes (`CM_COLORMAP`, image
/// displacement, `calc_color_idx`, `receive_only_shadows`) require
/// texture sampling we don't support and are silently no-ops here —
/// the `colormap` mode falls back to the `U` color path so a flame
/// that requested an image colormap still renders something.
///
/// # Authors
/// - Andreas Maschke
pub static PLANE_WF: VariationDef = VariationDef {
    name: "plane_wf",
    aliases: &[],
    display_name: "Plane WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("position", "Position", unlimited_float, 3.0, -100.0, 100.0, "Fixed coordinate along the axis the plane is perpendicular to. For axis=XY the plane sits at z=position; for YZ at x=position; for ZX at y=position."),
        param!("size", "Size", unlimited_float, 10.0, -100.0, 100.0, "Plane edge length. The two free axes get `(random − 0.5) × size` so positive `size` scatters in a square of side `size` centered on the axis."),
        param!("axis", "Axis", enum, 2, &["XY (fix Z)", "YZ (fix X)", "ZX (fix Y)"], "Which two axes form the plane. The third axis is fixed at `position`."),
        param!("direct_color", "Direct Color", bool, true, "Write a direct color scalar to `vc` based on the two random plane coordinates. Off skips the write and lets the standard color evolution carry through."),
        param!("color_mode", "Color Mode", enum, 3, &["Colormap (→ U fallback)", "U", "V", "UV"], "How the direct-color scalar is derived from the two random coordinates u, v ∈ [0,1). `U` and `V` write that coordinate directly, `UV` writes the product, `Colormap` would sample an image in JWildfire (no texture support here — falls back to U)."),
    ],
    needs_transform: false,
    writes_color: true,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_plane_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    // 2D mode: project the chosen plane down to (x, y) — only XY
    // shows the full random rectangle; YZ flattens to a Y-only line
    // (Z is invisible) and ZX flattens to X-only. We keep the same
    // color-write logic so direct-color flames don't disappear in
    // 2D mode just because the variation prefers 3D.
    let position = get_param(xform_id, variation_id, 0u);
    let size = get_param(xform_id, variation_id, 1u);
    let axis = i32(get_param(xform_id, variation_id, 2u));
    let direct_color = i32(get_param(xform_id, variation_id, 3u));
    let color_mode = i32(get_param(xform_id, variation_id, 4u));

    let u = 0.5 - rng_nextf(rng);
    let v = 0.5 - rng_nextf(rng);

    var x = 0.0;
    var y = 0.0;
    if (axis == 0) {
        // XY plane: u→x, v→y.
        x = u * size;
        y = v * size;
    } else if (axis == 1) {
        // YZ plane: u→y, position→x.
        x = position;
        y = u * size;
    } else {
        // ZX plane (default): position→y, u→x.
        x = u * size;
        y = position;
    }

    if (direct_color != 0) {
        var tc = u + 0.5;
        if (color_mode == 2) {
            tc = v + 0.5;
        } else if (color_mode == 3) {
            tc = (u + 0.5) * (v + 0.5);
        }
        // color_mode 0 (Colormap) and 1 (U) both write `u + 0.5`.
        *vc = clamp(tc, 0.0, 1.0);
    }

    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_plane_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let position = get_param(xform_id, variation_id, 0u);
    let size = get_param(xform_id, variation_id, 1u);
    let axis = i32(get_param(xform_id, variation_id, 2u));
    let direct_color = i32(get_param(xform_id, variation_id, 3u));
    let color_mode = i32(get_param(xform_id, variation_id, 4u));

    let u = 0.5 - rng_nextf(rng);
    let v = 0.5 - rng_nextf(rng);

    var x = 0.0;
    var y = 0.0;
    var z = 0.0;
    if (axis == 0) {
        // XY plane: u→x, v→y, z=position.
        x = u * size;
        y = v * size;
        z = position;
    } else if (axis == 1) {
        // YZ plane: x=position, u→y, v→z.
        x = position;
        y = u * size;
        z = v * size;
    } else {
        // ZX plane (default): u→x, y=position, v→z.
        x = u * size;
        y = position;
        z = v * size;
    }

    if (direct_color != 0) {
        var tc = u + 0.5;
        if (color_mode == 2) {
            tc = v + 0.5;
        } else if (color_mode == 3) {
            tc = (u + 0.5) * (v + 0.5);
        }
        *vc = clamp(tc, 0.0, 1.0);
    }

    return vec3<f32>(x, y, z);
}
"#,
};
