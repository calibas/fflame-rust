//! vibration2 (FarDareisMai / ported by Brad Stefanov)
//!
//! Two-wave directional vibration with per-wave modulation of dir,
//! angle, freq, and amp. Each wave has 5 base params (dir, angle,
//! freq, amp, phase) plus 8 modulation params (dm/dmfreq for dir,
//! tm/tmfreq for angle, fm/fmfreq for freq, am/amfreq for amp).
//!
//! Total: 26 user params (was over the old 16-slot ceiling — first
//! port unblocked by the packed-variation-params buffer change).
//! No init slots. Body factors cleanly through outer multiplier.
//!
//! Each wave computes:
//!   d_along_dir = p · (cos(dir), sin(dir))
//!   dirL  = dir   + dm · cos(d_along_dir · dmfreq · 2π)
//!   angleL = angle + tm · cos(d_along_dir · tmfreq · 2π)
//!   freqL  =        fm · cos(d_along_dir · fmfreq · 2π) / freq
//!   ampL   = amp + amp · am · cos(d_along_dir · amfreq · 2π)
//!   then projects onto rotated direction at total_angle = angleL+dirL
//!   and emits amp · sin((d_along_dir · freq · 2π) + freqL + phase·2π/freq)
//!
//! Source: `output/jwildfire-vars/output/vibration2.cpp`
//! (Java-recovered; cpp PluginVarCalc empty unported_stub).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Two-wave directional vibration — adds the sum of two independent sine
/// waves to the input, where each wave's direction, angle, frequency, and
/// amplitude are themselves modulated by sinusoids of the projected
/// coordinate along the wave's direction. Each wave has 5 base params
/// (`dir`, `angle`, `freq`, `amp`, `phase`) plus 4 modulation pairs
/// (`dm`/`dmfreq` for direction, `tm`/`tmfreq` for angle, `fm`/`fmfreq` for
/// frequency, `am`/`amfreq` for amplitude) — 13 params per wave × 2 waves =
/// 26 user params total, which was the first variation to exceed the old
/// 16-slot per-variation buffer ceiling and motivated switching to the
/// packed-variation-params buffer layout.
///
/// # Authors
/// - FarDareisMai
/// - Brad Stefanov
pub static VIBRATION2: VariationDef = VariationDef {
    name: "vibration2",
    aliases: &[],
    display_name: "Vibration 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("dir", "Direction", unlimited_float, 0.0, -10.0, 10.0, "Wave 1: direction angle (radians) along which the wave propagates. The input is projected onto this direction before phase computation."),
        param!("angle", "Angle", unlimited_float, 1.5708, -10.0, 10.0, "Wave 1: output rotation angle (radians) — controls which direction in the output plane the wave's displacement points."),
        param!("freq", "Frequency", unlimited_float, 1.0, -10.0, 10.0, "Wave 1: spatial frequency. Smaller = longer wavelength."),
        param!("amp", "Amplitude", unlimited_float, 0.25, -10.0, 10.0, "Wave 1: amplitude (peak displacement magnitude)."),
        param!("phase", "Phase", unlimited_float, 0.0, -10.0, 10.0, "Wave 1: phase offset in cycles; converted internally to radians as `2π·phase/freq`."),
        param!("dir2", "Direction 2", unlimited_float, 1.5708, -10.0, 10.0, "Wave 2: direction angle. See `dir`."),
        param!("angle2", "Angle 2", unlimited_float, 1.5708, -10.0, 10.0, "Wave 2: output rotation angle. See `angle`."),
        param!("freq2", "Frequency 2", unlimited_float, 1.0, -10.0, 10.0, "Wave 2: spatial frequency. See `freq`."),
        param!("amp2", "Amplitude 2", unlimited_float, 0.25, -10.0, 10.0, "Wave 2: amplitude. See `amp`."),
        param!("phase2", "Phase 2", unlimited_float, 0.0, -10.0, 10.0, "Wave 2: phase offset. See `phase`."),
        param!("dm", "Dir Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 1: direction modulation depth. The propagation direction wobbles by `dm · cos(d · dmfreq · 2π)` where `d` is the projected coordinate."),
        param!("dmfreq", "Dir Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 1: spatial frequency of the direction-modulation wobble."),
        param!("tm", "Angle Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 1: output-angle modulation depth (same shape as `dm` but applied to the output rotation angle)."),
        param!("tmfreq", "Angle Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 1: spatial frequency of the output-angle modulation."),
        param!("fm", "Freq Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 1: frequency-modulation depth. Modulates the wave's phase term by `fm · cos(d · fmfreq · 2π) / freq`."),
        param!("fmfreq", "Freq Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 1: spatial frequency of the frequency modulation."),
        param!("am", "Amp Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 1: amplitude-modulation depth. Multiplies amplitude by `1 + am · cos(d · amfreq · 2π)`."),
        param!("amfreq", "Amp Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 1: spatial frequency of the amplitude modulation."),
        param!("d2m", "Dir 2 Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 2: direction modulation depth. See `dm`."),
        param!("d2mfreq", "Dir 2 Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 2: direction modulation frequency. See `dmfreq`."),
        param!("t2m", "Angle 2 Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 2: angle modulation depth. See `tm`."),
        param!("t2mfreq", "Angle 2 Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 2: angle modulation frequency. See `tmfreq`."),
        param!("f2m", "Freq 2 Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 2: frequency modulation depth. See `fm`."),
        param!("f2mfreq", "Freq 2 Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 2: frequency modulation frequency. See `fmfreq`."),
        param!("a2m", "Amp 2 Modulation", unlimited_float, 0.0, -10.0, 10.0, "Wave 2: amplitude modulation depth. See `am`."),
        param!("a2mfreq", "Amp 2 Mod Freq", unlimited_float, 0.1, -10.0, 10.0, "Wave 2: amplitude modulation frequency. See `amfreq`."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn v2_modulate(amp: f32, freq: f32, x: f32) -> f32 {
    let two_pi = 6.28318530717959;
    return amp * cos(x * freq * two_pi);
}

fn v2_wave(p: vec2<f32>, dir: f32, angle: f32, freq: f32, amp: f32, phase: f32,
           dm: f32, dmfreq: f32, tm: f32, tmfreq: f32,
           fm: f32, fmfreq: f32, am: f32, amfreq: f32) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    var d_along_dir = p.x * cos(dir) + p.y * sin(dir);
    let dir_l = dir + v2_modulate(dm, dmfreq, d_along_dir);
    let angle_l = angle + v2_modulate(tm, tmfreq, d_along_dir);
    let safe_freq = select(freq, 1e-30, abs(freq) < 1e-30);
    let freq_l = v2_modulate(fm, fmfreq, d_along_dir) / safe_freq;
    let amp_l = amp + amp * v2_modulate(am, amfreq, d_along_dir);

    let total_angle = angle_l + dir_l;
    let cos_dir = cos(dir_l);
    let sin_dir = sin(dir_l);
    let cos_tot = cos(total_angle);
    let sin_tot = sin(total_angle);
    let scaled_freq = freq * two_pi;
    let phase_shift = two_pi * phase / safe_freq;
    d_along_dir = p.x * cos_dir + p.y * sin_dir;
    let local_amp = amp_l * sin(d_along_dir * scaled_freq + freq_l + phase_shift);

    return vec2<f32>(local_amp * cos_tot, local_amp * sin_tot);
}

fn variation_vibration2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let dir = get_param(xform_id, variation_id, 0u);
    let angle = get_param(xform_id, variation_id, 1u);
    let freq = get_param(xform_id, variation_id, 2u);
    let amp = get_param(xform_id, variation_id, 3u);
    let phase = get_param(xform_id, variation_id, 4u);
    let dir2 = get_param(xform_id, variation_id, 5u);
    let angle2 = get_param(xform_id, variation_id, 6u);
    let freq2 = get_param(xform_id, variation_id, 7u);
    let amp2 = get_param(xform_id, variation_id, 8u);
    let phase2 = get_param(xform_id, variation_id, 9u);
    let dm = get_param(xform_id, variation_id, 10u);
    let dmfreq = get_param(xform_id, variation_id, 11u);
    let tm = get_param(xform_id, variation_id, 12u);
    let tmfreq = get_param(xform_id, variation_id, 13u);
    let fm = get_param(xform_id, variation_id, 14u);
    let fmfreq = get_param(xform_id, variation_id, 15u);
    let am = get_param(xform_id, variation_id, 16u);
    let amfreq = get_param(xform_id, variation_id, 17u);
    let d2m = get_param(xform_id, variation_id, 18u);
    let d2mfreq = get_param(xform_id, variation_id, 19u);
    let t2m = get_param(xform_id, variation_id, 20u);
    let t2mfreq = get_param(xform_id, variation_id, 21u);
    let f2m = get_param(xform_id, variation_id, 22u);
    let f2mfreq = get_param(xform_id, variation_id, 23u);
    let a2m = get_param(xform_id, variation_id, 24u);
    let a2mfreq = get_param(xform_id, variation_id, 25u);

    let w1 = v2_wave(p, dir, angle, freq, amp, phase, dm, dmfreq, tm, tmfreq, fm, fmfreq, am, amfreq);
    let w2 = v2_wave(p, dir2, angle2, freq2, amp2, phase2, d2m, d2mfreq, t2m, t2mfreq, f2m, f2mfreq, a2m, a2mfreq);
    return p + w1 + w2;
}
"#,
    wgsl_3d: r#"
fn v2_modulate(amp: f32, freq: f32, x: f32) -> f32 {
    let two_pi = 6.28318530717959;
    return amp * cos(x * freq * two_pi);
}

fn v2_wave(p: vec2<f32>, dir: f32, angle: f32, freq: f32, amp: f32, phase: f32,
           dm: f32, dmfreq: f32, tm: f32, tmfreq: f32,
           fm: f32, fmfreq: f32, am: f32, amfreq: f32) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    var d_along_dir = p.x * cos(dir) + p.y * sin(dir);
    let dir_l = dir + v2_modulate(dm, dmfreq, d_along_dir);
    let angle_l = angle + v2_modulate(tm, tmfreq, d_along_dir);
    let safe_freq = select(freq, 1e-30, abs(freq) < 1e-30);
    let freq_l = v2_modulate(fm, fmfreq, d_along_dir) / safe_freq;
    let amp_l = amp + amp * v2_modulate(am, amfreq, d_along_dir);

    let total_angle = angle_l + dir_l;
    let cos_dir = cos(dir_l);
    let sin_dir = sin(dir_l);
    let cos_tot = cos(total_angle);
    let sin_tot = sin(total_angle);
    let scaled_freq = freq * two_pi;
    let phase_shift = two_pi * phase / safe_freq;
    d_along_dir = p.x * cos_dir + p.y * sin_dir;
    let local_amp = amp_l * sin(d_along_dir * scaled_freq + freq_l + phase_shift);

    return vec2<f32>(local_amp * cos_tot, local_amp * sin_tot);
}

fn variation_vibration2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let dir = get_param(xform_id, variation_id, 0u);
    let angle = get_param(xform_id, variation_id, 1u);
    let freq = get_param(xform_id, variation_id, 2u);
    let amp = get_param(xform_id, variation_id, 3u);
    let phase = get_param(xform_id, variation_id, 4u);
    let dir2 = get_param(xform_id, variation_id, 5u);
    let angle2 = get_param(xform_id, variation_id, 6u);
    let freq2 = get_param(xform_id, variation_id, 7u);
    let amp2 = get_param(xform_id, variation_id, 8u);
    let phase2 = get_param(xform_id, variation_id, 9u);
    let dm = get_param(xform_id, variation_id, 10u);
    let dmfreq = get_param(xform_id, variation_id, 11u);
    let tm = get_param(xform_id, variation_id, 12u);
    let tmfreq = get_param(xform_id, variation_id, 13u);
    let fm = get_param(xform_id, variation_id, 14u);
    let fmfreq = get_param(xform_id, variation_id, 15u);
    let am = get_param(xform_id, variation_id, 16u);
    let amfreq = get_param(xform_id, variation_id, 17u);
    let d2m = get_param(xform_id, variation_id, 18u);
    let d2mfreq = get_param(xform_id, variation_id, 19u);
    let t2m = get_param(xform_id, variation_id, 20u);
    let t2mfreq = get_param(xform_id, variation_id, 21u);
    let f2m = get_param(xform_id, variation_id, 22u);
    let f2mfreq = get_param(xform_id, variation_id, 23u);
    let a2m = get_param(xform_id, variation_id, 24u);
    let a2mfreq = get_param(xform_id, variation_id, 25u);

    let w1 = v2_wave(p.xy, dir, angle, freq, amp, phase, dm, dmfreq, tm, tmfreq, fm, fmfreq, am, amfreq);
    let w2 = v2_wave(p.xy, dir2, angle2, freq2, amp2, phase2, d2m, d2mfreq, t2m, t2mfreq, f2m, f2mfreq, a2m, a2mfreq);
    return vec3<f32>(p.x + w1.x + w2.x, p.y + w1.y + w2.y, p.z);
}
"#,
};
