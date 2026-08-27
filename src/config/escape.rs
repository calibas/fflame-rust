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

    /// Damped / Mann iteration (plan §3): `z ← (1−α)z + α·f(z)` with
    /// COMPLEX α. `1 + 0i` (the default) is plain iteration and
    /// compiles the wrap out entirely, keeping undamped shaders
    /// byte-identical; the published Mann/Ishikawa fractal families
    /// live at real α ∈ (0,1), the generalized-relaxation galleries
    /// at complex α.
    #[serde(default = "default_damping_re", skip_serializing_if = "is_one")]
    pub damping_re: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub damping_im: f32,

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

    /// Supersampling factor: the image renders at N× resolution per
    /// axis and box-downsamples (N² samples per display pixel).
    /// 1 = off. Part of the CONFIG (not a device preference) so a
    /// saved file reproduces exactly, everywhere — viewport, CLI,
    /// thumbnails alike.
    #[serde(default = "default_supersample", skip_serializing_if = "is_one_u32")]
    pub supersample: u32,

    /// Reference-orbit period hint (fraktaler-3's `reference.period`):
    /// for a location centered on a deep nucleus, the period of that
    /// nucleus. The renderer VERIFIES the center's orbit closes at
    /// this period before trusting it (a wrong hint falls back to
    /// plain references with a warning). None = detect/none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_period: Option<u32>,
}

fn default_supersample() -> u32 {
    1
}

fn is_one_u32(v: &u32) -> bool {
    *v == 1
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

/// The wire strings ConfigValue carries for [`BiomorphMode`] — one
/// place, so the manager's read and write arms cannot disagree.
pub fn biomorph_to_str(m: BiomorphMode) -> &'static str {
    match m {
        BiomorphMode::Off => "off",
        BiomorphMode::Re => "re",
        BiomorphMode::Im => "im",
    }
}

pub fn biomorph_from_str(s: &str) -> Option<BiomorphMode> {
    match s.trim().to_ascii_lowercase().as_str() {
        "off" => Some(BiomorphMode::Off),
        "re" => Some(BiomorphMode::Re),
        "im" => Some(BiomorphMode::Im),
        _ => None,
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
fn default_damping_re() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
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
            damping_re: default_damping_re(),
            damping_im: 0.0,
            biomorph: BiomorphMode::Off,
            coloring: default_coloring(),
            formula_params: BTreeMap::new(),
            coloring_params: BTreeMap::new(),
            supersample: 1,
            reference_period: None,
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

    /// Whether the Mann-iteration wrap is active (α ≠ 1). The
    /// assembler compiles the wrap in only when this is true.
    pub fn is_damped(&self) -> bool {
        self.damping_re != 1.0 || self.damping_im != 0.0
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

    /// Every escape path must survive the string-key round trip —
    /// that single property is what animation tracks, signals and
    /// `config.set` all resolve through.
    #[test]
    fn escape_paths_round_trip_their_string_keys() {
        use crate::config::delta::ConfigPath;
        let paths = [
            ConfigPath::EscapeFormula,
            ConfigPath::EscapeJulia,
            ConfigPath::EscapeJuliaRe,
            ConfigPath::EscapeJuliaIm,
            ConfigPath::EscapeCenterRe,
            ConfigPath::EscapeCenterIm,
            ConfigPath::EscapeZoomLog2,
            ConfigPath::EscapeRotation,
            ConfigPath::EscapeMaxIter,
            ConfigPath::EscapeBailout,
            ConfigPath::EscapeBiomorph,
            ConfigPath::EscapeColoring,
            ConfigPath::EscapeFormulaParam { param: "power".into() },
            ConfigPath::EscapeColoringParam { param: "trap_radius".into() },
        ];
        for p in paths {
            let key = p.to_string_key();
            assert_eq!(
                ConfigPath::from_string_key(&key).as_ref(),
                Some(&p),
                "`{key}` did not round-trip"
            );
            assert_eq!(
                p.update_type(),
                crate::config::delta::UpdateType::EscapeRerender,
                "{key}: every escape path re-renders the fragment frame"
            );
        }
    }

    /// Keyframe values resolve for the continuous parameters and refuse
    /// the structural/exact ones (formula, coloring, the deep-zoom
    /// center strings — the latter deliberately, per the plan).
    #[test]
    fn escape_animation_value_conversion() {
        use crate::config::delta::{json_to_config_value, ConfigPath, ConfigValue};
        let j = serde_json::json!(2.5);
        assert_eq!(
            json_to_config_value(&j, &ConfigPath::EscapeZoomLog2),
            Some(ConfigValue::Float(2.5))
        );
        assert_eq!(
            json_to_config_value(&serde_json::json!(512), &ConfigPath::EscapeMaxIter),
            Some(ConfigValue::UInt(512))
        );
        assert_eq!(
            json_to_config_value(&serde_json::json!(true), &ConfigPath::EscapeJulia),
            Some(ConfigValue::Bool(true))
        );
        assert_eq!(json_to_config_value(&serde_json::json!("0.1"), &ConfigPath::EscapeCenterRe), None);
        assert_eq!(
            json_to_config_value(&serde_json::json!("kaliset"), &ConfigPath::EscapeFormula),
            None
        );
    }

    /// The whole undo loop, through the same entry point the panel
    /// will use: update_param writes, reports EscapeRerender, and undo
    /// restores — including the keyed param map and the biomorph
    /// string form.
    #[test]
    fn escape_params_flow_through_config_manager_and_undo() {
        use crate::config::delta::{ConfigPath, ConfigValue, UpdateType};
        use crate::config::manager::ConfigManager;

        let mut mgr = ConfigManager::new(crate::config::FractalConfig::default());

        let ut = mgr
            .update_param(ConfigPath::EscapeZoomLog2, ConfigValue::Float(3.0))
            .unwrap();
        assert_eq!(ut, UpdateType::EscapeRerender);

        mgr.update_param(
            ConfigPath::EscapeFormulaParam { param: "power".into() },
            ConfigValue::Float(4.0),
        )
        .unwrap();
        mgr.update_param(
            ConfigPath::EscapeBiomorph,
            ConfigValue::String("re".into()),
        )
        .unwrap();
        // An unknown biomorph string is an error, not a silent default.
        assert!(mgr
            .update_param(ConfigPath::EscapeBiomorph, ConfigValue::String("sideways".into()))
            .is_err());

        assert_eq!(
            mgr.get_value(&ConfigPath::EscapeZoomLog2).unwrap(),
            ConfigValue::Float(3.0)
        );
        assert_eq!(
            mgr.get_value(&ConfigPath::EscapeFormulaParam { param: "power".into() }).unwrap(),
            ConfigValue::Float(4.0)
        );
        assert_eq!(
            mgr.get_value(&ConfigPath::EscapeBiomorph).unwrap(),
            ConfigValue::String("re".into())
        );

        // Undo restores, newest first.
        mgr.undo().unwrap();
        assert_eq!(
            mgr.get_value(&ConfigPath::EscapeBiomorph).unwrap(),
            ConfigValue::String("off".into())
        );
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
