//! Parametric-curve variations
//!
//! Variations that pick a random parameter `t` and emit `(f(t), g(t))`,
//! or sample a discrete index from a φ-scaled spiral. Combined with
//! `crop3D` (a popular axis-aligned 3D crop) since they share the same
//! "pick random + emit shape" pattern.
//!
//!   - `spirograph`                         — hypotrochoid sampler
//!   - `lissajous`  (Jed Kelsey)            — Lissajous-figure sampler
//!   - `vogel`      (Victor Ganora)         — Vogel-spiral sampler (φ²)
//!   - `crop3D`     (Xyrus02)               — axis-aligned 3D crop
//!
//! Sources:
//!   - output/jwildfire-vars/output/spirograph.cpp
//!   - output/jwildfire-vars/output/lissajous.cpp
//!   - output/jwildfire-vars/output/vogel.cpp
//!   - output/jwildfire-vars/output/crop3D.cpp
//!
//! Notes on faithfulness:
//!   - `spirograph`, `lissajous`, `vogel` all factor VVAR cleanly through
//!     the outer multiplier — no `needs_transform` needed.
//!   - `crop3D` upstream uses `FPx = VVAR · x` (assignment, not `+=`)
//!     and additionally sets `FPx = FPy = FPz = 0` when zero-mode +
//!     out-of-bounds. We use the standard outer-multiplier convention
//!     (returning the unscaled point) — matches cpp at any weight when
//!     crop3D is the only normal variation in its transform (typical
//!     use); diverges if mixed with other normal variations.
//!
//! Skipped from this cluster:
//!   - `curl_sp`, `curliecue`, `curliecue2` — DC writes color, or use
//!     persistent per-thread state across iterations.
//!   - `crob`, `maurer_rose`, `maurer_lines`, `rhodonea` — large (260+
//!     to 4677 lines) with many init slots; better as their own focused
//!     batches.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// spirograph: hypotrochoid sampler
//   t = lerp(tmin, tmax, rand);  y = lerp(ymin, ymax, rand)
//   x1 = (a+b)·cos(t) − c1·cos((a+b)/b · t)
//   y1 = (a+b)·sin(t) − c2·sin((a+b)/b · t)
//   out = (x1 + d·cos(t) + y, y1 + d·sin(t) + y)
// =============================================================================
/// Hypotrochoid curve sampler — picks a random parameter `t` and emits a
/// point on the classic spirograph curve. `a`/`b` are the outer/inner wheel
/// radii; `c1`/`c2` give per-axis amplitudes; `d` is the pen-arm length.
pub static SPIROGRAPH: VariationDef = VariationDef {
    name: "spirograph",
    aliases: &[],
    display_name: "Spirograph",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("a", "A", unlimited_float, 3.0, -10.0, 10.0, "Outer-wheel radius."),
        param!("b", "B", unlimited_float, 2.0, -10.0, 10.0, "Inner-wheel radius."),
        param!("d", "D", unlimited_float, 0.0, -10.0, 10.0, "Pen-arm length added to the output."),
        param!("c1", "C1", unlimited_float, 0.0, -10.0, 10.0, "X-axis amplitude of the inner-wheel cosine term."),
        param!("c2", "C2", unlimited_float, 0.0, -10.0, 10.0, "Y-axis amplitude of the inner-wheel sine term."),
        param!("tmin", "T Min", unlimited_float, -1.0, -100.0, 100.0, "Minimum value of the random parameter `t`."),
        param!("tmax", "T Max", unlimited_float, 1.0, -100.0, 100.0, "Maximum value of the random parameter `t`."),
        param!("ymin", "Y Min", unlimited_float, -1.0, -100.0, 100.0, "Minimum random Y offset added to both output coordinates."),
        param!("ymax", "Y Max", unlimited_float, 1.0, -100.0, 100.0, "Maximum random Y offset added to both output coordinates."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_spirograph(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let d = get_param(xform_id, variation_id, 2u);
    let c1 = get_param(xform_id, variation_id, 3u);
    let c2 = get_param(xform_id, variation_id, 4u);
    let tmin = get_param(xform_id, variation_id, 5u);
    let tmax = get_param(xform_id, variation_id, 6u);
    let ymin = get_param(xform_id, variation_id, 7u);
    let ymax = get_param(xform_id, variation_id, 8u);

    let t = (tmax - tmin) * rng_nextf(rng) + tmin;
    let y_off = (ymax - ymin) * rng_nextf(rng) + ymin;
    let safe_b = select(b, 1e-30, b == 0.0);
    let ratio = (a + b) / safe_b;
    let x1 = (a + b) * cos(t) - c1 * cos(ratio * t);
    let y1 = (a + b) * sin(t) - c2 * sin(ratio * t);
    return vec2<f32>(
        x1 + d * cos(t) + y_off,
        y1 + d * sin(t) + y_off,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_spirograph(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let d = get_param(xform_id, variation_id, 2u);
    let c1 = get_param(xform_id, variation_id, 3u);
    let c2 = get_param(xform_id, variation_id, 4u);
    let tmin = get_param(xform_id, variation_id, 5u);
    let tmax = get_param(xform_id, variation_id, 6u);
    let ymin = get_param(xform_id, variation_id, 7u);
    let ymax = get_param(xform_id, variation_id, 8u);

    let t = (tmax - tmin) * rng_nextf(rng) + tmin;
    let y_off = (ymax - ymin) * rng_nextf(rng) + ymin;
    let safe_b = select(b, 1e-30, b == 0.0);
    let ratio = (a + b) / safe_b;
    let x1 = (a + b) * cos(t) - c1 * cos(ratio * t);
    let y1 = (a + b) * sin(t) - c2 * sin(ratio * t);
    return vec3<f32>(
        x1 + d * cos(t) + y_off,
        y1 + d * sin(t) + y_off,
        p.z,
    );
}
"#),
};

// =============================================================================
// lissajous: Lissajous-figure sampler (Jed Kelsey)
//   t = lerp(tmin, tmax, rand);  y = rand − 0.5
//   x1 = sin(a·t + d);  y1 = sin(b·t)
//   out = (x1 + c·t + e·y, y1 + c·t + e·y)
// =============================================================================
/// Lissajous-figure sampler — picks a random `t` and emits `(sin(a·t + d),
/// sin(b·t))`. Adds linear drift `c·t` and noise `e·y` for thicker curves.
///
/// # Authors
/// - Jed Kelsey
pub static LISSAJOUS: VariationDef = VariationDef {
    name: "lissajous",
    aliases: &[],
    display_name: "Lissajous",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("tmin", "T Min", unlimited_float, -3.14159265, -100.0, 100.0, "Minimum value of the random parameter `t`."),
        param!("tmax", "T Max", unlimited_float, 3.14159265, -100.0, 100.0, "Maximum value of the random parameter `t`."),
        param!("a", "A", unlimited_float, 3.0, -20.0, 20.0, "X-axis frequency multiplier."),
        param!("b", "B", unlimited_float, 2.0, -20.0, 20.0, "Y-axis frequency multiplier."),
        param!("c", "C", unlimited_float, 0.0, -10.0, 10.0, "Linear drift added to both axes."),
        param!("d", "D", unlimited_float, 0.0, -10.0, 10.0, "Phase offset on the X axis."),
        param!("e", "E", unlimited_float, 0.0, -10.0, 10.0, "Noise scale added to both axes."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_lissajous(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let tmin = get_param(xform_id, variation_id, 0u);
    let tmax = get_param(xform_id, variation_id, 1u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d = get_param(xform_id, variation_id, 5u);
    let e = get_param(xform_id, variation_id, 6u);

    let t = (tmax - tmin) * rng_nextf(rng) + tmin;
    let y = rng_nextf(rng) - 0.5;
    let x1 = sin(a * t + d);
    let y1 = sin(b * t);
    return vec2<f32>(
        x1 + c * t + e * y,
        y1 + c * t + e * y,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_lissajous(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let tmin = get_param(xform_id, variation_id, 0u);
    let tmax = get_param(xform_id, variation_id, 1u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d = get_param(xform_id, variation_id, 5u);
    let e = get_param(xform_id, variation_id, 6u);

    let t = (tmax - tmin) * rng_nextf(rng) + tmin;
    let y = rng_nextf(rng) - 0.5;
    let x1 = sin(a * t + d);
    let y1 = sin(b * t);
    return vec3<f32>(
        x1 + c * t + e * y,
        y1 + c * t + e * y,
        p.z,
    );
}
"#),
};

// =============================================================================
// vogel: Vogel-spiral sampler (Victor Ganora)
//   i = uniform_int [0, n)
//   a = i · 2π / φ²                          (φ = golden ratio)
//   r = w · (sqrt(x²+y²) + ε + sqrt(i))
//   out = r · (cos(a) + scale·x, sin(a) + scale·y)
// =============================================================================
/// Vogel-spiral sampler — places points at integer-indexed angles `i ·
/// 2π/φ²` from a golden-angle spiral. Produces the iconic sunflower-seed
/// pattern.
///
/// # Authors
/// - Victor Ganora
pub static VOGEL: VariationDef = VariationDef {
    name: "vogel",
    aliases: &[],
    display_name: "Vogel",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("n", "N", int, 20.0, 1.0, 1000.0, "Number of seed positions (1-1000). Larger = more points around the spiral."),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Mixes the radial distance with the input coordinates. 0 = pure spiral; higher = blended with input."),
    ],
    needs_transform: false,
    writes_color: false,
    // 1 derived value at slot 2:
    //   2: two_pi_over_phi2  (2π / φ², where φ = golden ratio)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_vogel(user: array<f32, 2>) -> array<f32, 1> {
    let phi = 1.618033988749895;
    let two_pi = 6.28318530717959;
    var out: array<f32, 1>;
    out[0] = two_pi / (phi * phi);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_vogel(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);
    let two_pi_over_phi2 = get_param(xform_id, variation_id, 2u);

    let n_safe = max(n, 1.0);
    let i = floor(rng_nextf(rng) * n_safe) + 1.0;
    let a = i * two_pi_over_phi2;
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-6 + sqrt(max(i, 0.0));
    return vec2<f32>(
        r * (cos(a) + scale * p.x),
        r * (sin(a) + scale * p.y),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_vogel(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);
    let two_pi_over_phi2 = get_param(xform_id, variation_id, 2u);

    let n_safe = max(n, 1.0);
    let i = floor(rng_nextf(rng) * n_safe) + 1.0;
    let a = i * two_pi_over_phi2;
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-6 + sqrt(max(i, 0.0));
    return vec3<f32>(
        r * (cos(a) + scale * p.x),
        r * (sin(a) + scale * p.y),
        p.z,
    );
}
"#),
};

// =============================================================================
// crop3D: axis-aligned 3D crop with optional jitter (Xyrus02)
//   Outside the bounding box:
//     if zero ≠ 0:  contribute (0, 0, 0)
//     else:         clamp coordinates back into box, jittered by w/h/l
//                   towards the opposite edge by `scatter_area · range`
//   Inside the box: identity-pass
//
// Init keeps min/max for each axis (8 user + 6 init = 14 slots, fits the
// 16-slot budget). w/h/l (= range · 0.5 · scatter_area) computed inline
// in the body to fit within the budget.
//
// Notes:
//   - Upstream uses `FPx = VVAR · x` (assignment). We use `+=`-friendly
//     return (just `x` without the weight), letting the outer multiplier
//     reapply VVAR. This matches cpp output when crop3D is the only
//     normal-phase variation in its transform (typical use); diverges
//     when mixed with others.
//   - Z preserve isn't needed here — body explicitly handles Z output.
// =============================================================================
/// Axis-aligned 3D crop with optional jitter — constrains points to a 3D
/// box. Outside the box, points either collapse to zero (`zero` on) or
/// scatter back toward the nearest edge with `scatter_area` randomness.
///
/// # Authors
/// - Xyrus02
pub static CROP3D: VariationDef = VariationDef {
    name: "crop3D",
    aliases: &[],
    display_name: "Crop 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("left", "Left", unlimited_float, -1.0, -10.0, 10.0, "Left bound of the cropping box (X axis)."),
        param!("top", "Top", unlimited_float, -1.0, -10.0, 10.0, "Top bound (Y axis)."),
        param!("floor", "Floor", unlimited_float, -1.0, -10.0, 10.0, "Floor bound (Z axis)."),
        param!("right", "Right", unlimited_float, 1.0, -10.0, 10.0, "Right bound (X axis)."),
        param!("bottom", "Bottom", unlimited_float, 1.0, -10.0, 10.0, "Bottom bound (Y axis)."),
        param!("ceiling", "Ceiling", unlimited_float, 1.0, -10.0, 10.0, "Ceiling bound (Z axis)."),
        param!("scatter_area", "Scatter Area", unlimited_float, 0.0, 0.0, 1.0, "Width of the scatter band along each box edge. 0 = snap to edge; 1 = scatter across the full box width."),
        param!("zero", "Zero", bool, false, "When on, out-of-box points collapse to origin. When off, they scatter back toward the nearest edge."),
    ],
    needs_transform: false,
    writes_color: false,
    // 6 derived values at slots 8..14:
    //   8: _xmin  10: _ymin  12: _zmin
    //   9: _xmax  11: _ymax  13: _zmax
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_crop3d(user: array<f32, 8>) -> array<f32, 6> {
    let left = user[0]; let top = user[1]; let floor = user[2];
    let right = user[3]; let bottom = user[4]; let ceiling = user[5];
    var out: array<f32, 6>;
    out[0] = min(left, right);
    out[1] = max(left, right);
    out[2] = min(top, bottom);
    out[3] = max(top, bottom);
    out[4] = min(floor, ceiling);
    out[5] = max(floor, ceiling);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_crop3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let scatter = get_param(xform_id, variation_id, 6u);
    let zero = get_param(xform_id, variation_id, 7u);
    let xmin = get_param(xform_id, variation_id, 8u);
    let xmax = get_param(xform_id, variation_id, 9u);
    let ymin = get_param(xform_id, variation_id, 10u);
    let ymax = get_param(xform_id, variation_id, 11u);
    let w_range = (xmax - xmin) * 0.5 * scatter;
    let h_range = (ymax - ymin) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;
    let outside = (x < xmin) || (x > xmax) || (y < ymin) || (y > ymax);
    if (outside && zero != 0.0) {
        return vec2<f32>(0.0, 0.0);
    }
    if (x < xmin) { x = xmin + rng_nextf(rng) * w_range; }
    else if (x > xmax) { x = xmax - rng_nextf(rng) * w_range; }
    if (y < ymin) { y = ymin + rng_nextf(rng) * h_range; }
    else if (y > ymax) { y = ymax - rng_nextf(rng) * h_range; }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_crop3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let scatter = get_param(xform_id, variation_id, 6u);
    let zero = get_param(xform_id, variation_id, 7u);
    let xmin = get_param(xform_id, variation_id, 8u);
    let xmax = get_param(xform_id, variation_id, 9u);
    let ymin = get_param(xform_id, variation_id, 10u);
    let ymax = get_param(xform_id, variation_id, 11u);
    let zmin = get_param(xform_id, variation_id, 12u);
    let zmax = get_param(xform_id, variation_id, 13u);
    let w_range = (xmax - xmin) * 0.5 * scatter;
    let h_range = (ymax - ymin) * 0.5 * scatter;
    let l_range = (zmax - zmin) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;
    var z = p.z;
    let outside = (x < xmin) || (x > xmax) || (y < ymin) || (y > ymax) || (z < zmin) || (z > zmax);
    if (outside && zero != 0.0) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    if (x < xmin) { x = xmin + rng_nextf(rng) * w_range; }
    else if (x > xmax) { x = xmax - rng_nextf(rng) * w_range; }
    if (y < ymin) { y = ymin + rng_nextf(rng) * h_range; }
    else if (y > ymax) { y = ymax - rng_nextf(rng) * h_range; }
    if (z < zmin) { z = zmin + rng_nextf(rng) * l_range; }
    else if (z > zmax) { z = zmax - rng_nextf(rng) * l_range; }
    return vec3<f32>(x, y, z);
}
"#),
};
