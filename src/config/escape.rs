//! Escape-time fractal configuration.
//!
//! The per-config state for the fragment rendering modes (Mandelbrot
//! and kin) — everything `docs/projects/escape-time-fractals.md` calls
//! `EscapeConfig`. Lives inside [`FractalConfig`] behind
//! skip-if-default, so a flame that has never touched escape mode
//! serializes exactly as before, byte for byte.
//!
//! Two shapes deliberately unlike the rest of the config:
//!
//! * **The center is a pair of decimal strings**, not floats. A
//!   deep-zoom center at 1e-300 does not fit in any float the config
//!   could hold; strings are exact at every depth, cost nothing at
//!   shallow zoom (phase 1 parses them to f64), and become the input
//!   to the fixed-point module in phase 4 unchanged. Zoom is the
//!   float: `zoom_log2`, so animating the *exponent* is an ordinary
//!   float track.
//! * **Per-formula and per-coloring parameters are keyed maps**, not
//!   fields — the same choice `Transform::variation_params` made, so
//!   500 variations never needed 500 struct fields. `BTreeMap` rather
//!   than `HashMap` for deterministic iteration (UI ordering, JSON
//!   output, and future GPU packing order all read it).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Escape-time (fragment mode) settings. See the module docs.
///
/// `PartialEq` is load-bearing: `is_default` compares against
/// `Self::default()`, the same pattern `SolidShadingSettings` uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscapeConfig {
    /// Which formula to iterate, by registry name (`"mandelbrot"`,
    /// `"burning_ship"`, …). A name the build doesn't know renders the
    /// default formula with a warning rather than failing the load —
    /// same forward-compatibility posture as script flags.
    #[serde(default = "default_formula")]
    pub formula: String,

    /// Julia toggle: `false` = parameter plane (pixel is `c`),
    /// `true` = dynamical plane (pixel is `z₀`, `c` fixed below).
    #[serde(default, skip_serializing_if = "is_false")]
    pub julia: bool,
    /// The fixed `c` in Julia mode. Ignored (but preserved) otherwise,
    /// so toggling Julia off and on round-trips the seed.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub julia_re: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub julia_im: f32,

    /// View center, exact decimal strings (see module docs).
    #[serde(default = "default_center_re")]
    pub center_re: String,
    #[serde(default = "default_center_im")]
    pub center_im: String,
    /// Zoom as a log2 exponent: 0 = the formula's home view (span 4),
    /// each +1 doubles magnification. f64 so a deep dive animates
    /// smoothly long past f32 mantissa granularity.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub zoom_log2: f64,
    /// View rotation in radians, matching the flame convention.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f32,

    /// Iteration ceiling per pixel.
    #[serde(default = "default_max_iter")]
    pub max_iter: u32,
    /// Escape radius squared for escaping formulas. Non-escaping and
    /// convergent formulas read their own thresholds from params.
    #[serde(default = "default_bailout")]
    pub bailout: f32,

    /// Pickover biomorph classification: test |Re z| / |Im z|
    /// separately instead of |z|. A switch on every formula, not a
    /// formula (see the plan §3).
    #[serde(default, skip_serializing_if = "BiomorphMode::is_default")]
    pub biomorph: BiomorphMode,

    /// Which coloring reads the orbit summary, by registry name.
    #[serde(default = "default_coloring")]
    pub coloring: String,

    /// Per-formula parameters, keyed `"param"` within the active
    /// formula's namespace (`power`, `variant`, …). Parameters of
    /// formulas that are not active are preserved, so switching
    /// formulas back and forth keeps each one's settings — the same
    /// courtesy `variation_params` extends.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub formula_params: BTreeMap<String, f32>,

    /// Per-coloring parameters, same shape.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub coloring_params: BTreeMap<String, f32>,
}

/// Biomorph classification axis (Pickover): which component escape is
/// tested on, per pixel, in addition to the formula's own test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiomorphMode {
    #[default]
    Off,
    /// Classify on |Re z| alone.
    Re,
    /// Classify on |Im z| alone.
    Im,
}

impl BiomorphMode {
    pub fn is_default(v: &BiomorphMode) -> bool {
        *v == BiomorphMode::Off
    }
}

fn default_formula() -> String {
    "mandelbrot".to_string()
}
fn default_coloring() -> String {
    "smooth".to_string()
}
fn default_center_re() -> String {
    // The Mandelbrot home view. A formula whose home differs recenters
    // via its def, not by fighting this default.
    "-0.5".to_string()
}
fn default_center_im() -> String {
    "0".to_string()
}
fn default_max_iter() -> u32 {
    256
}
fn default_bailout() -> f32 {
    4.0
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

impl Default for EscapeConfig {
    fn default() -> Self {
        Self {
            formula: default_formula(),
            julia: false,
            julia_re: 0.0,
            julia_im: 0.0,
            center_re: default_center_re(),
            center_im: default_center_im(),
            zoom_log2: 0.0,
            rotation: 0.0,
            max_iter: default_max_iter(),
            bailout: default_bailout(),
            biomorph: BiomorphMode::Off,
            coloring: default_coloring(),
            formula_params: BTreeMap::new(),
            coloring_params: BTreeMap::new(),
        }
    }
}

impl EscapeConfig {
    /// For `skip_serializing_if`: an untouched escape config writes
    /// nothing, keeping every existing `.fflame` byte-stable.
    pub fn is_default(v: &EscapeConfig) -> bool {
        *v == EscapeConfig::default()
    }

    /// The center parsed to f64 — the phase-1 precision ceiling. Falls
    /// back to the default center on an unparseable string rather than
    /// jumping to the origin (which would read as "my flame is gone").
    pub fn center_f64(&self) -> (f64, f64) {
        (
            self.center_re.trim().parse().unwrap_or(-0.5),
            self.center_im.trim().parse().unwrap_or(0.0),
        )
    }

    /// Magnification as a plain factor (2^zoom_log2).
    pub fn zoom_factor(&self) -> f64 {
        self.zoom_log2.exp2()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_default_and_serializes_to_nothing_inside_a_config() {
        assert!(EscapeConfig::is_default(&EscapeConfig::default()));

        // The whole point of skip-if-default: a config that never
        // touched escape mode must not mention it. Byte-stability of
        // existing .fflame files rides on this.
        let json = crate::config::FractalConfig::default().to_json().unwrap();
        assert!(
            !json.contains("escape"),
            "default config JSON must not carry an escape section:\n{json}"
        );
    }

    #[test]
    fn a_touched_config_round_trips_exactly() {
        let mut esc = EscapeConfig {
            formula: "burning_ship".into(),
            julia: true,
            julia_re: 0.285,
            julia_im: 0.01,
            center_re: "-1.7433419053321".into(),
            center_im: "0.0000907687489".into(),
            zoom_log2: 21.5,
            rotation: 0.3,
            max_iter: 2000,
            bailout: 16.0,
            biomorph: BiomorphMode::Re,
            coloring: "orbit_trap".into(),
            ..EscapeConfig::default()
        };
        esc.formula_params.insert("power".into(), 3.0);
        esc.coloring_params.insert("trap_radius".into(), 0.25);

        let json = serde_json::to_string(&esc).unwrap();
        let back: EscapeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(esc, back);

        // The center strings survive VERBATIM — they are the deep-zoom
        // payload, and any normalization (trim, float round-trip)
        // would destroy precision phase 4 depends on.
        assert!(json.contains("-1.7433419053321"));
    }

    #[test]
    fn center_parsing_is_forgiving_but_exact() {
        let mut esc = EscapeConfig::default();
        assert_eq!(esc.center_f64(), (-0.5, 0.0));

        esc.center_re = " 0.25 ".into();
        esc.center_im = "1e-3".into();
        assert_eq!(esc.center_f64(), (0.25, 1e-3));

        // Garbage falls back to home, not to (0, 0).
        esc.center_re = "not a number".into();
        assert_eq!(esc.center_f64().0, -0.5);
    }

    #[test]
    fn a_config_with_escape_settings_survives_fractal_config_round_trip() {
        let mut config = crate::config::FractalConfig::default();
        config.escape.zoom_log2 = 4.0;
        config.escape.formula_params.insert("power".into(), 4.0);

        let json = config.to_json().unwrap();
        assert!(json.contains("escape"), "touched escape must serialize");
        let back = crate::config::FractalConfig::from_json(&json).unwrap();
        assert_eq!(back.escape, config.escape);
    }
}
