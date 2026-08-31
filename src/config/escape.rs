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

    /// Relief shading — a LAYER over whatever the coloring produced,
    /// not a coloring of its own. Off by default and skipped when
    /// off, so every existing file is byte-stable.
    #[serde(default, skip_serializing_if = "EscapeShading::is_default")]
    pub shading: EscapeShading,
}

fn default_supersample() -> u32 {
    1
}

fn is_one_u32(v: &u32) -> bool {
    *v == 1
}

/// How a shading layer is composited over the colour beneath it.
///
/// Named for what they do to the base rather than for a formula, and
/// chosen to be the four that read differently on a fractal: darken
/// (`Multiply`), lighten (`Screen`), contrast-preserving (`Overlay`)
/// and flat tint (`Mix`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadingBlend {
    /// `base * layer` — the natural shadow: never brightens.
    #[default]
    Multiply,
    /// `1-(1-base)(1-layer)` — the natural highlight: never darkens.
    Screen,
    /// Multiply where the base is dark, screen where it is light.
    /// Keeps the palette's own contrast; the strongest "engraved" look.
    Overlay,
    /// Straight linear interpolation toward the layer colour. Flattens,
    /// but it is the one that shows a coloured light honestly.
    Mix,
}

impl ShadingBlend {
    /// The discriminant the WGSL switch reads.
    pub fn to_gpu(self) -> u32 {
        match self {
            ShadingBlend::Multiply => 0,
            ShadingBlend::Screen => 1,
            ShadingBlend::Overlay => 2,
            ShadingBlend::Mix => 3,
        }
    }

    pub fn all() -> [ShadingBlend; 4] {
        [
            ShadingBlend::Multiply,
            ShadingBlend::Screen,
            ShadingBlend::Overlay,
            ShadingBlend::Mix,
        ]
    }
}

/// The wire strings ConfigValue carries for [`ShadingBlend`].
pub fn shading_blend_to_str(m: ShadingBlend) -> &'static str {
    match m {
        ShadingBlend::Multiply => "multiply",
        ShadingBlend::Screen => "screen",
        ShadingBlend::Overlay => "overlay",
        ShadingBlend::Mix => "mix",
    }
}

pub fn shading_blend_from_str(s: &str) -> ShadingBlend {
    match s {
        "screen" => ShadingBlend::Screen,
        "overlay" => ShadingBlend::Overlay,
        "mix" => ShadingBlend::Mix,
        _ => ShadingBlend::Multiply,
    }
}

/// Which field the relief is computed FROM.
///
/// Two genuinely different pictures, not two qualities of one. The
/// coloring's value is mapped to the palette through `fract`, so it
/// has a sawtooth at every band edge; taking the slope of the RAW
/// value ignores those and reads the underlying surface, while taking
/// the slope of the WRAPPED value treats each band edge as a cliff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadingField {
    /// The coloring's value before `fract` — smooth terrain relief.
    #[default]
    Smooth,
    /// The wrapped palette coordinate — every band becomes a step, for
    /// the engraved / contour-map look.
    Banded,
}

impl ShadingField {
    pub fn to_gpu(self) -> u32 {
        match self {
            ShadingField::Smooth => 0,
            ShadingField::Banded => 1,
        }
    }
}

/// The wire strings ConfigValue carries for [`ShadingField`].
pub fn shading_field_to_str(m: ShadingField) -> &'static str {
    match m {
        ShadingField::Smooth => "smooth",
        ShadingField::Banded => "banded",
    }
}

pub fn shading_field_from_str(s: &str) -> ShadingField {
    match s {
        "banded" => ShadingField::Banded,
        _ => ShadingField::Smooth,
    }
}

/// Relief shading: a lit-surface layer composited over the coloring.
///
/// Deliberately NOT a `ColoringDef`. A coloring returns one scalar
/// that the template maps through the palette, so colorings replace
/// each other by construction and could never decorate one another —
/// which is why `normal_map` (the analytic-normal coloring) takes over
/// the image instead of shading it. This runs after the palette
/// lookup, on the finished RGB, so it composes with every coloring and
/// every palette including `position_map` on the folding formulas.
///
/// The surface comes from the SLOPE of the coloring's own value field,
/// finite-differenced at render resolution. That is what makes it
/// universal: it needs no derivative, so it works on the perturbed
/// rungs and on the 13 of 25 formulas that define none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscapeShading {
    #[serde(default, skip_serializing_if = "is_false")]
    pub enabled: bool,

    /// Light azimuth in degrees, counter-clockwise from +x (east).
    /// 315 (north-west) is the cartographic convention and the
    /// default, because relief lit from any other quadrant reads as
    /// inverted to most people.
    #[serde(default = "default_light_angle")]
    pub light_angle: f32,
    /// Vertical exaggeration of the slope before lighting. This is the
    /// only control whose useful range depends on the coloring: an
    /// escape count climbs by ~1 per pixel near the boundary, a
    /// bounded coloring by ~1e-3.
    #[serde(default = "default_relief_height")]
    pub height: f32,
    /// Which field the slope is taken from.
    #[serde(default, skip_serializing_if = "ShadingField::is_default")]
    pub field: ShadingField,

    /// Colour applied where the surface faces away from the light.
    #[serde(default = "default_shadow_color")]
    pub shadow_color: [f32; 3],
    /// 0..4. Past 1 the layer saturates sooner rather than going
    /// further — which is the point on a DARK image, where a pixel
    /// sits close to black and the same `amt` moves it far less
    /// toward black than toward white. That gap is dynamic range, not
    /// a blend bug, and this is the control that compensates for it.
    #[serde(default = "default_shadow_strength")]
    pub shadow_strength: f32,
    #[serde(default, skip_serializing_if = "ShadingBlend::is_multiply")]
    pub shadow_blend: ShadingBlend,

    /// Colour applied where it faces into the light.
    #[serde(default = "default_highlight_color")]
    pub highlight_color: [f32; 3],
    #[serde(default = "default_highlight_strength")]
    pub highlight_strength: f32,
    #[serde(default = "default_highlight_blend")]
    pub highlight_blend: ShadingBlend,

    /// Gaussian sigma, in DISPLAY pixels, of the low-pass applied to
    /// the height field before its slope is taken. 0 = no softening.
    ///
    /// A ±1 central difference is the sharpest derivative estimate
    /// there is: it responds to every single-pixel wobble, which on a
    /// finely-detailed coloring reads as crunchy. Blurring the HEIGHT
    /// (not the image) softens the relief while leaving the colour
    /// beneath it untouched.
    ///
    /// Continuous, not a pixel count: this is the width of a
    /// Gaussian, so 0.5 and 0.8 differ. An earlier version rounded it
    /// to an integer stencil radius, which made the control coarse
    /// AND — because it sampled only a ring — did not blur at all.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub softness: f32,

    /// Surface texture: a micro-relief lit by the same light, so the
    /// surface reads as grainy or fibrous rather than glassy.
    #[serde(default, skip_serializing_if = "is_texture_none")]
    pub texture_kind: ShadingTexture,
    /// How pronounced the texture is. 0 = off.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub texture_strength: f32,
    /// Feature size in DISPLAY pixels — how coarse the grain is.
    #[serde(default = "default_texture_scale")]
    pub texture_scale: f32,
}

fn default_light_angle() -> f32 {
    315.0
}
fn default_relief_height() -> f32 {
    // The slope is measured in palette turns per DISPLAY pixel, which
    // is already a normalized unit -- every coloring's value is in
    // turns by construction, since that is what the palette cycles on.
    // What differs between colorings is how many turns they spend
    // across a view, so this is a starting point rather than a
    // universal: 10 puts both `smooth` on the Mandelbrot (0.04
    // turns/px) and `position_map` on Origami into visible relief.
    10.0
}
fn default_shadow_color() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}
fn default_shadow_strength() -> f32 {
    0.6
}
fn default_highlight_color() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}
fn default_highlight_strength() -> f32 {
    0.5
}
fn default_highlight_blend() -> ShadingBlend {
    ShadingBlend::Screen
}

impl ShadingBlend {
    fn is_multiply(v: &ShadingBlend) -> bool {
        *v == ShadingBlend::Multiply
    }
}

impl ShadingField {
    fn is_default(v: &ShadingField) -> bool {
        *v == ShadingField::Smooth
    }
}

impl Default for EscapeShading {
    fn default() -> Self {
        Self {
            enabled: false,
            light_angle: default_light_angle(),
            height: default_relief_height(),
            field: ShadingField::default(),
            shadow_color: default_shadow_color(),
            shadow_strength: default_shadow_strength(),
            shadow_blend: ShadingBlend::Multiply,
            highlight_color: default_highlight_color(),
            highlight_strength: default_highlight_strength(),
            highlight_blend: default_highlight_blend(),
            softness: 0.0,
            texture_kind: ShadingTexture::None,
            texture_strength: 0.0,
            texture_scale: default_texture_scale(),
        }
    }
}

impl EscapeShading {
    pub fn is_default(v: &EscapeShading) -> bool {
        *v == EscapeShading::default()
    }
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
/// Which surface texture the relief carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ShadingTexture {
    #[default]
    None,
    /// One octave of isotropic value noise — film grain, fine tooth.
    Grain,
    /// Octaves stretched along different axes, so it reads as fibre
    /// laid in a felt rather than as isotropic speckle.
    Paper,
}

impl ShadingTexture {
    pub fn to_gpu(self) -> u32 {
        match self {
            ShadingTexture::None => 0,
            ShadingTexture::Grain => 1,
            ShadingTexture::Paper => 2,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ShadingTexture::None => "none",
            ShadingTexture::Grain => "grain",
            ShadingTexture::Paper => "paper",
        }
    }
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "grain" => ShadingTexture::Grain,
            "paper" => ShadingTexture::Paper,
            _ => ShadingTexture::None,
        }
    }
}

fn default_texture_scale() -> f32 {
    2.0
}

fn is_texture_none(v: &ShadingTexture) -> bool {
    *v == ShadingTexture::None
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
            shading: EscapeShading::default(),
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
mod shading_tests {
    use super::*;

    /// An untouched shading block must not appear in the JSON at all.
    ///
    /// Every `.fflame` ever saved predates this feature, and the
    /// project's rule is that new fields are skip-if-default so old
    /// files stay byte-stable. This is the test that keeps the
    /// `skip_serializing_if` from being dropped in a later tidy-up —
    /// which would silently rewrite every config the first time it was
    /// re-saved.
    #[test]
    fn default_shading_serializes_to_nothing() {
        let esc = EscapeConfig::default();
        let json = serde_json::to_string(&esc).unwrap();
        assert!(
            !json.contains("shading"),
            "default shading was written to the config: {json}"
        );
        // And a config with no shading key must load with the defaults
        // rather than failing.
        let back: EscapeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.shading, EscapeShading::default());
        assert!(!back.shading.enabled);
    }

    /// Everything the layer can be set to must survive a round-trip.
    #[test]
    fn shading_round_trips_through_json() {
        let mut esc = EscapeConfig::default();
        esc.shading = EscapeShading {
            enabled: true,
            light_angle: 42.5,
            height: 37.0,
            field: ShadingField::Banded,
            shadow_color: [0.1, 0.2, 0.3],
            shadow_strength: 0.25,
            shadow_blend: ShadingBlend::Overlay,
            highlight_color: [0.9, 0.8, 0.7],
            highlight_strength: 0.75,
            highlight_blend: ShadingBlend::Mix,
            softness: 3.0,
            texture_kind: ShadingTexture::Paper,
            texture_strength: 0.6,
            texture_scale: 3.5,
        };
        let json = serde_json::to_string(&esc).unwrap();
        let back: EscapeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.shading, esc.shading);
    }

    /// The wire strings are the config's public surface (scripting,
    /// the API blob, saved files), so every enum value must survive
    /// the string round-trip its ConfigValue arm uses. A new variant
    /// added without a `from_str` arm would silently read back as the
    /// default and be very hard to spot.
    #[test]
    fn every_blend_and_field_survives_its_wire_string() {
        for b in ShadingBlend::all() {
            assert_eq!(shading_blend_from_str(shading_blend_to_str(b)), b);
        }
        for f in [ShadingField::Smooth, ShadingField::Banded] {
            assert_eq!(shading_field_from_str(shading_field_to_str(f)), f);
        }
        // The GPU discriminants must be distinct, or two blend modes
        // would render identically.
        let mut seen = std::collections::HashSet::new();
        for b in ShadingBlend::all() {
            assert!(seen.insert(b.to_gpu()), "duplicate GPU discriminant for {b:?}");
        }
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
