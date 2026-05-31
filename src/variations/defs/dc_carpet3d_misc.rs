//! dc_carpet3D (Xyrus02 / Brad Stefanov)
//!
//! 3D carpet IFS — picks a random corner of the unit square by independent
//! ±1 sign choices on X and Y (with per-axis offsets `stretch_x` /
//! `stretch_y`), reapplies the transform's affine to (x, y) (yes, twice —
//! intentional in upstream), scales by `scale_x` / `scale_y`, and writes
//! the next color via a fmod-based mixing rule that combines the previous
//! color register with `color_a..color_f` and the `x0 ^ y0` integer parity
//! of the corner choice. Z is then driven by the *new* color: `dz =
//! vc · scale_z + offset_z`, optionally overriding Z when `reset_z > 0`.
//!
//! Direct-color full port — exercises both `*vc` write *and* `*vc` read
//! within a single variation body: color is written first, then re-read
//! to compute the spatial Z output. Demonstrates that variation-port
//! blocker #11 (color-write coupled to spatial output) is no longer an
//! architectural blocker now that `vc: ptr<function, f32>` is plumbed.
//!
//! 14 user params (origin, color_a/b/c/d/e/f, stretch_x/y, scale_x/y,
//! scale_z, offset_z, reset_z). 1 init slot (`H = 0.1 · origin`,
//! precomputed because it's user-only).
//!
//! Source: `output/jwildfire-vars/output/dc_carpet3D.cpp`
//! (Java-recovered; cpp `PluginVarCalc` is the empty stub —
//! Java body embedded as comment is the canonical reference).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 3D Sierpinski-carpet IFS with direct-color writes and color-coupled Z.
///
/// Picks one of the four unit-square corners by independent ±1 sign
/// choices on X and Y (with `stretch_x` / `stretch_y` controlling the
/// magnitude of the corner offset), reapplies the transform's affine
/// matrix to the offset position, and scales each axis by `scale_x` /
/// `scale_y`. Color is updated via a fmod mixing rule combining the
/// previous color value with `color_a..color_f` and the parity of the
/// corner choice; the new color then drives Z via
/// `dz = color · scale_z + offset_z`, with `reset_z > 0` switching
/// Z-accumulation off so the color-derived `dz` becomes Z directly.
///
/// Visible color contribution requires the transform's Direct Color
/// slider to be > 0 (the per-iteration final color is
/// `c_base + direct_color · (vc − c_base)`).
///
/// # Authors
/// - Xyrus02
/// - Brad Stefanov
pub static DC_CARPET3D: VariationDef = VariationDef {
    name: "dc_carpet3D",
    aliases: &[],
    display_name: "DC Carpet 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("origin", "Origin", unlimited_float, 0.5, -10.0, 10.0, "Scales the per-iteration color-mix amplitude (precomputed as `H = 0.1 · origin`)."),
        param!("color_a", "Color A", unlimited_float, 0.5, -10.0, 10.0, "Multiplier on the previous color value in the color mix."),
        param!("color_b", "Color B", unlimited_float, 1.0, -10.0, 10.0, "Bias term in `hh = H · (color_b − x0⊕y0 − 1)`."),
        param!("color_c", "Color C", unlimited_float, 1.0, -10.0, 10.0, "Bias added to `hh` before multiplying by the previous color."),
        param!("color_d", "Color D", unlimited_float, 1.0, -10.0, 10.0, "Bias subtracted from `hh` before multiplying by the `x0⊕y0` parity term."),
        param!("color_e", "Color E", unlimited_float, 0.5, -10.0, 10.0, "Multiplier on the parity-term contribution to the color mix."),
        param!("color_f", "Color F", unlimited_float, 1.0, -10.0, 10.0, "fmod modulus on the final color value — controls the color repeat period."),
        param!("stretch_x", "Stretch X", unlimited_float, 1.0, -10.0, 10.0, "Magnitude of the ±1 X corner offset added to the input. Larger pulls corners further apart along X."),
        param!("stretch_y", "Stretch Y", unlimited_float, 1.0, -10.0, 10.0, "Magnitude of the ±1 Y corner offset added to the input."),
        param!("scale_x", "Scale X", unlimited_float, 1.0, -10.0, 10.0, "Post-affine X scale applied to the carpet position."),
        param!("scale_y", "Scale Y", unlimited_float, 1.0, -10.0, 10.0, "Post-affine Y scale applied to the carpet position."),
        param!("scale_z", "Scale Z", unlimited_float, 1.0, -10.0, 10.0, "Color-to-Z coupling multiplier — Z output includes `color · scale_z`."),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0, "Constant Z bump added to the color-driven Z term per iteration."),
        param!("reset_z", "Reset Z", unlimited_float, 0.0, 0.0, 1.0, "When > 0, the color-derived `dz` overrides Z rather than accumulating onto the pre-existing Z."),
    ],
    needs_transform: true,
    writes_color: true,
    // 1 derived value at slot 14:
    //   14: H = 0.1 * origin   (precomputed per-frame init)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_dc_carpet3D(user: array<f32, 14>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 0.1 * user[0];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    // 2D body: spatial carpet + color write. No Z output (2D mode), so the
    // `dz` term collapses; we still write color so the variation behaves
    // consistently across render modes.
    wgsl_2d: r#"
fn variation_dc_carpet3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let color_a = get_param(xform_id, variation_id, 1u);
    let color_b = get_param(xform_id, variation_id, 2u);
    let color_c = get_param(xform_id, variation_id, 3u);
    let color_d = get_param(xform_id, variation_id, 4u);
    let color_e = get_param(xform_id, variation_id, 5u);
    let color_f = get_param(xform_id, variation_id, 6u);
    let stretch_x = get_param(xform_id, variation_id, 7u);
    let stretch_y = get_param(xform_id, variation_id, 8u);
    let scale_x = get_param(xform_id, variation_id, 9u);
    let scale_y = get_param(xform_id, variation_id, 10u);
    let h = get_param(xform_id, variation_id, 14u);
    let xf = transforms[xform_id];

    let x0_i = select(1, -1, rng_nextf(rng) < 0.5);
    let y0_i = select(1, -1, rng_nextf(rng) > 0.5);
    let x0 = f32(x0_i);
    let y0 = f32(y0_i);
    let x0_xor_y0 = f32(x0_i ^ y0_i);
    let hh = h * (color_b - x0_xor_y0 - 1.0);

    // Read prior color, compute new mix, write back. The read demonstrates
    // that `vc` is a true read/write pointer; the write is the standard DC path.
    let current_vc = *vc;
    let mod_div = select(color_f, 1e-30, abs(color_f) < 1e-30);
    let raw = abs(current_vc * color_a * (color_c + hh) + x0_xor_y0 * (color_d - hh) * color_e);
    *vc = raw - floor(raw / mod_div) * mod_div;

    let x = p.x + x0 * stretch_x;
    let y = p.y + y0 * stretch_y;
    let nx = (xf.a * x + xf.b * y + xf.e) * scale_x;
    let ny = (xf.c * x + xf.d * y + xf.f) * scale_y;
    return vec2<f32>(nx, ny);
}
"#,
    // 3D body: spatial carpet + color write + color-driven Z.
    // The Z formula re-reads `*vc` after the write so it sees the newly
    // mixed color — that's the #11 test (write-then-read in spatial).
    wgsl_3d: r#"
fn variation_dc_carpet3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let color_a = get_param(xform_id, variation_id, 1u);
    let color_b = get_param(xform_id, variation_id, 2u);
    let color_c = get_param(xform_id, variation_id, 3u);
    let color_d = get_param(xform_id, variation_id, 4u);
    let color_e = get_param(xform_id, variation_id, 5u);
    let color_f = get_param(xform_id, variation_id, 6u);
    let stretch_x = get_param(xform_id, variation_id, 7u);
    let stretch_y = get_param(xform_id, variation_id, 8u);
    let scale_x = get_param(xform_id, variation_id, 9u);
    let scale_y = get_param(xform_id, variation_id, 10u);
    let scale_z = get_param(xform_id, variation_id, 11u);
    let offset_z = get_param(xform_id, variation_id, 12u);
    let reset_z = get_param(xform_id, variation_id, 13u);
    let h = get_param(xform_id, variation_id, 14u);
    let xf = transforms[xform_id];
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x0_i = select(1, -1, rng_nextf(rng) < 0.5);
    let y0_i = select(1, -1, rng_nextf(rng) > 0.5);
    let x0 = f32(x0_i);
    let y0 = f32(y0_i);
    let x0_xor_y0 = f32(x0_i ^ y0_i);
    let hh = h * (color_b - x0_xor_y0 - 1.0);

    // Write next color first (Java order: `pVarTP.color = ...` before `dz = ...`).
    let current_vc = *vc;
    let mod_div = select(color_f, 1e-30, abs(color_f) < 1e-30);
    let raw = abs(current_vc * color_a * (color_c + hh) + x0_xor_y0 * (color_d - hh) * color_e);
    let new_vc = raw - floor(raw / mod_div) * mod_div;
    *vc = new_vc;

    // Color-driven Z. With `needs_transform` we return `nz` such that the
    // outer `w · nz` multiplication restores Java's accumulator semantics:
    //   reset_z = 0:  pVarTP.z += w·p.z + dz   → return p.z + dz·inv_w
    //   reset_z > 0:  pVarTP.z  = dz           → return dz·inv_w
    let dz = new_vc * scale_z + offset_z;
    let nz = select(p.z + dz * inv_w, dz * inv_w, reset_z > 0.0);

    let x = p.x + x0 * stretch_x;
    let y = p.y + y0 * stretch_y;
    let nx = (xf.a * x + xf.b * y + xf.e) * scale_x;
    let ny = (xf.c * x + xf.d * y + xf.f) * scale_y;
    return vec3<f32>(nx, ny, nz);
}
"#,
};
