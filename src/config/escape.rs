//! Escape-time fractal configuration.
//!
//! The per-config state for the fragment rendering modes (Mandelbrot
//! and kin) — everything `docs/archive/escape-time/escape-time-fractals.md` calls
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

    /// The formula's SOURCE, when `formula` names one compiled from it
    /// (`custom-...`, `escape::lang`): carried whole, so the file renders
    /// on any machine without a formula library or the API. Kept when
    /// another formula is picked, so undo and switching back find it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_formula: Option<CustomSource>,

    /// The colouring's source, the same way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_coloring: Option<CustomSource>,

    /// The **camera lens**: a variation applied to the normalised
    /// screen offset before the view scale, by name. Empty is no lens.
    ///
    /// This warps the VIEW, not the fractal: the formula is untouched
    /// and only which point each pixel samples changes, which is the
    /// opposite direction from a flame's final transform. See
    /// `docs/archive/escape-time/escape-camera-lens.md`.
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

    /// Transformations (the KIFS plan, C4): variations applied to the
    /// sample point in the fractal's coordinates (`Plane`, UF's
    /// transformations), to each orbit value the colouring accumulates
    /// (`Orbit`, UF's trap position), or to `z` after every step
    /// (`Iteration`), in order within each stage. They share the lens's
    /// maps flame (`escape::lens`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transforms: Vec<EscapeTransform>,


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
    /// How mode D's solid is rendered (heightfield plan T3c). Lit by
    /// default, so a solid saved before the path tracer renders as it
    /// did; a terrain's tier is its own.
    #[serde(default = "default_solid_tier", skip_serializing_if = "is_default_solid_tier")]
    pub solid_tier: RenderTier,

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

    /// The 3D terrain view (docs/archive/terrain/heightfield-3d.md): this
    /// picture as a height field, seen through the solid camera. Off by
    /// default and skipped when off, so every existing file is
    /// byte-stable.
    #[serde(default, skip_serializing_if = "TerrainConfig::is_default")]
    pub terrain: TerrainConfig,

    /// The path tracer's settings, for a terrain and a solid alike.
    #[serde(default, skip_serializing_if = "PathTraceConfig::is_default")]
    pub path: PathTraceConfig,

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
    /// A simulation texture (`docs/archive/projects/sim-textures.md`): its recipe,
    /// in full, so the file is self-contained. None is no texture, the
    /// default, and is skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<EscapeTexture>,
    /// The texture as an overlay (sim-textures phase 2, survey R10):
    /// Kalles Fraktaler's texture, warped by the iteration count's slope
    /// and mixed into the colour before the relief. Off by default.
    #[serde(default, skip_serializing_if = "TextureOverlay::is_default")]
    pub texture_overlay: TextureOverlay,
}

/// How the texture covers the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureFit {
    /// Stretched over the whole frame, as Kalles Fraktaler resizes its
    /// image; a lookup past the edge holds the edge.
    #[default]
    Stretch,
    /// Repeated at its own size (times the tile scale): a periodic
    /// simulation texture tiles seamlessly.
    Tile,
}

impl TextureFit {
    pub fn to_gpu(self) -> u32 {
        match self {
            TextureFit::Stretch => 0,
            TextureFit::Tile => 1,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            TextureFit::Stretch => "stretch",
            TextureFit::Tile => "tile",
        }
    }
    pub fn from_name(s: &str) -> Self {
        if s == "tile" { TextureFit::Tile } else { TextureFit::Stretch }
    }
}

/// Kalles Fraktaler's texture overlay (`gl/kf.frag.glsl`
/// `KF_TextureWarp` and the texture block after the palette lookup):
/// the texture looked up at the pixel plus an offset driven by the
/// iteration count's difference to its neighbours, then mixed in.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextureOverlay {
    #[serde(default)]
    pub enabled: bool,
    /// How much of the texture replaces the colour, 0..1. KF2's default
    /// is 1: the texture alone, warped.
    #[serde(default = "default_overlay_merge")]
    pub merge: f32,
    /// KF2's power: how hard the iteration count's slope warps the
    /// lookup. A whole number, as KF2 keeps it. Default 200.
    #[serde(default = "default_overlay_power")]
    pub power: f32,
    /// KF2's ratio, in percent: scales the warp. Default 100.
    #[serde(default = "default_overlay_ratio")]
    pub ratio: f32,
    #[serde(default)]
    pub fit: TextureFit,
    /// Display pixels per texel when tiled.
    #[serde(default = "default_one")]
    pub tile_scale: f32,
}

fn default_overlay_merge() -> f32 {
    1.0
}
fn default_overlay_power() -> f32 {
    200.0
}
fn default_overlay_ratio() -> f32 {
    100.0
}

/// Ranges of the overlay's controls.
pub const OVERLAY_POWER_RANGE: (f32, f32) = (0.0, 1000.0);
pub const OVERLAY_RATIO_RANGE: (f32, f32) = (0.0, 400.0);
pub const OVERLAY_TILE_RANGE: (f32, f32) = (0.05, 20.0);

impl Default for TextureOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            merge: default_overlay_merge(),
            power: default_overlay_power(),
            ratio: default_overlay_ratio(),
            fit: TextureFit::default(),
            tile_scale: 1.0,
        }
    }
}

impl TextureOverlay {
    pub fn is_default(v: &TextureOverlay) -> bool {
        *v == TextureOverlay::default()
    }
}

/// Which polar part of a complex formula parameter `name.r` or
/// `name.angle` reads.
#[derive(Clone, Copy)]
enum Polar {
    Radius,
    Angle,
}

/// `name.r` / `name.angle`: the complex parameter `name` as a point in
/// polar form.
fn polar_part(name: &str) -> Option<(&str, Polar)> {
    if let Some(base) = name.strip_suffix(".r") {
        return Some((base, Polar::Radius));
    }
    name.strip_suffix(".angle").map(|base| (base, Polar::Angle))
}

impl EscapeConfig {
    /// A formula parameter's value, `default` giving an unset one's.
    /// `name.r` and `name.angle` read a complex parameter `name` (held
    /// as `name.re` and `name.im`) as a point in polar form, its radius
    /// and its angle in degrees, so it can animate around the origin as
    /// well as along x and y (phase 4, step 8).
    pub fn formula_param(&self, name: &str, default: impl Fn(&str) -> Option<f32>) -> Option<f32> {
        let get = |p: &str| self.formula_params.get(p).copied().or_else(|| default(p));
        match polar_part(name) {
            Some((base, part)) => {
                let (re, im) = (get(&format!("{base}.re"))?, get(&format!("{base}.im"))?);
                Some(match part {
                    Polar::Radius => re.hypot(im),
                    Polar::Angle => im.atan2(re).to_degrees(),
                })
            }
            None => get(name),
        }
    }

    /// What setting a formula parameter writes: the parameter itself,
    /// or for `name.r` / `name.angle` the complex parameter's two parts,
    /// its other polar part kept.
    pub fn formula_param_writes(&self, name: &str, v: f32, default: impl Fn(&str) -> Option<f32>) -> Vec<(String, f32)> {
        let Some((base, part)) = polar_part(name) else {
            return vec![(name.to_string(), v)];
        };
        let (re_k, im_k) = (format!("{base}.re"), format!("{base}.im"));
        let get = |p: &str| self.formula_params.get(p).copied().or_else(|| default(p)).unwrap_or(0.0);
        let (re, im) = (get(&re_k), get(&im_k));
        let (r, a) = match part {
            Polar::Radius => (v, im.atan2(re)),
            Polar::Angle => (re.hypot(im), v.to_radians()),
        };
        vec![(re_k, r * a.cos()), (im_k, r * a.sin())]
    }

    /// Whether anything draws with the texture, so it has to be on the
    /// GPU: the overlay, the relief's bump, or a colouring that reads it
    /// (the image trap).
    pub fn uses_texture(&self) -> bool {
        #[cfg(feature = "engine-escape")]
        let coloring = crate::escape::COLORINGS
            .iter()
            .any(|c| c.name == self.coloring && c.has_feature(crate::escape::ColoringFeature::TextureInLoop));
        #[cfg(not(feature = "engine-escape"))]
        let coloring = false;
        // The relief's simulation micro-texture, or a terrain's (which
        // lights its own surface whether the 2D relief is on or not).
        let surface = self.shading.texture_kind == ShadingTexture::Simulation
            && (self.shading.enabled || self.terrain_active());
        self.texture.is_some() && (self.texture_overlay.enabled || coloring || surface)
    }

    /// Whether the 3D terrain carries the surface texture: a kind at a
    /// strength (docs/projects/surface-textures.md). It has its own
    /// lighting, so the 2D relief's switch does not enter.
    pub fn surface_texture_active(&self) -> bool {
        self.terrain_active()
            && self.shading.texture_kind != ShadingTexture::None
            && self.shading.texture_strength > 0.0
            && (self.shading.texture_kind != ShadingTexture::Simulation || self.texture.is_some())
    }
}

/// A simulation texture a config uses: its name, and its recipe -- a
/// Simulation-mode config -- in full (`docs/archive/projects/sim-textures.md`,
/// decision 6), so the file that uses it needs nothing else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscapeTexture {
    pub name: String,
    /// Written and read the way a `.fflame` file is -- compact, with its
    /// version, and migrated on load -- not as the derive would.
    #[serde(with = "embedded_config")]
    pub config: Box<super::FractalConfig>,
}

impl PartialEq for EscapeTexture {
    fn eq(&self, other: &Self) -> bool {
        // FractalConfig has no PartialEq; its file form is its identity.
        self.name == other.name && self.config.to_json_value().ok() == other.config.to_json_value().ok()
    }
}

/// A config inside a config, in its file form.
mod embedded_config {
    use super::super::FractalConfig;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(config: &FractalConfig, s: S) -> Result<S::Ok, S::Error> {
        config.to_json_value().map_err(serde::ser::Error::custom)?.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Box<FractalConfig>, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        FractalConfig::from_json_value(value).map(Box::new).map_err(serde::de::Error::custom)
    }
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
    /// The distance estimate's own slope, from the derivative rather
    /// than from neighbouring pixels (survey R5, Kalles Fraktaler's
    /// analytic slopes): sharper, and free of stencil artefacts. Direct
    /// path only, on formulas with a derivative.
    Analytic,
    /// The colouring's value at points a small, fixed step away, run as
    /// orbits of their own (survey R6, Ultra Fractal's Slope): relief
    /// that belongs to the fractal rather than the pixel grid, the same
    /// at any output size. Three orbits a pixel; direct path only.
    Offset,
    /// Ultra Fractal's Embossed (survey R7): two orbits a small step
    /// either side of the pixel along the light, each reduced to a
    /// whole number ([`EmbossType`]); where they differ, the pixel is a
    /// shadow or a highlight by which came out higher. Bevelled contour
    /// lines, a fixed fraction of the view wide. Direct path only.
    Embossed,
    /// A folding formula's paper (`FormulaFeature::Creases`): the height
    /// rises or falls with the distance from each crease, and the slope
    /// is exact, carried through the folds (`CreaseSettings`;
    /// docs/projects/origami-creases.md). Flat on a formula without
    /// creases.
    Creases,
}

impl ShadingField {
    pub fn to_gpu(self) -> u32 {
        match self {
            ShadingField::Smooth => 0,
            ShadingField::Banded => 1,
            ShadingField::Layer => 2,
            ShadingField::Analytic => 3,
            ShadingField::Offset => 4,
            ShadingField::Embossed => 5,
            // 3 or more: a stored slope, which the texture overlay must
            // leave its channels to (`overlay_owns_blue`).
            ShadingField::Creases => 6,
        }
    }
}

/// The direction each fold of a folding formula takes before a
/// face-down layer of the wad reverses it (`CreaseSettings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreasePattern {
    /// Every fold a valley: each flap folds toward the viewer.
    #[default]
    Valley,
    /// Valley, mountain, valley, and so on.
    Alternate,
    /// Each fold's direction from the formula's line seed.
    Random,
}

impl CreasePattern {
    pub const ALL: [CreasePattern; 3] = [CreasePattern::Valley, CreasePattern::Alternate, CreasePattern::Random];
    pub fn as_str(self) -> &'static str {
        match self {
            CreasePattern::Valley => "valley",
            CreasePattern::Alternate => "alternate",
            CreasePattern::Random => "random",
        }
    }
    pub fn from_name(s: &str) -> Self {
        match s {
            "alternate" => CreasePattern::Alternate,
            "random" => CreasePattern::Random,
            _ => CreasePattern::Valley,
        }
    }
    pub fn to_gpu(self) -> f32 {
        match self {
            CreasePattern::Valley => 0.0,
            CreasePattern::Alternate => 1.0,
            CreasePattern::Random => 2.0,
        }
    }
    fn is_default(v: &CreasePattern) -> bool {
        *v == CreasePattern::Valley
    }
}

/// A folding formula's paper, for the Creases relief field and terrain
/// source (docs/projects/origami-creases.md). Each fold is a crease; the
/// paper rises or falls in a straight line with the distance from it,
/// the same slope on both sides. Fold i's crease weighs `decay^i`, so the
/// first is the main crease and each later one finer. A crease is a
/// valley on one face of the paper and a mountain on the other, so a
/// layer an odd number of earlier folds turned face down reverses it.
///
/// There is no depth: the relief's Height scales the slope, and a
/// terrain fits its heights to its own range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreaseSettings {
    #[serde(default, skip_serializing_if = "CreasePattern::is_default")]
    pub pattern: CreasePattern,
    /// How much finer each later crease is (`CREASE_DECAY_RANGE`).
    #[serde(default = "default_crease_decay", skip_serializing_if = "is_default_crease_decay")]
    pub decay: f32,
    /// Swaps mountain and valley.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip: bool,
}

fn default_crease_decay() -> f32 {
    0.6
}
fn is_default_crease_decay(v: &f32) -> bool {
    *v == default_crease_decay()
}
pub const CREASE_DECAY_RANGE: (f32, f32) = (0.05, 1.0);
/// How far from its crease a terrain's paper slopes before it levels
/// off, in paper sizes (Origami's line spread). The 2D relief lights
/// slopes, so its creases run on unbounded; a terrain fits its heights
/// to the whole range it sees, out to the horizon, where an unbounded
/// rise would dwarf every crease near the camera
/// (docs/projects/origami-creases.md).
pub const CREASE_TERRAIN_REACH: f32 = 1.0;

impl Default for CreaseSettings {
    fn default() -> Self {
        Self { pattern: CreasePattern::Valley, decay: default_crease_decay(), flip: false }
    }
}

impl CreaseSettings {
    fn is_default(v: &CreaseSettings) -> bool {
        *v == CreaseSettings::default()
    }
    /// The escape uniform's `crease` row: the decay, the pattern, the
    /// flip as a sign, and the reach (`CREASE_TERRAIN_REACH` for a
    /// terrain, else 0).
    pub fn to_gpu(&self, terrain: bool) -> [f32; 4] {
        [
            self.decay.clamp(CREASE_DECAY_RANGE.0, CREASE_DECAY_RANGE.1),
            self.pattern.to_gpu(),
            if self.flip { -1.0 } else { 1.0 },
            if terrain { CREASE_TERRAIN_REACH } else { 0.0 },
        ]
    }
}

/// What each of Embossed's two orbits is reduced to: Ultra Fractal's
/// Emboss Type (`Standard_EmbossedHelper`, Standard.ulb). The contour
/// lines fall where this whole number changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbossType {
    /// The iteration the orbit escaped on (0 if it never did).
    #[default]
    Iteration,
    /// How many iterates had a positive real part.
    RealPositive,
    /// How many iterates had a positive imaginary part.
    ImagPositive,
    /// The iteration at which |z|^2 was smallest -- the smallest of
    /// EITHER orbit so far, which is how Ultra Fractal keeps it.
    SmallestMagnitude,
    /// `trunc(ln|z|)` at escape (0 if it never did).
    Magnitude,
    /// Which of `sections` equal sectors `arg z` fell in at escape.
    Angle,
}

impl EmbossType {
    pub const ALL: [EmbossType; 6] = [
        EmbossType::Iteration,
        EmbossType::RealPositive,
        EmbossType::ImagPositive,
        EmbossType::SmallestMagnitude,
        EmbossType::Magnitude,
        EmbossType::Angle,
    ];
    /// Ultra Fractal's own enum order, which the shader switches on.
    pub fn to_gpu(self) -> u32 {
        match self {
            EmbossType::Iteration => 0,
            EmbossType::RealPositive => 1,
            EmbossType::ImagPositive => 2,
            EmbossType::SmallestMagnitude => 3,
            EmbossType::Magnitude => 4,
            EmbossType::Angle => 5,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            EmbossType::Iteration => "iteration",
            EmbossType::RealPositive => "real_positive",
            EmbossType::ImagPositive => "imag_positive",
            EmbossType::SmallestMagnitude => "smallest_magnitude",
            EmbossType::Magnitude => "magnitude",
            EmbossType::Angle => "angle",
        }
    }
    pub fn from_name(s: &str) -> Self {
        Self::ALL.into_iter().find(|m| m.as_str() == s).unwrap_or_default()
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Embossed's Angle sectors. Ultra Fractal's minimum is 1; the top is
/// what five bits of the shader's flag word hold.
pub const EMBOSS_SECTIONS_RANGE: (u32, u32) = (1, 32);

/// The wire strings ConfigValue carries for [`ShadingField`].
pub fn shading_field_to_str(m: ShadingField) -> &'static str {
    match m {
        ShadingField::Smooth => "smooth",
        ShadingField::Banded => "banded",
        ShadingField::Layer => "layer",
        ShadingField::Analytic => "analytic",
        ShadingField::Offset => "offset",
        ShadingField::Embossed => "embossed",
        ShadingField::Creases => "creases",
    }
}

pub fn shading_field_from_str(s: &str) -> ShadingField {
    match s {
        "banded" => ShadingField::Banded,
        "layer" => ShadingField::Layer,
        "analytic" => ShadingField::Analytic,
        "offset" => ShadingField::Offset,
        "embossed" => ShadingField::Embossed,
        "creases" => ShadingField::Creases,
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
    /// Rank-equalise the field (survey P7): each value becomes the share
    /// of the frame below it, so every part of the palette covers the
    /// same area of the picture -- techmatt's rank transfer, F3's
    /// histogram colouring. Measured from the same probe as the others.
    Equalize,
}

impl ContrastMode {
    pub fn to_gpu(self) -> u32 {
        match self {
            ContrastMode::Off => 0,
            ContrastMode::AutoRange => 1,
            ContrastMode::Flatten => 2,
            ContrastMode::Equalize => 3,
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
        ContrastMode::Equalize => "equalize",
    }
}

pub fn contrast_mode_from_str(s: &str) -> ContrastMode {
    match s {
        "auto_range" => ContrastMode::AutoRange,
        "flatten" => ContrastMode::Flatten,
        "equalize" => ContrastMode::Equalize,
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
    /// The paper a folding formula's Creases field and terrain source
    /// draw.
    #[serde(default, skip_serializing_if = "CreaseSettings::is_default")]
    pub creases: CreaseSettings,

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
    /// Where the texture applies, by slope (`TextureWhere`), and the
    /// slope it turns at, ± its softness (degrees). The 3D terrain and the
    /// 2D relief share them (docs/projects/surface-textures.md).
    #[serde(default, skip_serializing_if = "TextureWhere::is_default")]
    pub texture_where: TextureWhere,
    #[serde(default = "default_texture_angle", skip_serializing_if = "is_default_texture_angle")]
    pub texture_angle: f32,
    #[serde(default = "default_texture_angle_soft", skip_serializing_if = "is_default_texture_angle_soft")]
    pub texture_angle_soft: f32,

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
    /// The step to the offset orbits (`ShadingField::Offset`), as a
    /// fraction of the view's height: small enough to read the local
    /// slope, large enough to step over f32 noise. For Embossed it is
    /// the step either side of the pixel, which sets how wide the
    /// contour lines are (Ultra Fractal's Contour Size).
    #[serde(default = "default_relief_offset", skip_serializing_if = "is_default_relief_offset")]
    pub offset: f32,
    /// What Embossed's orbits are reduced to.
    #[serde(default, skip_serializing_if = "EmbossType::is_default")]
    pub emboss: EmbossType,
    /// Embossed's Angle sectors; Ultra Fractal's default is 2.
    #[serde(default = "default_emboss_sections", skip_serializing_if = "is_default_emboss_sections")]
    pub emboss_sections: u32,
}

fn default_emboss_sections() -> u32 {
    2
}
fn is_default_emboss_sections(v: &u32) -> bool {
    *v == default_emboss_sections()
}

fn default_relief_offset() -> f32 {
    1.0 / 1024.0
}
fn is_default_relief_offset(v: &f32) -> bool {
    *v == default_relief_offset()
}

/// The offset step's range, as a fraction of the view height.
pub const RELIEF_OFFSET_RANGE: (f32, f32) = (1.0e-5, 0.05);

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
            creases: CreaseSettings::default(),
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
            texture_where: TextureWhere::Everywhere,
            texture_angle: default_texture_angle(),
            texture_angle_soft: default_texture_angle_soft(),
            model: ReliefModel::default(),
            elevation: default_relief_elevation(),
            ambient: 0.0,
            stencil: SlopeStencil::default(),
            height_curve: HeightTransfer::default(),
            height_pre: 1.0,
            height_post: 1.0,
            offset: default_relief_offset(),
            emboss: EmbossType::default(),
            emboss_sections: default_emboss_sections(),
        }
    }
}

impl EscapeShading {
    pub fn is_default(v: &EscapeShading) -> bool {
        *v == EscapeShading::default()
    }
    /// Whether the relief needs the derivative orbit: analytic slopes.
    pub fn wants_derivative(&self) -> bool {
        self.enabled && self.field == ShadingField::Analytic
    }
    /// Whether the relief draws a folding formula's creases.
    pub fn wants_creases(&self) -> bool {
        self.enabled && self.field == ShadingField::Creases
    }
    /// Whether the relief runs orbits beside each pixel's own.
    pub fn wants_offset_orbits(&self) -> bool {
        self.enabled && matches!(self.field, ShadingField::Offset | ShadingField::Embossed)
    }
    /// What the relief lights, as the shade pass reads it: 0 a height it
    /// differences, 1 a slope the iterate pass stored (analytic, offset
    /// orbits), 2 Embossed's stored response.
    pub fn stored_relief(&self) -> u32 {
        match self.field {
            ShadingField::Analytic | ShadingField::Offset | ShadingField::Creases => 1,
            ShadingField::Embossed => 2,
            _ => 0,
        }
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
/// Where an escape-time transformation acts (the KIFS plan, C4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformStage {
    /// The sample point, in the fractal's coordinates, before the
    /// formula: the picture warped as the fractal is, zoom with it.
    #[default]
    Plane,
    /// Each orbit value the colouring accumulates (trap and average
    /// colourings), not the final `z` an escape count reads.
    Orbit,
    /// `z` after every step of the formula: a change to the fractal.
    Iteration,
}

impl TransformStage {
    pub const ALL: [TransformStage; 3] = [TransformStage::Plane, TransformStage::Orbit, TransformStage::Iteration];
}

/// One escape-time transformation: a variation at a stage, its amount
/// (0 the identity, 1 the variation, as the lens's) and its parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscapeTransform {
    /// Session-local identity, so an animation track follows its
    /// transformation through a move or a removal, as a flame
    /// transform's or an effect's does (`crate::scene::transforms::next_id`).
    /// Never in a file; the list's undo values carry it
    /// ([`transforms_to_wire`]).
    #[serde(skip)]
    pub id: u64,
    #[serde(default)]
    pub stage: TransformStage,
    pub variation: String,
    #[serde(default = "default_lens_amount", skip_serializing_if = "is_one")]
    pub amount: f32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, f32>,
}

// Equality ignores the runtime-only `id`, as `EffectInstance`'s does.
impl PartialEq for EscapeTransform {
    fn eq(&self, other: &Self) -> bool {
        self.stage == other.stage && self.variation == other.variation && self.amount == other.amount && self.params == other.params
    }
}

impl EscapeTransform {
    /// A transformation with a fresh id and no parameters set.
    pub fn new(stage: TransformStage, variation: &str, amount: f32) -> Self {
        Self { id: crate::scene::transforms::next_id(), stage, variation: variation.to_string(), amount, params: BTreeMap::new() }
    }
}

/// The transformation list as `ConfigPath::EscapeTransforms` carries it:
/// the file's JSON with each entry's `id` too, so an undo or redo of a
/// structural edit brings back the same ids and a track bound to one
/// keeps it.
pub fn transforms_to_wire(list: &[EscapeTransform]) -> String {
    let entries: Vec<serde_json::Value> = list
        .iter()
        .map(|t| {
            let mut v = serde_json::to_value(t).unwrap_or_default();
            if let Some(o) = v.as_object_mut() {
                o.insert("id".to_string(), t.id.into());
            }
            v
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_default()
}

/// [`transforms_to_wire`]'s inverse. An entry without an id (a script's,
/// a file's) or repeating one already read gets a fresh one.
pub fn transforms_from_wire(s: &str) -> Result<Vec<EscapeTransform>, serde_json::Error> {
    if s.trim().is_empty() {
        return Ok(Vec::new());
    }
    let entries: Vec<serde_json::Value> = serde_json::from_str(s)?;
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .map(|v| {
            let id = v.get("id").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let mut t: EscapeTransform = serde_json::from_value(v)?;
            t.id = if id != 0 && seen.insert(id) { id } else { crate::scene::transforms::next_id() };
            Ok(t)
        })
        .collect()
}

impl EscapeConfig {
    /// Whether a transformation moves an absolute coordinate -- the
    /// sample point or the iterate -- which the perturbation path cannot
    /// follow (C4).
    pub fn transforms_absolute(&self) -> bool {
        self.transforms.iter().any(|t| t.stage != TransformStage::Orbit && t.amount != 0.0)
    }
}

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
    /// The config's simulation texture (`EscapeConfig::texture`), its
    /// luminance as the micro-relief (docs/archive/projects/sim-textures.md,
    /// phase 3). Nothing without a texture.
    Simulation,
    /// Ridged octaves with a fractal falloff: weathered stone, for
    /// roughening cliffs (docs/projects/surface-textures.md).
    Rock,
}

impl ShadingTexture {
    pub const ALL: [ShadingTexture; 5] =
        [ShadingTexture::None, ShadingTexture::Grain, ShadingTexture::Paper, ShadingTexture::Rock, ShadingTexture::Simulation];
    pub fn to_gpu(self) -> u32 {
        match self {
            ShadingTexture::None => 0,
            ShadingTexture::Grain => 1,
            ShadingTexture::Paper => 2,
            ShadingTexture::Simulation => 3,
            ShadingTexture::Rock => 4,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ShadingTexture::None => "none",
            ShadingTexture::Grain => "grain",
            ShadingTexture::Paper => "paper",
            ShadingTexture::Simulation => "simulation",
            ShadingTexture::Rock => "rock",
        }
    }
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "grain" => ShadingTexture::Grain,
            "paper" => ShadingTexture::Paper,
            "simulation" => ShadingTexture::Simulation,
            "rock" => ShadingTexture::Rock,
            _ => ShadingTexture::None,
        }
    }
}

/// Where on the surface the texture applies, by its slope: everywhere,
/// on the steep faces (cliffs), or on the flats. The slope is the 3D
/// terrain's true normal's, or the 2D relief's tilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextureWhere {
    #[default]
    Everywhere,
    Steep,
    Flat,
}

impl TextureWhere {
    pub const ALL: [TextureWhere; 3] = [TextureWhere::Everywhere, TextureWhere::Steep, TextureWhere::Flat];
    pub fn to_gpu(self) -> u32 {
        match self {
            TextureWhere::Everywhere => 0,
            TextureWhere::Steep => 1,
            TextureWhere::Flat => 2,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            TextureWhere::Everywhere => "everywhere",
            TextureWhere::Steep => "steep",
            TextureWhere::Flat => "flat",
        }
    }
    pub fn from_name(s: &str) -> Self {
        match s {
            "steep" => TextureWhere::Steep,
            "flat" => TextureWhere::Flat,
            _ => TextureWhere::Everywhere,
        }
    }
    fn is_default(v: &TextureWhere) -> bool {
        *v == TextureWhere::Everywhere
    }
}

/// The slope where Steep begins, in degrees (`EscapeShading::texture_angle`).
pub const TEXTURE_ANGLE_RANGE: (f32, f32) = (0.0, 90.0);
/// The half-width of the change about it.
pub const TEXTURE_ANGLE_SOFT_RANGE: (f32, f32) = (0.0, 45.0);
fn default_texture_angle() -> f32 {
    45.0
}
fn is_default_texture_angle(v: &f32) -> bool {
    *v == default_texture_angle()
}
fn default_texture_angle_soft() -> f32 {
    10.0
}
fn is_default_texture_angle_soft(v: &f32) -> bool {
    *v == default_texture_angle_soft()
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

/// What a terrain's height is made from (heightfield plan, section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainSource {
    /// `H exp(-DE / w)`: the set the plateau, every filament on smooth
    /// flanks of width `w`. Where the formula has no derivative, the
    /// escape count instead.
    #[default]
    Distance,
    /// The smooth escape count, on a log curve: spiky toward the set.
    EscapeCount,
    /// The 2D relief's own height source (the Escape panel's relief
    /// field: the colouring's value, banded, a layer's).
    Relief,
    /// A folding formula's paper: the Creases field's height
    /// (`CreaseSettings`), whatever the 2D relief draws.
    Creases,
}

impl TerrainSource {
    pub const ALL: [TerrainSource; 4] =
        [TerrainSource::Distance, TerrainSource::EscapeCount, TerrainSource::Relief, TerrainSource::Creases];
    pub fn as_str(self) -> &'static str {
        match self {
            TerrainSource::Distance => "distance",
            TerrainSource::EscapeCount => "escape_count",
            TerrainSource::Relief => "relief",
            TerrainSource::Creases => "creases",
        }
    }
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "escape_count" => TerrainSource::EscapeCount,
            "relief" => TerrainSource::Relief,
            "creases" => TerrainSource::Creases,
            _ => TerrainSource::Distance,
        }
    }
    /// The iterate pass's height mode for it: the shaders' relief
    /// source codes -- the relief's own (0-6), and 8 and 9 for the two
    /// the terrain adds. Creases is the relief's code.
    pub fn shade_flags(self, relief: ShadingField) -> u32 {
        match self {
            TerrainSource::Distance => 8,
            TerrainSource::EscapeCount => 9,
            TerrainSource::Relief => relief.to_gpu(),
            TerrainSource::Creases => ShadingField::Creases.to_gpu(),
        }
    }
}

/// How a 3D view -- a terrain, or mode D's solid -- is rendered
/// (heightfield plan section 4's two tiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderTier {
    /// The lit tier while anything moves, path traced while nothing does;
    /// exports path traced.
    #[default]
    Auto,
    /// The lit tier only: direct light, traced shadows, horizon occlusion.
    Lit,
    /// Path traced always: noisy while moving.
    PathTraced,
}

impl RenderTier {
    pub const ALL: [RenderTier; 3] = [RenderTier::Auto, RenderTier::Lit, RenderTier::PathTraced];
    pub fn as_str(self) -> &'static str {
        match self {
            RenderTier::Auto => "auto",
            RenderTier::Lit => "lit",
            RenderTier::PathTraced => "path_traced",
        }
    }
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "lit" => RenderTier::Lit,
            "path_traced" => RenderTier::PathTraced,
            _ => RenderTier::Auto,
        }
    }
}

/// The most samples a terrain texel averages, each way (H4): 3² costs
/// nine section renders' worth.
pub const MAX_TERRAIN_PREFILTER: u32 = 3;
/// The most a terrain's heights are smoothed, in texels (H4): its
/// border of three sigmas stays a dozen texels.
pub const MAX_TERRAIN_SOFTNESS: f32 = 4.0;

/// How a terrain's escape count or relief value rises to its height
/// (docs/archive/terrain/terrain-height.md, H2): over the range the ground
/// measured, a logarithmic or linear rise, or a mesa that tops out
/// early, so the busy boundary is a flat top rather than spikes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainCurve {
    /// The source's own: logarithmic for an escape count, linear for a
    /// relief -- every terrain's map before the choice.
    #[default]
    Auto,
    Log,
    Linear,
    Mesa,
}

impl TerrainCurve {
    pub const ALL: [TerrainCurve; 4] = [TerrainCurve::Auto, TerrainCurve::Log, TerrainCurve::Linear, TerrainCurve::Mesa];
    pub fn as_str(self) -> &'static str {
        match self {
            TerrainCurve::Auto => "auto",
            TerrainCurve::Log => "log",
            TerrainCurve::Linear => "linear",
            TerrainCurve::Mesa => "mesa",
        }
    }
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "log" => TerrainCurve::Log,
            "linear" => TerrainCurve::Linear,
            "mesa" => TerrainCurve::Mesa,
            _ => TerrainCurve::Auto,
        }
    }
    /// The walk's code (`hf_rise`).
    pub fn code(self) -> u32 {
        match self {
            TerrainCurve::Auto => 0,
            TerrainCurve::Log => 1,
            TerrainCurve::Linear => 2,
            TerrainCurve::Mesa => 3,
        }
    }
}

/// What a terrain does where the set's interior is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainInterior {
    /// Flat at the terrain's top, in the interior's colour: the set
    /// stands as a mesa.
    #[default]
    Plateau,
    /// Transparent, sunk to the slab's floor: a hole through it.
    Hole,
    /// Water, flat at the plain's level -- the height the ground falls
    /// to far from the set -- so the set's rim rises round it: a lake the
    /// path tracer renders as a dielectric mirror over the tint.
    Lake,
}

impl TerrainInterior {
    pub const ALL: [TerrainInterior; 3] = [TerrainInterior::Plateau, TerrainInterior::Hole, TerrainInterior::Lake];
    pub fn as_str(self) -> &'static str {
        match self {
            TerrainInterior::Plateau => "plateau",
            TerrainInterior::Hole => "hole",
            TerrainInterior::Lake => "lake",
        }
    }
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "hole" => TerrainInterior::Hole,
            "lake" => TerrainInterior::Lake,
            _ => TerrainInterior::Plateau,
        }
    }
}

/// The path tracer's settings (heightfield plan T3), shared by the two
/// geometries it renders: a terrain and mode D's solid. Which tier each
/// uses is its own (`TerrainConfig::tier`, `EscapeConfig::solid_tier`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathTraceConfig {
    /// Samples a pixel: the viewport's target, and an export's count.
    #[serde(default = "default_path_samples", skip_serializing_if = "is_default_path_samples")]
    pub samples: u32,
    /// Bounces after the first surface: 0 is direct light only.
    #[serde(default = "default_path_bounces", skip_serializing_if = "is_default_path_bounces")]
    pub bounces: u32,
    /// The environment's brightness: the background colour lighting the
    /// scene from the whole sky, times this.
    #[serde(default = "default_path_environment", skip_serializing_if = "is_default_path_environment")]
    pub environment: f32,
    // The coat and the glow were here; they are the picture's material
    // now, beside the lights (`SolidShadingSettings::gloss`, and
    // docs/archive/projects/materials.md), lifted there on load.
    /// The lens: its radius, as a fraction of the distance to the target
    /// (0, a pinhole, is everything sharp), and the focal plane's
    /// distance, as a multiple of the target's (0 is the target).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub aperture: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub focus: f32,
    /// Filter the path tracer's noise (T5): an edge-aware blur of the
    /// light, guided by each pixel's surface -- its albedo, normal and
    /// depth -- that eases off as the samples gather. Off by default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub denoise: bool,
    /// The sky (T5): the background alone (off), or a gradient from the
    /// background at the horizon to `zenith` overhead -- what a ray that
    /// meets nothing sees, and what lights the scene. In sRGB, as the
    /// background is picked.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sky_gradient: bool,
    #[serde(default = "default_path_zenith", skip_serializing_if = "is_default_path_zenith")]
    pub zenith: [f32; 3],
}

/// A clear sky's blue overhead.
fn default_path_zenith() -> [f32; 3] {
    [0.22, 0.40, 0.72]
}
fn is_default_path_zenith(v: &[f32; 3]) -> bool {
    *v == default_path_zenith()
}

fn default_path_samples() -> u32 {
    256
}
fn is_default_path_samples(v: &u32) -> bool {
    *v == default_path_samples()
}
fn default_path_bounces() -> u32 {
    2
}
fn is_default_path_bounces(v: &u32) -> bool {
    *v == default_path_bounces()
}
fn default_path_environment() -> f32 {
    1.0
}
fn is_default_path_environment(v: &f32) -> bool {
    *v == default_path_environment()
}

impl Default for PathTraceConfig {
    fn default() -> Self {
        PathTraceConfig {
            samples: default_path_samples(),
            bounces: default_path_bounces(),
            environment: default_path_environment(),
            aperture: 0.0,
            focus: 0.0,
            denoise: false,
            sky_gradient: false,
            zenith: default_path_zenith(),
        }
    }
}

impl PathTraceConfig {
    pub fn is_default(v: &PathTraceConfig) -> bool {
        *v == PathTraceConfig::default()
    }
}

/// The 3D terrain view (heightfield plan): the escape picture as a
/// height field. The FOOTPRINT is a square of the plane at the view,
/// rendered as an ordinary escape picture with the height kept, then
/// lit in 3D.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainConfig {
    #[serde(default, skip_serializing_if = "is_false")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_default_terrain_source")]
    pub source: TerrainSource,
    /// The terrain's relief, as a fraction of the footprint's width.
    #[serde(default = "default_terrain_height", skip_serializing_if = "is_default_terrain_height")]
    pub height: f32,
    /// Distance: the flanks' width `w`, as a fraction of the footprint.
    #[serde(default = "default_terrain_de_width", skip_serializing_if = "is_default_terrain_de_width")]
    pub de_width: f32,
    /// Texels per screen pixel, against the antialiasing factor: the
    /// sections' fineness (plan section 12). 1 samples the fractal as a
    /// 2D render at that antialiasing does.
    #[serde(default = "default_terrain_detail", skip_serializing_if = "is_default_terrain_detail")]
    pub detail: f32,
    /// Samples a texel, each way (docs/archive/terrain/terrain-height.md, H4):
    /// a section's height texel is the mean of `prefilter²` samples over
    /// its footprint rather than one at its centre, so detail finer than
    /// a texel averages instead of aliasing into spikes. 1 is a point
    /// sample; each step costs its square in section renders.
    #[serde(default = "default_terrain_prefilter", skip_serializing_if = "is_default_terrain_prefilter")]
    pub prefilter: u32,
    /// How far the heights are smoothed, in texels (a Gaussian's sigma,
    /// H4). A texel is about `supersample × detail` of a screen pixel
    /// where the ground is nearest, so this is a softness on screen; the
    /// sections are rendered with a border of three sigmas so they meet
    /// without a seam.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub softness: f32,
    /// The ground a smooth surface through its samples -- a cubic
    /// B-spline, continuous to its curvature -- rather than a bilinear
    /// patch a texel, whose creases at every grid line show as stepped
    /// shadows and serrated ridges where the ground is magnified
    /// (terrain-height.md, H5). Slower to draw; re-renders nothing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub smooth: bool,
    /// How far the ground reaches, in view widths from the eye; the fog
    /// reaches the background just before.
    #[serde(default = "default_terrain_far", skip_serializing_if = "is_default_terrain_far")]
    pub far: f32,
    /// How much of that distance the fog takes: 0 none (the ground's
    /// edge shows), 1 from about a third of the way out.
    #[serde(default = "default_terrain_haze", skip_serializing_if = "is_default_terrain_haze")]
    pub haze: f32,
    /// How it is rendered: lit, path traced, or both by turns.
    #[serde(default, skip_serializing_if = "is_default_terrain_tier")]
    pub tier: RenderTier,
    #[serde(default, skip_serializing_if = "is_default_terrain_interior")]
    pub interior: TerrainInterior,
    /// How a count or a relief rises to its height (H2).
    #[serde(default, skip_serializing_if = "is_default_terrain_curve")]
    pub curve: TerrainCurve,
    /// The land upside down: `H − h`, for every source -- spikes become
    /// pits, the set's boundary a canyon. The interior stays as chosen.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
    /// A lake's colour under its surface (linear), and how blurred its
    /// reflections are: 0.02 is a still mirror.
    #[serde(default = "default_terrain_lake_tint", skip_serializing_if = "is_default_terrain_lake_tint")]
    pub lake_tint: [f32; 3],
    #[serde(default = "default_terrain_lake_roughness", skip_serializing_if = "is_default_terrain_lake_roughness")]
    pub lake_roughness: f32,
    /// How dark a shadowed surface goes; 0 traces no shadow rays. Mode
    /// D's Shadows, which are its formula's parameter.
    #[serde(default = "default_terrain_shadow", skip_serializing_if = "is_default_terrain_shadow")]
    pub shadow: f32,
    /// How sharply a shadow's edge falls off: the inverse of the
    /// light's size, as mode D's Shadow Sharpness.
    #[serde(default = "default_terrain_shadow_sharpness", skip_serializing_if = "is_default_terrain_shadow_sharpness")]
    pub shadow_sharpness: f32,
    /// How far occlusion looks for the slopes that enclose a point, as
    /// a fraction of the footprint. About the distance flanks' width.
    #[serde(default = "default_terrain_occlusion", skip_serializing_if = "is_default_terrain_occlusion")]
    pub occlusion: f32,
    /// The camera's target above where it rests -- the relief's top at
    /// the view's centre -- in view widths (camera-unification C4). It
    /// lifts the eye with it, so the camera can look up, or level, from
    /// above the ground; 0 is the camera every terrain had before.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub target_lift: f32,
    /// The relief fixed at a zoom (docs/archive/terrain/terrain-height.md, H3).
    /// Locked, the height, the flanks' width, occlusion's reach and a
    /// hole's depth are fractions of `lock_zoom`'s view width rather than
    /// the current one's ([`EscapeConfig::terrain_scale`]), so zooming
    /// approaches one landscape instead of rescaling it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub lock_zoom: f64,
    /// The count's or the relief's range at the lock, `[lo, hi]`, pinned
    /// so the heights stop following the ground's own range as it zooms
    /// and streams. `lo > hi` when none was measured: the ground's own
    /// range stands in.
    #[serde(default = "default_terrain_lock_range", skip_serializing_if = "is_default_terrain_lock_range")]
    pub lock_range: [f32; 2],
}

fn default_terrain_height() -> f32 {
    0.06
}
fn is_default_terrain_height(v: &f32) -> bool {
    *v == default_terrain_height()
}
fn default_terrain_de_width() -> f32 {
    1.0 / 150.0
}
fn is_default_terrain_de_width(v: &f32) -> bool {
    *v == default_terrain_de_width()
}
fn is_default_terrain_tier(v: &RenderTier) -> bool {
    *v == RenderTier::default()
}
fn default_terrain_detail() -> f32 {
    1.0
}
fn default_solid_tier() -> RenderTier {
    RenderTier::Lit
}
fn is_default_solid_tier(v: &RenderTier) -> bool {
    *v == default_solid_tier()
}
fn is_default_terrain_detail(v: &f32) -> bool {
    *v == default_terrain_detail()
}
fn default_terrain_far() -> f32 {
    8.0
}
fn is_default_terrain_far(v: &f32) -> bool {
    *v == default_terrain_far()
}
fn default_terrain_haze() -> f32 {
    1.0
}
fn is_default_terrain_haze(v: &f32) -> bool {
    *v == default_terrain_haze()
}
/// 0.98 since config v5 (3d-look-and-tools P2; 0.7 before, which the
/// migration keeps for files that omitted it).
fn default_terrain_shadow() -> f32 {
    0.98
}
fn is_default_terrain_shadow(v: &f32) -> bool {
    *v == default_terrain_shadow()
}
fn default_terrain_shadow_sharpness() -> f32 {
    12.0
}
fn is_default_terrain_shadow_sharpness(v: &f32) -> bool {
    *v == default_terrain_shadow_sharpness()
}
fn default_terrain_occlusion() -> f32 {
    0.006
}
fn is_default_terrain_occlusion(v: &f32) -> bool {
    *v == default_terrain_occlusion()
}
fn is_default_terrain_source(v: &TerrainSource) -> bool {
    *v == TerrainSource::default()
}
fn default_terrain_lake_tint() -> [f32; 3] {
    [0.02, 0.05, 0.07]
}
fn is_default_terrain_lake_tint(v: &[f32; 3]) -> bool {
    *v == default_terrain_lake_tint()
}
fn default_terrain_lake_roughness() -> f32 {
    0.05
}
fn is_default_terrain_lake_roughness(v: &f32) -> bool {
    *v == default_terrain_lake_roughness()
}
fn is_default_terrain_interior(v: &TerrainInterior) -> bool {
    *v == TerrainInterior::default()
}
fn is_default_terrain_curve(v: &TerrainCurve) -> bool {
    *v == TerrainCurve::default()
}
fn default_terrain_prefilter() -> u32 {
    1
}
fn is_default_terrain_prefilter(v: &u32) -> bool {
    *v == default_terrain_prefilter()
}
fn default_terrain_lock_range() -> [f32; 2] {
    [1.0, 0.0]
}
fn is_default_terrain_lock_range(v: &[f32; 2]) -> bool {
    *v == default_terrain_lock_range()
}

impl Default for TerrainConfig {
    fn default() -> Self {
        TerrainConfig {
            enabled: false,
            source: TerrainSource::default(),
            height: default_terrain_height(),
            de_width: default_terrain_de_width(),
            detail: default_terrain_detail(),
            prefilter: default_terrain_prefilter(),
            softness: 0.0,
            smooth: false,
            far: default_terrain_far(),
            haze: default_terrain_haze(),
            tier: RenderTier::default(),
            interior: TerrainInterior::default(),
            curve: TerrainCurve::default(),
            invert: false,
            lake_tint: default_terrain_lake_tint(),
            lake_roughness: default_terrain_lake_roughness(),
            shadow: default_terrain_shadow(),
            shadow_sharpness: default_terrain_shadow_sharpness(),
            occlusion: default_terrain_occlusion(),
            target_lift: 0.0,
            locked: false,
            lock_zoom: 0.0,
            lock_range: default_terrain_lock_range(),
        }
    }
}

impl TerrainConfig {
    pub fn is_default(v: &TerrainConfig) -> bool {
        *v == TerrainConfig::default()
    }

    /// The pinned count or relief range, when the lock holds one.
    pub fn pinned_range(&self) -> Option<[f32; 2]> {
        let [lo, hi] = self.lock_range;
        (self.locked && lo <= hi && lo.is_finite() && hi.is_finite()).then_some(self.lock_range)
    }
}

impl EscapeConfig {
    /// What the terrain's lengths -- its height, the flanks' width,
    /// occlusion's reach, a hole's depth -- are fractions of, in current
    /// view widths: 1, or with the relief locked (H3), the lock's view
    /// width, `2^(zoom - lock_zoom)` current ones. Held within 2^±24 of
    /// the lock, where an f32 world still resolves the view.
    pub fn terrain_scale(&self) -> f64 {
        if !self.terrain.locked {
            return 1.0;
        }
        let d = self.zoom_log2 - self.terrain.lock_zoom;
        if d.is_finite() { d.clamp(-24.0, 24.0).exp2() } else { 1.0 }
    }

    /// Whether the iterate pass needs the derivative orbit: the
    /// relief's analytic slopes, or the terrain's distance.
    pub fn wants_derivative(&self) -> bool {
        self.shading.wants_derivative() || (self.terrain_active() && self.terrain.source == TerrainSource::Distance)
    }

    /// Whether the iterate pass draws a folding formula's creases: the
    /// relief's Creases field, or a terrain made of them. The pass then
    /// always runs whole -- the recolour shortcut, which has no formula
    /// code, cannot recompute them.
    pub fn wants_creases(&self) -> bool {
        self.shading.wants_creases() || (self.terrain_active() && self.terrain.source == TerrainSource::Creases)
    }

    /// Whether the 3D terrain is in force: switched on, for a formula
    /// that draws a plane (a solid IFS is already 3D, mode D's own),
    /// in a build that carries it. The gallery modules are built
    /// without the `terrain` feature (plan H2) and render a terrain
    /// config as its 2D picture, relief and all.
    pub fn terrain_active(&self) -> bool {
        #[cfg(feature = "terrain")]
        {
            self.terrain.enabled && crate::escape::ifs::get_ifs(&self.formula).is_none_or(|d| !d.solid)
        }
        #[cfg(not(feature = "terrain"))]
        {
            false
        }
    }
}

/// One entry of a formula file, as a config carries it: the language
/// it is written in, the entry's name, and the entry's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomSource {
    pub dialect: CustomDialect,
    pub entry: String,
    pub source: String,
    /// The user's variations the entry calls with `vary` (a Rhai entry;
    /// the KIFS plan, C1e), each as its source -- `entry` its name -- as
    /// it was when the entry was used: the entry compiles against these,
    /// so the config carries everything it needs. Flat: a variation one
    /// of them calls is listed here too.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variations: Vec<CustomSource>,
}

/// The language of a [`CustomSource`]: Ultra Fractal's (`.ufm`, `.ucl`),
/// or Fractint's (`.frm`), which differs in its numeric conventions; or
/// a Kalles Fraktaler-style hybrid, its `source` the hybrid's JSON
/// (`escape::lang::hybrid`), a formula only; or a Rhai formula
/// (`escape::lang::rhai`), the language's own since phase 4, one formula
/// to a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomDialect {
    #[default]
    UltraFractal,
    Fractint,
    Hybrid,
    Rhai,
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
            solid_tier: default_solid_tier(),
            max_iter: default_max_iter(),
            bailout: default_bailout(),
            damping_re: default_damping_re(),
            damping_im: 0.0,
            biomorph: BiomorphMode::Off,
            coloring: default_coloring(),
            formula_params: BTreeMap::new(),
            coloring_params: BTreeMap::new(),
            custom_formula: None,
            custom_coloring: None,
            lens: String::new(),
            lens_params: BTreeMap::new(),
            lens_amount: default_lens_amount(),
            transforms: Vec::new(),
            supersample: 1,
            downsample: DownsampleMode::Box,
            reference_period: None,
            shading: EscapeShading::default(),
            terrain: TerrainConfig::default(),
            path: PathTraceConfig::default(),
            contrast: EscapeContrast::default(),
            palette_map: PaletteMap::default(),
            layer: ColoringLayer::default(),
            texture: None,
            texture_overlay: TextureOverlay::default(),
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
        for m in [ContrastMode::Off, ContrastMode::AutoRange, ContrastMode::Flatten, ContrastMode::Equalize] {
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
            texture_where: TextureWhere::Steep,
            texture_angle: 60.0,
            texture_angle_soft: 5.0,
            model: ReliefModel::Lambert,
            elevation: 55.0,
            ambient: 0.3,
            stencil: SlopeStencil::Roberts,
            height_curve: HeightTransfer::Cos,
            height_pre: 4.0,
            height_post: 0.25,
            offset: 0.002,
            emboss: EmbossType::SmallestMagnitude,
            emboss_sections: 5,
            creases: CreaseSettings { pattern: CreasePattern::Random, decay: 0.8, flip: true },
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
        for f in [
            ShadingField::Smooth,
            ShadingField::Banded,
            ShadingField::Layer,
            ShadingField::Analytic,
            ShadingField::Offset,
            ShadingField::Embossed,
            ShadingField::Creases,
        ] {
            assert_eq!(shading_field_from_str(shading_field_to_str(f)), f);
        }
        for t in EmbossType::ALL {
            assert_eq!(EmbossType::from_name(t.as_str()), t);
        }
        for p in CreasePattern::ALL {
            assert_eq!(CreasePattern::from_name(p.as_str()), p);
        }
        for k in ShadingTexture::ALL {
            assert_eq!(ShadingTexture::from_str_or_default(k.as_str()), k);
        }
        for w in TextureWhere::ALL {
            assert_eq!(TextureWhere::from_name(w.as_str()), w);
        }
        for s in TerrainSource::ALL {
            assert_eq!(TerrainSource::from_str_or_default(s.as_str()), s);
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

    /// The crease settings are skipped while default -- a shading block
    /// that never drew creases does not mention them -- and each one set
    /// is written on its own.
    #[test]
    fn crease_settings_are_written_only_when_set() {
        let on = EscapeShading { enabled: true, field: ShadingField::Creases, ..EscapeShading::default() };
        let json = serde_json::to_string(&on).unwrap();
        assert!(json.contains("\"field\":\"creases\""), "{json}");
        assert!(!json.contains("\"creases\":{"), "default creases must not be written: {json}");
        let mut set = on.clone();
        set.creases.decay = 0.85;
        let json = serde_json::to_string(&set).unwrap();
        assert!(json.contains("\"creases\":{\"decay\":0.85}"), "{json}");
        let back: EscapeShading = serde_json::from_str(&json).unwrap();
        assert_eq!(back.creases, set.creases);
        // The uniform row: decay, pattern, flip as a sign, the reach.
        let s = CreaseSettings { pattern: CreasePattern::Alternate, decay: 0.7, flip: true };
        assert_eq!(s.to_gpu(false), [0.7, 1.0, -1.0, 0.0]);
        assert_eq!(s.to_gpu(true), [0.7, 1.0, -1.0, CREASE_TERRAIN_REACH]);
        // The relief and the terrain source share the code, and a terrain
        // made of creases asks for them whatever the relief draws.
        assert_eq!(TerrainSource::Creases.shade_flags(ShadingField::Smooth), ShadingField::Creases.to_gpu());
        let mut esc = EscapeConfig::default();
        assert!(!esc.wants_creases());
        esc.terrain.enabled = true;
        esc.terrain.source = TerrainSource::Creases;
        assert_eq!(esc.wants_creases(), esc.terrain_active());
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

    /// A complex formula parameter as a point in polar form (phase 4,
    /// step 8): `seed.r` and `seed.angle` read and set `seed.re` and
    /// `seed.im` through the manager, and undo as any edit.
    #[test]
    fn a_complex_parameter_reads_and_sets_as_a_point() {
        use crate::config::{ConfigManager, ConfigPath};
        let mut config = crate::config::FractalConfig::default();
        config.escape.formula_params.insert("seed.re".into(), 1.0);
        config.escape.formula_params.insert("seed.im".into(), 0.0);
        let mut mgr = ConfigManager::new(config);
        let get = |mgr: &ConfigManager, p: &str| f32::try_from(mgr.get_value(&ConfigPath::EscapeFormulaParam { param: p.into() }).unwrap()).unwrap();
        assert_eq!((get(&mgr, "seed.r"), get(&mgr, "seed.angle")), (1.0, 0.0));
        mgr.update_param(ConfigPath::EscapeFormulaParam { param: "seed.angle".into() }, 90.0f32.into()).unwrap();
        assert!(get(&mgr, "seed.re").abs() < 1e-6 && (get(&mgr, "seed.im") - 1.0).abs() < 1e-6);
        mgr.update_param(ConfigPath::EscapeFormulaParam { param: "seed.r".into() }, 2.0f32.into()).unwrap();
        assert!((get(&mgr, "seed.im") - 2.0).abs() < 1e-5 && (get(&mgr, "seed.angle") - 90.0).abs() < 1e-3);
        mgr.undo().unwrap();
        mgr.undo().unwrap();
        assert!((get(&mgr, "seed.re") - 1.0).abs() < 1e-6 && get(&mgr, "seed.im").abs() < 1e-6, "undone");
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
            ConfigPath::EscapeCustomFormula,
            ConfigPath::EscapeCustomColoring,
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

#[cfg(test)]
mod transform_id_tests {
    use super::*;

    /// The id rides the list's wire form and nothing else: a file never
    /// holds it, a round trip keeps it, and an entry without one or
    /// repeating one gets a fresh one.
    #[test]
    fn a_transformations_id_rides_the_wire_and_never_the_file() {
        let a = EscapeTransform::new(TransformStage::Plane, "julian", 0.5);
        let b = EscapeTransform::new(TransformStage::Orbit, "swirl", 1.0);
        assert_ne!(a.id, b.id);
        let list = vec![a.clone(), b.clone()];
        let back = transforms_from_wire(&transforms_to_wire(&list)).unwrap();
        assert_eq!(back.iter().map(|t| t.id).collect::<Vec<_>>(), vec![a.id, b.id]);
        assert_eq!(back, list);

        let file = serde_json::to_string(&list).unwrap();
        assert!(!file.contains("\"id\""), "a file holds the id: {file}");
        let read = transforms_from_wire(&file).unwrap();
        assert!(read.iter().all(|t| t.id != 0) && read[0].id != read[1].id, "fresh ids for a file's entries");

        let twice = transforms_to_wire(&[a.clone(), a.clone()]);
        let read = transforms_from_wire(&twice).unwrap();
        assert_eq!(read[0].id, a.id);
        assert_ne!(read[1].id, a.id, "a repeated id is replaced");
        assert!(transforms_from_wire("").unwrap().is_empty());

        // Equality is the content's, as an effect's.
        let mut c = a.clone();
        c.id = 0;
        assert_eq!(c, a);
    }
}
