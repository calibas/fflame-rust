//! Miscellaneous small 2D variations
//!
//! Eight small standalone 2D variations from upstream's
//! port-candidate list — all factor VVAR cleanly through the outer
//! multiplier (with one internal-weight exception, `tile_hlp`):
//!
//!   - `split`         — coin-flip-mirror per axis based on cosine of
//!                       scaled coords
//!   - `squirrel`      — `cos(sqrt(u)) · tan(x)` style warp
//!   - `stripes`       — vertical-stripe warp with parabolic Y bend
//!   - `shift`         — rotated additive shift (Tatyana Zabanova)
//!   - `pressure_wave` — sin-wave additive distortion (timothy-vincent)
//!   - `sphericaln`    — N-power spherical with random branch
//!   - `spligon`       — polygonal spike-tiler
//!   - `tile_hlp`      — tile-stripe blur (Zy0rg) — uses internal weight,
//!                       needs `needs_transform` for the offset term
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`. All factor VVAR cleanly except
//! `tile_hlp` (the offset is `width · VVAR` — needs `needs_transform`
//! to read the per-variation weight directly).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// split: coin-flip mirror per axis (Apophysis pack)
//   if cos(x · xsize · π) >= 0:  out_y = +y  else: out_y = -y
//   if cos(y · ysize · π) >= 0:  out_x = +x  else: out_x = -x
// =============================================================================
/// Coin-flip mirror per axis — flips Y based on the sign of
/// `cos(x · xsize · π)`, and flips X based on the sign of
/// `cos(y · ysize · π)`. Produces a tiled mirroring pattern whose tile
/// spacing is controlled by `xsize` and `ysize`.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static SPLIT: VariationDef = VariationDef {
    name: "split",
    aliases: &[],
    display_name: "Split",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("xsize", "X size", unlimited_float, 0.4, -10.0, 10.0, "X-axis frequency of the mirror-flip cosine."),
        param!("ysize", "Y size", unlimited_float, 0.6, -10.0, 10.0, "Y-axis frequency of the mirror-flip cosine."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_split(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xsize = get_param(xform_id, variation_id, 0u);
    let ysize = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    var oy = p.y;
    if (cos(p.x * xsize * pi) < 0.0) { oy = -p.y; }
    var ox = p.x;
    if (cos(p.y * ysize * pi) < 0.0) { ox = -p.x; }
    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: r#"
fn variation_split(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xsize = get_param(xform_id, variation_id, 0u);
    let ysize = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    var oy = p.y;
    if (cos(p.x * xsize * pi) < 0.0) { oy = -p.y; }
    var ox = p.x;
    if (cos(p.y * ysize * pi) < 0.0) { ox = -p.x; }
    return vec3<f32>(ox, oy, p.z);
}
"#,
};

// =============================================================================
// squirrel: cos·tan composite warp (Raykoid666)
//   u = (a + ε) · x² + (b + ε) · y²
//   out = (cos(sqrt(u)) · tan(x), sin(sqrt(u)) · tan(y))
//
// Note: cpp uses `FPx = ... * VVAR` (assignment, not `+=`). With
// `+=`-friendly outer multiplier our return matches when squirrel is
// the only normal variation in its transform — typical use; documented
// in the existing crop3d caveat pattern.
// =============================================================================
/// Cos/tan composite warp — outputs `(cos(s)·tan(x), sin(s)·tan(y))` where
/// `s = sqrt(a·x² + b·y²)`. The tangent terms produce vertical and
/// horizontal stripes near `x = π/2 + kπ` etc.
///
/// # Authors
/// - Raykoid666
pub static SQUIRREL: VariationDef = VariationDef {
    name: "squirrel",
    aliases: &[],
    display_name: "Squirrel",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::Replace],
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "X² weight in the radius term."),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0, "Y² weight in the radius term."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_squirrel(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let eps = 1e-6;
    let u = (a + eps) * p.x * p.x + (b + eps) * p.y * p.y;
    let s = sqrt(max(u, 0.0));
    return vec2<f32>(cos(s) * tan(p.x), sin(s) * tan(p.y));
}
"#,
    wgsl_3d: r#"
fn variation_squirrel(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let eps = 1e-6;
    let u = (a + eps) * p.x * p.x + (b + eps) * p.y * p.y;
    let s = sqrt(max(u, 0.0));
    return vec3<f32>(cos(s) * tan(p.x), sin(s) * tan(p.y), p.z);
}
"#,
};

// =============================================================================
// stripes: vertical stripes with parabolic Y bend (apo plugins pack)
//   round_x = floor(x + 0.5);  off = x − round_x
//   out_x = off · (1 − space) + round_x
//   out_y = y + off² · warp
// =============================================================================
/// Vertical-stripe warp with parabolic Y bend — snaps X toward the
/// nearest integer (with the local offset compressed by `1 − space`),
/// and bends Y as a parabola of the local stripe offset. Produces
/// vertical stripes that arch up or down depending on `warp`.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static STRIPES: VariationDef = VariationDef {
    name: "stripes",
    aliases: &[],
    display_name: "Stripes",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("space", "Space", unlimited_float, 0.2, -5.0, 5.0, "Compression factor for X within each stripe — 1 collapses to integer values, 0 disables compression."),
        param!("warp", "Warp", unlimited_float, 0.6, -5.0, 5.0, "Parabolic Y-bend amplitude as a function of local stripe offset."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_stripes(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let space = get_param(xform_id, variation_id, 0u);
    let warp = get_param(xform_id, variation_id, 1u);
    let round_x = floor(p.x + 0.5);
    let off = p.x - round_x;
    return vec2<f32>(
        off * (1.0 - space) + round_x,
        p.y + off * off * warp,
    );
}
"#,
    wgsl_3d: r#"
fn variation_stripes(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let space = get_param(xform_id, variation_id, 0u);
    let warp = get_param(xform_id, variation_id, 1u);
    let round_x = floor(p.x + 0.5);
    let off = p.x - round_x;
    return vec3<f32>(
        off * (1.0 - space) + round_x,
        p.y + off * off * warp,
        p.z,
    );
}
"#,
};

// =============================================================================
// shift: rotated additive shift (Tatyana Zabanova / Stefanov)
//   ang = angle · π / 180
//   out_x = x + cos(ang) · shift_x − sin(ang) · shift_y
//   out_y = y − cos(ang) · shift_y − sin(ang) · shift_x
//   (pre-computes sin/cos of ang via init slot pair)
// =============================================================================
/// Rotated additive shift — adds a 2D shift vector to the input, where the
/// shift direction is rotated by `angle` degrees. Equivalent to translating
/// in a rotated frame.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov
pub static SHIFT: VariationDef = VariationDef {
    name: "shift",
    aliases: &[],
    display_name: "Shift",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("shift_x", "Shift X", unlimited_float, 0.0, -10.0, 10.0, "X-axis shift amount (in the rotated frame)."),
        param!("shift_y", "Shift Y", unlimited_float, 0.0, -10.0, 10.0, "Y-axis shift amount (in the rotated frame)."),
        param!("angle", "Angle (deg)", angle, 0.0, "Rotation angle of the shift vector, in degrees."),
    ],
    // 2 derived values at slots 3..5:
    //   3: cos_ang
    //   4: sin_ang
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_shift(user: array<f32, 3>) -> array<f32, 2> {
    let ang_rad = user[2] * 3.14159265358979 / 180.0;
    var out: array<f32, 2>;
    out[0] = cos(ang_rad);
    out[1] = sin(ang_rad);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_shift(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sx = get_param(xform_id, variation_id, 0u);
    let sy = get_param(xform_id, variation_id, 1u);
    let cs = get_param(xform_id, variation_id, 3u);
    let sn = get_param(xform_id, variation_id, 4u);
    return vec2<f32>(
        p.x + cs * sx - sn * sy,
        p.y - cs * sy - sn * sx,
    );
}
"#,
    wgsl_3d: r#"
fn variation_shift(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sx = get_param(xform_id, variation_id, 0u);
    let sy = get_param(xform_id, variation_id, 1u);
    let cs = get_param(xform_id, variation_id, 3u);
    let sn = get_param(xform_id, variation_id, 4u);
    return vec3<f32>(
        p.x + cs * sx - sn * sy,
        p.y - cs * sy - sn * sx,
        p.z,
    );
}
"#,
};

// =============================================================================
// pressure_wave: sin-wave additive distortion (timothy-vincent / DarkBeam)
//   if x_freq == 0:  pwx = 1, ipwx = 1
//   else:            pwx = x_freq · 2π,  ipwx = 1/pwx
//   (same for y)
//   out = (x + ipwx · sin(pwx · x), y + ipwy · sin(pwy · y))
//
// (cpp PluginVarPrepare hardcoded the four init values to 1.0 — porter
// bug; the actual derivation lives in Java's `setParameter`. Recovered
// from the comment block.)
// =============================================================================
/// Sin-wave additive distortion — adds `sin(2π·freq·coord) / (2π·freq)` to
/// each axis. Amplitude is inversely proportional to frequency, so higher-
/// frequency ripples are smaller. With `freq = 0`, the term degenerates to
/// `sin(coord)`.
///
/// # Authors
/// - timothy-vincent
/// - DarkBeam
pub static PRESSURE_WAVE: VariationDef = VariationDef {
    name: "pressure_wave",
    aliases: &[],
    display_name: "Pressure Wave",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("x_freq", "X freq", unlimited_float, 1.0, -50.0, 50.0, "X-axis sine frequency. Amplitude scales as `1 / (2π·freq)`; 0 degenerates to `sin(x)`."),
        param!("y_freq", "Y freq", unlimited_float, 1.0, -50.0, 50.0, "Y-axis sine frequency. Same scaling rule as x_freq."),
    ],
    // 4 derived values at slots 2..6:
    //   2: pwx   (x_freq · 2π,  or 1.0 if x_freq == 0)
    //   3: pwy   (y_freq · 2π,  or 1.0)
    //   4: ipwx  (1 / pwx)
    //   5: ipwy  (1 / pwy)
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_pressure_wave(user: array<f32, 2>) -> array<f32, 4> {
    let two_pi = 6.28318530717959;
    var pwx = 1.0;
    var pwy = 1.0;
    if (user[0] != 0.0) { pwx = user[0] * two_pi; }
    if (user[1] != 0.0) { pwy = user[1] * two_pi; }
    var out: array<f32, 4>;
    out[0] = pwx;
    out[1] = pwy;
    out[2] = 1.0 / pwx;
    out[3] = 1.0 / pwy;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_pressure_wave(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let pwx = get_param(xform_id, variation_id, 2u);
    let pwy = get_param(xform_id, variation_id, 3u);
    let ipwx = get_param(xform_id, variation_id, 4u);
    let ipwy = get_param(xform_id, variation_id, 5u);
    return vec2<f32>(
        p.x + ipwx * sin(pwx * p.x),
        p.y + ipwy * sin(pwy * p.y),
    );
}
"#,
    wgsl_3d: r#"
fn variation_pressure_wave(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let pwx = get_param(xform_id, variation_id, 2u);
    let pwy = get_param(xform_id, variation_id, 3u);
    let ipwx = get_param(xform_id, variation_id, 4u);
    let ipwy = get_param(xform_id, variation_id, 5u);
    return vec3<f32>(
        p.x + ipwx * sin(pwx * p.x),
        p.y + ipwy * sin(pwy * p.y),
        p.z,
    );
}
"#,
};

// =============================================================================
// sphericaln: N-power spherical with random branch
//   R = (sqrt(x² + y²))^dist
//   N = floor(power · rand)
//   α = atan2(y, x) + N · 2π / floor(power)
//   out = (cos α / R, sin α / R)
// =============================================================================
/// N-power spherical with random branch — combines a spherical inversion
/// `1/r^dist` with a random angular shift `n · 2π / ⌊|power|⌋` where `n =
/// ⌊power · rand⌋`. Equivalent to spherical-inverting the input and then
/// rotating by one of `⌊|power|⌋` random angles.
/// 
/// # Authors
/// - eralex61
pub static SPHERICALN: VariationDef = VariationDef {
    name: "sphericalN",
    aliases: &[],
    display_name: "Spherical N",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("power", "Power", unlimited_float, 1.0, -50.0, 50.0, "Number of angular branches — `⌊|power|⌋` determines the spoke count, and each iteration picks one at random."),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0, "Radial-power exponent — output radius is `1 / r^dist`. `dist = 1` reduces to a standard spherical inversion."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sphericalN(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);
    let two_pi = 6.28318530717959;
    let safe_power_floor = max(floor(abs(power)), 1.0);
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let R = pow(max(r0, 1e-30), dist);
    let n = floor(power * rng_nextf(rng));
    let alpha = atan2(p.y, p.x) + n * two_pi / safe_power_floor;
    return vec2<f32>(cos(alpha) / max(R, 1e-30), sin(alpha) / max(R, 1e-30));
}
"#,
    wgsl_3d: r#"
fn variation_sphericalN(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);
    let two_pi = 6.28318530717959;
    let safe_power_floor = max(floor(abs(power)), 1.0);
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let R = pow(max(r0, 1e-30), dist);
    let n = floor(power * rng_nextf(rng));
    let alpha = atan2(p.y, p.x) + n * two_pi / safe_power_floor;
    return vec3<f32>(cos(alpha) / max(R, 1e-30), sin(alpha) / max(R, 1e-30), p.z);
}
"#,
};

// =============================================================================
// spligon: polygonal spike-tiler
//   th    = sides / (2π)
//   thi   = 1 / th
//   j     = π · i / (-2 · sides)
//   t     = thi · floor(atan2(x, y) · th) + j      (upstream cpp swap)
//   out   = (x + cos(t), y + sin(t))               (r = 1, hardcoded)
// =============================================================================
/// Polygonal spike-tiler — snaps the input angle to the nearest of `sides`
/// polygon spokes (with a per-spoke offset from `i`), then adds a unit-
/// length spike at that angle to the input. Creates a tiling of spikes
/// radiating at regular angles.
///
/// # Authors
/// - DarkBeam
pub static SPLIGON: VariationDef = VariationDef {
    name: "spligon",
    aliases: &[],
    display_name: "Spligon",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("sides", "Sides", unlimited_float, 3.0, 1.0, 50.0, "Number of polygon spokes."),
        param!("i", "I", unlimited_float, 1.0, -10.0, 10.0, "Spike angular offset — rotates the spike direction within its spoke sector."),
    ],
    // 3 derived values at slots 2..5:
    //   2: th   (sides / 2π)
    //   3: thi  (1 / th)
    //   4: j    (π · i / (-2 · sides))
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_spligon(user: array<f32, 2>) -> array<f32, 3> {
    let sides = user[0];
    let i_val = user[1];
    let two_pi = 6.28318530717959;
    let pi = 3.14159265358979;
    let safe_sides = select(sides, 1e-30, sides == 0.0);
    let th = safe_sides / two_pi;
    var out: array<f32, 3>;
    out[0] = th;
    out[1] = 1.0 / th;
    out[2] = pi * i_val / (-2.0 * safe_sides);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_spligon(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let th = get_param(xform_id, variation_id, 2u);
    let thi = get_param(xform_id, variation_id, 3u);
    let j = get_param(xform_id, variation_id, 4u);
    let t = thi * floor(atan2(p.x, p.y) * th) + j;  // upstream cpp swap
    return vec2<f32>(p.x + cos(t), p.y + sin(t));
}
"#,
    wgsl_3d: r#"
fn variation_spligon(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let th = get_param(xform_id, variation_id, 2u);
    let thi = get_param(xform_id, variation_id, 3u);
    let j = get_param(xform_id, variation_id, 4u);
    let t = thi * floor(atan2(p.x, p.y) * th) + j;
    return vec3<f32>(p.x + cos(t), p.y + sin(t), p.z);
}
"#,
};

// =============================================================================
// tile_hlp: tile-stripe blur (Zy0rg / Zabanova / Stefanov)
//   width2 = width · w
//   x = FTx / width
//   aux = fractional-part-with-sign of x
//   if cos(aux · π) < 2·rand − 1:
//     aux2 = ±width2 (sign = -sign(x))
//   else:
//     aux2 = 0
//   FPx += FTx · w + aux2          (mixed: linear-in-w plus offset that
//                                    is also linear-in-w via width2)
//   FPy += w · FTy
//
// Both terms ARE linear in w, but the "+ aux2" branch isn't a clean
// outer-multiplier factor (the branch decision uses width and rand, not
// w). Read w via needs_transform and let the body apply it directly.
// =============================================================================
/// Tile-stripe blur — divides X into stripes of given `width`, and per
/// iteration either shifts X by ±width to a neighboring stripe (with the
/// probability driven by stripe position and a uniform random) or leaves it
/// alone. Y passes through. Produces blurred banding along X.
///
/// # Authors
/// - Zy0rg
/// - Tatyana Zabanova
/// - Brad Stefanov
pub static TILE_HLP: VariationDef = VariationDef {
    name: "tile_hlp",
    aliases: &[],
    display_name: "Tile HLP",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[
        param!("width", "Width", unlimited_float, 1.0, -10.0, 10.0, "Stripe width along X. Smaller width = more, narrower stripes."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_tile_hlp(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let width = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;

    let safe_width = select(width, 1e-30, width == 0.0);
    let width2 = safe_width * w;
    let x = p.x / safe_width;
    var aux: f32;
    if (x > 0.0) {
        aux = x - floor(x);
    } else {
        aux = x + ceil(-x);
    }
    aux = cos(aux * pi);
    var aux2: f32 = 0.0;
    if (aux < rng_nextf(rng) * 2.0 - 1.0) {
        aux2 = select(width2, -width2, x > 0.0);
    }
    let fpx = p.x * w + aux2;
    let fpy = w * p.y;
    return vec2<f32>(fpx * inv_w, fpy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_tile_hlp(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let width = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;

    let safe_width = select(width, 1e-30, width == 0.0);
    let width2 = safe_width * w;
    let x = p.x / safe_width;
    var aux: f32;
    if (x > 0.0) {
        aux = x - floor(x);
    } else {
        aux = x + ceil(-x);
    }
    aux = cos(aux * pi);
    var aux2: f32 = 0.0;
    if (aux < rng_nextf(rng) * 2.0 - 1.0) {
        aux2 = select(width2, -width2, x > 0.0);
    }
    let fpx = p.x * w + aux2;
    let fpy = w * p.y;
    return vec3<f32>(fpx * inv_w, fpy * inv_w, p.z);
}
"#,
};
