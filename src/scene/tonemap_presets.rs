//! Bundled tonemap presets — named brightness/curve "looks" the user
//! can snap to without touching their flame, palette, or background.
//!
//! v1 keeps the list hardcoded in source. Presets ship as a single
//! `&[TonemapPreset]`, no asset loading, no WASM/desktop split.
//! Future iterations can promote this to a user-editable JSON file
//! if/when the preset library gets large enough to warrant it.
//!
//! Initial set derived from the corpus survey performed during the
//! `levels-scale-invariance` project (docs/projects/tonemap-and-palette-improvements.md):
//! ~90% of 141 .fflame files use identical defaults; the deviants
//! cluster into 3 named recipes (Apophysis Bubble / Apophysis Discus /
//! Subtle Gamma 2.2). The "Vivid / High Contrast / Low-Key" entries
//! are hand-tuned starting points for users who want quick stylistic
//! variation.

use crate::config::defaults::*;

/// A named "look" — a partial FractalConfig containing only the
/// brightness/curve fields. Selecting a preset writes these values
/// into the current FractalConfig and leaves everything else
/// (flame, palette, background, view state) untouched.
///
/// `use_curve` is included so a preset can disable any custom curve
/// the user previously authored; the curve's *shape*
/// (`FractalConfig::tonemap_curve`) is intentionally not part of
/// presets — bundled presets stay simple, and the user's edited
/// curve survives preset switching.
#[derive(Debug, Clone, Copy)]
pub struct TonemapPreset {
    pub name: &'static str,
    pub exposure: f32,
    pub gamma: f32,
    pub gamma_threshold: f32,
    pub brightness: f32,
    pub vibrancy: f32,
    pub saturation: f32,
    pub hue_shift: f32,
    pub use_curve: bool,
    pub levels_low: f32,
    pub levels_high: f32,
    pub levels_gamma: f32,
    pub alpha_blend_low: f32,
    pub alpha_blend_high: f32,
}

/// The bundled preset library. Order is the order shown in the UI dropdown.
pub const TONEMAP_PRESETS: &[TonemapPreset] = &[
    // Default — exactly the FractalConfig defaults. Selecting this
    // is a fast reset for users who got lost adjusting sliders.
    TonemapPreset {
        name: "Default",
        exposure: DEFAULT_EXPOSURE,
        gamma: DEFAULT_GAMMA,
        gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
        brightness: DEFAULT_BRIGHTNESS,
        vibrancy: 1.0,
        saturation: DEFAULT_SATURATION,
        hue_shift: DEFAULT_HUE_SHIFT,
        use_curve: true,
        levels_low: 0.0,
        levels_high: 1.0,
        levels_gamma: 1.0,
        alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
    },
    // Subtle (Gamma 2.2) — seven flames in the corpus use this exact
    // tonemap point. A mild gamma boost over default; everything else
    // stays at default.
    TonemapPreset {
        name: "Subtle (Gamma 2.2)",
        exposure: DEFAULT_EXPOSURE,
        gamma: 2.2,
        gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
        brightness: DEFAULT_BRIGHTNESS,
        vibrancy: 1.0,
        saturation: DEFAULT_SATURATION,
        hue_shift: DEFAULT_HUE_SHIFT,
        use_curve: true,
        levels_low: 0.0,
        levels_high: 1.0,
        levels_gamma: 1.0,
        alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
    },
    // Apophysis Bubble — seven flames cluster here (bubble2/4/5/6-3d,
    // bubble-3d, grand-julian2, warmup). High gamma + high threshold
    // + boosted brightness; characteristic "smooth bright bubble" look.
    TonemapPreset {
        name: "Apophysis Bubble",
        exposure: DEFAULT_EXPOSURE,
        gamma: 3.0,
        gamma_threshold: 55.0,
        brightness: 30.0,
        vibrancy: 1.0,
        saturation: 1.5,
        hue_shift: DEFAULT_HUE_SHIFT,
        use_curve: true,
        levels_low: 0.0,
        levels_high: 1.0,
        levels_gamma: 1.0,
        alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
    },
    // Apophysis Discus — three flames share this exact tonemap
    // (discus, discus2, discus3-anim). Heavy gamma + high threshold,
    // vibrancy boost, more saturation than Bubble.
    TonemapPreset {
        name: "Apophysis Discus",
        exposure: DEFAULT_EXPOSURE,
        gamma: 5.7,
        gamma_threshold: 140.0,
        brightness: 12.0,
        vibrancy: 1.7,
        saturation: 1.85,
        hue_shift: DEFAULT_HUE_SHIFT,
        use_curve: true,
        levels_low: 0.0,
        levels_high: 1.0,
        levels_gamma: 1.0,
        alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
    },
    // Vivid — moderate saturation + vibrancy boost over default for
    // popping colors without changing brightness/gamma.
    TonemapPreset {
        name: "Vivid",
        exposure: DEFAULT_EXPOSURE,
        gamma: DEFAULT_GAMMA,
        gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
        brightness: DEFAULT_BRIGHTNESS,
        vibrancy: 1.3,
        saturation: 1.4,
        hue_shift: DEFAULT_HUE_SHIFT,
        use_curve: true,
        levels_low: 0.0,
        levels_high: 1.0,
        levels_gamma: 1.0,
        alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
    },
    // High Contrast — gamma + Levels working together to bring the
    // bright core forward and fade dim wings toward background.
    TonemapPreset {
        name: "High Contrast",
        exposure: DEFAULT_EXPOSURE,
        gamma: 1.5,
        gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
        brightness: DEFAULT_BRIGHTNESS,
        vibrancy: 1.0,
        saturation: DEFAULT_SATURATION,
        hue_shift: DEFAULT_HUE_SHIFT,
        use_curve: true,
        levels_low: 0.0,
        levels_high: 2.0,  // clip at 2× mean density — tighter than default
        levels_gamma: 1.0,
        alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
    },
    // Low-Key — dimmer exposure produces a moody, restrained look
    // useful for dark-palette flames against a black background.
    TonemapPreset {
        name: "Low-Key",
        exposure: 0.6,
        gamma: DEFAULT_GAMMA,
        gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
        brightness: DEFAULT_BRIGHTNESS,
        vibrancy: 1.0,
        saturation: DEFAULT_SATURATION,
        hue_shift: DEFAULT_HUE_SHIFT,
        use_curve: true,
        levels_low: 0.0,
        levels_high: 1.0,
        levels_gamma: 1.0,
        alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
        alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
    },
];
