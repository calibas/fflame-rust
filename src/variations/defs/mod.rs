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
mod mobiq4d;
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
mod blur_extras;
mod boarders;
mod standalone_exotics;
mod parametric_curves;
mod stub_recoveries;
mod maurer_hyper;
mod misc_2d;
mod misc_extras;
mod bipolar_series;
mod singleton_misc;
mod misc_extras2;
mod misc_extras3;
mod stub_recoveries2;
mod lazy_family;
mod misc_extras4;
mod watchlist_misc;
mod classic_blades_misc;
mod apo_misc;
mod erf_misc;
mod simple_classics;
mod inflate_z;
mod apo_misc7;
mod apo_misc8;
mod apo_misc9;
mod apo_misc10;
mod apo_misc11;
mod apo_misc12;
mod apo_misc13;
mod spin_phase;
mod apo_misc14;
mod apo_misc15;
mod rosoni_misc;
mod apo_misc16;
mod bwraps7_misc;
mod apo_misc17;
mod bwraps2_phase;
mod post_heat_misc;
mod post_rblur_misc;
mod onion2_misc;
mod jacobi_elliptic;
mod circlecrop_phase;
mod circlecrop_misc;
mod exblur_misc;
mod curl_sp_misc;
mod extrude_misc;
mod butterfly_fay_misc;
mod minkowskope_misc;
mod glynnlissa_misc;
mod glynnspiro_misc;
mod glynnsshape_misc;
mod apo_misc18;
mod apo_misc19;
mod apo_misc20;
mod apo_misc21;
mod apo_misc22;
mod sosa_attractors;
mod sosa_attractors2;
mod sosa_attractors3;
mod sosa_attractors4;
mod wf_curves;
mod waves_wf_family;
mod pointgrid_misc;
mod dc_misc;
mod dc_misc2;
mod affine3d_misc;
mod truchet2_misc;
mod truchet_misc;
mod post_axis_symmetry_misc;
mod pre_wave3d_misc;
mod circle_rand_misc;
mod iconattractor_misc;
mod waveblur_misc;
mod siercarpet_misc;
mod popcorn2_3d_misc;
mod jac_asn_misc;
mod plusrecip_misc;
mod gamma_misc;
mod bubblet3d_misc;
mod waves2b_misc;
mod dc_carpet3d_misc;
mod dc_tc_read;
mod vibration2_misc;
mod gridout3d_misc;
mod jubiq4d;
mod jubiq_misc;
mod supershape3d_misc;
mod wz_lost_variations;
mod quaternion_misc;
mod xtrb_misc;
mod curliecue2_misc;
mod farblur_misc;
mod macmillan_misc;
mod harmonograph_misc;
mod rhodonea_misc;
mod complex_misc;
mod hexaplay3d_misc;
mod hexnix3d_misc;
mod klein_group_misc;
mod subflame;
// `pub mod` so the shader builder can call `synth::specialize_wgsl_*`
// directly for per-flame WGSL specialization.
pub mod synth;
mod fractwf;
mod mandelbrot;
mod crackle;
mod dc_perlin;
mod szubieta;
mod chladni;
mod chladni_disc;
mod cymatics3d;
mod orbital;
mod hofstadter;
mod hofstadter3d;
mod polyhedra_project;
mod menger;
mod polychoron;
mod honeycomb;
mod honeycomb4d;
mod quasiconformal;
mod apollonian_gasket;
mod cubic_julia;
mod fuchsian_triangle;
mod hecke_group;
mod hyperbolic_camera;
mod jacobian_counterexample;
mod littlewood;
pub mod mondrianomies;
mod minkowski;
mod minkowski_camera;
mod stereogram;
mod lorentz_mobius;
mod schottky_group;
mod sphere_packing;
mod su_custom;
mod surface_group;
mod von_dyck;
mod su_mobius;
mod glsl_fractals;
mod glsl_tilings;
mod glsl_fields;
mod octapol;
mod quaternion_julia;
mod quaternion_rotation;
mod solid_samplers;
mod quaternion_linear;
mod quaternion_julia_set;
mod quaternion_camera;
mod yplot3d_wf;
mod yplot2d_wf;
mod parplot2d_wf;
mod polarplot2d_wf;
mod polarplot3d_wf;
mod combimirror;
mod sunflower;
mod dc_hexes_wf;
mod dc_mandelbox2d;
mod mobius_dragon_3d;
mod sym_ng;
mod sym_bg;
mod prose3d;
mod cut_hextruchetflow;
mod cut_sqcir;
mod cut_yuebing;
mod cut_vasarely;
mod cut_sincos;
mod cut_simple;
mod cut_helper;
mod cut_helper2;
mod analytic_blurs;

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
pub use mobiq4d::*;
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
pub use blur_extras::*;
pub use boarders::*;
pub use standalone_exotics::*;
pub use parametric_curves::*;
pub use stub_recoveries::*;
pub use maurer_hyper::*;
pub use misc_2d::*;
pub use misc_extras::*;
pub use bipolar_series::*;
pub use singleton_misc::*;
pub use misc_extras2::*;
pub use misc_extras3::*;
pub use stub_recoveries2::*;
pub use lazy_family::*;
pub use misc_extras4::*;
pub use watchlist_misc::*;
pub use classic_blades_misc::*;
pub use apo_misc::*;
pub use erf_misc::*;
pub use simple_classics::*;
pub use inflate_z::*;
pub use apo_misc7::*;
pub use apo_misc8::*;
pub use apo_misc9::*;
pub use apo_misc10::*;
pub use apo_misc11::*;
pub use apo_misc12::*;
pub use apo_misc13::*;
pub use spin_phase::*;
pub use apo_misc14::*;
pub use apo_misc15::*;
pub use rosoni_misc::*;
pub use apo_misc16::*;
pub use bwraps7_misc::*;
pub use apo_misc17::*;
pub use bwraps2_phase::*;
pub use post_heat_misc::*;
pub use post_rblur_misc::*;
pub use onion2_misc::*;
pub use jacobi_elliptic::*;
pub use circlecrop_phase::*;
pub use circlecrop_misc::*;
pub use exblur_misc::*;
pub use curl_sp_misc::*;
pub use extrude_misc::*;
pub use butterfly_fay_misc::*;
pub use minkowskope_misc::*;
pub use glynnlissa_misc::*;
pub use glynnspiro_misc::*;
pub use glynnsshape_misc::*;
pub use apo_misc18::*;
pub use apo_misc19::*;
pub use apo_misc20::*;
pub use apo_misc21::*;
pub use apo_misc22::*;
pub use sosa_attractors::*;
pub use sosa_attractors2::*;
pub use sosa_attractors3::*;
pub use sosa_attractors4::*;
pub use wf_curves::*;
pub use waves_wf_family::*;
pub use pointgrid_misc::*;
pub use dc_misc::*;
pub use dc_misc2::*;
pub use affine3d_misc::*;
pub use truchet2_misc::*;
pub use truchet_misc::*;
pub use post_axis_symmetry_misc::*;
pub use pre_wave3d_misc::*;
pub use circle_rand_misc::*;
pub use iconattractor_misc::*;
pub use waveblur_misc::*;
pub use siercarpet_misc::*;
pub use popcorn2_3d_misc::*;
pub use jac_asn_misc::*;
pub use plusrecip_misc::*;
pub use gamma_misc::*;
pub use bubblet3d_misc::*;
pub use waves2b_misc::*;
pub use dc_carpet3d_misc::*;
pub use dc_tc_read::*;
pub use vibration2_misc::*;
pub use gridout3d_misc::*;
pub use jubiq4d::*;
pub use jubiq_misc::*;
pub use supershape3d_misc::*;
pub use wz_lost_variations::*;
pub use quaternion_misc::*;
pub use xtrb_misc::*;
pub use curliecue2_misc::*;
pub use farblur_misc::*;
pub use macmillan_misc::*;
pub use harmonograph_misc::*;
pub use rhodonea_misc::*;
pub use complex_misc::*;
pub use hexaplay3d_misc::*;
pub use hexnix3d_misc::*;
pub use klein_group_misc::*;
pub use subflame::*;
pub use synth::*;
pub use fractwf::*;
pub use mandelbrot::*;
pub use crackle::*;
pub use dc_perlin::*;
pub use szubieta::*;
pub use chladni::*;
pub use chladni_disc::*;
pub use cymatics3d::*;
pub use orbital::*;
pub use hofstadter::*;
pub use hofstadter3d::*;
pub use polyhedra_project::*;
pub use menger::*;
pub use polychoron::*;
pub use honeycomb::*;
pub use honeycomb4d::*;
pub use quasiconformal::*;
pub use apollonian_gasket::*;
pub use cubic_julia::*;
pub use fuchsian_triangle::*;
pub use hecke_group::*;
pub use hyperbolic_camera::*;
pub use jacobian_counterexample::*;
pub use littlewood::*;
pub use mondrianomies::*;
pub use minkowski::*;
pub use minkowski_camera::*;
pub use stereogram::*;
pub use lorentz_mobius::*;
pub use schottky_group::*;
pub use sphere_packing::*;
pub use su_custom::*;
pub use surface_group::*;
pub use von_dyck::*;
pub use su_mobius::*;
pub use glsl_fractals::*;
pub use glsl_tilings::*;
pub use glsl_fields::*;
pub use octapol::*;
pub use quaternion_julia::*;
pub use quaternion_rotation::*;
pub use solid_samplers::*;
pub use quaternion_linear::*;
pub use quaternion_julia_set::*;
pub use quaternion_camera::*;
pub use yplot3d_wf::*;
pub use yplot2d_wf::*;
pub use parplot2d_wf::*;
pub use polarplot2d_wf::*;
pub use polarplot3d_wf::*;
pub use combimirror::*;
pub use sunflower::*;
pub use dc_hexes_wf::*;
pub use dc_mandelbox2d::*;
pub use mobius_dragon_3d::*;
pub use sym_ng::*;
pub use sym_bg::*;
pub use prose3d::*;
pub use cut_hextruchetflow::*;
pub use cut_sqcir::*;
pub use cut_yuebing::*;
pub use cut_vasarely::*;
pub use cut_sincos::*;
pub use cut_simple::*;
pub use cut_helper::*;
pub use cut_helper2::*;
pub use analytic_blurs::*;

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
    &SCRY_3D,
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
    &JULIASCOPE_3DB,
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
    &ARCTRUCHET,
    // Additional blur primitives
    &SINEBLUR,
    &STARBLUR,
    &R_CIRCLEBLUR,
    // Boarders / border-tile family
    &BOARDERS,
    &BOARDERS2,
    &PRE_BOARDERS2,
    &SPLITBRDR,
    // Standalone exotics
    &KALEIDOSCOPE,
    &TAURUS,
    &HOLE2,
    // Parametric curves + 3D crop
    &SPIROGRAPH,
    &LISSAJOUS,
    &VOGEL,
    &CROP3D,
    // Stub-bucket recoveries (cpp PluginVarCalc empty; ported from Java)
    &BSPLIT,
    &CYLINDER2,
    &ECLIPSE,
    &LOZI,
    &PULSE,
    &HYPERSHIFT,
    // Larger param-heavy: maurer_rose + hypercrop
    &MAURER_ROSE,
    &HYPERCROP,
    // Miscellaneous small 2D primitives
    &SPLIT,
    &SQUIRREL,
    &STRIPES,
    &SHIFT,
    &PRESSURE_WAVE,
    &SPHERICALN,
    &SPLIGON,
    &TILE_HLP,
    // Miscellaneous larger / 3D primitives
    &HO,
    &CHUNK,
    &PTRANSFORM,
    &RATIONAL3,
    &TILE_REVERSE,
    &ORTHO,
    // Bipolar (B-) and Elliptic (E-) coordinate-system series (Faber)
    &BCOLLIDE,
    &BMOD,
    &BSWIRL,
    &BARYCENTROID,
    &ECOLLIDE,
    &EMOD,
    &ESWIRL,
    &ESCALE,
    &EPUSH,
    &EROTATE,
    // Singleton misc
    &CORNERS,
    &MODULUS,
    &OCTAGON,
    &CIRCUS,
    &CIRCLIZE,
    &CIRCLIZE2,
    &ATAN_VAR,
    &MURL,
    // More misc
    &COLLIDEOSCOPE,
    &BENT2,
    &MCARPET,
    &LINEART3D,
    &OSCILLOSCOPE,
    &FIBONACCI2,
    // Even more misc
    &OSCILLOSCOPE2,
    &LINEART,
    &PHOENIX_JULIA,
    &POW_BLOCK,
    // Stub-bucket recoveries 2nd file
    &DISC3,
    &PROJECTIVE,
    &TQMIRROR,
    &INTERSECTION,
    // Lazy family (Faber / FarDareisMai)
    &LAZYJESS,
    &LAZYTRAVIS,
    // Misc 4
    &ANAMORPHCYL,
    &SVF,
    &SHREDLIN,
    &SHREDRAD,
    &XHEART,
    &STWIN,
    &WHORL,
    &DEVIL_WARP,
    // Internal-weight watchlist + small misc
    &TRADE,
    &VORON,
    &SQUIRCULAR,
    &FLUX,
    &RAYS,
    &RAYS1,
    &LOONIE2,
    &FOURTH,
    // Classic blades + misc
    &ARCH,
    &BI_LINEAR,
    &BLADE,
    &BLADE3D,
    &SQUARIZE,
    &SQUISH,
    &TWOFACE,
    &TWINTRIAN,
    &UNPOLAR,
    // Apo misc 5
    &XERF,
    &INVERTED_JULIA,
    &IDISC,
    &CONIC,
    &POWER,
    &ROUNDSPHER,
    &ROUNDSPHER3D,
    &CUBIC3D,
    &CUBIC_LATTICE_3D,
    &CHECKS,
    &CONE,
    // Erf family + small misc
    &ERF,
    &ERF3D,
    &D_SPHERICAL,
    &DUSTPOINT,
    &DELTAA,
    &EDISC,
    &CURVE,
    &ELLIPTIC2,
    // Simple Apophysis classics
    &EXP2,
    &EXPONENTIAL,
    &FLIPY,
    &FUNNEL,
    &INVPOLAR,
    &PERSPECTIVE,
    &LINE,
    &HOLESQ,
    // Inflate Z family + foci_3D + sintrange
    &INFLATEZ_1,
    &INFLATEZ_2,
    &INFLATEZ_3,
    &INFLATEZ_4,
    &INFLATEZ_5,
    &INFLATEZ_6,
    &SINTRANGE,
    &FOCI_3D,
    // Apo misc 7
    &ASTERIA,
    &ESTIQ,
    &FDISC,
    &BTRANSFORM,
    &NPOLAR,
    // Apo misc 8
    &CSC_SQUARED,
    &HYPERBOLICELLIPSE,
    &LAYERED_SPIRAL,
    &ATAN2_SPIRALS,
    &GRIDOUT2,
    // Apo misc 9
    &EJULIA,
    &EMOTION,
    &FLOWER_DB,
    &JULIAN2,
    // Apo misc 10
    &MASK,
    &OVOID3D,
    &MURL2,
    &MINKQM,
    // Apo misc 11
    &SWIRL3,
    &WDISC,
    &SPH3D,
    &INVSQUIRCULAR,
    &SPHERE_NJA,
    // Apo misc 12
    &RINGS,
    &RIPPLED,
    &WAFFLE,
    &STRIPFIT,
    // Apo misc 13
    &Q_ODE,
    &RIPPLE,
    &SCRY2,
    // Spin / pre / post phase
    &PRE_SPIN_Z,
    &POST_SPIN_Z,
    &POST_SPHERICAL,
    &PRE_DISC3D,
    // Apo misc 14
    &WAVES2_RADIAL,
    &SPLIPTIC_BS,
    &POINCARE3D,
    // Apo misc 15
    &PRE_SINUSOIDAL3D,
    &PRE_BLUR3D,
    &JULIAN3DX,
    // Rosoni
    &ROSONI,
    // Apo misc 16
    &SEASHELL3D,
    &HYPERSHIFT2,
    // bwraps7
    &BWRAPS7,
    // Apo misc 17
    &LOQ,
    &SPIROGRAPH3D,
    // bwraps2 phase variants
    &PRE_BWRAPS2,
    &POST_BWRAPS2,
    // post_heat
    &POST_HEAT,
    // post_rblur
    &POST_RBLUR,
    // onion2
    &ONION2,
    // Jacobi elliptic family
    &JAC_SN,
    &JAC_CN,
    &JAC_DN,
    // Circle-crop phase variants
    &PRE_CIRCLECROP,
    &POST_CIRCLECROP,
    // circlecrop (normal phase) + exblur
    &CIRCLECROP,
    &EXBLUR,
    // curl_sp
    &CURL_SP,
    // extrude
    &EXTRUDE,
    // butterfly_fay
    &BUTTERFLY_FAY,
    // minkowskope
    &MINKOWSKOPE,
    // glynnlissa
    &GLYNNLISSA,
    // glynnspiro
    &GLYNNSPIRO,
    // glynnSShape
    &GLYNNSSHAPE,
    // apo_misc18: lazysensen, spherecrop, xheart_blur_wf
    &LAZYSENSEN,
    &SPHERECROP,
    &XHEART_BLUR_WF,
    // apo_misc19: mobius_strip, circleLinear
    &MOBIUS_STRIP,
    &CIRCLE_LINEAR,
    // apo_misc20: cannabiscurve_wf, spherical3D_wf, swirl3D_wf
    &CANNABISCURVE_WF,
    &SPHERICAL3D_WF,
    &SWIRL3D_WF,
    // apo_misc21: heart_wf, post_ztranslate_wf, post_mirror_wf
    &HEART_WF,
    &POST_ZTRANSLATE_WF,
    &POST_MIRROR_WF,
    // apo_misc22: dc_carpet, post_point_symmetry_wf, cpow3_wf
    &DC_CARPET,
    &POST_POINT_SYMMETRY_WF,
    &CPOW3_WF,
    // sosa_attractors: clifford_js, svensson_js, sattractor_js
    &CLIFFORD_JS,
    &SVENSSON_JS,
    &SATTRACTOR_JS,
    // sosa_attractors2: threepoint_js, lorenz_js, woggle_js
    &THREEPOINT_JS,
    &LORENZ_JS,
    &WOGGLE_JS,
    // sosa_attractors3: lace_js, wallpaper_js
    &LACE_JS,
    &WALLPAPER_JS,
    // sosa_attractors4: hadamard_js, invtree_js, crown_js
    &HADAMARD_JS,
    &INVTREE_JS,
    &CROWN_JS,
    // wf_curves: epispiral_wf, cloverleaf_wf, rose_wf, bubble_wf
    &EPISPIRAL_WF,
    &CLOVERLEAF_WF,
    &ROSE_WF,
    &BUBBLE_WF,
    &PLANE_WF,
    &CHECKERBOARD_WF,
    // waves_wf_family: waves2_wf, waves3_wf, waves4_wf, dinis_surface_wf
    &WAVES2_WF,
    &WAVES3_WF,
    &WAVES4_WF,
    &DINIS_SURFACE_WF,
    // pointgrid_misc: pointgrid_wf, pointgrid3d_wf, apocarpet_js
    &POINTGRID_WF,
    &POINTGRID3D_WF,
    &APOCARPET_JS,
    // dc_misc: dc_cylinder, dc_cylinder2, dc_triangle
    &DC_CYLINDER,
    &DC_CYLINDER2,
    &DC_TRIANGLE,
    // dc_misc2: dc_cube, pre_rect_wf
    &DC_CUBE,
    &PRE_RECT_WF,
    // affine3d_misc: affine3D
    &AFFINE3D,
    // truchet2_misc: truchet2
    &TRUCHET2,
    // truchet_misc: truchet
    &TRUCHET,
    // post_axis_symmetry_misc: post_axis_symmetry_wf
    &POST_AXIS_SYMMETRY_WF,
    // pre_wave3d_misc: pre_wave3D_wf
    &PRE_WAVE3D_WF,
    // circle_rand_misc: circleRand, CircleTrans1
    &CIRCLE_RAND,
    &CIRCLE_TRANS1,
    // iconattractor_misc: iconattractor_js
    &ICONATTRACTOR_JS,
    // waveblur_misc: waveblur_wf
    &WAVEBLUR_WF,
    // siercarpet_misc: siercarpet_js
    &SIERCARPET_JS,
    // popcorn2_3d_misc: popcorn2_3D
    &POPCORN2_3D,
    // jac_asn_misc: jac_asn
    &JAC_ASN,
    // plusrecip_misc: plusrecip
    &PLUSRECIP,
    // gamma_misc: gamma
    &GAMMA,
    // bubblet3d_misc: bubbleT3D
    &BUBBLE_T3D,
    // waves2b_misc: waves2b
    &WAVES2B,
    // dc_carpet3d_misc: dc_carpet3D
    &DC_CARPET3D,
    // dc_tc_read: TC-read variations (read *vc to drive spatial Z)
    &DC_ZTRANSL,
    &PRE_DCZTRANSL,
    &COLORSCALE_WF,
    &POST_COLORSCALE_WF,
    // vibration2_misc: vibration2 (26 user — first port unblocked by packed buffer)
    &VIBRATION2,
    // gridout3d_misc: gridout3D (26 user)
    &GRIDOUT_3D,
    // jubiq_misc: jubiq (24 user + 2 init)
    &JUBIQ,
    // supershape3d_misc: superShape3d (16 user + 10 init)
    &SUPERSHAPE_3D,
    // wz_lost_variations: z (13 user + 7 init), w (14 user + 7 init) — Faber's Lost Variations
    &Z_VARIATION,
    &W_VARIATION,
    // quaternion_misc: quaternion (92 user + 1 init = 93 slots) — zephyrtronium / Stefanov mega-variation
    &QUATERNION,
    // xtrb_misc: xtrb (6 user + 22 init = 28 slots) — Zueuk's TriBorders trilinear hex variation
    &XTRB,
    // curliecue2_misc: curliecue2 (1 user + 4 state) — Sosa walker; first state-using port
    &CURLIECUE2,
    // farblur_misc: farblur (6 user + 5 state, needs_accum) — zephyrtronium; first accum-reading port
    &FARBLUR,
    // macmillan_misc: macmillan (5 user + 3 state, needs_accum + writes_color + custom init)
    &MACMILLAN,
    // harmonograph_misc: harmonograph_js (18 user) — Sosa damped-pendulum harmonograph
    &HARMONOGRAPH_JS,
    // rhodonea_misc: rhodonea (15 user + 5 init, 7×7 mode switch) — CozyG rose curves
    &RHODONEA,
    // complex_misc: complex (64 user) — cothe / Stefanov 14-subfunction 2D analog of quaternion
    &COMPLEX,
    // hexaplay3d_misc: hexaplay3D (3 user + 3 state, replacement-style accum + state_init) — Berlin 2009
    &HEXAPLAY_3D,
    // hexnix3d_misc: hexnix3D (4 user + 3 state, replacement-style with smooth+majplane modes) — Berlin 2009
    &HEXNIX_3D,
    // klein_group_misc: klein_group (6 user + 16 init + 1 state) — Indra's Pearls Kleinian limit set
    &KLEIN_GROUP,
    // subflame: subflame_wf (8 user + 5 state, needs_rng + writes_color, blur class)
    // — JWildfire's nested-IFS variation. P3 registers; P4 implements.
    &SUBFLAME_WF,
    // pre-phase variant of subflame_wf — same nested IFS, output goes to
    // the affine point instead of the variation sum. Body intentionally
    // ignores scale/angle/offset (JWildfire's transform() override does
    // a raw `q` assignment).
    &PRE_SUBFLAME_WF,
    &SYNTH,
    // fract_*_wf family — escape-time fractals (Dragon, Julia, Mandelbrot,
    // Meteors, Pearls, Salamander). All share the iterate-mode body via
    // shaders/core/fractwf.wgsl; per-variation difference is just the
    // iterator-step math (selected via a `kind` enum).
    &FRACT_DRAGON_WF,
    &FRACT_METEORS_WF,
    &FRACT_PEARLS_WF,
    &FRACT_SALAMANDER_WF,
    &FRACT_JULIA_WF,
    &FRACT_MANDELBROT_WF,
    // Standalone `mandelbrot` — random-walk Buddhabrot, persistent state
    // (3 slots), distinct from the fract_*_wf family. Source predates JWF
    // (originally Jed Kelsey's Apophysis plugin, 2007).
    &MANDELBROT,
    // crackle / dc_crackle_wf — Voronoi-cell base shapes (slobo777).
    // Share `crackle_body` via shader-builder dedupe; require the
    // noise.wgsl + voronoi.wgsl helpers.
    &CRACKLE,
    &DC_CRACKLE_WF,
    // dc_perlin — Perlin-noise base shape with direct color.
    // Requires noise.wgsl only (no Voronoi).
    &DC_PERLIN,
    // szubieta — grid-of-polygons base shape (Jesus Sosa, 2018).
    // JWildfire builds a primitive list in init(); we compute the
    // same thing in pure per-call math.
    &SZUBIETA,
    // glsl_* family — JWildfire's shadertoy-style procedural base
    // shapes. Spatial output is always the uniform [-0.5, 0.5]²
    // sample; the interesting content is in the per-pixel direct-RGB
    // color computation, written to `vrc` via Feature::WritesRgb.
    // Each `glsl_*` is an independent hand-port of one specific
    // shadertoy algorithm (not user-supplied code — see
    // docs/projects/variation-port-blockers.md for the distinction).
    // Grouped by theme: iterative fractals, kaleidoscope/tilings,
    // procedural fields.
    &GLSL_MANDELBOX2D,
    &GLSL_KALISET,
    &GLSL_KALISET2,
    &GLSL_APOLLONIAN,
    &GLSL_KALEIDOSCOPIC,
    &GLSL_KALEIDOCOMPLEX,
    &GLSL_HYPERBOLICTILE,
    &GLSL_MANDALA,
    &GLSL_SQUARES,
    &GLSL_HOSHI,
    &GLSL_ACRILIC,
    &GLSL_CIRCLESBLUE,
    &GLSL_CIRCUITS,
    &GLSL_FRACTALDOTS,
    &GLSL_STARSFIELD,
    &GLSL_GRID3D,
    // octapol (4 user + 2 init, needs_accum) — Georg K. (xyrus02)
    // octagon-with-polar-core; miss branches clobber the accumulator
    // per JWF semantics.
    &OCTAPOL,
    // linear3D — split from `linear` (which used to alias it) because
    // JWF gives them different z semantics: linear3D writes z
    // unconditionally (Feature::AlwaysZ), linear gates z on
    // preserve_z. Same identity body.
    &LINEAR3D,
    // yplot3d_wf — 3D surface plot, preset-only port (7 baked
    // formulas; JWF's runtime-compiled custom formulas unsupported).
    &YPLOT3D_WF,
    // Rest of the JWF plot family — same preset-only model as
    // yplot3d_wf; formula switches generated by
    // scripts/gen_plot_wf_formulas.py. isosfplot3d_wf deliberately NOT
    // ported (needs a doHide / plot-suppression mechanism).
    &YPLOT2D_WF,
    &PARPLOT2D_WF,
    &POLARPLOT2D_WF,
    &POLARPLOT3D_WF,
    // combimirror — combined V/H/Z/point mirror with per-branch color
    // shifts (Thomas Michels). Replace-style normal variation; idisc
    // weight cancellation.
    &COMBIMIRROR,
    // sunflower — phyllotaxis spiral of regular polygons (Jesus Sosa);
    // DrawFunc reduced to per-call math + the nBlur polygon sampler.
    &SUNFLOWER,
    // dc_hexes_wf — hexagonal Voronoi cell warp with Voronoi-distance
    // direct color (slobo777's Hexes / thargor6). Deterministic.
    &DC_HEXES_WF,
    // dc_mandelbox2D — 20-iter Mandelbox escape coloring (Jesus Sosa,
    // DC_BaseFunc). Shares the iteration with glsl_mandelbox2D but is a
    // distinct variation (zoom/ColorOnly sampling, DC color/Z model).
    &DC_MANDELBOX2D,
    // mobius_dragon_3D — Möbius/reciprocal feedback + Log_tile spread
    // (Whittaker Courtney). Replace-style; idisc weight cancellation.
    &MOBIUS_DRAGON_3D,
    // sym_ng1..17 — Network Symmetry Group base shapes (Jesus Sosa);
    // generated by scripts/gen_sym_ng.py.
    &SYM_NG1,
    &SYM_NG2,
    &SYM_NG3,
    &SYM_NG4,
    &SYM_NG5,
    &SYM_NG6,
    &SYM_NG7,
    &SYM_NG8,
    &SYM_NG9,
    &SYM_NG10,
    &SYM_NG11,
    &SYM_NG12,
    &SYM_NG13,
    &SYM_NG14,
    &SYM_NG15,
    &SYM_NG16,
    &SYM_NG17,
    // sym_bg1..7 — Band Symmetry Group base shapes (Jesus Sosa);
    // generated by scripts/gen_sym_ng.py.
    &SYM_BG1,
    &SYM_BG2,
    &SYM_BG3,
    &SYM_BG4,
    &SYM_BG5,
    &SYM_BG6,
    &SYM_BG7,
    &PROSE3D,
    &CUT_HEXTRUCHETFLOW,
    &CUT_SQCIR,
    &CUT_YUEBING,
    &CUT_VASARELY,
    &CUT_SINCOS,
    &CUT_CHAINS,
    &CUT_HEXDOTS,
    &CUT_BOOLEANS,
    &CUT_SQSPLITS,
    &CUT_BTREE,
    &CUT_FRACTAL,
    &CUT_GLYPHO,
    &CUT_CIRCDES,
    &CUT_SNOWFLAKE,
    &CUT_JIGSAW,
    &CUT_RGRID,
    &CUT_X,
    &CUT_METABALLS,
    &CUT_KALEIDO,
    &CUT_SPIRAL,
    &CUT_SWARP,
    &CUT_APOLLONIAN,
    &CUT_ZIGZAG,
    &CUT_BRICKS,
    &CUT_WEB,
    &CUT_FINGERPRINT,
    &CUT_TILEILLUSION,
    &CUT_PATTERN,
    &CUT_CELTIC,
    &ANALYTIC_BLUR,
    &ANALYTIC_GAUSSIAN_BLUR,
    &QUATERNION_JULIA,
    &QUATERNION_ROTATION,
    &QUATERNION_LINEAR,
    &QUATERNION_JULIA_SET,
    &QUATERNION_CAMERA,
    &SOLID_SPHERE,
    // bubble_solid was removed before release of this branch (its fill
    // consumed image samples — replaced by the side-emitting
    // sphere_volume). Tail position, so no stable IDs shifted.
    &SPHERE_VOLUME,
    &CHLADNI,
    &CHLADNI_DISC,
    &CYMATICS3D,
    &ORBITAL,
    &HOFSTADTER,
    &HOFSTADTER3D,
    &POLYHEDRON,
    &POLYHEDRON_VOLUME,
    &MENGER,
    &POLYCHORON,
    &HONEYCOMB,
    &HONEYCOMB4D,
    &QUASICONFORMAL,
    &SU_MOBIUS,
    &FUCHSIAN_TRIANGLE,
    &JACOBIAN_COUNTEREXAMPLE,
    &LORENTZ_MOBIUS,
    &APOLLONIAN_GASKET,
    &HECKE_GROUP,
    &SCHOTTKY_GROUP,
    &CUBIC_JULIA,
    &SU_CUSTOM,
    &VON_DYCK,
    &SURFACE_GROUP,
    &MOBIQ4D,
    &JUBIQ4D,
    &LITTLEWOOD,
    &SPHERE_PACKING,
    &HYPERBOLIC_CAMERA,
    &MONDRIANOMIES,
    &MINKOWSKI,
    &MINKOWSKI_CAMERA,
    &STEREOGRAM,
];
