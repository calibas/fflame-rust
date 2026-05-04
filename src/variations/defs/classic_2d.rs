//! Classic 2D variations from Apophysis / JWildfire
//!
//! A grab-bag of frequently-used 2D variations: geometric distortions,
//! cropping/blur primitives, and panoramic projections. Each is a faithful
//! port of the upstream C++ plugin (preserving any C++→Java porter quirks
//! so flames built against the C++ ports render the same).
//!
//! Sources (all from
//! https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/tree/master/output ):
//!   - `fan.cpp`          — affine-coefficient–driven fan sweep
//!   - `fisheye.cpp`      — classic fish-eye (X/Y swapped — the original
//!                          Apophysis bug; `eyefish` already in our registry
//!                          is the corrected version)
//!   - `gridout.cpp`      — discrete grid-cell escape pattern
//!   - `circular.cpp`     — Tatyana Zabanova's circular blur
//!   - `panorama1.cpp`    — spherical panoramic projection
//!   - `panorama2.cpp`    — alt-spherical panoramic projection
//!
//! Notes on faithfulness:
//!   - All upstream bodies multiply by `VVAR` internally; we factor it out
//!     so the outer `result += weight * variation(...)` dispatcher applies
//!     it. Algebraically identical at any weight.
//!   - `fan` reads the affine translation coefficients (XFORM_COEFF_20/_21)
//!     to size and offset its sweep; this needs `needs_transform: true` so
//!     the body can read `transforms[xform_id].e/f`.
//!   - `fisheye` preserves the C++ port's X/Y swap on the angular term.
//!     Java intent (and our existing `eyefish`) computes it correctly;
//!     porters of `fisheye` flames want the buggy version.
//!   - `panorama1` and `panorama2` use upstream's `atan2(x1, y1)` ordering
//!     (also a non-standard "x first" form); preserved.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// fan: affine-coefficient–driven fan sweep
//   dx  = π · e²            (e = X-translation of pre-affine, +SMALL_EPSILON)
//   dx2 = dx / 2
//   a   = atan2(x, y) ± dx2 (sign chosen by mod-test against dx; cpp swap
//                            preserved on the atan call)
//   r   = sqrt(x² + y²)
//   out = r · (cos(a), sin(a))   (weight applied outside)
// =============================================================================
pub static FAN: VariationDef = VariationDef {
    name: "fan",
    display_name: "Fan",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
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
fn variation_fan(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xf = transforms[xform_id];
    let dx = 3.14159265358979 * xf.e * xf.e + 1e-6;
    let dx2 = 0.5 * dx;
    let theta = atan2(p.x, p.y);  // upstream atan2(FTx, FTy) — preserved
    let m = theta + xf.f - trunc((theta + xf.f) / dx) * dx;
    var a: f32;
    if (m > dx2) {
        a = theta - dx2;
    } else {
        a = theta + dx2;
    }
    let r = sqrt(p.x * p.x + p.y * p.y);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_fan(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xf = transforms[xform_id];
    let dx = 3.14159265358979 * xf.e * xf.e + 1e-6;
    let dx2 = 0.5 * dx;
    let theta = atan2(p.x, p.y);
    let m = theta + xf.f - trunc((theta + xf.f) / dx) * dx;
    var a: f32;
    if (m > dx2) {
        a = theta - dx2;
    } else {
        a = theta + dx2;
    }
    let r = sqrt(p.x * p.x + p.y * p.y);
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// fisheye: classic fish-eye distortion
//   r' = 2r / (r + 1)     where r = sqrt(x² + y²)
//   out = r'/r · (y, x)   (X/Y SWAPPED — preserved cpp porter bug; the
//                          corrected form is `eyefish` already in our registry)
// =============================================================================
pub static FISHEYE: VariationDef = VariationDef {
    name: "fisheye",
    display_name: "Fisheye",
    category: VariationCategory::Advanced2D,
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
fn variation_fisheye(p: vec2<f32>) -> vec2<f32> {
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let r = 2.0 * r0 / (r0 + 1.0);
    let denom = r0 + 1e-6;
    return vec2<f32>(r * p.y / denom, r * p.x / denom);
}
"#,
    wgsl_3d: Some(r#"
fn variation_fisheye(p: vec3<f32>) -> vec3<f32> {
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let r = 2.0 * r0 / (r0 + 1.0);
    let denom = r0 + 1e-6;
    return vec3<f32>(r * p.y / denom, r * p.x / denom, p.z);
}
"#),
};

// =============================================================================
// gridout: 8-neighbor grid escape (Faber 2007–08, DarkBeam 2017)
//   Compares (rint(x), rint(y)) to chase the point onto a grid cell edge,
//   producing a discrete tile pattern.
// =============================================================================
pub static GRIDOUT: VariationDef = VariationDef {
    name: "gridout",
    display_name: "Gridout",
    category: VariationCategory::Advanced2D,
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
fn variation_gridout(p: vec2<f32>) -> vec2<f32> {
    let x = round(p.x);
    let y = round(p.y);
    var ox = p.x;
    var oy = p.y;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) { ox = p.x + 1.0; } else { oy = p.y + 1.0; }
        } else {
            if (y <= x) { ox = p.x + 1.0; } else { oy = p.y - 1.0; }
        }
    } else {
        if (x > 0.0) {
            if (y >= x) { ox = p.x - 1.0; } else { oy = p.y + 1.0; }
        } else {
            if (y > -x) { ox = p.x - 1.0; } else { oy = p.y - 1.0; }
        }
    }
    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_gridout(p: vec3<f32>) -> vec3<f32> {
    let x = round(p.x);
    let y = round(p.y);
    var ox = p.x;
    var oy = p.y;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) { ox = p.x + 1.0; } else { oy = p.y + 1.0; }
        } else {
            if (y <= x) { ox = p.x + 1.0; } else { oy = p.y - 1.0; }
        }
    } else {
        if (x > 0.0) {
            if (y >= x) { ox = p.x - 1.0; } else { oy = p.y + 1.0; }
        } else {
            if (y > -x) { ox = p.x - 1.0; } else { oy = p.y - 1.0; }
        }
    }
    return vec3<f32>(ox, oy, p.z);
}
"#),
};

// =============================================================================
// circular: Tatyana Zabanova's randomized rotation by deterministic + RNG term
//   c_a = angle (deg → rad)
//   aux = fract(sin(12.9898·x + 78.233·y + seed) · 43758.5453)
//   rnd = (2 · (rand() + aux) − 2) · c_a
//   out = (cos(θ + rnd), sin(θ + rnd)) · r
// =============================================================================
pub static CIRCULAR: VariationDef = VariationDef {
    name: "circular",
    display_name: "Circular",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("angle", "Angle", angle, 90.0),
        param!("seed", "Seed", unlimited_float, 0.0, -100.0, 100.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_circular(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let angle_deg = get_param(xform_id, variation_id, 0u);
    let seed = get_param(xform_id, variation_id, 1u);
    let c_a = angle_deg * 3.14159265358979 / 180.0;
    let h = sin(p.x * 12.9898 + p.y * 78.233 + seed) * 43758.5453;
    let aux = h - trunc(h);
    let rnd = (2.0 * (rng_nextf(rng) + aux) - 2.0) * c_a;
    let rad = sqrt(p.x * p.x + p.y * p.y);
    let ang = atan2(p.y, p.x);
    return vec2<f32>(cos(ang + rnd) * rad, sin(ang + rnd) * rad);
}
"#,
    wgsl_3d: Some(r#"
fn variation_circular(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let angle_deg = get_param(xform_id, variation_id, 0u);
    let seed = get_param(xform_id, variation_id, 1u);
    let c_a = angle_deg * 3.14159265358979 / 180.0;
    let h = sin(p.x * 12.9898 + p.y * 78.233 + seed) * 43758.5453;
    let aux = h - trunc(h);
    let rnd = (2.0 * (rng_nextf(rng) + aux) - 2.0) * c_a;
    let rad = sqrt(p.x * p.x + p.y * p.y);
    let ang = atan2(p.y, p.x);
    return vec3<f32>(cos(ang + rnd) * rad, sin(ang + rnd) * rad, p.z);
}
"#),
};

// =============================================================================
// panorama1: spherical-style projection
//   aux = 1 / sqrt(x² + y² + 1)
//   x1, y1 = aux · (x, y)
//   out = (atan2(x1, y1)/π, sqrt(x1² + y1²) − 0.5)
// =============================================================================
pub static PANORAMA1: VariationDef = VariationDef {
    name: "panorama1",
    display_name: "Panorama 1",
    category: VariationCategory::Advanced2D,
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
fn variation_panorama1(p: vec2<f32>) -> vec2<f32> {
    let aux = 1.0 / sqrt(p.x * p.x + p.y * p.y + 1.0);
    let x1 = p.x * aux;
    let y1 = p.y * aux;
    let r1 = sqrt(x1 * x1 + y1 * y1);
    return vec2<f32>(atan2(x1, y1) * 0.31830988618379, r1 - 0.5);
}
"#,
    wgsl_3d: Some(r#"
fn variation_panorama1(p: vec3<f32>) -> vec3<f32> {
    let aux = 1.0 / sqrt(p.x * p.x + p.y * p.y + 1.0);
    let x1 = p.x * aux;
    let y1 = p.y * aux;
    let r1 = sqrt(x1 * x1 + y1 * y1);
    return vec3<f32>(atan2(x1, y1) * 0.31830988618379, r1 - 0.5, p.z);
}
"#),
};

// =============================================================================
// panorama2: alt-spherical (denom uses sqrt(x²+y²)+1 instead of sqrt(x²+y²+1))
// =============================================================================
pub static PANORAMA2: VariationDef = VariationDef {
    name: "panorama2",
    display_name: "Panorama 2",
    category: VariationCategory::Advanced2D,
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
fn variation_panorama2(p: vec2<f32>) -> vec2<f32> {
    let aux = 1.0 / (sqrt(p.x * p.x + p.y * p.y) + 1.0);
    let x1 = p.x * aux;
    let y1 = p.y * aux;
    let r1 = sqrt(x1 * x1 + y1 * y1);
    return vec2<f32>(atan2(x1, y1) * 0.31830988618379, r1 - 0.5);
}
"#,
    wgsl_3d: Some(r#"
fn variation_panorama2(p: vec3<f32>) -> vec3<f32> {
    let aux = 1.0 / (sqrt(p.x * p.x + p.y * p.y) + 1.0);
    let x1 = p.x * aux;
    let y1 = p.y * aux;
    let r1 = sqrt(x1 * x1 + y1 * y1);
    return vec3<f32>(atan2(x1, y1) * 0.31830988618379, r1 - 0.5, p.z);
}
"#),
};
