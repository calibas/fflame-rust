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

/// The most antialiasing an escape render will attempt, as a factor
/// per axis.
///
/// Lives with the CONFIG rather than the renderer because clamping a
/// saved value is a config concern, and a build without the escape
/// engine still has to parse and bound an escape config — the mode
/// round-trips so a file is never silently rewritten. Re-exported from
/// `escape::renderer` for the code that thinks of it as a renderer
/// limit.
///
/// What the grid cannot actually hold at a given size is made up by
/// ACCUMULATION rather than refused (see `EscapeRenderer::sample_grid`),
/// so this is the ceiling on what a user can ask for, not on what a
/// device can do.
pub const MAX_SUPERSAMPLE: u32 = 8;
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
    ///
    /// A new config starts at `default_bailout` (1e4); a file without the
    /// key means 4, the default it was written under, so it renders as
    /// it did. The app always writes the key.
    #[serde(default = "legacy_bailout")]
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

    /// The **camera lens**: a variation applied to the normalised
    /// screen offset before the view scale, by name. Empty is no lens.
    ///
    /// This warps the VIEW, not the fractal: the formula is untouched
    /// and only which point each pixel samples changes, which is the
    /// opposite direction from a flame's final transform. See
    /// `docs/projects/escape-camera-lens.md`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lens: String,

    /// The lens variation's parameters, keyed by name inside that
    /// variation's namespace -- the same shape as `formula_params`,
    /// and preserved across a change of lens for the same reason.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lens_params: BTreeMap<String, f32>,

    /// How much of the lens to apply: `n` at 0, `L(n)` at 1, and
    /// `n + amount * (L(n) - n)` in general -- so past 1 it overshoots
    /// and below 0 it runs backwards.
    ///
    /// Without it a lens is all-or-nothing, and most of them are far
    /// too strong at full strength to be a camera effect rather than
    /// a subject. The simulation's layer warp reached for the same
    /// control (`mix(q, mapped, rate)`) for the same reason.
    ///
    /// The negative half is not symmetry for its own sake. A lens
    /// either magnifies the middle or shrinks it, and which one a
    /// given variation does is a property of that variation; running
    /// the displacement backwards turns any of them around, which is
    /// the first-order inverse of the map.
    #[serde(default = "default_lens_amount", skip_serializing_if = "is_one")]
    pub lens_amount: f32,


    /// Where a solid render looks: the point the camera orbits and
    /// approaches, as exact decimal strings.
    ///
    /// Strings for the same reason the 2D centre is one. A deep zoom
    /// is an approach to a POINT, and the eye's distance shrinks with
    /// `zoom_log2` while the target holds still — so the target is the
    /// quantity that needs digits, and an `f32` camera position would
    /// cap 3D at a zoom the plane passed long ago (D8).
    ///
    /// Empty means "the attractor's own centre", which is what frames
    /// a flame you have just switched to without being told where it
    /// is. The moment the camera is moved they become explicit.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cam_target_x: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cam_target_y: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cam_target_z: String,

    /// Elevation above the target's horizon, radians.
    ///
    /// The camera's frame is the flame's 4-angle chain
    /// (`Rz(rotation)·Rx(pitch)·Ry(bank)·Rz(−yaw)`, see
    /// `solid_camera`), in which this and `cam_yaw` are re-expressed
    /// so that zero here is the horizon and not the flame's top-down
    /// view. The poles are ordinary: the chain always has an up.
    #[serde(default = "default_cam_pitch", skip_serializing_if = "is_default_cam_pitch")]
    pub cam_pitch: f32,
    /// Rotation about the target, radians.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cam_yaw: f32,
    /// The third angle, radians: the flame camera's bank, in the same
    /// slot of the same chain. The screen roll is `rotation`, shared
    /// with the plane.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cam_bank: f32,
    /// Vertical field of view, radians -- the frame's height; the
    /// width follows the aspect.
    #[serde(default = "default_cam_fov", skip_serializing_if = "is_default_cam_fov")]
    pub cam_fov: f32,

    /// Supersampling factor: the image renders at N× resolution per
    /// axis and box-downsamples (N² samples per display pixel).
    /// 1 = off. Part of the CONFIG (not a device preference) so a
    /// saved file reproduces exactly, everywhere — viewport, CLI,
    /// thumbnails alike.
    #[serde(default = "default_supersample", skip_serializing_if = "is_one_u32")]
    pub supersample: u32,

    /// How the supersampled grid is combined back into one pixel.
    ///
    /// The default is a plain average in LINEAR light, which is what
    /// a camera does and is right for luminance. It is also why fine
    /// coloured filaments read as washed out: a saturated line
    /// covering one sample in nine is averaged with eight neighbours,
    /// and correct dilution looks like lost colour. The other modes
    /// trade physical correctness for keeping that colour.
    #[serde(default, skip_serializing_if = "is_downsample_default")]
    pub downsample: DownsampleMode,

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

    /// Auto-exposure for the coloring's value field. Off by default
    /// and skipped when off, so every existing file is byte-stable.
    #[serde(default, skip_serializing_if = "EscapeContrast::is_default")]
    pub contrast: EscapeContrast,

    /// How the coloring's value becomes a palette colour: a transfer
    /// curve on the value, a curve on each palette cycle, and stepped
    /// bands. The default is the identity and is skipped, so every
    /// existing file is byte-stable.
    #[serde(default, skip_serializing_if = "PaletteMap::is_default")]
    pub palette_map: PaletteMap,

    /// A second colouring blended into the first before the palette
    /// lookup (`docs/projects/escape-coloring-survey.md`, item 5). No
    /// colouring is no layer, which is the default and is skipped.
    #[serde(default, skip_serializing_if = "ColoringLayer::is_default")]
    pub layer: ColoringLayer,
}

/// Looking down at about 24°, which shows a solid's top and one
/// side rather than a silhouette.
fn default_cam_pitch() -> f32 {
    0.42
}

fn is_default_cam_pitch(v: &f32) -> bool {
    (*v - default_cam_pitch()).abs() < f32::EPSILON
}

/// About 40°, which is a normal lens rather than a dramatic one.
fn default_cam_fov() -> f32 {
    0.7
}

fn is_default_cam_fov(v: &f32) -> bool {
    (*v - default_cam_fov()).abs() < f32::EPSILON
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
    /// The W3C soft light: a gentle multiply below the middle, a gentle
    /// screen above it. Ultra Fractal recommends it (and Hard Light) for
    /// lighting layers, which land on mid-grey where the ground is flat.
    SoftLight,
    /// Overlay with the roles swapped: the light colour decides between
    /// multiply and screen, so the light reads harder than the base.
    HardLight,
}

impl ShadingBlend {
    /// The discriminant the WGSL switch reads.
    pub fn to_gpu(self) -> u32 {
        match self {
            ShadingBlend::Multiply => 0,
            ShadingBlend::Screen => 1,
            ShadingBlend::Overlay => 2,
            ShadingBlend::Mix => 3,
            ShadingBlend::SoftLight => 4,
            ShadingBlend::HardLight => 5,
        }
    }

    pub fn all() -> [ShadingBlend; 6] {
        [
            ShadingBlend::Multiply,
            ShadingBlend::Screen,
            ShadingBlend::Overlay,
            ShadingBlend::Mix,
            ShadingBlend::SoftLight,
            ShadingBlend::HardLight,
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
        ShadingBlend::SoftLight => "soft_light",
        ShadingBlend::HardLight => "hard_light",
    }
}

pub fn shading_blend_from_str(s: &str) -> ShadingBlend {
    match s {
        "screen" => ShadingBlend::Screen,
        "overlay" => ShadingBlend::Overlay,
        "mix" => ShadingBlend::Mix,
        "soft_light" => ShadingBlend::SoftLight,
        "hard_light" => ShadingBlend::HardLight,
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
    /// The texture layer's value: relief from one field, colour from
    /// another (survey R3). Without a layer, the colouring's own.
    Layer,
}

impl ShadingField {
    pub fn to_gpu(self) -> u32 {
        match self {
            ShadingField::Smooth => 0,
            ShadingField::Banded => 1,
            ShadingField::Layer => 2,
        }
    }
}

/// The wire strings ConfigValue carries for [`ShadingField`].
pub fn shading_field_to_str(m: ShadingField) -> &'static str {
    match m {
        ShadingField::Smooth => "smooth",
        ShadingField::Banded => "banded",
        ShadingField::Layer => "layer",
    }
}

pub fn shading_field_from_str(s: &str) -> ShadingField {
    match s {
        "banded" => ShadingField::Banded,
        "layer" => ShadingField::Layer,
        _ => ShadingField::Smooth,
    }
}

/// How the coloring's value range is fitted to the palette.
///
/// The problem this exists for: an orbit-STATISTIC coloring
/// (`magnitude_average` and kin) is a SMOOTH function of c, and a
/// smooth function restricted to a shrinking window converges to its
/// own first-order Taylor expansion. Measured on Ducks: fit a plane to
/// the field and it explains 1.0000 of the variance at zoom 14 AND at
/// zoom 26.6 -- the field IS a plane -- while its spread falls from
/// 7.2e-5 to 1.2e-8. Through a cyclic palette a plane is a set of
/// parallel bands, so panning rotates them and eventually nothing
/// moves at all. Raising `max_iter` does not recover it.
///
/// An escaping fractal never has this trouble: its escape count is a
/// discontinuous integer, so it stays contrasty at any depth. This is
/// the price of the non-escaping families, and it is a COLOURING
/// problem, not a precision one -- the render agrees with exact orbits
/// to ~1e-7 the whole way down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContrastMode {
    /// The coloring's own `scale`/`offset` and nothing else.
    #[default]
    Off,
    /// Map the field's measured range onto one palette turn. Turns an
    /// invisible ramp into a full-range one -- still a ramp, but a
    /// visible one, and it stops the exposure drifting as you zoom.
    AutoRange,
    /// Subtract the fitted PLANE, then range the residual. This is the
    /// one that shows structure a plane was hiding; where the field is
    /// a perfect plane it correctly shows almost nothing, because
    /// there is nothing left.
    Flatten,
}

impl ContrastMode {
    pub fn to_gpu(self) -> u32 {
        match self {
            ContrastMode::Off => 0,
            ContrastMode::AutoRange => 1,
            ContrastMode::Flatten => 2,
        }
    }
    pub fn is_off(&self) -> bool {
        *self == ContrastMode::Off
    }
}

/// The wire strings ConfigValue carries for [`ContrastMode`].
pub fn contrast_mode_to_str(m: ContrastMode) -> &'static str {
    match m {
        ContrastMode::Off => "off",
        ContrastMode::AutoRange => "auto_range",
        ContrastMode::Flatten => "flatten",
    }
}

pub fn contrast_mode_from_str(s: &str) -> ContrastMode {
    match s {
        "auto_range" => ContrastMode::AutoRange,
        "flatten" => ContrastMode::Flatten,
        _ => ContrastMode::Off,
    }
}

/// Auto-exposure for the coloring's value field.
///
/// Measured from the rendered field itself once it settles, then
/// applied in the recolor pass -- so it costs one cheap dispatch and
/// nothing during iteration. See [`ContrastMode`] for why it exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscapeContrast {
    #[serde(default, skip_serializing_if = "ContrastMode::is_off")]
    pub mode: ContrastMode,
    /// Fraction trimmed from EACH end before ranging, 0..0.25.
    ///
    /// A percentile rather than the raw min/max: one pixel sitting on
    /// a singularity (Ducks' `log` guard returns -34.5) would
    /// otherwise set the whole scale and flatten everything else back
    /// out. Measured on the reported view, the guard value is ~5
    /// orders of magnitude outside the rest of the field.
    #[serde(default = "default_contrast_clip")]
    pub clip: f32,
    /// How far to travel from the coloring's own mapping toward the
    /// fitted one. 1 is the fitted mapping; 0 is off with the
    /// measurement still running, which makes the control continuous.
    #[serde(default = "default_contrast_strength")]
    pub strength: f32,
    /// Palette turns the fitted range is mapped onto. 1 gives exactly
    /// one turn (no repeats); higher values cycle the palette that
    /// many times across the field, which is how these colorings are
    /// usually driven.
    #[serde(default = "default_contrast_turns")]
    pub turns: f32,
}

fn default_contrast_clip() -> f32 {
    0.005
}
fn default_contrast_strength() -> f32 {
    1.0
}
fn default_contrast_turns() -> f32 {
    1.0
}

impl Default for EscapeContrast {
    fn default() -> Self {
        Self {
            mode: ContrastMode::default(),
            clip: default_contrast_clip(),
            strength: default_contrast_strength(),
            turns: default_contrast_turns(),
        }
    }
}

impl EscapeContrast {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Whether the renderer must measure the field at all.
    pub fn is_active(&self) -> bool {
        !self.mode.is_off() && self.strength > 0.0
    }
}

/// A curve on the coloring's value, applied before the palette wraps
/// it (`docs/projects/escape-coloring-survey.md` P1).
///
/// Every curve is `g(v) = k f(v/k)` with `f(1) = 1`, where `k` is
/// [`PaletteMap::pivot`]: the value every curve leaves where Linear
/// would. Below it the root and log curves stretch the value, above it
/// they compress it, so palette cycles crowd or spread across the
/// picture. Negative values are mirrored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferCurve {
    #[default]
    Linear,
    SquareRoot,
    CubeRoot,
    /// `log2(1 + u)`, techmatt's.
    Log,
    /// `ln(1 + ln(1 + u)) / ln(1 + ln 2)`, KF2's: flatter still.
    LogLog,
    Square,
    /// `atan(u) / atan(1)`: bounded, so however far the value runs the
    /// palette cycles at most twice the pivot.
    ArcTan,
}

impl TransferCurve {
    pub const ALL: [TransferCurve; 7] = [
        TransferCurve::Linear,
        TransferCurve::SquareRoot,
        TransferCurve::CubeRoot,
        TransferCurve::Log,
        TransferCurve::LogLog,
        TransferCurve::Square,
        TransferCurve::ArcTan,
    ];
    /// The shader's code for it (`esc_transfer`).
    pub fn to_gpu(self) -> u32 {
        match self {
            TransferCurve::Linear => 0,
            TransferCurve::SquareRoot => 1,
            TransferCurve::CubeRoot => 2,
            TransferCurve::Log => 3,
            TransferCurve::LogLog => 4,
            TransferCurve::Square => 5,
            TransferCurve::ArcTan => 6,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            TransferCurve::Linear => "linear",
            TransferCurve::SquareRoot => "square_root",
            TransferCurve::CubeRoot => "cube_root",
            TransferCurve::Log => "log",
            TransferCurve::LogLog => "log_log",
            TransferCurve::Square => "square",
            TransferCurve::ArcTan => "arc_tan",
        }
    }
    /// An unknown name is Linear, as an unknown contrast mode is Off.
    pub fn from_name(s: &str) -> Self {
        Self::ALL.into_iter().find(|c| c.as_str() == s).unwrap_or_default()
    }
    /// `f(u)` for `u >= 0`, the shape before the pivot scales it. The
    /// shader's `esc_transfer` is the same arithmetic in f32.
    pub fn shape(self, u: f32) -> f32 {
        match self {
            TransferCurve::Linear => u,
            TransferCurve::SquareRoot => u.sqrt(),
            TransferCurve::CubeRoot => u.cbrt(),
            TransferCurve::Log => (1.0 + u).log2(),
            TransferCurve::LogLog => {
                (1.0 + (1.0 + u).ln()).ln() / (1.0 + std::f32::consts::LN_2).ln()
            }
            TransferCurve::Square => u * u,
            TransferCurve::ArcTan => u.atan() / std::f32::consts::FRAC_PI_4,
        }
    }
}

/// A curve on the position within one palette cycle, applied after the
/// wrap: it changes which colours a cycle dwells on, not how many
/// cycles there are. The escape-side counterpart of the Colors panel's
/// Log Redistribute, computed in the shader so the table keeps its
/// resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteCurve {
    #[default]
    Linear,
    SquareRoot,
    Square,
    /// `log2(1 + t)`.
    Log,
    /// `2^t - 1`, Log's mirror.
    Exp,
    /// Smoothstep: lingers at both ends.
    SCurve,
    /// Smoothstep's inverse: lingers in the middle.
    InverseS,
}

impl PaletteCurve {
    pub const ALL: [PaletteCurve; 7] = [
        PaletteCurve::Linear,
        PaletteCurve::SquareRoot,
        PaletteCurve::Square,
        PaletteCurve::Log,
        PaletteCurve::Exp,
        PaletteCurve::SCurve,
        PaletteCurve::InverseS,
    ];
    /// The shader's code for it (`esc_palette_curve`).
    pub fn to_gpu(self) -> u32 {
        match self {
            PaletteCurve::Linear => 0,
            PaletteCurve::SquareRoot => 1,
            PaletteCurve::Square => 2,
            PaletteCurve::Log => 3,
            PaletteCurve::Exp => 4,
            PaletteCurve::SCurve => 5,
            PaletteCurve::InverseS => 6,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PaletteCurve::Linear => "linear",
            PaletteCurve::SquareRoot => "square_root",
            PaletteCurve::Square => "square",
            PaletteCurve::Log => "log",
            PaletteCurve::Exp => "exp",
            PaletteCurve::SCurve => "s_curve",
            PaletteCurve::InverseS => "inverse_s",
        }
    }
    pub fn from_name(s: &str) -> Self {
        Self::ALL.into_iter().find(|c| c.as_str() == s).unwrap_or_default()
    }
    /// The curve on `t` in 0..1; the shader's `esc_palette_curve`.
    pub fn apply(self, t: f32) -> f32 {
        match self {
            PaletteCurve::Linear => t,
            PaletteCurve::SquareRoot => t.sqrt(),
            PaletteCurve::Square => t * t,
            PaletteCurve::Log => (1.0 + t).log2(),
            PaletteCurve::Exp => t.exp2() - 1.0,
            PaletteCurve::SCurve => t * t * (3.0 - 2.0 * t),
            PaletteCurve::InverseS => {
                0.5 - ((1.0 - 2.0 * t).clamp(-1.0, 1.0).asin() / 3.0).sin()
            }
        }
    }
}

/// How the coloring's value becomes a palette colour. See
/// [`EscapeConfig::palette_map`] and the survey's section 7, item 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteMap {
    #[serde(default, skip_serializing_if = "is_linear_transfer")]
    pub transfer: TransferCurve,
    /// The value the transfer leaves unchanged (see [`TransferCurve`]).
    #[serde(default = "default_pivot", skip_serializing_if = "is_one")]
    pub pivot: f32,
    #[serde(default, skip_serializing_if = "is_linear_curve")]
    pub curve: PaletteCurve,
    /// Each palette stop as a flat band from its position to the next
    /// stop, instead of a blend between them.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stepped: bool,
}

fn default_pivot() -> f32 {
    1.0
}
fn is_linear_transfer(c: &TransferCurve) -> bool {
    *c == TransferCurve::Linear
}
fn is_linear_curve(c: &PaletteCurve) -> bool {
    *c == PaletteCurve::Linear
}

/// The pivot's range: wide enough for a raw smooth count (thousands)
/// and a fitted 0..1 field alike.
pub const PIVOT_RANGE: (f32, f32) = (1.0e-3, 1.0e4);

impl Default for PaletteMap {
    fn default() -> Self {
        Self {
            transfer: TransferCurve::Linear,
            pivot: default_pivot(),
            curve: PaletteCurve::Linear,
            stepped: false,
        }
    }
}

impl PaletteMap {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// The two curve codes in one word: transfer in bits 0-7, palette
    /// curve in bits 8-15.
    pub fn gpu_flags(&self) -> u32 {
        self.transfer.to_gpu() | (self.curve.to_gpu() << 8)
    }
    /// The pivot as the shader reads it: clamped into range, so a
    /// hand-edited 0 cannot divide by zero.
    pub fn gpu_pivot(&self) -> f32 {
        self.pivot.clamp(PIVOT_RANGE.0, PIVOT_RANGE.1)
    }
    /// `g(v)`: the transfer at this pivot, mirrored for negative values.
    pub fn transfer_value(&self, v: f32) -> f32 {
        if self.transfer == TransferCurve::Linear {
            return v;
        }
        let k = self.gpu_pivot();
        v.signum() * k * self.transfer.shape(v.abs() / k)
    }
}

/// How a texture layer's palette position combines with the base's.
/// Each works on the two wrapped positions, `a` the base's and `b` the
/// layer's, and all but Add travel `weight` of the way from `a`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerBlend {
    /// `1 - (1 - a)(1 - b)`: brightens where the texture is high.
    /// techmatt's choice for stripes and curvature over smooth.
    #[default]
    Screen,
    /// `ab`: darkens where the texture is low.
    Multiply,
    /// `fract(a + weight * b)`: shifts the palette by the texture, so
    /// a cycling palette keeps cycling.
    Add,
    /// Multiply below the middle, Screen above: contrast.
    Overlay,
    /// `b`: the texture alone, faded in by the weight.
    Mix,
}

impl LayerBlend {
    pub const ALL: [LayerBlend; 5] = [
        LayerBlend::Screen,
        LayerBlend::Multiply,
        LayerBlend::Add,
        LayerBlend::Overlay,
        LayerBlend::Mix,
    ];
    /// The shader's code for it (`esc_layer`).
    pub fn to_gpu(self) -> u32 {
        match self {
            LayerBlend::Screen => 0,
            LayerBlend::Multiply => 1,
            LayerBlend::Add => 2,
            LayerBlend::Overlay => 3,
            LayerBlend::Mix => 4,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            LayerBlend::Screen => "screen",
            LayerBlend::Multiply => "multiply",
            LayerBlend::Add => "add",
            LayerBlend::Overlay => "overlay",
            LayerBlend::Mix => "mix",
        }
    }
    pub fn from_name(s: &str) -> Self {
        Self::ALL.into_iter().find(|b| b.as_str() == s).unwrap_or_default()
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// The blend of positions `a` (base) and `b` (layer) at `weight`;
    /// the shader's `esc_layer` is the same arithmetic in f32.
    pub fn apply(self, a: f32, b: f32, weight: f32) -> f32 {
        let blended = match self {
            LayerBlend::Screen => 1.0 - (1.0 - a) * (1.0 - b),
            LayerBlend::Multiply => a * b,
            LayerBlend::Add => return (a + weight * b).rem_euclid(1.0),
            LayerBlend::Overlay => {
                if a < 0.5 {
                    2.0 * a * b
                } else {
                    1.0 - 2.0 * (1.0 - a) * (1.0 - b)
                }
            }
            LayerBlend::Mix => b,
        };
        a + (blended - a) * weight
    }
}

/// A texture layer: a second escape colouring and how it blends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColoringLayer {
    /// A mode-A colouring, by registry name; empty is no layer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coloring: String,
    /// The layer colouring's parameters, keyed as `coloring_params`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "LayerBlend::is_default")]
    pub blend: LayerBlend,
    /// How far the blend travels from the base, 0..1.
    #[serde(default = "default_layer_weight", skip_serializing_if = "is_default_layer_weight")]
    pub weight: f32,
}

/// techmatt's weight for a Screen-blended texture over smooth.
fn default_layer_weight() -> f32 {
    0.85
}
fn is_default_layer_weight(v: &f32) -> bool {
    *v == default_layer_weight()
}

impl Default for ColoringLayer {
    fn default() -> Self {
        Self {
            coloring: String::new(),
            params: BTreeMap::new(),
            blend: LayerBlend::default(),
            weight: default_layer_weight(),
        }
    }
}

impl ColoringLayer {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Whether a layer is named at all (it may still be refused: see
    /// `escape::layer_of`).
    pub fn is_on(&self) -> bool {
        !self.coloring.is_empty()
    }
}

/// How the relief turns a slope into light and shade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliefModel {
    /// The signed tilt toward the light's azimuth: zero on flat
    /// ground, symmetric, monotonic in the slope (see `shade_pixel`).
    #[default]
    Tilt,
    /// Lambert's law with the light raised `elevation` above the
    /// horizon, as Ultra Fractal lights: flat ground reads as the light
    /// does there, and a slope facing away falls into shadow sooner the
    /// lower the light.
    Lambert,
}

impl ReliefModel {
    pub const ALL: [ReliefModel; 2] = [ReliefModel::Tilt, ReliefModel::Lambert];
    pub fn to_gpu(self) -> u32 {
        match self {
            ReliefModel::Tilt => 0,
            ReliefModel::Lambert => 1,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ReliefModel::Tilt => "tilt",
            ReliefModel::Lambert => "lambert",
        }
    }
    pub fn from_name(s: &str) -> Self {
        Self::ALL.into_iter().find(|m| m.as_str() == s).unwrap_or_default()
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// How the relief estimates the slope from neighbouring heights
/// (Kalles Fraktaler offers the same family).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlopeStencil {
    /// `(h(p+1) - h(p-1)) / 2`: the sharpest symmetric estimate.
    #[default]
    Central,
    /// `h(p+1) - h(p)`: half a pixel off-centre, and crisper for it.
    Forward,
    /// The two diagonal differences of a 2x2 block, turned back onto
    /// the axes: picks up diagonal detail the axes miss.
    Roberts,
    /// The plane fitted to the 3x3 neighbourhood by least squares: the
    /// smoothest of the four, and the least sensitive to one bad pixel.
    LeastSquares,
}

impl SlopeStencil {
    pub const ALL: [SlopeStencil; 4] = [
        SlopeStencil::Central,
        SlopeStencil::Forward,
        SlopeStencil::Roberts,
        SlopeStencil::LeastSquares,
    ];
    pub fn to_gpu(self) -> u32 {
        match self {
            SlopeStencil::Central => 0,
            SlopeStencil::Forward => 1,
            SlopeStencil::Roberts => 2,
            SlopeStencil::LeastSquares => 3,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            SlopeStencil::Central => "central",
            SlopeStencil::Forward => "forward",
            SlopeStencil::Roberts => "roberts",
            SlopeStencil::LeastSquares => "least_squares",
        }
    }
    pub fn from_name(s: &str) -> Self {
        Self::ALL.into_iter().find(|m| m.as_str() == s).unwrap_or_default()
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A curve on the height before its slope is taken (Ultra Fractal's
/// Slope height transfers): `post * f(pre * h)`, mirrored for negative
/// heights where `f` is not odd already. Sine and cosine ripple the
/// surface into terraces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeightTransfer {
    #[default]
    Linear,
    Log,
    SquareRoot,
    CubeRoot,
    Square,
    Cube,
    Sin,
    Cos,
}

impl HeightTransfer {
    pub const ALL: [HeightTransfer; 8] = [
        HeightTransfer::Linear,
        HeightTransfer::Log,
        HeightTransfer::SquareRoot,
        HeightTransfer::CubeRoot,
        HeightTransfer::Square,
        HeightTransfer::Cube,
        HeightTransfer::Sin,
        HeightTransfer::Cos,
    ];
    pub fn to_gpu(self) -> u32 {
        match self {
            HeightTransfer::Linear => 0,
            HeightTransfer::Log => 1,
            HeightTransfer::SquareRoot => 2,
            HeightTransfer::CubeRoot => 3,
            HeightTransfer::Square => 4,
            HeightTransfer::Cube => 5,
            HeightTransfer::Sin => 6,
            HeightTransfer::Cos => 7,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            HeightTransfer::Linear => "linear",
            HeightTransfer::Log => "log",
            HeightTransfer::SquareRoot => "square_root",
            HeightTransfer::CubeRoot => "cube_root",
            HeightTransfer::Square => "square",
            HeightTransfer::Cube => "cube",
            HeightTransfer::Sin => "sin",
            HeightTransfer::Cos => "cos",
        }
    }
    pub fn from_name(s: &str) -> Self {
        Self::ALL.into_iter().find(|m| m.as_str() == s).unwrap_or_default()
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// `f(x)`; the relief shader's `relief_height_curve` is the same in
    /// f32.
    pub fn apply(self, x: f32) -> f32 {
        match self {
            HeightTransfer::Linear => x,
            HeightTransfer::Log => x.signum() * x.abs().ln_1p(),
            HeightTransfer::SquareRoot => x.signum() * x.abs().sqrt(),
            HeightTransfer::CubeRoot => x.cbrt(),
            HeightTransfer::Square => x.signum() * x * x,
            HeightTransfer::Cube => x * x * x,
            HeightTransfer::Sin => x.sin(),
            HeightTransfer::Cos => x.cos(),
        }
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
/// rungs and on the 14 of 26 formulas that define none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscapeShading {
    #[serde(default, skip_serializing_if = "is_false")]
    pub enabled: bool,

    /// Light azimuth in degrees, counter-clockwise from +x (east): 90 is
    /// up, 180 west. A new config starts at 135, the upper left -- the
    /// cartographic convention, because relief lit from any other
    /// quadrant reads as inverted to most people. A file without the key
    /// means 315, the lower right, the default it was written under (the
    /// old comment called 315 north-west; counter-clockwise from east it
    /// is south-east).
    #[serde(default = "legacy_light_angle")]
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

    /// How a slope becomes light: the signed tilt (the default, and
    /// every picture before this option) or Lambert's law.
    #[serde(default, skip_serializing_if = "ReliefModel::is_default")]
    pub model: ReliefModel,
    /// The light's height above the horizon in degrees, for Lambert;
    /// Ultra Fractal's default is 30.
    #[serde(default = "default_relief_elevation", skip_serializing_if = "is_default_relief_elevation")]
    pub elevation: f32,
    /// A floor under the shadow, 0..1: how much of the base survives on
    /// the side facing away from the light.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ambient: f32,
    /// How the slope is estimated from neighbouring heights.
    #[serde(default, skip_serializing_if = "SlopeStencil::is_default")]
    pub stencil: SlopeStencil,
    /// A curve on the height before its slope is taken.
    #[serde(default, skip_serializing_if = "HeightTransfer::is_default")]
    pub height_curve: HeightTransfer,
    /// Scale applied to the height before the curve.
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub height_pre: f32,
    /// Scale applied after it.
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub height_post: f32,
}

fn default_relief_elevation() -> f32 {
    30.0
}
fn is_default_relief_elevation(v: &f32) -> bool {
    *v == default_relief_elevation()
}
fn default_one() -> f32 {
    1.0
}

fn default_light_angle() -> f32 {
    135.0
}
/// The light of a file that does not name one: the default until
/// 2026-10-02.
fn legacy_light_angle() -> f32 {
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
            model: ReliefModel::default(),
            elevation: default_relief_elevation(),
            ambient: 0.0,
            stencil: SlopeStencil::default(),
            height_curve: HeightTransfer::default(),
            height_pre: 1.0,
            height_post: 1.0,
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

/// How far [`EscapeConfig::lens_amount`] may be pushed either way.
///
/// Positive bulges the middle out and negative pinches it in; past 1
/// either way the lens overshoots rather than blends.
///
/// Here rather than beside the lens engine because the config module
/// is compiled whether or not `engine-escape` is, and the clamp on
/// the write path lives in the config manager. `escape::lens`
/// re-exports it.
pub const LENS_AMOUNT_LIMIT: f32 = 5.0;

/// A lens at full strength. One rather than zero so that choosing a
/// lens shows it: a default of nothing would look like a broken
/// picker.
fn default_lens_amount() -> f32 {
    1.0
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
/// A new config's bailout: the default colouring's recommendation
/// (smooth, `ColoringDef::recommended_bailout`). The smooth count's
/// error falls off with the escape radius -- worst pixel 0.75
/// iterations at 4, 0.12 at 10, nothing measurable at 1e4 (radius 100).
fn default_bailout() -> f32 {
    1.0e4
}
/// The bailout of a file that does not name one: the default until
/// 2026-10-02, and so the one every built-in preset that names none
/// was drawn at.
pub const LEGACY_BAILOUT: f32 = 4.0;
fn legacy_bailout() -> f32 {
    LEGACY_BAILOUT
}
fn default_damping_re() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
/// How supersampled samples are combined into a display pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DownsampleMode {
    /// Plain average in linear light. Physically what a sensor does,
    /// and correct for luminance.
    #[default]
    Box,
    /// Average in a perceptual (gamma 2.2) space. Mixing two
    /// saturated colours there lands between them rather than at
    /// their linear sum, so hues stay their own rather than drifting
    /// toward the brighter neighbour — at the cost of thin bright
    /// detail reading darker than it physically should.
    Perceptual,
    /// Weight each sample by its own saturation, so a coloured
    /// filament is not diluted to grey by neutral neighbours. Keeps
    /// fine colour vivid; deliberately not energy-preserving.
    Vivid,
}

impl DownsampleMode {
    pub fn to_gpu(self) -> u32 {
        match self {
            DownsampleMode::Box => 0,
            DownsampleMode::Perceptual => 1,
            DownsampleMode::Vivid => 2,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            DownsampleMode::Box => "box",
            DownsampleMode::Perceptual => "perceptual",
            DownsampleMode::Vivid => "vivid",
        }
    }
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "perceptual" => DownsampleMode::Perceptual,
            "vivid" => DownsampleMode::Vivid,
            _ => DownsampleMode::Box,
        }
    }
}

fn is_downsample_default(v: &DownsampleMode) -> bool {
    *v == DownsampleMode::Box
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
            cam_target_x: String::new(),
            cam_target_y: String::new(),
            cam_target_z: String::new(),
            cam_pitch: default_cam_pitch(),
            cam_yaw: 0.0,
            cam_bank: 0.0,
            cam_fov: default_cam_fov(),
            max_iter: default_max_iter(),
            bailout: default_bailout(),
            damping_re: default_damping_re(),
            damping_im: 0.0,
            biomorph: BiomorphMode::Off,
            coloring: default_coloring(),
            formula_params: BTreeMap::new(),
            coloring_params: BTreeMap::new(),
            lens: String::new(),
            lens_params: BTreeMap::new(),
            lens_amount: default_lens_amount(),
            supersample: 1,
            downsample: DownsampleMode::Box,
            reference_period: None,
            shading: EscapeShading::default(),
            contrast: EscapeContrast::default(),
            palette_map: PaletteMap::default(),
            layer: ColoringLayer::default(),
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

    /// The zoom exponent in BASE 10 — the display unit.
    ///
    /// Every other deep-zoom tool reports magnification as a power of
    /// ten (fraktaler-3, Kalles Fraktaler, Ultra Fractal), so that is
    /// what the UI shows. The stored/engine value stays base 2 on
    /// purpose: see [`Self::zoom_log2`] — there it is a BIT COUNT
    /// (`limbs_for_zoom` adds it straight to a bit total), it is added
    /// directly into floatexp base-2 exponents, and the renderer
    /// splits it into the shader's exact `s_m · 2^s_e` pixel spacing.
    /// A base-10 store would put an irrational factor in front of all
    /// three.
    pub fn zoom_log10(&self) -> f64 {
        self.zoom_log2 * std::f64::consts::LOG10_2
    }

    /// Inverse of [`Self::zoom_log10`], for UI edits.
    pub fn log10_to_log2(log10: f64) -> f64 {
        log10 * std::f64::consts::LOG2_10
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

    /// Contrast is off by default and must stay out of the file, for
    /// the same reason shading does: every existing config would
    /// otherwise be rewritten the first time it was re-saved.
    #[test]
    fn default_contrast_serializes_to_nothing() {
        let esc = EscapeConfig::default();
        let json = serde_json::to_string(&esc).unwrap();
        assert!(
            !json.contains("contrast"),
            "default contrast was written to the config: {json}"
        );
        let back: EscapeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.contrast, EscapeContrast::default());
        assert!(!back.contrast.is_active());
    }

    #[test]
    fn contrast_round_trips_and_gates_on_strength() {
        let mut esc = EscapeConfig::default();
        esc.contrast = EscapeContrast {
            mode: ContrastMode::Flatten,
            clip: 0.02,
            strength: 0.75,
            turns: 4.0,
        };
        let json = serde_json::to_string(&esc).unwrap();
        let back: EscapeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.contrast, esc.contrast);
        assert!(back.contrast.is_active());
        // Strength 0 means "measure nothing": it is the continuous
        // way to turn the stage off, so it must not cost a probe.
        let mut off = esc.clone();
        off.contrast.strength = 0.0;
        assert!(!off.contrast.is_active());
        // Every mode's wire string must survive.
        for m in [ContrastMode::Off, ContrastMode::AutoRange, ContrastMode::Flatten] {
            assert_eq!(contrast_mode_from_str(contrast_mode_to_str(m)), m);
        }
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
            model: ReliefModel::Lambert,
            elevation: 55.0,
            ambient: 0.3,
            stencil: SlopeStencil::Roberts,
            height_curve: HeightTransfer::Cos,
            height_pre: 4.0,
            height_post: 0.25,
        };
        let json = serde_json::to_string(&esc).unwrap();
        let back: EscapeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.shading, esc.shading);
        // A shading block written before these options reads as the
        // relief it drew: the tilt, no ambient, the central stencil, no
        // curve.
        let old: EscapeShading = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert_eq!(
            (old.model, old.ambient, old.stencil, old.height_curve, old.height_pre, old.height_post),
            (ReliefModel::Tilt, 0.0, SlopeStencil::Central, HeightTransfer::Linear, 1.0, 1.0)
        );
        for m in ReliefModel::ALL {
            assert_eq!(ReliefModel::from_name(m.as_str()), m);
        }
        for m in SlopeStencil::ALL {
            assert_eq!(SlopeStencil::from_name(m.as_str()), m);
        }
        for m in HeightTransfer::ALL {
            assert_eq!(HeightTransfer::from_name(m.as_str()), m);
            assert!(m.apply(0.7).is_finite() && m.apply(-0.7).is_finite(), "{m:?}");
        }
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
        for f in [ShadingField::Smooth, ShadingField::Banded, ShadingField::Layer] {
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

    /// A new config starts at a bailout of 1e4, but a file that names none
    /// keeps the 4 it was written under, and a saved config always names
    /// one -- so the default can move without moving a saved picture.
    #[test]
    fn a_file_without_a_bailout_keeps_the_old_default() {
        assert_eq!(EscapeConfig::default().bailout, 1.0e4);
        let old: EscapeConfig = serde_json::from_str(r#"{"formula":"mandelbrot"}"#).expect("an old escape block");
        assert_eq!(old.bailout, 4.0);
        let json = serde_json::to_string(&EscapeConfig { formula: "burning_ship".into(), ..EscapeConfig::default() }).unwrap();
        assert!(json.contains("\"bailout\":10000.0"), "the bailout must be written: {json}");
    }

    /// New relief is lit from the upper left (135 degrees counter-clockwise
    /// from east); a file that names no angle keeps the lower right it
    /// was written under, and a saved shading block always names one.
    #[test]
    fn relief_without_a_light_angle_keeps_the_old_light() {
        assert_eq!(EscapeShading::default().light_angle, 135.0);
        let old: EscapeShading = serde_json::from_str(r#"{"enabled":true}"#).expect("an old shading block");
        assert_eq!(old.light_angle, 315.0);
        let json = serde_json::to_string(&EscapeShading { enabled: true, ..EscapeShading::default() }).unwrap();
        assert!(json.contains("\"light_angle\":135.0"), "the light must be written: {json}");
    }

    /// The palette map is skipped while it is the identity, and a set
    /// one comes back exactly.
    #[test]
    fn a_palette_map_is_written_only_when_set_and_round_trips() {
        let json = serde_json::to_string(&EscapeConfig::default()).unwrap();
        assert!(!json.contains("palette_map"), "{json}");
        let esc = EscapeConfig {
            palette_map: PaletteMap {
                transfer: TransferCurve::LogLog,
                pivot: 12.5,
                curve: PaletteCurve::InverseS,
                stepped: true,
            },
            ..EscapeConfig::default()
        };
        let json = serde_json::to_string(&esc).unwrap();
        assert!(json.contains(r#""transfer":"log_log""#), "{json}");
        assert!(json.contains(r#""curve":"inverse_s""#), "{json}");
        let back: EscapeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.palette_map, esc.palette_map);
    }

    /// Every transfer leaves the pivot where Linear would, rises
    /// monotonically, and mirrors for negative values -- the three
    /// promises the pivot control's tooltip makes.
    #[test]
    fn every_transfer_meets_linear_at_the_pivot() {
        for transfer in TransferCurve::ALL {
            assert!((transfer.shape(1.0) - 1.0).abs() < 1e-6, "{transfer:?}: f(1) = {}", transfer.shape(1.0));
            assert_eq!(transfer.shape(0.0), 0.0, "{transfer:?}");
            let mut prev = 0.0f32;
            for i in 1..=400 {
                let u = i as f32 * 0.05;
                let f = transfer.shape(u);
                assert!(f > prev, "{transfer:?} is not increasing at {u}");
                prev = f;
            }
            let pm = PaletteMap { transfer, pivot: 7.0, ..PaletteMap::default() };
            assert!((pm.transfer_value(7.0) - 7.0).abs() < 1e-4, "{transfer:?}");
            assert_eq!(pm.transfer_value(-3.0), -pm.transfer_value(3.0), "{transfer:?}");
            assert_eq!(TransferCurve::from_name(transfer.as_str()), transfer);
        }
        assert_eq!(TransferCurve::from_name("from_the_future"), TransferCurve::Linear);
    }

    /// A palette curve keeps both ends of the cycle where they were and
    /// only moves what lies between.
    #[test]
    fn every_palette_curve_keeps_the_ends_of_a_cycle() {
        for curve in PaletteCurve::ALL {
            assert!(curve.apply(0.0).abs() < 1e-6, "{curve:?} at 0");
            assert!((curve.apply(1.0) - 1.0).abs() < 1e-6, "{curve:?} at 1");
            let mut prev = curve.apply(0.0);
            for i in 1..=100 {
                let t = curve.apply(i as f32 / 100.0);
                assert!(t > prev, "{curve:?} is not increasing at {}", i as f32 / 100.0);
                prev = t;
            }
            assert_eq!(PaletteCurve::from_name(curve.as_str()), curve);
        }
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
            ConfigPath::EscapeLens,
            ConfigPath::EscapeLensAmount,
            ConfigPath::EscapeLensParam { param: "power".into() },
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
