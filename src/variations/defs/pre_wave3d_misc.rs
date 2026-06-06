//! pre_wave3d_wf
//!
//! Pre-phase wave perturbation that displaces the affine input along
//! one of four axis modes (XY, YZ, ZX, RADIAL). For axial modes,
//! displaces the perpendicular axis by the sine wave amplitude. For
//! radial mode, displaces along the unit vector from the center.
//!
//! 7 user params:
//!   - axis (int 0=XY, 1=YZ, 2=ZX, 3=RADIAL)
//!   - wavelen (default 0.5)
//!   - phase
//!   - damping (default 0.01; zero or near-zero disables damping)
//!   - centre_x, centre_y, centre_z
//!
//! No init slots. Body: `r = distance from center along chosen axes`,
//! `dl = r / wavelen`, `amplitude = w · exp(-dl · damping)` (or just
//! `w` if damping ≈ 0), `amp = amplitude · sin(2π·dl + phase)`.
//!
//! needs_transform=true to access weight in pre phase.
//!
//! Source: `output/jwildfire-vars/output/pre_wave3d_wf.cpp`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Pre-phase 3D wave perturbation — displaces the affine input by a sine-
/// wave amplitude `amp = w · exp(-dl · damping) · sin(2π·dl + phase)` where
/// `dl = distance/wavelen`. The displacement direction depends on `axis`:
/// planar modes (XY/YZ/ZX) push perpendicular to the plane; radial mode
/// pushes along the unit vector from the center. Damping is disabled when
/// `|damping| ≈ 0`.
pub static PRE_WAVE3D_WF: VariationDef = VariationDef {
    name: "pre_wave3D_wf",
    aliases: &[],
    display_name: "Pre Wave 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Pre,
    features: &[Feature::NeedsTransform],
    parameters: &[
        param!("axis", "Axis", enum, 0, &["XY → Z", "YZ → X", "ZX → Y", "Radial"], "Wave plane and displacement axis. Plane modes wave one coordinate in and out of the other two; Radial waves along the unit vector from the origin."),
        param!("wavelen", "Wavelength", unlimited_float, 0.5, -10.0, 10.0, "Wavelength of the sine perturbation."),
        param!("phase", "Phase", unlimited_float, 0.0, -10.0, 10.0, "Phase offset on the sine argument."),
        param!("damping", "Damping", unlimited_float, 0.01, -10.0, 10.0, "Exponential damping rate (proportional to `dl`). 0 disables damping."),
        param!("centre_x", "Centre X", unlimited_float, 0.0, -10.0, 10.0, "X coordinate of the wave center (subtracted from input before the distance calculation)."),
        param!("centre_y", "Centre Y", unlimited_float, 0.0, -10.0, 10.0, "Y coordinate of the wave center."),
        param!("centre_z", "Centre Z", unlimited_float, 0.0, -10.0, 10.0, "Z coordinate of the wave center."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_pre_wave3D_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let axis = i32(get_param(xform_id, variation_id, 0u));
    let wavelen = get_param(xform_id, variation_id, 1u);
    let phase = get_param(xform_id, variation_id, 2u);
    let damping = get_param(xform_id, variation_id, 3u);
    let cx = get_param(xform_id, variation_id, 4u);
    let cy = get_param(xform_id, variation_id, 5u);
    let cz = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let two_pi = 6.28318530717959;

    let dx = p.x - cx;
    let dy = p.y - cy;
    let dz = -cz;
    var r: f32;
    if (axis == 3) {
        r = sqrt(dx * dx + dy * dy + dz * dz);
    } else if (axis == 1) {
        r = sqrt(dy * dy + dz * dz);
    } else if (axis == 2) {
        r = sqrt(dz * dz + dx * dx);
    } else {
        r = sqrt(dx * dx + dy * dy);
    }
    let safe_wavelen = select(wavelen, 1e-30, abs(wavelen) < 1e-30);
    let dl = r / safe_wavelen;
    var amplitude = w;
    if (abs(damping) > 1e-6) {
        amplitude = amplitude * exp(-dl * damping);
    }
    let amp = amplitude * sin(two_pi * dl + phase);

    if (axis == 3) {
        let safe_r = select(r, 1e-30, r < 1e-30);
        return vec2<f32>(p.x + dx / safe_r * amp, p.y + dy / safe_r * amp);
    }
    if (axis == 1) {
        return vec2<f32>(p.x + amp, p.y);
    }
    if (axis == 2) {
        return vec2<f32>(p.x, p.y + amp);
    }
    return vec2<f32>(p.x, p.y);
}
"#,
    wgsl_3d: r#"
fn variation_pre_wave3D_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let axis = i32(get_param(xform_id, variation_id, 0u));
    let wavelen = get_param(xform_id, variation_id, 1u);
    let phase = get_param(xform_id, variation_id, 2u);
    let damping = get_param(xform_id, variation_id, 3u);
    let cx = get_param(xform_id, variation_id, 4u);
    let cy = get_param(xform_id, variation_id, 5u);
    let cz = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let two_pi = 6.28318530717959;

    let dx = p.x - cx;
    let dy = p.y - cy;
    let dz = p.z - cz;
    var r: f32;
    if (axis == 3) {
        r = sqrt(dx * dx + dy * dy + dz * dz);
    } else if (axis == 1) {
        r = sqrt(dy * dy + dz * dz);
    } else if (axis == 2) {
        r = sqrt(dz * dz + dx * dx);
    } else {
        r = sqrt(dx * dx + dy * dy);
    }
    let safe_wavelen = select(wavelen, 1e-30, abs(wavelen) < 1e-30);
    let dl = r / safe_wavelen;
    var amplitude = w;
    if (abs(damping) > 1e-6) {
        amplitude = amplitude * exp(-dl * damping);
    }
    let amp = amplitude * sin(two_pi * dl + phase);

    if (axis == 3) {
        let safe_r = select(r, 1e-30, r < 1e-30);
        return vec3<f32>(p.x + dx / safe_r * amp, p.y + dy / safe_r * amp, p.z + dz / safe_r * amp);
    }
    if (axis == 1) {
        return vec3<f32>(p.x + amp, p.y, p.z);
    }
    if (axis == 2) {
        return vec3<f32>(p.x, p.y + amp, p.z);
    }
    return vec3<f32>(p.x, p.y, p.z + amp);
}
"#,
};
