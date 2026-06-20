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
//!   - cut_web         — log-polar spider-web grid
//!   - cut_fingerprint — whorled fingerprint ridges (atan accumulation)
//!   - cut_tileillusion— sheared wavy-tile checkerboard illusion
//!   - cut_pattern     — rotated-tile radial wave pattern
//!   - cut_celtic      — triple hex-offset Celtic-knot circles
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

// =============================================================================
// cut_web
// =============================================================================
pub static CUT_WEB: VariationDef = VariationDef {
    name: "cut_web",
    aliases: &[],
    display_name: "Cut Web",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("time", "Time", unlimited_float, 0.0, -100.0, 100.0, "Scrolls the log-polar grid radially."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("thick", "Thickness", float, 0.05, 0.0, 0.9, "Web strand thickness."),
        param!("invert", "Invert", bool, true, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_web_eval(p0: vec2<f32>, c: vec2<f32>, strength: f32) -> vec2<f32> {
    let p = p0 - c;
    let l = log(length(p));
    let ang = atan2(p.y, p.x);
    return vec2<f32>(l, ang) * strength;
}
fn cut_web_getColour(p: vec2<f32>, time: f32, thick: f32) -> f32 {
    const PI: f32 = 3.14159265359;
    let fac = 10.0 / (2.0 * PI);
    var ep = cut_web_eval(p, vec2<f32>(0.0, 0.0), 1.0);
    let d = ep.x * 0.05 * fac;
    ep = ep + vec2<f32>(-ep.y, ep.x) * 4.0;
    let arg = ep * fac + time;
    let modded = arg - 2.0 * floor(arg / 2.0);
    let xx = abs(modded - 1.0) - thick;
    let si = smoothstep(vec2<f32>(-0.5 * d), vec2<f32>(0.5 * d), xx);
    return si.x + si.y;
}
fn variation_cut_web(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let time = get_param(xform_id, variation_id, 1u);
    let mode = i32(get_param(xform_id, variation_id, 2u));
    let thick = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = 2.0 * rng_nextf(rng) - 1.0; y = 2.0 * rng_nextf(rng) - 1.0; }
    let u = vec2<f32>(x * 0.5, y * 0.5);
    let color = cut_web_getColour(u, time, thick);

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
fn cut_web_eval(p0: vec2<f32>, c: vec2<f32>, strength: f32) -> vec2<f32> {
    let p = p0 - c;
    let l = log(length(p));
    let ang = atan2(p.y, p.x);
    return vec2<f32>(l, ang) * strength;
}
fn cut_web_getColour(p: vec2<f32>, time: f32, thick: f32) -> f32 {
    const PI: f32 = 3.14159265359;
    let fac = 10.0 / (2.0 * PI);
    var ep = cut_web_eval(p, vec2<f32>(0.0, 0.0), 1.0);
    let d = ep.x * 0.05 * fac;
    ep = ep + vec2<f32>(-ep.y, ep.x) * 4.0;
    let arg = ep * fac + time;
    let modded = arg - 2.0 * floor(arg / 2.0);
    let xx = abs(modded - 1.0) - thick;
    let si = smoothstep(vec2<f32>(-0.5 * d), vec2<f32>(0.5 * d), xx);
    return si.x + si.y;
}
fn variation_cut_web(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let time = get_param(xform_id, variation_id, 1u);
    let mode = i32(get_param(xform_id, variation_id, 2u));
    let thick = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = 2.0 * rng_nextf(rng) - 1.0; y = 2.0 * rng_nextf(rng) - 1.0; }
    let u = vec2<f32>(x * 0.5, y * 0.5);
    let color = cut_web_getColour(u, time, thick);

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
// cut_fingerprint
// =============================================================================
pub static CUT_FINGERPRINT: VariationDef = VariationDef {
    name: "cut_fingerprint",
    aliases: &[],
    display_name: "Cut FingerPrint",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 10000.0, 0.0, 1000000.0, "Selects the ridge whorl pattern (`floor(7·seed)` seeds the hash walk)."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("zoom", "Zoom", float, 20.0, 1.0, 100.0, "Pattern scale: `uv = point·zoom`."),
        param!("width", "Width", float, 0.8, 0.02, 1.0, "Ridge width fraction."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_fingerprint_hash2(p0: vec2<f32>) -> vec2<f32> {
    let p = vec2<f32>(dot(p0, vec2<f32>(63.31, 127.63)), dot(p0, vec2<f32>(395.467, 213.799)));
    return fract(sin(p) * 43141.59265) * 2.0 - 1.0;
}
fn cut_fingerprint_getColor(uv0: vec2<f32>, seed: f32, width: f32) -> f32 {
    let bounds = smoothstep(9.0, 10.0, length(uv0 * vec2<f32>(0.7, 0.5)));
    var a = 0.0;
    var h = vec2<f32>(floor(7.0 * seed), 0.0);
    for (var i = 0; i < 50; i = i + 1) {
        let s = sign(h.x);
        h = cut_fingerprint_hash2(h) * vec2<f32>(15.0, 20.0);
        a = a + s * atan2(uv0.x - h.x, uv0.y - h.y);
    }
    let uv = uv0 + abs(cut_fingerprint_hash2(h));
    a = a + atan2(uv.y, uv.x);
    let pp = (1.0 - bounds) * width;
    let s = min(0.3, pp);
    let l = length(uv) + 0.319 * a;
    let m = l - 2.0 * floor(l / 2.0);
    let v = (1.0 - smoothstep(2.0 - s, 2.0, m)) * smoothstep(pp, pp + s, m);
    return v;
}
fn variation_cut_fingerprint(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let seed = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let width = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let uv = vec2<f32>(x, y) * zoom;
    let color = cut_fingerprint_getColor(uv, seed, width);

    var hidden = false;
    if (invert == 0) {
        if (color == 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn cut_fingerprint_hash2(p0: vec2<f32>) -> vec2<f32> {
    let p = vec2<f32>(dot(p0, vec2<f32>(63.31, 127.63)), dot(p0, vec2<f32>(395.467, 213.799)));
    return fract(sin(p) * 43141.59265) * 2.0 - 1.0;
}
fn cut_fingerprint_getColor(uv0: vec2<f32>, seed: f32, width: f32) -> f32 {
    let bounds = smoothstep(9.0, 10.0, length(uv0 * vec2<f32>(0.7, 0.5)));
    var a = 0.0;
    var h = vec2<f32>(floor(7.0 * seed), 0.0);
    for (var i = 0; i < 50; i = i + 1) {
        let s = sign(h.x);
        h = cut_fingerprint_hash2(h) * vec2<f32>(15.0, 20.0);
        a = a + s * atan2(uv0.x - h.x, uv0.y - h.y);
    }
    let uv = uv0 + abs(cut_fingerprint_hash2(h));
    a = a + atan2(uv.y, uv.x);
    let pp = (1.0 - bounds) * width;
    let s = min(0.3, pp);
    let l = length(uv) + 0.319 * a;
    let m = l - 2.0 * floor(l / 2.0);
    let v = (1.0 - smoothstep(2.0 - s, 2.0, m)) * smoothstep(pp, pp + s, m);
    return v;
}
fn variation_cut_fingerprint(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let seed = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let width = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let uv = vec2<f32>(x, y) * zoom;
    let color = cut_fingerprint_getColor(uv, seed, width);

    var hidden = false;
    if (invert == 0) {
        if (color == 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};

// =============================================================================
// cut_tileillusion
// =============================================================================
pub static CUT_TILEILLUSION: VariationDef = VariationDef {
    name: "cut_tileillusion",
    aliases: &[],
    display_name: "Cut TileIllusion",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("time", "Time", unlimited_float, 0.0, -100.0, 100.0, "Horizontal shear offset alternating per row (drives the illusion)."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("zoom", "Zoom", unlimited_float, 15.0, -100.0, 100.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_tileillusion_f(x: f32) -> f32 {
    return x + 0.1 * sin(1.6 * x);
}
fn cut_tileillusion_solve(x0i: f32, x1i: f32, y: f32) -> f32 {
    var x0 = x0i;
    var x1 = x1i;
    var y0 = cut_tileillusion_f(x0);
    var y1 = cut_tileillusion_f(x1);
    if (y1 < y0) {
        let x2 = x1; x1 = x0; x0 = x2;
        let y2 = y1; y1 = y0; y0 = y2;
    }
    var xn = 0.0;
    var yn = 0.0;
    for (var i = 0; i < 20; i = i + 1) {
        xn = x0 + (x1 - x0) / (y1 - y0) * (y - y0);
        yn = cut_tileillusion_f(xn);
        if (yn > y) { x1 = xn; y1 = yn; } else { x0 = xn; y0 = yn; }
    }
    return xn;
}
fn variation_cut_tileillusion(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let time = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var uv = vec2<f32>(x * zoom, y * zoom);
    let fuvy = cut_tileillusion_f(uv.y);
    let y0 = cut_tileillusion_solve(uv.y - 3.0, uv.y, floor(fuvy));
    let y1 = cut_tileillusion_solve(uv.y, uv.y + 3.0, floor(fuvy) + 1.0);
    uv.y = fuvy;
    uv.x = uv.x / (y1 - y0);
    let fy = floor(uv.y);
    let s = fy - 2.0 * floor(fy / 2.0);
    var color = 0.0;
    if (fract(uv.y) > 0.05) {
        uv.x = uv.x + time * sign(s - 0.5);
        let cc = floor(uv.x) + floor(uv.y);
        color = cc - 2.0 * floor(cc / 2.0);
    }

    var hidden = false;
    if (invert == 0) {
        if (color == 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn cut_tileillusion_f(x: f32) -> f32 {
    return x + 0.1 * sin(1.6 * x);
}
fn cut_tileillusion_solve(x0i: f32, x1i: f32, y: f32) -> f32 {
    var x0 = x0i;
    var x1 = x1i;
    var y0 = cut_tileillusion_f(x0);
    var y1 = cut_tileillusion_f(x1);
    if (y1 < y0) {
        let x2 = x1; x1 = x0; x0 = x2;
        let y2 = y1; y1 = y0; y0 = y2;
    }
    var xn = 0.0;
    var yn = 0.0;
    for (var i = 0; i < 20; i = i + 1) {
        xn = x0 + (x1 - x0) / (y1 - y0) * (y - y0);
        yn = cut_tileillusion_f(xn);
        if (yn > y) { x1 = xn; y1 = yn; } else { x0 = xn; y0 = yn; }
    }
    return xn;
}
fn variation_cut_tileillusion(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let time = get_param(xform_id, variation_id, 0u);
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var uv = vec2<f32>(x * zoom, y * zoom);
    let fuvy = cut_tileillusion_f(uv.y);
    let y0 = cut_tileillusion_solve(uv.y - 3.0, uv.y, floor(fuvy));
    let y1 = cut_tileillusion_solve(uv.y, uv.y + 3.0, floor(fuvy) + 1.0);
    uv.y = fuvy;
    uv.x = uv.x / (y1 - y0);
    let fy = floor(uv.y);
    let s = fy - 2.0 * floor(fy / 2.0);
    var color = 0.0;
    if (fract(uv.y) > 0.05) {
        uv.x = uv.x + time * sign(s - 0.5);
        let cc = floor(uv.x) + floor(uv.y);
        color = cc - 2.0 * floor(cc / 2.0);
    }

    var hidden = false;
    if (invert == 0) {
        if (color == 0.0) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (color > 0.0) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};

// =============================================================================
// cut_pattern
// =============================================================================
pub static CUT_PATTERN: VariationDef = VariationDef {
    name: "cut_pattern",
    aliases: &[],
    display_name: "Cut Pattern",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("time", "Time", unlimited_float, 0.9, -100.0, 100.0, "Animates the radial wave phase."),
        param!("zoom", "Zoom", unlimited_float, 0.5, -50.0, 50.0, "Pattern scale: `point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_pattern_rotate2D(st0: vec2<f32>, angle: f32) -> vec2<f32> {
    var st = st0 - 0.5;
    let c = cos(angle);
    let s = sin(angle);
    st = vec2<f32>(c * st.x - s * st.y, s * st.x + c * st.y);
    return st + 0.5;
}
fn cut_pattern_rotateTilePattern(st0: vec2<f32>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    var st = st0 * 2.0;
    var index = 0.0;
    index = index + step(1.0, st.x - 2.0 * floor(st.x / 2.0));
    index = index + step(1.0, st.y - 2.0 * floor(st.y / 2.0)) * 2.0;
    st = fract(st);
    if (index == 0.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 1.0 / 4.0); }
    else if (index == 1.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 3.0 / 4.0); }
    else if (index == 2.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 1.0 / 4.0); }
    else if (index == 3.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 3.0 / 4.0); }
    return st;
}
fn variation_cut_pattern(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));
    const PI: f32 = 3.14159265359;

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var uv = vec2<f32>(x, y) * zoom;
    uv = fract(uv);
    uv = cut_pattern_rotateTilePattern(uv);
    let pos = vec2<f32>(0.0, 5.0) - uv;
    let radius = length(pos);
    // JWF also computes angle = atan2(pos.x, pos.y) here but never uses it; omitted.
    let r = sin(radius * sin(uv.y * PI * 5.0 + time + cos(sin(uv.x * PI * 3.0) * PI * 2.0 + sin(uv.y * PI * 15.0))) * 1.0 * sin(uv.y * PI + sin(uv.x * PI * 5.0)));
    let color = cos(r * PI * 2.0 + PI * 2.0) * 0.9 + sin(r * PI * 2.0 + PI * 2.0) * 0.7;

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
fn cut_pattern_rotate2D(st0: vec2<f32>, angle: f32) -> vec2<f32> {
    var st = st0 - 0.5;
    let c = cos(angle);
    let s = sin(angle);
    st = vec2<f32>(c * st.x - s * st.y, s * st.x + c * st.y);
    return st + 0.5;
}
fn cut_pattern_rotateTilePattern(st0: vec2<f32>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    var st = st0 * 2.0;
    var index = 0.0;
    index = index + step(1.0, st.x - 2.0 * floor(st.x / 2.0));
    index = index + step(1.0, st.y - 2.0 * floor(st.y / 2.0)) * 2.0;
    st = fract(st);
    if (index == 0.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 1.0 / 4.0); }
    else if (index == 1.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 3.0 / 4.0); }
    else if (index == 2.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 1.0 / 4.0); }
    else if (index == 3.0) { st = cut_pattern_rotate2D(st, PI * 2.0 * 3.0 / 4.0); }
    return st;
}
fn variation_cut_pattern(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));
    const PI: f32 = 3.14159265359;

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    var uv = vec2<f32>(x, y) * zoom;
    uv = fract(uv);
    uv = cut_pattern_rotateTilePattern(uv);
    let pos = vec2<f32>(0.0, 5.0) - uv;
    let radius = length(pos);
    let r = sin(radius * sin(uv.y * PI * 5.0 + time + cos(sin(uv.x * PI * 3.0) * PI * 2.0 + sin(uv.y * PI * 15.0))) * 1.0 * sin(uv.y * PI + sin(uv.x * PI * 5.0)));
    let color = cos(r * PI * 2.0 + PI * 2.0) * 0.9 + sin(r * PI * 2.0 + PI * 2.0) * 0.7;

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
// cut_celtic
// =============================================================================
pub static CUT_CELTIC: VariationDef = VariationDef {
    name: "cut_celtic",
    aliases: &[],
    display_name: "Cut Celtic",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("zoom", "Zoom", unlimited_float, 5.0, -50.0, 50.0, "Pattern scale (knot count): `point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cut_celtic_circ(uv: vec2<f32>, r: f32) -> f32 {
    let d = length(uv);
    return smoothstep(d, d + 0.02, r);
}
fn cut_celtic_celticShit(uv: vec2<f32>, r: f32) -> f32 {
    let r1 = 0.38;
    let r2 = 0.45;
    let c1 = cut_celtic_circ(uv, r1);
    let c5 = cut_celtic_circ(uv, r2);
    let c2 = cut_celtic_circ(uv + vec2<f32>(0.5, -0.288675), r2);
    let c3 = cut_celtic_circ(uv + vec2<f32>(-0.5, -0.288675), r2);
    let c4 = cut_celtic_circ(uv + vec2<f32>(0.0, 0.57735), r2);
    let d = c5 - c2 - c3 - c4;
    return mix(0.0, d, c1);
}
fn variation_cut_celtic(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }

    var uv1 = vec2<f32>(x * 1.0, y * 1.155) * zoom;
    let m = uv1.y - 2.0 * floor(uv1.y / 2.0);
    uv1.x = uv1.x + step(1.0, m) * 0.5;
    uv1 = fract(uv1);
    uv1.y = uv1.y / 1.155;
    uv1.x = uv1.x - 1.155 / 2.0;
    uv1.y = uv1.y - 0.5;

    var uv2 = vec2<f32>(x * 1.0, y * 1.155) * zoom;
    uv2.x = uv2.x - 0.5;
    uv2.y = uv2.y - 0.288675;
    let n = uv2.y - 2.0 * floor(uv2.y / 2.0);
    uv2.x = uv2.x + step(1.0, n) * 0.5;
    uv2 = fract(uv2);
    uv2.y = uv2.y / 1.155;
    uv2.x = uv2.x - 1.155 / 2.0;
    uv2.y = uv2.y - 0.5;

    var uv3 = vec2<f32>(x * 1.0, y * 1.155) * zoom;
    uv3.x = uv3.x + 1.0;
    uv3.y = uv3.y - 0.65;
    let o = uv3.y - 2.0 * floor(uv3.y / 2.0);
    uv3.x = uv3.x + step(1.0, o) * 0.5;
    uv3 = fract(uv3);
    uv3.y = uv3.y / 1.155;
    uv3.x = uv3.x - 1.155 / 2.0;
    uv3.y = uv3.y - 0.5;

    let color = cut_celtic_celticShit(uv1, 0.5) + cut_celtic_celticShit(uv2, 0.5) + cut_celtic_celticShit(uv3, 0.5);

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
fn cut_celtic_circ(uv: vec2<f32>, r: f32) -> f32 {
    let d = length(uv);
    return smoothstep(d, d + 0.02, r);
}
fn cut_celtic_celticShit(uv: vec2<f32>, r: f32) -> f32 {
    let r1 = 0.38;
    let r2 = 0.45;
    let c1 = cut_celtic_circ(uv, r1);
    let c5 = cut_celtic_circ(uv, r2);
    let c2 = cut_celtic_circ(uv + vec2<f32>(0.5, -0.288675), r2);
    let c3 = cut_celtic_circ(uv + vec2<f32>(-0.5, -0.288675), r2);
    let c4 = cut_celtic_circ(uv + vec2<f32>(0.0, 0.57735), r2);
    let d = c5 - c2 - c3 - c4;
    return mix(0.0, d, c1);
}
fn variation_cut_celtic(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let zoom = get_param(xform_id, variation_id, 1u);
    let invert = i32(get_param(xform_id, variation_id, 2u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }

    var uv1 = vec2<f32>(x * 1.0, y * 1.155) * zoom;
    let m = uv1.y - 2.0 * floor(uv1.y / 2.0);
    uv1.x = uv1.x + step(1.0, m) * 0.5;
    uv1 = fract(uv1);
    uv1.y = uv1.y / 1.155;
    uv1.x = uv1.x - 1.155 / 2.0;
    uv1.y = uv1.y - 0.5;

    var uv2 = vec2<f32>(x * 1.0, y * 1.155) * zoom;
    uv2.x = uv2.x - 0.5;
    uv2.y = uv2.y - 0.288675;
    let n = uv2.y - 2.0 * floor(uv2.y / 2.0);
    uv2.x = uv2.x + step(1.0, n) * 0.5;
    uv2 = fract(uv2);
    uv2.y = uv2.y / 1.155;
    uv2.x = uv2.x - 1.155 / 2.0;
    uv2.y = uv2.y - 0.5;

    var uv3 = vec2<f32>(x * 1.0, y * 1.155) * zoom;
    uv3.x = uv3.x + 1.0;
    uv3.y = uv3.y - 0.65;
    let o = uv3.y - 2.0 * floor(uv3.y / 2.0);
    uv3.x = uv3.x + step(1.0, o) * 0.5;
    uv3 = fract(uv3);
    uv3.y = uv3.y / 1.155;
    uv3.x = uv3.x - 1.155 / 2.0;
    uv3.y = uv3.y - 0.5;

    let color = cut_celtic_celticShit(uv1, 0.5) + cut_celtic_celticShit(uv2, 0.5) + cut_celtic_celticShit(uv3, 0.5);

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
