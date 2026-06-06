//! Numbered/3D variants of existing variations
//!
//! Twelve ports — pure 3D primitives, then parameterized variants:
//!   - spherical3d, sinusoidal3d, square, square3d, disc3d
//!   - bubble2, popcorn2, splits3d, waves2_3d
//!   - juliaq, julia3dq, juliac
//!
//! Skipped from this batch (deferred to a later "heavy-init" batch):
//!   - cpow2, cpow3 — multiple init-time precomputed fields
//!   - disc2 — multi-stage init logic with conditional adjustments
//!
//! Skipped to watchlists:
//!   - popcorn (needs XForm.coeff access — new affine-access watchlist)
//!   - loonie_3d (internal weight `sqrvvar = VVAR²` — internal-weight watchlist)
//!   - bipolar2, julian2, julian3dx, waves2b — bigger param sets, separate batch

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// =============================================================================
// spherical3d: 3D spherical inversion
//   T = x²+y²+z²+ε,  r = 1/T
//   (x', y', z') = r · (x, y, z)
// =============================================================================
/// 3D version of Spherical — inverts each point through the unit sphere
/// (`1/r²`). Pulls distant points toward the origin and pushes nearby
/// points outward in all three axes.
pub static SPHERICAL3D: VariationDef = VariationDef {
    name: "spherical3D",
    aliases: &[],
    display_name: "Spherical 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_spherical3D(p: vec2<f32>) -> vec2<f32> {
    let t = max(p.x * p.x + p.y * p.y, 1e-30);
    let r = 1.0 / t;
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: r#"
fn variation_spherical3D(p: vec3<f32>) -> vec3<f32> {
    let t = max(p.x * p.x + p.y * p.y + p.z * p.z, 1e-30);
    let r = 1.0 / t;
    return vec3<f32>(p.x * r, p.y * r, p.z * r);
}
"#,
};

// =============================================================================
// sinusoidal3d: gossamer_light's 3D sinusoidal
//   x' = sin(x), y' = sin(y), z' = atan2(x², y²) · cos(z)
// =============================================================================
/// 3D sinusoidal — applies `sin` to X and Y like Sinusoidal, then adds
/// `atan2(x², y²) · cos(z)` on the Z axis.
///
/// # Authors
/// - gossamer light
pub static SINUSOIDAL3D: VariationDef = VariationDef {
    name: "sinusoidal3d",
    aliases: &[],
    display_name: "Sinusoidal 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sinusoidal3d(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(p.x), sin(p.y));
}
"#,
    wgsl_3d: r#"
fn variation_sinusoidal3d(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(sin(p.x), sin(p.y), atan2(p.x * p.x, p.y * p.y) * cos(p.z));
}
"#,
};

// =============================================================================
// square: random 2D unit-square sampler
//   (x', y') = (uniform - 0.5, uniform - 0.5)
// =============================================================================
/// Random 2D unit-square sampler — replaces the input with a uniformly
/// random point in `[-0.5, 0.5]²`.
pub static SQUARE: VariationDef = VariationDef {
    name: "square",
    aliases: &[],
    display_name: "Square",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_square(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    return vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5);
}
"#,
    wgsl_3d: r#"
fn variation_square(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    return vec3<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5, p.z);
}
"#,
};

// =============================================================================
// square3d: 3D version — Z gets its own random offset
// =============================================================================
/// 3D unit-cube version of Square — random point in `[-0.5, 0.5]³`.
pub static SQUARE3D: VariationDef = VariationDef {
    name: "square3D",
    aliases: &[],
    display_name: "Square 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_square3D(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    return vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5);
}
"#,
    wgsl_3d: r#"
fn variation_square3D(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    return vec3<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5);
}
"#,
};

// =============================================================================
// disc3d: 3D disc inversion (note: takes a `pi` parameter — defaults to π but
// upstream exposes it for tweaking).
// =============================================================================
/// 3D version of Disc with a tweakable `pi` constant. Wraps the (x, y)
/// plane onto a disc and adds a `r·cos(z)` Z component.
///
/// # Authors
/// - Larry Berlin
pub static DISC3D: VariationDef = VariationDef {
    name: "disc3d",
    aliases: &[],
    display_name: "Disc 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "pi", display_name: "Pi", param_type: ParamType::UnlimitedFloat,
                            default_value: 3.14159265358979, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Phase constant — defaults to π. Tweaking it warps the disc shape.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_disc3d(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let pi_p = get_param(xform_id, variation_id, 0u);
    let r = sqrt(p.x * p.x + p.y * p.y + 1e-30);
    let vv = atan2(p.x, p.y) / (pi_p + 1e-30);
    return vec2<f32>(vv * sin(pi_p * r), vv * cos(pi_p * r));
}
"#,
    wgsl_3d: r#"
fn variation_disc3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let pi_p = get_param(xform_id, variation_id, 0u);
    let r = sqrt(p.x * p.x + p.y * p.y + 1e-30);
    let vv = atan2(p.x, p.y) / (pi_p + 1e-30);
    return vec3<f32>(vv * sin(pi_p * r), vv * cos(pi_p * r), vv * (r * cos(p.z)));
}
"#,
};

// =============================================================================
// bubble2: FracFx's parameterized 3D bubble (ratio scaling per axis)
//   T = (x²+y²+z²)/4 + 1
//   r = 1/T
//   x' = x · r · param_x
//   y' = y · r · param_y
//   z' = (z ± param_z) + z · r · param_z   (sign on param_z chosen by sign(z))
// =============================================================================
/// Parameterized 3D bubble with separate X / Y / Z scaling. Maps the input
/// onto a sphere and stretches each axis independently.
///
/// # Authors
/// - FracFx
pub static BUBBLE2: VariationDef = VariationDef {
    name: "bubble2",
    aliases: &[],
    display_name: "Bubble2",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x", display_name: "X", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("X-axis scaling of the sphere projection.") },
        VariationParamDef { name: "y", display_name: "Y", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Y-axis scaling.") },
        VariationParamDef { name: "z", display_name: "Z", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Z-axis displacement plus scaling.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bubble2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let t = (p.x * p.x + p.y * p.y) / 4.0 + 1.0;
    let r = 1.0 / t;
    return vec2<f32>(p.x * r * x_p, p.y * r * y_p);
}
"#,
    wgsl_3d: r#"
fn variation_bubble2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let z_p = get_param(xform_id, variation_id, 2u);
    let t = (p.x * p.x + p.y * p.y + p.z * p.z) / 4.0 + 1.0;
    let r = 1.0 / t;
    let z_sign = select(-1.0, 1.0, p.z >= 0.0);
    let z_out = (p.z + z_sign * z_p) + p.z * r * z_p;
    return vec3<f32>(p.x * r * x_p, p.y * r * y_p, z_out);
}
"#,
};

// =============================================================================
// popcorn2: parameterized popcorn (no XForm.coeff dependency, unlike `popcorn`)
//   x' = x + param_x · sin(tan(y · param_c))
//   y' = y + param_y · sin(tan(x · param_c))
// =============================================================================
/// Parameterized version of Popcorn — adds `param · sin(tan(coord · c))`
/// to each axis. Unlike the original Popcorn (which reads parameters from
/// the affine matrix), this one has dedicated `x` / `y` / `c` sliders.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static POPCORN2: VariationDef = VariationDef {
    name: "popcorn2",
    aliases: &[],
    display_name: "Popcorn2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x", display_name: "X", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("X-axis displacement strength.") },
        VariationParamDef { name: "y", display_name: "Y", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.5, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Y-axis displacement strength.") },
        VariationParamDef { name: "c", display_name: "C", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.5, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Frequency of the tan-sine wave that drives the displacement on both axes.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_popcorn2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    return vec2<f32>(p.x + x_p * sin(tan(p.y * c)), p.y + y_p * sin(tan(p.x * c)));
}
"#,
    wgsl_3d: r#"
fn variation_popcorn2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    return vec3<f32>(p.x + x_p * sin(tan(p.y * c)), p.y + y_p * sin(tan(p.x * c)), p.z);
}
"#,
};

// =============================================================================
// splits3d: TyrantWave's 3D splits — pushes each coordinate away from zero
// by a fixed offset per axis.
// =============================================================================
/// 3D version of Splits — pushes each coordinate away from zero by a fixed
/// per-axis offset, creating a gap along each axis.
///
/// # Authors
/// - TyrantWave
pub static SPLITS3D: VariationDef = VariationDef {
    name: "splits3D",
    aliases: &[],
    display_name: "Splits 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x", display_name: "X", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.1, min_value: Some(-10.0), max_value: Some(10.0), description: Some("How far positive-X and negative-X points get pushed apart along X.") },
        VariationParamDef { name: "y", display_name: "Y", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.3, min_value: Some(-10.0), max_value: Some(10.0), description: Some("How far positive-Y and negative-Y points get pushed apart along Y.") },
        VariationParamDef { name: "z", display_name: "Z", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.2, min_value: Some(-10.0), max_value: Some(10.0), description: Some("How far positive-Z and negative-Z points get pushed apart along Z.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_splits3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let xs = select(-x_p, x_p, p.x >= 0.0);
    let ys = select(-y_p, y_p, p.y >= 0.0);
    return vec2<f32>(p.x + xs, p.y + ys);
}
"#,
    wgsl_3d: r#"
fn variation_splits3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let z_p = get_param(xform_id, variation_id, 2u);
    let xs = select(-x_p, x_p, p.x >= 0.0);
    let ys = select(-y_p, y_p, p.y >= 0.0);
    let zs = select(-z_p, z_p, p.z >= 0.0);
    return vec3<f32>(p.x + xs, p.y + ys, p.z + zs);
}
"#,
};

// =============================================================================
// waves2_3d: 3D waves2 with z' driven by avg(x, y)
//   avgxy = (x + y) / 2  (computed per-iteration in upstream's "Prepare")
//   x' = x + scale · sin(y · freq)
//   y' = y + scale · sin(x · freq)
//   z' = z + scale · sin(avgxy · freq)
// =============================================================================
/// 3D version of Waves2 — adds `scale · sin(freq · avg(x, y))` to the Z
/// coordinate alongside the standard 2D Waves2 X/Y displacement.
/// 
/// # Authors
/// - Larry Berlin
pub static WAVES2_3D: VariationDef = VariationDef {
    name: "waves2_3D",
    aliases: &[],
    display_name: "Waves2 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "freq", display_name: "Freq", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Wave frequency on all three axes.") },
        VariationParamDef { name: "scale", display_name: "Scale", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Wave amplitude — how strongly points get displaced.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waves2_3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let freq = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(p.x + scale * sin(p.y * freq), p.y + scale * sin(p.x * freq));
}
"#,
    wgsl_3d: r#"
fn variation_waves2_3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let freq = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);
    let avgxy = (p.x + p.y) / 2.0;
    return vec3<f32>(
        p.x + scale * sin(p.y * freq),
        p.y + scale * sin(p.x * freq),
        p.z + scale * sin(avgxy * freq),
    );
}
"#,
};

// =============================================================================
// juliaq: Zueuk's parameterized julia (rational power)
//   inv_power = divisor / power
//   inv_power_2pi = 2π / power
//   half_inv_power = 0.5 · inv_power
//   a = atan2(y, x) · inv_power + uniform_int · inv_power_2pi
//   r = (x² + y²)^half_inv_power
//   x' = r · cos(a),  y' = r · sin(a)
// =============================================================================
/// Rational-power Julia — like JuliaN but with separate `power` and
/// `divisor`, allowing fractional/rational branch counts (e.g. 3/2 gives
/// 1.5 branches).
///
/// # Authors
/// - Zueuk
pub static JULIAQ: VariationDef = VariationDef {
    name: "juliaq",
    aliases: &[],
    display_name: "JuliaQ",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        VariationParamDef { name: "power", display_name: "Power", param_type: ParamType::Integer,
                            default_value: 3.0, min_value: Some(1.0), max_value: Some(64.0), description: Some("Number of Julia branches in the rational power.") },
        VariationParamDef { name: "divisor", display_name: "Divisor", param_type: ParamType::Integer,
                            default_value: 2.0, min_value: Some(1.0), max_value: Some(64.0), description: Some("Rational-power divisor. Combined with `power` lets you pick non-integer branch counts (e.g. power=3, divisor=2 → 1.5 branches).") },
    ],
    // 3 derived values stored in slots 2..5:
    //   2: inv_power       (divisor / power)
    //   3: inv_power_2pi   (2π / power)
    //   4: half_inv_power  (0.5 · divisor / power)
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_juliaq(user: array<f32, 2>) -> array<f32, 3> {
    let power = max(user[0], 1.0);
    let divisor = user[1];
    let inv_power = divisor / power;
    var out: array<f32, 3>;
    out[0] = inv_power;
    out[1] = 6.28318530717959 / power;
    out[2] = 0.5 * inv_power;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_juliaq(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let inv_power = get_param(xform_id, variation_id, 2u);
    let inv_power_2pi = get_param(xform_id, variation_id, 3u);
    let half_inv_power = get_param(xform_id, variation_id, 4u);
    let rand_int = f32(i32(rng_next(rng) & 0x7FFFFFFFu));
    let a = atan2(p.y, p.x) * inv_power + rand_int * inv_power_2pi;
    let r = pow(max(p.x * p.x + p.y * p.y, 1e-30), half_inv_power);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_juliaq(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let inv_power = get_param(xform_id, variation_id, 2u);
    let inv_power_2pi = get_param(xform_id, variation_id, 3u);
    let half_inv_power = get_param(xform_id, variation_id, 4u);
    let rand_int = f32(i32(rng_next(rng) & 0x7FFFFFFFu));
    let a = atan2(p.y, p.x) * inv_power + rand_int * inv_power_2pi;
    let r = pow(max(p.x * p.x + p.y * p.y, 1e-30), half_inv_power);
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#,
};

// =============================================================================
// julia3dq: 3D version of juliaq — different exponent on r2d+z²
//   abs_inv_power = |inv_power|
//   half_inv_power = 0.5 · inv_power - 0.5
//   z_arg = z · abs_inv_power
//   r2d = x² + y²
//   r = (r2d + z_arg²)^half_inv_power
//   z' = r · z_arg
//   r *= sqrt(r2d)
//   x' = r · cos(a),  y' = r · sin(a)
// =============================================================================
/// 3D version of JuliaQ — extends the rational-power Julia into the Z axis.
///
/// # Authors
/// - Zueuk
pub static JULIA3DQ: VariationDef = VariationDef {
    name: "julia3Dq",
    aliases: &[],
    display_name: "Julia 3D Q",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        VariationParamDef { name: "power", display_name: "Power", param_type: ParamType::Integer,
                            default_value: 3.0, min_value: Some(1.0), max_value: Some(64.0), description: Some("Number of Julia branches.") },
        VariationParamDef { name: "divisor", display_name: "Divisor", param_type: ParamType::Integer,
                            default_value: 2.0, min_value: Some(1.0), max_value: Some(64.0), description: Some("Rational-power divisor.") },
    ],
    // 4 derived values stored in slots 2..6:
    //   2: inv_power       (divisor / power)
    //   3: inv_power_2pi   (2π / power)
    //   4: half_inv_power  (0.5 · inv_power − 0.5)
    //   5: abs_inv_power   (|inv_power|)
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_julia3Dq(user: array<f32, 2>) -> array<f32, 4> {
    let power = max(user[0], 1.0);
    let divisor = user[1];
    let inv_power = divisor / power;
    var out: array<f32, 4>;
    out[0] = inv_power;
    out[1] = 6.28318530717959 / power;
    out[2] = 0.5 * inv_power - 0.5;
    out[3] = abs(inv_power);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_julia3Dq(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let inv_power = get_param(xform_id, variation_id, 2u);
    let inv_power_2pi = get_param(xform_id, variation_id, 3u);
    let half_inv_power = get_param(xform_id, variation_id, 4u);
    let rand_int = f32(i32(rng_next(rng) & 0x7FFFFFFFu));
    let a = atan2(p.y, p.x) * inv_power + rand_int * inv_power_2pi;
    let r2d = max(p.x * p.x + p.y * p.y, 1e-30);
    let r_pow = pow(r2d, half_inv_power);
    let r = r_pow * sqrt(r2d);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_julia3Dq(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let inv_power = get_param(xform_id, variation_id, 2u);
    let inv_power_2pi = get_param(xform_id, variation_id, 3u);
    let half_inv_power = get_param(xform_id, variation_id, 4u);
    let abs_inv_power = get_param(xform_id, variation_id, 5u);
    let rand_int = f32(i32(rng_next(rng) & 0x7FFFFFFFu));
    let a = atan2(p.y, p.x) * inv_power + rand_int * inv_power_2pi;
    let z_arg = p.z * abs_inv_power;
    let r2d = max(p.x * p.x + p.y * p.y, 1e-30);
    let r_pow = pow(max(r2d + z_arg * z_arg, 1e-30), half_inv_power);
    let r = r_pow * sqrt(r2d);
    return vec3<f32>(r * cos(a), r * sin(a), r_pow * z_arg);
}
"#,
};

// =============================================================================
// juliac: David Young's complex-power julia
//   re_recip = 1 / (re_param + ε)
//   im_scaled = im_param / 100
//   arg = atan2(y, x) + (uniform_int mod re_recip) · 2π
//   lnmod = dist · 0.5 · log(x² + y²)
//   a = arg · re_recip + lnmod · im_scaled
//   mod2 = exp(lnmod · re_recip - arg · im_scaled)
//   x' = mod2 · cos(a),  y' = mod2 · sin(a)
//
// The C++ port uses `double VAR(re) = 1 / (VAR(re) + ε)` (variable shadowing
// via macro) — we name the locals differently to avoid confusion.
// =============================================================================
/// Complex-power Julia — `power = re + i·im`. Like CPow but with a `dist`
/// parameter that scales the log-of-radius term separately.
///
/// # Authors
/// - David Young
pub static JULIAC: VariationDef = VariationDef {
    name: "juliac",
    aliases: &[],
    display_name: "JuliaC",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        VariationParamDef { name: "re", display_name: "Re", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Real part of the complex power.") },
        VariationParamDef { name: "im", display_name: "Im", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Imaginary part of the complex power.") },
        VariationParamDef { name: "dist", display_name: "Dist", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Distance scaling on the log-of-radius term — affects how rapidly the spiral grows outward.") },
    ],
    // 2 derived values stored in slots 3..5:
    //   3: re_recip   (1 / (re_param + ε))
    //   4: im_scaled  (im_param / 100)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_juliac(user: array<f32, 3>) -> array<f32, 2> {
    let re_param = user[0];
    let im_param = user[1];
    var out: array<f32, 2>;
    out[0] = 1.0 / (re_param + 1e-30);
    out[1] = im_param / 100.0;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_juliac(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let dist = get_param(xform_id, variation_id, 2u);
    let re_recip = get_param(xform_id, variation_id, 3u);
    let im_scaled = get_param(xform_id, variation_id, 4u);
    let rand_int = f32(i32(rng_next(rng) & 0x7FFFFFFFu));
    // (rand_int mod re_recip) — using fract/floor since re_recip can be huge
    let arg = atan2(p.y, p.x) + (rand_int - floor(rand_int / max(abs(re_recip), 1e-30)) * re_recip) * two_pi;
    let lnmod = dist * 0.5 * log(max(p.x * p.x + p.y * p.y, 1e-30));
    let a = arg * re_recip + lnmod * im_scaled;
    let mod2 = exp(lnmod * re_recip - arg * im_scaled);
    return vec2<f32>(mod2 * cos(a), mod2 * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_juliac(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let dist = get_param(xform_id, variation_id, 2u);
    let re_recip = get_param(xform_id, variation_id, 3u);
    let im_scaled = get_param(xform_id, variation_id, 4u);
    let rand_int = f32(i32(rng_next(rng) & 0x7FFFFFFFu));
    let arg = atan2(p.y, p.x) + (rand_int - floor(rand_int / max(abs(re_recip), 1e-30)) * re_recip) * two_pi;
    let lnmod = dist * 0.5 * log(max(p.x * p.x + p.y * p.y, 1e-30));
    let a = arg * re_recip + lnmod * im_scaled;
    let mod2 = exp(lnmod * re_recip - arg * im_scaled);
    return vec3<f32>(mod2 * cos(a), mod2 * sin(a), p.z);
}
"#,
};
