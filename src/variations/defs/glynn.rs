//! Glynn family — Faber's "Glynnia" + eralex61's "GlynnSim" set
//!
//! Five popular organic-shape variations from JWildfire:
//!   - `glynnia`   (Michael Faber)  — RNG branch on radius and on θ-quadrant
//!   - `glynnia3`                   — glynnia with 4 user-tunable knobs
//!   - `glynnSim1` (eralex61)       — Glynn-set inversion + circle generator
//!   - `glynnSim2` (eralex61)       — Glynn-set with arc-segment generator
//!   - `glynnSim3` (eralex61)       — Glynn-set with two-radius branching
//!
//! Sources:
//!   - output/jwildfire-vars/output/glynnia.cpp
//!   - output/jwildfire-vars/output/glynnia3.cpp
//!   - output/jwildfire-vars/output/glynnSim1.cpp
//!   - output/jwildfire-vars/output/glynnSim2.cpp
//!   - output/jwildfire-vars/output/glynnSim3.cpp
//!
//! Notes on faithfulness:
//!   - All factor VVAR through the outer-multiplier convention. The
//!     internal `_vvar2 = VVAR · sqrt(2)/2` of the glynnia variants
//!     becomes `sqrt(2)/2` here; the outer multiplier restores the
//!     weight scaling.
//!   - The C++ ports of glynnSim1/2/3 declare a `circle()` helper that
//!     takes `Point p` BY VALUE — writes inside the helper don't reach
//!     the caller, so the C++ version reads stale `_toolPoint` state.
//!     This is a porter bug; the Java original passes `Point` by
//!     reference (Java objects are reference types) and the helper
//!     correctly mutates the shared point. We follow the Java
//!     semantics by inlining the helper into the body — flames built
//!     against the JWildfire Java engine will render the same; flames
//!     built against the buggy C++ port will differ.
//!   - `_absPow` etc. that depend only on user params live in init
//!     slots; per-iteration math keeps user-param reads cheap.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// glynnia: random-branched glynn-set warp (Michael Faber)
//   r = sqrt(x² + y²)
//   if r >= 1:
//     if rand > 0.5:   out = (sqrt(2)/2) · (sqrt(r+x), -y/sqrt(r+x))
//     else:            d = r+x; dx = sqrt(r·(y² + d²));  out = (d/dx, y/dx)
//   else (r < 1):
//     if rand > 0.5:   out = -(sqrt(2)/2) · (sqrt(r+x), y/sqrt(r+x))
//     else:            d = r+x; dx = sqrt(r·(y² + d²));  out = (-d/dx, y/dx)
// =============================================================================
/// Random-branched Glynn-set warp — splits into 4 branches per iteration
/// based on radius and a random coin flip. Produces the characteristic
/// organic Glynn fractal shapes.
///
/// # Authors
/// - Michael Faber
pub static GLYNNIA: VariationDef = VariationDef {
    name: "glynnia",
    aliases: &[],
    display_name: "Glynnia",
    category: VariationCategory::Advanced2D,
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
fn variation_glynnia(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let half_sqrt2 = 0.7071067811865476;
    let r = sqrt(p.x * p.x + p.y * p.y);

    if (r >= 1.0) {
        if (rng_nextf(rng) > 0.5) {
            let d = sqrt(max(r + p.x, 0.0));
            if (d == 0.0) { return vec2<f32>(0.0, 0.0); }
            return vec2<f32>(half_sqrt2 * d, -half_sqrt2 / d * p.y);
        }
        let d = r + p.x;
        let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
        if (dx == 0.0) { return vec2<f32>(0.0, 0.0); }
        return vec2<f32>(d / dx, p.y / dx);
    }

    if (rng_nextf(rng) > 0.5) {
        let d = sqrt(max(r + p.x, 0.0));
        if (d == 0.0) { return vec2<f32>(0.0, 0.0); }
        return vec2<f32>(-half_sqrt2 * d, -half_sqrt2 / d * p.y);
    }
    let d = r + p.x;
    let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
    if (dx == 0.0) { return vec2<f32>(0.0, 0.0); }
    return vec2<f32>(-d / dx, p.y / dx);
}
"#,
    wgsl_3d: Some(r#"
fn variation_glynnia(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let half_sqrt2 = 0.7071067811865476;
    let r = sqrt(p.x * p.x + p.y * p.y);

    if (r >= 1.0) {
        if (rng_nextf(rng) > 0.5) {
            let d = sqrt(max(r + p.x, 0.0));
            if (d == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
            return vec3<f32>(half_sqrt2 * d, -half_sqrt2 / d * p.y, p.z);
        }
        let d = r + p.x;
        let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
        if (dx == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
        return vec3<f32>(d / dx, p.y / dx, p.z);
    }

    if (rng_nextf(rng) > 0.5) {
        let d = sqrt(max(r + p.x, 0.0));
        if (d == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
        return vec3<f32>(-half_sqrt2 * d, -half_sqrt2 / d * p.y, p.z);
    }
    let d = r + p.x;
    let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
    if (dx == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
    return vec3<f32>(-d / dx, p.y / dx, p.z);
}
"#),
};

// =============================================================================
// glynnia3: glynnia with 4 user-tunable knobs (Michael Faber, Maulana Randa, CozyG)
//   Like glynnia but:
//     r = rscale · sqrt(x² + y²)
//     d = dscale · (r + x)   (or sqrt(d) where glynnia took sqrt directly)
//     branch threshold: r > rthresh && y > ythresh   (vs r >= 1)
// =============================================================================
/// Glynnia with 4 user-tunable knobs — exposes the radius/distance scaling
/// and the branch threshold as parameters.
///
/// # Authors
/// - Michael Faber
/// - Maulana Randa
/// - CozyG
pub static GLYNNIA3: VariationDef = VariationDef {
    name: "glynnia3",
    aliases: &[],
    display_name: "Glynnia 3",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("rscale", "R scale", unlimited_float, 1.0, -10.0, 10.0, "Radial scaling on the input distance."),
        param!("dscale", "D scale", unlimited_float, 1.0, -10.0, 10.0, "Scaling on the `d = r + x` term used by both branches."),
        param!("rthresh", "R threshold", unlimited_float, 0.0, -10.0, 10.0, "Radius threshold for the inside-vs-outside branch split."),
        param!("ythresh", "Y threshold", unlimited_float, 0.0, -10.0, 10.0, "Y threshold added to the branch condition — only points with `y > ythresh` take the outer branch."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_glynnia3(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let rscale = get_param(xform_id, variation_id, 0u);
    let dscale = get_param(xform_id, variation_id, 1u);
    let rthresh = get_param(xform_id, variation_id, 2u);
    let ythresh = get_param(xform_id, variation_id, 3u);
    let half_sqrt2 = 0.7071067811865476;

    let r = rscale * sqrt(p.x * p.x + p.y * p.y);

    if (r > rthresh && p.y > ythresh) {
        if (rng_nextf(rng) > 0.5) {
            let d = dscale * sqrt(max(r + p.x, 0.0));
            if (d == 0.0) { return vec2<f32>(0.0, 0.0); }
            return vec2<f32>(half_sqrt2 * d, -half_sqrt2 / d * p.y);
        }
        let d = dscale * (r + p.x);
        let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
        if (dx == 0.0) { return vec2<f32>(0.0, 0.0); }
        return vec2<f32>(d / dx, p.y / dx);
    }

    if (rng_nextf(rng) > 0.5) {
        let d = dscale * sqrt(max(r + p.x, 0.0));
        if (d == 0.0) { return vec2<f32>(0.0, 0.0); }
        return vec2<f32>(-half_sqrt2 * d, -half_sqrt2 / d * p.y);
    }
    let d = dscale * (r + p.x);
    let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
    if (dx == 0.0) { return vec2<f32>(0.0, 0.0); }
    return vec2<f32>(-d / dx, p.y / dx);
}
"#,
    wgsl_3d: Some(r#"
fn variation_glynnia3(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let rscale = get_param(xform_id, variation_id, 0u);
    let dscale = get_param(xform_id, variation_id, 1u);
    let rthresh = get_param(xform_id, variation_id, 2u);
    let ythresh = get_param(xform_id, variation_id, 3u);
    let half_sqrt2 = 0.7071067811865476;

    let r = rscale * sqrt(p.x * p.x + p.y * p.y);

    if (r > rthresh && p.y > ythresh) {
        if (rng_nextf(rng) > 0.5) {
            let d = dscale * sqrt(max(r + p.x, 0.0));
            if (d == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
            return vec3<f32>(half_sqrt2 * d, -half_sqrt2 / d * p.y, p.z);
        }
        let d = dscale * (r + p.x);
        let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
        if (dx == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
        return vec3<f32>(d / dx, p.y / dx, p.z);
    }

    if (rng_nextf(rng) > 0.5) {
        let d = dscale * sqrt(max(r + p.x, 0.0));
        if (d == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
        return vec3<f32>(-half_sqrt2 * d, -half_sqrt2 / d * p.y, p.z);
    }
    let d = dscale * (r + p.x);
    let dx = sqrt(max(r * (p.y * p.y + d * d), 0.0));
    if (dx == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
    return vec3<f32>(-d / dx, p.y / dx, p.z);
}
"#),
};

// =============================================================================
// glynnSim1: Glynn-set inversion + offset-circle generator (eralex61)
//   Init: x1 = radius·cos(π·phi1/180), y1 = radius·sin(π·phi1/180), absPow = |pow|
//   Body:
//     r = sqrt(x²+y²);  α = radius/r
//     if r < radius:               (inside the radius — generate a new point on
//                                   the offset circle)
//        out = circle(rng)
//     else:
//        if rand > contrast · α^|pow|:  tp = (x, y)
//        else:                          tp = α² · (x, y)
//        if (tp - (x1,y1))² < radius1²:  out = circle(rng)
//        else:                            out = tp
//   circle(rng):
//     r2 = radius1 · (thickness + (1−thickness) · rand)
//     φ  = 2π · rand
//     out = (r2·cos(φ) + x1, r2·sin(φ) + y1)
// =============================================================================
/// Glynn-set inversion with an offset-circle generator — points inside the
/// radius spawn new points on a circle at `(x1, y1)`; outside, points get
/// inverted with probabilistic threshold.
///
/// # Authors
/// - eralex61
pub static GLYNN_SIM1: VariationDef = VariationDef {
    name: "glynnSim1",
    aliases: &[],
    display_name: "Glynn Sim 1",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -5.0, 5.0, "Main Glynn-set inversion radius."),
        param!("radius1", "Radius 1", unlimited_float, 0.1, -5.0, 5.0, "Offset-circle radius for the generator."),
        param!("phi1", "Phi 1", unlimited_float, 110.0, -360.0, 360.0, "Angular position of the offset circle (degrees)."),
        param!("thickness", "Thickness", float, 0.1, 0.0, 1.0, "Circle thickness — fraction of the radius (0 = points on the boundary, 1 = filled disc)."),
        param!("pow", "Pow", unlimited_float, 1.5, -10.0, 10.0, "Power for the contrast probability — higher concentrates points near the boundary."),
        param!("contrast", "Contrast", float, 0.5, 0.0, 1.0, "Probability scaling for the inversion-vs-pass-through branch."),
    ],
    needs_transform: false,
    writes_color: false,
    // 3 derived values at slots 6..9:
    //   6: x1     radius · cos(π·phi1/180)
    //   7: y1     radius · sin(π·phi1/180)
    //   8: absPow |pow|
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_glynnSim1(user: array<f32, 6>) -> array<f32, 3> {
    let radius = user[0];
    let phi1 = user[2];
    let pow_p = user[4];
    let a = 3.14159265358979 * phi1 / 180.0;
    var out: array<f32, 3>;
    out[0] = radius * cos(a);
    out[1] = radius * sin(a);
    out[2] = abs(pow_p);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_glynnSim1(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let radius1 = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 3u);
    let contrast = get_param(xform_id, variation_id, 5u);
    let x1 = get_param(xform_id, variation_id, 6u);
    let y1 = get_param(xform_id, variation_id, 7u);
    let abs_pow = get_param(xform_id, variation_id, 8u);

    let r = sqrt(p.x * p.x + p.y * p.y);
    let alpha = radius / max(r, 1e-30);

    var tx = p.x;
    var ty = p.y;
    var generate = false;
    if (r < radius) {
        generate = true;
    } else {
        if (rng_nextf(rng) > contrast * pow(alpha, abs_pow)) {
            tx = p.x;
            ty = p.y;
        } else {
            tx = alpha * alpha * p.x;
            ty = alpha * alpha * p.y;
        }
        let zd = (tx - x1) * (tx - x1) + (ty - y1) * (ty - y1);
        if (zd < radius1 * radius1) {
            generate = true;
        }
    }
    if (generate) {
        let rr = radius1 * (thickness + (1.0 - thickness) * rng_nextf(rng));
        let phi = 6.28318530717959 * rng_nextf(rng);
        tx = rr * cos(phi) + x1;
        ty = rr * sin(phi) + y1;
    }
    return vec2<f32>(tx, ty);
}
"#,
    wgsl_3d: Some(r#"
fn variation_glynnSim1(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let radius1 = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 3u);
    let contrast = get_param(xform_id, variation_id, 5u);
    let x1 = get_param(xform_id, variation_id, 6u);
    let y1 = get_param(xform_id, variation_id, 7u);
    let abs_pow = get_param(xform_id, variation_id, 8u);

    let r = sqrt(p.x * p.x + p.y * p.y);
    let alpha = radius / max(r, 1e-30);

    var tx = p.x;
    var ty = p.y;
    var generate = false;
    if (r < radius) {
        generate = true;
    } else {
        if (rng_nextf(rng) > contrast * pow(alpha, abs_pow)) {
            tx = p.x;
            ty = p.y;
        } else {
            tx = alpha * alpha * p.x;
            ty = alpha * alpha * p.y;
        }
        let zd = (tx - x1) * (tx - x1) + (ty - y1) * (ty - y1);
        if (zd < radius1 * radius1) {
            generate = true;
        }
    }
    if (generate) {
        let rr = radius1 * (thickness + (1.0 - thickness) * rng_nextf(rng));
        let phi = 6.28318530717959 * rng_nextf(rng);
        tx = rr * cos(phi) + x1;
        ty = rr * sin(phi) + y1;
    }
    return vec3<f32>(tx, ty, p.z);
}
"#),
};

// =============================================================================
// glynnSim2: Glynn-set with arc-segment generator (eralex61)
//   Init: phi10 = π·phi1/180, phi20 = π·phi2/180
//         γ = thickness · (2·radius + thickness) / (radius + thickness)
//         δ = phi20 − phi10
//         absPow = |pow|
//   Body:
//     r = sqrt(x²+y²);  α = radius/r
//     if r < radius:    out = arc_circle(rng)
//     else:
//        if rand > contrast · α^|pow|:  out = (x, y)
//        else:                          out = α² · (x, y)
//   arc_circle(rng):
//     r2 = radius + thickness − γ · rand
//     φ  = phi10 + δ · rand          (segment of an arc, not full circle)
//     out = (r2·cos(φ), r2·sin(φ))
// =============================================================================
/// Glynn-set with arc-segment generator — like GlynnSim1 but the generator
/// spawns points along an arc instead of a full circle. `phi1` / `phi2`
/// define the arc bounds.
///
/// # Authors
/// - eralex61
pub static GLYNN_SIM2: VariationDef = VariationDef {
    name: "glynnSim2",
    aliases: &[],
    display_name: "Glynn Sim 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -5.0, 5.0, "Main Glynn-set inversion radius."),
        param!("thickness", "Thickness", float, 0.1, 0.0, 1.0, "Arc thickness — fraction of the radius."),
        param!("contrast", "Contrast", float, 0.5, 0.0, 1.0, "Probability scaling for the inversion-vs-pass-through branch."),
        param!("pow", "Pow", unlimited_float, 1.5, -10.0, 10.0, "Power for the contrast probability."),
        param!("phi1", "Phi 1", unlimited_float, 110.0, -360.0, 360.0, "Start angle of the arc segment (degrees)."),
        param!("phi2", "Phi 2", unlimited_float, 150.0, -360.0, 360.0, "End angle of the arc segment (degrees)."),
    ],
    needs_transform: false,
    writes_color: false,
    // 5 derived values at slots 6..11:
    //   6: phi10
    //   7: gamma
    //   8: delta = phi20 − phi10
    //   9: absPow = |pow|
    //  10: rad_plus_thick = radius + thickness   (precomputed for arc circle r2)
    init_param_count: 5,
    wgsl_init: Some(r#"
fn init_glynnSim2(user: array<f32, 6>) -> array<f32, 5> {
    let radius = user[0];
    let thickness = user[1];
    let pow_p = user[3];
    let phi1 = user[4];
    let phi2 = user[5];
    let pi180 = 3.14159265358979 / 180.0;
    let phi10 = pi180 * phi1;
    let phi20 = pi180 * phi2;
    var out: array<f32, 5>;
    out[0] = phi10;
    out[1] = thickness * (2.0 * radius + thickness) / max(radius + thickness, 1e-30);
    out[2] = phi20 - phi10;
    out[3] = abs(pow_p);
    out[4] = radius + thickness;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_glynnSim2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let contrast = get_param(xform_id, variation_id, 2u);
    let phi10 = get_param(xform_id, variation_id, 6u);
    let gamma = get_param(xform_id, variation_id, 7u);
    let delta = get_param(xform_id, variation_id, 8u);
    let abs_pow = get_param(xform_id, variation_id, 9u);
    let rad_plus_thick = get_param(xform_id, variation_id, 10u);

    let r = sqrt(p.x * p.x + p.y * p.y);
    let alpha = radius / max(r, 1e-30);

    if (r < radius) {
        let r2 = rad_plus_thick - gamma * rng_nextf(rng);
        let phi = phi10 + delta * rng_nextf(rng);
        return vec2<f32>(r2 * cos(phi), r2 * sin(phi));
    }
    if (rng_nextf(rng) > contrast * pow(alpha, abs_pow)) {
        return p;
    }
    return vec2<f32>(alpha * alpha * p.x, alpha * alpha * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_glynnSim2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let contrast = get_param(xform_id, variation_id, 2u);
    let phi10 = get_param(xform_id, variation_id, 6u);
    let gamma = get_param(xform_id, variation_id, 7u);
    let delta = get_param(xform_id, variation_id, 8u);
    let abs_pow = get_param(xform_id, variation_id, 9u);
    let rad_plus_thick = get_param(xform_id, variation_id, 10u);

    let r = sqrt(p.x * p.x + p.y * p.y);
    let alpha = radius / max(r, 1e-30);

    if (r < radius) {
        let r2 = rad_plus_thick - gamma * rng_nextf(rng);
        let phi = phi10 + delta * rng_nextf(rng);
        return vec3<f32>(r2 * cos(phi), r2 * sin(phi), p.z);
    }
    if (rng_nextf(rng) > contrast * pow(alpha, abs_pow)) {
        return p;
    }
    return vec3<f32>(alpha * alpha * p.x, alpha * alpha * p.y, p.z);
}
"#),
};

// =============================================================================
// glynnSim3: Glynn-set with two-radius branching (eralex61)
//   Init: r1 = radius + thickness;  r2 = radius² / r1
//         γ  = r1 / (r1 + r2);      absPow = |pow|
//   Body:
//     r = sqrt(x²+y²);  α = radius/r
//     if r < r1:    out = circle2(rng)         (random circle: r1 with prob γ
//                                               else r2)
//     else:
//        if rand > contrast · α^|pow|:  out = (x, y)
//        else:                          out = α² · (x, y)
// =============================================================================
/// Glynn-set with two-radius branching — generator picks between two
/// circles (`r1` and `r2 = radius²/r1`) with probability γ.
///
/// # Authors
/// - eralex61
pub static GLYNN_SIM3: VariationDef = VariationDef {
    name: "glynnSim3",
    aliases: &[],
    display_name: "Glynn Sim 3",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -5.0, 5.0, "Main Glynn-set inversion radius."),
        param!("thickness", "Thickness", float, 0.1, 0.0, 1.0, "Outer-circle thickness — combined with radius to form `r1` and `r2 = radius² / r1`."),
        param!("contrast", "Contrast", float, 0.5, 0.0, 1.0, "Probability scaling for the inversion-vs-pass-through branch."),
        param!("pow", "Pow", unlimited_float, 1.5, -10.0, 10.0, "Power for the contrast probability."),
    ],
    needs_transform: false,
    writes_color: false,
    // 4 derived values at slots 4..8:
    //   4: r1     radius + thickness
    //   5: r2     radius² / r1
    //   6: gamma  r1 / (r1 + r2)
    //   7: absPow |pow|
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_glynnSim3(user: array<f32, 4>) -> array<f32, 4> {
    let radius = user[0];
    let thickness = user[1];
    let pow_p = user[3];
    let r1 = radius + thickness;
    let safe_r1 = select(r1, 1e-30, r1 == 0.0);
    let r2 = (radius * radius) / safe_r1;
    var out: array<f32, 4>;
    out[0] = r1;
    out[1] = r2;
    out[2] = r1 / max(r1 + r2, 1e-30);
    out[3] = abs(pow_p);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_glynnSim3(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let contrast = get_param(xform_id, variation_id, 2u);
    let r1 = get_param(xform_id, variation_id, 4u);
    let r2 = get_param(xform_id, variation_id, 5u);
    let gamma = get_param(xform_id, variation_id, 6u);
    let abs_pow = get_param(xform_id, variation_id, 7u);

    let r = sqrt(p.x * p.x + p.y * p.y);
    let alpha = radius / max(r, 1e-30);

    if (r < r1) {
        let phi = 6.28318530717959 * rng_nextf(rng);
        var rr = r2;
        if (rng_nextf(rng) < gamma) {
            rr = r1;
        }
        return vec2<f32>(rr * cos(phi), rr * sin(phi));
    }
    if (rng_nextf(rng) > contrast * pow(alpha, abs_pow)) {
        return p;
    }
    return vec2<f32>(alpha * alpha * p.x, alpha * alpha * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_glynnSim3(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let contrast = get_param(xform_id, variation_id, 2u);
    let r1 = get_param(xform_id, variation_id, 4u);
    let r2 = get_param(xform_id, variation_id, 5u);
    let gamma = get_param(xform_id, variation_id, 6u);
    let abs_pow = get_param(xform_id, variation_id, 7u);

    let r = sqrt(p.x * p.x + p.y * p.y);
    let alpha = radius / max(r, 1e-30);

    if (r < r1) {
        let phi = 6.28318530717959 * rng_nextf(rng);
        var rr = r2;
        if (rng_nextf(rng) < gamma) {
            rr = r1;
        }
        return vec3<f32>(rr * cos(phi), rr * sin(phi), p.z);
    }
    if (rng_nextf(rng) > contrast * pow(alpha, abs_pow)) {
        return p;
    }
    return vec3<f32>(alpha * alpha * p.x, alpha * alpha * p.y, p.z);
}
"#),
};
