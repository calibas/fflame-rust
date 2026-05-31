//! Internal-weight watchlist + small misc batch
//!
//! Eight more variations:
//!   - `trade`       (Faber)              — two-disc swap; clean
//!   - `voron`       (eralex61)           — Voronoi-cell snap with hash
//!                                           noise lookup; clean
//!   - `squircular`  (?)                  — squircular Möbius warp;
//!                                           VVAR appears non-linearly
//!                                           in body (`VVAR² · r − ...`)
//!   - `flux`        (meckie)             — `xpw = x + VVAR` shift;
//!                                           additive internal weight
//!   - `rays`        (Z+ Jan 2007)        — RNG-driven ray spread;
//!                                           output quadratic in VVAR
//!   - `rays1`       (Raykoid666)         — `u = 1/tan(sqrt(t)) + VVAR·(2/π)²`;
//!                                           additive internal weight
//!   - `loonie2`     (dark-beam)          — N-sided loonie; sqrvvar=w²
//!                                           threshold, runtime loop
//!   - `fourth`      (guagapunyaimel)     — 4-quadrant compound
//!                                           (spherical/loonie/susan/linear)
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! `trade` and `voron` are clean factor-through-outer.
//! Everything else uses `needs_transform` for the various internal-VVAR
//! patterns.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// trade: two-disc swap (Michael Faber)
//   Init: c1 = r1 + d1;  c2 = r2 + d2
//   Body: if x > 0: try fitting into right disc (radius r1, center (c1, 0))
//                   and re-emit at left disc; or pass-through if outside
//         else:    mirror: fit left disc, re-emit at right; pass-through outside
// =============================================================================
/// Two-disc swap — defines two discs (one at `(r1+d1, 0)` with radius `r1`,
/// one at `(-(r2+d2), 0)` with radius `r2`). Points inside the right disc
/// get warped to the corresponding position in the left disc, and vice
/// versa; points outside both pass through.
///
/// # Authors
/// - Michael Faber
pub static TRADE: VariationDef = VariationDef {
    name: "trade",
    aliases: &[],
    display_name: "Trade",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("r1", "R1", unlimited_float, 1.0, 0.001, 10.0, "Right-disc radius."),
        param!("d1", "D1", unlimited_float, 1.0, -10.0, 10.0, "Right-disc center offset from origin — center sits at `(r1 + d1, 0)`."),
        param!("r2", "R2", unlimited_float, 1.0, 0.001, 10.0, "Left-disc radius."),
        param!("d2", "D2", unlimited_float, 1.0, -10.0, 10.0, "Left-disc center offset from origin — center sits at `(-(r2 + d2), 0)`."),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 4..6:
    //   4: c1  (r1 + d1)
    //   5: c2  (r2 + d2)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_trade(user: array<f32, 4>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = user[0] + user[1];
    out[1] = user[2] + user[3];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_trade(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let r1 = get_param(xform_id, variation_id, 0u);
    let r2 = get_param(xform_id, variation_id, 2u);
    let c1 = get_param(xform_id, variation_id, 4u);
    let c2 = get_param(xform_id, variation_id, 5u);

    if (p.x > 0.0) {
        var r = sqrt((c1 - p.x) * (c1 - p.x) + p.y * p.y);
        if (r <= r1) {
            r = r * r2 / max(r1, 1e-30);
            let a = atan2(p.y, c1 - p.x);
            return vec2<f32>(r * cos(a) - c2, r * sin(a));
        }
    } else {
        var r = sqrt((-c2 - p.x) * (-c2 - p.x) + p.y * p.y);
        if (r <= r2) {
            r = r * r1 / max(r2, 1e-30);
            let a = atan2(p.y, -c2 - p.x);
            return vec2<f32>(r * cos(a) + c1, r * sin(a));
        }
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_trade(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let r1 = get_param(xform_id, variation_id, 0u);
    let r2 = get_param(xform_id, variation_id, 2u);
    let c1 = get_param(xform_id, variation_id, 4u);
    let c2 = get_param(xform_id, variation_id, 5u);

    if (p.x > 0.0) {
        var r = sqrt((c1 - p.x) * (c1 - p.x) + p.y * p.y);
        if (r <= r1) {
            r = r * r2 / max(r1, 1e-30);
            let a = atan2(p.y, c1 - p.x);
            return vec3<f32>(r * cos(a) - c2, r * sin(a), p.z);
        }
    } else {
        var r = sqrt((-c2 - p.x) * (-c2 - p.x) + p.y * p.y);
        if (r <= r2) {
            r = r * r1 / max(r2, 1e-30);
            let a = atan2(p.y, -c2 - p.x);
            return vec3<f32>(r * cos(a) + c1, r * sin(a), p.z);
        }
    }
    return p;
}
"#,
};

// =============================================================================
// voron: Voronoi-cell snap with bit-mixed hash (eralex61)
//   3×3 cell scan; each cell yields 1..num candidate Voronoi sites,
//   site coords from a deterministic int hash. Output snaps the input
//   towards the nearest site by lerp factor `k`.
//
//   DiscretNoise(X) = ((n²·15731 + 789221)·n + 1376312589) & 0x7fffffff
//                     · AM    where AM = 1/2147483647 ≈ 4.66e−10
//   (cpp's int math wraps on overflow; WGSL i32 also wraps. Match.)
// =============================================================================
/// Voronoi-cell snap with hash noise — scans the 3×3 grid of cells around
/// the input, generates 1–`num` deterministic Voronoi site positions per
/// cell via a bit-mixed integer hash, finds the nearest site, and lerps the
/// input toward it by factor `k`. Produces classic Voronoi cellular
/// patterns.
///
/// # Authors
/// - eralex61
pub static VORON: VariationDef = VariationDef {
    name: "voron",
    aliases: &[],
    display_name: "Voron",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("k", "K", unlimited_float, 0.99, -10.0, 10.0, "Lerp factor toward the nearest Voronoi site. 1 = snap fully to site; 0 = pass through unchanged."),
        param!("step", "Step", unlimited_float, 0.25, 0.001, 10.0, "Voronoi cell size."),
        param!("num", "Num", int, 1.0, 1.0, 5.0, "Maximum sites per cell (1–5). Actual count per cell is hashed from the cell index."),
        param!("xseed", "X seed", int, 3.0, -1000.0, 1000.0, "Hash seed for X-coordinate site generation."),
        param!("yseed", "Y seed", int, 7.0, -1000.0, 1000.0, "Hash seed for Y-coordinate site generation."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn voron_noise(x_seed: i32) -> f32 {
    var n = x_seed;
    n = (n << 13) ^ n;
    let v = n * (n * n * 15731 + 789221) + 1376312589;
    return f32(v & 0x7fffffff) * 4.6566128752457969e-10;
}

fn variation_voron(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let step = get_param(xform_id, variation_id, 1u);
    let num = max(get_param(xform_id, variation_id, 2u), 1.0);
    let xseed = i32(get_param(xform_id, variation_id, 3u));
    let yseed = i32(get_param(xform_id, variation_id, 4u));
    let safe_step = select(step, 1e-30, abs(step) < 1e-30);

    var rmin: f32 = 20.0;
    var x0: f32 = 0.0;
    var y0: f32 = 0.0;
    let m = i32(floor(p.x / safe_step));
    let n = i32(floor(p.y / safe_step));

    // 3×3 cell scan
    for (var i: i32 = -1; i < 2; i = i + 1) {
        for (var j: i32 = -1; j < 2; j = j + 1) {
            let m1 = m + i;
            let n1 = n + j;
            let kk = i32(1.0 + floor(num * voron_noise(19 * m1 + 257 * n1 + xseed)));
            for (var l: i32 = 0; l < kk; l = l + 1) {
                let xc = (voron_noise(l + 64 * m1 + 15 * n1 + xseed) + f32(m1)) * step;
                let yc = (voron_noise(l + 21 * m1 + 33 * n1 + yseed) + f32(n1)) * step;
                let dx = p.x - xc;
                let dy = p.y - yc;
                let r = sqrt(dx * dx + dy * dy);
                if (r < rmin) {
                    rmin = r;
                    x0 = xc;
                    y0 = yc;
                }
            }
        }
    }
    return vec2<f32>(k * (p.x - x0) + x0, k * (p.y - y0) + y0);
}
"#,
    wgsl_3d: r#"
fn voron_noise(x_seed: i32) -> f32 {
    var n = x_seed;
    n = (n << 13) ^ n;
    let v = n * (n * n * 15731 + 789221) + 1376312589;
    return f32(v & 0x7fffffff) * 4.6566128752457969e-10;
}

fn variation_voron(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let step = get_param(xform_id, variation_id, 1u);
    let num = max(get_param(xform_id, variation_id, 2u), 1.0);
    let xseed = i32(get_param(xform_id, variation_id, 3u));
    let yseed = i32(get_param(xform_id, variation_id, 4u));
    let safe_step = select(step, 1e-30, abs(step) < 1e-30);

    var rmin: f32 = 20.0;
    var x0: f32 = 0.0;
    var y0: f32 = 0.0;
    let m = i32(floor(p.x / safe_step));
    let n = i32(floor(p.y / safe_step));

    for (var i: i32 = -1; i < 2; i = i + 1) {
        for (var j: i32 = -1; j < 2; j = j + 1) {
            let m1 = m + i;
            let n1 = n + j;
            let kk = i32(1.0 + floor(num * voron_noise(19 * m1 + 257 * n1 + xseed)));
            for (var l: i32 = 0; l < kk; l = l + 1) {
                let xc = (voron_noise(l + 64 * m1 + 15 * n1 + xseed) + f32(m1)) * step;
                let yc = (voron_noise(l + 21 * m1 + 33 * n1 + yseed) + f32(n1)) * step;
                let dx = p.x - xc;
                let dy = p.y - yc;
                let r = sqrt(dx * dx + dy * dy);
                if (r < rmin) {
                    rmin = r;
                    x0 = xc;
                    y0 = yc;
                }
            }
        }
    }
    return vec3<f32>(k * (p.x - x0) + x0, k * (p.y - y0) + y0, p.z);
}
"#,
};

// =============================================================================
// squircular: squircular Möbius warp
//   Cpp uses VVAR non-linearly: `r = sqrt(VVAR²·r − 4·u²·v²)` and
//   `r = sqrt(1 + u²/v² − rs/(VVAR·v²)·r)`. Output: FPx += xs · r,
//   FPy += v/u · r. Body lacks VVAR multiplier on the output —
//   needs_transform + divide-out.
// =============================================================================
/// Squircular Möbius warp — maps the input through a "squircle"-style
/// transformation (intermediate between a circle and a square). The
/// variation weight enters non-linearly in the body, so the output shape
/// changes qualitatively with weight rather than just scaling.
pub static SQUIRCULAR: VariationDef = VariationDef {
    name: "squircular",
    aliases: &[],
    display_name: "Squircular",
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
fn variation_squircular(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let u = p.x;
    let v = p.y;
    let safe_v = select(v, 1e-30, abs(v) < 1e-30);
    let safe_u = select(u, 1e-30, abs(u) < 1e-30);
    let r0 = u * u + v * v;
    let rs = sqrt(max(r0, 0.0));
    let xs = select(-1.0, 1.0, u > 0.0);

    let r1 = sqrt(max(w * w * r0 - 4.0 * u * u * v * v, 0.0));
    let inner = 1.0 + u * u / (safe_v * safe_v) - rs / (w * safe_v * safe_v) * r1;
    let r = sqrt(max(inner, 0.0)) * 0.7071067811865476;  // 1/sqrt(2)

    let fx = xs * r;
    let fy = v / safe_u * r;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_squircular(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let u = p.x;
    let v = p.y;
    let safe_v = select(v, 1e-30, abs(v) < 1e-30);
    let safe_u = select(u, 1e-30, abs(u) < 1e-30);
    let r0 = u * u + v * v;
    let rs = sqrt(max(r0, 0.0));
    let xs = select(-1.0, 1.0, u > 0.0);

    let r1 = sqrt(max(w * w * r0 - 4.0 * u * u * v * v, 0.0));
    let inner = 1.0 + u * u / (safe_v * safe_v) - rs / (w * safe_v * safe_v) * r1;
    let r = sqrt(max(inner, 0.0)) * 0.7071067811865476;

    let fx = xs * r;
    let fy = v / safe_u * r;
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// flux: VVAR-shift Möbius (meckie)
//   xpw = x + VVAR;  xmw = x − VVAR
//   avgr = VVAR · (2 + spread) · sqrt(sqrt(y² + xpw²) / sqrt(y² + xmw²))
//   avga = (atan2(y, xmw) − atan2(y, xpw)) · 0.5
//   out = avgr · (cos avga, sin avga)
// (additive use of VVAR in xpw/xmw — needs_transform)
// =============================================================================
/// VVAR-shift Möbius warp — computes `xpw = x + w` and `xmw = x − w` (where
/// `w` is the variation weight), then emits a Möbius-style radial × angular
/// combination: `r = w·(2 + spread)·sqrt(sqrt(y² + xpw²) / sqrt(y² +
/// xmw²))` and `a = (atan2(y, xmw) − atan2(y, xpw)) / 2`. Produces flux-
/// like field-line patterns between two virtual poles at `±w`.
///
/// # Authors
/// - meckie
pub static FLUX: VariationDef = VariationDef {
    name: "flux",
    aliases: &[],
    display_name: "Flux",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("spread", "Spread", unlimited_float, 0.3, -10.0, 10.0, "Output magnitude scale, offset from a base value of 2."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_flux(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let spread = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let xpw = p.x + w;
    let xmw = p.x - w;
    let denom_arg = max(p.y * p.y + xmw * xmw, 1e-30);
    let num_arg = max(p.y * p.y + xpw * xpw, 1e-30);
    let avgr = w * (2.0 + spread) * sqrt(sqrt(num_arg) / sqrt(denom_arg));
    let avga = (atan2(p.y, xmw) - atan2(p.y, xpw)) * 0.5;
    return vec2<f32>(avgr * cos(avga) * inv_w, avgr * sin(avga) * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_flux(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let spread = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let xpw = p.x + w;
    let xmw = p.x - w;
    let denom_arg = max(p.y * p.y + xmw * xmw, 1e-30);
    let num_arg = max(p.y * p.y + xpw * xpw, 1e-30);
    let avgr = w * (2.0 + spread) * sqrt(sqrt(num_arg) / sqrt(denom_arg));
    let avga = (atan2(p.y, xmw) - atan2(p.y, xpw)) * 0.5;
    return vec3<f32>(avgr * cos(avga) * inv_w, avgr * sin(avga) * inv_w, p.z);
}
"#,
};

// =============================================================================
// rays: RNG-driven ray spread (Z+ Jan 2007)
//   ang = VVAR · rand · π
//   r = VVAR / (x² + y² + ε)
//   tanr = VVAR · tan(ang) · r
//   out = (tanr · cos(x), tanr · sin(y))
// (Output is cubic in VVAR — needs_transform divide-out)
// =============================================================================
/// RNG-driven ray spread — picks a random angle `ang = w · rand · π`, then
/// emits `(tan(ang) · r · cos(x), tan(ang) · r · sin(y))` with `r = w / (x²
/// + y²)`. The tangent term creates spiky rays radiating in random angular
/// directions.
///
/// # Authors
/// - Z+
pub static RAYS: VariationDef = VariationDef {
    name: "rays",
    aliases: &[],
    display_name: "Rays",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_rays(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let ang = w * rng_nextf(rng) * pi;
    let r = w / (p.x * p.x + p.y * p.y + 1e-6);
    let tanr = w * tan(ang) * r;
    return vec2<f32>(tanr * cos(p.x) * inv_w, tanr * sin(p.y) * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_rays(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let ang = w * rng_nextf(rng) * pi;
    let r = w / (p.x * p.x + p.y * p.y + 1e-6);
    let tanr = w * tan(ang) * r;
    return vec3<f32>(tanr * cos(p.x) * inv_w, tanr * sin(p.y) * inv_w, p.z);
}
"#,
};

// =============================================================================
// rays1: 1/tan + VVAR · (2/π)² (Raykoid666)
//   t = x² + y²
//   u = 1/tan(sqrt(t)) + VVAR · (2/π)²
//   out = (VVAR · u · t / x, VVAR · u · t / y)
// (additive VVAR inside `u` — needs_transform)
// =============================================================================
/// Cotangent + (2/π)² ray spread — computes `u = cot(sqrt(x²+y²)) + w ·
/// (2/π)²`, then emits `(u·t/x, u·t/y)` where `t = x² + y²`. Produces
/// concentric-ring ray patterns driven by the cotangent's pole structure.
///
/// # Authors
/// - Raykoid666
pub static RAYS1: VariationDef = VariationDef {
    name: "rays1",
    aliases: &[],
    display_name: "Rays 1",
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
fn variation_rays1(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_over_pi_sq = 0.40528473456935106;  // (2/π)²

    let t = p.x * p.x + p.y * p.y;
    let s = sqrt(max(t, 1e-30));
    let tan_s = tan(s);
    let safe_tan = select(tan_s, 1e-30, abs(tan_s) < 1e-30);
    let u = 1.0 / safe_tan + w * two_over_pi_sq;
    let safe_x = select(p.x, 1e-30, abs(p.x) < 1e-30);
    let safe_y = select(p.y, 1e-30, abs(p.y) < 1e-30);
    let fx = w * u * t / safe_x;
    let fy = w * u * t / safe_y;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_rays1(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_over_pi_sq = 0.40528473456935106;

    let t = p.x * p.x + p.y * p.y;
    let s = sqrt(max(t, 1e-30));
    let tan_s = tan(s);
    let safe_tan = select(tan_s, 1e-30, abs(tan_s) < 1e-30);
    let u = 1.0 / safe_tan + w * two_over_pi_sq;
    let safe_x = select(p.x, 1e-30, abs(p.x) < 1e-30);
    let safe_y = select(p.y, 1e-30, abs(p.y) < 1e-30);
    let fx = w * u * t / safe_x;
    let fy = w * u * t / safe_y;
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// loonie2: N-sided loonie (dark-beam)
//   sqrvvar = w²  (read via needs_transform)
//   Init: sina/cosa = sin/cos(2π/sides),
//         sins/coss = sin/cos(-π/2 · star),
//         sinc/cosc = sin/cos(π/2 · circle)
//   Body: rotate input N-1 times computing `r2 = max(r2, x · coss + |y| · sins)`,
//         then mix by `cosc`/`sinc`, square or signed-square based on i.
//         Branch on sign of r2 vs sqrvvar.
// =============================================================================
/// N-sided loonie — generalizes the standard `loonie` warp to N-sided
/// star/circle hybrids. Computes a maximum projection across `sides`
/// rotations, then mixes with a circular term via `circle` and optionally
/// folds with a star pattern via `star`. Inside the squared-weight
/// threshold the input scales outward; outside, it passes through.
///
/// # Authors
/// - DarkBeam
pub static LOONIE2: VariationDef = VariationDef {
    name: "loonie2",
    aliases: &[],
    display_name: "Loonie 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("sides", "Sides", int, 4.0, 1.0, 50.0, "Polygon side count (≥ 1)."),
        param!("star", "Star", unlimited_float, 0.0, -10.0, 10.0, "Star-fold rotation amount (scaled by −π/2 internally)."),
        param!("circle", "Circle", unlimited_float, 0.0, -10.0, 10.0, "Circularity mixing factor: 0 = pure star/polygon shape, 1 = pure circle."),
    ],
    needs_transform: true,
    writes_color: false,
    // 6 derived values at slots 3..9:
    //   3: sina  (sin(2π/sides))
    //   4: cosa  (cos(2π/sides))
    //   5: sins  (sin(-π/2 · star))
    //   6: coss  (cos(-π/2 · star))
    //   7: sinc  (sin(π/2 · circle))
    //   8: cosc  (cos(π/2 · circle))
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_loonie2(user: array<f32, 3>) -> array<f32, 6> {
    let sides = max(user[0], 1.0);
    let star = user[1];
    let circle_p = user[2];
    let two_pi = 6.28318530717959;
    let half_pi = 1.5707963267948966;
    let a = two_pi / sides;
    let as_ = -half_pi * star;
    let ac = half_pi * circle_p;
    var out: array<f32, 6>;
    out[0] = sin(a);
    out[1] = cos(a);
    out[2] = sin(as_);
    out[3] = cos(as_);
    out[4] = sin(ac);
    out[5] = cos(ac);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_loonie2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sides_in = max(get_param(xform_id, variation_id, 0u), 1.0);
    let sina = get_param(xform_id, variation_id, 3u);
    let cosa = get_param(xform_id, variation_id, 4u);
    let sins = get_param(xform_id, variation_id, 5u);
    let coss = get_param(xform_id, variation_id, 6u);
    let sinc = get_param(xform_id, variation_id, 7u);
    let cosc = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let sqrvvar = w * w;

    var xrt = p.x;
    var yrt = p.y;
    var r2 = xrt * coss + abs(yrt) * sins;
    let circle_v = sqrt(p.x * p.x + p.y * p.y);
    let sides_i = i32(sides_in);
    var i: i32 = 0;
    for (var k: i32 = 0; k < sides_i - 1; k = k + 1) {
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
        r2 = max(r2, xrt * coss + abs(yrt) * sins);
        i = k + 1;
    }
    r2 = r2 * cosc + circle_v * sinc;
    if (i > 1) {
        r2 = r2 * r2;
    } else {
        r2 = abs(r2) * r2;
    }

    if (r2 > 0.0 && r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
        return vec2<f32>(scale * p.x, scale * p.y);
    } else if (r2 < 0.0) {
        // 2-faces effect
        let inv_scale = 1.0 / max(sqrt(max(sqrvvar / max(-r2, 1e-30) - 1.0, 1e-30)), 1e-30);
        let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
        // Cpp: r = w / sqrt(...); FPx += r · x.  Outer × w gives w² /
        // sqrt(...) · x — wrong by factor of w. Divide out one w.
        return vec2<f32>(inv_scale * p.x * inv_w, inv_scale * p.y * inv_w);
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_loonie2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sides_in = max(get_param(xform_id, variation_id, 0u), 1.0);
    let sina = get_param(xform_id, variation_id, 3u);
    let cosa = get_param(xform_id, variation_id, 4u);
    let sins = get_param(xform_id, variation_id, 5u);
    let coss = get_param(xform_id, variation_id, 6u);
    let sinc = get_param(xform_id, variation_id, 7u);
    let cosc = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let sqrvvar = w * w;

    var xrt = p.x;
    var yrt = p.y;
    var r2 = xrt * coss + abs(yrt) * sins;
    let circle_v = sqrt(p.x * p.x + p.y * p.y);
    let sides_i = i32(sides_in);
    var i: i32 = 0;
    for (var k: i32 = 0; k < sides_i - 1; k = k + 1) {
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
        r2 = max(r2, xrt * coss + abs(yrt) * sins);
        i = k + 1;
    }
    r2 = r2 * cosc + circle_v * sinc;
    if (i > 1) {
        r2 = r2 * r2;
    } else {
        r2 = abs(r2) * r2;
    }

    if (r2 > 0.0 && r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
        return vec3<f32>(scale * p.x, scale * p.y, p.z);
    } else if (r2 < 0.0) {
        let inv_scale = 1.0 / max(sqrt(max(sqrvvar / max(-r2, 1e-30) - 1.0, 1e-30)), 1e-30);
        let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
        return vec3<f32>(inv_scale * p.x * inv_w, inv_scale * p.y * inv_w, p.z);
    }
    return p;
}
"#,
};

// =============================================================================
// fourth: 4-quadrant compound (guagapunyaimel)
//   sqrvvar = w²  (read via needs_transform)
//   Q4 (x>0, y>0):  spherical: out = w · r · (cos a, sin a) / r²
//   Q1 (x>0, y<0):  loonie: if r² < sqrvvar:  scale + w·sqrt(sqrvvar/r²−1)
//                            else:             pass-through
//   Q3 (x<0, y>0):  susan: shift then rotate-or-radial
//   Q2 (x<0, y<0):  linear pass-through
//
// Mostly clean except the Q3 "susan" branch has `+ x_param`/`− y_param`
// add-on terms without VVAR — divide-out via needs_transform.
// =============================================================================
/// 4-quadrant compound — applies a different variation in each quadrant of
/// the input: `(+,+)` uses spherical, `(+,−)` uses loonie (with squared-
/// weight threshold), `(−,+)` uses lazysusan (shift + spin + twist),
/// `(−,−)` is linear pass-through. Useful for combining four distinct
/// behaviors in a single transform.
///
/// # Authors
/// - guagapunyaimel
pub static FOURTH: VariationDef = VariationDef {
    name: "fourth",
    aliases: &[],
    display_name: "Fourth",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("spin", "Spin", unlimited_float, 3.14159265, -10.0, 10.0, "Lazysusan-quadrant rotation amount, in radians."),
        param!("space", "Space", unlimited_float, 0.0, -10.0, 10.0, "Lazysusan-quadrant radial nudge for the outside-threshold case."),
        param!("twist", "Twist", unlimited_float, 0.0, -10.0, 10.0, "Lazysusan-quadrant additional rotation, proportional to distance from the threshold edge."),
        param!("x", "X", unlimited_float, 0.0, -10.0, 10.0, "Lazysusan-quadrant X center offset."),
        param!("y", "Y", unlimited_float, 0.0, -10.0, 10.0, "Lazysusan-quadrant Y center offset."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_fourth(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let spin = get_param(xform_id, variation_id, 0u);
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_p = get_param(xform_id, variation_id, 3u);
    let y_p = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let sqrvvar = w * w;

    var fx: f32;
    var fy: f32;
    if (p.x > 0.0 && p.y > 0.0) {
        // Q4 spherical
        let r2 = p.x * p.x + p.y * p.y;
        let safe_r2 = max(r2, 1e-30);
        let a = atan2(p.y, p.x);
        let inv_r = 1.0 / sqrt(safe_r2);
        fx = w * inv_r * cos(a);
        fy = w * inv_r * sin(a);
    } else if (p.x > 0.0 && p.y < 0.0) {
        // Q1 loonie
        let r2 = p.x * p.x + p.y * p.y;
        if (r2 < sqrvvar) {
            let r = w * sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
            fx = r * p.x;
            fy = r * p.y;
        } else {
            fx = w * p.x;
            fy = w * p.y;
        }
    } else if (p.x < 0.0 && p.y > 0.0) {
        // Q3 susan
        let xs = p.x - x_p;
        let ys = p.y + y_p;
        let r = sqrt(xs * xs + ys * ys);
        if (r < w) {
            let a = atan2(ys, xs) + spin + twist * (w - r);
            let r2 = w * r;
            // The +x_p / -y_p add-ons lack VVAR — accept the divide-out.
            fx = r2 * cos(a) + x_p;
            fy = r2 * sin(a) - y_p;
        } else {
            let r2 = w * (1.0 + space / max(r, 1e-30));
            fx = r2 * xs + x_p;
            fy = r2 * ys - y_p;
        }
    } else {
        // Q2 linear
        fx = w * p.x;
        fy = w * p.y;
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_fourth(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let spin = get_param(xform_id, variation_id, 0u);
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_p = get_param(xform_id, variation_id, 3u);
    let y_p = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let sqrvvar = w * w;

    var fx: f32;
    var fy: f32;
    if (p.x > 0.0 && p.y > 0.0) {
        let r2 = p.x * p.x + p.y * p.y;
        let safe_r2 = max(r2, 1e-30);
        let a = atan2(p.y, p.x);
        let inv_r = 1.0 / sqrt(safe_r2);
        fx = w * inv_r * cos(a);
        fy = w * inv_r * sin(a);
    } else if (p.x > 0.0 && p.y < 0.0) {
        let r2 = p.x * p.x + p.y * p.y;
        if (r2 < sqrvvar) {
            let r = w * sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
            fx = r * p.x;
            fy = r * p.y;
        } else {
            fx = w * p.x;
            fy = w * p.y;
        }
    } else if (p.x < 0.0 && p.y > 0.0) {
        let xs = p.x - x_p;
        let ys = p.y + y_p;
        let r = sqrt(xs * xs + ys * ys);
        if (r < w) {
            let a = atan2(ys, xs) + spin + twist * (w - r);
            let r2 = w * r;
            fx = r2 * cos(a) + x_p;
            fy = r2 * sin(a) - y_p;
        } else {
            let r2 = w * (1.0 + space / max(r, 1e-30));
            fx = r2 * xs + x_p;
            fy = r2 * ys - y_p;
        }
    } else {
        fx = w * p.x;
        fy = w * p.y;
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};
