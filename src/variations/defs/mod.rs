//! Core variation definitions
//!
//! Each variation is defined with its metadata and WGSL code.
//! This module exports all core variations for registration.

mod basic;
mod advanced;
mod depth3d;
mod rotation3d;
mod full3d;
mod blur;
mod pre_phase;
mod post_phase;
mod extended;
mod hyperbolic;
mod trig;
mod quaternion;
mod sqrt_hyperbolic;
mod trig_bs;
mod exp_log;
mod shapes;
mod shapes2;
mod numbered;
mod heavy_init;
mod init_ports;
mod affine_ports;
mod dc;
mod hypertile;
mod classic_2d;
mod mobius_extended;
mod circle_blur;
mod numbered_extras;
mod glynn;
mod wedge_extended;
mod shapes3;
mod radial_extras;
mod internal_weight;
mod pre_post_bridges;
mod truchet;

pub use basic::*;
pub use advanced::*;
pub use depth3d::*;
pub use rotation3d::*;
pub use full3d::*;
pub use blur::*;
pub use pre_phase::*;
pub use post_phase::*;
pub use extended::*;
pub use hyperbolic::*;
pub use trig::*;
pub use quaternion::*;
pub use sqrt_hyperbolic::*;
pub use trig_bs::*;
pub use exp_log::*;
pub use shapes::*;
pub use shapes2::*;
pub use numbered::*;
pub use heavy_init::*;
pub use init_ports::*;
pub use affine_ports::*;
pub use dc::*;
pub use hypertile::*;
pub use classic_2d::*;
pub use mobius_extended::*;
pub use circle_blur::*;
pub use numbered_extras::*;
pub use glynn::*;
pub use wedge_extended::*;
pub use shapes3::*;
pub use radial_extras::*;
pub use internal_weight::*;
pub use pre_post_bridges::*;
pub use truchet::*;

use super::definition::VariationDef;

/// All core variations in registration order
///
/// IMPORTANT: Never reorder existing entries - only append new ones.
/// The order determines variation indices for preset compatibility.
pub static ALL_VARIATIONS: &[&VariationDef] = &[
    // Basic 2D (0-4)
    &LINEAR,
    &SINUSOIDAL,
    &SPHERICAL,
    &SWIRL,
    &HORSESHOE,
    // Advanced 2D (5-15)
    &POLAR,
    &HANDKERCHIEF,
    &HEART,
    &DISC,
    &SPIRAL,
    &HYPERBOLIC,
    &DIAMOND,
    &EX,
    &JULIA,
    &BENT,
    &WAVES,
    // 3D Depth (16-17)
    &ZCONE,
    &FLATTEN,
    // Full 3D (18)
    &HEMISPHERE,
    // 3D Rotation (19-22)
    &PRE_ROTATE_X,
    &PRE_ROTATE_Y,
    &POST_ROTATE_X,
    &POST_ROTATE_Y,
    // 3D Depth (23)
    &ZSCALE,
    // Extended variations (24+)
    &JULIAN,
    &BLOB,
    &EYEFISH,
    &BUBBLE,
    &CYLINDER,
    &NOISE,
    &BLUR,
    &GAUSSIAN_BLUR,
    &POLAR2,
    &CROSS,
    &LOONIE,
    &SCRY,
    &FOCI,
    &ELLIPTIC,
    &WAVES2,
    &LOG,
    &ESCHER,
    &BIPOLAR,
    &LAZYSUSAN,
    &RINGS2,
    &FAN2,
    &PDJ,
    &CURL,
    &RECTANGLES,
    &SPLITS,
    &NGON,
    &AUGER,
    // Blur variations (83+)
    &ZBLUR,
    &BLUR3D,
    &PRE_BLUR,
    // Pre-phase variations (86+)
    &PRE_ZSCALE,
    &PRE_ZTRANSLATE,
    &PRE_SPHERICAL,
    &PRE_SINUSOIDAL,
    &PRE_DISC,
    &PRE_BWRAPS,
    &PRE_CROP,
    &PRE_FALLOFF2,
    // Normal-phase extended (94+)
    &ZTRANSLATE,
    &JULIA3D,
    &FALLOFF2,
    &WEDGE,
    &EPISPIRAL,
    &BWRAPS,
    &JULIASCOPE,
    &JULIA3DZ,
    &CURL3D,
    &RADIAL_BLUR,
    &BLUR_CIRCLE,
    &BLUR_ZOOM,
    &BLUR_PIXELIZE,
    &SEPARATION,
    &MOBIUS,
    &CROP,
    // Post-phase variations (110+)
    &POST_BWRAPS,
    &POST_CROP,
    &POST_FALLOFF2,
    &POST_CURL,
    &POST_CURL3D,
    // Later additions
    &CPOW,
    // Inverse hyperbolic / arc complex (batch 1, 2026-04-26)
    &ACOTH,
    &ACOSH,
    &ACOSECH,
    &ARCSECH,
    &ARCSECH2,
    &ARCSINH,
    &ARCTANH,
    // Direct trigonometric and hyperbolic (batch 2, 2026-04-26)
    &SIN,
    &COS,
    &TAN,
    &SEC,
    &CSC,
    &COT,
    &SINH,
    &COSH,
    &TANH,
    &COTH,
    &SECH,
    &CSCH,
    // Quaternion-style trig/hyperbolic (batch 3, 2026-04-26)
    &SINQ,
    &COSQ,
    &TANQ,
    &COTQ,
    &SECQ,
    &CSCQ,
    &SINHQ,
    &COSHQ,
    &TANHQ,
    &COTHQ,
    &SECHQ,
    &CSCHQ,
    // Square-root prefixed inverse hyperbolic (batch 4, 2026-04-26)
    &SQRT_ACOTH,
    &SQRT_ACOSH,
    &SQRT_ACOSECH,
    &SQRT_ASECH,
    &SQRT_ASINH,
    &SQRT_ATANH,
    // Brad Stefanov's parameterized direct trig/hyperbolic (batch 5, 2026-04-26)
    &SIN2_BS,
    &COS2_BS,
    &TAN2_BS,
    &SEC2_BS,
    &CSC2_BS,
    &COT2_BS,
    &SINH2_BS,
    &COSH2_BS,
    &TANH2_BS,
    &COTH2_BS,
    &SECH2_BS,
    &CSCH2_BS,
    &EXP2_BS,
    // Exp / log family (batch 6, 2026-04-26)
    &EXP,
    &LOG_DB,
    &LOG_TILE2,
    &TILE_LOG,
    // Misc trig + standalone shapes (batch 7, 2026-04-26)
    &TANCOS,
    &TANGENT,
    &TANGENT3D,
    &SECANT2,
    &COSINE,
    &PETAL,
    &CARDIOID,
    &HELIX,
    &HELICOID,
    &PARABOLA,
    &PIE,
    &PIE3D,
    // Standalone shapes continued (batch 8, 2026-04-26)
    &BUTTERFLY,
    &BUTTERFLY3D,
    &ENNEPERS,
    &PYRAMID,
    &RAYS2,
    &RAYS3,
    &SPIRALWING,
    &WHITNEY_UMBRELLA,
    &CHRYSANTHEMUM,
    &CELL,
    &ENNEPERS2,
    &FLOWER,
    // Numbered/3D variants of existing variations (batch 9, 2026-04-27)
    &SPHERICAL3D,
    &SINUSOIDAL3D,
    &SQUARE,
    &SQUARE3D,
    &DISC3D,
    &BUBBLE2,
    &POPCORN2,
    &SPLITS3D,
    &WAVES2_3D,
    &JULIAQ,
    &JULIA3DQ,
    &JULIAC,
    // Heavy-init variants (batch 10, 2026-04-27)
    &CPOW2,
    &CPOW3,
    &DISC2,
    // Init-dispatch ports (batch 11, 2026-04-28) — variations on the
    // porter-omitted-init watchlist, enabled by the new wgsl_init system
    &TARGET,
    &YIN_YANG,
    // Affine-access ports (off the affine-access watchlist)
    &POPCORN,
    // Direct-color (DC) variations — first uses of writes_color
    &DC_LINEAR,
    &DC_BUBBLE,
    // Hypertile family (Zueuk's hyperbolic tilings)
    &HYPERTILE,
    &HYPERTILE1,
    &HYPERTILE2,
    &HYPERTILE3D,
    &HYPERTILE3D1,
    &HYPERTILE3D2,
    // Classic 2D — popular geometric / blur / panoramic primitives
    &FAN,
    &FISHEYE,
    &GRIDOUT,
    &CIRCULAR,
    &PANORAMA1,
    &PANORAMA2,
    // Möbius family extensions
    &MOBIUSN,
    &MOBIQ,
    // Circle / blur distortions
    &CIRCLEBLUR,
    &CIRCLESPLIT,
    &FLIPCIRCLE,
    &BLUR_LINEAR,
    // Numbered extras (continuation of batch 9 numbered.rs)
    &BIPOLAR2,
    &BLOB3D,
    &CIRCULAR2,
    // Glynn family — Faber's glynnia + eralex61's GlynnSim
    &GLYNNIA,
    &GLYNNIA3,
    &GLYNN_SIM1,
    &GLYNN_SIM2,
    &GLYNN_SIM3,
    // Wedge family extensions
    &WEDGE_JULIA,
    &WEDGE_SPH,
    // Standalone shapes — 3rd batch (super_shape, henon, apollony)
    &SUPER_SHAPE,
    &HENON,
    &APOLLONY,
    // Radial extras — variations whose X/Y output doesn't scale with weight
    &ONION,
    &TARGET_SP,
    // Internal-weight watchlist via needs_transform
    &LOONIE3,
    &LOONIE_3D,
    &SIGMOID,
    &BLOCKY,
    // Pre/post bridges
    &PRE_CURL,
    &POST_JULIAQ,
    &POST_JULIA3DQ,
    // Truchet family
    &TRUCHET_FILL,
];
