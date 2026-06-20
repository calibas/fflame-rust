//! cut_* stencils with helper functions, batch 2 (Jesus Sosa) — more
//! JWildfire "cut_*" procedural masks whose GPU code defines helper
//! function(s), continuing from `cut_helper`.
//!
//! Same family contract (sample by mode → evaluate pattern → hide via
//! `Feature::CanHide`, Replace output). Helpers are inlined with their JWF
//! `cut_<name>_…` prefix so definitions never collide across the
//! concatenated per-flame shader; only one of the 2D/3D body is emitted per
//! flame, so the duplicated helper definitions are never both present.
//!
//! Contained here:
//!   - cut_apollonian  — Apollonian gasket fold (inversion iteration)
//!   - cut_zigzag      — mirrored zig-zag stripe tiles
//!   - cut_bricks      — running-bond brick grid
//!
//! GLSL `mod(a,b)` is inlined as `a − b·floor(a/b)`. `seed` (where present)
//! is JWF's UI wall-clock hack, declared for `.flame` round-trip only.
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// cut_apollonian
// =============================================================================
pub static CUT_APOLLONIAN: VariationDef = VariationDef {
    name: "cut_apollonian",
    aliases: &[],
    display_name: "Cut Apollonian",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("levels", "Levels", int, 4.0, 1.0, 10.0, "Number of inversion-fold iterations."),
        param!("zoom", "Zoom", float, 2.0, 0.1, 50.0, "Pattern scale: `point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_apollonian_apollo(xy: vec2<f32>, n: i32) -> f32 {
    var scale = 1.0;
    var pp = xy;
    // JWF also tracks t0/t1 (min reductions) here but never uses them; omitted.
    for (var i = 0; i < n; i = i + 1) {
        pp = fract(pp * 0.5 + 0.5) * 2.0 - 1.0;
        let k = 1.34 / dot(pp, pp);
        pp = pp * k;
        scale = scale * k;
    }
    var d = 0.25 * abs(pp.y) / scale;
    d = smoothstep(0.001, 0.002, d);
    return d;
}
fn variation_cut_apollonian(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let levels = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let pt = vec2<f32>(x * zoom, y * zoom);
    let col = cut_apollonian_apollo(pt, levels);

    var hidden = false;
    if (invert == 0) {
        if (col > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (col <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn cut_apollonian_apollo(xy: vec2<f32>, n: i32) -> f32 {
    var scale = 1.0;
    var pp = xy;
    for (var i = 0; i < n; i = i + 1) {
        pp = fract(pp * 0.5 + 0.5) * 2.0 - 1.0;
        let k = 1.34 / dot(pp, pp);
        pp = pp * k;
        scale = scale * k;
    }
    var d = 0.25 * abs(pp.y) / scale;
    d = smoothstep(0.001, 0.002, d);
    return d;
}
fn variation_cut_apollonian(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let levels = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let pt = vec2<f32>(x * zoom, y * zoom);
    let col = cut_apollonian_apollo(pt, levels);

    var hidden = false;
    if (invert == 0) {
        if (col > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (col <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};

// =============================================================================
// cut_zigzag
// =============================================================================
pub static CUT_ZIGZAG: VariationDef = VariationDef {
    name: "cut_zigzag",
    aliases: &[],
    display_name: "Cut ZigZag",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("xpar", "X Par", unlimited_float, 1.0, -50.0, 50.0, "X scale factor applied after zoom."),
        param!("ypar", "Y Par", unlimited_float, 2.0, -50.0, 50.0, "Y scale factor applied after zoom (controls zig frequency)."),
        param!("zoom", "Zoom", unlimited_float, 2.0, -50.0, 50.0, "Pattern scale: `st = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_zigzag_mirrorTile(st0: vec2<f32>) -> vec2<f32> {
    var st = st0;
    if (fract(st.y * 0.5) > 0.5) {
        st.x = st.x + 0.5;
        st.y = 1.0 - st.y;
    }
    return fract(st);
}
fn cut_zigzag_fillY(st: vec2<f32>, pct: f32, antia: f32) -> f32 {
    return smoothstep(pct - antia, pct, st.y);
}
fn variation_cut_zigzag(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let xpar = get_param(xform_id, variation_id, 1u);
    let ypar = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));
    const PI: f32 = 3.14159265359;

    var xp: f32;
    var yp: f32;
    if (mode == 0) { xp = p.x; yp = p.y; } else { xp = rng_nextf(rng) - 0.5; yp = rng_nextf(rng) - 0.5; }
    var st = vec2<f32>(xp, yp);
    st = st * zoom;
    st = st * vec2<f32>(xpar, ypar);
    st = cut_zigzag_mirrorTile(st);
    let xx = st.x * 2.0;
    let a = floor(1.0 + sin(xx * PI));
    let b = floor(1.0 + sin((xx + 1.0) * PI));
    let f = fract(xx);
    let color = cut_zigzag_fillY(st, mix(a, b, f), 0.01);

    var hidden = false;
    if (invert == 0) {
        if (color >= 0.5) { xp = 0.0; yp = 0.0; hidden = true; }
    } else {
        if (color < 0.5) { xp = 0.0; yp = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(xp, yp);
}
"#,
    wgsl_3d: r#"
fn cut_zigzag_mirrorTile(st0: vec2<f32>) -> vec2<f32> {
    var st = st0;
    if (fract(st.y * 0.5) > 0.5) {
        st.x = st.x + 0.5;
        st.y = 1.0 - st.y;
    }
    return fract(st);
}
fn cut_zigzag_fillY(st: vec2<f32>, pct: f32, antia: f32) -> f32 {
    return smoothstep(pct - antia, pct, st.y);
}
fn variation_cut_zigzag(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let xpar = get_param(xform_id, variation_id, 1u);
    let ypar = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));
    const PI: f32 = 3.14159265359;

    var xp: f32;
    var yp: f32;
    if (mode == 0) { xp = p.x; yp = p.y; } else { xp = rng_nextf(rng) - 0.5; yp = rng_nextf(rng) - 0.5; }
    var st = vec2<f32>(xp, yp);
    st = st * zoom;
    st = st * vec2<f32>(xpar, ypar);
    st = cut_zigzag_mirrorTile(st);
    let xx = st.x * 2.0;
    let a = floor(1.0 + sin(xx * PI));
    let b = floor(1.0 + sin((xx + 1.0) * PI));
    let f = fract(xx);
    let color = cut_zigzag_fillY(st, mix(a, b, f), 0.01);

    var hidden = false;
    if (invert == 0) {
        if (color >= 0.5) { xp = 0.0; yp = 0.0; hidden = true; }
    } else {
        if (color < 0.5) { xp = 0.0; yp = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(xp, yp, p.z);
}
"#,
};

// =============================================================================
// cut_bricks
// =============================================================================
pub static CUT_BRICKS: VariationDef = VariationDef {
    name: "cut_bricks",
    aliases: &[],
    display_name: "Cut Bricks",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("size", "Size", float, 0.9, 0.1, 0.9999, "Brick fill fraction within each cell (mortar gap = 1 − size)."),
        param!("zoom", "Zoom", unlimited_float, 3.0, -50.0, 50.0, "Pattern scale (brick count): `point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_bricks_brickTile(st0: vec2<f32>) -> vec2<f32> {
    var st = st0;
    let mody = st.y - 2.0 * floor(st.y / 2.0);
    st.x = st.x + step(1.0, mody) * 0.5;
    return fract(st);
}
fn cut_bricks_box(st: vec2<f32>, size0: vec2<f32>) -> f32 {
    let sz = vec2<f32>(0.5, 0.5) - size0 * 0.5;
    var uv = smoothstep(sz, sz + vec2<f32>(1.0e-4), st);
    uv = uv * smoothstep(sz, sz + vec2<f32>(1.0e-4), vec2<f32>(1.0, 1.0) - st);
    return uv.x * uv.y;
}
fn variation_cut_bricks(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let size = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }
    var u = vec2<f32>(x * zoom, y * zoom);
    u = cut_bricks_brickTile(u);
    let color = cut_bricks_box(u, vec2<f32>(size, size));

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x - cx, y - cy);
}
"#,
    wgsl_3d: r#"
fn cut_bricks_brickTile(st0: vec2<f32>) -> vec2<f32> {
    var st = st0;
    let mody = st.y - 2.0 * floor(st.y / 2.0);
    st.x = st.x + step(1.0, mody) * 0.5;
    return fract(st);
}
fn cut_bricks_box(st: vec2<f32>, size0: vec2<f32>) -> f32 {
    let sz = vec2<f32>(0.5, 0.5) - size0 * 0.5;
    var uv = smoothstep(sz, sz + vec2<f32>(1.0e-4), st);
    uv = uv * smoothstep(sz, sz + vec2<f32>(1.0e-4), vec2<f32>(1.0, 1.0) - st);
    return uv.x * uv.y;
}
fn variation_cut_bricks(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let size = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }
    var u = vec2<f32>(x * zoom, y * zoom);
    u = cut_bricks_brickTile(u);
    let color = cut_bricks_box(u, vec2<f32>(size, size));

    var hidden = false;
    if (invert == 0) {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color <= 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x - cx, y - cy, p.z);
}
"#,
};
