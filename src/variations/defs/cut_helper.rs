//! cut_* stencils with one helper (Jesus Sosa) — JWildfire "cut_*"
//! procedural masks whose GPU code defines a single helper function.
//!
//! Same family contract as `cut_simple` (sample by mode → evaluate pattern
//! → hide via `Feature::CanHide`, Replace output). Each helper is inlined
//! into the variation's WGSL with its JWF name prefix (`cut_<name>_…`) so
//! definitions never collide across the concatenated per-flame shader. The
//! shader builder emits only the 2D or 3D body of a given flame, so the
//! duplicated helper definitions in each body are never both present.
//!
//! Contained here:
//!   - cut_x          — rotated absolute-value cross (45° fold)
//!   - cut_metaballs  — Voronoi metaball field (hash-jittered cells)
//!   - cut_kaleido    — summed-distance kaleidoscope (N orbiting centers)
//!   - cut_spiral     — 64-step rotation parity spiral
//!   - cut_swarp      — sine-warped tiled distance bands
//!
//! `cut_x`/`cut_spiral` use JWF's `Mat2_Init(a,b,c,d)`/`times` rotation
//! helpers (row-major `[[a,b],[c,d]]`, `times(m,v)=(a·x+b·y, c·x+d·y)`),
//! inlined directly. `seed` (where present) is JWF's UI wall-clock hack,
//! declared for `.flame` round-trip only.
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// cut_x
// =============================================================================
pub static CUT_X: VariationDef = VariationDef {
    name: "cut_x",
    aliases: &[],
    display_name: "Cut X",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("zoom", "Zoom", unlimited_float, 1.0, -50.0, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
        param!("size", "Size", unlimited_float, 0.1, -10.0, 10.0, "Half-thickness of the cross arms."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_x(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let size = get_param(xform_id, variation_id, 3u);
    const PI: f32 = 3.14159265359;

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var st = abs(vec2<f32>(x, y) * zoom);
    // times(cut_x_rot(PI/4), st): row-major rotation by +PI/4.
    let ca = cos(PI / 4.0);
    let sa = sin(PI / 4.0);
    st = vec2<f32>(ca * st.x - sa * st.y, sa * st.x + ca * st.y);
    st = abs(st);
    let line = smoothstep(0.0, 0.009, st.y - size);
    let color = mix(0.0, 1.0, line);

    var hidden = false;
    if (invert == 0) {
        if (color < 0.1) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color >= 0.1) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_cut_x(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let size = get_param(xform_id, variation_id, 3u);
    const PI: f32 = 3.14159265359;

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var st = abs(vec2<f32>(x, y) * zoom);
    let ca = cos(PI / 4.0);
    let sa = sin(PI / 4.0);
    st = vec2<f32>(ca * st.x - sa * st.y, sa * st.x + ca * st.y);
    st = abs(st);
    let line = smoothstep(0.0, 0.009, st.y - size);
    let color = mix(0.0, 1.0, line);

    var hidden = false;
    if (invert == 0) {
        if (color < 0.1) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color >= 0.1) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};

// =============================================================================
// cut_metaballs
// =============================================================================
pub static CUT_METABALLS: VariationDef = VariationDef {
    name: "cut_metaballs",
    aliases: &[],
    display_name: "Cut MetaBalls",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("zoom", "Zoom", float, 7.0, 0.1, 50.0, "Pattern scale (cell count): `point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
        param!("time", "Time", unlimited_float, 0.0, -100.0, 100.0, "Animates the per-cell metaball jitter."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_metaballs_random2(pp: vec2<f32>) -> vec2<f32> {
    let tm = vec2<f32>(dot(pp, vec2<f32>(127.1, 311.7)), dot(pp, vec2<f32>(269.5, 183.3)));
    return fract(sin(tm) * 43758.5453);
}
fn variation_cut_metaballs(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let time = get_param(xform_id, variation_id, 3u);

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }
    let st = vec2<f32>(x * zoom, y * zoom);
    let i_st = floor(st);
    let f_st = fract(st);
    var m_dist = 1.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let neighbor = vec2<f32>(f32(i), f32(j));
            var offset = cut_metaballs_random2(i_st + neighbor);
            let tmp = offset * 6.2831 + time;
            offset = sin(tmp) * 0.5 + 0.5;
            let pos = neighbor + offset - f_st;
            let dist = length(pos);
            m_dist = min(m_dist, m_dist * dist);
        }
    }
    let color = step(0.060, m_dist);

    var hidden = false;
    if (invert == 0) {
        if (color == 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x - cx, y - cy);
}
"#,
    wgsl_3d: r#"
fn cut_metaballs_random2(pp: vec2<f32>) -> vec2<f32> {
    let tm = vec2<f32>(dot(pp, vec2<f32>(127.1, 311.7)), dot(pp, vec2<f32>(269.5, 183.3)));
    return fract(sin(tm) * 43758.5453);
}
fn variation_cut_metaballs(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));
    let time = get_param(xform_id, variation_id, 3u);

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }
    let st = vec2<f32>(x * zoom, y * zoom);
    let i_st = floor(st);
    let f_st = fract(st);
    var m_dist = 1.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let neighbor = vec2<f32>(f32(i), f32(j));
            var offset = cut_metaballs_random2(i_st + neighbor);
            let tmp = offset * 6.2831 + time;
            offset = sin(tmp) * 0.5 + 0.5;
            let pos = neighbor + offset - f_st;
            let dist = length(pos);
            m_dist = min(m_dist, m_dist * dist);
        }
    }
    let color = step(0.060, m_dist);

    var hidden = false;
    if (invert == 0) {
        if (color == 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x - cx, y - cy, p.z);
}
"#,
};

// =============================================================================
// cut_kaleido
// =============================================================================
pub static CUT_KALEIDO: VariationDef = VariationDef {
    name: "cut_kaleido",
    aliases: &[],
    display_name: "Cut Kaleido",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("time", "Time", unlimited_float, 0.0, -100.0, 100.0, "Rotates the orbiting centers and modulates the ring frequency."),
        param!("n", "N", int, 6.0, 2.0, 20.0, "Number of orbiting distance centers."),
        param!("zoom", "Zoom", unlimited_float, 0.5, -50.0, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_kaleido_distToColor(d: f32) -> f32 {
    return 0.0 - cos(d * 13.0);
}
fn variation_cut_kaleido(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let n = clamp(i32(get_param(xform_id, variation_id, 3u)), 2, 20);
    let zoom = get_param(xform_id, variation_id, 4u);
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let uv = vec2<f32>(x * zoom, y * zoom);
    var ci: array<vec2<f32>, 20>;
    for (var i = 0; i < n; i = i + 1) {
        let fi = 2.0 * 3.14 * (f32(i) + 0.02 * time) / f32(n);
        ci[i] = vec2<f32>(0.5 * sin(fi), 0.5 * cos(fi));
    }
    var d = 1.0;
    let k = 100.0 + 10.0 * sin(time / 5.0);
    for (var i = 0; i < n; i = i + 1) {
        d = d + sin(k * distance(uv, ci[i]));
    }
    let color = cut_kaleido_distToColor(d / f32(n));

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn cut_kaleido_distToColor(d: f32) -> f32 {
    return 0.0 - cos(d * 13.0);
}
fn variation_cut_kaleido(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let n = clamp(i32(get_param(xform_id, variation_id, 3u)), 2, 20);
    let zoom = get_param(xform_id, variation_id, 4u);
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let uv = vec2<f32>(x * zoom, y * zoom);
    var ci: array<vec2<f32>, 20>;
    for (var i = 0; i < n; i = i + 1) {
        let fi = 2.0 * 3.14 * (f32(i) + 0.02 * time) / f32(n);
        ci[i] = vec2<f32>(0.5 * sin(fi), 0.5 * cos(fi));
    }
    var d = 1.0;
    let k = 100.0 + 10.0 * sin(time / 5.0);
    for (var i = 0; i < n; i = i + 1) {
        d = d + sin(k * distance(uv, ci[i]));
    }
    let color = cut_kaleido_distToColor(d / f32(n));

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};

// =============================================================================
// cut_spiral
// =============================================================================
pub static CUT_SPIRAL: VariationDef = VariationDef {
    name: "cut_spiral",
    aliases: &[],
    display_name: "Cut Spiral",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("time", "Time", unlimited_float, 0.0, -100.0, 100.0, "Rotation angle per spiral step."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("zoom", "Zoom", unlimited_float, 2.5, -50.0, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_spiral(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let time = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = 2.0 * rng_nextf(rng) - 1.0; y = 2.0 * rng_nextf(rng) - 1.0; }
    var uv = vec2<f32>(x * zoom, y * zoom);
    // cut_spiral_rot2(time) = Mat2_Init(1.1 sin, 1.1 cos, -1.1 cos, 1.1 sin).
    let a = 1.1 * sin(time);
    let b = 1.1 * cos(time);
    var l = 0;
    for (var i = 0; i < 64; i = i + 1) {
        uv = vec2<f32>(a * uv.x + b * uv.y, -b * uv.x + a * uv.y);
        if (uv.y > 1.0) { break; }
        l = l ^ 1;
    }
    let color = f32(l);

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_cut_spiral(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let time = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = 2.0 * rng_nextf(rng) - 1.0; y = 2.0 * rng_nextf(rng) - 1.0; }
    var uv = vec2<f32>(x * zoom, y * zoom);
    let a = 1.1 * sin(time);
    let b = 1.1 * cos(time);
    var l = 0;
    for (var i = 0; i < 64; i = i + 1) {
        uv = vec2<f32>(a * uv.x + b * uv.y, -b * uv.x + a * uv.y);
        if (uv.y > 1.0) { break; }
        l = l ^ 1;
    }
    let color = f32(l);

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};

// =============================================================================
// cut_swarp
// =============================================================================
pub static CUT_SWARP: VariationDef = VariationDef {
    name: "cut_swarp",
    aliases: &[],
    display_name: "Cut SWarp",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("time", "Time", unlimited_float, 0.0, -100.0, 100.0, "Animates the sine warp and band phase."),
        param!("type", "Type", enum, 0, &["Euclidean", "Chebyshev", "Hexagonal", "Octagonal"], "Distance metric for the band field."),
        param!("zoom", "Zoom", unlimited_float, 1.0, -50.0, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_swarp_dist(p0: vec2<f32>, btype: i32) -> f32 {
    var distance = 0.0;
    if (btype == 0) { distance = length(p0); }
    let p = abs(p0);
    if (btype == 1) { distance = max(p.x, p.y); }
    if (btype == 2) { distance = max(p.x * 0.8660254 + p.y * 0.5, p.y); }
    if (btype == 3) { distance = max(max(p.x, p.y), (p.x + p.y) * 0.7071); }
    return distance;
}
fn variation_cut_swarp(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let btype = i32(get_param(xform_id, variation_id, 3u));
    let zoom = get_param(xform_id, variation_id, 4u);
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var uv = vec2<f32>(x * zoom, y * zoom);
    let v0 = uv * 5.0 + cos(uv * 11.0 + time) + time;
    let v1 = sin(v0);
    let t0 = dot(v1, vec2<f32>(0.5, 0.5));
    uv = uv + t0 * 0.02;
    uv.y = uv.y + sin(uv.x * 7.0 + cos(uv.x * 4.0 + time * 2.0) * 1.0 + time) * 0.05;
    let s = ceil(uv.x) * 2.0 - 1.0;
    let m = 6.0;
    uv.y = uv.y + s / m;
    let n = 5.0;
    let k = sin(s * 3.14159 / 2.0 + cut_swarp_dist(uv, btype) * n * m * 3.14159 / 2.0);
    let color = sqrt(max(k, 0.0));

    var hidden = false;
    if (invert == 0) {
        if (color > 0.5) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.5) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn cut_swarp_dist(p0: vec2<f32>, btype: i32) -> f32 {
    var distance = 0.0;
    if (btype == 0) { distance = length(p0); }
    let p = abs(p0);
    if (btype == 1) { distance = max(p.x, p.y); }
    if (btype == 2) { distance = max(p.x * 0.8660254 + p.y * 0.5, p.y); }
    if (btype == 3) { distance = max(max(p.x, p.y), (p.x + p.y) * 0.7071); }
    return distance;
}
fn variation_cut_swarp(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let btype = i32(get_param(xform_id, variation_id, 3u));
    let zoom = get_param(xform_id, variation_id, 4u);
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var uv = vec2<f32>(x * zoom, y * zoom);
    let v0 = uv * 5.0 + cos(uv * 11.0 + time) + time;
    let v1 = sin(v0);
    let t0 = dot(v1, vec2<f32>(0.5, 0.5));
    uv = uv + t0 * 0.02;
    uv.y = uv.y + sin(uv.x * 7.0 + cos(uv.x * 4.0 + time * 2.0) * 1.0 + time) * 0.05;
    let s = ceil(uv.x) * 2.0 - 1.0;
    let m = 6.0;
    uv.y = uv.y + s / m;
    let n = 5.0;
    let k = sin(s * 3.14159 / 2.0 + cut_swarp_dist(uv, btype) * n * m * 3.14159 / 2.0);
    let color = sqrt(max(k, 0.0));

    var hidden = false;
    if (invert == 0) {
        if (color > 0.5) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.5) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};
