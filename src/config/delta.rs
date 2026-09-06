/// Delta-based configuration change tracking
///
/// This module defines the core types for tracking configuration changes:
/// - ConfigPath: Identifies any parameter in FractalConfig
/// - ConfigValue: Type-safe container for parameter values
/// - ConfigDelta: Records a single parameter change (old → new)
/// - ConfigChange: Batch of deltas (single undo point)
/// - UpdateType: What kind of update is needed for a change

use crate::scene::palette::{Palette, ColorMode, PathCaptureMode, PathMapStyle, PathTrackingMode};
use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::transforms::RenderMode;
use std::fmt::{self, Display, Formatter};
use web_time::Instant;

/// Identifies a specific parameter in the configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigPath {
    // ===== View parameters (no fractal recalc needed) =====
    Zoom,
    Pan,  // Combined PanX and PanY into single Vec2 value
    PanX, // Separate X component for animation
    PanY, // Separate Y component for animation
    Rotation,
    CameraRotationX,
    CameraRotationY,
    /// JWildfire/Apophysis bank angle (Y-axis rotation in the
    /// 4-angle camera matrix). Stored in radians on
    /// `FractalConfig::camera_bank`.
    CameraBank,
    CameraX,
    CameraY,
    CameraZ,
    DofFocusDistance,
    DofBlurStrength,
    FogStrength,
    FogStart,
    FilterRadius,
    FilterBlurEdges,

    // ===== Tone mapping (no iteration reset needed) =====
    Exposure,
    Gamma,
    GammaThreshold,
    Brightness,
    Vibrancy,
    WhiteLevel,
    Saturation,
    HueShift,
    AlphaBlendLow,
    AlphaBlendHigh,
    DensityScale,
    TonemapMode,
    HighlightMode,
    TonemapCurve,
    UseCurve,
    // Levels controls (density-to-opacity mapping)
    LevelsEnabled,
    LevelsLow,
    LevelsHigh,
    LevelsGamma,

    // ===== Color (no iteration reset, just color buffer update) =====
    ColorMode,
    PathMapStyle,  // Prefix or Suffix coloring for PathMap mode
    PathCaptureMode,  // FirstHit, FirstAfterBurnIn, or LastHit
    PathTrackingMode,  // First (first 32 iterations) or Recent (rolling window of 32 most recent)
    PaletteIndex,
    Palette, // Embedded palette data (custom palettes)
    PaletteRotation,
    PaletteSize, // Palette texture resolution (256-4096)
    PaletteSqueeze, // Palette squeeze: 1.0 = normal, >1 = repeat, <1 = portion
    PaletteSqueezeMode, // Linear vs Geometric squeeze algorithm
    PaletteSqueezeFalloff, // Geometric squeeze octave ratio (typically ~0.5)
    PaletteLogStrength, // Exponential redistribution of the squeezed palette
    PaletteReverse, // Flip the resulting palette lookup table (toggle)
    SpeedFactor,
    BackgroundColor,
    BackgroundColorR, // Separate R component for animation
    BackgroundColorG, // Separate G component for animation
    BackgroundColorB, // Separate B component for animation

    // ===== Rendering settings (affects iteration speed/quality) =====
    BlendFactor,
    UseDynamicBlend,
    MaxIterations,
    DeterministicRng,

    // ===== Transform-level changes (require iteration reset) =====
    TransformCount,
    TransformWeight { index: usize },
    TransformColor { index: usize },
    TransformColorSpeed { index: usize },
    TransformOpacity { index: usize },
    TransformDirectColor { index: usize },
    TransformAffine { index: usize, param: AffineParam },
    TransformVariation { index: usize, variation: String },
    TransformVariationParam {
        index: usize,
        variation: String,
        param: String,
    },
    /// JWildfire `fx_priority` phase override for an `Any`-phase variation
    /// (raw priority `<0` pre / `0` main / `>0` post). Stored sparsely in
    /// `Transform::variation_priorities` — see `docs/projects/jwf-features.md`.
    TransformVariationPriority { index: usize, variation: String },
    /// The transform's variation dispatch order (`variation_order`). Carries
    /// the whole ordered name list; used by the UI reorder controls. See
    /// `Transform::variation_order`.
    TransformVariationOrder { index: usize },
    /// Post-affine enabled flag for a transform
    TransformPostAffineEnabled { index: usize },
    /// Post-affine transformation parameter for a transform
    TransformPostAffine { index: usize, param: AffineParam },

    // JWildfire-extension plane affines. Each path identifies one of
    // the six positions in a `[f32; 6]` plane-coefficient array
    // (`position` = 0..6, JWildfire's XML positional order matching
    // `yz_coefs[position]`). Normal pool only for now — Linked/Final
    // pool variants can be added when the UI grows past Normal
    // editing. The round-trip on XML/JSON already covers all pools;
    // this is just about which transforms get UI controls.
    TransformYzCoefs { index: usize, position: u8 },
    TransformZxCoefs { index: usize, position: u8 },
    TransformYzPostCoefs { index: usize, position: u8 },
    TransformZxPostCoefs { index: usize, position: u8 },
    /// High-level transform origin X (translate X)
    /// Computed from affine: origin_x = e
    TransformOriginX { index: usize },
    /// High-level transform origin Y (translate Y)
    /// Computed from affine: origin_y = -f (Apophysis convention)
    TransformOriginY { index: usize },
    /// High-level transform rotation (angle in radians)
    /// Computed from: atan2(b, a)
    TransformRotation { index: usize },
    /// High-level transform scale (uniform scaling)
    /// Computed from: sqrt(a² + b²)
    TransformScale { index: usize },

    // High-level ops on the post-affine half of a normal-pool
    // transform. Same decomposition as the pre-affine versions
    // above, applied to post_a..post_f.
    TransformPostAffineOriginX { index: usize },
    TransformPostAffineOriginY { index: usize },
    TransformPostAffineRotation { index: usize },
    TransformPostAffineScale { index: usize },

    // ===== Linked Transform pool (require iteration reset) =====
    // Linked transforms run sequentially after a normal transform's
    // variations and feed forward into the next iteration. Pool members
    // are referenced by index; multiple normals can share via
    // attachment lists. See per-transform-linked-and-final.md.
    LinkedTransformAffine { index: usize, param: AffineParam },
    LinkedTransformPostAffineEnabled { index: usize },
    LinkedTransformPostAffine { index: usize, param: AffineParam },
    // JWildfire-extension plane affines — Linked pool counterparts
    // of the Normal-pool `Transform{Yz,Zx}{,Post}Coefs` variants.
    LinkedTransformYzCoefs { index: usize, position: u8 },
    LinkedTransformZxCoefs { index: usize, position: u8 },
    LinkedTransformYzPostCoefs { index: usize, position: u8 },
    LinkedTransformZxPostCoefs { index: usize, position: u8 },
    LinkedTransformVariation { index: usize, variation: String },
    LinkedTransformVariationParam {
        index: usize,
        variation: String,
        param: String,
    },
    /// fx_priority phase override — Linked pool counterpart.
    LinkedTransformVariationPriority { index: usize, variation: String },
    /// Variation order — Linked pool counterpart.
    LinkedTransformVariationOrder { index: usize },
    // High-level pre-affine ops on a linked-pool transform.
    LinkedTransformOriginX { index: usize },
    LinkedTransformOriginY { index: usize },
    LinkedTransformRotation { index: usize },
    LinkedTransformScale { index: usize },
    // High-level post-affine ops on a linked-pool transform.
    LinkedTransformPostAffineOriginX { index: usize },
    LinkedTransformPostAffineOriginY { index: usize },
    LinkedTransformPostAffineRotation { index: usize },
    LinkedTransformPostAffineScale { index: usize },

    // ===== Final Transform pool (require iteration reset) =====
    // Final transforms run sequentially after the Linked chain to
    // shape the plotted point only (output not fed forward — pure
    // filter). Pool members referenced by index.
    //
    // Legacy `FinalTransform*` variants (no index) lived here before
    // the per-pool model — they routed to `final_transforms[0]` and
    // existed to keep older animation tracks loadable. Removed in
    // Phase 9 of the unified-render-pipeline branch; the migration
    // shim in `from_string_key` keeps any saved tracks targeting
    // the legacy `FinalTransform.{field}` strings working by
    // mapping them to index 0.
    FinalTransformAffine { index: usize, param: AffineParam },
    FinalTransformPostAffineEnabled { index: usize },
    FinalTransformPostAffine { index: usize, param: AffineParam },
    // JWildfire-extension plane affines — Final pool counterparts.
    FinalTransformYzCoefs { index: usize, position: u8 },
    FinalTransformZxCoefs { index: usize, position: u8 },
    FinalTransformYzPostCoefs { index: usize, position: u8 },
    FinalTransformZxPostCoefs { index: usize, position: u8 },
    FinalTransformVariation { index: usize, variation: String },
    FinalTransformVariationParam {
        index: usize,
        variation: String,
        param: String,
    },
    /// fx_priority phase override — Final pool counterpart.
    FinalTransformVariationPriority { index: usize, variation: String },
    /// Variation order — Final pool counterpart.
    FinalTransformVariationOrder { index: usize },
    // High-level pre-affine ops on a final-pool transform.
    FinalTransformOriginX { index: usize },
    FinalTransformOriginY { index: usize },
    FinalTransformRotation { index: usize },
    FinalTransformScale { index: usize },
    // High-level post-affine ops on a final-pool transform.
    FinalTransformPostAffineOriginX { index: usize },
    FinalTransformPostAffineOriginY { index: usize },
    FinalTransformPostAffineRotation { index: usize },
    FinalTransformPostAffineScale { index: usize },

    // ===== Flame-level (require iteration reset) =====
    RenderMode,
    PerspectiveStrength,
    DepthDensityCompensation,
    FarDensityFade,
    SolidStrength,
    SurfaceThickness,
    SolidShadowStrength,
    // Solid rendering Phase 1: deferred lighting (shade pass).
    ShadingStrength,
    SolidAmbient,
    SolidDiffuse,
    SolidSpecular,
    SolidShininess,
    SsaoStrength,
    SsaoRadius,
    NormalSmoothing,
    GapFill,
    /// Enable/disable one of the 4 shade-pass lights.
    SolidLightEnabled { index: usize },
    /// Parameter of one shade-pass light: "azimuth", "elevation",
    /// "intensity", "color_r", "color_g", "color_b".
    SolidLightParam { index: usize, param: String },
    FarDensityFadeStart,
    /// Xaos (chaos) weight for transition from src transform to dst transform
    /// Modifies the probability of selecting dst when coming from src
    Xaos { src: usize, dst: usize },
    /// Solo transform index (0-indexed). When Some(n), only transform n is active,
    /// all others effectively have weight 0. Used for debugging.
    /// Matches Apophysis XML: soloxform="N"
    SoloTransform,

    // ===== Post-symmetry (plot-time density replication) =====
    // Flipping `PostSymmetryType` between None and a non-None value
    // changes the `HAS_POST_SYMMETRY` shader gate and forces a shader
    // rebuild via ShaderConstants. The geometry fields below only
    // update the uniform (no recompile).
    PostSymmetryType,
    PostSymmetryOrder,
    PostSymmetryCenterX,
    PostSymmetryCenterY,
    PostSymmetryDistance,
    PostSymmetryRotation,

    /// JWildfire's `preserve_z` flag — when true, the chaos game's
    /// Z carries across iterations; when false (default), Z is reset
    /// each iteration. Flipping it changes the `FLATTEN_Z_PER_ITER`
    /// shader gate and forces a shader rebuild via ShaderConstants.
    PreserveZ,

    // ===== Effects (post-processing, no iteration reset needed) =====
    /// Enable/disable a density effect
    DensityEffectEnabled { index: usize },
    /// Parameter value for a density effect
    DensityEffectParam { index: usize, param: String },
    /// Enable/disable a color effect
    ColorEffectEnabled { index: usize },
    /// Parameter value for a color effect
    ColorEffectParam { index: usize, param: String },
    /// Add a new color effect
    AddColorEffect { effect_type: String },
    /// Remove a color effect by index
    RemoveColorEffect { index: usize },
    /// Add a new density effect
    AddDensityEffect { effect_type: String },
    /// Remove a density effect by index
    RemoveDensityEffect { index: usize },

    // ===== Escape-time (fragment mode) — config.escape.* =====
    // All of these route to UpdateType::EscapeRerender: the fragment
    // renderer re-renders whole frames, so there is no flame-style
    // reset/accumulate distinction to encode per path.
    /// Formula registry name (`config.escape.formula`).
    EscapeFormula,
    /// Julia toggle (parameter plane vs dynamical plane).
    EscapeJulia,
    /// Julia seed, real part.
    EscapeJuliaRe,
    /// Julia seed, imaginary part.
    EscapeJuliaIm,
    /// View center, exact decimal string (deep-zoom payload — undoable,
    /// deliberately NOT animatable; see the plan's open questions).
    EscapeCenterRe,
    EscapeCenterIm,
    /// Zoom exponent (log2). Stored f64; travels as ConfigValue::Float,
    /// whose f32 mantissa (~6e-5 granularity at exponent 1000) is far
    /// below anything a slider or track produces. Phase 4 revisits if a
    /// deep dive ever needs finer undo steps.
    EscapeZoomLog2,
    /// View rotation, radians.
    EscapeRotation,
    /// Per-pixel iteration ceiling.
    EscapeMaxIter,
    /// Supersampling factor (1 = off): N× per axis + box downsample.
    EscapeSupersample,
    EscapeDownsample,
    /// Reference-orbit period hint (0 = none). Verified before use.
    EscapeReferencePeriod,

    // ---------------------------------------------------------------
    // Simulation mode. Routed to one of three update types by how much
    // of the run survives the change: SimRerender keeps the field,
    // SimResample interpolates it into a new grid, SimReseed restarts.
    // ---------------------------------------------------------------
    /// Which model steps, by registry name.
    SimModel,
    /// Which colouring maps the field, by registry name.
    SimColoring,
    /// Whether the grid is Fixed or bound to the viewport.
    SimGridMode,
    /// Fixed-grid width in cells.
    SimGridWidth,
    /// Fixed-grid height in cells.
    SimGridHeight,
    /// Bound-grid cells per output pixel.
    SimGridScale,
    /// Seed for the initial field and every stochastic model.
    SimSeed,
    /// Which initial shape (`noise`, `blob`, `blobs`, `ring`, `line`,
    /// `center`).
    SimInitKind,
    /// Noise init amplitude.
    SimInitAmplitude,
    /// Blob/blobs/ring radius, in cells.
    SimInitRadius,
    /// How many blobs the `blobs` init places.
    SimInitCount,
    /// **The step count, and the animation target that drives the
    /// simulation's own progression.**
    ///
    /// A still is the state at exactly this many steps from the seed,
    /// so animating it animates the run: the state at time *t* is
    /// `round(track(t))` steps, which keeps a frame a function of its
    /// time rather than of how many frames preceded it (master plan
    /// D5b). A track that decreases costs a reseed and re-run, which is
    /// the price of a rule that cannot be stepped backwards.
    SimSteps,
    /// Free-running speed for the interactive Run button. NOT an
    /// animation target — the timeline uses `SimSteps`.
    SimStepsPerFrame,
    /// Model time step, where the model has one.
    SimDt,
    /// What a step reads outside the grid.
    SimBoundary,
    /// The warp stage's per-step rates (pipeline section 4.1): scale
    /// about the centre, radians, cells, cells, and the swirl's rim
    /// rate. All animatable -- a ramp on the zoom is space beginning
    /// to expand.
    SimWarpZoom,
    SimWarpRotation,
    SimWarpPanX,
    SimWarpPanY,
    SimWarpFlow,
    /// How the warp samples: bilinear or nearest. Not animatable.
    SimWarpFilter,
    /// The matte: which cells are figure and which are background
    /// (`SimMatte`). The channel and the direction are choices; the
    /// cutoff and the softness are quantities and animate -- a cutoff
    /// sweeping down is the figure growing into the background.
    SimMatteChannel,
    SimMatteCutoff,
    SimMatteSoftness,
    SimMatteInvert,
    /// Threshold or distance field (`SimMatteEdge`). A choice, not
    /// animatable.
    SimMatteEdge,
    /// Resolve filter when the grid is smaller than the output.
    SimUpscale,
    /// Resolve filter when the grid is larger than the output.
    SimDownscale,
    /// One model parameter, by name.
    SimModelParam { param: String },
    /// One colouring parameter, by name.
    SimColoringParam { param: String },
    /// Escape radius squared.
    EscapeBailout,
    /// Mann-iteration damping α (complex): `z ← (1−α)z + α·f(z)`.
    /// `1 + 0i` = plain iteration.
    EscapeDampingRe,
    EscapeDampingIm,
    /// Biomorph classification axis, as its wire string
    /// (`"off"`/`"re"`/`"im"`).
    EscapeBiomorph,
    // Relief shading — a lit-surface layer over the coloring's output.
    // Ten paths rather than one struct-valued path because the whole
    // undo/redo and scripting surface is keyed on leaf parameters.
    EscapeShadingEnabled,
    EscapeShadingLightAngle,
    EscapeShadingHeight,
    EscapeShadingField,
    EscapeContrastMode,
    EscapeContrastClip,
    EscapeContrastStrength,
    EscapeContrastTurns,
    EscapeShadingShadowColor,
    EscapeShadingShadowStrength,
    EscapeShadingShadowBlend,
    EscapeShadingHighlightColor,
    EscapeShadingHighlightStrength,
    EscapeShadingHighlightBlend,
    EscapeShadingSoftness,
    EscapeShadingTextureKind,
    EscapeShadingTextureStrength,
    EscapeShadingTextureScale,
    /// Coloring registry name.
    EscapeColoring,
    /// One parameter of the ACTIVE formula, by name — keyed like
    /// `DensityEffectParam`, so formulas never need per-name variants.
    EscapeFormulaParam { param: String },
    /// One parameter of the active coloring, same shape.
    EscapeColoringParam { param: String },

    // ===== System Settings (device-specific, not tracked for undo) =====
    SystemIterationsPerThread,
    SystemBurnIn,
    SystemOrbitCacheMb,
    SystemVsyncEnabled,
    SystemTargetFps,
    SystemFlyMouseSensitivity,
    SystemFlyMoveSpeed,
    SystemFlySprintMultiplier,
    SystemFlyInvertY,
    SystemFlyCameraMode,
    SystemExportWidth,
    SystemExportHeight,
    SystemPngStripMetadata,
    SystemLanguage,
    SystemShowHelpOnStartup,
}

/// Affine transformation parameter (a, b, c, d, e, f, g)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffineParam {
    A,
    B,
    C,
    D,
    E,
    F,
    G, // Z offset for 3D mode
}

/// Identifies a specific transform across the three pools.
/// Used by reusable UI render fns to construct the right ConfigPath
/// variant and read from the right pool when displaying / editing
/// affine, post-affine, variations, and variation_params.
///
/// See `docs/projects/per-transform-linked-and-final.md`.
/// Which transform pool a path-or-action targets.
/// Use `TransformRef` when an index is also needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TransformKind {
    Normal,
    Linked,
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TransformRef {
    Normal(usize),
    Linked(usize),
    Final(usize),
}

impl TransformKind {
    pub fn at(self, index: usize) -> TransformRef {
        match self {
            TransformKind::Normal => TransformRef::Normal(index),
            TransformKind::Linked => TransformRef::Linked(index),
            TransformKind::Final => TransformRef::Final(index),
        }
    }
}

impl TransformRef {
    pub fn index(&self) -> usize {
        match self {
            TransformRef::Normal(i) | TransformRef::Linked(i) | TransformRef::Final(i) => *i,
        }
    }

    pub fn kind(&self) -> TransformKind {
        match self {
            TransformRef::Normal(_) => TransformKind::Normal,
            TransformRef::Linked(_) => TransformKind::Linked,
            TransformRef::Final(_) => TransformKind::Final,
        }
    }

    /// Short tag used to disambiguate egui id_salts and log strings across pools.
    pub fn pool_kind(&self) -> &'static str {
        match self {
            TransformRef::Normal(_) => "normal",
            TransformRef::Linked(_) => "linked",
            TransformRef::Final(_) => "final",
        }
    }

    /// Look up the corresponding transform in the flame.
    pub fn get<'a>(&self, flame: &'a crate::scene::transforms::Flame)
        -> Option<&'a crate::scene::transforms::Transform>
    {
        match self {
            TransformRef::Normal(i) => flame.transforms.get(*i),
            TransformRef::Linked(i) => flame.linked_transforms.get(*i),
            TransformRef::Final(i) => flame.final_transforms.get(*i),
        }
    }

    /// Mutable lookup of the corresponding transform in the flame.
    pub fn get_mut<'a>(&self, flame: &'a mut crate::scene::transforms::Flame)
        -> Option<&'a mut crate::scene::transforms::Transform>
    {
        match self {
            TransformRef::Normal(i) => flame.transforms.get_mut(*i),
            TransformRef::Linked(i) => flame.linked_transforms.get_mut(*i),
            TransformRef::Final(i) => flame.final_transforms.get_mut(*i),
        }
    }

    pub fn affine_path(&self, param: AffineParam) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformAffine { index: *i, param },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformAffine { index: *i, param },
            TransformRef::Final(i) => ConfigPath::FinalTransformAffine { index: *i, param },
        }
    }

    pub fn post_affine_enabled_path(&self) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformPostAffineEnabled { index: *i },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformPostAffineEnabled { index: *i },
            TransformRef::Final(i) => ConfigPath::FinalTransformPostAffineEnabled { index: *i },
        }
    }

    pub fn post_affine_path(&self, param: AffineParam) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformPostAffine { index: *i, param },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformPostAffine { index: *i, param },
            TransformRef::Final(i) => ConfigPath::FinalTransformPostAffine { index: *i, param },
        }
    }

    pub fn variation_path(&self, variation: String) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformVariation { index: *i, variation },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformVariation { index: *i, variation },
            TransformRef::Final(i) => ConfigPath::FinalTransformVariation { index: *i, variation },
        }
    }

    pub fn variation_priority_path(&self, variation: String) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformVariationPriority { index: *i, variation },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformVariationPriority { index: *i, variation },
            TransformRef::Final(i) => ConfigPath::FinalTransformVariationPriority { index: *i, variation },
        }
    }

    pub fn variation_order_path(&self) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformVariationOrder { index: *i },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformVariationOrder { index: *i },
            TransformRef::Final(i) => ConfigPath::FinalTransformVariationOrder { index: *i },
        }
    }

    pub fn variation_param_path(&self, variation: String, param: String) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformVariationParam {
                index: *i, variation, param,
            },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformVariationParam {
                index: *i, variation, param,
            },
            TransformRef::Final(i) => ConfigPath::FinalTransformVariationParam {
                index: *i, variation, param,
            },
        }
    }

    /// Build the right ConfigPath variant for a JWildfire YZ-plane
    /// pre-affine coefficient on whichever pool this transform lives
    /// in. The position is the 0..6 index into the [f32; 6] array
    /// (JWildfire's XML positional order).
    pub fn yz_coefs_path(&self, position: u8) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformYzCoefs { index: *i, position },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformYzCoefs { index: *i, position },
            TransformRef::Final(i) => ConfigPath::FinalTransformYzCoefs { index: *i, position },
        }
    }

    /// ZX-plane pre-affine path. See [`Self::yz_coefs_path`].
    pub fn zx_coefs_path(&self, position: u8) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformZxCoefs { index: *i, position },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformZxCoefs { index: *i, position },
            TransformRef::Final(i) => ConfigPath::FinalTransformZxCoefs { index: *i, position },
        }
    }

    /// YZ-plane post-affine path. See [`Self::yz_coefs_path`].
    pub fn yz_post_coefs_path(&self, position: u8) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformYzPostCoefs { index: *i, position },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformYzPostCoefs { index: *i, position },
            TransformRef::Final(i) => ConfigPath::FinalTransformYzPostCoefs { index: *i, position },
        }
    }

    /// ZX-plane post-affine path. See [`Self::yz_coefs_path`].
    pub fn zx_post_coefs_path(&self, position: u8) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformZxPostCoefs { index: *i, position },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformZxPostCoefs { index: *i, position },
            TransformRef::Final(i) => ConfigPath::FinalTransformZxPostCoefs { index: *i, position },
        }
    }
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            // View
            ConfigPath::Zoom => write!(f, "Zoom"),
            ConfigPath::Pan => write!(f, "Pan"),
            ConfigPath::PanX => write!(f, "Pan X"),
            ConfigPath::PanY => write!(f, "Pan Y"),
            ConfigPath::Rotation => write!(f, "Rotation"),
            ConfigPath::CameraRotationX => write!(f, "Camera Pitch"),
            ConfigPath::CameraRotationY => write!(f, "Camera Yaw"),
            ConfigPath::CameraBank => write!(f, "Camera Bank"),
            ConfigPath::CameraX => write!(f, "Camera X"),
            ConfigPath::CameraY => write!(f, "Camera Y"),
            ConfigPath::CameraZ => write!(f, "Camera Z"),
            ConfigPath::DofFocusDistance => write!(f, "DOF Focus Distance"),
            ConfigPath::DofBlurStrength => write!(f, "DOF Blur Strength"),
            ConfigPath::FogStrength => write!(f, "Fog Strength"),
            ConfigPath::FogStart => write!(f, "Fog Start"),
            ConfigPath::FilterRadius => write!(f, "Spatial Filter"),
            ConfigPath::FilterBlurEdges => write!(f, "Blur Edges"),

            // Tone mapping
            ConfigPath::Exposure => write!(f, "Exposure"),
            ConfigPath::Gamma => write!(f, "Gamma"),
            ConfigPath::GammaThreshold => write!(f, "Gamma Threshold"),
            ConfigPath::Brightness => write!(f, "Brightness"),
            ConfigPath::Vibrancy => write!(f, "Vibrancy"),
            ConfigPath::WhiteLevel => write!(f, "Highlights"),
            ConfigPath::Saturation => write!(f, "Saturation"),
            ConfigPath::HueShift => write!(f, "Hue Shift"),
            ConfigPath::AlphaBlendLow => write!(f, "Alpha Blend Low"),
            ConfigPath::AlphaBlendHigh => write!(f, "Alpha Blend High"),
            ConfigPath::DensityScale => write!(f, "Density Scale"),
            ConfigPath::TonemapMode => write!(f, "Tonemap Mode"),
            ConfigPath::HighlightMode => write!(f, "Highlight Mode"),
            ConfigPath::TonemapCurve => write!(f, "Tone Curve"),
            ConfigPath::UseCurve => write!(f, "Use Tone Curve"),
            ConfigPath::LevelsEnabled => write!(f, "Levels Enabled"),
            ConfigPath::LevelsLow => write!(f, "Levels Low"),
            ConfigPath::LevelsHigh => write!(f, "Levels High"),
            ConfigPath::LevelsGamma => write!(f, "Levels Midtones"),

            // Color
            ConfigPath::ColorMode => write!(f, "Color Mode"),
            ConfigPath::PathMapStyle => write!(f, "PathMap Style"),
            ConfigPath::PathCaptureMode => write!(f, "PathMap Capture Mode"),
            ConfigPath::PathTrackingMode => write!(f, "PathMap Tracking Mode"),
            ConfigPath::PaletteIndex => write!(f, "Palette"),
            ConfigPath::Palette => write!(f, "Palette Data"),
            ConfigPath::PaletteRotation => write!(f, "Palette Rotation"),
            ConfigPath::PaletteSize => write!(f, "Palette Size"),
            ConfigPath::PaletteSqueeze => write!(f, "Palette Squeeze"),
            ConfigPath::PaletteSqueezeMode => write!(f, "Palette Squeeze Mode"),
            ConfigPath::PaletteSqueezeFalloff => write!(f, "Palette Squeeze Falloff"),
            ConfigPath::PaletteLogStrength => write!(f, "Palette Log Strength"),
            ConfigPath::PaletteReverse => write!(f, "Palette Reverse"),
            ConfigPath::SpeedFactor => write!(f, "Speed Blend Factor"),
            ConfigPath::BackgroundColor => write!(f, "Background Color"),
            ConfigPath::BackgroundColorR => write!(f, "Background Red"),
            ConfigPath::BackgroundColorG => write!(f, "Background Green"),
            ConfigPath::BackgroundColorB => write!(f, "Background Blue"),

            // Rendering
            ConfigPath::BlendFactor => write!(f, "Blend Factor"),
            ConfigPath::UseDynamicBlend => write!(f, "Use Dynamic Blend"),
            ConfigPath::MaxIterations => write!(f, "Max Iterations"),
            ConfigPath::DeterministicRng => write!(f, "Deterministic RNG"),

            // Transforms
            ConfigPath::TransformCount => write!(f, "Transform Count"),
            ConfigPath::TransformWeight { index } => {
                write!(f, "Transform {} → Weight", index + 1)
            }
            ConfigPath::TransformColor { index } => {
                write!(f, "Transform {} → Color", index + 1)
            }
            ConfigPath::TransformColorSpeed { index } => {
                write!(f, "Transform {} → Color Speed", index + 1)
            }
            ConfigPath::TransformOpacity { index } => {
                write!(f, "Transform {} → Opacity", index + 1)
            }
            ConfigPath::TransformDirectColor { index } => {
                write!(f, "Transform {} → Direct Color", index + 1)
            }
            ConfigPath::TransformAffine { index, param } => {
                write!(f, "Transform {} → Affine {:?}", index + 1, param)
            }
            ConfigPath::TransformVariation { index, variation } => {
                write!(f, "Transform {} → {} variation", index + 1, variation)
            }
            ConfigPath::TransformVariationPriority { index, variation } => {
                write!(f, "Transform {} → {} phase", index + 1, variation)
            }
            ConfigPath::TransformVariationOrder { index }
            | ConfigPath::LinkedTransformVariationOrder { index }
            | ConfigPath::FinalTransformVariationOrder { index } => {
                write!(f, "Transform {} → Variation Order", index + 1)
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                write!(
                    f,
                    "Transform {} → {} → {}",
                    index + 1,
                    variation,
                    param
                )
            }
            ConfigPath::TransformOriginX { index } => {
                write!(f, "Transform {} → Origin X", index + 1)
            }
            ConfigPath::TransformOriginY { index } => {
                write!(f, "Transform {} → Origin Y", index + 1)
            }
            ConfigPath::TransformRotation { index } => {
                write!(f, "Transform {} → Rotation", index + 1)
            }
            ConfigPath::TransformScale { index } => {
                write!(f, "Transform {} → Scale", index + 1)
            }
            ConfigPath::TransformPostAffineEnabled { index } => {
                write!(f, "Transform {} → Post-Affine Enabled", index + 1)
            }
            ConfigPath::TransformPostAffineOriginX { index } => {
                write!(f, "Transform {} → Post-Affine Origin X", index + 1)
            }
            ConfigPath::TransformPostAffineOriginY { index } => {
                write!(f, "Transform {} → Post-Affine Origin Y", index + 1)
            }
            ConfigPath::TransformPostAffineRotation { index } => {
                write!(f, "Transform {} → Post-Affine Rotation", index + 1)
            }
            ConfigPath::TransformPostAffineScale { index } => {
                write!(f, "Transform {} → Post-Affine Scale", index + 1)
            }
            ConfigPath::TransformPostAffine { index, param } => {
                write!(f, "Transform {} → Post-Affine {:?}", index + 1, param)
            }
            ConfigPath::TransformYzCoefs { index, position } => {
                write!(f, "Transform {} → YZ Coef [{}]", index + 1, position)
            }
            ConfigPath::TransformZxCoefs { index, position } => {
                write!(f, "Transform {} → ZX Coef [{}]", index + 1, position)
            }
            ConfigPath::TransformYzPostCoefs { index, position } => {
                write!(f, "Transform {} → YZ Post Coef [{}]", index + 1, position)
            }
            ConfigPath::TransformZxPostCoefs { index, position } => {
                write!(f, "Transform {} → ZX Post Coef [{}]", index + 1, position)
            }

            // Linked Transform pool
            ConfigPath::LinkedTransformAffine { index, param } => {
                write!(f, "Linked Transform {} → Affine {:?}", index + 1, param)
            }
            ConfigPath::LinkedTransformPostAffineEnabled { index } => {
                write!(f, "Linked Transform {} → Post-Affine Enabled", index + 1)
            }
            ConfigPath::LinkedTransformPostAffine { index, param } => {
                write!(f, "Linked Transform {} → Post-Affine {:?}", index + 1, param)
            }
            ConfigPath::LinkedTransformYzCoefs { index, position } => {
                write!(f, "Linked Transform {} → YZ Coef [{}]", index + 1, position)
            }
            ConfigPath::LinkedTransformZxCoefs { index, position } => {
                write!(f, "Linked Transform {} → ZX Coef [{}]", index + 1, position)
            }
            ConfigPath::LinkedTransformYzPostCoefs { index, position } => {
                write!(f, "Linked Transform {} → YZ Post Coef [{}]", index + 1, position)
            }
            ConfigPath::LinkedTransformZxPostCoefs { index, position } => {
                write!(f, "Linked Transform {} → ZX Post Coef [{}]", index + 1, position)
            }
            ConfigPath::LinkedTransformVariation { index, variation } => {
                write!(f, "Linked Transform {} → {} variation", index + 1, variation)
            }
            ConfigPath::LinkedTransformVariationPriority { index, variation } => {
                write!(f, "Linked Transform {} → {} phase", index + 1, variation)
            }
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
                write!(f, "Linked Transform {} → {} → {}", index + 1, variation, param)
            }
            ConfigPath::LinkedTransformOriginX { index } => {
                write!(f, "Linked Transform {} → Origin X", index + 1)
            }
            ConfigPath::LinkedTransformOriginY { index } => {
                write!(f, "Linked Transform {} → Origin Y", index + 1)
            }
            ConfigPath::LinkedTransformRotation { index } => {
                write!(f, "Linked Transform {} → Rotation", index + 1)
            }
            ConfigPath::LinkedTransformScale { index } => {
                write!(f, "Linked Transform {} → Scale", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineOriginX { index } => {
                write!(f, "Linked Transform {} → Post-Affine Origin X", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineOriginY { index } => {
                write!(f, "Linked Transform {} → Post-Affine Origin Y", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineRotation { index } => {
                write!(f, "Linked Transform {} → Post-Affine Rotation", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineScale { index } => {
                write!(f, "Linked Transform {} → Post-Affine Scale", index + 1)
            }

            // Final Transform pool
            ConfigPath::FinalTransformAffine { index, param } => {
                write!(f, "Final Transform {} → Affine {:?}", index + 1, param)
            }
            ConfigPath::FinalTransformPostAffineEnabled { index } => {
                write!(f, "Final Transform {} → Post-Affine Enabled", index + 1)
            }
            ConfigPath::FinalTransformPostAffine { index, param } => {
                write!(f, "Final Transform {} → Post-Affine {:?}", index + 1, param)
            }
            ConfigPath::FinalTransformYzCoefs { index, position } => {
                write!(f, "Final Transform {} → YZ Coef [{}]", index + 1, position)
            }
            ConfigPath::FinalTransformZxCoefs { index, position } => {
                write!(f, "Final Transform {} → ZX Coef [{}]", index + 1, position)
            }
            ConfigPath::FinalTransformYzPostCoefs { index, position } => {
                write!(f, "Final Transform {} → YZ Post Coef [{}]", index + 1, position)
            }
            ConfigPath::FinalTransformZxPostCoefs { index, position } => {
                write!(f, "Final Transform {} → ZX Post Coef [{}]", index + 1, position)
            }
            ConfigPath::FinalTransformVariation { index, variation } => {
                write!(f, "Final Transform {} → {} variation", index + 1, variation)
            }
            ConfigPath::FinalTransformVariationPriority { index, variation } => {
                write!(f, "Final Transform {} → {} phase", index + 1, variation)
            }
            ConfigPath::FinalTransformVariationParam { index, variation, param } => {
                write!(f, "Final Transform {} → {} → {}", index + 1, variation, param)
            }
            ConfigPath::FinalTransformOriginX { index } => {
                write!(f, "Final Transform {} → Origin X", index + 1)
            }
            ConfigPath::FinalTransformOriginY { index } => {
                write!(f, "Final Transform {} → Origin Y", index + 1)
            }
            ConfigPath::FinalTransformRotation { index } => {
                write!(f, "Final Transform {} → Rotation", index + 1)
            }
            ConfigPath::FinalTransformScale { index } => {
                write!(f, "Final Transform {} → Scale", index + 1)
            }
            ConfigPath::FinalTransformPostAffineOriginX { index } => {
                write!(f, "Final Transform {} → Post-Affine Origin X", index + 1)
            }
            ConfigPath::FinalTransformPostAffineOriginY { index } => {
                write!(f, "Final Transform {} → Post-Affine Origin Y", index + 1)
            }
            ConfigPath::FinalTransformPostAffineRotation { index } => {
                write!(f, "Final Transform {} → Post-Affine Rotation", index + 1)
            }
            ConfigPath::FinalTransformPostAffineScale { index } => {
                write!(f, "Final Transform {} → Post-Affine Scale", index + 1)
            }

            // Escape-time
            ConfigPath::EscapeFormula => write!(f, "Escape Formula"),
            ConfigPath::EscapeJulia => write!(f, "Julia Mode"),
            ConfigPath::EscapeJuliaRe => write!(f, "Julia Seed Re"),
            ConfigPath::EscapeJuliaIm => write!(f, "Julia Seed Im"),
            ConfigPath::EscapeCenterRe => write!(f, "Escape Center Re"),
            ConfigPath::EscapeCenterIm => write!(f, "Escape Center Im"),
            ConfigPath::EscapeZoomLog2 => write!(f, "Escape Zoom"),
            ConfigPath::EscapeRotation => write!(f, "Escape Rotation"),
            ConfigPath::EscapeMaxIter => write!(f, "Escape Max Iterations"),
            ConfigPath::SimModel => write!(f, "Simulation Model"),
            ConfigPath::SimColoring => write!(f, "Simulation Coloring"),
            ConfigPath::SimGridMode => write!(f, "Simulation Grid Mode"),
            ConfigPath::SimGridWidth => write!(f, "Simulation Grid Width"),
            ConfigPath::SimGridHeight => write!(f, "Simulation Grid Height"),
            ConfigPath::SimGridScale => write!(f, "Simulation Grid Scale"),
            ConfigPath::SimSeed => write!(f, "Simulation Seed"),
            ConfigPath::SimInitKind => write!(f, "Simulation Init"),
            ConfigPath::SimInitAmplitude => write!(f, "Simulation Init Amplitude"),
            ConfigPath::SimInitRadius => write!(f, "Simulation Init Radius"),
            ConfigPath::SimInitCount => write!(f, "Simulation Init Count"),
            ConfigPath::SimSteps => write!(f, "Simulation Steps"),
            ConfigPath::SimStepsPerFrame => write!(f, "Simulation Steps Per Frame"),
            ConfigPath::SimDt => write!(f, "Simulation dt"),
            ConfigPath::SimBoundary => write!(f, "Simulation Boundary"),
            ConfigPath::SimWarpZoom => write!(f, "Simulation Warp Zoom"),
            ConfigPath::SimWarpRotation => write!(f, "Simulation Warp Rotation"),
            ConfigPath::SimWarpPanX => write!(f, "Simulation Warp Pan X"),
            ConfigPath::SimWarpPanY => write!(f, "Simulation Warp Pan Y"),
            ConfigPath::SimWarpFlow => write!(f, "Simulation Warp Flow"),
            ConfigPath::SimWarpFilter => write!(f, "Simulation Warp Filter"),
            ConfigPath::SimMatteChannel => write!(f, "Simulation Matte Channel"),
            ConfigPath::SimMatteCutoff => write!(f, "Simulation Matte Cutoff"),
            ConfigPath::SimMatteSoftness => write!(f, "Simulation Matte Softness"),
            ConfigPath::SimMatteInvert => write!(f, "Simulation Matte Invert"),
            ConfigPath::SimMatteEdge => write!(f, "Simulation Matte Edge"),
            ConfigPath::SimUpscale => write!(f, "Simulation Upscale"),
            ConfigPath::SimDownscale => write!(f, "Simulation Downscale"),
            ConfigPath::SimModelParam { param } => write!(f, "Simulation {param}"),
            ConfigPath::SimColoringParam { param } => write!(f, "Simulation Color {param}"),
            ConfigPath::EscapeSupersample => write!(f, "Escape Antialiasing"),
            ConfigPath::EscapeDownsample => write!(f, "Escape Downsample"),
            ConfigPath::EscapeReferencePeriod => write!(f, "Escape Reference Period"),
            ConfigPath::EscapeBailout => write!(f, "Escape Bailout"),
            ConfigPath::EscapeDampingRe => write!(f, "Damping (re)"),
            ConfigPath::EscapeDampingIm => write!(f, "Damping (im)"),
            ConfigPath::EscapeBiomorph => write!(f, "Biomorph Mode"),
            ConfigPath::EscapeShadingEnabled => write!(f, "Relief Shading"),
            ConfigPath::EscapeShadingLightAngle => write!(f, "Relief Light Angle"),
            ConfigPath::EscapeShadingHeight => write!(f, "Relief Height"),
            ConfigPath::EscapeShadingField => write!(f, "Relief Source Field"),
            ConfigPath::EscapeContrastMode => write!(f, "Auto Contrast"),
            ConfigPath::EscapeContrastClip => write!(f, "Contrast Clip"),
            ConfigPath::EscapeContrastStrength => write!(f, "Contrast Strength"),
            ConfigPath::EscapeContrastTurns => write!(f, "Contrast Turns"),
            ConfigPath::EscapeShadingShadowColor => write!(f, "Relief Shadow Colour"),
            ConfigPath::EscapeShadingShadowStrength => write!(f, "Relief Shadow Strength"),
            ConfigPath::EscapeShadingShadowBlend => write!(f, "Relief Shadow Blend"),
            ConfigPath::EscapeShadingHighlightColor => write!(f, "Relief Highlight Colour"),
            ConfigPath::EscapeShadingHighlightStrength => write!(f, "Relief Highlight Strength"),
            ConfigPath::EscapeShadingHighlightBlend => write!(f, "Relief Highlight Blend"),
            ConfigPath::EscapeShadingSoftness => write!(f, "Relief Softness"),
            ConfigPath::EscapeShadingTextureKind => write!(f, "Relief Texture"),
            ConfigPath::EscapeShadingTextureStrength => write!(f, "Relief Texture Strength"),
            ConfigPath::EscapeShadingTextureScale => write!(f, "Relief Texture Scale"),
            ConfigPath::EscapeColoring => write!(f, "Escape Coloring"),
            ConfigPath::EscapeFormulaParam { param } => write!(f, "Formula → {param}"),
            ConfigPath::EscapeColoringParam { param } => write!(f, "Coloring → {param}"),

            // Flame
            ConfigPath::RenderMode => write!(f, "Render Mode"),
            ConfigPath::PerspectiveStrength => write!(f, "Perspective Strength"),
            ConfigPath::DepthDensityCompensation => write!(f, "Depth Density Compensation"),
            ConfigPath::FarDensityFade => write!(f, "Far Density Fade"),
            ConfigPath::SolidStrength => write!(f, "Solid Strength"),
            ConfigPath::SurfaceThickness => write!(f, "Surface Thickness"),
            ConfigPath::SolidShadowStrength => write!(f, "Shadow Strength"),
            ConfigPath::ShadingStrength => write!(f, "Shading Strength"),
            ConfigPath::SolidAmbient => write!(f, "Ambient Light"),
            ConfigPath::SolidDiffuse => write!(f, "Diffuse Light"),
            ConfigPath::SolidSpecular => write!(f, "Specular"),
            ConfigPath::SolidShininess => write!(f, "Shininess"),
            ConfigPath::SsaoStrength => write!(f, "SSAO Strength"),
            ConfigPath::SsaoRadius => write!(f, "SSAO Radius"),
            ConfigPath::NormalSmoothing => write!(f, "Normal Smoothing"),
            ConfigPath::GapFill => write!(f, "Gap Fill"),
            ConfigPath::SolidLightEnabled { index } => write!(f, "Light {} Enabled", index + 1),
            ConfigPath::SolidLightParam { index, param } => write!(f, "Light {} {}", index + 1, param),
            ConfigPath::FarDensityFadeStart => write!(f, "Far Density Fade Start"),
            ConfigPath::Xaos { src, dst } => {
                write!(f, "Xaos {} → {}", src + 1, dst + 1)
            }
            ConfigPath::SoloTransform => write!(f, "Solo Transform"),

            // Post-symmetry
            ConfigPath::PostSymmetryType => write!(f, "Symmetry Type"),
            ConfigPath::PostSymmetryOrder => write!(f, "Symmetry Order"),
            ConfigPath::PostSymmetryCenterX => write!(f, "Symmetry Center X"),
            ConfigPath::PostSymmetryCenterY => write!(f, "Symmetry Center Y"),
            ConfigPath::PostSymmetryDistance => write!(f, "Symmetry Distance"),
            ConfigPath::PostSymmetryRotation => write!(f, "Symmetry Rotation"),
            ConfigPath::PreserveZ => write!(f, "Preserve Z"),

            // Effects
            ConfigPath::DensityEffectEnabled { index } => {
                write!(f, "Density Effect {} → Enabled", index + 1)
            }
            ConfigPath::DensityEffectParam { index, param } => {
                write!(f, "Density Effect {} → {}", index + 1, param)
            }
            ConfigPath::ColorEffectEnabled { index } => {
                write!(f, "Color Effect {} → Enabled", index + 1)
            }
            ConfigPath::ColorEffectParam { index, param } => {
                write!(f, "Color Effect {} → {}", index + 1, param)
            }
            ConfigPath::AddColorEffect { effect_type } => {
                write!(f, "Add Color Effect: {}", effect_type)
            }
            ConfigPath::RemoveColorEffect { index } => {
                write!(f, "Remove Color Effect {}", index + 1)
            }
            ConfigPath::AddDensityEffect { effect_type } => {
                write!(f, "Add Density Effect: {}", effect_type)
            }
            ConfigPath::RemoveDensityEffect { index } => {
                write!(f, "Remove Density Effect {}", index + 1)
            }

            // System Settings
            ConfigPath::SystemIterationsPerThread => write!(f, "System: Iterations Per Thread"),
            ConfigPath::SystemBurnIn => write!(f, "System: Burn-in Iterations"),
            ConfigPath::SystemOrbitCacheMb => write!(f, "System: Orbit Cache Size (MB)"),
            ConfigPath::SystemVsyncEnabled => write!(f, "System: VSync Enabled"),
            ConfigPath::SystemTargetFps => write!(f, "System: Target FPS"),
            ConfigPath::SystemFlyMouseSensitivity => write!(f, "System: Fly Mouse Sensitivity"),
            ConfigPath::SystemFlyMoveSpeed => write!(f, "System: Fly Move Speed"),
            ConfigPath::SystemFlySprintMultiplier => write!(f, "System: Fly Sprint Multiplier"),
            ConfigPath::SystemFlyInvertY => write!(f, "System: Fly Invert Y"),
            ConfigPath::SystemFlyCameraMode => write!(f, "System: Fly Camera Mode"),
            ConfigPath::SystemExportWidth => write!(f, "System: Export Width"),
            ConfigPath::SystemExportHeight => write!(f, "System: Export Height"),
            ConfigPath::SystemPngStripMetadata => write!(f, "System: Strip PNG Metadata"),
            ConfigPath::SystemLanguage => write!(f, "System: Language"),
            ConfigPath::SystemShowHelpOnStartup => write!(f, "System: Show Help On Startup"),
        }
    }
}

/// Represents an i18n key with optional parameters for translation
#[derive(Debug, Clone)]
pub struct I18nKey {
    /// The translation key (e.g., "history.param.zoom")
    pub key: String,
    /// Optional parameters for interpolation (e.g., index, variation name)
    pub params: Vec<(String, String)>,
}

impl I18nKey {
    /// Create a simple key with no parameters
    pub fn simple(key: &str) -> Self {
        Self {
            key: key.to_string(),
            params: Vec::new(),
        }
    }

    /// Create a key with parameters
    pub fn with_params(key: &str, params: Vec<(&str, String)>) -> Self {
        Self {
            key: key.to_string(),
            params: params.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }

    /// Serialize to a string format for storage in history descriptions
    /// Format: `key` or `key|param1=value1|param2=value2`
    pub fn to_serialized(&self) -> String {
        if self.params.is_empty() {
            self.key.clone()
        } else {
            let params_str = self.params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("|");
            format!("{}|{}", self.key, params_str)
        }
    }
}

/// A–F letter for a JWildfire plane-affine coefficient position, so a
/// plane-coef history entry reads "Transform 1 -> ZX Affine C" like the
/// XY affine entries. The plane array is stored in JWF order
/// `[00, 01, 10, 11, 20, 21]` = `[a, c, b, d, e, f]` (see
/// `Transform::plane_to_triangle_apophysis`), so positions 1 and 2 are
/// `c`/`b`, not `b`/`c` — matching the Transform panel's cell labels.
fn plane_coef_letter(position: u8) -> &'static str {
    ["A", "C", "B", "D", "E", "F"].get(position as usize).copied().unwrap_or("?")
}

impl ConfigPath {
    /// Convert to an i18n key for translation
    /// Returns an I18nKey struct with the key and any parameters needed for interpolation
    pub fn to_i18n_key(&self) -> I18nKey {
        match self {
            // View
            ConfigPath::Zoom => I18nKey::simple("history.param.zoom"),
            ConfigPath::Pan => I18nKey::simple("history.param.pan"),
            ConfigPath::PanX => I18nKey::simple("history.param.pan_x"),
            ConfigPath::PanY => I18nKey::simple("history.param.pan_y"),
            ConfigPath::Rotation => I18nKey::simple("history.param.rotation"),
            ConfigPath::CameraRotationX => I18nKey::simple("history.param.camera_pitch"),
            ConfigPath::CameraRotationY => I18nKey::simple("history.param.camera_yaw"),
            ConfigPath::CameraBank => I18nKey::simple("history.param.camera_bank"),
            ConfigPath::CameraX => I18nKey::simple("history.param.camera_x"),
            ConfigPath::CameraY => I18nKey::simple("history.param.camera_y"),
            ConfigPath::CameraZ => I18nKey::simple("history.param.camera_z"),
            ConfigPath::DofFocusDistance => I18nKey::simple("history.param.dof_focus_distance"),
            ConfigPath::DofBlurStrength => I18nKey::simple("history.param.dof_blur_strength"),
            ConfigPath::FogStrength => I18nKey::simple("history.param.fog_strength"),
            ConfigPath::FogStart => I18nKey::simple("history.param.fog_start"),
            ConfigPath::FilterRadius => I18nKey::simple("history.param.filter_radius"),
            ConfigPath::FilterBlurEdges => I18nKey::simple("history.param.filter_blur_edges"),

            // Tone mapping
            ConfigPath::Exposure => I18nKey::simple("history.param.exposure"),
            ConfigPath::Gamma => I18nKey::simple("history.param.gamma"),
            ConfigPath::GammaThreshold => I18nKey::simple("history.param.gamma_threshold"),
            ConfigPath::Brightness => I18nKey::simple("history.param.brightness"),
            ConfigPath::Vibrancy => I18nKey::simple("history.param.vibrancy"),
            ConfigPath::WhiteLevel => I18nKey::simple("history.param.white_level"),
            ConfigPath::Saturation => I18nKey::simple("history.param.saturation"),
            ConfigPath::HueShift => I18nKey::simple("history.param.hue_shift"),
            ConfigPath::AlphaBlendLow => I18nKey::simple("history.param.alpha_blend_low"),
            ConfigPath::AlphaBlendHigh => I18nKey::simple("history.param.alpha_blend_high"),
            ConfigPath::DensityScale => I18nKey::simple("history.param.density_scale"),
            ConfigPath::TonemapMode => I18nKey::simple("history.param.tonemap_mode"),
            ConfigPath::HighlightMode => I18nKey::simple("history.param.highlight_mode"),
            ConfigPath::TonemapCurve => I18nKey::simple("history.param.tone_curve"),
            ConfigPath::UseCurve => I18nKey::simple("history.param.use_tone_curve"),
            ConfigPath::LevelsEnabled => I18nKey::simple("history.param.levels_enabled"),
            ConfigPath::LevelsLow => I18nKey::simple("history.param.levels_low"),
            ConfigPath::LevelsHigh => I18nKey::simple("history.param.levels_high"),
            ConfigPath::LevelsGamma => I18nKey::simple("history.param.levels_midtones"),

            // Color
            ConfigPath::ColorMode => I18nKey::simple("history.param.color_mode"),
            ConfigPath::PathMapStyle => I18nKey::simple("history.param.pathmap_style"),
            ConfigPath::PathCaptureMode => I18nKey::simple("history.param.pathmap_capture_mode"),
            ConfigPath::PathTrackingMode => I18nKey::simple("history.param.pathmap_tracking_mode"),
            ConfigPath::PaletteIndex => I18nKey::simple("history.param.palette"),
            ConfigPath::Palette => I18nKey::simple("history.param.palette_data"),
            ConfigPath::PaletteRotation => I18nKey::simple("history.param.palette_rotation"),
            ConfigPath::PaletteSize => I18nKey::simple("history.param.palette_size"),
            ConfigPath::PaletteSqueeze => I18nKey::simple("history.param.palette_squeeze"),
            ConfigPath::PaletteSqueezeMode => I18nKey::simple("history.param.palette_squeeze_mode"),
            ConfigPath::PaletteSqueezeFalloff => I18nKey::simple("history.param.palette_squeeze_falloff"),
            ConfigPath::PaletteLogStrength => I18nKey::simple("history.param.palette_log_strength"),
            ConfigPath::PaletteReverse => I18nKey::simple("history.param.palette_reverse"),
            ConfigPath::SpeedFactor => I18nKey::simple("history.param.speed_blend_factor"),
            ConfigPath::BackgroundColor => I18nKey::simple("history.param.background_color"),
            ConfigPath::BackgroundColorR => I18nKey::simple("history.param.background_red"),
            ConfigPath::BackgroundColorG => I18nKey::simple("history.param.background_green"),
            ConfigPath::BackgroundColorB => I18nKey::simple("history.param.background_blue"),

            // Rendering
            ConfigPath::BlendFactor => I18nKey::simple("history.param.blend_factor"),
            ConfigPath::UseDynamicBlend => I18nKey::simple("history.param.use_dynamic_blend"),
            ConfigPath::MaxIterations => I18nKey::simple("history.param.max_iterations"),
            ConfigPath::DeterministicRng => I18nKey::simple("history.param.deterministic_rng"),

            // Escape-time
            ConfigPath::EscapeFormula => I18nKey::simple("history.param.escape_formula"),
            ConfigPath::EscapeJulia => I18nKey::simple("history.param.escape_julia"),
            ConfigPath::EscapeJuliaRe => I18nKey::simple("history.param.escape_julia_re"),
            ConfigPath::EscapeJuliaIm => I18nKey::simple("history.param.escape_julia_im"),
            ConfigPath::EscapeCenterRe => I18nKey::simple("history.param.escape_center_re"),
            ConfigPath::EscapeCenterIm => I18nKey::simple("history.param.escape_center_im"),
            ConfigPath::EscapeZoomLog2 => I18nKey::simple("history.param.escape_zoom"),
            ConfigPath::EscapeRotation => I18nKey::simple("history.param.escape_rotation"),
            ConfigPath::EscapeMaxIter => I18nKey::simple("history.param.escape_max_iter"),
            ConfigPath::SimModel => I18nKey::simple("history.param.sim_model"),
            ConfigPath::SimColoring => I18nKey::simple("history.param.sim_coloring"),
            ConfigPath::SimGridMode => I18nKey::simple("history.param.sim_grid_mode"),
            ConfigPath::SimGridWidth => I18nKey::simple("history.param.sim_grid_width"),
            ConfigPath::SimGridHeight => I18nKey::simple("history.param.sim_grid_height"),
            ConfigPath::SimGridScale => I18nKey::simple("history.param.sim_grid_scale"),
            ConfigPath::SimSeed => I18nKey::simple("history.param.sim_seed"),
            ConfigPath::SimInitKind => I18nKey::simple("history.param.sim_init_kind"),
            ConfigPath::SimInitAmplitude => I18nKey::simple("history.param.sim_init_amplitude"),
            ConfigPath::SimInitRadius => I18nKey::simple("history.param.sim_init_radius"),
            ConfigPath::SimInitCount => I18nKey::simple("history.param.sim_init_count"),
            ConfigPath::SimSteps => I18nKey::simple("history.param.sim_steps"),
            ConfigPath::SimStepsPerFrame => I18nKey::simple("history.param.sim_steps_per_frame"),
            ConfigPath::SimDt => I18nKey::simple("history.param.sim_dt"),
            ConfigPath::SimBoundary => I18nKey::simple("history.param.sim_boundary"),
            ConfigPath::SimWarpZoom => I18nKey::simple("history.param.sim_warp_zoom"),
            ConfigPath::SimWarpRotation => I18nKey::simple("history.param.sim_warp_rotation"),
            ConfigPath::SimWarpPanX => I18nKey::simple("history.param.sim_warp_pan_x"),
            ConfigPath::SimWarpPanY => I18nKey::simple("history.param.sim_warp_pan_y"),
            ConfigPath::SimWarpFlow => I18nKey::simple("history.param.sim_warp_flow"),
            ConfigPath::SimWarpFilter => I18nKey::simple("history.param.sim_warp_filter"),
            ConfigPath::SimMatteChannel => I18nKey::simple("history.param.sim_matte_channel"),
            ConfigPath::SimMatteCutoff => I18nKey::simple("history.param.sim_matte_cutoff"),
            ConfigPath::SimMatteSoftness => I18nKey::simple("history.param.sim_matte_softness"),
            ConfigPath::SimMatteInvert => I18nKey::simple("history.param.sim_matte_invert"),
            ConfigPath::SimMatteEdge => I18nKey::simple("history.param.sim_matte_edge"),
            ConfigPath::SimUpscale => I18nKey::simple("history.param.sim_upscale"),
            ConfigPath::SimDownscale => I18nKey::simple("history.param.sim_downscale"),
            ConfigPath::SimModelParam { param } => I18nKey::with_params(
                "history.param.sim_model_param",
                vec![("param", param.clone())],
            ),
            ConfigPath::SimColoringParam { param } => I18nKey::with_params(
                "history.param.sim_coloring_param",
                vec![("param", param.clone())],
            ),
            ConfigPath::EscapeSupersample => I18nKey::simple("history.param.escape_supersample"),
            ConfigPath::EscapeDownsample => I18nKey::simple("history.param.escape_downsample"),
            ConfigPath::EscapeReferencePeriod => {
                I18nKey::simple("history.param.escape_reference_period")
            }
            ConfigPath::EscapeBailout => I18nKey::simple("history.param.escape_bailout"),
            ConfigPath::EscapeDampingRe => I18nKey::simple("history.param.escape_damping_re"),
            ConfigPath::EscapeDampingIm => I18nKey::simple("history.param.escape_damping_im"),
            ConfigPath::EscapeBiomorph => I18nKey::simple("history.param.escape_biomorph"),
            ConfigPath::EscapeShadingEnabled => I18nKey::simple("history.param.escape_shading_enabled"),
            ConfigPath::EscapeShadingLightAngle => I18nKey::simple("history.param.escape_shading_light_angle"),
            ConfigPath::EscapeShadingHeight => I18nKey::simple("history.param.escape_shading_height"),
            ConfigPath::EscapeShadingField => I18nKey::simple("history.param.escape_shading_field"),
            ConfigPath::EscapeContrastMode => I18nKey::simple("history.param.escape_contrast_mode"),
            ConfigPath::EscapeContrastClip => I18nKey::simple("history.param.escape_contrast_clip"),
            ConfigPath::EscapeContrastStrength => {
                I18nKey::simple("history.param.escape_contrast_strength")
            }
            ConfigPath::EscapeContrastTurns => I18nKey::simple("history.param.escape_contrast_turns"),
            ConfigPath::EscapeShadingShadowColor => I18nKey::simple("history.param.escape_shading_shadow_color"),
            ConfigPath::EscapeShadingShadowStrength => I18nKey::simple("history.param.escape_shading_shadow_strength"),
            ConfigPath::EscapeShadingShadowBlend => I18nKey::simple("history.param.escape_shading_shadow_blend"),
            ConfigPath::EscapeShadingHighlightColor => I18nKey::simple("history.param.escape_shading_highlight_color"),
            ConfigPath::EscapeShadingHighlightStrength => I18nKey::simple("history.param.escape_shading_highlight_strength"),
            ConfigPath::EscapeShadingHighlightBlend => I18nKey::simple("history.param.escape_shading_highlight_blend"),
            ConfigPath::EscapeShadingSoftness => I18nKey::simple("history.param.escape_shading_softness"),
            ConfigPath::EscapeShadingTextureKind => I18nKey::simple("history.param.escape_shading_texture_kind"),
            ConfigPath::EscapeShadingTextureStrength => I18nKey::simple("history.param.escape_shading_texture_strength"),
            ConfigPath::EscapeShadingTextureScale => I18nKey::simple("history.param.escape_shading_texture_scale"),
            ConfigPath::EscapeColoring => I18nKey::simple("history.param.escape_coloring"),
            ConfigPath::EscapeFormulaParam { param } => I18nKey::with_params(
                "history.param.escape_formula_param",
                vec![("param", param.clone())],
            ),
            ConfigPath::EscapeColoringParam { param } => I18nKey::with_params(
                "history.param.escape_coloring_param",
                vec![("param", param.clone())],
            ),

            // Transforms
            ConfigPath::TransformCount => I18nKey::simple("history.param.transform_count"),
            ConfigPath::TransformWeight { index } => I18nKey::with_params(
                "history.param.transform_weight",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformColor { index } => I18nKey::with_params(
                "history.param.transform_color",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformColorSpeed { index } => I18nKey::with_params(
                "history.param.transform_color_speed",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformOpacity { index } => I18nKey::with_params(
                "history.param.transform_opacity",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformDirectColor { index } => I18nKey::with_params(
                "history.param.transform_direct_color",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformAffine { index, param } => I18nKey::with_params(
                "history.param.transform_affine",
                vec![
                    ("index", (index + 1).to_string()),
                    ("param", format!("{:?}", param)),
                ],
            ),
            ConfigPath::TransformVariation { index, variation } => I18nKey::with_params(
                "history.param.transform_variation",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                ],
            ),
            ConfigPath::TransformVariationPriority { index, variation }
            | ConfigPath::LinkedTransformVariationPriority { index, variation }
            | ConfigPath::FinalTransformVariationPriority { index, variation } => I18nKey::with_params(
                "history.param.transform_variation_priority",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                ],
            ),
            ConfigPath::TransformVariationOrder { index }
            | ConfigPath::LinkedTransformVariationOrder { index }
            | ConfigPath::FinalTransformVariationOrder { index } => I18nKey::with_params(
                "history.param.transform_variation_order",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformVariationParam { index, variation, param } => I18nKey::with_params(
                "history.param.transform_variation_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::TransformOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineEnabled { index } => I18nKey::with_params(
                "history.param.transform_post_affine_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffine { index, param } => I18nKey::with_params(
                "history.param.transform_post_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            // Reuse the pre-affine high-level i18n keys for the
            // post-affine ones — the UI distinguishes by the
            // "Post-Affine" label that surrounds the value.
            ConfigPath::TransformPostAffineOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),
            // JWildfire plane coefs — fall back to a generic key with
            // index + position parameters. Localizers can flesh these
            // out per-plane later if it matters for UX; the history
            // panel shows them as e.g. "Transform 2 → YZ Coef [4]".
            ConfigPath::TransformYzCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_yz_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::TransformZxCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_zx_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::TransformYzPostCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_yz_post_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::TransformZxPostCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_zx_post_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),

            // Linked Transform pool
            ConfigPath::LinkedTransformAffine { index, param } => I18nKey::with_params(
                "history.param.transform_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            ConfigPath::LinkedTransformPostAffineEnabled { index } => I18nKey::with_params(
                "history.param.transform_post_affine_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformPostAffine { index, param } => I18nKey::with_params(
                "history.param.transform_post_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            // Reuse the normal-pool plane-coef i18n keys — surrounding
            // panel labels distinguish the pool, same pattern as the
            // existing LinkedTransformOriginX => transform_origin_x
            // reuse below.
            ConfigPath::LinkedTransformYzCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_yz_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::LinkedTransformZxCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_zx_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::LinkedTransformYzPostCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_yz_post_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::LinkedTransformZxPostCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_zx_post_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::LinkedTransformVariation { index, variation } => I18nKey::with_params(
                "history.param.transform_variation",
                vec![("index", (index + 1).to_string()), ("variation", variation.clone())],
            ),
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => I18nKey::with_params(
                "history.param.transform_variation_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::LinkedTransformOriginX { index }
            | ConfigPath::LinkedTransformPostAffineOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformOriginY { index }
            | ConfigPath::LinkedTransformPostAffineOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformRotation { index }
            | ConfigPath::LinkedTransformPostAffineRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformScale { index }
            | ConfigPath::LinkedTransformPostAffineScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),

            // Final Transform pool — reuses transform_* i18n keys; the
            // user-visible "Linked"/"Final" distinction comes from the
            // panel header.
            ConfigPath::FinalTransformAffine { index, param } => I18nKey::with_params(
                "history.param.transform_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            ConfigPath::FinalTransformPostAffineEnabled { index } => I18nKey::with_params(
                "history.param.transform_post_affine_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformPostAffine { index, param } => I18nKey::with_params(
                "history.param.transform_post_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            ConfigPath::FinalTransformYzCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_yz_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::FinalTransformZxCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_zx_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::FinalTransformYzPostCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_yz_post_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::FinalTransformZxPostCoefs { index, position } => I18nKey::with_params(
                "history.param.transform_zx_post_coefs",
                vec![("index", (index + 1).to_string()), ("param", plane_coef_letter(*position).to_string())],
            ),
            ConfigPath::FinalTransformVariation { index, variation } => I18nKey::with_params(
                "history.param.transform_variation",
                vec![("index", (index + 1).to_string()), ("variation", variation.clone())],
            ),
            ConfigPath::FinalTransformVariationParam { index, variation, param } => I18nKey::with_params(
                "history.param.transform_variation_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::FinalTransformOriginX { index }
            | ConfigPath::FinalTransformPostAffineOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformOriginY { index }
            | ConfigPath::FinalTransformPostAffineOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformRotation { index }
            | ConfigPath::FinalTransformPostAffineRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformScale { index }
            | ConfigPath::FinalTransformPostAffineScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),

            // Flame
            ConfigPath::RenderMode => I18nKey::simple("history.param.render_mode"),
            ConfigPath::PerspectiveStrength => I18nKey::simple("history.param.perspective_strength"),
            ConfigPath::DepthDensityCompensation => I18nKey::simple("history.param.depth_density_compensation"),
            ConfigPath::FarDensityFade => I18nKey::simple("history.param.far_density_fade"),
            ConfigPath::SolidStrength => I18nKey::simple("history.param.solid_strength"),
            ConfigPath::SurfaceThickness => I18nKey::simple("history.param.surface_thickness"),
            ConfigPath::SolidShadowStrength => I18nKey::simple("history.param.shadow_strength"),
            ConfigPath::ShadingStrength => I18nKey::simple("history.param.shading_strength"),
            ConfigPath::SolidAmbient => I18nKey::simple("history.param.solid_ambient"),
            ConfigPath::SolidDiffuse => I18nKey::simple("history.param.solid_diffuse"),
            ConfigPath::SolidSpecular => I18nKey::simple("history.param.solid_specular"),
            ConfigPath::SolidShininess => I18nKey::simple("history.param.solid_shininess"),
            ConfigPath::SsaoStrength => I18nKey::simple("history.param.ssao_strength"),
            ConfigPath::SsaoRadius => I18nKey::simple("history.param.ssao_radius"),
            ConfigPath::NormalSmoothing => I18nKey::simple("history.param.normal_smoothing"),
            ConfigPath::GapFill => I18nKey::simple("history.param.gap_fill"),
            ConfigPath::SolidLightEnabled { .. } => I18nKey::simple("history.param.solid_light_enabled"),
            ConfigPath::SolidLightParam { .. } => I18nKey::simple("history.param.solid_light_param"),
            ConfigPath::FarDensityFadeStart => I18nKey::simple("history.param.far_density_fade_start"),
            ConfigPath::Xaos { src, dst } => I18nKey::with_params(
                "history.param.xaos",
                vec![
                    ("src", (src + 1).to_string()),
                    ("dst", (dst + 1).to_string()),
                ],
            ),
            ConfigPath::SoloTransform => I18nKey::simple("history.param.solo_transform"),
            ConfigPath::PostSymmetryType => I18nKey::simple("history.param.post_symmetry_type"),
            ConfigPath::PostSymmetryOrder => I18nKey::simple("history.param.post_symmetry_order"),
            ConfigPath::PostSymmetryCenterX => I18nKey::simple("history.param.post_symmetry_center_x"),
            ConfigPath::PostSymmetryCenterY => I18nKey::simple("history.param.post_symmetry_center_y"),
            ConfigPath::PostSymmetryDistance => I18nKey::simple("history.param.post_symmetry_distance"),
            ConfigPath::PostSymmetryRotation => I18nKey::simple("history.param.post_symmetry_rotation"),
            ConfigPath::PreserveZ => I18nKey::simple("history.param.preserve_z"),

            // Effects
            ConfigPath::DensityEffectEnabled { index } => I18nKey::with_params(
                "history.param.density_effect_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::DensityEffectParam { index, param } => I18nKey::with_params(
                "history.param.density_effect_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::ColorEffectEnabled { index } => I18nKey::with_params(
                "history.param.color_effect_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::ColorEffectParam { index, param } => I18nKey::with_params(
                "history.param.color_effect_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::AddColorEffect { effect_type } => I18nKey::with_params(
                "history.param.add_color_effect",
                vec![("effect_type", effect_type.clone())],
            ),
            ConfigPath::RemoveColorEffect { index } => I18nKey::with_params(
                "history.param.remove_color_effect",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::AddDensityEffect { effect_type } => I18nKey::with_params(
                "history.param.add_density_effect",
                vec![("effect_type", effect_type.clone())],
            ),
            ConfigPath::RemoveDensityEffect { index } => I18nKey::with_params(
                "history.param.remove_density_effect",
                vec![("index", (index + 1).to_string())],
            ),

            // System Settings
            ConfigPath::SystemIterationsPerThread => I18nKey::simple("history.param.system_iterations_per_thread"),
            ConfigPath::SystemBurnIn => I18nKey::simple("history.param.system_burn_in"),
            ConfigPath::SystemOrbitCacheMb => {
                I18nKey::simple("history.param.system_orbit_cache_mb")
            }
            ConfigPath::SystemVsyncEnabled => I18nKey::simple("history.param.system_vsync_enabled"),
            ConfigPath::SystemTargetFps => I18nKey::simple("history.param.system_target_fps"),
            ConfigPath::SystemFlyMouseSensitivity => I18nKey::simple("history.param.system_fly_mouse_sensitivity"),
            ConfigPath::SystemFlyMoveSpeed => I18nKey::simple("history.param.system_fly_move_speed"),
            ConfigPath::SystemFlySprintMultiplier => I18nKey::simple("history.param.system_fly_sprint_multiplier"),
            ConfigPath::SystemFlyInvertY => I18nKey::simple("history.param.system_fly_invert_y"),
            ConfigPath::SystemFlyCameraMode => I18nKey::simple("history.param.system_fly_camera_mode"),
            ConfigPath::SystemExportWidth => I18nKey::simple("history.param.system_export_width"),
            ConfigPath::SystemExportHeight => I18nKey::simple("history.param.system_export_height"),
            ConfigPath::SystemPngStripMetadata => I18nKey::simple("history.param.system_png_strip_metadata"),
            ConfigPath::SystemLanguage => I18nKey::simple("history.param.system_language"),
            ConfigPath::SystemShowHelpOnStartup => I18nKey::simple("history.param.system_show_help_on_startup"),
        }
    }
}

/// A value that can be stored in FractalConfig
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// Unit value for operations that don't need a value (e.g., Add/Remove)
    Unit,
    Float(f32),
    Int(i32),
    UInt(u32),
    UInt64(u64),
    Bool(bool),
    String(String),
    /// Ordered list of strings — used for `variation_order` reordering.
    StringList(Vec<String>),
    Vec2(f32, f32),  // For pan coordinates and other 2D values
    ColorRgb([f32; 3]),
    ToneMapMode(ToneMapMode),
    HighlightMode(crate::scene::tonemap::HighlightMode),
    ColorMode(ColorMode),
    PathMapStyle(PathMapStyle),
    PathCaptureMode(PathCaptureMode),
    PathTrackingMode(PathTrackingMode),
    RenderMode(RenderMode),
    ToneCurve(ToneCurve),
    Palette(Palette),
    SqueezeMode(crate::scene::palette::SqueezeMode),
}

impl ConfigValue {
    /// Check if two values are approximately equal (for floats)
    pub fn approx_eq(&self, other: &Self) -> bool {
        const EPSILON_F32: f32 = 1e-6;

        match (self, other) {
            (ConfigValue::Float(a), ConfigValue::Float(b)) => (a - b).abs() < EPSILON_F32,
            (ConfigValue::Vec2(x1, y1), ConfigValue::Vec2(x2, y2)) => {
                (x1 - x2).abs() < EPSILON_F32 && (y1 - y2).abs() < EPSILON_F32
            }
            (ConfigValue::ColorRgb(a), ConfigValue::ColorRgb(b)) => a
                .iter()
                .zip(b.iter())
                .all(|(x, y)| (x - y).abs() < EPSILON_F32),
            (ConfigValue::Int(a), ConfigValue::Int(b)) => a == b,
            (ConfigValue::UInt(a), ConfigValue::UInt(b)) => a == b,
            (ConfigValue::UInt64(a), ConfigValue::UInt64(b)) => a == b,
            (ConfigValue::Bool(a), ConfigValue::Bool(b)) => a == b,
            (ConfigValue::String(a), ConfigValue::String(b)) => a == b,
            (ConfigValue::StringList(a), ConfigValue::StringList(b)) => a == b,
            (ConfigValue::ToneMapMode(a), ConfigValue::ToneMapMode(b)) => a == b,
            (ConfigValue::HighlightMode(a), ConfigValue::HighlightMode(b)) => a == b,
            (ConfigValue::ColorMode(a), ConfigValue::ColorMode(b)) => a == b,
            (ConfigValue::SqueezeMode(a), ConfigValue::SqueezeMode(b)) => a == b,
            (ConfigValue::RenderMode(a), ConfigValue::RenderMode(b)) => a == b,
            // For complex types, do shallow comparison or always return false
            _ => false,
        }
    }
}

impl Display for ConfigValue {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ConfigValue::Unit => write!(f, "()"),
            ConfigValue::Float(v) => write!(f, "{:.3}", v),
            ConfigValue::Int(v) => write!(f, "{}", v),
            ConfigValue::UInt(v) => write!(f, "{}", v),
            ConfigValue::UInt64(v) => write!(f, "{}", v),
            ConfigValue::Bool(v) => write!(f, "{}", v),
            ConfigValue::String(v) => write!(f, "{}", v),
            ConfigValue::StringList(v) => write!(f, "[{}]", v.join(", ")),
            ConfigValue::Vec2(x, y) => write!(f, "({:.3}, {:.3})", x, y),
            ConfigValue::ColorRgb([r, g, b]) => {
                write!(f, "RGB({:.2}, {:.2}, {:.2})", r, g, b)
            }
            ConfigValue::ToneMapMode(m) => write!(f, "{:?}", m),
            ConfigValue::HighlightMode(m) => write!(f, "{:?}", m),
            ConfigValue::ColorMode(m) => write!(f, "{:?}", m),
            ConfigValue::SqueezeMode(m) => write!(f, "{:?}", m),
            ConfigValue::PathMapStyle(m) => write!(f, "{:?}", m),
            ConfigValue::PathCaptureMode(m) => write!(f, "{:?}", m),
            ConfigValue::PathTrackingMode(m) => write!(f, "{:?}", m),
            ConfigValue::RenderMode(m) => write!(f, "{:?}", m),
            ConfigValue::ToneCurve(curve) => {
                write!(f, "[Tone Curve: {} pts: {:?}]",
                    curve.points.len(),
                    curve.points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
            }
            ConfigValue::Palette(p) => write!(f, "{}", p.name),
        }
    }
}

// Conversion traits: From basic types to ConfigValue
impl From<()> for ConfigValue {
    fn from(_: ()) -> Self {
        ConfigValue::Unit
    }
}

impl From<f32> for ConfigValue {
    fn from(v: f32) -> Self {
        ConfigValue::Float(v)
    }
}

impl From<i32> for ConfigValue {
    fn from(v: i32) -> Self {
        ConfigValue::Int(v)
    }
}

impl From<u32> for ConfigValue {
    fn from(v: u32) -> Self {
        ConfigValue::UInt(v)
    }
}

impl From<u64> for ConfigValue {
    fn from(v: u64) -> Self {
        ConfigValue::UInt64(v)
    }
}

impl From<bool> for ConfigValue {
    fn from(v: bool) -> Self {
        ConfigValue::Bool(v)
    }
}

impl From<String> for ConfigValue {
    fn from(v: String) -> Self {
        ConfigValue::String(v)
    }
}

impl From<Vec<String>> for ConfigValue {
    fn from(v: Vec<String>) -> Self {
        ConfigValue::StringList(v)
    }
}

impl From<&str> for ConfigValue {
    fn from(v: &str) -> Self {
        ConfigValue::String(v.to_string())
    }
}

impl From<(f32, f32)> for ConfigValue {
    fn from((x, y): (f32, f32)) -> Self {
        ConfigValue::Vec2(x, y)
    }
}

impl From<[f32; 3]> for ConfigValue {
    fn from(v: [f32; 3]) -> Self {
        ConfigValue::ColorRgb(v)
    }
}

impl From<ToneMapMode> for ConfigValue {
    fn from(v: ToneMapMode) -> Self {
        ConfigValue::ToneMapMode(v)
    }
}

impl From<crate::scene::tonemap::HighlightMode> for ConfigValue {
    fn from(v: crate::scene::tonemap::HighlightMode) -> Self {
        ConfigValue::HighlightMode(v)
    }
}

impl From<ColorMode> for ConfigValue {
    fn from(v: ColorMode) -> Self {
        ConfigValue::ColorMode(v)
    }
}

impl From<crate::scene::palette::SqueezeMode> for ConfigValue {
    fn from(v: crate::scene::palette::SqueezeMode) -> Self {
        ConfigValue::SqueezeMode(v)
    }
}

impl From<PathMapStyle> for ConfigValue {
    fn from(v: PathMapStyle) -> Self {
        ConfigValue::PathMapStyle(v)
    }
}

impl From<PathCaptureMode> for ConfigValue {
    fn from(v: PathCaptureMode) -> Self {
        ConfigValue::PathCaptureMode(v)
    }
}

impl From<PathTrackingMode> for ConfigValue {
    fn from(v: PathTrackingMode) -> Self {
        ConfigValue::PathTrackingMode(v)
    }
}

impl From<RenderMode> for ConfigValue {
    fn from(v: RenderMode) -> Self {
        ConfigValue::RenderMode(v)
    }
}

impl From<ToneCurve> for ConfigValue {
    fn from(v: ToneCurve) -> Self {
        ConfigValue::ToneCurve(v)
    }
}

impl From<Palette> for ConfigValue {
    fn from(v: Palette) -> Self {
        ConfigValue::Palette(v)
    }
}

/// A single parameter change
#[derive(Debug, Clone)]
pub struct ConfigDelta {
    pub path: ConfigPath,
    pub old_value: ConfigValue,
    pub new_value: ConfigValue,
    pub timestamp: Instant,
}

impl ConfigDelta {
    /// Create delta by comparing values
    pub fn new(path: ConfigPath, old: ConfigValue, new: ConfigValue) -> Self {
        Self {
            path,
            old_value: old,
            new_value: new,
            timestamp: Instant::now(),
        }
    }

    /// Invert delta (for undo)
    pub fn invert(&self) -> Self {
        Self {
            path: self.path.clone(),
            old_value: self.new_value.clone(),
            new_value: self.old_value.clone(),
            timestamp: Instant::now(),
        }
    }

    /// Human-readable description using i18n key
    /// Returns serialized i18n key with params - UI translates via translate_description()
    /// Format: `key` or `key|param1=value1|param2=value2`
    pub fn description(&self) -> String {
        self.path.to_i18n_key().to_serialized()
    }
}

/// Specialized snapshot data for structural changes
/// Stores only what's needed (before/after states) for efficient undo/redo
#[derive(Debug, Clone)]
pub enum SnapshotData {
    /// Full config replacement (preset loading, file import)
    /// Stores both before and after states for bidirectional undo/redo
    FullConfig {
        before: Box<super::fractal_config::FractalConfig>,
        after: Box<super::fractal_config::FractalConfig>,
    },

    /// Transform added
    /// Undo: remove at index + restore xaos, Redo: insert at index
    AddTransform {
        index: usize,
        transform: crate::scene::transforms::Transform,
        /// Xaos matrix state before the add (restored on undo)
        xaos_before: Option<Vec<Vec<f32>>>,
        /// If Some, this was a clone — duplicates source's xaos relationships on apply/redo
        clone_from: Option<usize>,
    },

    /// Transform deleted
    /// Undo: re-insert at index + restore xaos, Redo: remove at index
    DeleteTransform {
        index: usize,
        transform: crate::scene::transforms::Transform,
        /// Xaos matrix state before the delete (restored on undo)
        xaos_before: Option<Vec<Vec<f32>>>,
    },

    /// Transform modified (affine edit, triangle editor, etc.)
    /// Stores before/after states for complete restoration. `kind`
    /// selects which pool the `index` indexes into — Triangle Editor
    /// drives all three (Normal / Linked / Final) and the apply path
    /// dispatches accordingly.
    /// Undo: restore before, Redo: restore after
    ModifyTransform {
        kind: TransformKind,
        index: usize,
        before: crate::scene::transforms::Transform,
        after: crate::scene::transforms::Transform,
    },

    /// Color effect added
    /// Undo: remove at index, Redo: insert at index
    AddColorEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Color effect deleted
    /// Undo: re-insert at index, Redo: remove at index
    DeleteColorEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Density effect added
    /// Undo: remove at index, Redo: insert at index
    AddDensityEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Density effect deleted
    /// Undo: re-insert at index, Redo: remove at index
    DeleteDensityEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Color effect moved (reordered)
    /// Undo: move from to_index back to from_index
    /// Redo: move from from_index to to_index
    MoveColorEffect { from_index: usize, to_index: usize },

    /// Density effect moved (reordered)
    /// Undo: move from to_index back to from_index
    /// Redo: move from from_index to to_index
    MoveDensityEffect { from_index: usize, to_index: usize },

    /// Subflame added.
    /// Undo: swap editing_target to `target_before`, remove subflame at `index`.
    /// Redo: swap to Main, append subflame at `index` (always end of list at
    /// time of add per the add-only-on-Main gate).
    /// `flame` is the *full* added Flame so redo recreates byte-for-byte
    /// even after intervening edits to the rest of the config.
    AddSubflame {
        index: usize,
        flame: crate::scene::transforms::Flame,
        /// Editing context at the moment of the add. Always Main today
        /// (the public API gates add on Main), but stored for future
        /// flexibility and so the undo can confidently restore context.
        target_before: super::manager::EditingTarget,
    },

    /// Subflame deleted.
    /// Undo: re-insert `flame` at `index`, swap editing_target to `target_before`.
    /// Redo: silent-swap to Main, remove subflame at `index`.
    /// `flame` holds the entire deleted Flame so undo restores it
    /// exactly — every transform, variation, parameter, even nested
    /// state — independent of any subsequent edits.
    DeleteSubflame {
        index: usize,
        flame: crate::scene::transforms::Flame,
        /// Editing context before the delete. Can be Main or any
        /// Subflame{i} (delete_subflame auto-swaps to Main internally,
        /// so we capture the pre-swap state here).
        target_before: super::manager::EditingTarget,
    },

    /// Editing target swapped (user clicked Main / Subflame N in the
    /// Subflames panel).
    /// Undo: swap to `before`. Redo: swap to `after`.
    /// No state-data change — only the editing context.
    SwapTarget {
        before: super::manager::EditingTarget,
        after: super::manager::EditingTarget,
    },
}

/// A batch of related changes (single undo point)
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub deltas: Vec<ConfigDelta>,
    pub timestamp: Instant,
    pub description: String,
    /// Snapshot data for structural changes
    /// When Some: use snapshot logic for undo/redo (bidirectional or specialized)
    /// When None: use deltas for undo/redo
    pub snapshot: Option<SnapshotData>,
    /// Last update time for coalescing sequence
    /// When coalescing: timestamp = first change, last_update_time = most recent change
    /// This allows checking both inactivity (time since last update) and total span (time since first)
    pub last_update_time: Instant,
    /// Which flame this change was made against. Undo/redo silently
    /// swaps the editing context to match this target before applying
    /// the inverse delta or snapshot. Without this tag, an edit made
    /// while on Subflame{N} would be replayed against whatever flame
    /// happens to be swapped in at undo time, silently corrupting
    /// state. Stamped by `ConfigManager::push_undo` from the manager's
    /// current `editing_target`.
    pub target: super::manager::EditingTarget,
}

impl ConfigChange {
    /// Create from single delta
    pub fn single(delta: ConfigDelta) -> Self {
        let timestamp = delta.timestamp;
        let description = delta.description();
        Self {
            deltas: vec![delta],
            timestamp,
            description,
            snapshot: None,
            last_update_time: timestamp,  // Initially same as timestamp
            // Stamped by push_undo with the manager's editing_target —
            // this default only matters for any direct ConfigChange use
            // outside the manager (e.g., tests).
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create from multiple deltas with custom description
    pub fn batch(deltas: Vec<ConfigDelta>, description: String) -> Self {
        let timestamp = deltas
            .first()
            .map(|d| d.timestamp)
            .unwrap_or_else(Instant::now);
        Self {
            deltas,
            timestamp,
            description,
            snapshot: None,
            last_update_time: timestamp,  // Initially same as timestamp
            // Stamped by push_undo with the manager's editing_target —
            // this default only matters for any direct ConfigChange use
            // outside the manager (e.g., tests).
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create full config snapshot (preset loading, file import)
    /// Stores both before and after states for bidirectional undo/redo
    pub fn full_config_snapshot(
        before: super::fractal_config::FractalConfig,
        after: super::fractal_config::FractalConfig,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::FullConfig {
                before: Box::new(before),
                after: Box::new(after),
            }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create add transform snapshot
    /// Stores the added transform and xaos state for efficient undo/redo
    pub fn add_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        xaos_before: Option<Vec<Vec<f32>>>,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddTransform { index, transform, xaos_before, clone_from: None }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create clone transform snapshot
    /// Like add, but duplicates the source transform's xaos relationships
    pub fn clone_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        xaos_before: Option<Vec<Vec<f32>>>,
        clone_from: usize,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddTransform { index, transform, xaos_before, clone_from: Some(clone_from) }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create delete transform snapshot
    /// Stores the deleted transform and xaos state for efficient undo/redo
    pub fn delete_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        xaos_before: Option<Vec<Vec<f32>>>,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteTransform { index, transform, xaos_before }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create modify transform snapshot
    /// Stores before/after transform states for complete restoration.
    /// `kind` selects which pool the `index` indexes into.
    pub fn modify_transform_snapshot(
        kind: TransformKind,
        index: usize,
        before: crate::scene::transforms::Transform,
        after: crate::scene::transforms::Transform,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::ModifyTransform { kind, index, before, after }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create add color effect snapshot
    pub fn add_color_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddColorEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create delete color effect snapshot
    pub fn delete_color_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteColorEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create add density effect snapshot
    pub fn add_density_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddDensityEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create delete density effect snapshot
    pub fn delete_density_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteDensityEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create a color effect move change (reorder)
    pub fn move_color_effect_snapshot(
        from_index: usize,
        to_index: usize,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::MoveColorEffect { from_index, to_index }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create a density effect move change (reorder)
    pub fn move_density_effect_snapshot(
        from_index: usize,
        to_index: usize,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::MoveDensityEffect { from_index, to_index }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Subflame added.
    pub fn add_subflame_snapshot(
        index: usize,
        flame: crate::scene::transforms::Flame,
        target_before: super::manager::EditingTarget,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddSubflame { index, flame, target_before }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Subflame deleted. `flame` must hold the FULL removed Flame so
    /// undo restores it byte-for-byte.
    pub fn delete_subflame_snapshot(
        index: usize,
        flame: crate::scene::transforms::Flame,
        target_before: super::manager::EditingTarget,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteSubflame { index, flame, target_before }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Editing target switched (Main ↔ Subflame{N}).
    pub fn swap_target_snapshot(
        before: super::manager::EditingTarget,
        after: super::manager::EditingTarget,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description: format!("Switch editing target ({:?} → {:?})", before, after),
            snapshot: Some(SnapshotData::SwapTarget { before, after }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Invert change (for undo)
    pub fn invert(&self) -> Self {
        let now = Instant::now();
        Self {
            deltas: self.deltas.iter().rev().map(|d| d.invert()).collect(),
            timestamp: now,
            description: format!("Undo: {}", self.description),
            snapshot: self.snapshot.clone(),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Determine update type needed for these changes
    pub fn update_type(&self) -> UpdateType {
        let mut result = UpdateType::None;
        for delta in &self.deltas {
            result = result.merge(delta.path.update_type());
        }
        result
    }
}

/// What kind of update is needed for a change
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdateType {
    None,            // No update needed
    ViewOnly,        // Just update view transform (zoom, pan, rotation)
    ToneMappingOnly, // Re-run tonemap pass (exposure, gamma)
    ShadingOnly,     // Refresh the solid-rendering shade pass params (lighting) — no reset, no re-accumulation
    ColorOnly,       // Re-run color accumulation (palette, color mode)
    IterationReset,  // Full reset - clear accumulation, restart iterations
    /// Re-render the escape-time (fragment mode) frame. Last, so
    /// `merge` (which takes the Ord max) never downgrades it: a change
    /// set mixing escape and flame paths must still re-render escape,
    /// and the flame flags ride along in UpdateAction's union anyway.
    EscapeRerender,
    /// Recolour the simulation from its current field: a colouring,
    /// palette or resolve-filter change that must not disturb the run.
    SimRerender,
    /// Resample the running field into a new grid (a bound grid's
    /// scale changed). Keeps the run; interpolates once.
    SimResample,
    /// Restart the run from the seed: model, seed, init, boundary or a
    /// grid size changed, and none of those can be carried across.
    ///
    /// Last in the ordering deliberately. `merge` takes the Ord max, so
    /// a change set that both recolours and reseeds reseeds — the
    /// stronger action subsumes the weaker, never the other way round.
    SimReseed,
}

impl UpdateType {
    /// Merge two update types (take the more severe one)
    pub fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

impl ConfigPath {
    /// What kind of update does changing this parameter require?
    pub fn update_type(&self) -> UpdateType {
        match self {
            // View parameters - just math, no GPU work
            ConfigPath::Zoom
            | ConfigPath::Pan
            | ConfigPath::PanX
            | ConfigPath::PanY
            | ConfigPath::Rotation
            | ConfigPath::CameraRotationX
            | ConfigPath::CameraRotationY
            | ConfigPath::CameraBank
            | ConfigPath::CameraX
            | ConfigPath::CameraY
            | ConfigPath::CameraZ => UpdateType::ViewOnly,

            // DOF, fog, and spatial filter change the per-sample
            // contribution shape — old accumulation is "stale" relative
            // to the new dynamics. Reset so the user sees the new look
            // cleanly rather than seeing it average in over many frames.
            ConfigPath::DofFocusDistance
            | ConfigPath::DofBlurStrength
            | ConfigPath::FogStrength
            | ConfigPath::FogStart
            | ConfigPath::FarDensityFade
            | ConfigPath::SolidStrength
            | ConfigPath::SurfaceThickness
            | ConfigPath::FarDensityFadeStart
            | ConfigPath::FilterRadius
            | ConfigPath::FilterBlurEdges => UpdateType::IterationReset,

            // Solid-rendering LIGHTING: the shade pass runs after
            // accumulation, so lighting never invalidates accumulated
            // data — refresh the renderer's shading copy and let the
            // per-frame shade+tonemap pick it up. The one structural
            // case (a change flipping the depth-capture requirement,
            // e.g. lighting toggled on while solid_strength is 0) is
            // escalated by the app layer to a full flame update.
            ConfigPath::ShadingStrength
            | ConfigPath::SolidAmbient
            | ConfigPath::SolidDiffuse
            | ConfigPath::SolidSpecular
            | ConfigPath::SolidShininess
            | ConfigPath::SsaoStrength
            | ConfigPath::SsaoRadius
            | ConfigPath::NormalSmoothing
            | ConfigPath::GapFill
            | ConfigPath::SolidShadowStrength
            | ConfigPath::SolidLightEnabled { .. }
            | ConfigPath::SolidLightParam { .. } => UpdateType::ShadingOnly,

            // Tone mapping - re-run tonemap shader
            ConfigPath::Exposure
            | ConfigPath::Gamma
            | ConfigPath::GammaThreshold
            | ConfigPath::Brightness
            | ConfigPath::Vibrancy
            | ConfigPath::WhiteLevel
            | ConfigPath::Saturation
            | ConfigPath::HueShift
            | ConfigPath::AlphaBlendLow
            | ConfigPath::AlphaBlendHigh
            | ConfigPath::DensityScale
            | ConfigPath::TonemapMode
            | ConfigPath::HighlightMode
            | ConfigPath::TonemapCurve
            | ConfigPath::UseCurve
            | ConfigPath::BackgroundColor
            | ConfigPath::BackgroundColorR
            | ConfigPath::BackgroundColorG
            | ConfigPath::BackgroundColorB
            | ConfigPath::LevelsEnabled
            | ConfigPath::LevelsLow
            | ConfigPath::LevelsHigh
            | ConfigPath::LevelsGamma => UpdateType::ToneMappingOnly,

            // Color parameters - re-run accumulation with new colors
            ConfigPath::ColorMode
            | ConfigPath::PaletteIndex
            | ConfigPath::Palette
            | ConfigPath::PaletteRotation
            | ConfigPath::PaletteSize
            | ConfigPath::PaletteSqueeze
            | ConfigPath::PaletteSqueezeMode
            | ConfigPath::PaletteSqueezeFalloff
            | ConfigPath::PaletteLogStrength
            | ConfigPath::PaletteReverse
            | ConfigPath::SpeedFactor
            // PathMapStyle affects color computation in compute shader, needs accumulation reset
            | ConfigPath::PathMapStyle => UpdateType::ColorOnly,

            // PathCaptureMode affects path buffer capture logic in compute shader
            ConfigPath::PathCaptureMode => UpdateType::IterationReset,

            // PathTrackingMode affects path tracking logic in compute shader
            ConfigPath::PathTrackingMode => UpdateType::IterationReset,

            // Rendering settings - affect iteration behavior
            ConfigPath::BlendFactor
            | ConfigPath::UseDynamicBlend => UpdateType::IterationReset,

            // Transform/flame changes - full reset
            ConfigPath::TransformCount
            | ConfigPath::TransformWeight { .. }
            | ConfigPath::TransformColor { .. }
            | ConfigPath::TransformColorSpeed { .. }
            | ConfigPath::TransformOpacity { .. }
            | ConfigPath::TransformDirectColor { .. }
            | ConfigPath::TransformAffine { .. }
            | ConfigPath::TransformPostAffineEnabled { .. }
            | ConfigPath::TransformPostAffine { .. }
            | ConfigPath::TransformYzCoefs { .. }
            | ConfigPath::TransformZxCoefs { .. }
            | ConfigPath::TransformYzPostCoefs { .. }
            | ConfigPath::TransformZxPostCoefs { .. }
            | ConfigPath::TransformVariation { .. }
            | ConfigPath::TransformVariationParam { .. }
            | ConfigPath::TransformVariationPriority { .. }
            | ConfigPath::LinkedTransformVariationPriority { .. }
            | ConfigPath::FinalTransformVariationPriority { .. }
            | ConfigPath::TransformVariationOrder { .. }
            | ConfigPath::LinkedTransformVariationOrder { .. }
            | ConfigPath::FinalTransformVariationOrder { .. }
            | ConfigPath::TransformOriginX { .. }
            | ConfigPath::TransformOriginY { .. }
            | ConfigPath::TransformRotation { .. }
            | ConfigPath::TransformScale { .. }
            | ConfigPath::TransformPostAffineOriginX { .. }
            | ConfigPath::TransformPostAffineOriginY { .. }
            | ConfigPath::TransformPostAffineRotation { .. }
            | ConfigPath::TransformPostAffineScale { .. }
            | ConfigPath::LinkedTransformAffine { .. }
            | ConfigPath::LinkedTransformPostAffineEnabled { .. }
            | ConfigPath::LinkedTransformPostAffine { .. }
            | ConfigPath::LinkedTransformYzCoefs { .. }
            | ConfigPath::LinkedTransformZxCoefs { .. }
            | ConfigPath::LinkedTransformYzPostCoefs { .. }
            | ConfigPath::LinkedTransformZxPostCoefs { .. }
            | ConfigPath::LinkedTransformVariation { .. }
            | ConfigPath::LinkedTransformVariationParam { .. }
            | ConfigPath::LinkedTransformOriginX { .. }
            | ConfigPath::LinkedTransformOriginY { .. }
            | ConfigPath::LinkedTransformRotation { .. }
            | ConfigPath::LinkedTransformScale { .. }
            | ConfigPath::LinkedTransformPostAffineOriginX { .. }
            | ConfigPath::LinkedTransformPostAffineOriginY { .. }
            | ConfigPath::LinkedTransformPostAffineRotation { .. }
            | ConfigPath::LinkedTransformPostAffineScale { .. }
            | ConfigPath::FinalTransformAffine { .. }
            | ConfigPath::FinalTransformPostAffineEnabled { .. }
            | ConfigPath::FinalTransformPostAffine { .. }
            | ConfigPath::FinalTransformYzCoefs { .. }
            | ConfigPath::FinalTransformZxCoefs { .. }
            | ConfigPath::FinalTransformYzPostCoefs { .. }
            | ConfigPath::FinalTransformZxPostCoefs { .. }
            | ConfigPath::FinalTransformVariation { .. }
            | ConfigPath::FinalTransformVariationParam { .. }
            | ConfigPath::FinalTransformOriginX { .. }
            | ConfigPath::FinalTransformOriginY { .. }
            | ConfigPath::FinalTransformRotation { .. }
            | ConfigPath::FinalTransformScale { .. }
            | ConfigPath::FinalTransformPostAffineOriginX { .. }
            | ConfigPath::FinalTransformPostAffineOriginY { .. }
            | ConfigPath::FinalTransformPostAffineRotation { .. }
            | ConfigPath::FinalTransformPostAffineScale { .. }
            | ConfigPath::RenderMode
            | ConfigPath::PerspectiveStrength
            | ConfigPath::DepthDensityCompensation
            | ConfigPath::Xaos { .. }
            | ConfigPath::SoloTransform
            | ConfigPath::PostSymmetryType
            | ConfigPath::PostSymmetryOrder
            | ConfigPath::PostSymmetryCenterX
            | ConfigPath::PostSymmetryCenterY
            | ConfigPath::PostSymmetryDistance
            | ConfigPath::PostSymmetryRotation
            | ConfigPath::PreserveZ
            | ConfigPath::MaxIterations
            | ConfigPath::DeterministicRng => UpdateType::IterationReset,

            // Escape-time: the fragment renderer re-renders the frame;
            // no flame-style reset/accumulate distinction exists there.
            ConfigPath::EscapeFormula
            | ConfigPath::EscapeJulia
            | ConfigPath::EscapeJuliaRe
            | ConfigPath::EscapeJuliaIm
            | ConfigPath::EscapeCenterRe
            | ConfigPath::EscapeCenterIm
            | ConfigPath::EscapeZoomLog2
            | ConfigPath::EscapeRotation
            | ConfigPath::EscapeMaxIter
            | ConfigPath::EscapeSupersample
            | ConfigPath::EscapeDownsample
            | ConfigPath::EscapeReferencePeriod
            | ConfigPath::EscapeBailout
            | ConfigPath::EscapeDampingRe
            | ConfigPath::EscapeDampingIm
            | ConfigPath::EscapeBiomorph
            | ConfigPath::EscapeShadingEnabled
            | ConfigPath::EscapeShadingLightAngle
            | ConfigPath::EscapeShadingHeight
            | ConfigPath::EscapeContrastMode
            | ConfigPath::EscapeContrastClip
            | ConfigPath::EscapeContrastStrength
            | ConfigPath::EscapeContrastTurns
            | ConfigPath::EscapeShadingField
            | ConfigPath::EscapeShadingShadowColor
            | ConfigPath::EscapeShadingShadowStrength
            | ConfigPath::EscapeShadingShadowBlend
            | ConfigPath::EscapeShadingHighlightColor
            | ConfigPath::EscapeShadingHighlightStrength
            | ConfigPath::EscapeShadingHighlightBlend
            | ConfigPath::EscapeShadingSoftness
            | ConfigPath::EscapeShadingTextureKind
            | ConfigPath::EscapeShadingTextureStrength
            | ConfigPath::EscapeShadingTextureScale
            | ConfigPath::EscapeColoring
            | ConfigPath::EscapeFormulaParam { .. }
            | ConfigPath::EscapeColoringParam { .. } => UpdateType::EscapeRerender,

            // Simulation: split by how much of the run survives. This
            // grouping is the whole reason there are three update types
            // rather than one -- a colouring tweak must not throw away
            // a 10,000-step field, and a model change cannot keep it.
            ConfigPath::SimColoring
            | ConfigPath::SimUpscale
            | ConfigPath::SimDownscale
            | ConfigPath::SimSteps
            | ConfigPath::SimStepsPerFrame
            | ConfigPath::SimDt
            // The warp changes what the NEXT steps do to the field,
            // not the field: the run continues.
            | ConfigPath::SimWarpZoom
            | ConfigPath::SimWarpRotation
            | ConfigPath::SimWarpPanX
            | ConfigPath::SimWarpPanY
            | ConfigPath::SimWarpFlow
            | ConfigPath::SimWarpFilter
            // The matte is a colouring decision: the field is
            // untouched, only which of it is drawn.
            | ConfigPath::SimMatteChannel
            | ConfigPath::SimMatteCutoff
            | ConfigPath::SimMatteSoftness
            | ConfigPath::SimMatteInvert
            | ConfigPath::SimMatteEdge
            | ConfigPath::SimModelParam { .. }
            | ConfigPath::SimColoringParam { .. } => UpdateType::SimRerender,

            // A bound grid's scale change resamples the live field
            // rather than restarting: the run continues at a new
            // resolution (pipeline section 7).
            ConfigPath::SimGridScale => UpdateType::SimResample,

            // Nothing here can be carried across: the model's rule, the
            // field's contents, the lattice size or what a step reads
            // at the edges all change what the state MEANS.
            ConfigPath::SimModel
            | ConfigPath::SimGridMode
            | ConfigPath::SimGridWidth
            | ConfigPath::SimGridHeight
            | ConfigPath::SimSeed
            | ConfigPath::SimInitKind
            | ConfigPath::SimInitAmplitude
            | ConfigPath::SimInitRadius
            | ConfigPath::SimInitCount
            | ConfigPath::SimBoundary => UpdateType::SimReseed,

            // Effects (post-processing, just need tonemap re-run)
            ConfigPath::DensityEffectEnabled { .. }
            | ConfigPath::DensityEffectParam { .. }
            | ConfigPath::ColorEffectEnabled { .. }
            | ConfigPath::ColorEffectParam { .. }
            | ConfigPath::AddColorEffect { .. }
            | ConfigPath::RemoveColorEffect { .. }
            | ConfigPath::AddDensityEffect { .. }
            | ConfigPath::RemoveDensityEffect { .. } => UpdateType::ToneMappingOnly,

            // System Settings
            ConfigPath::SystemIterationsPerThread | ConfigPath::SystemBurnIn => UpdateType::IterationReset,
            // Disk housekeeping only: nothing on the GPU changes.
            ConfigPath::SystemOrbitCacheMb => UpdateType::None,
            ConfigPath::SystemVsyncEnabled | ConfigPath::SystemTargetFps | ConfigPath::SystemFlyMouseSensitivity | ConfigPath::SystemFlyMoveSpeed | ConfigPath::SystemFlySprintMultiplier | ConfigPath::SystemFlyInvertY | ConfigPath::SystemFlyCameraMode => UpdateType::ViewOnly,
            ConfigPath::SystemExportWidth | ConfigPath::SystemExportHeight | ConfigPath::SystemLanguage | ConfigPath::SystemShowHelpOnStartup
            // Nothing to re-render: it only changes what a future
            // export writes into the file.
            | ConfigPath::SystemPngStripMetadata => UpdateType::None,
        }
    }

    /// Serialize ConfigPath to a stable string key for animation files
    ///
    /// Format examples:
    /// - Simple: "Zoom", "Exposure", "Pan"
    /// - Transform: "Transform.0.Weight", "Transform.0.Affine.A"
    /// - Variation: "Transform.0.Variation.linear", "Transform.0.VariationParam.julian.power"
    pub fn to_string_key(&self) -> String {
        match self {
            // View
            ConfigPath::Zoom => "Zoom".to_string(),
            ConfigPath::Pan => "Pan".to_string(),
            ConfigPath::PanX => "PanX".to_string(),
            ConfigPath::PanY => "PanY".to_string(),
            ConfigPath::Rotation => "Rotation".to_string(),
            ConfigPath::CameraRotationX => "CameraRotationX".to_string(),
            ConfigPath::CameraRotationY => "CameraRotationY".to_string(),
            ConfigPath::CameraBank => "CameraBank".to_string(),
            ConfigPath::CameraX => "CameraX".to_string(),
            ConfigPath::CameraY => "CameraY".to_string(),
            ConfigPath::CameraZ => "CameraZ".to_string(),
            ConfigPath::DofFocusDistance => "DofFocusDistance".to_string(),
            ConfigPath::DofBlurStrength => "DofBlurStrength".to_string(),
            ConfigPath::FogStrength => "FogStrength".to_string(),
            ConfigPath::FogStart => "FogStart".to_string(),
            ConfigPath::FilterRadius => "FilterRadius".to_string(),
            ConfigPath::FilterBlurEdges => "FilterBlurEdges".to_string(),

            // Tone mapping
            ConfigPath::Exposure => "Exposure".to_string(),
            ConfigPath::Gamma => "Gamma".to_string(),
            ConfigPath::GammaThreshold => "GammaThreshold".to_string(),
            ConfigPath::Brightness => "Brightness".to_string(),
            ConfigPath::Vibrancy => "Vibrancy".to_string(),
            ConfigPath::WhiteLevel => "WhiteLevel".to_string(),
            ConfigPath::Saturation => "Saturation".to_string(),
            ConfigPath::HueShift => "HueShift".to_string(),
            ConfigPath::AlphaBlendLow => "AlphaBlendLow".to_string(),
            ConfigPath::AlphaBlendHigh => "AlphaBlendHigh".to_string(),
            ConfigPath::DensityScale => "DensityScale".to_string(),
            ConfigPath::TonemapMode => "TonemapMode".to_string(),
            ConfigPath::HighlightMode => "HighlightMode".to_string(),
            ConfigPath::TonemapCurve => "TonemapCurve".to_string(),
            ConfigPath::UseCurve => "UseCurve".to_string(),
            ConfigPath::LevelsEnabled => "LevelsEnabled".to_string(),
            ConfigPath::LevelsLow => "LevelsLow".to_string(),
            ConfigPath::LevelsHigh => "LevelsHigh".to_string(),
            ConfigPath::LevelsGamma => "LevelsGamma".to_string(),

            // Color
            ConfigPath::ColorMode => "ColorMode".to_string(),
            ConfigPath::PathMapStyle => "PathMapStyle".to_string(),
            ConfigPath::PathCaptureMode => "PathCaptureMode".to_string(),
            ConfigPath::PathTrackingMode => "PathTrackingMode".to_string(),
            ConfigPath::PaletteIndex => "PaletteIndex".to_string(),
            ConfigPath::Palette => "Palette".to_string(),
            ConfigPath::PaletteRotation => "PaletteRotation".to_string(),
            ConfigPath::PaletteSize => "PaletteSize".to_string(),
            ConfigPath::PaletteSqueeze => "PaletteSqueeze".to_string(),
            ConfigPath::PaletteSqueezeMode => "PaletteSqueezeMode".to_string(),
            ConfigPath::PaletteSqueezeFalloff => "PaletteSqueezeFalloff".to_string(),
            ConfigPath::PaletteLogStrength => "PaletteLogStrength".to_string(),
            ConfigPath::PaletteReverse => "PaletteReverse".to_string(),
            ConfigPath::SpeedFactor => "SpeedFactor".to_string(),
            ConfigPath::BackgroundColor => "BackgroundColor".to_string(),
            ConfigPath::BackgroundColorR => "BackgroundColorR".to_string(),
            ConfigPath::BackgroundColorG => "BackgroundColorG".to_string(),
            ConfigPath::BackgroundColorB => "BackgroundColorB".to_string(),

            // Rendering
            ConfigPath::BlendFactor => "BlendFactor".to_string(),
            ConfigPath::UseDynamicBlend => "UseDynamicBlend".to_string(),
            ConfigPath::MaxIterations => "MaxIterations".to_string(),
            ConfigPath::DeterministicRng => "DeterministicRng".to_string(),

            // Transforms
            ConfigPath::TransformCount => "TransformCount".to_string(),
            ConfigPath::TransformWeight { index } => format!("Transform.{}.Weight", index),
            ConfigPath::TransformColor { index } => format!("Transform.{}.Color", index),
            ConfigPath::TransformColorSpeed { index } => format!("Transform.{}.ColorSpeed", index),
            ConfigPath::TransformOpacity { index } => format!("Transform.{}.Opacity", index),
            ConfigPath::TransformDirectColor { index } => format!("Transform.{}.DirectColor", index),
            ConfigPath::TransformAffine { index, param } => {
                format!("Transform.{}.Affine.{}", index, param.to_char())
            }
            ConfigPath::TransformVariation { index, variation } => {
                format!("Transform.{}.Variation.{}", index, variation)
            }
            ConfigPath::TransformVariationPriority { index, variation } => {
                format!("Transform.{}.VariationPriority.{}", index, variation)
            }
            ConfigPath::TransformVariationOrder { index } => {
                format!("Transform.{}.VariationOrder", index)
            }
            ConfigPath::LinkedTransformVariationOrder { index } => {
                format!("LinkedTransform.{}.VariationOrder", index)
            }
            ConfigPath::FinalTransformVariationOrder { index } => {
                format!("FinalTransform.{}.VariationOrder", index)
            }
            ConfigPath::TransformVariationParam { index, variation, param } => {
                format!("Transform.{}.VariationParam.{}.{}", index, variation, param)
            }
            ConfigPath::TransformOriginX { index } => format!("Transform.{}.OriginX", index),
            ConfigPath::TransformOriginY { index } => format!("Transform.{}.OriginY", index),
            ConfigPath::TransformRotation { index } => format!("Transform.{}.Rotation", index),
            ConfigPath::TransformScale { index } => format!("Transform.{}.Scale", index),
            ConfigPath::TransformPostAffineEnabled { index } => format!("Transform.{}.PostAffineEnabled", index),
            ConfigPath::TransformPostAffine { index, param } => {
                format!("Transform.{}.PostAffine.{}", index, param.to_char())
            }
            ConfigPath::TransformYzCoefs { index, position } => {
                format!("Transform.{}.YzCoefs.{}", index, position)
            }
            ConfigPath::TransformZxCoefs { index, position } => {
                format!("Transform.{}.ZxCoefs.{}", index, position)
            }
            ConfigPath::TransformYzPostCoefs { index, position } => {
                format!("Transform.{}.YzPostCoefs.{}", index, position)
            }
            ConfigPath::TransformZxPostCoefs { index, position } => {
                format!("Transform.{}.ZxPostCoefs.{}", index, position)
            }
            ConfigPath::TransformPostAffineOriginX { index } => format!("Transform.{}.PostAffineOriginX", index),
            ConfigPath::TransformPostAffineOriginY { index } => format!("Transform.{}.PostAffineOriginY", index),
            ConfigPath::TransformPostAffineRotation { index } => format!("Transform.{}.PostAffineRotation", index),
            ConfigPath::TransformPostAffineScale { index } => format!("Transform.{}.PostAffineScale", index),

            // Linked Transform pool
            ConfigPath::LinkedTransformAffine { index, param } => {
                format!("LinkedTransform.{}.Affine.{}", index, param.to_char())
            }
            ConfigPath::LinkedTransformPostAffineEnabled { index } => {
                format!("LinkedTransform.{}.PostAffineEnabled", index)
            }
            ConfigPath::LinkedTransformPostAffine { index, param } => {
                format!("LinkedTransform.{}.PostAffine.{}", index, param.to_char())
            }
            ConfigPath::LinkedTransformYzCoefs { index, position } => {
                format!("LinkedTransform.{}.YzCoefs.{}", index, position)
            }
            ConfigPath::LinkedTransformZxCoefs { index, position } => {
                format!("LinkedTransform.{}.ZxCoefs.{}", index, position)
            }
            ConfigPath::LinkedTransformYzPostCoefs { index, position } => {
                format!("LinkedTransform.{}.YzPostCoefs.{}", index, position)
            }
            ConfigPath::LinkedTransformZxPostCoefs { index, position } => {
                format!("LinkedTransform.{}.ZxPostCoefs.{}", index, position)
            }
            ConfigPath::LinkedTransformVariation { index, variation } => {
                format!("LinkedTransform.{}.Variation.{}", index, variation)
            }
            ConfigPath::LinkedTransformVariationPriority { index, variation } => {
                format!("LinkedTransform.{}.VariationPriority.{}", index, variation)
            }
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
                format!("LinkedTransform.{}.VariationParam.{}.{}", index, variation, param)
            }
            ConfigPath::LinkedTransformOriginX { index } => format!("LinkedTransform.{}.OriginX", index),
            ConfigPath::LinkedTransformOriginY { index } => format!("LinkedTransform.{}.OriginY", index),
            ConfigPath::LinkedTransformRotation { index } => format!("LinkedTransform.{}.Rotation", index),
            ConfigPath::LinkedTransformScale { index } => format!("LinkedTransform.{}.Scale", index),
            ConfigPath::LinkedTransformPostAffineOriginX { index } => format!("LinkedTransform.{}.PostAffineOriginX", index),
            ConfigPath::LinkedTransformPostAffineOriginY { index } => format!("LinkedTransform.{}.PostAffineOriginY", index),
            ConfigPath::LinkedTransformPostAffineRotation { index } => format!("LinkedTransform.{}.PostAffineRotation", index),
            ConfigPath::LinkedTransformPostAffineScale { index } => format!("LinkedTransform.{}.PostAffineScale", index),

            // Final Transform pool. Emits the new `FinalTransform.{index}.{field}`
            // format. The parser in `from_string_key` also accepts the
            // historical `PoolFinalTransform.{index}.{field}` form (saved by
            // the prior branch) and the legacy `FinalTransform.{field}`
            // (no index, single-final) form for backward compat.
            ConfigPath::FinalTransformAffine { index, param } => {
                format!("FinalTransform.{}.Affine.{}", index, param.to_char())
            }
            ConfigPath::FinalTransformPostAffineEnabled { index } => {
                format!("FinalTransform.{}.PostAffineEnabled", index)
            }
            ConfigPath::FinalTransformPostAffine { index, param } => {
                format!("FinalTransform.{}.PostAffine.{}", index, param.to_char())
            }
            ConfigPath::FinalTransformYzCoefs { index, position } => {
                format!("FinalTransform.{}.YzCoefs.{}", index, position)
            }
            ConfigPath::FinalTransformZxCoefs { index, position } => {
                format!("FinalTransform.{}.ZxCoefs.{}", index, position)
            }
            ConfigPath::FinalTransformYzPostCoefs { index, position } => {
                format!("FinalTransform.{}.YzPostCoefs.{}", index, position)
            }
            ConfigPath::FinalTransformZxPostCoefs { index, position } => {
                format!("FinalTransform.{}.ZxPostCoefs.{}", index, position)
            }
            ConfigPath::FinalTransformVariation { index, variation } => {
                format!("FinalTransform.{}.Variation.{}", index, variation)
            }
            ConfigPath::FinalTransformVariationPriority { index, variation } => {
                format!("FinalTransform.{}.VariationPriority.{}", index, variation)
            }
            ConfigPath::FinalTransformVariationParam { index, variation, param } => {
                format!("FinalTransform.{}.VariationParam.{}.{}", index, variation, param)
            }
            ConfigPath::FinalTransformOriginX { index } => format!("FinalTransform.{}.OriginX", index),
            ConfigPath::FinalTransformOriginY { index } => format!("FinalTransform.{}.OriginY", index),
            ConfigPath::FinalTransformRotation { index } => format!("FinalTransform.{}.Rotation", index),
            ConfigPath::FinalTransformScale { index } => format!("FinalTransform.{}.Scale", index),
            ConfigPath::FinalTransformPostAffineOriginX { index } => format!("FinalTransform.{}.PostAffineOriginX", index),
            ConfigPath::FinalTransformPostAffineOriginY { index } => format!("FinalTransform.{}.PostAffineOriginY", index),
            ConfigPath::FinalTransformPostAffineRotation { index } => format!("FinalTransform.{}.PostAffineRotation", index),
            ConfigPath::FinalTransformPostAffineScale { index } => format!("FinalTransform.{}.PostAffineScale", index),

            // Flame
            ConfigPath::RenderMode => "RenderMode".to_string(),

            // Escape-time
            ConfigPath::EscapeFormula => "Escape.Formula".to_string(),
            ConfigPath::EscapeJulia => "Escape.Julia".to_string(),
            ConfigPath::EscapeJuliaRe => "Escape.JuliaRe".to_string(),
            ConfigPath::EscapeJuliaIm => "Escape.JuliaIm".to_string(),
            ConfigPath::EscapeCenterRe => "Escape.CenterRe".to_string(),
            ConfigPath::EscapeCenterIm => "Escape.CenterIm".to_string(),
            ConfigPath::EscapeZoomLog2 => "Escape.ZoomLog2".to_string(),
            ConfigPath::EscapeRotation => "Escape.Rotation".to_string(),
            ConfigPath::EscapeMaxIter => "Escape.MaxIter".to_string(),
            ConfigPath::SimModel => "Sim.Model".to_string(),
            ConfigPath::SimColoring => "Sim.Coloring".to_string(),
            ConfigPath::SimGridMode => "Sim.GridMode".to_string(),
            ConfigPath::SimGridWidth => "Sim.GridWidth".to_string(),
            ConfigPath::SimGridHeight => "Sim.GridHeight".to_string(),
            ConfigPath::SimGridScale => "Sim.GridScale".to_string(),
            ConfigPath::SimSeed => "Sim.Seed".to_string(),
            ConfigPath::SimInitKind => "Sim.InitKind".to_string(),
            ConfigPath::SimInitAmplitude => "Sim.InitAmplitude".to_string(),
            ConfigPath::SimInitRadius => "Sim.InitRadius".to_string(),
            ConfigPath::SimInitCount => "Sim.InitCount".to_string(),
            ConfigPath::SimSteps => "Sim.Steps".to_string(),
            ConfigPath::SimStepsPerFrame => "Sim.StepsPerFrame".to_string(),
            ConfigPath::SimDt => "Sim.Dt".to_string(),
            ConfigPath::SimBoundary => "Sim.Boundary".to_string(),
            ConfigPath::SimWarpZoom => "Sim.WarpZoom".to_string(),
            ConfigPath::SimWarpRotation => "Sim.WarpRotation".to_string(),
            ConfigPath::SimWarpPanX => "Sim.WarpPanX".to_string(),
            ConfigPath::SimWarpPanY => "Sim.WarpPanY".to_string(),
            ConfigPath::SimWarpFlow => "Sim.WarpFlow".to_string(),
            ConfigPath::SimWarpFilter => "Sim.WarpFilter".to_string(),
            ConfigPath::SimMatteChannel => "Sim.MatteChannel".to_string(),
            ConfigPath::SimMatteCutoff => "Sim.MatteCutoff".to_string(),
            ConfigPath::SimMatteSoftness => "Sim.MatteSoftness".to_string(),
            ConfigPath::SimMatteInvert => "Sim.MatteInvert".to_string(),
            ConfigPath::SimMatteEdge => "Sim.MatteEdge".to_string(),
            ConfigPath::SimUpscale => "Sim.Upscale".to_string(),
            ConfigPath::SimDownscale => "Sim.Downscale".to_string(),
            ConfigPath::SimModelParam { param } => format!("Sim.ModelParam.{param}"),
            ConfigPath::SimColoringParam { param } => format!("Sim.ColoringParam.{param}"),
            ConfigPath::EscapeSupersample => "Escape.Supersample".to_string(),
            ConfigPath::EscapeDownsample => "Escape.Downsample".to_string(),
            ConfigPath::EscapeReferencePeriod => "Escape.ReferencePeriod".to_string(),
            ConfigPath::EscapeBailout => "Escape.Bailout".to_string(),
            ConfigPath::EscapeDampingRe => "Escape.DampingRe".to_string(),
            ConfigPath::EscapeDampingIm => "Escape.DampingIm".to_string(),
            ConfigPath::EscapeBiomorph => "Escape.Biomorph".to_string(),
            ConfigPath::EscapeShadingEnabled => "Escape.Shading.Enabled".to_string(),
            ConfigPath::EscapeShadingLightAngle => "Escape.Shading.LightAngle".to_string(),
            ConfigPath::EscapeShadingHeight => "Escape.Shading.Height".to_string(),
            ConfigPath::EscapeShadingField => "Escape.Shading.Field".to_string(),
            ConfigPath::EscapeContrastMode => "Escape.Contrast.Mode".to_string(),
            ConfigPath::EscapeContrastClip => "Escape.Contrast.Clip".to_string(),
            ConfigPath::EscapeContrastStrength => "Escape.Contrast.Strength".to_string(),
            ConfigPath::EscapeContrastTurns => "Escape.Contrast.Turns".to_string(),
            ConfigPath::EscapeShadingShadowColor => "Escape.Shading.ShadowColor".to_string(),
            ConfigPath::EscapeShadingShadowStrength => "Escape.Shading.ShadowStrength".to_string(),
            ConfigPath::EscapeShadingShadowBlend => "Escape.Shading.ShadowBlend".to_string(),
            ConfigPath::EscapeShadingHighlightColor => "Escape.Shading.HighlightColor".to_string(),
            ConfigPath::EscapeShadingHighlightStrength => "Escape.Shading.HighlightStrength".to_string(),
            ConfigPath::EscapeShadingHighlightBlend => "Escape.Shading.HighlightBlend".to_string(),
            ConfigPath::EscapeShadingSoftness => "Escape.Shading.Softness".to_string(),
            ConfigPath::EscapeShadingTextureKind => "Escape.Shading.TextureKind".to_string(),
            ConfigPath::EscapeShadingTextureStrength => "Escape.Shading.TextureStrength".to_string(),
            ConfigPath::EscapeShadingTextureScale => "Escape.Shading.TextureScale".to_string(),
            ConfigPath::EscapeColoring => "Escape.Coloring".to_string(),
            ConfigPath::EscapeFormulaParam { param } => format!("Escape.FormulaParam.{param}"),
            ConfigPath::EscapeColoringParam { param } => format!("Escape.ColoringParam.{param}"),
            ConfigPath::PerspectiveStrength => "PerspectiveStrength".to_string(),
            ConfigPath::DepthDensityCompensation => "DepthDensityCompensation".to_string(),
            ConfigPath::FarDensityFade => "FarDensityFade".to_string(),
            ConfigPath::SolidStrength => "SolidStrength".to_string(),
            ConfigPath::SurfaceThickness => "SurfaceThickness".to_string(),
            ConfigPath::SolidShadowStrength => "SolidShadowStrength".to_string(),
            ConfigPath::ShadingStrength => "ShadingStrength".to_string(),
            ConfigPath::SolidAmbient => "SolidAmbient".to_string(),
            ConfigPath::SolidDiffuse => "SolidDiffuse".to_string(),
            ConfigPath::SolidSpecular => "SolidSpecular".to_string(),
            ConfigPath::SolidShininess => "SolidShininess".to_string(),
            ConfigPath::SsaoStrength => "SsaoStrength".to_string(),
            ConfigPath::SsaoRadius => "SsaoRadius".to_string(),
            ConfigPath::NormalSmoothing => "NormalSmoothing".to_string(),
            ConfigPath::GapFill => "GapFill".to_string(),
            ConfigPath::SolidLightEnabled { index } => format!("SolidLight.{}.enabled", index),
            ConfigPath::SolidLightParam { index, param } => format!("SolidLight.{}.{}", index, param),
            ConfigPath::FarDensityFadeStart => "FarDensityFadeStart".to_string(),
            ConfigPath::Xaos { src, dst } => format!("Xaos.{}.{}", src, dst),
            ConfigPath::SoloTransform => "SoloTransform".to_string(),
            ConfigPath::PostSymmetryType => "PostSymmetryType".to_string(),
            ConfigPath::PostSymmetryOrder => "PostSymmetryOrder".to_string(),
            ConfigPath::PostSymmetryCenterX => "PostSymmetryCenterX".to_string(),
            ConfigPath::PostSymmetryCenterY => "PostSymmetryCenterY".to_string(),
            ConfigPath::PostSymmetryDistance => "PostSymmetryDistance".to_string(),
            ConfigPath::PostSymmetryRotation => "PostSymmetryRotation".to_string(),
            ConfigPath::PreserveZ => "PreserveZ".to_string(),

            // Effects
            ConfigPath::DensityEffectEnabled { index } => format!("DensityEffect.{}.Enabled", index),
            ConfigPath::DensityEffectParam { index, param } => format!("DensityEffect.{}.{}", index, param),
            ConfigPath::ColorEffectEnabled { index } => format!("ColorEffect.{}.Enabled", index),
            ConfigPath::ColorEffectParam { index, param } => format!("ColorEffect.{}.{}", index, param),
            ConfigPath::AddColorEffect { effect_type } => format!("ColorEffect.Add.{}", effect_type),
            ConfigPath::RemoveColorEffect { index } => format!("ColorEffect.Remove.{}", index),
            ConfigPath::AddDensityEffect { effect_type } => format!("DensityEffect.Add.{}", effect_type),
            ConfigPath::RemoveDensityEffect { index } => format!("DensityEffect.Remove.{}", index),

            // System Settings (not typically animated, but included for completeness)
            ConfigPath::SystemIterationsPerThread => "System.IterationsPerThread".to_string(),
            ConfigPath::SystemBurnIn => "System.BurnIn".to_string(),
            ConfigPath::SystemOrbitCacheMb => "System.OrbitCacheMb".to_string(),
            ConfigPath::SystemVsyncEnabled => "System.VsyncEnabled".to_string(),
            ConfigPath::SystemTargetFps => "System.TargetFps".to_string(),
            ConfigPath::SystemFlyMouseSensitivity => "System.FlyMouseSensitivity".to_string(),
            ConfigPath::SystemFlyMoveSpeed => "System.FlyMoveSpeed".to_string(),
            ConfigPath::SystemFlySprintMultiplier => "System.FlySprintMultiplier".to_string(),
            ConfigPath::SystemFlyInvertY => "System.FlyInvertY".to_string(),
            ConfigPath::SystemFlyCameraMode => "System.FlyCameraMode".to_string(),
            ConfigPath::SystemExportWidth => "System.ExportWidth".to_string(),
            ConfigPath::SystemExportHeight => "System.ExportHeight".to_string(),
            ConfigPath::SystemPngStripMetadata => "System.PngStripMetadata".to_string(),
            ConfigPath::SystemLanguage => "System.Language".to_string(),
            ConfigPath::SystemShowHelpOnStartup => "System.ShowHelpOnStartup".to_string(),
        }
    }

    /// Parse ConfigPath from string key (inverse of to_string_key)
    ///
    /// Returns None if the string doesn't match a valid path format
    pub fn from_string_key(s: &str) -> Option<Self> {
        // Simple paths (no dots)
        match s {
            // View
            "Zoom" => return Some(ConfigPath::Zoom),
            "Pan" => return Some(ConfigPath::Pan),
            "PanX" => return Some(ConfigPath::PanX),
            "PanY" => return Some(ConfigPath::PanY),
            "Rotation" => return Some(ConfigPath::Rotation),
            "CameraRotationX" => return Some(ConfigPath::CameraRotationX),
            "CameraRotationY" => return Some(ConfigPath::CameraRotationY),
            "CameraBank" => return Some(ConfigPath::CameraBank),
            "CameraX" => return Some(ConfigPath::CameraX),
            "CameraY" => return Some(ConfigPath::CameraY),
            "CameraZ" => return Some(ConfigPath::CameraZ),
            "DofFocusDistance" => return Some(ConfigPath::DofFocusDistance),
            "DofBlurStrength" => return Some(ConfigPath::DofBlurStrength),
            "FogStrength" => return Some(ConfigPath::FogStrength),
            "FogStart" => return Some(ConfigPath::FogStart),
            "FilterRadius" => return Some(ConfigPath::FilterRadius),
            "FilterBlurEdges" => return Some(ConfigPath::FilterBlurEdges),

            // Tone mapping
            "Exposure" => return Some(ConfigPath::Exposure),
            "Gamma" => return Some(ConfigPath::Gamma),
            "GammaThreshold" => return Some(ConfigPath::GammaThreshold),
            "Brightness" => return Some(ConfigPath::Brightness),
            "Vibrancy" => return Some(ConfigPath::Vibrancy),
            "WhiteLevel" => return Some(ConfigPath::WhiteLevel),
            "Saturation" => return Some(ConfigPath::Saturation),
            "HueShift" => return Some(ConfigPath::HueShift),
            "AlphaBlendLow" => return Some(ConfigPath::AlphaBlendLow),
            "AlphaBlendHigh" => return Some(ConfigPath::AlphaBlendHigh),
            "DensityScale" => return Some(ConfigPath::DensityScale),
            "TonemapMode" => return Some(ConfigPath::TonemapMode),
            "HighlightMode" => return Some(ConfigPath::HighlightMode),
            "TonemapCurve" => return Some(ConfigPath::TonemapCurve),
            "UseCurve" => return Some(ConfigPath::UseCurve),
            "LevelsEnabled" => return Some(ConfigPath::LevelsEnabled),
            "LevelsLow" => return Some(ConfigPath::LevelsLow),
            "LevelsHigh" => return Some(ConfigPath::LevelsHigh),
            "LevelsGamma" => return Some(ConfigPath::LevelsGamma),

            // Color
            "ColorMode" => return Some(ConfigPath::ColorMode),
            "PathMapStyle" => return Some(ConfigPath::PathMapStyle),
            "PathCaptureMode" => return Some(ConfigPath::PathCaptureMode),
            "PathTrackingMode" => return Some(ConfigPath::PathTrackingMode),
            "PaletteIndex" => return Some(ConfigPath::PaletteIndex),
            "Palette" => return Some(ConfigPath::Palette),
            "PaletteRotation" => return Some(ConfigPath::PaletteRotation),
            "PaletteSize" => return Some(ConfigPath::PaletteSize),
            "PaletteSqueeze" => return Some(ConfigPath::PaletteSqueeze),
            "PaletteSqueezeMode" => return Some(ConfigPath::PaletteSqueezeMode),
            "PaletteSqueezeFalloff" => return Some(ConfigPath::PaletteSqueezeFalloff),
            "PaletteLogStrength" => return Some(ConfigPath::PaletteLogStrength),
            "PaletteReverse" => return Some(ConfigPath::PaletteReverse),
            "SpeedFactor" => return Some(ConfigPath::SpeedFactor),
            "BackgroundColor" => return Some(ConfigPath::BackgroundColor),
            "BackgroundColorR" => return Some(ConfigPath::BackgroundColorR),
            "BackgroundColorG" => return Some(ConfigPath::BackgroundColorG),
            "BackgroundColorB" => return Some(ConfigPath::BackgroundColorB),

            // Rendering
            "BlendFactor" => return Some(ConfigPath::BlendFactor),
            "UseDynamicBlend" => return Some(ConfigPath::UseDynamicBlend),
            "MaxIterations" => return Some(ConfigPath::MaxIterations),
            "DeterministicRng" => return Some(ConfigPath::DeterministicRng),

            // Flame
            "TransformCount" => return Some(ConfigPath::TransformCount),
            "RenderMode" => return Some(ConfigPath::RenderMode),
            "PerspectiveStrength" => return Some(ConfigPath::PerspectiveStrength),
            "DepthDensityCompensation" => return Some(ConfigPath::DepthDensityCompensation),
            "FarDensityFade" => return Some(ConfigPath::FarDensityFade),
            "SolidStrength" => return Some(ConfigPath::SolidStrength),
            "SurfaceThickness" => return Some(ConfigPath::SurfaceThickness),
            "SolidShadowStrength" => return Some(ConfigPath::SolidShadowStrength),
            "ShadingStrength" => return Some(ConfigPath::ShadingStrength),
            "SolidAmbient" => return Some(ConfigPath::SolidAmbient),
            "SolidDiffuse" => return Some(ConfigPath::SolidDiffuse),
            "SolidSpecular" => return Some(ConfigPath::SolidSpecular),
            "SolidShininess" => return Some(ConfigPath::SolidShininess),
            "SsaoStrength" => return Some(ConfigPath::SsaoStrength),
            "SsaoRadius" => return Some(ConfigPath::SsaoRadius),
            "NormalSmoothing" => return Some(ConfigPath::NormalSmoothing),
            "GapFill" => return Some(ConfigPath::GapFill),
            "FarDensityFadeStart" => return Some(ConfigPath::FarDensityFadeStart),
            "SoloTransform" => return Some(ConfigPath::SoloTransform),
            "PostSymmetryType" => return Some(ConfigPath::PostSymmetryType),
            "PostSymmetryOrder" => return Some(ConfigPath::PostSymmetryOrder),
            "PostSymmetryCenterX" => return Some(ConfigPath::PostSymmetryCenterX),
            "PostSymmetryCenterY" => return Some(ConfigPath::PostSymmetryCenterY),
            "PostSymmetryDistance" => return Some(ConfigPath::PostSymmetryDistance),
            "PostSymmetryRotation" => return Some(ConfigPath::PostSymmetryRotation),
            "PreserveZ" => return Some(ConfigPath::PreserveZ),

            _ => {}
        }

        // Parse compound paths with dots

        // Escape-time paths: Escape.{field} and Escape.{kind}Param.{param}
        if let Some(rest) = s.strip_prefix("Escape.") {
            let parts: Vec<&str> = rest.split('.').collect();
            match parts.as_slice() {
                ["Formula"] => return Some(ConfigPath::EscapeFormula),
                ["Julia"] => return Some(ConfigPath::EscapeJulia),
                ["JuliaRe"] => return Some(ConfigPath::EscapeJuliaRe),
                ["JuliaIm"] => return Some(ConfigPath::EscapeJuliaIm),
                ["CenterRe"] => return Some(ConfigPath::EscapeCenterRe),
                ["CenterIm"] => return Some(ConfigPath::EscapeCenterIm),
                ["ZoomLog2"] => return Some(ConfigPath::EscapeZoomLog2),
                ["Rotation"] => return Some(ConfigPath::EscapeRotation),
                ["MaxIter"] => return Some(ConfigPath::EscapeMaxIter),
                ["Supersample"] => return Some(ConfigPath::EscapeSupersample),
                ["Downsample"] => return Some(ConfigPath::EscapeDownsample),
                ["ReferencePeriod"] => return Some(ConfigPath::EscapeReferencePeriod),
                ["Bailout"] => return Some(ConfigPath::EscapeBailout),
                ["DampingRe"] => return Some(ConfigPath::EscapeDampingRe),
                ["DampingIm"] => return Some(ConfigPath::EscapeDampingIm),
                ["Biomorph"] => return Some(ConfigPath::EscapeBiomorph),
                ["Shading", "Enabled"] => return Some(ConfigPath::EscapeShadingEnabled),
                ["Shading", "LightAngle"] => return Some(ConfigPath::EscapeShadingLightAngle),
                ["Shading", "Height"] => return Some(ConfigPath::EscapeShadingHeight),
                ["Contrast", "Mode"] => return Some(ConfigPath::EscapeContrastMode),
                ["Contrast", "Clip"] => return Some(ConfigPath::EscapeContrastClip),
                ["Contrast", "Strength"] => return Some(ConfigPath::EscapeContrastStrength),
                ["Contrast", "Turns"] => return Some(ConfigPath::EscapeContrastTurns),
                ["Shading", "Field"] => return Some(ConfigPath::EscapeShadingField),
                ["Shading", "ShadowColor"] => return Some(ConfigPath::EscapeShadingShadowColor),
                ["Shading", "ShadowStrength"] => return Some(ConfigPath::EscapeShadingShadowStrength),
                ["Shading", "ShadowBlend"] => return Some(ConfigPath::EscapeShadingShadowBlend),
                ["Shading", "HighlightColor"] => return Some(ConfigPath::EscapeShadingHighlightColor),
                ["Shading", "HighlightStrength"] => return Some(ConfigPath::EscapeShadingHighlightStrength),
                ["Shading", "HighlightBlend"] => return Some(ConfigPath::EscapeShadingHighlightBlend),
                ["Shading", "Softness"] => return Some(ConfigPath::EscapeShadingSoftness),
                ["Shading", "TextureKind"] => return Some(ConfigPath::EscapeShadingTextureKind),
                ["Shading", "TextureStrength"] => return Some(ConfigPath::EscapeShadingTextureStrength),
                ["Shading", "TextureScale"] => return Some(ConfigPath::EscapeShadingTextureScale),
                ["Coloring"] => return Some(ConfigPath::EscapeColoring),
                ["FormulaParam", param] => {
                    return Some(ConfigPath::EscapeFormulaParam { param: param.to_string() })
                }
                ["ColoringParam", param] => {
                    return Some(ConfigPath::EscapeColoringParam { param: param.to_string() })
                }
                _ => return None,
            }
        }

        // Simulation paths: Sim.{field} and Sim.{kind}Param.{param}
        if let Some(rest) = s.strip_prefix("Sim.") {
            let parts: Vec<&str> = rest.split('.').collect();
            match parts.as_slice() {
                ["Model"] => return Some(ConfigPath::SimModel),
                ["Coloring"] => return Some(ConfigPath::SimColoring),
                ["GridMode"] => return Some(ConfigPath::SimGridMode),
                ["GridWidth"] => return Some(ConfigPath::SimGridWidth),
                ["GridHeight"] => return Some(ConfigPath::SimGridHeight),
                ["GridScale"] => return Some(ConfigPath::SimGridScale),
                ["Seed"] => return Some(ConfigPath::SimSeed),
                ["InitKind"] => return Some(ConfigPath::SimInitKind),
                ["InitAmplitude"] => return Some(ConfigPath::SimInitAmplitude),
                ["InitRadius"] => return Some(ConfigPath::SimInitRadius),
                ["InitCount"] => return Some(ConfigPath::SimInitCount),
                ["Steps"] => return Some(ConfigPath::SimSteps),
                ["StepsPerFrame"] => return Some(ConfigPath::SimStepsPerFrame),
                ["Dt"] => return Some(ConfigPath::SimDt),
                ["Boundary"] => return Some(ConfigPath::SimBoundary),
                ["WarpZoom"] => return Some(ConfigPath::SimWarpZoom),
                ["WarpRotation"] => return Some(ConfigPath::SimWarpRotation),
                ["WarpPanX"] => return Some(ConfigPath::SimWarpPanX),
                ["WarpPanY"] => return Some(ConfigPath::SimWarpPanY),
                ["WarpFlow"] => return Some(ConfigPath::SimWarpFlow),
                ["WarpFilter"] => return Some(ConfigPath::SimWarpFilter),
                ["MatteChannel"] => return Some(ConfigPath::SimMatteChannel),
                ["MatteCutoff"] => return Some(ConfigPath::SimMatteCutoff),
                ["MatteSoftness"] => return Some(ConfigPath::SimMatteSoftness),
                ["MatteInvert"] => return Some(ConfigPath::SimMatteInvert),
                ["MatteEdge"] => return Some(ConfigPath::SimMatteEdge),
                ["Upscale"] => return Some(ConfigPath::SimUpscale),
                ["Downscale"] => return Some(ConfigPath::SimDownscale),
                ["ModelParam", param] => {
                    return Some(ConfigPath::SimModelParam { param: param.to_string() })
                }
                ["ColoringParam", param] => {
                    return Some(ConfigPath::SimColoringParam { param: param.to_string() })
                }
                _ => return None,
            }
        }

        let parts: Vec<&str> = s.split('.').collect();

        // Transform paths: Transform.{index}.{field}...
        if parts.len() >= 3 && parts[0] == "Transform" {
            let index: usize = parts[1].parse().ok()?;

            match parts[2] {
                "Weight" => return Some(ConfigPath::TransformWeight { index }),
                "Color" => return Some(ConfigPath::TransformColor { index }),
                "ColorSpeed" => return Some(ConfigPath::TransformColorSpeed { index }),
                "Opacity" => return Some(ConfigPath::TransformOpacity { index }),
                "DirectColor" => return Some(ConfigPath::TransformDirectColor { index }),
                "Affine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::TransformAffine { index, param });
                }
                "Variation" if parts.len() == 4 => {
                    return Some(ConfigPath::TransformVariation {
                        index,
                        variation: parts[3].to_string(),
                    });
                }
                "VariationParam" if parts.len() == 5 => {
                    return Some(ConfigPath::TransformVariationParam {
                        index,
                        variation: parts[3].to_string(),
                        param: parts[4].to_string(),
                    });
                }
                "PostAffineEnabled" => return Some(ConfigPath::TransformPostAffineEnabled { index }),
                "PostAffine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::TransformPostAffine { index, param });
                }
                "OriginX" => return Some(ConfigPath::TransformOriginX { index }),
                "OriginY" => return Some(ConfigPath::TransformOriginY { index }),
                "Rotation" => return Some(ConfigPath::TransformRotation { index }),
                "Scale" => return Some(ConfigPath::TransformScale { index }),
                "PostAffineOriginX" => return Some(ConfigPath::TransformPostAffineOriginX { index }),
                "PostAffineOriginY" => return Some(ConfigPath::TransformPostAffineOriginY { index }),
                "PostAffineRotation" => return Some(ConfigPath::TransformPostAffineRotation { index }),
                "PostAffineScale" => return Some(ConfigPath::TransformPostAffineScale { index }),
                _ => {}
            }
        }

        // LinkedTransform pool paths: LinkedTransform.{index}.{field}...
        // (mirrors Transform.{index}.{field}... but indexes into
        // flame.linked_transforms instead of flame.transforms.)
        if parts.len() >= 3 && parts[0] == "LinkedTransform" {
            let index: usize = parts[1].parse().ok()?;
            match parts[2] {
                "Affine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::LinkedTransformAffine { index, param });
                }
                "PostAffineEnabled" => {
                    return Some(ConfigPath::LinkedTransformPostAffineEnabled { index });
                }
                "PostAffine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::LinkedTransformPostAffine { index, param });
                }
                "Variation" if parts.len() == 4 => {
                    return Some(ConfigPath::LinkedTransformVariation {
                        index,
                        variation: parts[3].to_string(),
                    });
                }
                "VariationParam" if parts.len() == 5 => {
                    return Some(ConfigPath::LinkedTransformVariationParam {
                        index,
                        variation: parts[3].to_string(),
                        param: parts[4].to_string(),
                    });
                }
                "OriginX" => return Some(ConfigPath::LinkedTransformOriginX { index }),
                "OriginY" => return Some(ConfigPath::LinkedTransformOriginY { index }),
                "Rotation" => return Some(ConfigPath::LinkedTransformRotation { index }),
                "Scale" => return Some(ConfigPath::LinkedTransformScale { index }),
                "PostAffineOriginX" => return Some(ConfigPath::LinkedTransformPostAffineOriginX { index }),
                "PostAffineOriginY" => return Some(ConfigPath::LinkedTransformPostAffineOriginY { index }),
                "PostAffineRotation" => return Some(ConfigPath::LinkedTransformPostAffineRotation { index }),
                "PostAffineScale" => return Some(ConfigPath::LinkedTransformPostAffineScale { index }),
                _ => {}
            }
        }

        // FinalTransform pool paths.
        //
        // Three string formats are accepted; all map to the indexed
        // FinalTransform* enum variants.
        //   1. New canonical:  `FinalTransform.{index}.{field}...`
        //   2. Historical:     `PoolFinalTransform.{index}.{field}...`
        //                      (emitted by the prior branch before the
        //                      Phase 9 rename; saved animation tracks
        //                      from that period still resolve.)
        //   3. Legacy single:  `FinalTransform.{field}...` (no index)
        //                      — back when flame had a single Final
        //                      transform. Mapped to index 0 since the
        //                      legacy variants routed to
        //                      `final_transforms[0]`.
        //
        // Legacy keys with no indexed counterpart (`FinalTransform.Enabled`,
        // `FinalTransform.OriginX|OriginY|Rotation|Scale`) parse as
        // `None` — old animation tracks targeting them become no-ops on
        // load, since those UI helpers were tied to a single-final model
        // that no longer exists.
        if parts.len() >= 3 && (parts[0] == "FinalTransform" || parts[0] == "PoolFinalTransform") {
            if let Ok(index) = parts[1].parse::<usize>() {
                match parts[2] {
                    "Affine" if parts.len() == 4 => {
                        let param = AffineParam::from_char(parts[3].chars().next()?)?;
                        return Some(ConfigPath::FinalTransformAffine { index, param });
                    }
                    "PostAffineEnabled" => {
                        return Some(ConfigPath::FinalTransformPostAffineEnabled { index });
                    }
                    "PostAffine" if parts.len() == 4 => {
                        let param = AffineParam::from_char(parts[3].chars().next()?)?;
                        return Some(ConfigPath::FinalTransformPostAffine { index, param });
                    }
                    "Variation" if parts.len() == 4 => {
                        return Some(ConfigPath::FinalTransformVariation {
                            index,
                            variation: parts[3].to_string(),
                        });
                    }
                    "VariationParam" if parts.len() == 5 => {
                        return Some(ConfigPath::FinalTransformVariationParam {
                            index,
                            variation: parts[3].to_string(),
                            param: parts[4].to_string(),
                        });
                    }
                    "OriginX" => return Some(ConfigPath::FinalTransformOriginX { index }),
                    "OriginY" => return Some(ConfigPath::FinalTransformOriginY { index }),
                    "Rotation" => return Some(ConfigPath::FinalTransformRotation { index }),
                    "Scale" => return Some(ConfigPath::FinalTransformScale { index }),
                    "PostAffineOriginX" => return Some(ConfigPath::FinalTransformPostAffineOriginX { index }),
                    "PostAffineOriginY" => return Some(ConfigPath::FinalTransformPostAffineOriginY { index }),
                    "PostAffineRotation" => return Some(ConfigPath::FinalTransformPostAffineRotation { index }),
                    "PostAffineScale" => return Some(ConfigPath::FinalTransformPostAffineScale { index }),
                    _ => {}
                }
            }
        }
        // Legacy single-final migration: `FinalTransform.{field}...`
        // with parts[1] not numeric → route to index 0.
        if parts.len() >= 2 && parts[0] == "FinalTransform" {
            match parts[1] {
                "Affine" if parts.len() == 3 => {
                    let param = AffineParam::from_char(parts[2].chars().next()?)?;
                    return Some(ConfigPath::FinalTransformAffine { index: 0, param });
                }
                "Variation" if parts.len() == 3 => {
                    return Some(ConfigPath::FinalTransformVariation {
                        index: 0,
                        variation: parts[2].to_string(),
                    });
                }
                "VariationParam" if parts.len() == 4 => {
                    return Some(ConfigPath::FinalTransformVariationParam {
                        index: 0,
                        variation: parts[2].to_string(),
                        param: parts[3].to_string(),
                    });
                }
                "PostAffineEnabled" => {
                    return Some(ConfigPath::FinalTransformPostAffineEnabled { index: 0 });
                }
                "PostAffine" if parts.len() == 3 => {
                    let param = AffineParam::from_char(parts[2].chars().next()?)?;
                    return Some(ConfigPath::FinalTransformPostAffine { index: 0, param });
                }
                _ => {}
            }
        }

        // System paths
        if parts.len() == 2 && parts[0] == "System" {
            match parts[1] {
                "IterationsPerThread" => return Some(ConfigPath::SystemIterationsPerThread),
                "BurnIn" => return Some(ConfigPath::SystemBurnIn),
                "OrbitCacheMb" => return Some(ConfigPath::SystemOrbitCacheMb),
                "VsyncEnabled" => return Some(ConfigPath::SystemVsyncEnabled),
                "TargetFps" => return Some(ConfigPath::SystemTargetFps),
                "FlyMouseSensitivity" => return Some(ConfigPath::SystemFlyMouseSensitivity),
                "FlyMoveSpeed" => return Some(ConfigPath::SystemFlyMoveSpeed),
                "FlySprintMultiplier" => return Some(ConfigPath::SystemFlySprintMultiplier),
                "FlyInvertY" => return Some(ConfigPath::SystemFlyInvertY),
                "FlyCameraMode" => return Some(ConfigPath::SystemFlyCameraMode),
                "ExportWidth" => return Some(ConfigPath::SystemExportWidth),
                "ExportHeight" => return Some(ConfigPath::SystemExportHeight),
                "PngStripMetadata" => return Some(ConfigPath::SystemPngStripMetadata),
                "Language" => return Some(ConfigPath::SystemLanguage),
                "ShowHelpOnStartup" => return Some(ConfigPath::SystemShowHelpOnStartup),
                _ => {}
            }
        }

        // Effect paths: DensityEffect.{index}.{Enabled|param} or ColorEffect.{index}.{Enabled|param}
        if parts.len() == 3 && parts[0] == "SolidLight" {
            if let Ok(index) = parts[1].parse::<usize>() {
                if parts[2] == "enabled" {
                    return Some(ConfigPath::SolidLightEnabled { index });
                }
                return Some(ConfigPath::SolidLightParam { index, param: parts[2].to_string() });
            }
        }
        if parts.len() == 3 && parts[0] == "DensityEffect" {
            if let Ok(index) = parts[1].parse::<usize>() {
                if parts[2] == "Enabled" {
                    return Some(ConfigPath::DensityEffectEnabled { index });
                } else {
                    return Some(ConfigPath::DensityEffectParam { index, param: parts[2].to_string() });
                }
            }
        }

        if parts.len() == 3 && parts[0] == "ColorEffect" {
            if let Ok(index) = parts[1].parse::<usize>() {
                if parts[2] == "Enabled" {
                    return Some(ConfigPath::ColorEffectEnabled { index });
                } else {
                    return Some(ConfigPath::ColorEffectParam { index, param: parts[2].to_string() });
                }
            }
        }

        // Xaos paths: Xaos.{src}.{dst}
        if parts.len() == 3 && parts[0] == "Xaos" {
            if let (Ok(src), Ok(dst)) = (parts[1].parse::<usize>(), parts[2].parse::<usize>()) {
                return Some(ConfigPath::Xaos { src, dst });
            }
        }

        None
    }
}

impl AffineParam {
    /// Convert to single character representation
    pub fn to_char(&self) -> char {
        match self {
            AffineParam::A => 'A',
            AffineParam::B => 'B',
            AffineParam::C => 'C',
            AffineParam::D => 'D',
            AffineParam::E => 'E',
            AffineParam::F => 'F',
            AffineParam::G => 'G',
        }
    }

    /// Parse from single character
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'A' | 'a' => Some(AffineParam::A),
            'B' | 'b' => Some(AffineParam::B),
            'C' | 'c' => Some(AffineParam::C),
            'D' | 'd' => Some(AffineParam::D),
            'E' | 'e' => Some(AffineParam::E),
            'F' | 'f' => Some(AffineParam::F),
            'G' | 'g' => Some(AffineParam::G),
            _ => None,
        }
    }
}

/// Convert JSON value to ConfigValue based on the expected type for a ConfigPath
///
/// This is used by the animation system to convert interpolated JSON values
/// back to strongly-typed ConfigValues for the ConfigManager.
///
/// # Arguments
/// * `json` - The JSON value to convert
/// * `path` - The ConfigPath that determines the expected type
///
/// # Returns
/// * `Some(ConfigValue)` on successful conversion
/// * `None` if the JSON value cannot be converted to the expected type
pub fn json_to_config_value(json: &serde_json::Value, path: &ConfigPath) -> Option<ConfigValue> {
    use serde_json::Value;

    match path {
        // Float parameters
        ConfigPath::Zoom
        | ConfigPath::PanX
        | ConfigPath::PanY
        | ConfigPath::Rotation
        | ConfigPath::CameraRotationX
        | ConfigPath::CameraRotationY
        | ConfigPath::CameraBank
        | ConfigPath::CameraX
        | ConfigPath::CameraY
        | ConfigPath::CameraZ
        | ConfigPath::DofFocusDistance
        | ConfigPath::DofBlurStrength
        | ConfigPath::FogStrength
        | ConfigPath::FogStart
        | ConfigPath::FilterRadius
        | ConfigPath::FilterBlurEdges
        | ConfigPath::Exposure
        | ConfigPath::Gamma
        | ConfigPath::GammaThreshold
        | ConfigPath::Brightness
        | ConfigPath::Vibrancy
        | ConfigPath::WhiteLevel
        | ConfigPath::Saturation
        | ConfigPath::HueShift
        | ConfigPath::AlphaBlendLow
        | ConfigPath::AlphaBlendHigh
        | ConfigPath::DensityScale
        | ConfigPath::PaletteRotation
        | ConfigPath::PaletteSize
        | ConfigPath::PaletteSqueeze
        | ConfigPath::PaletteSqueezeFalloff
        | ConfigPath::PaletteLogStrength
        | ConfigPath::SpeedFactor
        | ConfigPath::BackgroundColorR
        | ConfigPath::BackgroundColorG
        | ConfigPath::BackgroundColorB
        | ConfigPath::BlendFactor
        | ConfigPath::PerspectiveStrength
        | ConfigPath::DepthDensityCompensation
        | ConfigPath::SolidStrength
        | ConfigPath::SurfaceThickness
        | ConfigPath::ShadingStrength
        | ConfigPath::SolidAmbient
        | ConfigPath::SolidDiffuse
        | ConfigPath::SolidSpecular
        | ConfigPath::SolidShininess
        | ConfigPath::SsaoStrength
        | ConfigPath::SsaoRadius
        | ConfigPath::NormalSmoothing
        | ConfigPath::GapFill
        | ConfigPath::SolidShadowStrength
        | ConfigPath::SolidLightParam { .. }
        | ConfigPath::FarDensityFade
        | ConfigPath::FarDensityFadeStart
        | ConfigPath::TransformWeight { .. }
        | ConfigPath::TransformColor { .. }
        | ConfigPath::TransformColorSpeed { .. }
        | ConfigPath::TransformOpacity { .. }
        | ConfigPath::TransformDirectColor { .. }
        | ConfigPath::TransformAffine { .. }
        | ConfigPath::TransformPostAffine { .. }
        | ConfigPath::TransformYzCoefs { .. }
        | ConfigPath::TransformZxCoefs { .. }
        | ConfigPath::TransformYzPostCoefs { .. }
        | ConfigPath::TransformZxPostCoefs { .. }
        | ConfigPath::TransformVariation { .. }
        | ConfigPath::TransformVariationParam { .. }
        | ConfigPath::TransformOriginX { .. }
        | ConfigPath::TransformOriginY { .. }
        | ConfigPath::TransformRotation { .. }
        | ConfigPath::TransformScale { .. }
        | ConfigPath::TransformPostAffineOriginX { .. }
        | ConfigPath::TransformPostAffineOriginY { .. }
        | ConfigPath::TransformPostAffineRotation { .. }
        | ConfigPath::TransformPostAffineScale { .. }
        | ConfigPath::LinkedTransformAffine { .. }
        | ConfigPath::LinkedTransformPostAffine { .. }
        | ConfigPath::LinkedTransformYzCoefs { .. }
        | ConfigPath::LinkedTransformZxCoefs { .. }
        | ConfigPath::LinkedTransformYzPostCoefs { .. }
        | ConfigPath::LinkedTransformZxPostCoefs { .. }
        | ConfigPath::LinkedTransformVariation { .. }
        | ConfigPath::LinkedTransformVariationParam { .. }
        | ConfigPath::LinkedTransformOriginX { .. }
        | ConfigPath::LinkedTransformOriginY { .. }
        | ConfigPath::LinkedTransformRotation { .. }
        | ConfigPath::LinkedTransformScale { .. }
        | ConfigPath::LinkedTransformPostAffineOriginX { .. }
        | ConfigPath::LinkedTransformPostAffineOriginY { .. }
        | ConfigPath::LinkedTransformPostAffineRotation { .. }
        | ConfigPath::LinkedTransformPostAffineScale { .. }
        | ConfigPath::FinalTransformAffine { .. }
        | ConfigPath::FinalTransformPostAffine { .. }
        | ConfigPath::FinalTransformYzCoefs { .. }
        | ConfigPath::FinalTransformZxCoefs { .. }
        | ConfigPath::FinalTransformYzPostCoefs { .. }
        | ConfigPath::FinalTransformZxPostCoefs { .. }
        | ConfigPath::FinalTransformVariation { .. }
        | ConfigPath::FinalTransformVariationParam { .. }
        | ConfigPath::FinalTransformOriginX { .. }
        | ConfigPath::FinalTransformOriginY { .. }
        | ConfigPath::FinalTransformRotation { .. }
        | ConfigPath::FinalTransformScale { .. }
        | ConfigPath::FinalTransformPostAffineOriginX { .. }
        | ConfigPath::FinalTransformPostAffineOriginY { .. }
        | ConfigPath::FinalTransformPostAffineRotation { .. }
        | ConfigPath::FinalTransformPostAffineScale { .. }
        | ConfigPath::Xaos { .. }
        | ConfigPath::SystemTargetFps
        | ConfigPath::SystemFlyMouseSensitivity
        | ConfigPath::SystemFlyMoveSpeed
        | ConfigPath::SystemFlySprintMultiplier
        | ConfigPath::SystemFlyInvertY
        | ConfigPath::LevelsLow
        | ConfigPath::LevelsHigh
        | ConfigPath::LevelsGamma
        | ConfigPath::PostSymmetryCenterX
        | ConfigPath::PostSymmetryCenterY
        | ConfigPath::PostSymmetryDistance
        | ConfigPath::PostSymmetryRotation => {
            json.as_f64().map(|f| ConfigValue::Float(f as f32))
        }

        // Simulation paths with no meaningful interpolation: a model
        // name, a boundary rule or a grid mode has no value "between"
        // two keyframes, so these get no track rather than a track that
        // snaps. Listed explicitly instead of falling into a wildcard,
        // so a future animatable field has to be classified here.
        ConfigPath::SimModel
        | ConfigPath::SimColoring
        | ConfigPath::SimGridMode
        | ConfigPath::SimSeed
        | ConfigPath::SimInitKind
        | ConfigPath::SimBoundary
        | ConfigPath::SimWarpFilter
        | ConfigPath::SimMatteChannel
        | ConfigPath::SimMatteInvert
        | ConfigPath::SimMatteEdge
        | ConfigPath::SimUpscale
        | ConfigPath::SimDownscale => None,

        // Vec2 (pan coordinates)
        ConfigPath::Pan => {
            if let Value::Array(arr) = json {
                if arr.len() == 2 {
                    let x = arr[0].as_f64()? as f32;
                    let y = arr[1].as_f64()? as f32;
                    return Some(ConfigValue::Vec2(x, y));
                }
            }
            None
        }

        // RGB color
        ConfigPath::BackgroundColor => {
            if let Value::Array(arr) = json {
                if arr.len() == 3 {
                    let r = arr[0].as_f64()? as f32;
                    let g = arr[1].as_f64()? as f32;
                    let b = arr[2].as_f64()? as f32;
                    return Some(ConfigValue::ColorRgb([r, g, b]));
                }
            }
            None
        }

        // Boolean parameters
        ConfigPath::UseCurve
        | ConfigPath::LevelsEnabled
        | ConfigPath::UseDynamicBlend
        | ConfigPath::DeterministicRng
        | ConfigPath::PaletteReverse
        | ConfigPath::TransformPostAffineEnabled { .. }
        | ConfigPath::LinkedTransformPostAffineEnabled { .. }
        | ConfigPath::FinalTransformPostAffineEnabled { .. }
        | ConfigPath::SystemVsyncEnabled
        | ConfigPath::SystemShowHelpOnStartup
        | ConfigPath::PreserveZ
        => {
            json.as_bool().map(ConfigValue::Bool)
        }

        // UInt parameters
        ConfigPath::PaletteIndex
        | ConfigPath::TransformCount
        | ConfigPath::SystemIterationsPerThread
        | ConfigPath::SystemBurnIn
        | ConfigPath::SystemOrbitCacheMb
        | ConfigPath::SystemExportWidth
        | ConfigPath::SystemExportHeight
        | ConfigPath::SystemPngStripMetadata => {
            json.as_u64().map(|u| ConfigValue::UInt(u as u32))
        }

        // UInt64 parameters
        ConfigPath::MaxIterations => {
            json.as_u64().map(ConfigValue::UInt64)
        }

        // Optional usize as Int (-1 = None, 0+ = Some(index))
        ConfigPath::SoloTransform
        | ConfigPath::PostSymmetryType
        | ConfigPath::PostSymmetryOrder => {
            json.as_i64().map(|i| ConfigValue::Int(i as i32))
        }

        // String parameters
        ConfigPath::SystemLanguage | ConfigPath::SystemFlyCameraMode => {
            json.as_str().map(|s| ConfigValue::String(s.to_string()))
        }

        // Enum types: deserialize via serde, which accepts both the canonical
        // snake_case wire form (`"2d"`, `"path_map"`, ...) and the legacy
        // PascalCase `alias`es from old config / animation files.
        ConfigPath::TonemapMode => {
            serde_json::from_value::<ToneMapMode>(json.clone())
                .ok()
                .map(ConfigValue::ToneMapMode)
        }

        ConfigPath::HighlightMode => {
            serde_json::from_value::<crate::scene::tonemap::HighlightMode>(json.clone())
                .ok()
                .map(ConfigValue::HighlightMode)
        }

        ConfigPath::ColorMode => {
            serde_json::from_value::<ColorMode>(json.clone())
                .ok()
                .map(ConfigValue::ColorMode)
        }

        ConfigPath::PaletteSqueezeMode => {
            serde_json::from_value::<crate::scene::palette::SqueezeMode>(json.clone())
                .ok()
                .map(ConfigValue::SqueezeMode)
        }

        ConfigPath::PathMapStyle => {
            serde_json::from_value::<PathMapStyle>(json.clone())
                .ok()
                .or_else(|| {
                    // Pre-serde legacy style names from very old config files.
                    json.as_str().and_then(|s| match s {
                        "Similar" => Some(PathMapStyle::Prefix),
                        "Distinct" | "ScrambledPrefix" => Some(PathMapStyle::PrefixDistinct),
                        "ScrambledSuffix" => Some(PathMapStyle::SuffixDistinct),
                        _ => None,
                    })
                })
                .map(ConfigValue::PathMapStyle)
        }

        ConfigPath::PathCaptureMode => {
            serde_json::from_value::<PathCaptureMode>(json.clone())
                .ok()
                .map(ConfigValue::PathCaptureMode)
        }

        ConfigPath::PathTrackingMode => {
            serde_json::from_value::<PathTrackingMode>(json.clone())
                .ok()
                .map(ConfigValue::PathTrackingMode)
        }

        ConfigPath::RenderMode => {
            serde_json::from_value::<RenderMode>(json.clone())
                .ok()
                .map(ConfigValue::RenderMode)
        }

        // Effect enabled flags (bool)
        ConfigPath::DensityEffectEnabled { .. }
        | ConfigPath::ColorEffectEnabled { .. }
        | ConfigPath::SolidLightEnabled { .. } => {
            json.as_bool().map(ConfigValue::Bool)
        }

        // Effect parameters (float)
        ConfigPath::DensityEffectParam { .. }
        | ConfigPath::ColorEffectParam { .. } => {
            json.as_f64().map(|f| ConfigValue::Float(f as f32))
        }

        // Escape-time continuous parameters
        ConfigPath::EscapeJuliaRe
        | ConfigPath::EscapeJuliaIm
        | ConfigPath::EscapeZoomLog2
        | ConfigPath::EscapeRotation
        | ConfigPath::EscapeBailout
        | ConfigPath::EscapeDampingRe
        | ConfigPath::EscapeDampingIm
        | ConfigPath::EscapeFormulaParam { .. }
        | ConfigPath::EscapeColoringParam { .. } => {
            json.as_f64().map(|f| ConfigValue::Float(f as f32))
        }
        ConfigPath::EscapeMaxIter => json.as_u64().map(|v| ConfigValue::UInt(v as u32)),

        // Simulation. Only the quantities that mean something when
        // interpolated between two keyframes are here; the rest fall
        // through to None and cannot be given a track.
        //
        // Sim.Steps is the important one: it IS the animation of the
        // simulation's progression (master plan D5b).
        ConfigPath::SimSteps
        | ConfigPath::SimStepsPerFrame
        | ConfigPath::SimGridWidth
        | ConfigPath::SimGridHeight
        | ConfigPath::SimInitRadius
        | ConfigPath::SimInitCount => json.as_u64().map(|v| ConfigValue::UInt(v as u32)),
        ConfigPath::SimDt
        | ConfigPath::SimGridScale
        | ConfigPath::SimInitAmplitude
        | ConfigPath::SimWarpZoom
        | ConfigPath::SimWarpRotation
        | ConfigPath::SimWarpPanX
        | ConfigPath::SimWarpPanY
        | ConfigPath::SimWarpFlow
        | ConfigPath::SimMatteCutoff
        | ConfigPath::SimMatteSoftness
        | ConfigPath::SimModelParam { .. }
        | ConfigPath::SimColoringParam { .. } => {
            json.as_f64().map(|v| ConfigValue::Float(v as f32))
        }
        ConfigPath::EscapeSupersample => json.as_u64().map(|v| ConfigValue::UInt(v as u32)),
        ConfigPath::EscapeJulia => json.as_bool().map(ConfigValue::Bool),
        // Relief shading: the continuous controls animate (sweeping the
        // light around a still is the obvious use), the selectors and
        // the on/off do not.
        ConfigPath::EscapeShadingLightAngle
        | ConfigPath::EscapeShadingHeight
        | ConfigPath::EscapeContrastClip
        | ConfigPath::EscapeContrastStrength
        | ConfigPath::EscapeContrastTurns
        | ConfigPath::EscapeShadingShadowStrength
        | ConfigPath::EscapeShadingHighlightStrength
        | ConfigPath::EscapeShadingSoftness
        | ConfigPath::EscapeShadingTextureStrength
        | ConfigPath::EscapeShadingTextureScale => {
            json.as_f64().map(|v| ConfigValue::Float(v as f32))
        }
        ConfigPath::EscapeShadingShadowColor | ConfigPath::EscapeShadingHighlightColor => {
            let a = json.as_array()?;
            if a.len() != 3 {
                return None;
            }
            let mut rgb = [0.0f32; 3];
            for (slot, v) in rgb.iter_mut().zip(a) {
                *slot = v.as_f64()? as f32;
            }
            Some(ConfigValue::ColorRgb(rgb))
        }
        ConfigPath::EscapeShadingEnabled
        | ConfigPath::EscapeContrastMode
        | ConfigPath::EscapeShadingField
        | ConfigPath::EscapeShadingTextureKind
        | ConfigPath::EscapeDownsample
        | ConfigPath::EscapeShadingShadowBlend
        | ConfigPath::EscapeShadingHighlightBlend => None,
        // Selectors and the deep-zoom center strings are structural /
        // exact — not animatable (centers deliberately: see the plan's
        // open questions on center-path animation).
        ConfigPath::EscapeFormula
        | ConfigPath::EscapeColoring
        | ConfigPath::EscapeReferencePeriod
        | ConfigPath::EscapeBiomorph
        | ConfigPath::EscapeCenterRe
        | ConfigPath::EscapeCenterIm => None,

        // Complex types not supported for animation (yet)
        ConfigPath::TonemapCurve | ConfigPath::Palette => None,

        // Add/Remove operations not animatable
        ConfigPath::AddColorEffect { .. }
        | ConfigPath::RemoveColorEffect { .. }
        | ConfigPath::AddDensityEffect { .. }
        | ConfigPath::RemoveDensityEffect { .. } => None,

        // fx_priority phase overrides are intentionally NOT animatable —
        // moving a variation between phases is a structural choice, not a
        // continuous parameter. (They still round-trip through undo/redo
        // and .flame import/export.)
        ConfigPath::TransformVariationPriority { .. }
        | ConfigPath::LinkedTransformVariationPriority { .. }
        | ConfigPath::FinalTransformVariationPriority { .. }
        // Variation order is a structural reorder, not a continuous param.
        | ConfigPath::TransformVariationOrder { .. }
        | ConfigPath::LinkedTransformVariationOrder { .. }
        | ConfigPath::FinalTransformVariationOrder { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_creation_and_inversion() {
        let delta = ConfigDelta::new(
            ConfigPath::Exposure,
            1.0.into(),
            0.5.into(),
        );

        // Check path (can't use == without PartialEq, but we can check via Display)
        assert_eq!(format!("{}", delta.path), "Exposure");
        assert!(delta.old_value.approx_eq(&ConfigValue::Float(1.0)));
        assert!(delta.new_value.approx_eq(&ConfigValue::Float(0.5)));

        let inverted = delta.invert();
        assert!(inverted.old_value.approx_eq(&ConfigValue::Float(0.5)));
        assert!(inverted.new_value.approx_eq(&ConfigValue::Float(1.0)));
    }

    #[test]
    fn test_config_value_approx_eq() {
        let v1 = ConfigValue::Float(1.0);
        let v2 = ConfigValue::Float(1.0 + 1e-7);
        let v3 = ConfigValue::Float(1.0 + 1e-5);

        assert!(v1.approx_eq(&v2)); // Within epsilon
        assert!(!v1.approx_eq(&v3)); // Outside epsilon
    }

    #[test]
    fn test_update_type_merge() {
        assert_eq!(
            UpdateType::ViewOnly.merge(UpdateType::ToneMappingOnly),
            UpdateType::ToneMappingOnly
        );
        assert_eq!(
            UpdateType::ToneMappingOnly.merge(UpdateType::IterationReset),
            UpdateType::IterationReset
        );
        assert_eq!(
            UpdateType::None.merge(UpdateType::ViewOnly),
            UpdateType::ViewOnly
        );
    }

    #[test]
    fn test_config_path_update_type() {
        assert!(matches!(ConfigPath::Exposure.update_type(), UpdateType::ToneMappingOnly));
        assert!(matches!(ConfigPath::Zoom.update_type(), UpdateType::ViewOnly));
        assert!(matches!(
            ConfigPath::TransformVariation {
                index: 0,
                variation: "linear".to_string()
            }
            .update_type(),
            UpdateType::IterationReset
        ));
    }

    #[test]
    fn test_config_change_batch() {
        let deltas = vec![
            ConfigDelta::new(ConfigPath::Zoom, 1.0.into(), 2.0.into()),
            ConfigDelta::new(ConfigPath::Pan, (0.0, 0.0).into(), (1.0, 0.0).into()),
        ];

        let change = ConfigChange::batch(deltas, "Reset View".to_string());

        assert_eq!(change.deltas.len(), 2);
        assert_eq!(change.description, "Reset View");
        assert_eq!(change.update_type(), UpdateType::ViewOnly);
    }

    #[test]
    fn test_config_path_string_roundtrip_simple() {
        // Test simple paths
        let paths = vec![
            ConfigPath::Zoom,
            ConfigPath::Pan,
            ConfigPath::Rotation,
            ConfigPath::Exposure,
            ConfigPath::Gamma,
            ConfigPath::Brightness,
            ConfigPath::ColorMode,
            ConfigPath::RenderMode,
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for {:?}", path);
        }
    }

    #[test]
    fn test_config_path_string_roundtrip_transform() {
        // Test transform paths
        let paths = vec![
            ConfigPath::TransformWeight { index: 0 },
            ConfigPath::TransformWeight { index: 5 },
            ConfigPath::TransformColor { index: 2 },
            ConfigPath::TransformAffine { index: 1, param: AffineParam::A },
            ConfigPath::TransformAffine { index: 3, param: AffineParam::G },
            ConfigPath::TransformVariation { index: 0, variation: "linear".to_string() },
            ConfigPath::TransformVariation { index: 2, variation: "sinusoidal".to_string() },
            ConfigPath::TransformVariationParam {
                index: 0,
                variation: "julian".to_string(),
                param: "power".to_string(),
            },
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for key: {}", key);
        }
    }

    #[test]
    fn test_config_path_string_roundtrip_transform_operations() {
        // Test high-level transform operation paths (origin, rotation, scale)
        let paths = vec![
            ConfigPath::TransformOriginX { index: 0 },
            ConfigPath::TransformOriginX { index: 3 },
            ConfigPath::TransformOriginY { index: 0 },
            ConfigPath::TransformOriginY { index: 5 },
            ConfigPath::TransformRotation { index: 1 },
            ConfigPath::TransformRotation { index: 2 },
            ConfigPath::TransformScale { index: 0 },
            ConfigPath::TransformScale { index: 4 },
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for key: {}", key);
        }
    }

    /// Phase 9 migration shim: legacy `FinalTransform.{field}...` keys
    /// (no index — emitted before the per-pool model) and the prior
    /// branch's `PoolFinalTransform.{index}.{field}...` keys must both
    /// parse into the new indexed `FinalTransform*` variants. Old
    /// animation tracks targeting either format keep working without
    /// a manual migration step.
    #[test]
    fn legacy_final_transform_keys_migrate_to_indexed() {
        // Legacy single-final keys → index 0
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.Affine.e"),
            Some(ConfigPath::FinalTransformAffine { index: 0, param: AffineParam::E })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.Variation.spherical"),
            Some(ConfigPath::FinalTransformVariation {
                index: 0, variation: "spherical".to_string()
            })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.VariationParam.julian.power"),
            Some(ConfigPath::FinalTransformVariationParam {
                index: 0, variation: "julian".to_string(), param: "power".to_string(),
            })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.PostAffineEnabled"),
            Some(ConfigPath::FinalTransformPostAffineEnabled { index: 0 })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.PostAffine.a"),
            Some(ConfigPath::FinalTransformPostAffine { index: 0, param: AffineParam::A })
        );

        // Legacy keys with no indexed counterpart parse as None — old
        // tracks targeting the singular UI helpers become no-ops.
        assert_eq!(ConfigPath::from_string_key("FinalTransform.Enabled"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.OriginX"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.OriginY"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.Rotation"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.Scale"), None);

        // Prior-branch PoolFinalTransform keys → same indexed variants.
        assert_eq!(
            ConfigPath::from_string_key("PoolFinalTransform.2.Affine.b"),
            Some(ConfigPath::FinalTransformAffine { index: 2, param: AffineParam::B })
        );
        assert_eq!(
            ConfigPath::from_string_key("PoolFinalTransform.0.PostAffineEnabled"),
            Some(ConfigPath::FinalTransformPostAffineEnabled { index: 0 })
        );
    }

    /// Comprehensive roundtrip enforcement: every ConfigPath variant
    /// that's reachable as an animation target MUST roundtrip cleanly
    /// through to_string_key → from_string_key. Adding a new variant
    /// without updating both directions is a silent break of the
    /// animation contract — animation tracks targeting that variant
    /// will fail to parse on load and silently no-op.
    ///
    /// Action-only variants (AddColorEffect, RemoveColorEffect,
    /// AddDensityEffect, RemoveDensityEffect) are intentionally
    /// excluded — they're one-shot UI actions, never animation
    /// targets.
    #[test]
    fn test_config_path_string_roundtrip_complete() {
        let paths = vec![
            // View
            ConfigPath::Zoom,
            ConfigPath::Pan,
            ConfigPath::PanX,
            ConfigPath::PanY,
            ConfigPath::Rotation,
            ConfigPath::CameraRotationX,
            ConfigPath::CameraRotationY,
            ConfigPath::CameraBank,
            ConfigPath::CameraX,
            ConfigPath::CameraY,
            ConfigPath::CameraZ,
            ConfigPath::DofFocusDistance,
            ConfigPath::DofBlurStrength,
            ConfigPath::FogStrength,
            ConfigPath::FogStart,
            ConfigPath::FilterRadius,
            ConfigPath::FilterBlurEdges,

            // Tone mapping
            ConfigPath::Exposure,
            ConfigPath::Gamma,
            ConfigPath::GammaThreshold,
            ConfigPath::Brightness,
            ConfigPath::Vibrancy,
            ConfigPath::WhiteLevel,
            ConfigPath::Saturation,
            ConfigPath::HueShift,
            ConfigPath::AlphaBlendLow,
            ConfigPath::AlphaBlendHigh,
            ConfigPath::DensityScale,
            ConfigPath::TonemapMode,
            ConfigPath::HighlightMode,
            ConfigPath::TonemapCurve,
            ConfigPath::UseCurve,
            ConfigPath::LevelsEnabled,
            ConfigPath::LevelsLow,
            ConfigPath::LevelsHigh,
            ConfigPath::LevelsGamma,

            // Color
            ConfigPath::ColorMode,
            ConfigPath::PathMapStyle,
            ConfigPath::PathCaptureMode,
            ConfigPath::PathTrackingMode,
            ConfigPath::PaletteIndex,
            ConfigPath::Palette,
            ConfigPath::PaletteRotation,
            ConfigPath::PaletteSize,
            ConfigPath::PaletteSqueeze,
            ConfigPath::PaletteSqueezeMode,
            ConfigPath::PaletteSqueezeFalloff,
            ConfigPath::PaletteLogStrength,
            ConfigPath::PaletteReverse,
            ConfigPath::SpeedFactor,
            ConfigPath::BackgroundColor,
            ConfigPath::BackgroundColorR,
            ConfigPath::BackgroundColorG,
            ConfigPath::BackgroundColorB,

            // Rendering
            ConfigPath::BlendFactor,
            ConfigPath::UseDynamicBlend,
            ConfigPath::MaxIterations,
            ConfigPath::DeterministicRng,

            // Transform pool (Normal)
            ConfigPath::TransformCount,
            ConfigPath::TransformWeight { index: 0 },
            ConfigPath::TransformColor { index: 1 },
            ConfigPath::TransformColorSpeed { index: 2 },
            ConfigPath::TransformOpacity { index: 0 },
            ConfigPath::TransformDirectColor { index: 0 },
            ConfigPath::TransformAffine { index: 0, param: AffineParam::A },
            ConfigPath::TransformAffine { index: 5, param: AffineParam::G },
            ConfigPath::TransformVariation { index: 0, variation: "linear".to_string() },
            ConfigPath::TransformVariationParam {
                index: 0, variation: "julian".to_string(), param: "power".to_string(),
            },
            ConfigPath::TransformOriginX { index: 0 },
            ConfigPath::TransformOriginY { index: 0 },
            ConfigPath::TransformRotation { index: 0 },
            ConfigPath::TransformScale { index: 0 },
            ConfigPath::TransformPostAffineEnabled { index: 0 },
            ConfigPath::TransformPostAffine { index: 0, param: AffineParam::F },

            // Linked + Final pools (covered separately by
            // test_config_path_string_roundtrip_pool_transforms — included
            // again here to keep this test comprehensive).
            ConfigPath::LinkedTransformAffine { index: 0, param: AffineParam::A },
            ConfigPath::LinkedTransformPostAffineEnabled { index: 0 },
            ConfigPath::LinkedTransformPostAffine { index: 0, param: AffineParam::F },
            ConfigPath::LinkedTransformVariation {
                index: 0, variation: "spherical".to_string(),
            },
            ConfigPath::LinkedTransformVariationParam {
                index: 0, variation: "julian".to_string(), param: "power".to_string(),
            },
            ConfigPath::FinalTransformAffine { index: 0, param: AffineParam::B },
            ConfigPath::FinalTransformPostAffineEnabled { index: 0 },
            ConfigPath::FinalTransformPostAffine { index: 0, param: AffineParam::E },
            ConfigPath::FinalTransformVariation {
                index: 0, variation: "bipolar".to_string(),
            },
            ConfigPath::FinalTransformVariationParam {
                index: 0, variation: "bipolar".to_string(), param: "shift".to_string(),
            },

            // Flame
            ConfigPath::RenderMode,
            ConfigPath::PerspectiveStrength,
            ConfigPath::DepthDensityCompensation,
            ConfigPath::FarDensityFade,
            ConfigPath::SolidStrength,
            ConfigPath::SurfaceThickness,
            ConfigPath::SolidShadowStrength,
            ConfigPath::ShadingStrength,
            ConfigPath::SolidAmbient,
            ConfigPath::SolidDiffuse,
            ConfigPath::SolidSpecular,
            ConfigPath::SolidShininess,
            ConfigPath::SsaoStrength,
            ConfigPath::SsaoRadius,
            ConfigPath::NormalSmoothing,
            ConfigPath::GapFill,
            ConfigPath::SolidLightEnabled { index: 2 },
            ConfigPath::SolidLightParam { index: 1, param: "azimuth".to_string() },
            ConfigPath::SolidLightParam { index: 3, param: "color_b".to_string() },
            ConfigPath::FarDensityFadeStart,
            ConfigPath::Xaos { src: 0, dst: 1 },
            ConfigPath::Xaos { src: 3, dst: 7 },
            ConfigPath::SoloTransform,

            // Effects
            ConfigPath::DensityEffectEnabled { index: 0 },
            ConfigPath::DensityEffectParam { index: 0, param: "strength".to_string() },
            ConfigPath::ColorEffectEnabled { index: 0 },
            ConfigPath::ColorEffectParam { index: 0, param: "amount".to_string() },

            // System
            ConfigPath::SystemIterationsPerThread,
            ConfigPath::SystemBurnIn,
            ConfigPath::SystemVsyncEnabled,
            ConfigPath::SystemTargetFps,
            ConfigPath::SystemFlyMouseSensitivity,
            ConfigPath::SystemFlyMoveSpeed,
            ConfigPath::SystemFlySprintMultiplier,
            ConfigPath::SystemFlyInvertY,
            ConfigPath::SystemFlyCameraMode,
            ConfigPath::SystemExportWidth,
            ConfigPath::SystemExportHeight,
            ConfigPath::SystemPngStripMetadata,
            ConfigPath::SystemLanguage,
            ConfigPath::SystemShowHelpOnStartup,
        ];

        let mut failed: Vec<String> = Vec::new();
        for path in paths {
            let key = path.to_string_key();
            match ConfigPath::from_string_key(&key) {
                Some(parsed) if parsed == path => { /* ok */ }
                Some(parsed) => {
                    failed.push(format!(
                        "key={:?} → parsed={:?} but expected={:?}",
                        key, parsed, path
                    ));
                }
                None => {
                    failed.push(format!("key={:?} (from {:?}) failed to parse", key, path));
                }
            }
        }

        assert!(
            failed.is_empty(),
            "ConfigPath roundtrip failures:\n  {}",
            failed.join("\n  "),
        );
    }

    #[test]
    fn test_config_path_string_roundtrip_pool_transforms() {
        // Linked + Final pool paths — animation tracks save these as
        // strings and re-parse on load. Must roundtrip cleanly or
        // animation silently no-ops on those pool members.
        let paths = vec![
            ConfigPath::LinkedTransformAffine { index: 0, param: AffineParam::A },
            ConfigPath::LinkedTransformAffine { index: 7, param: AffineParam::G },
            ConfigPath::LinkedTransformPostAffineEnabled { index: 2 },
            ConfigPath::LinkedTransformPostAffine { index: 1, param: AffineParam::F },
            ConfigPath::LinkedTransformVariation {
                index: 3,
                variation: "spherical".to_string(),
            },
            ConfigPath::LinkedTransformVariationParam {
                index: 0,
                variation: "julian".to_string(),
                param: "power".to_string(),
            },
            ConfigPath::FinalTransformAffine { index: 0, param: AffineParam::B },
            ConfigPath::FinalTransformPostAffineEnabled { index: 4 },
            ConfigPath::FinalTransformPostAffine { index: 1, param: AffineParam::E },
            ConfigPath::FinalTransformVariation {
                index: 2,
                variation: "bipolar".to_string(),
            },
            ConfigPath::FinalTransformVariationParam {
                index: 0,
                variation: "bipolar".to_string(),
                param: "shift".to_string(),
            },
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for key: {}", key);
        }
    }

    #[test]
    fn test_affine_param_char_roundtrip() {
        let params = vec![
            AffineParam::A,
            AffineParam::B,
            AffineParam::C,
            AffineParam::D,
            AffineParam::E,
            AffineParam::F,
            AffineParam::G,
        ];

        for param in params {
            let c = param.to_char();
            let parsed = AffineParam::from_char(c);
            assert_eq!(parsed, Some(param));
        }
    }
}
