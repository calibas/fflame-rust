//! Miscellaneous larger-param variations
//!
//! Six more standalone variations from upstream, larger than the
//! `misc_2d.rs` set (more params or 3D).
//!
//!   - `ho`           (Larry Berlin)        — 3D hyperbolic-octahedron
//!                                            mapping
//!   - `chunk`        (zephyrtronium)        — quadratic-conic mask
//!   - `ptransform`   (Maschke / Apophysis) — log-polar transform
//!   - `rational3`    (Xyrus / CozyG)       — degree-3 complex rational
//!   - `tile_reverse` (Whittaker Courtney)  — random-mirror tile blur
//!   - `ortho`        (Faber)               — orthogonal Möbius warp
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`. All factor VVAR cleanly as outer
//! multiplier, except:
//!   - `chunk` — pass-through `FPx += FTx` lines lack VVAR. Body uses
//!     `needs_transform` + divide-out so outer × w restores the cpp
//!     output. (Also the `r <= 0` comparison's sign depends on the
//!     weight's sign — read inside via the same divide-out body.)

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// ho: 3D hyperbolic octahedron (Larry Berlin)
//   x = (cu·cv)^xpow + xpow · (cu·cv) + 0.25 · atan2(v², w²)
//   y = (su·cv)^ypow + ypow · (su·cv) + 0.25 · atan2(u², w²)
//   z = sv^zpow + zpow · sv
//   where u, v, w are the input p.x, p.y, p.z
// =============================================================================
pub static HO: VariationDef = VariationDef {
    name: "ho",
    display_name: "HO",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("xpow", "X Power", unlimited_float, 3.0, -10.0, 10.0),
        param!("ypow", "Y Power", unlimited_float, 3.0, -10.0, 10.0),
        param!("zpow", "Z Power", unlimited_float, 3.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ho(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xpow = get_param(xform_id, variation_id, 0u);
    let ypow = get_param(xform_id, variation_id, 1u);
    // z = 0 in 2D mode; atan2(v², 0²) is atan2(non-neg, 0) → ±π/2
    let u = p.x;
    let v = p.y;
    let cu = cos(u); let su = sin(u);
    let cv = cos(v); let sv = sin(v);
    let cucv = cu * cv;
    let sucv = su * cv;
    let at_x = atan2(v * v, 0.0);
    let at_y = atan2(u * u, 0.0);
    let x = pow(max(abs(cucv), 1e-30), xpow) * sign_or_one(cucv) + cucv * xpow + 0.25 * at_x;
    let y = pow(max(abs(sucv), 1e-30), ypow) * sign_or_one(sucv) + sucv * ypow + 0.25 * at_y;
    return vec2<f32>(x, y);
}

fn sign_or_one(v: f32) -> f32 {
    return select(sign(v), 1.0, v == 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_ho(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xpow = get_param(xform_id, variation_id, 0u);
    let ypow = get_param(xform_id, variation_id, 1u);
    let zpow = get_param(xform_id, variation_id, 2u);
    let u = p.x; let v = p.y; let w = p.z;
    let cu = cos(u); let su = sin(u);
    let cv = cos(v); let sv = sin(v);
    let cucv = cu * cv;
    let sucv = su * cv;
    let at_x = atan2(v * v, w * w);
    let at_y = atan2(u * u, w * w);
    let x = pow(max(abs(cucv), 1e-30), xpow) * sign_or_one_3d(cucv) + cucv * xpow + 0.25 * at_x;
    let y = pow(max(abs(sucv), 1e-30), ypow) * sign_or_one_3d(sucv) + sucv * ypow + 0.25 * at_y;
    let z = pow(max(abs(sv), 1e-30), zpow) * sign_or_one_3d(sv) + sv * zpow;
    return vec3<f32>(x, y, z);
}

fn sign_or_one_3d(v: f32) -> f32 {
    return select(sign(v), 1.0, v == 0.0);
}
"#),
};

// =============================================================================
// chunk: quadratic-conic mask (zephyrtronium)
//   r = a·x² + b·x·y + c·y² + d·x + e·y + f       (with VVAR baked into
//                                                    each coefficient)
//   mode 0:  if r ≤ 0:  out = (x, y)               (no VVAR multiplier)
//   mode 1:  if r > 0:  out = (x, y)
//
// To match cpp at any weight, body reads w via needs_transform:
//   - the sign of `r` matches `unweighted_r · sign(w)`; we compute
//     `r_un = a·x²+...` and test against 0 with `w >= 0` semantics.
//     For w < 0 the comparison flips.
//   - output is identity-pass `(x, y, z)`; we return `p / w` so outer
//     × w restores `p`.
// =============================================================================
pub static CHUNK: VariationDef = VariationDef {
    name: "chunk",
    display_name: "Chunk",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
        param!("d", "D", unlimited_float, 0.0, -10.0, 10.0),
        param!("e", "E", unlimited_float, 0.0, -10.0, 10.0),
        param!("f", "F", unlimited_float, -1.0, -10.0, 10.0),
        param!("mode", "Mode", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_chunk(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let mode = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let r = w * (a * p.x * p.x + b * p.x * p.y + c * p.y * p.y + d * p.x + e * p.y + f);
    var keep = false;
    if (mode < 0.5) {
        keep = r <= 0.0;
    } else {
        keep = r > 0.0;
    }
    if (keep) {
        return vec2<f32>(p.x * inv_w, p.y * inv_w);
    }
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_chunk(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let mode = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let r = w * (a * p.x * p.x + b * p.x * p.y + c * p.y * p.y + d * p.x + e * p.y + f);
    var keep = false;
    if (mode < 0.5) {
        keep = r <= 0.0;
    } else {
        keep = r > 0.0;
    }
    if (keep) {
        // Z preserve scales with w (FPz += VVAR · FTz); leave p.z so
        // outer × w produces VVAR · p.z.
        return vec3<f32>(p.x * inv_w, p.y * inv_w, p.z);
    }
    return vec3<f32>(0.0, 0.0, 0.0);
}
"#),
};

// =============================================================================
// ptransform: log-polar transform (Maschke / Apophysis pack)
//   r0    = sqrt(x² + y²) + ε
//   ρ     = (use_log ? log(r0) : r0) / power + move
//   θ     = atan2(x, y) + rotate              (upstream cpp swap)
//   if x ≥ 0: ρ += split  else: ρ -= split
//   if use_log: ρ = exp(ρ)
//   out   = (ρ · cos θ, ρ · sin θ)
// =============================================================================
pub static PTRANSFORM: VariationDef = VariationDef {
    name: "ptransform",
    display_name: "P Transform",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("rotate", "Rotate", unlimited_float, 0.0, -10.0, 10.0),
        param!("power", "Power", int, 1.0, -50.0, 50.0),
        param!("move", "Move", unlimited_float, 0.0, -10.0, 10.0),
        param!("split", "Split", unlimited_float, 0.0, -10.0, 10.0),
        param!("use_log", "Use Log", int, 1.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ptransform(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let rotate = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let move_p = get_param(xform_id, variation_id, 2u);
    let split = get_param(xform_id, variation_id, 3u);
    let use_log = get_param(xform_id, variation_id, 4u);

    let r0 = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let safe_power = select(power, 1e-30, power == 0.0);
    var rho = select(r0, log(r0), use_log != 0.0) / safe_power + move_p;
    let theta = atan2(p.x, p.y) + rotate;  // upstream cpp swap
    if (p.x >= 0.0) { rho = rho + split; } else { rho = rho - split; }
    if (use_log != 0.0) { rho = exp(rho); }
    return vec2<f32>(rho * cos(theta), rho * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_ptransform(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let rotate = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let move_p = get_param(xform_id, variation_id, 2u);
    let split = get_param(xform_id, variation_id, 3u);
    let use_log = get_param(xform_id, variation_id, 4u);

    let r0 = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let safe_power = select(power, 1e-30, power == 0.0);
    var rho = select(r0, log(r0), use_log != 0.0) / safe_power + move_p;
    let theta = atan2(p.x, p.y) + rotate;
    if (p.x >= 0.0) { rho = rho + split; } else { rho = rho - split; }
    if (use_log != 0.0) { rho = exp(rho); }
    return vec3<f32>(rho * cos(theta), rho * sin(theta), p.z);
}
"#),
};

// =============================================================================
// rational3: degree-3 complex-rational warp (Xyrus / CozyG)
//   t(z) / b(z) where t and b are degree-3 polynomials in (x, y) using
//   a, b, c, d (numerator) and e, f, g, h (denominator) coefficients.
//   The complex-arithmetic body is ported verbatim.
// =============================================================================
pub static RATIONAL3: VariationDef = VariationDef {
    name: "rational3",
    display_name: "Rational 3",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 0.5, -10.0, 10.0),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 0.0, -10.0, 10.0),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0),
        param!("e", "E", unlimited_float, 0.0, -10.0, 10.0),
        param!("f", "F", unlimited_float, 1.0, -10.0, 10.0),
        param!("g", "G", unlimited_float, 0.0, -10.0, 10.0),
        param!("h", "H", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_rational3(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let g = get_param(xform_id, variation_id, 6u);
    let h = get_param(xform_id, variation_id, 7u);

    let xs = p.x * p.x;
    let ys = p.y * p.y;
    let xc = xs * p.x;
    let yc = ys * p.y;
    let zt3 = xc - 3.0 * p.x * ys;
    let zt2 = xs - ys;
    let zb3 = 3.0 * xs * p.y - yc;
    let zb2 = 2.0 * p.x * p.y;

    let tr = a * zt3 + b * zt2 + c * p.x + d;
    let ti = a * zb3 + b * zb2 + c * p.y;
    let br = e * zt3 + f * zt2 + g * p.x + h;
    let bi = e * zb3 + f * zb2 + g * p.y;
    let r3den = 1.0 / max(br * br + bi * bi, 1e-30);
    return vec2<f32>(
        (tr * br + ti * bi) * r3den,
        (ti * br - tr * bi) * r3den,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_rational3(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let g = get_param(xform_id, variation_id, 6u);
    let h = get_param(xform_id, variation_id, 7u);

    let xs = p.x * p.x;
    let ys = p.y * p.y;
    let xc = xs * p.x;
    let yc = ys * p.y;
    let zt3 = xc - 3.0 * p.x * ys;
    let zt2 = xs - ys;
    let zb3 = 3.0 * xs * p.y - yc;
    let zb2 = 2.0 * p.x * p.y;

    let tr = a * zt3 + b * zt2 + c * p.x + d;
    let ti = a * zb3 + b * zb2 + c * p.y;
    let br = e * zt3 + f * zt2 + g * p.x + h;
    let bi = e * zb3 + f * zb2 + g * p.y;
    let r3den = 1.0 / max(br * br + bi * bi, 1e-30);
    return vec3<f32>(
        (tr * br + ti * bi) * r3den,
        (ti * br - tr * bi) * r3den,
        p.z,
    );
}
"#),
};

// =============================================================================
// tile_reverse: random-mirror tile blur (Whittaker Courtney 2018-12-14)
//   rev = (reversal == 1) ? -1 : 1
//   if vertical:
//     out_y = rev · y ± space    (sign by rng coin-flip)
//     out_x = x
//   else:
//     out_x = rev · x ± space
//     out_y = y
// =============================================================================
pub static TILE_REVERSE: VariationDef = VariationDef {
    name: "tile_reverse",
    display_name: "Tile Reverse",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("space", "Space", unlimited_float, 1.0, -10.0, 10.0),
        param!("reversal", "Reversal", unlimited_float, 1.0, -10.0, 10.0),
        param!("vertical", "Vertical", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_tile_reverse(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let space = get_param(xform_id, variation_id, 0u);
    let reversal = get_param(xform_id, variation_id, 1u);
    let vertical = get_param(xform_id, variation_id, 2u);
    let rev = select(1.0, -1.0, reversal == 1.0);
    let n = rng_nextf(rng);
    let sign_v = select(-1.0, 1.0, n < 0.5);

    if (vertical == 1.0) {
        return vec2<f32>(p.x, rev * p.y + sign_v * space);
    }
    return vec2<f32>(rev * p.x + sign_v * space, p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_tile_reverse(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let space = get_param(xform_id, variation_id, 0u);
    let reversal = get_param(xform_id, variation_id, 1u);
    let vertical = get_param(xform_id, variation_id, 2u);
    let rev = select(1.0, -1.0, reversal == 1.0);
    let n = rng_nextf(rng);
    let sign_v = select(-1.0, 1.0, n < 0.5);

    if (vertical == 1.0) {
        return vec3<f32>(p.x, rev * p.y + sign_v * space, p.z);
    }
    return vec3<f32>(rev * p.x + sign_v * space, p.y, p.z);
}
"#),
};

// =============================================================================
// ortho: orthogonal Möbius warp (Faber)
//   Inside the unit disc (r² < 1):
//     positive-x branch:   xo = (r²+1)/(2x);  ro = dist((x,y), (xo, 0))
//                          a = fmod(in·θ + atan2(y, xo−x) + θ, 2θ) − θ
//                          out = (xo − cos(a)·ro, sin(a)·ro)
//     negative-x branch: same with sign flips (matches cpp code)
//   Outside the unit disc (r² ≥ 1): inversion-then-same-process via
//     `out` parameter applied to the unit-disc projection.
//
// Lots of conditionals; VVAR strictly outer multiplier throughout.
// Body is a fairly direct port of the upstream cpp.
// =============================================================================
pub static ORTHO: VariationDef = VariationDef {
    name: "ortho",
    display_name: "Ortho",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("in_p", "In", unlimited_float, 0.0, -10.0, 10.0),
        param!("out_p", "Out", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ortho(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let in_p = get_param(xform_id, variation_id, 0u);
    let out_p = get_param(xform_id, variation_id, 1u);

    var fpx: f32 = 0.0;
    var fpy: f32 = 0.0;
    let r = p.x * p.x + p.y * p.y;

    if (r < 1.0) {
        if (p.x >= 0.0) {
            let xo = (r + 1.0) / (2.0 * max(p.x, 1e-30));
            let ro = sqrt((p.x - xo) * (p.x - xo) + p.y * p.y);
            let theta = atan2(1.0, ro);
            var a = in_p * theta + atan2(p.y, xo - p.x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            fpx = xo - cos(a) * ro;
            fpy = sin(a) * ro;
        } else {
            let xo = -(r + 1.0) / (2.0 * min(p.x, -1e-30));
            let ro = sqrt((-p.x - xo) * (-p.x - xo) + p.y * p.y);
            let theta = atan2(1.0, ro);
            var a = in_p * theta + atan2(p.y, xo + p.x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            fpx = -(xo - cos(a) * ro);
            fpy = sin(a) * ro;
        }
    } else {
        let r_inv = 1.0 / sqrt(r);
        let ta = atan2(p.y, p.x);
        let x = r_inv * cos(ta);
        let y = r_inv * sin(ta);

        if (x >= 0.0) {
            let xo = (x * x + y * y + 1.0) / (2.0 * max(x, 1e-30));
            let ro = sqrt((x - xo) * (x - xo) + y * y);
            let theta = atan2(1.0, ro);
            var a = out_p * theta + atan2(y, xo - x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            let nx = xo - cos(a) * ro;
            let ny = sin(a) * ro;
            let nr = nx * nx + ny * ny;
            let inv_n = 1.0 / max(nr, 1e-30);
            fpx = nx * inv_n;
            fpy = ny * inv_n;
        } else {
            let xo = -(x * x + y * y + 1.0) / (2.0 * min(x, -1e-30));
            let ro = sqrt((-x - xo) * (-x - xo) + y * y);
            let theta = atan2(1.0, ro);
            var a = out_p * theta + atan2(y, xo + x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            let nx = -(xo - cos(a) * ro);
            let ny = sin(a) * ro;
            let nr = nx * nx + ny * ny;
            let inv_n = 1.0 / max(nr, 1e-30);
            fpx = nx * inv_n;
            fpy = ny * inv_n;
        }
    }
    return vec2<f32>(fpx, fpy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_ortho(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let in_p = get_param(xform_id, variation_id, 0u);
    let out_p = get_param(xform_id, variation_id, 1u);

    var fpx: f32 = 0.0;
    var fpy: f32 = 0.0;
    let r = p.x * p.x + p.y * p.y;

    if (r < 1.0) {
        if (p.x >= 0.0) {
            let xo = (r + 1.0) / (2.0 * max(p.x, 1e-30));
            let ro = sqrt((p.x - xo) * (p.x - xo) + p.y * p.y);
            let theta = atan2(1.0, ro);
            var a = in_p * theta + atan2(p.y, xo - p.x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            fpx = xo - cos(a) * ro;
            fpy = sin(a) * ro;
        } else {
            let xo = -(r + 1.0) / (2.0 * min(p.x, -1e-30));
            let ro = sqrt((-p.x - xo) * (-p.x - xo) + p.y * p.y);
            let theta = atan2(1.0, ro);
            var a = in_p * theta + atan2(p.y, xo + p.x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            fpx = -(xo - cos(a) * ro);
            fpy = sin(a) * ro;
        }
    } else {
        let r_inv = 1.0 / sqrt(r);
        let ta = atan2(p.y, p.x);
        let x = r_inv * cos(ta);
        let y = r_inv * sin(ta);

        if (x >= 0.0) {
            let xo = (x * x + y * y + 1.0) / (2.0 * max(x, 1e-30));
            let ro = sqrt((x - xo) * (x - xo) + y * y);
            let theta = atan2(1.0, ro);
            var a = out_p * theta + atan2(y, xo - x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            let nx = xo - cos(a) * ro;
            let ny = sin(a) * ro;
            let nr = nx * nx + ny * ny;
            let inv_n = 1.0 / max(nr, 1e-30);
            fpx = nx * inv_n;
            fpy = ny * inv_n;
        } else {
            let xo = -(x * x + y * y + 1.0) / (2.0 * min(x, -1e-30));
            let ro = sqrt((-x - xo) * (-x - xo) + y * y);
            let theta = atan2(1.0, ro);
            var a = out_p * theta + atan2(y, xo + x) + theta;
            a = a - floor(a / (2.0 * theta)) * (2.0 * theta) - theta;
            let nx = -(xo - cos(a) * ro);
            let ny = sin(a) * ro;
            let nr = nx * nx + ny * ny;
            let inv_n = 1.0 / max(nr, 1e-30);
            fpx = nx * inv_n;
            fpy = ny * inv_n;
        }
    }
    return vec3<f32>(fpx, fpy, p.z);
}
"#),
};
