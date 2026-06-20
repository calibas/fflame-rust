//! cut_* simple/inline stencils (Jesus Sosa) — a batch of JWildfire
//! "cut_*" procedural masks whose GPU bodies are self-contained (no helper
//! functions), collected into one file.
//!
//! Each is a shadertoy-style mask: it samples a point (the affine input in
//! `mode=0`, or a fresh random point in `mode=1`), evaluates a procedural
//! pattern, and HIDES points crossing a threshold via `Feature::CanHide`
//! (JWildfire's `pVarTP.doHide`). Output is a replace (`pVarTP.x =
//! pAmount·x`). Patterns are transcribed from the GPU bodies in the
//! corresponding `Cut*Func.java` sources.
//!
//! Contained here:
//!   - cut_chains     — interlocking-chain sine field
//!   - cut_hexdots    — hexagonal nearest-cell dots
//!   - cut_booleans   — XOR/AND/OR bit-pattern of the integer grid
//!   - cut_sqsplits   — radially-split square tiles
//!   - cut_btree      — recursive binary-tree bands (exp2/ceil folding)
//!   - cut_fractal    — abs/inversion Julia-style escape mask
//!
//! Note: GLSL/CUDA `mod(a,b)` is inlined as `a − b·floor(a/b)` (WGSL has no
//! `mod` builtin and a shared helper would collide across the concatenated
//! per-variation WGSL). Helper-free per the family convention; `seed` is
//! JWF's UI wall-clock hack and is declared (where present) only for
//! `.flame` round-trip — the GPU never reads it.
//!
//! # Authors
//! - Jesus Sosa (cut_* family)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// cut_chains
// =============================================================================
pub static CUT_CHAINS: VariationDef = VariationDef {
    name: "cut_chains",
    aliases: &[],
    display_name: "Cut Chains",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("shiftX", "Shift X", unlimited_float, 0.0, -50.0, 50.0, "X offset added to `uv` after scaling."),
        param!("shiftY", "Shift Y", unlimited_float, 0.0, -50.0, 50.0, "Y offset added to `uv` after scaling."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("zoom", "Zoom", unlimited_float, 2.5, -50.0, 50.0, "Pattern scale: `uv = point·zoom + shift`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_chains(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let shiftx = get_param(xform_id, variation_id, 0u);
    let shifty = get_param(xform_id, variation_id, 1u);
    let mode = i32(get_param(xform_id, variation_id, 2u));
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) {
        x = p.x; y = p.y;
    } else {
        x = rng_nextf(rng); y = rng_nextf(rng);
        cx = 0.5; cy = 0.5;
    }
    let u = vec2<f32>(x * zoom, y * zoom) + vec2<f32>(shiftx, shifty);
    let wy = cos(u.y * 12.0);
    let wx = sin(u.x * 15.0);
    let w = cos(u.x * 30.0);
    let color = smoothstep(-0.25, 0.25, mix(wy + wx, wx * 1.4, w * 3.0));

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
fn variation_cut_chains(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let shiftx = get_param(xform_id, variation_id, 0u);
    let shifty = get_param(xform_id, variation_id, 1u);
    let mode = i32(get_param(xform_id, variation_id, 2u));
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) {
        x = p.x; y = p.y;
    } else {
        x = rng_nextf(rng); y = rng_nextf(rng);
        cx = 0.5; cy = 0.5;
    }
    let u = vec2<f32>(x * zoom, y * zoom) + vec2<f32>(shiftx, shifty);
    let wy = cos(u.y * 12.0);
    let wx = sin(u.x * 15.0);
    let w = cos(u.x * 30.0);
    let color = smoothstep(-0.25, 0.25, mix(wy + wx, wx * 1.4, w * 3.0));

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
// cut_hexdots
// =============================================================================
pub static CUT_HEXDOTS: VariationDef = VariationDef {
    name: "cut_hexdots",
    aliases: &[],
    display_name: "Cut HexDots",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("size", "Size", float, 0.5, 0.01, 1.0, "Dot threshold: larger keeps more of each hex cell."),
        param!("zoom", "Zoom", float, 4.0, 0.1, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_hexdots(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let size = get_param(xform_id, variation_id, 1u);
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let u = vec2<f32>(x * zoom, y * zoom);
    let s = vec2<f32>(1.0, 1.732);
    let a = (u - s * floor(u / s)) * 2.0 - s;
    let ub = u + s * 0.5;
    let b = (ub - s * floor(ub / s)) * 2.0 - s;
    let col = 0.8 * min(dot(a, a), dot(b, b));

    var hidden = false;
    if (invert == 0) {
        if (col > size) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (col <= size) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_cut_hexdots(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 0u));
    let size = get_param(xform_id, variation_id, 1u);
    let zoom = get_param(xform_id, variation_id, 2u);
    let invert = i32(get_param(xform_id, variation_id, 3u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let u = vec2<f32>(x * zoom, y * zoom);
    let s = vec2<f32>(1.0, 1.732);
    let a = (u - s * floor(u / s)) * 2.0 - s;
    let ub = u + s * 0.5;
    let b = (ub - s * floor(ub / s)) * 2.0 - s;
    let col = 0.8 * min(dot(a, a), dot(b, b));

    var hidden = false;
    if (invert == 0) {
        if (col > size) { x = 0.0; y = 0.0; hidden = true; }
    } else {
        if (col <= size) { x = 0.0; y = 0.0; hidden = true; }
    }
    *hide = hidden;
    return vec3<f32>(x, y, p.z);
}
"#,
};

// =============================================================================
// cut_booleans
// =============================================================================
pub static CUT_BOOLEANS: VariationDef = VariationDef {
    name: "cut_booleans",
    aliases: &[],
    display_name: "Cut Booleans",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("type", "Type", enum, 0, &["Xor", "And", "Or"], "Bitwise op applied to the integer grid coordinates."),
        param!("zoom", "Zoom", unlimited_float, 250.0, -1000.0, 1000.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_booleans(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let btype = i32(get_param(xform_id, variation_id, 2u));
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }
    let u = vec2<f32>(x, y) * zoom;
    let ix = i32(u.x);
    let iy = i32(u.y);
    var temp = 0;
    if (btype == 0) { temp = ix ^ iy; } else if (btype == 1) { temp = ix & iy; } else { temp = ix | iy; }
    let color = sin(f32(temp));

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
fn variation_cut_booleans(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let btype = i32(get_param(xform_id, variation_id, 2u));
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    var cx = 0.0;
    var cy = 0.0;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng); y = rng_nextf(rng); cx = 0.5; cy = 0.5; }
    let u = vec2<f32>(x, y) * zoom;
    let ix = i32(u.x);
    let iy = i32(u.y);
    var temp = 0;
    if (btype == 0) { temp = ix ^ iy; } else if (btype == 1) { temp = ix & iy; } else { temp = ix | iy; }
    let color = sin(f32(temp));

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
// cut_sqsplits
// =============================================================================
pub static CUT_SQSPLITS: VariationDef = VariationDef {
    name: "cut_sqsplits",
    aliases: &[],
    display_name: "Cut SqSplits",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 1000.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("time", "Time", unlimited_float, 0.9, -100.0, 100.0, "Phase of the radial split rotation."),
        param!("zoom", "Zoom", unlimited_float, 2.0, -50.0, 50.0, "Pattern scale: `uv = point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_sqsplits(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let uu = vec2<f32>(x * zoom, y * zoom);
    let uf = fract(uu) - 0.5;
    let ui = floor(uu - 2.0 * floor(uu / 2.0));
    let t = time * 0.35;
    let a = atan2(uf.x, uf.y) / 6.3;
    let r = fract(t + t + 2.0 * select(1.0 - a, a, ui.x != ui.y));
    let sharp = 180.0 * length(uf);
    let k = clamp((r - 0.5) * sharp, 0.0, 1.0) + clamp(1.0 - r * sharp, 0.0, 1.0);
    let color = 1.0 - k;

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
fn variation_cut_sqsplits(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 4u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let uu = vec2<f32>(x * zoom, y * zoom);
    let uf = fract(uu) - 0.5;
    let ui = floor(uu - 2.0 * floor(uu / 2.0));
    let t = time * 0.35;
    let a = atan2(uf.x, uf.y) / 6.3;
    let r = fract(t + t + 2.0 * select(1.0 - a, a, ui.x != ui.y));
    let sharp = 180.0 * length(uf);
    let k = clamp((r - 0.5) * sharp, 0.0, 1.0) + clamp(1.0 - r * sharp, 0.0, 1.0);
    let color = 1.0 - k;

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
// cut_btree
// =============================================================================
pub static CUT_BTREE: VariationDef = VariationDef {
    name: "cut_btree",
    aliases: &[],
    display_name: "Cut BTree",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 0.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("time", "Time", unlimited_float, 1.0, -100.0, 100.0, "Branch-level phase (via `fract(time)`)."),
        param!("thickness", "Thickness", float, 5.0, 1.0, 10.0, "Branch thickness divisor: larger = thinner branches."),
        param!("zoom", "Zoom", unlimited_float, 1.0, -50.0, 50.0, "Pattern scale: `w = point·zoom`."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("invert", "Invert", bool, true, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_btree(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let time = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let mode = i32(get_param(xform_id, variation_id, 4u));
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = 2.0 * rng_nextf(rng) - 1.0; y = 2.0 * rng_nextf(rng) - 1.0; }
    var w = vec2<f32>(x * zoom, y * zoom);
    var tv = (w / w) * fract(time);
    let l = w * w * 5.0 - tv;
    tv = tv + 1.0;
    let m = exp2(ceil(l)) * tv;
    w = fract(m * w.x) - 0.5;
    let t0 = sign(w) / 4.0;
    let t1 = abs(smoothstep(vec2<f32>(0.0), vec2<f32>(1.0), fract(l)) * t0 - w);
    let t2 = m / t1 / (100.0 * thickness);
    let color = t2.y;

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
fn variation_cut_btree(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let time = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let zoom = get_param(xform_id, variation_id, 3u);
    let mode = i32(get_param(xform_id, variation_id, 4u));
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = 2.0 * rng_nextf(rng) - 1.0; y = 2.0 * rng_nextf(rng) - 1.0; }
    var w = vec2<f32>(x * zoom, y * zoom);
    var tv = (w / w) * fract(time);
    let l = w * w * 5.0 - tv;
    tv = tv + 1.0;
    let m = exp2(ceil(l)) * tv;
    w = fract(m * w.x) - 0.5;
    let t0 = sign(w) / 4.0;
    let t1 = abs(smoothstep(vec2<f32>(0.0), vec2<f32>(1.0), fract(l)) * t0 - w);
    let t2 = m / t1 / (100.0 * thickness);
    let color = t2.y;

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

// =============================================================================
// cut_fractal
// =============================================================================
pub static CUT_FRACTAL: VariationDef = VariationDef {
    name: "cut_fractal",
    aliases: &[],
    display_name: "Cut Fractal",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::Replace, Feature::CanHide],
    parameters: &[
        param!("seed", "Seed", int, 0.0, 0.0, 1000000.0, "UI-only in JWildfire; ignored by the renderer."),
        param!("mode", "Mode", enum, 1, &["Affine Input", "Random Point"], "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call."),
        param!("time", "Time", unlimited_float, 0.0, -100.0, 100.0, "Animates the fold constant `c`."),
        param!("iters", "Iterations", int, 30.0, 1.0, 100.0, "Number of abs/inversion fold iterations."),
        param!("zoom", "Zoom", unlimited_float, 1.0, -50.0, 50.0, "Pattern scale: `point·zoom`."),
        param!("invert", "Invert", bool, false, "Invert the cut — keep what would be hidden and vice versa."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cut_fractal(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let iters = i32(get_param(xform_id, variation_id, 3u));
    let zoom = get_param(xform_id, variation_id, 4u);
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let c = vec2<f32>(cos(time + 1.5), sin(time + 1.8)) * 0.15 - vec2<f32>(0.25);
    let tt = vec2<f32>(x * zoom, y * zoom);
    var u = vec2<f32>(tt.y, tt.x);
    for (var i = 0; i < iters; i = i + 1) {
        u = abs(u) * (0.5 / dot(u, u)) + c;
    }
    let color = abs(dot(u, u) - 0.01) * 2.0;

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
fn variation_cut_fractal(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let time = get_param(xform_id, variation_id, 2u);
    let iters = i32(get_param(xform_id, variation_id, 3u));
    let zoom = get_param(xform_id, variation_id, 4u);
    let invert = i32(get_param(xform_id, variation_id, 5u));

    var x: f32;
    var y: f32;
    if (mode == 0) { x = p.x; y = p.y; } else { x = rng_nextf(rng) - 0.5; y = rng_nextf(rng) - 0.5; }
    let c = vec2<f32>(cos(time + 1.5), sin(time + 1.8)) * 0.15 - vec2<f32>(0.25);
    let tt = vec2<f32>(x * zoom, y * zoom);
    var u = vec2<f32>(tt.y, tt.x);
    for (var i = 0; i < iters; i = i + 1) {
        u = abs(u) * (0.5 / dot(u, u)) + c;
    }
    let color = abs(dot(u, u) - 0.01) * 2.0;

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
