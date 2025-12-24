//! Core variation definitions
//!
//! Each variation is defined with its metadata and WGSL code.
//! This module exports all core variations for registration.

mod basic;
mod advanced;
mod depth3d;
mod rotation3d;
mod full3d;

pub use basic::*;
pub use advanced::*;
pub use depth3d::*;
pub use rotation3d::*;
pub use full3d::*;

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
];
