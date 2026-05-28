//! Standalone shape variations (continued)
//!
//! - `butterfly`, `butterfly3d` — butterfly curve
//! - `ennepers`, `ennepers2` — Enneper's surface mappings
//! - `pyramid` — Zueuk's 3D pyramid (cubic distance norm)
//! - `rays2`, `rays3` — Raykoid666's tan/cos ray patterns (rays/rays1
//!   skipped: internal-weight, see watchlist)
//! - `spiralwing` — Raykoid666's spiral wing
//! - `whitney_umbrella` — Don Town's Whitney umbrella surface
//! - `chrysanthemum` — Sosa's chrysanthemum curve (RNG-driven)
//! - `cell` — Apophysis cell (1 param)
//! - `flower` — cyberxaos's flower (2 params, RNG)
//!
//! Notes:
//!   - `ennepers` upstream uses `FPx = ...` (assignment) instead of `+=`,
//!     and only multiplies the first term by `pAmount`. Treating both as
//!     porter typos here — accumulating both terms with the outer weight
//!     gives the more sensible mapping `(x(1 − x²/3 + y²), y(1 − y²/3 + x²))`.
//!   - `rays`, `rays1`, `flux`, `target`, `yin_yang` were skipped from this
//!     batch — `rays`/`rays1`/`flux` use `VVAR` non-linearly inside the
//!     formula (internal-weight watchlist material), and `target`/`yin_yang`
//!     need init-time precomputed fields recovered from the Java source.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// =============================================================================
// butterfly: x' = K · sqrt(|xy|/(x²+(2y)²+ε)) · x;  y' = … · 2y
// =============================================================================
/// Butterfly-shaped curve produced by a normalized cross-coordinate
/// stretch. Output sketches the classic butterfly silhouette around the
/// origin.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static BUTTERFLY: VariationDef = VariationDef {
    name: "butterfly",
    aliases: &[],
    display_name: "Butterfly",
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
fn variation_butterfly(p: vec2<f32>) -> vec2<f32> {
    let k = 1.302940031741119;
    let y2 = p.y * 2.0;
    let r = k * sqrt(abs(p.y * p.x) / (1e-30 + p.x * p.x + y2 * y2));
    return vec2<f32>(r * p.x, r * y2);
}
"#,
    wgsl_3d: Some(r#"
fn variation_butterfly(p: vec3<f32>) -> vec3<f32> {
    let k = 1.302940031741119;
    let y2 = p.y * 2.0;
    let r = k * sqrt(abs(p.y * p.x) / (1e-30 + p.x * p.x + y2 * y2));
    return vec3<f32>(r * p.x, r * y2, p.z);
}
"#),
};

// =============================================================================
// butterfly3d: butterfly + z' = r · |2y| · sqrt(x² + y²) / 4
// =============================================================================
/// 3D version of Butterfly — same XY butterfly curve plus a Z component
/// that scales with the radial distance times `|2y|`.
pub static BUTTERFLY3D: VariationDef = VariationDef {
    name: "butterfly3d",
    aliases: &[],
    display_name: "Butterfly 3D",
    category: VariationCategory::Full3D,
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
fn variation_butterfly3d(p: vec2<f32>) -> vec2<f32> {
    let k = 1.302940031741119;
    let y2 = p.y * 2.0;
    let r = k * sqrt(abs(p.y * p.x) / (1e-30 + p.x * p.x + y2 * y2));
    return vec2<f32>(r * p.x, r * y2);
}
"#,
    wgsl_3d: Some(r#"
fn variation_butterfly3d(p: vec3<f32>) -> vec3<f32> {
    let k = 1.302940031741119;
    let y2 = p.y * 2.0;
    let r = k * sqrt(abs(p.y * p.x) / (1e-30 + p.x * p.x + y2 * y2));
    let z_out = r * abs(y2) * sqrt(p.x * p.x + p.y * p.y) / 4.0;
    return vec3<f32>(r * p.x, r * y2, z_out);
}
"#),
};

// =============================================================================
// ennepers: Enneper's-surface mapping
//   x' = x · (1 − x²/3 + y²)
//   y' = y · (1 − y²/3 + x²)
// (See file comment about upstream's broken `=` form.)
// =============================================================================
/// Enneper's-surface parametric mapping — `(x(1 − x²/3 + y²), y(1 − y²/3 +
/// x²))`. Inspired by Alfred Enneper's classical minimal surface.
///
/// # Authors
/// - Raykoid666
pub static ENNEPERS: VariationDef = VariationDef {
    name: "ennepers",
    aliases: &[],
    display_name: "Ennepers",
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
fn variation_ennepers(p: vec2<f32>) -> vec2<f32> {
    let x2 = p.x * p.x;
    let y2 = p.y * p.y;
    return vec2<f32>(p.x * (1.0 - x2 / 3.0 + y2), p.y * (1.0 - y2 / 3.0 + x2));
}
"#,
    wgsl_3d: Some(r#"
fn variation_ennepers(p: vec3<f32>) -> vec3<f32> {
    let x2 = p.x * p.x;
    let y2 = p.y * p.y;
    return vec3<f32>(p.x * (1.0 - x2 / 3.0 + y2), p.y * (1.0 - y2 / 3.0 + x2), p.z);
}
"#),
};

// =============================================================================
// pyramid: Zueuk's cubic-distance pyramid
//   x' = x³ / (|x³| + |y³| + |z³| + ε)
// (z component takes |z³| in upstream.)
// =============================================================================
/// 3D pyramid using cubic-distance norm — each coordinate is cubed and
/// divided by the sum of absolute cubes. Produces a pyramid-shaped
/// silhouette.
///
/// # Authors
/// - Zueuk
pub static PYRAMID: VariationDef = VariationDef {
    name: "pyramid",
    aliases: &[],
    display_name: "Pyramid",
    category: VariationCategory::Full3D,
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
fn variation_pyramid(p: vec2<f32>) -> vec2<f32> {
    let x3 = p.x * p.x * p.x;
    let y3 = p.y * p.y * p.y;
    let r = 1.0 / (abs(x3) + abs(y3) + 1e-9);
    return vec2<f32>(x3 * r, y3 * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_pyramid(p: vec3<f32>) -> vec3<f32> {
    let x3 = p.x * p.x * p.x;
    let y3 = p.y * p.y * p.y;
    let z3 = abs(p.z * p.z * p.z);
    let r = 1.0 / (abs(x3) + abs(y3) + z3 + 1e-9);
    return vec3<f32>(x3 * r, y3 * r, z3 * r);
}
"#),
};

// =============================================================================
// rays2: Raykoid666's rays #2 (rays/rays1 skipped — internal weight)
//   t = x²+y², u = 1/cos((t+ε)·tan(1/t + ε))
//   x' = u·t/(10·x), y' = u·t/(10·y)
// =============================================================================
/// Cosine-of-tangent rays — uses `1/cos((t+ε)·tan(1/t+ε))` on the squared
/// radius `t`. Creates intricate ray patterns radiating from the origin.
///
/// # Authors
/// - Raykoid666
pub static RAYS2: VariationDef = VariationDef {
    name: "rays2",
    aliases: &[],
    display_name: "Rays2",
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
fn variation_rays2(p: vec2<f32>) -> vec2<f32> {
    let eps = 1e-30;
    let t = p.x * p.x + p.y * p.y + eps;
    let u = 1.0 / cos((t + eps) * tan(1.0 / t + eps));
    let scale = u * t / 10.0;
    return vec2<f32>(scale / select(p.x, eps, p.x == 0.0), scale / select(p.y, eps, p.y == 0.0));
}
"#,
    wgsl_3d: Some(r#"
fn variation_rays2(p: vec3<f32>) -> vec3<f32> {
    let eps = 1e-30;
    let t = p.x * p.x + p.y * p.y + eps;
    let u = 1.0 / cos((t + eps) * tan(1.0 / t + eps));
    let scale = u * t / 10.0;
    return vec3<f32>(scale / select(p.x, eps, p.x == 0.0), scale / select(p.y, eps, p.y == 0.0), p.z);
}
"#),
};

// =============================================================================
// rays3: Raykoid666's rays #3
//   t = x²+y², u = 1/sqrt(cos(sin(t²+ε) · sin(1/t² + ε)))
//   x' = u·cos(t)·t/(10·x), y' = u·tan(t)·t/(10·y)
// =============================================================================
/// Variant of Rays2 with `sqrt(cos(sin(...)·sin(...)))` and tangent on Y.
/// Denser ray pattern with sharper structure.
///
/// # Authors
/// - Raykoid666
pub static RAYS3: VariationDef = VariationDef {
    name: "rays3",
    aliases: &[],
    display_name: "Rays3",
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
fn variation_rays3(p: vec2<f32>) -> vec2<f32> {
    let eps = 1e-30;
    let t = p.x * p.x + p.y * p.y + eps;
    let t2 = t * t;
    let u = 1.0 / sqrt(max(cos(sin(t2 + eps) * sin(1.0 / t2 + eps)), 1e-20));
    return vec2<f32>(
        (u * cos(t) * t / 10.0) / select(p.x, eps, p.x == 0.0),
        (u * tan(t) * t / 10.0) / select(p.y, eps, p.y == 0.0),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_rays3(p: vec3<f32>) -> vec3<f32> {
    let eps = 1e-30;
    let t = p.x * p.x + p.y * p.y + eps;
    let t2 = t * t;
    let u = 1.0 / sqrt(max(cos(sin(t2 + eps) * sin(1.0 / t2 + eps)), 1e-20));
    return vec3<f32>(
        (u * cos(t) * t / 10.0) / select(p.x, eps, p.x == 0.0),
        (u * tan(t) * t / 10.0) / select(p.y, eps, p.y == 0.0),
        p.z,
    );
}
"#),
};

// =============================================================================
// spiralwing: Raykoid666
//   c1 = x², c2 = y², d = 1/(c1+c2+ε), c2 → sin(c2)
//   x' = d · cos(c1) · sin(c2)
//   y' = d · sin(c1) · sin(c2)
// =============================================================================
/// Spiral wing — uses cos/sin of `x²` with `sin(y²)` modulation. Produces
/// wing-shaped spiral patterns.
///
/// # Authors
/// - Raykoid666
pub static SPIRALWING: VariationDef = VariationDef {
    name: "spiralwing",
    aliases: &[],
    display_name: "Spiral Wing",
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
fn variation_spiralwing(p: vec2<f32>) -> vec2<f32> {
    let c1 = p.x * p.x;
    let c2 = p.y * p.y;
    let d = 1.0 / (c1 + c2 + 1e-30);
    let s2 = sin(c2);
    return vec2<f32>(d * cos(c1) * s2, d * sin(c1) * s2);
}
"#,
    wgsl_3d: Some(r#"
fn variation_spiralwing(p: vec3<f32>) -> vec3<f32> {
    let c1 = p.x * p.x;
    let c2 = p.y * p.y;
    let d = 1.0 / (c1 + c2 + 1e-30);
    let s2 = sin(c2);
    return vec3<f32>(d * cos(c1) * s2, d * sin(c1) * s2, p.z);
}
"#),
};

// =============================================================================
// whitney_umbrella: parametric Whitney umbrella surface
//   x' = u · v        (u = x, v = y)
//   y' = u
//   z' = v²
// =============================================================================
/// Parametric Whitney umbrella surface — output is `(xy, x, y²)`. The
/// classical algebraic surface with the same name.
///
/// # Authors
/// - Don Town
pub static WHITNEY_UMBRELLA: VariationDef = VariationDef {
    name: "whitney_umbrella",
    aliases: &[],
    display_name: "Whitney Umbrella",
    category: VariationCategory::Full3D,
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
fn variation_whitney_umbrella(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x * p.y, p.x);
}
"#,
    wgsl_3d: Some(r#"
fn variation_whitney_umbrella(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(p.x * p.y, p.x, p.y * p.y);
}
"#),
};

// =============================================================================
// chrysanthemum: Sosa's chrysanthemum curve
//   u = 21π · uniform()
//   r = 5(1 + sin(11u/5)) − 4·sin(17u/3)⁴ · sin(2cos(3u) − 28u)⁸
//   x' = r·cos(u), y' = r·sin(u)
// =============================================================================
/// Chrysanthemum curve — Sosa's flower-like parametric curve. Plots `r` as
/// a function of a random angle, producing dense overlapping petal
/// patterns.
///
/// # Authors
/// - Jesus Sosa
pub static CHRYSANTHEMUM: VariationDef = VariationDef {
    name: "chrysanthemum",
    aliases: &[],
    display_name: "Chrysanthemum",
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
fn variation_chrysanthemum(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let u = 21.0 * pi * rng_nextf(rng);
    let p4_b = sin(17.0 * u / 3.0);
    let p4 = p4_b * p4_b * p4_b * p4_b;
    let p8_b = sin(2.0 * cos(3.0 * u) - 28.0 * u);
    let p8_2 = p8_b * p8_b;
    let p8_4 = p8_2 * p8_2;
    let p8 = p8_4 * p8_4;
    let r = 5.0 * (1.0 + sin(11.0 * u / 5.0)) - 4.0 * p4 * p8;
    return vec2<f32>(r * cos(u), r * sin(u));
}
"#,
    wgsl_3d: Some(r#"
fn variation_chrysanthemum(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let u = 21.0 * pi * rng_nextf(rng);
    let p4_b = sin(17.0 * u / 3.0);
    let p4 = p4_b * p4_b * p4_b * p4_b;
    let p8_b = sin(2.0 * cos(3.0 * u) - 28.0 * u);
    let p8_2 = p8_b * p8_b;
    let p8_4 = p8_2 * p8_2;
    let p8 = p8_4 * p8_4;
    let r = 5.0 * (1.0 + sin(11.0 * u / 5.0)) - 4.0 * p4 * p8;
    return vec3<f32>(r * cos(u), r * sin(u), p.z);
}
"#),
};

// =============================================================================
// cell: Apophysis cell (interleaved cells of size `size`)
// =============================================================================
/// Cellular tiling — divides the plane into cells of the given size and
/// rearranges them in an interleaved pattern. Produces a checkered,
/// displaced look.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static CELL: VariationDef = VariationDef {
    name: "cell",
    aliases: &[],
    display_name: "Cell",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef { name: "size", display_name: "Size", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.6, min_value: Some(0.01), max_value: Some(10.0), description: Some("Width of each cell in the grid.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cell(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let size = max(get_param(xform_id, variation_id, 0u), 1e-6);
    let inv_cell_size = 1.0 / size;
    let x_i = floor(p.x * inv_cell_size);
    let y_i = floor(p.y * inv_cell_size);
    let dx = p.x - x_i * size;
    let dy = p.y - y_i * size;
    var ix = x_i;
    var iy = y_i;
    if (y_i >= 0.0) {
        if (x_i >= 0.0) { iy = y_i * 2.0; ix = x_i * 2.0; }
        else            { iy = y_i * 2.0; ix = -(2.0 * x_i + 1.0); }
    } else {
        if (x_i >= 0.0) { iy = -(2.0 * y_i + 1.0); ix = x_i * 2.0; }
        else            { iy = -(2.0 * y_i + 1.0); ix = -(2.0 * x_i + 1.0); }
    }
    return vec2<f32>(dx + ix * size, -(dy + iy * size));
}
"#,
    wgsl_3d: Some(r#"
fn variation_cell(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let size = max(get_param(xform_id, variation_id, 0u), 1e-6);
    let inv_cell_size = 1.0 / size;
    let x_i = floor(p.x * inv_cell_size);
    let y_i = floor(p.y * inv_cell_size);
    let dx = p.x - x_i * size;
    let dy = p.y - y_i * size;
    var ix = x_i;
    var iy = y_i;
    if (y_i >= 0.0) {
        if (x_i >= 0.0) { iy = y_i * 2.0; ix = x_i * 2.0; }
        else            { iy = y_i * 2.0; ix = -(2.0 * x_i + 1.0); }
    } else {
        if (x_i >= 0.0) { iy = -(2.0 * y_i + 1.0); ix = x_i * 2.0; }
        else            { iy = -(2.0 * y_i + 1.0); ix = -(2.0 * x_i + 1.0); }
    }
    return vec3<f32>(dx + ix * size, -(dy + iy * size), p.z);
}
"#),
};

// =============================================================================
// ennepers2: dark-beam's parameterized Enneper variant (3 params, 3D)
//   r2 = 1 / (x²+y²)
//   dxy = (a·x)² − (b·y)²
//   x' = x · (a² − dxy·r2 − c·sqrt(|x|))
//   y' = y · (b² − dxy·r2 − c·sqrt(|y|))
//   z' = dxy · 0.5 · sqrt(r2)
// =============================================================================
/// Parameterized Enneper variant — 3-parameter 3D extension of Ennepers
/// with separate `a`/`b`/`c` controls.
///
/// # Authors
/// - DarkBeam
pub static ENNEPERS2: VariationDef = VariationDef {
    name: "ennepers2",
    aliases: &[],
    display_name: "Ennepers2",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef { name: "a", display_name: "A", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Coefficient on the X factor.") },
        VariationParamDef { name: "b", display_name: "B", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.3333, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Coefficient on the Y factor.") },
        VariationParamDef { name: "c", display_name: "C", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.075, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Square-root correction strength applied to both X and Y outputs.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ennepers2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let r2 = 1.0 / max(p.x * p.x + p.y * p.y, 1e-30);
    let ax = a * p.x; let by = b * p.y;
    let dxy = ax * ax - by * by;
    return vec2<f32>(
        p.x * (a * a - dxy * r2 - c * sqrt(abs(p.x))),
        p.y * (b * b - dxy * r2 - c * sqrt(abs(p.y))),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_ennepers2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let r2 = 1.0 / max(p.x * p.x + p.y * p.y, 1e-30);
    let ax = a * p.x; let by = b * p.y;
    let dxy = ax * ax - by * by;
    return vec3<f32>(
        p.x * (a * a - dxy * r2 - c * sqrt(abs(p.x))),
        p.y * (b * b - dxy * r2 - c * sqrt(abs(p.y))),
        dxy * 0.5 * sqrt(r2),
    );
}
"#),
};

// =============================================================================
// flower: cyberxaos's flower
//   theta = atan2(x, y)            (NOTE: argument order matches upstream)
//   d = sqrt(x²+y²) + ε
//   r = (uniform − holes) · cos(petals · theta) / d
//   x' = r · x, y' = r · y
// =============================================================================
/// Flower — produces petal patterns based on a uniform-random distance
/// scaled by `cos(petals · angle)`. The `holes` parameter controls how
/// hollow the center is.
///
/// # Authors
/// - cyberxaos
pub static FLOWER: VariationDef = VariationDef {
    name: "flower",
    aliases: &[],
    display_name: "Flower",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "holes", display_name: "Holes", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.4, min_value: Some(-10.0), max_value: Some(10.0), description: Some("How hollow the center of the flower is. Higher = bigger center hole.") },
        VariationParamDef { name: "petals", display_name: "Petals", param_type: ParamType::UnlimitedFloat,
                            default_value: 7.0, min_value: Some(1.0), max_value: Some(64.0), description: Some("Number of petals around the flower.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_flower(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let holes = get_param(xform_id, variation_id, 0u);
    let petals = get_param(xform_id, variation_id, 1u);
    let theta = atan2(p.x, p.y);
    let d = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let r = (rng_nextf(rng) - holes) * cos(petals * theta) / d;
    return vec2<f32>(r * p.x, r * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_flower(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let holes = get_param(xform_id, variation_id, 0u);
    let petals = get_param(xform_id, variation_id, 1u);
    let theta = atan2(p.x, p.y);
    let d = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let r = (rng_nextf(rng) - holes) * cos(petals * theta) / d;
    return vec3<f32>(r * p.x, r * p.y, p.z);
}
"#),
};
