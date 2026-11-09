//! Rhai twins of the built-in variations (docs/projects/rhai-plugins.md,
//! P7 steps 3 and 4): each a Rhai variation that does what its built-in
//! does, under the built-in's display name, with its built-in's
//! parameters exactly -- names, captions, hints, kinds, defaults, ranges,
//! a free range, a log slider (`every_variation_twin_compiles_as_its_builtin`)
//! -- and its features. Held to the built-in per point by the probe's
//! pairs (`probe::pairs`, `every_variation_twin_runs_as_its_builtin`):
//! exact where no trig is, within ulps where the language's range-reduced
//! trig meets the built-in's hardware trig (the portable side, the user's
//! rule: whatever makes WebGPU, Metal and Vulkan agree).
//!
//! Sources in `assets/variations/twins/`, one a built-in, each named
//! `rhai_<the built-in's name, lowercased>` (`INFO.name`, what `vary`
//! calls it by); [`skeleton`] writes one's `INFO` and `PARAMS` from the
//! def, with the built-in's WGSL as a comment to port from.

use crate::config::escape::CustomSource;
use crate::params::{ParamDef, ParamKind};

/// Every twin: the built-in's name, the twin's source.
pub const TWINS: &[(&str, &str)] = &[
    // Written by hand (V1).
    ("julia", include_str!("../../assets/variations/twins/julia.rhai")),
    ("rings2", include_str!("../../assets/variations/twins/rings2.rhai")),
    ("curl", include_str!("../../assets/variations/twins/curl.rhai")),
    ("flipcircle", include_str!("../../assets/variations/twins/flipcircle.rhai")),
    ("sinusoidal3d", include_str!("../../assets/variations/twins/sinusoidal3d.rhai")),
    ("exp", include_str!("../../assets/variations/twins/exp.rhai")),
    ("hyperbolic", include_str!("../../assets/variations/twins/hyperbolic.rhai")),
    // Translated from the built-ins' WGSL (V2, `scripts/translate_variation_twins.py`).
    ("CircleTrans1", include_str!("../../assets/variations/twins/CircleTrans1.rhai")),
    ("acosech", include_str!("../../assets/variations/twins/acosech.rhai")),
    ("acosh", include_str!("../../assets/variations/twins/acosh.rhai")),
    ("acoth", include_str!("../../assets/variations/twins/acoth.rhai")),
    ("affine3D", include_str!("../../assets/variations/twins/affine3D.rhai")),
    ("anamorphcyl", include_str!("../../assets/variations/twins/anamorphcyl.rhai")),
    ("apocarpet_js", include_str!("../../assets/variations/twins/apocarpet_js.rhai")),
    ("apollonian_gasket", include_str!("../../assets/variations/twins/apollonian_gasket.rhai")),
    ("apollony", include_str!("../../assets/variations/twins/apollony.rhai")),
    ("arch", include_str!("../../assets/variations/twins/arch.rhai")),
    ("arcsech", include_str!("../../assets/variations/twins/arcsech.rhai")),
    ("arcsech2", include_str!("../../assets/variations/twins/arcsech2.rhai")),
    ("arcsinh", include_str!("../../assets/variations/twins/arcsinh.rhai")),
    ("arctanh", include_str!("../../assets/variations/twins/arctanh.rhai")),
    ("arctruchet", include_str!("../../assets/variations/twins/arctruchet.rhai")),
    ("asteria", include_str!("../../assets/variations/twins/asteria.rhai")),
    ("atan", include_str!("../../assets/variations/twins/atan.rhai")),
    ("atan2_spirals", include_str!("../../assets/variations/twins/atan2_spirals.rhai")),
    ("auger", include_str!("../../assets/variations/twins/auger.rhai")),
    ("bCollide", include_str!("../../assets/variations/twins/bCollide.rhai")),
    ("bMod", include_str!("../../assets/variations/twins/bMod.rhai")),
    ("bSwirl", include_str!("../../assets/variations/twins/bSwirl.rhai")),
    ("bTransform", include_str!("../../assets/variations/twins/bTransform.rhai")),
    ("barycentroid", include_str!("../../assets/variations/twins/barycentroid.rhai")),
    ("bent", include_str!("../../assets/variations/twins/bent.rhai")),
    ("bent2", include_str!("../../assets/variations/twins/bent2.rhai")),
    ("bi_linear", include_str!("../../assets/variations/twins/bi_linear.rhai")),
    ("bipolar", include_str!("../../assets/variations/twins/bipolar.rhai")),
    ("bipolar2", include_str!("../../assets/variations/twins/bipolar2.rhai")),
    ("blade", include_str!("../../assets/variations/twins/blade.rhai")),
    ("blade3D", include_str!("../../assets/variations/twins/blade3D.rhai")),
    ("blob", include_str!("../../assets/variations/twins/blob.rhai")),
    ("blob3D", include_str!("../../assets/variations/twins/blob3D.rhai")),
    ("blocky", include_str!("../../assets/variations/twins/blocky.rhai")),
    ("blur", include_str!("../../assets/variations/twins/blur.rhai")),
    ("blur3D", include_str!("../../assets/variations/twins/blur3D.rhai")),
    ("blur_circle", include_str!("../../assets/variations/twins/blur_circle.rhai")),
    ("blur_linear", include_str!("../../assets/variations/twins/blur_linear.rhai")),
    ("blur_pixelize", include_str!("../../assets/variations/twins/blur_pixelize.rhai")),
    ("blur_zoom", include_str!("../../assets/variations/twins/blur_zoom.rhai")),
    ("boarders", include_str!("../../assets/variations/twins/boarders.rhai")),
    ("boarders2", include_str!("../../assets/variations/twins/boarders2.rhai")),
    ("bsplit", include_str!("../../assets/variations/twins/bsplit.rhai")),
    ("bubble", include_str!("../../assets/variations/twins/bubble.rhai")),
    ("bubble2", include_str!("../../assets/variations/twins/bubble2.rhai")),
    ("bubbleT3D", include_str!("../../assets/variations/twins/bubbleT3D.rhai")),
    ("bubble_wf", include_str!("../../assets/variations/twins/bubble_wf.rhai")),
    ("butterfly", include_str!("../../assets/variations/twins/butterfly.rhai")),
    ("butterfly3D", include_str!("../../assets/variations/twins/butterfly3D.rhai")),
    ("butterfly_fay", include_str!("../../assets/variations/twins/butterfly_fay.rhai")),
    ("bwraps", include_str!("../../assets/variations/twins/bwraps.rhai")),
    ("bwraps7", include_str!("../../assets/variations/twins/bwraps7.rhai")),
    ("cannabiscurve_wf", include_str!("../../assets/variations/twins/cannabiscurve_wf.rhai")),
    ("cardioid", include_str!("../../assets/variations/twins/cardioid.rhai")),
    ("cell", include_str!("../../assets/variations/twins/cell.rhai")),
    ("checkerboard_wf", include_str!("../../assets/variations/twins/checkerboard_wf.rhai")),
    ("checks", include_str!("../../assets/variations/twins/checks.rhai")),
    ("chladni", include_str!("../../assets/variations/twins/chladni.rhai")),
    ("chladni_disc", include_str!("../../assets/variations/twins/chladni_disc.rhai")),
    ("chrysanthemum", include_str!("../../assets/variations/twins/chrysanthemum.rhai")),
    ("chunk", include_str!("../../assets/variations/twins/chunk.rhai")),
    ("circleLinear", include_str!("../../assets/variations/twins/circleLinear.rhai")),
    ("circleRand", include_str!("../../assets/variations/twins/circleRand.rhai")),
    ("circleblur", include_str!("../../assets/variations/twins/circleblur.rhai")),
    ("circlecrop", include_str!("../../assets/variations/twins/circlecrop.rhai")),
    ("circlesplit", include_str!("../../assets/variations/twins/circlesplit.rhai")),
    ("circlize", include_str!("../../assets/variations/twins/circlize.rhai")),
    ("circlize2", include_str!("../../assets/variations/twins/circlize2.rhai")),
    ("circular", include_str!("../../assets/variations/twins/circular.rhai")),
    ("circular2", include_str!("../../assets/variations/twins/circular2.rhai")),
    ("circus", include_str!("../../assets/variations/twins/circus.rhai")),
    ("clifford_js", include_str!("../../assets/variations/twins/clifford_js.rhai")),
    ("cloverleaf_wf", include_str!("../../assets/variations/twins/cloverleaf_wf.rhai")),
    ("collideoscope", include_str!("../../assets/variations/twins/collideoscope.rhai")),
    ("colorscale_wf", include_str!("../../assets/variations/twins/colorscale_wf.rhai")),
    ("combimirror", include_str!("../../assets/variations/twins/combimirror.rhai")),
    ("complex", include_str!("../../assets/variations/twins/complex.rhai")),
    ("cone", include_str!("../../assets/variations/twins/cone.rhai")),
    ("conic", include_str!("../../assets/variations/twins/conic.rhai")),
    ("corners", include_str!("../../assets/variations/twins/corners.rhai")),
    ("cos", include_str!("../../assets/variations/twins/cos.rhai")),
    ("cos2_bs", include_str!("../../assets/variations/twins/cos2_bs.rhai")),
    ("cosh", include_str!("../../assets/variations/twins/cosh.rhai")),
    ("cosh2_bs", include_str!("../../assets/variations/twins/cosh2_bs.rhai")),
    ("coshq", include_str!("../../assets/variations/twins/coshq.rhai")),
    ("cosine", include_str!("../../assets/variations/twins/cosine.rhai")),
    ("cosq", include_str!("../../assets/variations/twins/cosq.rhai")),
    ("cot", include_str!("../../assets/variations/twins/cot.rhai")),
    ("cot2_bs", include_str!("../../assets/variations/twins/cot2_bs.rhai")),
    ("coth", include_str!("../../assets/variations/twins/coth.rhai")),
    ("coth2_bs", include_str!("../../assets/variations/twins/coth2_bs.rhai")),
    ("cothq", include_str!("../../assets/variations/twins/cothq.rhai")),
    ("cotq", include_str!("../../assets/variations/twins/cotq.rhai")),
    ("cpow", include_str!("../../assets/variations/twins/cpow.rhai")),
    ("cpow2", include_str!("../../assets/variations/twins/cpow2.rhai")),
    ("cpow3", include_str!("../../assets/variations/twins/cpow3.rhai")),
    ("cpow3_wf", include_str!("../../assets/variations/twins/cpow3_wf.rhai")),
    ("crackle", include_str!("../../assets/variations/twins/crackle.rhai")),
    ("crackle_fast", include_str!("../../assets/variations/twins/crackle_fast.rhai")),
    ("crop", include_str!("../../assets/variations/twins/crop.rhai")),
    ("crop3D", include_str!("../../assets/variations/twins/crop3D.rhai")),
    ("cross", include_str!("../../assets/variations/twins/cross.rhai")),
    ("crown_js", include_str!("../../assets/variations/twins/crown_js.rhai")),
    ("csc", include_str!("../../assets/variations/twins/csc.rhai")),
    ("csc2_bs", include_str!("../../assets/variations/twins/csc2_bs.rhai")),
    ("csc_squared", include_str!("../../assets/variations/twins/csc_squared.rhai")),
    ("csch", include_str!("../../assets/variations/twins/csch.rhai")),
    ("csch2_bs", include_str!("../../assets/variations/twins/csch2_bs.rhai")),
    ("cschq", include_str!("../../assets/variations/twins/cschq.rhai")),
    ("cscq", include_str!("../../assets/variations/twins/cscq.rhai")),
    ("cubic3D", include_str!("../../assets/variations/twins/cubic3D.rhai")),
    ("cubicLattice_3D", include_str!("../../assets/variations/twins/cubicLattice_3D.rhai")),
    ("cubic_julia", include_str!("../../assets/variations/twins/cubic_julia.rhai")),
    ("curl3D", include_str!("../../assets/variations/twins/curl3D.rhai")),
    ("curl_sp", include_str!("../../assets/variations/twins/curl_sp.rhai")),
    ("curliecue2", include_str!("../../assets/variations/twins/curliecue2.rhai")),
    ("curve", include_str!("../../assets/variations/twins/curve.rhai")),
    ("cut_apollonian", include_str!("../../assets/variations/twins/cut_apollonian.rhai")),
    ("cut_booleans", include_str!("../../assets/variations/twins/cut_booleans.rhai")),
    ("cut_bricks", include_str!("../../assets/variations/twins/cut_bricks.rhai")),
    ("cut_btree", include_str!("../../assets/variations/twins/cut_btree.rhai")),
    ("cut_celtic", include_str!("../../assets/variations/twins/cut_celtic.rhai")),
    ("cut_chains", include_str!("../../assets/variations/twins/cut_chains.rhai")),
    ("cut_circdes", include_str!("../../assets/variations/twins/cut_circdes.rhai")),
    ("cut_fingerprint", include_str!("../../assets/variations/twins/cut_fingerprint.rhai")),
    ("cut_fractal", include_str!("../../assets/variations/twins/cut_fractal.rhai")),
    ("cut_glypho", include_str!("../../assets/variations/twins/cut_glypho.rhai")),
    ("cut_hexdots", include_str!("../../assets/variations/twins/cut_hexdots.rhai")),
    ("cut_hextruchetflow", include_str!("../../assets/variations/twins/cut_hextruchetflow.rhai")),
    ("cut_jigsaw", include_str!("../../assets/variations/twins/cut_jigsaw.rhai")),
    ("cut_kaleido", include_str!("../../assets/variations/twins/cut_kaleido.rhai")),
    ("cut_metaballs", include_str!("../../assets/variations/twins/cut_metaballs.rhai")),
    ("cut_pattern", include_str!("../../assets/variations/twins/cut_pattern.rhai")),
    ("cut_rgrid", include_str!("../../assets/variations/twins/cut_rgrid.rhai")),
    ("cut_sincos", include_str!("../../assets/variations/twins/cut_sincos.rhai")),
    ("cut_snowflake", include_str!("../../assets/variations/twins/cut_snowflake.rhai")),
    ("cut_spiral", include_str!("../../assets/variations/twins/cut_spiral.rhai")),
    ("cut_sqcir", include_str!("../../assets/variations/twins/cut_sqcir.rhai")),
    ("cut_sqsplits", include_str!("../../assets/variations/twins/cut_sqsplits.rhai")),
    ("cut_swarp", include_str!("../../assets/variations/twins/cut_swarp.rhai")),
    ("cut_tileillusion", include_str!("../../assets/variations/twins/cut_tileillusion.rhai")),
    ("cut_vasarely", include_str!("../../assets/variations/twins/cut_vasarely.rhai")),
    ("cut_web", include_str!("../../assets/variations/twins/cut_web.rhai")),
    ("cut_x", include_str!("../../assets/variations/twins/cut_x.rhai")),
    ("cut_yuebing", include_str!("../../assets/variations/twins/cut_yuebing.rhai")),
    ("cut_zigzag", include_str!("../../assets/variations/twins/cut_zigzag.rhai")),
    ("cylinder", include_str!("../../assets/variations/twins/cylinder.rhai")),
    ("cylinder2", include_str!("../../assets/variations/twins/cylinder2.rhai")),
    ("cymatics3d", include_str!("../../assets/variations/twins/cymatics3d.rhai")),
    ("d_spherical", include_str!("../../assets/variations/twins/d_spherical.rhai")),
    ("dc_bubble", include_str!("../../assets/variations/twins/dc_bubble.rhai")),
    ("dc_carpet", include_str!("../../assets/variations/twins/dc_carpet.rhai")),
    ("dc_carpet3D", include_str!("../../assets/variations/twins/dc_carpet3D.rhai")),
    ("dc_crackle_wf", include_str!("../../assets/variations/twins/dc_crackle_wf.rhai")),
    ("dc_cube", include_str!("../../assets/variations/twins/dc_cube.rhai")),
    ("dc_cylinder", include_str!("../../assets/variations/twins/dc_cylinder.rhai")),
    ("dc_cylinder2", include_str!("../../assets/variations/twins/dc_cylinder2.rhai")),
    ("dc_hexes_wf", include_str!("../../assets/variations/twins/dc_hexes_wf.rhai")),
    ("dc_linear", include_str!("../../assets/variations/twins/dc_linear.rhai")),
    ("dc_mandelbox2D", include_str!("../../assets/variations/twins/dc_mandelbox2D.rhai")),
    ("dc_perlin", include_str!("../../assets/variations/twins/dc_perlin.rhai")),
    ("dc_triangle", include_str!("../../assets/variations/twins/dc_triangle.rhai")),
    ("dc_ztransl", include_str!("../../assets/variations/twins/dc_ztransl.rhai")),
    ("deltaA", include_str!("../../assets/variations/twins/deltaA.rhai")),
    ("devil_warp", include_str!("../../assets/variations/twins/devil_warp.rhai")),
    ("diamond", include_str!("../../assets/variations/twins/diamond.rhai")),
    ("dinis_surface_wf", include_str!("../../assets/variations/twins/dinis_surface_wf.rhai")),
    ("disc", include_str!("../../assets/variations/twins/disc.rhai")),
    ("disc2", include_str!("../../assets/variations/twins/disc2.rhai")),
    ("disc3", include_str!("../../assets/variations/twins/disc3.rhai")),
    ("disc3d", include_str!("../../assets/variations/twins/disc3d.rhai")),
    ("dustpoint", include_str!("../../assets/variations/twins/dustpoint.rhai")),
    ("eCollide", include_str!("../../assets/variations/twins/eCollide.rhai")),
    ("eJulia", include_str!("../../assets/variations/twins/eJulia.rhai")),
    ("eMod", include_str!("../../assets/variations/twins/eMod.rhai")),
    ("eMotion", include_str!("../../assets/variations/twins/eMotion.rhai")),
    ("ePush", include_str!("../../assets/variations/twins/ePush.rhai")),
    ("eRotate", include_str!("../../assets/variations/twins/eRotate.rhai")),
    ("eScale", include_str!("../../assets/variations/twins/eScale.rhai")),
    ("eSwirl", include_str!("../../assets/variations/twins/eSwirl.rhai")),
    ("eclipse", include_str!("../../assets/variations/twins/eclipse.rhai")),
    ("edisc", include_str!("../../assets/variations/twins/edisc.rhai")),
    ("electron_orbital", include_str!("../../assets/variations/twins/electron_orbital.rhai")),
    ("elliptic", include_str!("../../assets/variations/twins/elliptic.rhai")),
    ("elliptic2", include_str!("../../assets/variations/twins/elliptic2.rhai")),
    ("ennepers", include_str!("../../assets/variations/twins/ennepers.rhai")),
    ("ennepers2", include_str!("../../assets/variations/twins/ennepers2.rhai")),
    ("epispiral", include_str!("../../assets/variations/twins/epispiral.rhai")),
    ("epispiral_wf", include_str!("../../assets/variations/twins/epispiral_wf.rhai")),
    ("erf", include_str!("../../assets/variations/twins/erf.rhai")),
    ("erf3D", include_str!("../../assets/variations/twins/erf3D.rhai")),
    ("escher", include_str!("../../assets/variations/twins/escher.rhai")),
    ("estiq", include_str!("../../assets/variations/twins/estiq.rhai")),
    ("ex", include_str!("../../assets/variations/twins/ex.rhai")),
    ("exblur", include_str!("../../assets/variations/twins/exblur.rhai")),
    ("exp2", include_str!("../../assets/variations/twins/exp2.rhai")),
    ("exp2_bs", include_str!("../../assets/variations/twins/exp2_bs.rhai")),
    ("exponential", include_str!("../../assets/variations/twins/exponential.rhai")),
    ("extrude", include_str!("../../assets/variations/twins/extrude.rhai")),
    ("eyefish", include_str!("../../assets/variations/twins/eyefish.rhai")),
    ("falloff2", include_str!("../../assets/variations/twins/falloff2.rhai")),
    ("fan", include_str!("../../assets/variations/twins/fan.rhai")),
    ("fan2", include_str!("../../assets/variations/twins/fan2.rhai")),
    ("farblur", include_str!("../../assets/variations/twins/farblur.rhai")),
    ("fdisc", include_str!("../../assets/variations/twins/fdisc.rhai")),
    ("fibonacci2", include_str!("../../assets/variations/twins/fibonacci2.rhai")),
    ("fisheye", include_str!("../../assets/variations/twins/fisheye.rhai")),
    ("flatten", include_str!("../../assets/variations/twins/flatten.rhai")),
    ("flipy", include_str!("../../assets/variations/twins/flipy.rhai")),
    ("flower", include_str!("../../assets/variations/twins/flower.rhai")),
    ("flower_db", include_str!("../../assets/variations/twins/flower_db.rhai")),
    ("flux", include_str!("../../assets/variations/twins/flux.rhai")),
    ("foci", include_str!("../../assets/variations/twins/foci.rhai")),
    ("foci_3D", include_str!("../../assets/variations/twins/foci_3D.rhai")),
    ("fourth", include_str!("../../assets/variations/twins/fourth.rhai")),
    ("fract_dragon_wf", include_str!("../../assets/variations/twins/fract_dragon_wf.rhai")),
    ("fract_julia_wf", include_str!("../../assets/variations/twins/fract_julia_wf.rhai")),
    ("fract_mandelbrot_wf", include_str!("../../assets/variations/twins/fract_mandelbrot_wf.rhai")),
    ("fract_meteors_wf", include_str!("../../assets/variations/twins/fract_meteors_wf.rhai")),
    ("fract_pearls_wf", include_str!("../../assets/variations/twins/fract_pearls_wf.rhai")),
    ("fract_salamander_wf", include_str!("../../assets/variations/twins/fract_salamander_wf.rhai")),
    ("fuchsian_triangle", include_str!("../../assets/variations/twins/fuchsian_triangle.rhai")),
    ("funnel", include_str!("../../assets/variations/twins/funnel.rhai")),
    ("gamma", include_str!("../../assets/variations/twins/gamma.rhai")),
    ("gaussian_blur", include_str!("../../assets/variations/twins/gaussian_blur.rhai")),
    ("glsl_acrilic", include_str!("../../assets/variations/twins/glsl_acrilic.rhai")),
    ("glsl_apollonian", include_str!("../../assets/variations/twins/glsl_apollonian.rhai")),
    ("glsl_circlesblue", include_str!("../../assets/variations/twins/glsl_circlesblue.rhai")),
    ("glsl_circuits", include_str!("../../assets/variations/twins/glsl_circuits.rhai")),
    ("glsl_fractaldots", include_str!("../../assets/variations/twins/glsl_fractaldots.rhai")),
    ("glsl_grid3D", include_str!("../../assets/variations/twins/glsl_grid3D.rhai")),
    ("glsl_hoshi", include_str!("../../assets/variations/twins/glsl_hoshi.rhai")),
    ("glsl_hyperbolictile", include_str!("../../assets/variations/twins/glsl_hyperbolictile.rhai")),
    ("glsl_kaleidocomplex", include_str!("../../assets/variations/twins/glsl_kaleidocomplex.rhai")),
    ("glsl_kaleidoscopic", include_str!("../../assets/variations/twins/glsl_kaleidoscopic.rhai")),
    ("glsl_kaliset", include_str!("../../assets/variations/twins/glsl_kaliset.rhai")),
    ("glsl_kaliset2", include_str!("../../assets/variations/twins/glsl_kaliset2.rhai")),
    ("glsl_mandala", include_str!("../../assets/variations/twins/glsl_mandala.rhai")),
    ("glsl_mandelbox2D", include_str!("../../assets/variations/twins/glsl_mandelbox2D.rhai")),
    ("glsl_squares", include_str!("../../assets/variations/twins/glsl_squares.rhai")),
    ("glsl_starsfield", include_str!("../../assets/variations/twins/glsl_starsfield.rhai")),
    ("glynnSShape", include_str!("../../assets/variations/twins/glynnSShape.rhai")),
    ("glynnSim1", include_str!("../../assets/variations/twins/glynnSim1.rhai")),
    ("glynnSim2", include_str!("../../assets/variations/twins/glynnSim2.rhai")),
    ("glynnSim3", include_str!("../../assets/variations/twins/glynnSim3.rhai")),
    ("glynnia", include_str!("../../assets/variations/twins/glynnia.rhai")),
    ("glynnia3", include_str!("../../assets/variations/twins/glynnia3.rhai")),
    ("glynnlissa", include_str!("../../assets/variations/twins/glynnlissa.rhai")),
    ("glynnspiro", include_str!("../../assets/variations/twins/glynnspiro.rhai")),
    ("gridout", include_str!("../../assets/variations/twins/gridout.rhai")),
    ("gridout2", include_str!("../../assets/variations/twins/gridout2.rhai")),
    ("gridout3D", include_str!("../../assets/variations/twins/gridout3D.rhai")),
    ("hadamard_js", include_str!("../../assets/variations/twins/hadamard_js.rhai")),
    ("handkerchief", include_str!("../../assets/variations/twins/handkerchief.rhai")),
    ("harmonograph_js", include_str!("../../assets/variations/twins/harmonograph_js.rhai")),
    ("heart", include_str!("../../assets/variations/twins/heart.rhai")),
    ("heart_wf", include_str!("../../assets/variations/twins/heart_wf.rhai")),
    ("hecke_group", include_str!("../../assets/variations/twins/hecke_group.rhai")),
    ("helicoid", include_str!("../../assets/variations/twins/helicoid.rhai")),
    ("helix", include_str!("../../assets/variations/twins/helix.rhai")),
    ("hemisphere", include_str!("../../assets/variations/twins/hemisphere.rhai")),
    ("henon", include_str!("../../assets/variations/twins/henon.rhai")),
    ("hexaplay3D", include_str!("../../assets/variations/twins/hexaplay3D.rhai")),
    ("hexnix3D", include_str!("../../assets/variations/twins/hexnix3D.rhai")),
    ("ho", include_str!("../../assets/variations/twins/ho.rhai")),
    ("hofstadter", include_str!("../../assets/variations/twins/hofstadter.rhai")),
    ("hofstadter3d", include_str!("../../assets/variations/twins/hofstadter3d.rhai")),
    ("hole2", include_str!("../../assets/variations/twins/hole2.rhai")),
    ("holesq", include_str!("../../assets/variations/twins/holesq.rhai")),
    ("honeycomb", include_str!("../../assets/variations/twins/honeycomb.rhai")),
    ("honeycomb4d", include_str!("../../assets/variations/twins/honeycomb4d.rhai")),
    ("horseshoe", include_str!("../../assets/variations/twins/horseshoe.rhai")),
    ("hyperbolic_camera", include_str!("../../assets/variations/twins/hyperbolic_camera.rhai")),
    ("hyperbolicellipse", include_str!("../../assets/variations/twins/hyperbolicellipse.rhai")),
    ("hypercrop", include_str!("../../assets/variations/twins/hypercrop.rhai")),
    ("hypershift", include_str!("../../assets/variations/twins/hypershift.rhai")),
    ("hypershift2", include_str!("../../assets/variations/twins/hypershift2.rhai")),
    ("hypertile", include_str!("../../assets/variations/twins/hypertile.rhai")),
    ("hypertile1", include_str!("../../assets/variations/twins/hypertile1.rhai")),
    ("hypertile2", include_str!("../../assets/variations/twins/hypertile2.rhai")),
    ("hypertile3D", include_str!("../../assets/variations/twins/hypertile3D.rhai")),
    ("hypertile3D1", include_str!("../../assets/variations/twins/hypertile3D1.rhai")),
    ("hypertile3D2", include_str!("../../assets/variations/twins/hypertile3D2.rhai")),
    ("iconattractor_js", include_str!("../../assets/variations/twins/iconattractor_js.rhai")),
    ("idisc", include_str!("../../assets/variations/twins/idisc.rhai")),
    ("inflateZ_1", include_str!("../../assets/variations/twins/inflateZ_1.rhai")),
    ("inflateZ_2", include_str!("../../assets/variations/twins/inflateZ_2.rhai")),
    ("inflateZ_3", include_str!("../../assets/variations/twins/inflateZ_3.rhai")),
    ("inflateZ_4", include_str!("../../assets/variations/twins/inflateZ_4.rhai")),
    ("inflateZ_5", include_str!("../../assets/variations/twins/inflateZ_5.rhai")),
    ("inflateZ_6", include_str!("../../assets/variations/twins/inflateZ_6.rhai")),
    ("intersection", include_str!("../../assets/variations/twins/intersection.rhai")),
    ("inverted_julia", include_str!("../../assets/variations/twins/inverted_julia.rhai")),
    ("invpolar", include_str!("../../assets/variations/twins/invpolar.rhai")),
    ("invsquircular", include_str!("../../assets/variations/twins/invsquircular.rhai")),
    ("invtree_js", include_str!("../../assets/variations/twins/invtree_js.rhai")),
    ("jac_asn", include_str!("../../assets/variations/twins/jac_asn.rhai")),
    ("jac_cn", include_str!("../../assets/variations/twins/jac_cn.rhai")),
    ("jac_dn", include_str!("../../assets/variations/twins/jac_dn.rhai")),
    ("jac_sn", include_str!("../../assets/variations/twins/jac_sn.rhai")),
    ("jacobian_counterexample", include_str!("../../assets/variations/twins/jacobian_counterexample.rhai")),
    ("jubiQ", include_str!("../../assets/variations/twins/jubiQ.rhai")),
    ("jubiq4d", include_str!("../../assets/variations/twins/jubiq4d.rhai")),
    ("julia3D", include_str!("../../assets/variations/twins/julia3D.rhai")),
    ("julia3Dq", include_str!("../../assets/variations/twins/julia3Dq.rhai")),
    ("julia3Dz", include_str!("../../assets/variations/twins/julia3Dz.rhai")),
    ("juliac", include_str!("../../assets/variations/twins/juliac.rhai")),
    ("julian", include_str!("../../assets/variations/twins/julian.rhai")),
    ("julian2", include_str!("../../assets/variations/twins/julian2.rhai")),
    ("julian3Dx", include_str!("../../assets/variations/twins/julian3Dx.rhai")),
    ("juliaq", include_str!("../../assets/variations/twins/juliaq.rhai")),
    ("juliascope", include_str!("../../assets/variations/twins/juliascope.rhai")),
    ("juliascope3Db", include_str!("../../assets/variations/twins/juliascope3Db.rhai")),
    ("kaleidoscope", include_str!("../../assets/variations/twins/kaleidoscope.rhai")),
    ("klein_group", include_str!("../../assets/variations/twins/klein_group.rhai")),
    ("lace_js", include_str!("../../assets/variations/twins/lace_js.rhai")),
    ("layered_spiral", include_str!("../../assets/variations/twins/layered_spiral.rhai")),
    ("lazyTravis", include_str!("../../assets/variations/twins/lazyTravis.rhai")),
    ("lazyjess", include_str!("../../assets/variations/twins/lazyjess.rhai")),
    ("lazysensen", include_str!("../../assets/variations/twins/lazysensen.rhai")),
    ("lazysusan", include_str!("../../assets/variations/twins/lazysusan.rhai")),
    ("line", include_str!("../../assets/variations/twins/line.rhai")),
    ("linear", include_str!("../../assets/variations/twins/linear.rhai")),
    ("linear3D", include_str!("../../assets/variations/twins/linear3D.rhai")),
    ("linearT", include_str!("../../assets/variations/twins/linearT.rhai")),
    ("linearT3D", include_str!("../../assets/variations/twins/linearT3D.rhai")),
    ("lissajous", include_str!("../../assets/variations/twins/lissajous.rhai")),
    ("littlewood", include_str!("../../assets/variations/twins/littlewood.rhai")),
    ("log", include_str!("../../assets/variations/twins/log.rhai")),
    ("log_db", include_str!("../../assets/variations/twins/log_db.rhai")),
    ("log_tile2", include_str!("../../assets/variations/twins/log_tile2.rhai")),
    ("loonie", include_str!("../../assets/variations/twins/loonie.rhai")),
    ("loonie2", include_str!("../../assets/variations/twins/loonie2.rhai")),
    ("loonie3", include_str!("../../assets/variations/twins/loonie3.rhai")),
    ("loonie_3D", include_str!("../../assets/variations/twins/loonie_3D.rhai")),
    ("loq", include_str!("../../assets/variations/twins/loq.rhai")),
    ("lorentz_mobius", include_str!("../../assets/variations/twins/lorentz_mobius.rhai")),
    ("lorenz_js", include_str!("../../assets/variations/twins/lorenz_js.rhai")),
    ("lozi", include_str!("../../assets/variations/twins/lozi.rhai")),
    ("lsystem_path", include_str!("../../assets/variations/twins/lsystem_path.rhai")),
    ("lsystem_path_3D", include_str!("../../assets/variations/twins/lsystem_path_3D.rhai")),
    ("lsystem_tree", include_str!("../../assets/variations/twins/lsystem_tree.rhai")),
    ("macmillan", include_str!("../../assets/variations/twins/macmillan.rhai")),
    ("mandelbrot", include_str!("../../assets/variations/twins/mandelbrot.rhai")),
    ("mask", include_str!("../../assets/variations/twins/mask.rhai")),
    ("matrix3D", include_str!("../../assets/variations/twins/matrix3D.rhai")),
    ("maurer_rose", include_str!("../../assets/variations/twins/maurer_rose.rhai")),
    ("mcarpet", include_str!("../../assets/variations/twins/mcarpet.rhai")),
    ("menger", include_str!("../../assets/variations/twins/menger.rhai")),
    ("minkQM", include_str!("../../assets/variations/twins/minkQM.rhai")),
    ("minkowski", include_str!("../../assets/variations/twins/minkowski.rhai")),
    ("minkowski_camera", include_str!("../../assets/variations/twins/minkowski_camera.rhai")),
    ("minkowskope", include_str!("../../assets/variations/twins/minkowskope.rhai")),
    ("mobiq", include_str!("../../assets/variations/twins/mobiq.rhai")),
    ("mobiq4d", include_str!("../../assets/variations/twins/mobiq4d.rhai")),
    ("mobius", include_str!("../../assets/variations/twins/mobius.rhai")),
    ("mobiusN", include_str!("../../assets/variations/twins/mobiusN.rhai")),
    ("mobius_dragon_3D", include_str!("../../assets/variations/twins/mobius_dragon_3D.rhai")),
    ("mobius_strip", include_str!("../../assets/variations/twins/mobius_strip.rhai")),
    ("modulus", include_str!("../../assets/variations/twins/modulus.rhai")),
    ("multi_kaleidoscope", include_str!("../../assets/variations/twins/multi_kaleidoscope.rhai")),
    ("murl", include_str!("../../assets/variations/twins/murl.rhai")),
    ("murl2", include_str!("../../assets/variations/twins/murl2.rhai")),
    ("ngon", include_str!("../../assets/variations/twins/ngon.rhai")),
    ("noise", include_str!("../../assets/variations/twins/noise.rhai")),
    ("npolar", include_str!("../../assets/variations/twins/npolar.rhai")),
    ("octagon", include_str!("../../assets/variations/twins/octagon.rhai")),
    ("octapol", include_str!("../../assets/variations/twins/octapol.rhai")),
    ("onion", include_str!("../../assets/variations/twins/onion.rhai")),
    ("onion2", include_str!("../../assets/variations/twins/onion2.rhai")),
    ("ortho", include_str!("../../assets/variations/twins/ortho.rhai")),
    ("oscilloscope", include_str!("../../assets/variations/twins/oscilloscope.rhai")),
    ("oscilloscope2", include_str!("../../assets/variations/twins/oscilloscope2.rhai")),
    ("ovoid3d", include_str!("../../assets/variations/twins/ovoid3d.rhai")),
    ("pRose3D", include_str!("../../assets/variations/twins/pRose3D.rhai")),
    ("pTransform", include_str!("../../assets/variations/twins/pTransform.rhai")),
    ("panorama1", include_str!("../../assets/variations/twins/panorama1.rhai")),
    ("panorama2", include_str!("../../assets/variations/twins/panorama2.rhai")),
    ("parabola", include_str!("../../assets/variations/twins/parabola.rhai")),
    ("parplot2d_wf", include_str!("../../assets/variations/twins/parplot2d_wf.rhai")),
    ("pdj", include_str!("../../assets/variations/twins/pdj.rhai")),
    ("perspective", include_str!("../../assets/variations/twins/perspective.rhai")),
    ("petal", include_str!("../../assets/variations/twins/petal.rhai")),
    ("phoenix_julia", include_str!("../../assets/variations/twins/phoenix_julia.rhai")),
    ("pie", include_str!("../../assets/variations/twins/pie.rhai")),
    ("pie3D", include_str!("../../assets/variations/twins/pie3D.rhai")),
    ("plane_wf", include_str!("../../assets/variations/twins/plane_wf.rhai")),
    ("plusrecip", include_str!("../../assets/variations/twins/plusrecip.rhai")),
    ("poincare3D", include_str!("../../assets/variations/twins/poincare3D.rhai")),
    ("pointgrid3d_wf", include_str!("../../assets/variations/twins/pointgrid3d_wf.rhai")),
    ("pointgrid_wf", include_str!("../../assets/variations/twins/pointgrid_wf.rhai")),
    ("polar", include_str!("../../assets/variations/twins/polar.rhai")),
    ("polar2", include_str!("../../assets/variations/twins/polar2.rhai")),
    ("polarplot2d_wf", include_str!("../../assets/variations/twins/polarplot2d_wf.rhai")),
    ("polarplot3d_wf", include_str!("../../assets/variations/twins/polarplot3d_wf.rhai")),
    ("polychoron", include_str!("../../assets/variations/twins/polychoron.rhai")),
    ("polyhedron", include_str!("../../assets/variations/twins/polyhedron.rhai")),
    ("popcorn", include_str!("../../assets/variations/twins/popcorn.rhai")),
    ("popcorn2", include_str!("../../assets/variations/twins/popcorn2.rhai")),
    ("popcorn2_3D", include_str!("../../assets/variations/twins/popcorn2_3D.rhai")),
    ("post_axis_symmetry_wf", include_str!("../../assets/variations/twins/post_axis_symmetry_wf.rhai")),
    ("post_bwraps", include_str!("../../assets/variations/twins/post_bwraps.rhai")),
    ("post_bwraps2", include_str!("../../assets/variations/twins/post_bwraps2.rhai")),
    ("post_circlecrop", include_str!("../../assets/variations/twins/post_circlecrop.rhai")),
    ("post_colorscale_wf", include_str!("../../assets/variations/twins/post_colorscale_wf.rhai")),
    ("post_crop", include_str!("../../assets/variations/twins/post_crop.rhai")),
    ("post_curl", include_str!("../../assets/variations/twins/post_curl.rhai")),
    ("post_curl3D", include_str!("../../assets/variations/twins/post_curl3D.rhai")),
    ("post_falloff2", include_str!("../../assets/variations/twins/post_falloff2.rhai")),
    ("post_heat", include_str!("../../assets/variations/twins/post_heat.rhai")),
    ("post_julia3Dq", include_str!("../../assets/variations/twins/post_julia3Dq.rhai")),
    ("post_juliaq", include_str!("../../assets/variations/twins/post_juliaq.rhai")),
    ("post_mirror_wf", include_str!("../../assets/variations/twins/post_mirror_wf.rhai")),
    ("post_point_symmetry_wf", include_str!("../../assets/variations/twins/post_point_symmetry_wf.rhai")),
    ("post_rblur", include_str!("../../assets/variations/twins/post_rblur.rhai")),
    ("post_rotate_x", include_str!("../../assets/variations/twins/post_rotate_x.rhai")),
    ("post_rotate_y", include_str!("../../assets/variations/twins/post_rotate_y.rhai")),
    ("post_spherical", include_str!("../../assets/variations/twins/post_spherical.rhai")),
    ("post_spin_z", include_str!("../../assets/variations/twins/post_spin_z.rhai")),
    ("post_ztranslate_wf", include_str!("../../assets/variations/twins/post_ztranslate_wf.rhai")),
    ("pow_block", include_str!("../../assets/variations/twins/pow_block.rhai")),
    ("power", include_str!("../../assets/variations/twins/power.rhai")),
    ("pre_blur", include_str!("../../assets/variations/twins/pre_blur.rhai")),
    ("pre_blur3D", include_str!("../../assets/variations/twins/pre_blur3D.rhai")),
    ("pre_boarders2", include_str!("../../assets/variations/twins/pre_boarders2.rhai")),
    ("pre_bwraps", include_str!("../../assets/variations/twins/pre_bwraps.rhai")),
    ("pre_bwraps2", include_str!("../../assets/variations/twins/pre_bwraps2.rhai")),
    ("pre_circlecrop", include_str!("../../assets/variations/twins/pre_circlecrop.rhai")),
    ("pre_crop", include_str!("../../assets/variations/twins/pre_crop.rhai")),
    ("pre_curl", include_str!("../../assets/variations/twins/pre_curl.rhai")),
    ("pre_dcztransl", include_str!("../../assets/variations/twins/pre_dcztransl.rhai")),
    ("pre_disc", include_str!("../../assets/variations/twins/pre_disc.rhai")),
    ("pre_disc3d", include_str!("../../assets/variations/twins/pre_disc3d.rhai")),
    ("pre_falloff2", include_str!("../../assets/variations/twins/pre_falloff2.rhai")),
    ("pre_rect_wf", include_str!("../../assets/variations/twins/pre_rect_wf.rhai")),
    ("pre_rotate_x", include_str!("../../assets/variations/twins/pre_rotate_x.rhai")),
    ("pre_rotate_y", include_str!("../../assets/variations/twins/pre_rotate_y.rhai")),
    ("pre_sinusoidal", include_str!("../../assets/variations/twins/pre_sinusoidal.rhai")),
    ("pre_sinusoidal3d", include_str!("../../assets/variations/twins/pre_sinusoidal3d.rhai")),
    ("pre_spherical", include_str!("../../assets/variations/twins/pre_spherical.rhai")),
    ("pre_spin_z", include_str!("../../assets/variations/twins/pre_spin_z.rhai")),
    ("pre_wave3D_wf", include_str!("../../assets/variations/twins/pre_wave3D_wf.rhai")),
    ("pre_zscale", include_str!("../../assets/variations/twins/pre_zscale.rhai")),
    ("pre_ztranslate", include_str!("../../assets/variations/twins/pre_ztranslate.rhai")),
    ("pressure_wave", include_str!("../../assets/variations/twins/pressure_wave.rhai")),
    ("projective", include_str!("../../assets/variations/twins/projective.rhai")),
    ("pulse", include_str!("../../assets/variations/twins/pulse.rhai")),
    ("pyramid", include_str!("../../assets/variations/twins/pyramid.rhai")),
    ("q_ode", include_str!("../../assets/variations/twins/q_ode.rhai")),
    ("quasiconformal", include_str!("../../assets/variations/twins/quasiconformal.rhai")),
    ("quaternion", include_str!("../../assets/variations/twins/quaternion.rhai")),
    ("quaternion_camera", include_str!("../../assets/variations/twins/quaternion_camera.rhai")),
    ("quaternion_julia", include_str!("../../assets/variations/twins/quaternion_julia.rhai")),
    ("quaternion_julia_set", include_str!("../../assets/variations/twins/quaternion_julia_set.rhai")),
    ("quaternion_linear", include_str!("../../assets/variations/twins/quaternion_linear.rhai")),
    ("quaternion_rotation", include_str!("../../assets/variations/twins/quaternion_rotation.rhai")),
    ("r_circleblur", include_str!("../../assets/variations/twins/r_circleblur.rhai")),
    ("radial_blur", include_str!("../../assets/variations/twins/radial_blur.rhai")),
    ("rational3", include_str!("../../assets/variations/twins/rational3.rhai")),
    ("rays", include_str!("../../assets/variations/twins/rays.rhai")),
    ("rays1", include_str!("../../assets/variations/twins/rays1.rhai")),
    ("rays2", include_str!("../../assets/variations/twins/rays2.rhai")),
    ("rays3", include_str!("../../assets/variations/twins/rays3.rhai")),
    ("rectangles", include_str!("../../assets/variations/twins/rectangles.rhai")),
    ("rhodonea", include_str!("../../assets/variations/twins/rhodonea.rhai")),
    ("rings", include_str!("../../assets/variations/twins/rings.rhai")),
    ("ripple", include_str!("../../assets/variations/twins/ripple.rhai")),
    ("rippled", include_str!("../../assets/variations/twins/rippled.rhai")),
    ("rose_wf", include_str!("../../assets/variations/twins/rose_wf.rhai")),
    ("rosoni", include_str!("../../assets/variations/twins/rosoni.rhai")),
    ("roundspher", include_str!("../../assets/variations/twins/roundspher.rhai")),
    ("roundspher3D", include_str!("../../assets/variations/twins/roundspher3D.rhai")),
    ("sattractor_js", include_str!("../../assets/variations/twins/sattractor_js.rhai")),
    ("schottky_group", include_str!("../../assets/variations/twins/schottky_group.rhai")),
    ("scry", include_str!("../../assets/variations/twins/scry.rhai")),
    ("scry2", include_str!("../../assets/variations/twins/scry2.rhai")),
    ("scry_3D", include_str!("../../assets/variations/twins/scry_3D.rhai")),
    ("seashell3D", include_str!("../../assets/variations/twins/seashell3D.rhai")),
    ("sec", include_str!("../../assets/variations/twins/sec.rhai")),
    ("sec2_bs", include_str!("../../assets/variations/twins/sec2_bs.rhai")),
    ("secant2", include_str!("../../assets/variations/twins/secant2.rhai")),
    ("sech", include_str!("../../assets/variations/twins/sech.rhai")),
    ("sech2_bs", include_str!("../../assets/variations/twins/sech2_bs.rhai")),
    ("sechq", include_str!("../../assets/variations/twins/sechq.rhai")),
    ("secq", include_str!("../../assets/variations/twins/secq.rhai")),
    ("separation", include_str!("../../assets/variations/twins/separation.rhai")),
    ("shift", include_str!("../../assets/variations/twins/shift.rhai")),
    ("shredlin", include_str!("../../assets/variations/twins/shredlin.rhai")),
    ("shredrad", include_str!("../../assets/variations/twins/shredrad.rhai")),
    ("siercarpet_js", include_str!("../../assets/variations/twins/siercarpet_js.rhai")),
    ("sigmoid", include_str!("../../assets/variations/twins/sigmoid.rhai")),
    ("sin", include_str!("../../assets/variations/twins/sin.rhai")),
    ("sin2_bs", include_str!("../../assets/variations/twins/sin2_bs.rhai")),
    ("sineblur", include_str!("../../assets/variations/twins/sineblur.rhai")),
    ("sinh", include_str!("../../assets/variations/twins/sinh.rhai")),
    ("sinh2_bs", include_str!("../../assets/variations/twins/sinh2_bs.rhai")),
    ("sinhq", include_str!("../../assets/variations/twins/sinhq.rhai")),
    ("sinq", include_str!("../../assets/variations/twins/sinq.rhai")),
    ("sintrange", include_str!("../../assets/variations/twins/sintrange.rhai")),
    ("sinusoidal", include_str!("../../assets/variations/twins/sinusoidal.rhai")),
    ("solid_sphere", include_str!("../../assets/variations/twins/solid_sphere.rhai")),
    ("sph3D", include_str!("../../assets/variations/twins/sph3D.rhai")),
    ("sphere_nja", include_str!("../../assets/variations/twins/sphere_nja.rhai")),
    ("sphere_packing", include_str!("../../assets/variations/twins/sphere_packing.rhai")),
    ("spherecrop", include_str!("../../assets/variations/twins/spherecrop.rhai")),
    ("spherical", include_str!("../../assets/variations/twins/spherical.rhai")),
    ("spherical3D", include_str!("../../assets/variations/twins/spherical3D.rhai")),
    ("spherical3D_wf", include_str!("../../assets/variations/twins/spherical3D_wf.rhai")),
    ("sphericalN", include_str!("../../assets/variations/twins/sphericalN.rhai")),
    ("spiral", include_str!("../../assets/variations/twins/spiral.rhai")),
    ("spiralwing", include_str!("../../assets/variations/twins/spiralwing.rhai")),
    ("spirograph", include_str!("../../assets/variations/twins/spirograph.rhai")),
    ("spirograph3D", include_str!("../../assets/variations/twins/spirograph3D.rhai")),
    ("spligon", include_str!("../../assets/variations/twins/spligon.rhai")),
    ("spliptic_bs", include_str!("../../assets/variations/twins/spliptic_bs.rhai")),
    ("split", include_str!("../../assets/variations/twins/split.rhai")),
    ("splitbrdr", include_str!("../../assets/variations/twins/splitbrdr.rhai")),
    ("splits", include_str!("../../assets/variations/twins/splits.rhai")),
    ("splits3D", include_str!("../../assets/variations/twins/splits3D.rhai")),
    ("spray_blur", include_str!("../../assets/variations/twins/spray_blur.rhai")),
    ("sqrt_acosech", include_str!("../../assets/variations/twins/sqrt_acosech.rhai")),
    ("sqrt_acosh", include_str!("../../assets/variations/twins/sqrt_acosh.rhai")),
    ("sqrt_acoth", include_str!("../../assets/variations/twins/sqrt_acoth.rhai")),
    ("sqrt_asech", include_str!("../../assets/variations/twins/sqrt_asech.rhai")),
    ("sqrt_asinh", include_str!("../../assets/variations/twins/sqrt_asinh.rhai")),
    ("sqrt_atanh", include_str!("../../assets/variations/twins/sqrt_atanh.rhai")),
    ("square", include_str!("../../assets/variations/twins/square.rhai")),
    ("square3D", include_str!("../../assets/variations/twins/square3D.rhai")),
    ("squarize", include_str!("../../assets/variations/twins/squarize.rhai")),
    ("squircular", include_str!("../../assets/variations/twins/squircular.rhai")),
    ("squirrel", include_str!("../../assets/variations/twins/squirrel.rhai")),
    ("squish", include_str!("../../assets/variations/twins/squish.rhai")),
    ("starblur", include_str!("../../assets/variations/twins/starblur.rhai")),
    ("stereogram", include_str!("../../assets/variations/twins/stereogram.rhai")),
    ("stripes", include_str!("../../assets/variations/twins/stripes.rhai")),
    ("stripfit", include_str!("../../assets/variations/twins/stripfit.rhai")),
    ("stwin", include_str!("../../assets/variations/twins/stwin.rhai")),
    ("su_custom", include_str!("../../assets/variations/twins/su_custom.rhai")),
    ("su_mobius", include_str!("../../assets/variations/twins/su_mobius.rhai")),
    ("sunflower", include_str!("../../assets/variations/twins/sunflower.rhai")),
    ("superShape3d", include_str!("../../assets/variations/twins/superShape3d.rhai")),
    ("super_shape", include_str!("../../assets/variations/twins/super_shape.rhai")),
    ("surface_group", include_str!("../../assets/variations/twins/surface_group.rhai")),
    ("svensson_js", include_str!("../../assets/variations/twins/svensson_js.rhai")),
    ("svf", include_str!("../../assets/variations/twins/svf.rhai")),
    ("swirl", include_str!("../../assets/variations/twins/swirl.rhai")),
    ("swirl3", include_str!("../../assets/variations/twins/swirl3.rhai")),
    ("swirl3D_wf", include_str!("../../assets/variations/twins/swirl3D_wf.rhai")),
    ("sym_bg1", include_str!("../../assets/variations/twins/sym_bg1.rhai")),
    ("sym_bg2", include_str!("../../assets/variations/twins/sym_bg2.rhai")),
    ("sym_bg3", include_str!("../../assets/variations/twins/sym_bg3.rhai")),
    ("sym_bg4", include_str!("../../assets/variations/twins/sym_bg4.rhai")),
    ("sym_bg5", include_str!("../../assets/variations/twins/sym_bg5.rhai")),
    ("sym_bg6", include_str!("../../assets/variations/twins/sym_bg6.rhai")),
    ("sym_bg7", include_str!("../../assets/variations/twins/sym_bg7.rhai")),
    ("sym_ng1", include_str!("../../assets/variations/twins/sym_ng1.rhai")),
    ("sym_ng10", include_str!("../../assets/variations/twins/sym_ng10.rhai")),
    ("sym_ng11", include_str!("../../assets/variations/twins/sym_ng11.rhai")),
    ("sym_ng12", include_str!("../../assets/variations/twins/sym_ng12.rhai")),
    ("sym_ng13", include_str!("../../assets/variations/twins/sym_ng13.rhai")),
    ("sym_ng14", include_str!("../../assets/variations/twins/sym_ng14.rhai")),
    ("sym_ng15", include_str!("../../assets/variations/twins/sym_ng15.rhai")),
    ("sym_ng16", include_str!("../../assets/variations/twins/sym_ng16.rhai")),
    ("sym_ng17", include_str!("../../assets/variations/twins/sym_ng17.rhai")),
    ("sym_ng2", include_str!("../../assets/variations/twins/sym_ng2.rhai")),
    ("sym_ng3", include_str!("../../assets/variations/twins/sym_ng3.rhai")),
    ("sym_ng4", include_str!("../../assets/variations/twins/sym_ng4.rhai")),
    ("sym_ng5", include_str!("../../assets/variations/twins/sym_ng5.rhai")),
    ("sym_ng6", include_str!("../../assets/variations/twins/sym_ng6.rhai")),
    ("sym_ng7", include_str!("../../assets/variations/twins/sym_ng7.rhai")),
    ("sym_ng8", include_str!("../../assets/variations/twins/sym_ng8.rhai")),
    ("sym_ng9", include_str!("../../assets/variations/twins/sym_ng9.rhai")),
    ("synth", include_str!("../../assets/variations/twins/synth.rhai")),
    ("szubieta", include_str!("../../assets/variations/twins/szubieta.rhai")),
    ("tan", include_str!("../../assets/variations/twins/tan.rhai")),
    ("tan2_bs", include_str!("../../assets/variations/twins/tan2_bs.rhai")),
    ("tancos", include_str!("../../assets/variations/twins/tancos.rhai")),
    ("tangent", include_str!("../../assets/variations/twins/tangent.rhai")),
    ("tangent3D", include_str!("../../assets/variations/twins/tangent3D.rhai")),
    ("tanh", include_str!("../../assets/variations/twins/tanh.rhai")),
    ("tanh2_bs", include_str!("../../assets/variations/twins/tanh2_bs.rhai")),
    ("tanhq", include_str!("../../assets/variations/twins/tanhq.rhai")),
    ("tanq", include_str!("../../assets/variations/twins/tanq.rhai")),
    ("target", include_str!("../../assets/variations/twins/target.rhai")),
    ("target_sp", include_str!("../../assets/variations/twins/target_sp.rhai")),
    ("taurus", include_str!("../../assets/variations/twins/taurus.rhai")),
    ("threepoint_js", include_str!("../../assets/variations/twins/threepoint_js.rhai")),
    ("tile_hlp", include_str!("../../assets/variations/twins/tile_hlp.rhai")),
    ("tile_log", include_str!("../../assets/variations/twins/tile_log.rhai")),
    ("tile_reverse", include_str!("../../assets/variations/twins/tile_reverse.rhai")),
    ("tqmirror", include_str!("../../assets/variations/twins/tqmirror.rhai")),
    ("trade", include_str!("../../assets/variations/twins/trade.rhai")),
    ("truchet", include_str!("../../assets/variations/twins/truchet.rhai")),
    ("truchet2", include_str!("../../assets/variations/twins/truchet2.rhai")),
    ("truchet_fill", include_str!("../../assets/variations/twins/truchet_fill.rhai")),
    ("twintrian", include_str!("../../assets/variations/twins/twintrian.rhai")),
    ("twoface", include_str!("../../assets/variations/twins/twoface.rhai")),
    ("unpolar", include_str!("../../assets/variations/twins/unpolar.rhai")),
    ("vibration2", include_str!("../../assets/variations/twins/vibration2.rhai")),
    ("vogel", include_str!("../../assets/variations/twins/vogel.rhai")),
    ("von_dyck", include_str!("../../assets/variations/twins/von_dyck.rhai")),
    ("voron", include_str!("../../assets/variations/twins/voron.rhai")),
    ("w", include_str!("../../assets/variations/twins/w.rhai")),
    ("waffle", include_str!("../../assets/variations/twins/waffle.rhai")),
    ("wallpaper_js", include_str!("../../assets/variations/twins/wallpaper_js.rhai")),
    ("waveblur_wf", include_str!("../../assets/variations/twins/waveblur_wf.rhai")),
    ("waves", include_str!("../../assets/variations/twins/waves.rhai")),
    ("waves2", include_str!("../../assets/variations/twins/waves2.rhai")),
    ("waves2_3D", include_str!("../../assets/variations/twins/waves2_3D.rhai")),
    ("waves2_radial", include_str!("../../assets/variations/twins/waves2_radial.rhai")),
    ("waves2_wf", include_str!("../../assets/variations/twins/waves2_wf.rhai")),
    ("waves2b", include_str!("../../assets/variations/twins/waves2b.rhai")),
    ("waves3_wf", include_str!("../../assets/variations/twins/waves3_wf.rhai")),
    ("waves4_wf", include_str!("../../assets/variations/twins/waves4_wf.rhai")),
    ("wdisc", include_str!("../../assets/variations/twins/wdisc.rhai")),
    ("wedge", include_str!("../../assets/variations/twins/wedge.rhai")),
    ("wedge_julia", include_str!("../../assets/variations/twins/wedge_julia.rhai")),
    ("wedge_sph", include_str!("../../assets/variations/twins/wedge_sph.rhai")),
    ("whitney_umbrella", include_str!("../../assets/variations/twins/whitney_umbrella.rhai")),
    ("whorl", include_str!("../../assets/variations/twins/whorl.rhai")),
    ("woggle_js", include_str!("../../assets/variations/twins/woggle_js.rhai")),
    ("xerf", include_str!("../../assets/variations/twins/xerf.rhai")),
    ("xheart", include_str!("../../assets/variations/twins/xheart.rhai")),
    ("xheart_blur_wf", include_str!("../../assets/variations/twins/xheart_blur_wf.rhai")),
    ("xtrb", include_str!("../../assets/variations/twins/xtrb.rhai")),
    ("yin_yang", include_str!("../../assets/variations/twins/yin_yang.rhai")),
    ("yplot2d_wf", include_str!("../../assets/variations/twins/yplot2d_wf.rhai")),
    ("yplot3d_wf", include_str!("../../assets/variations/twins/yplot3d_wf.rhai")),
    ("z", include_str!("../../assets/variations/twins/z.rhai")),
    ("zblur", include_str!("../../assets/variations/twins/zblur.rhai")),
    ("zcone", include_str!("../../assets/variations/twins/zcone.rhai")),
    ("zscale", include_str!("../../assets/variations/twins/zscale.rhai")),
    ("ztranslate", include_str!("../../assets/variations/twins/ztranslate.rhai")),
];

/// The twin of the built-in variation `name`, when it has one.
pub fn source(name: &str) -> Option<&'static str> {
    TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The twin as a flame carries it (`Flame::custom_variations`).
pub fn carried(name: &str) -> Option<CustomSource> {
    super::custom::carried(source(name)?)
}

/// `flame` with every `name` the built-in's twin, which it carries: the
/// planners' gates (tests).
#[cfg(test)]
pub fn twin_flame(flame: &crate::scene::transforms::Flame, name: &str) -> crate::scene::transforms::Flame {
    let key = crate::variations::twins::key(name).expect("a twin");
    let mut f = flame.clone();
    for t in &mut f.transforms {
        if let Some(w) = t.variations.remove(name) {
            t.variations.insert(key.clone(), w);
        }
        for v in &mut t.variation_order {
            if v == name {
                *v = key.clone();
            }
        }
        let params: Vec<(String, f32)> = t.variation_params.drain().collect();
        for (k, v) in params {
            let k = match k.strip_prefix(&format!("{name}.")) {
                Some(rest) => format!("{key}.{rest}"),
                None => k,
            };
            t.variation_params.insert(k, v);
        }
    }
    f.custom_variations = vec![crate::variations::twins::carried(name).expect("a twin")];
    f
}

/// The twin's registry key: what a transform names it by.
pub fn key(name: &str) -> Option<String> {
    super::custom::key_of(source(name)?)
}

/// The built-in whose twin `source` is, UNEDITED: the twin's text, with
/// or without the id Edit as Rhai gives it (`lang::rhai::with_info_id`),
/// line ends aside. While it is, it computes its built-in, so it keeps
/// what is still a def's alone: the deep-zoom planner's data
/// ([`planner_name`]) and its name in a `.flame` export.
pub fn builtin_of(source: &str) -> Option<&'static str> {
    use std::hash::{Hash, Hasher};
    fn hash(text: &str) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        h.finish()
    }
    static BY_TEXT: std::sync::OnceLock<std::collections::HashMap<u64, &'static str>> = std::sync::OnceLock::new();
    let by_text = BY_TEXT.get_or_init(|| TWINS.iter().map(|(n, s)| (hash(&s.replace("\r\n", "\n")), *n)).collect());
    let text = source.replace("\r\n", "\n");
    let id = crate::lang::rhai::info_string(&text, "id");
    let bare = match &id {
        Some(id) => text.replacen(&format!(" id: \"{id}\","), "", 1),
        None => text.clone(),
    };
    let name = *by_text.get(&hash(&bare))?;
    // The id where Edit as Rhai puts it, and nothing else moved.
    let twin = self::source(name)?.replace("\r\n", "\n");
    let same = match &id {
        Some(id) => crate::lang::rhai::with_info_id(&twin, id) == text,
        None => twin == text,
    };
    same.then_some(name)
}

/// What a compiled variation computes, as text to compare: its shaders
/// with its own names (`own`: its key, its `INFO.name`) blanked, its
/// phase, features, state, derived slots, emissions, and its parameters'
/// names and kinds. Not its title, description, category, authors, nor
/// its parameters' defaults, ranges or captions: they change no maths.
fn maths_of(dl: &crate::api::types::VariationDownload, own: &[&str]) -> String {
    let blank = |s: &Option<String>| {
        let mut s = s.clone().unwrap_or_default();
        for n in own.iter().filter(|n| !n.is_empty()) {
            s = s.replace(n, "@");
        }
        s
    };
    let mut features = dl.features.clone();
    features.sort();
    let params: Vec<String> = dl.parameters.iter().map(|p| format!("{}:{:?}", p.name, p.param_type)).collect();
    format!(
        "{:?}|{features:?}|{}|{}|{}|{params:?}|{}|{}|{}|{}",
        dl.phase,
        dl.state_count,
        dl.init_param_count,
        dl.plot_emits,
        blank(&dl.shader_2d),
        blank(&dl.shader_3d),
        blank(&dl.shader_init),
        blank(&dl.shader_state_init)
    )
}

/// A source's functions, its text from the first `fn` with its spacing
/// collapsed: what a change of its `INFO` or `PARAMS` alone leaves as it
/// was.
fn functions_of(source: &str) -> String {
    let text = source.replace("\r\n", "\n");
    let start = if text.starts_with("fn ") { Some(0) } else { text.find("\nfn ").map(|i| i + 1) };
    start.map(|i| text[i..].split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default()
}

/// The built-in whose twin computes what `dl` computes, compiled from
/// `source`: the same maths ([`maths_of`]) however its title, `INFO.name`,
/// description or parameters' captions and defaults were changed. The twin
/// it is compared with is the one its `INFO.name` names (`rhai_<built-in>`)
/// or the one whose functions it has; each twin compiled once, when first
/// asked. Run outside the registry's lock: compiling may read it.
pub(crate) fn builtin_computed_by(source: &str, dl: &crate::api::types::VariationDownload) -> Option<&'static str> {
    use std::collections::HashMap;
    static BY_FUNCTIONS: std::sync::OnceLock<HashMap<String, &'static str>> = std::sync::OnceLock::new();
    static MATHS: std::sync::OnceLock<std::sync::Mutex<HashMap<&'static str, Option<String>>>> = std::sync::OnceLock::new();
    let by_functions = BY_FUNCTIONS.get_or_init(|| TWINS.iter().map(|(n, s)| (functions_of(s), *n)).collect());
    let own_name = crate::lang::rhai::variation_name(source).unwrap_or_default();
    let mut candidates: Vec<&'static str> = Vec::new();
    if let Some(&(n, _)) = own_name.strip_prefix("rhai_").and_then(|b| TWINS.iter().find(|(n, _)| *n == b)) {
        candidates.push(n);
    }
    if let Some(&n) = by_functions.get(&functions_of(source)) {
        if !candidates.contains(&n) {
            candidates.push(n);
        }
    }
    let mine = maths_of(dl, &[&dl.name, &own_name]);
    let maths = MATHS.get_or_init(Default::default);
    candidates.into_iter().find(|&b| {
        let cached = maths.lock().ok().and_then(|m| m.get(b).cloned());
        let theirs = cached.unwrap_or_else(|| {
            let twin = self::source(b)?;
            let entry = "uf_twin_maths";
            let name = crate::lang::rhai::variation_name(twin).unwrap_or_default();
            let computed = super::custom::compile_as(twin, entry).ok().map(|t| maths_of(&t, &[entry, &name]));
            if let Ok(mut m) = maths.lock() {
                m.insert(b, computed.clone());
            }
            computed
        });
        theirs.as_deref() == Some(mine.as_str())
    })
}

/// Note what registered under `key` by what it computes: an unedited
/// twin's built-in where [`note`]'s text did not find one but the maths is
/// the same (a title or a caption changed). After registration, outside
/// the registry's lock.
pub(crate) fn note_maths(key: &str, source: &str, dl: &crate::api::types::VariationDownload) {
    if builtin_of_key(key).is_some() {
        return;
    }
    if let Some(b) = builtin_computed_by(source, dl) {
        if let Ok(mut u) = unedited().write() {
            u.insert(key.to_string(), b);
        }
    }
}

/// The registered keys that are unedited twins, and of which built-in:
/// noted at registration ([`note`], [`note_maths`]).
fn unedited() -> &'static std::sync::RwLock<std::collections::HashMap<String, &'static str>> {
    static U: std::sync::OnceLock<std::sync::RwLock<std::collections::HashMap<String, &'static str>>> = std::sync::OnceLock::new();
    U.get_or_init(Default::default)
}

/// Note what registered under `key`: an unedited twin's built-in, or
/// nothing (`custom::register_payload`).
pub(crate) fn note(key: &str, source: &str) {
    let builtin = builtin_of(source);
    if let Ok(mut u) = unedited().write() {
        match builtin {
            Some(b) => u.insert(key.to_string(), b),
            None => u.remove(key),
        };
    }
}

/// The built-in the variation registered under `key` is an unedited twin
/// of. A built-in's name is never a key, so it is not looked up.
pub fn builtin_of_key(key: &str) -> Option<&'static str> {
    if !crate::scripted::is_key(key) {
        return None;
    }
    unedited().read().ok()?.get(key).copied()
}

#[cfg(test)]
thread_local! {
    /// Off: no twin borrows its built-in's planner data, so a test on
    /// this thread reaches a twin's own -- its Rhai inverse
    /// (`user_inverse`), its own derived bounds.
    pub(crate) static BORROW: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// The name the deep-zoom planner's tables know the variation `name` by
/// (`inverse`, `bound`, `derive`): an unedited twin's built-in's, as its
/// own; anything else its own. Not a many-valued built-in's: the replay
/// forces its arm in the built-in's WGSL text (`bound::force_arms`),
/// which a twin's is not, so a twin of one has no planner data.
pub fn planner_name(name: &str) -> &str {
    #[cfg(test)]
    if !BORROW.with(|b| b.get()) {
        return name;
    }
    match builtin_of_key(name) {
        Some(b) if super::bound::arms_for(b).is_none() => b,
        _ => name,
    }
}

/// A float as a Rhai literal that parses back to the same f32: the
/// shortest form, with the point Rhai wants.
fn float(v: f64) -> String {
    let s = format!("{:?}", v as f32);
    if s.contains('.') || s.contains("inf") || s.contains("NaN") {
        s
    } else if let Some(e) = s.find('e') {
        format!("{}.0{}", &s[..e], &s[e..])
    } else {
        format!("{s}.0")
    }
}

fn text(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A def's parameters as a twin's `PARAMS` map, each as the built-in
/// declares it: a free range `free: true`, a log slider `exponential:
/// true`, a range of five decades that is not one `exponential: false`,
/// an angle `type: "angle"`.
pub fn params_rhai(params: &[ParamDef]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::from("const PARAMS = #{\n");
    for p in params {
        let head = match &p.kind {
            ParamKind::Float { default, min, max, free, log, angle } => {
                let mut s = format!("value: {}, min: {}, max: {}", float(*default), float(*min), float(*max));
                if *free {
                    s += ", free: true";
                }
                if *log {
                    s += ", exponential: true";
                } else if *min > 0.0 && *max / *min >= 1e5 {
                    s += ", exponential: false";
                }
                if *angle {
                    s += ", type: \"angle\"";
                }
                s
            }
            ParamKind::Int { default, min, max, free } => {
                let mut s = format!("value: {default}, min: {min}, max: {max}");
                if *free {
                    s += ", free: true";
                }
                s
            }
            ParamKind::Bool { default } => format!("value: {default}"),
            ParamKind::Choice { default, options } => {
                let names: Vec<String> = options.iter().map(text).collect();
                format!("value: {default}, choices: [{}]", names.join(", "))
            }
            ParamKind::Text { .. } | ParamKind::Color { .. } => "value: 0.0".to_string(),
        };
        out += &format!("    {}: #{{\n        {head},\n        caption: {},\n", p.name, text(&p.label));
        if !p.hint.is_empty() {
            out += &format!("        hint: {},\n", text(&p.hint));
        }
        out += "    },\n";
    }
    out += "};\n\n";
    out
}

/// A twin's beginning, from the built-in `name`'s def: its doc block
/// (`block`, the `///` lines above the def, verbatim: its description and
/// authors, which the API reads as text), its `INFO` (its display name;
/// `rhai_` and its name lowercased, what `vary` calls it) and its
/// `PARAMS`, then the built-in's WGSL as a comment to port, and a body
/// that returns its point.
pub fn skeleton(name: &str, block: Option<&str>) -> Option<String> {
    let reg = super::global_registry();
    let info = reg.get(name)?;
    let mut out = block.map(|b| format!("{b}\n\n")).unwrap_or_default();
    out += &format!("// {}, the built-in variation's twin.\n\n", info.display_name);
    out += &params_rhai(&info.parameters);
    let phase = match info.phase {
        super::VariationPhase::Normal => String::new(),
        ref p => format!(", phase: \"{}\"", p.clone().to_api_str()),
    };
    // Its slow parameters, a slider's committed when the drag ends.
    let slow: Vec<String> = info.parameters.iter().filter(|d| d.slow).map(|d| format!("\"{}\"", d.name)).collect();
    let slow = if slow.is_empty() { String::new() } else { format!(", slow: [{}]", slow.join(", ")) };
    out += &format!(
        "const INFO = #{{ title: {}, name: \"rhai_{}\", category: \"{}\"{phase}{slow} }};\n\n",
        text(&info.display_name),
        name.to_ascii_lowercase(),
        info.category.to_api_str()
    );
    for (what, wgsl) in [("2D", info.wgsl_source.as_deref()), ("3D", info.wgsl_source_3d.as_deref()), ("init", info.wgsl_source_init.as_deref())] {
        if let Some(w) = wgsl.filter(|w| !w.trim().is_empty()) {
            out += &format!("// The built-in's {what} WGSL:\n");
            for line in w.trim().lines() {
                out += &format!("//   {line}\n");
            }
            out += "\n";
        }
    }
    out += "fn variation(z, p) {\n    z\n}\n";
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A twin as Edit as Rhai makes it: with an id of its own.
    fn own(name: &str) -> String {
        crate::lang::rhai::with_own_id(source(name).unwrap(), &crate::scripted::mint_id())
    }

    /// **An unedited twin is known as its built-in**: as shipped, with the
    /// id Edit as Rhai gives it, with Windows line ends -- and not once a
    /// character of it is changed.
    #[test]
    fn an_unedited_twin_is_known_as_its_builtin() {
        for (name, src) in TWINS {
            assert_eq!(builtin_of(src), Some(*name), "{name}: as shipped");
            assert_eq!(builtin_of(&own(name)), Some(*name), "{name}: with an id of its own");
        }
        // From LF line ends, as a checkout with `core.autocrlf` gives CRLF.
        let julia = own("julia").replace("\r\n", "\n");
        assert_eq!(builtin_of(&julia.replace('\n', "\r\n")), Some("julia"), "line ends aside");
        assert_eq!(builtin_of(&format!("{julia}// mine now\n")), None, "a comment added is an edit");
        assert_eq!(builtin_of(&julia.replacen("0.5", "0.25", 1)), None, "so is a number changed");
        assert_eq!(builtin_of(&julia.replacen("const INFO = #{ id:", "const INFO = #{ id :", 1)), None, "or the id moved");
    }

    /// **An unedited twin keeps its built-in's deep-zoom planner data**:
    /// its key finds the built-in's inverse, bound and interval body
    /// (`twins::planner_name`); an edited twin's finds none.
    #[test]
    fn an_unedited_twin_keeps_its_builtins_planner_data() {
        use crate::variations::{bound, derive, inverse};
        let registered = |src: &str| {
            let key = super::super::custom::key_of(src).unwrap();
            note(&key, src);
            key
        };
        let (julia, bubble, blur) = (registered(&own("julia")), registered(&own("bubble")), registered(&own("blur")));
        assert_eq!(builtin_of_key(&julia), Some("julia"));
        assert_eq!(inverse::for_name(&julia).map(|d| d.name), Some("julia"));
        assert_eq!(inverse::for_name(&bubble).map(|d| d.name), Some("bubble"));
        assert_eq!(bound::for_name(&bubble).map(|d| d.name), Some("bubble"));
        assert_eq!(bound::for_name(&blur).map(|d| d.name), Some("blur"));
        let none = |_: &str| 0.0;
        let ball = bound::Ball::new([0.4, -0.3], 0.1);
        assert_eq!(derive::derive(&bubble, &none, 0.8, ball), derive::derive("bubble", &none, 0.8, ball), "its interval body");
        assert!(derive::derive("bubble", &none, 0.8, ball).is_ok());

        let edited = registered(&format!("{}// mine now\n", own("bubble")));
        assert_eq!(builtin_of_key(&edited), None);
        assert!(inverse::for_name(&edited).is_none() && bound::for_name(&edited).is_none());
        assert!(derive::derive(&edited, &none, 0.8, ball).is_err());
    }

    /// **A twin is unedited by its maths, not its text** (the user,
    /// 2026-11-08: bubble's twin retitled "Bubble2" lost the Flame
    /// Attractor and Focused Rendering): its title or `INFO.name` changed,
    /// or a comment added, it computes its built-in still and keeps its
    /// planner data; a constant of its body changed, it does not.
    #[test]
    fn a_twin_is_unedited_by_its_maths_not_its_text() {
        let compiled = |src: &str| super::super::custom::compile(src).unwrap();
        let found = |src: &str| builtin_computed_by(src, &compiled(src));
        // From LF line ends, as a checkout with `core.autocrlf` gives CRLF.
        let base = own("bubble").replace("\r\n", "\n");
        let retitled = base.replacen("title: \"Bubble\"", "title: \"Bubble2\"", 1);
        assert_ne!(retitled, base, "the twin has the title");
        assert_eq!(builtin_of(&retitled), None, "its text is not the twin's");
        assert_eq!(found(&retitled), Some("bubble"), "its maths is");
        let renamed = retitled.replacen("name: \"rhai_bubble\"", "name: \"my_bubble\"", 1);
        assert_eq!(found(&renamed), Some("bubble"), "found by its functions");
        let commented = retitled.replacen("    let scale = 1.0 / r;\n", "    // mine now\n    let scale = 1.0 / r;\n", 1);
        assert_ne!(commented, retitled);
        assert_eq!(found(&commented), Some("bubble"), "found by its name");
        let edited = retitled.replacen("r2 / 4.0", "r2 / 3.0", 1);
        assert_ne!(edited, retitled);
        assert_eq!(found(&edited), None, "another map");
        assert_eq!(found(&own("julia")), Some("julia"));
    }

    /// **How far Focused Rendering's derived bounds reach a Rhai
    /// variation's own code** (a measure, not a gate): each twin's compiled
    /// body through the interval evaluator (`derive::derive_own`), against
    /// its built-in's, at the parameters' defaults, over one disc. Prints
    /// the counts and why a twin refuses where its built-in derives.
    /// Ignored: it registers every twin.
    #[test]
    #[ignore = "a measure; registers every twin"]
    fn measure_derived_bounds_of_twins() {
        use crate::variations::{bound::Ball, derive};
        let ball = Ball::new([0.3, -0.2], 0.5);
        let (mut both, mut builtin_only, mut twin_only, mut neither) = (0, 0, 0, 0);
        let mut why: std::collections::BTreeMap<String, Vec<&str>> = Default::default();
        for (name, src) in chosen_twins() {
            let Ok(key) = super::super::custom::register(src) else { continue };
            let reg = super::super::global_registry().clone();
            let info = reg.get(name).unwrap();
            let defaults: std::collections::HashMap<String, f64> = info.parameters.iter().map(|p| (p.name.to_string(), f64::from(p.slot_default()))).collect();
            let params = |p: &str| defaults.get(p).copied().unwrap_or(0.0);
            let builtin = derive::derive(name, &params, 1.0, ball);
            let twin = derive::derive_own(&key, &params, 1.0, ball);
            match (&builtin, &twin) {
                (Ok(_), Ok(_)) => both += 1,
                (Ok(_), Err(e)) => {
                    builtin_only += 1;
                    why.entry(e.to_string()).or_default().push(name);
                }
                (Err(_), Ok(_)) => twin_only += 1,
                (Err(_), Err(_)) => neither += 1,
            }
        }
        println!("derived: both {both}, the built-in alone {builtin_only}, the twin alone {twin_only}, neither {neither}");
        let mut reasons: Vec<_> = why.into_iter().collect();
        reasons.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (reason, names) in reasons {
            println!("  {:3} {reason}: {}", names.len(), names.iter().take(8).cloned().collect::<Vec<_>>().join(" "));
        }
    }

    /// **A variation is named by its label where the user reads it**: a
    /// Rhai variation by its title, never its key, in the Flame
    /// Attractor's and Focused Rendering's refusals as in the panels, and
    /// transforms numbered from 1. In a registry of the test's own.
    #[test]
    fn refusals_name_a_rhai_variation_by_its_title() {
        use crate::scene::{ifs_analysis::{Disqualification, NotAffine}, ifs_ball::NoBall};
        let mut reg = crate::variations::VariationRegistry::new();
        let src = own("bubble").replacen("title: \"Bubble\"", "title: \"Bubble2\"", 1).replacen("r2 / 4.0", "r2 / 3.0", 1);
        let dl = super::super::custom::compile(&src).unwrap();
        let key = super::super::custom::register_into(&mut reg, &dl, &src).unwrap();
        assert!(crate::scripted::is_key(&key), "{key}");
        assert_eq!(reg.label(&key), "Bubble2");
        assert_eq!(reg.label("bubble"), "Bubble");
        assert_eq!(reg.label("no_such_variation"), "no_such_variation");
        let d = Disqualification::NotAffine { index: 0, why: NotAffine::Variation(key.clone()) };
        assert_eq!(d.describe(&reg), "transform 1 uses Bubble2, which has no inverse");
        assert_eq!(d.to_string(), format!("transform 0 uses `{key}`, which is not affine"), "logs keep the key");
        let f = Disqualification::FinalNotAffine { why: Some(NotAffine::Priority(key.clone())) };
        assert_eq!(f.describe(&reg), "the final transform moves Bubble2 out of the weighted sum");
        let b = NoBall::Unbounded(key.clone(), Some("bounds are worked out for built-in variations only, so far".into()));
        assert_eq!(b.describe(&reg), "Bubble2 has no forward bound (bounds are worked out for built-in variations only, so far)");
    }

    /// **A transform with unedited twins is analysed as with their
    /// built-ins**: the forward bounds' disc (`bubble` + `blur`) and the
    /// inverse walk's map (`julia`, a kernel the user may move to pre or
    /// post) come out the same. In a registry of the test's own, as
    /// registering in the global one would stall the suite.
    #[test]
    fn an_unedited_twins_transform_is_analysed_as_its_builtins() {
        use crate::scene::transforms::Transform;
        let mut reg = crate::variations::VariationRegistry::new();
        let mut as_twins = |t: &Transform, names: &[&str]| {
            let mut t = t.clone();
            for name in names {
                let src = own(name);
                let dl = super::super::custom::compile(&src).unwrap();
                let key = super::super::custom::register_into(&mut reg, &dl, &src).unwrap();
                assert!(t.rename_variation(name, &key));
            }
            t
        };
        let mut t = Transform::default();
        (t.a, t.d, t.e) = (0.5, 0.5, 0.25);
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("bubble", 0.8);
        t.set_variation("blur", 0.3);
        let twins = as_twins(&t, &["bubble", "blur"]);
        let mut julia = t.clone();
        julia.variations.clear();
        julia.variation_order.clear();
        julia.set_variation("julia", 1.0);
        let julia_twin = as_twins(&julia, &["julia"]);

        use crate::scene::{ifs_analysis, ifs_ball};
        let input = crate::variations::bound::Ball::new([0.3, -0.2], 0.6);
        let ball = ifs_ball::transform_ball_2d(&t, &reg, input);
        assert!(ball.is_ok(), "{ball:?}");
        assert_eq!(ifs_ball::transform_ball_2d(&twins, &reg, input), ball, "the forward bounds");
        let map = ifs_analysis::transform_map_2d_ordered(&julia, &reg, &[]);
        assert!(map.is_ok());
        assert_eq!(ifs_analysis::transform_map_2d_ordered(&julia_twin, &reg, &[]), map, "the inverse walk's map");
    }

    /// The defs' source files, which hold the built-ins' doc blocks.
    fn def_sources() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/variations/defs");
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
            .map(|e| std::fs::read_to_string(e.path()).unwrap())
            .collect()
    }

    /// **Each twin keeps its built-in's description and authors**: it
    /// opens with its def's doc block verbatim, so what the API reads from
    /// it -- as text, from either -- is the built-in's, and the library
    /// shows them.
    #[test]
    fn every_variation_twin_keeps_its_builtins_doc() {
        let sources = def_sources();
        let docs = super::super::docs::parse_sources(sources.iter().map(String::as_str));
        for (name, src) in TWINS {
            let block = sources.iter().find_map(|s| super::super::docs::block_text(s, name)).unwrap_or_else(|| panic!("{name}: no doc block"));
            assert!(src.replace("\r\n", "\n").starts_with(&format!("{block}\n")), "{name}: its file does not open with its def's block");
            assert_eq!(super::super::docs::leading_block(src).as_ref(), docs.get(*name), "{name}");
            let labels = crate::lang::rhai::source_labels(src).unwrap();
            assert_eq!(labels.author, docs[*name].authors.join(", "), "{name}: its authors reach the library");
            assert_eq!(labels.description.resolve(), docs[*name].description, "{name}: its description reaches the library");
        }
    }

    /// WRITER, not a test: `TWIN_SKELETONS=julia,rings2` writes each
    /// one's skeleton to `assets/variations/twins/<name>.rhai`, where none
    /// is yet.
    /// **The twins' inverses undo their forward maps**
    /// (docs/projects/rhai-planners.md, B1): every twin's that has one,
    /// written by hand (`scripts/twin_inverses.py`) -- the eleven planar
    /// kernels' and the built-ins' that have none in Rust -- each
    /// compiled and checked at its defaults (`user_inverse::check`).
    #[test]
    fn the_twins_inverses_undo_their_maps() {
        // The solid's (B3).
        for name in ["julia3D", "julia3Dz"] {
            match crate::variations::user_inverse::compile3(&own(name), name) {
                Some(Ok(_)) => {}
                Some(Err(e)) => panic!("{name}'s 3D inverse: {e}"),
                None => panic!("{name} has no 3D inverse"),
            }
        }
        let with: Vec<&str> = TWINS.iter().filter(|(_, s)| s.contains("fn inverse(")).map(|(n, _)| *n).collect();
        assert!(with.len() >= 20, "{with:?}");
        // A forward map for a sum's Newton solve (B2) -- one that draws, its
        // draws read from a parameter, an arm's (D2).
        let fwd = |n: &str| crate::variations::user_inverse::compile(&own(n), n).and_then(Result::ok).map(|i| i.forward_wgsl.is_some());
        assert_eq!(fwd("julia"), Some(true), "julia draws its root, read from the arm's draw");
        assert_eq!(fwd("bubble"), Some(true));
        for name in with {
            match crate::variations::user_inverse::compile(&own(name), name) {
                Some(Ok(inv)) => assert!(inv.branches >= 1, "{name}"),
                Some(Err(e)) => panic!("{name}: {e}"),
                None => panic!("{name} has no inverse"),
            }
        }
    }

    /// `TWIN_WGSL_DUMP=a,b`: each twin's compiled 2D and 3D bodies, to
    /// `output/twin-wgsl/<name>-2d.wgsl` and `-3d.wgsl` -- what the
    /// renderer and the bounds' evaluator read.
    #[test]
    #[ignore = "writes files"]
    fn dump_twin_wgsl() {
        let Ok(names) = std::env::var("TWIN_WGSL_DUMP") else { return };
        let dir = std::path::Path::new("output/twin-wgsl");
        std::fs::create_dir_all(dir).unwrap();
        for name in names.split(',') {
            let dl = super::super::custom::compile(&own(name)).unwrap();
            std::fs::write(dir.join(format!("{name}-2d.wgsl")), dl.shader_2d.unwrap_or_default()).unwrap();
            std::fs::write(dir.join(format!("{name}-3d.wgsl")), dl.shader_3d.unwrap_or_default()).unwrap();
            if let Some(Ok(inv)) = crate::variations::user_inverse::compile(&own(name), name) {
                std::fs::write(dir.join(format!("{name}-inverse.wgsl")), &inv.wgsl).unwrap();
            }
        }
    }

    #[test]
    #[ignore = "writes files"]
    fn write_variation_twin_skeletons() {
        let Ok(names) = std::env::var("TWIN_SKELETONS") else { return };
        for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            let path = format!("assets/variations/twins/{name}.rhai");
            if std::path::Path::new(&path).exists() {
                println!("{path}: there already");
                continue;
            }
            let block = def_sources().iter().find_map(|s| super::super::docs::block_text(s, name));
            let text = skeleton(name, block.as_deref()).unwrap_or_else(|| panic!("no built-in `{name}`"));
            std::fs::write(&path, text).unwrap();
            println!("wrote {path}");
        }
    }

    /// Each twin compiles as the app compiles a variation (its WGSL
    /// validated), under its built-in's display name, with its built-in's
    /// parameters exactly and the features that change what it draws: the
    /// colour it writes, z always written, the running sum read.
    #[test]
    fn every_variation_twin_compiles_as_its_builtin() {
        use crate::variations::Feature as F;
        let reg = super::super::global_registry();
        let mut failed = Vec::new();
        for (name, src) in TWINS {
            let builtin = reg.get(name).unwrap_or_else(|| panic!("no built-in `{name}`"));
            let dl = match super::super::custom::compile(src) {
                Ok(dl) => dl,
                Err(e) => {
                    failed.push(format!("{name}: {}", e.lines().next().unwrap_or_default()));
                    continue;
                }
            };
            assert_eq!(dl.display_name, builtin.display_name, "{name}: its title");
            let p = crate::lang::rhai::transformation(src).or_else(|_| crate::lang::rhai::transformation3(src)).unwrap();
            let params = super::super::custom::param_defs(&p, src).unwrap();
            assert_eq!(params.len(), builtin.parameters.len(), "{name}: its parameters");
            for (t, b) in params.iter().zip(&builtin.parameters) {
                assert_eq!((&t.name, &t.label, &t.hint, &t.kind, t.slow), (&b.name, &b.label, &b.hint, &b.kind, b.slow), "{name}.{}", b.name);
            }
            let has = |f: &str| dl.features.iter().any(|x| x == f);
            for (f, wire) in [(F::WritesColor, "writes_color"), (F::AlwaysZ, "always_z"), (F::NeverZ, "never_z"), (F::NeedsAccum, "needs_accum"), (F::Replace, "replace"), (F::CanHide, "can_hide"), (F::WritesRgb, "writes_rgb"), (F::NeedsW, "needs_w")] {
                assert_eq!(has(wire), builtin.features.contains(&f), "{name}: {f:?}");
            }
            assert_eq!(u32::from(dl.plot_emits), builtin.plot_emit_cap(), "{name}: the most points one call emits");
            assert_eq!(dl.name, key(name).unwrap(), "{name}: its key");
            assert_eq!(dl.category, builtin.category.to_api_str(), "{name}: its category");
            assert_eq!(super::super::custom::phase_of(src), builtin.phase, "{name}: its phase");
        }
        assert!(failed.is_empty(), "{} of {} twins do not compile:
{}", failed.len(), TWINS.len(), failed.join("
"));
    }

    /// The points a twin is held at: the probe's adversarial inputs (the
    /// signed zeros, the axes, the far and the tiny) and a grid of
    /// ordinary ones, each with a z.
    fn points() -> Vec<[f32; 3]> {
        let mut pts: Vec<[f32; 3]> = crate::probe::probe_inputs().iter().map(|p| [p.x, p.y, p.z]).collect();
        for i in 0..9 {
            for j in 0..9 {
                let (x, y) = (-3.1 + 0.77 * i as f32, -2.9 + 0.73 * j as f32);
                pts.push([x, y, 0.37 * (i as f32 - j as f32)]);
            }
        }
        pts
    }

    /// Whether a twin's output `got` agrees with its built-in's `want`:
    /// both finite and within 2e-5 of the larger one's size (1 below it)
    /// -- the ulps the language's range-reduced trig and a fused
    /// multiply-add part them by -- or alike not finite.
    fn agrees(want: f32, got: f32) -> bool {
        match (want.is_finite(), got.is_finite()) {
            (true, true) => (want - got).abs() <= 2e-5 * want.abs().max(got.abs()).max(1.0),
            (false, false) => want.is_nan() == got.is_nan() && (want.is_nan() || want.signum() == got.signum()),
            _ => false,
        }
    }

    /// Within the f64 truth's reach of f32: 1e-4 of its size (1 below it),
    /// or alike not finite.
    fn near(x: f32, t: f64) -> bool {
        if !t.is_finite() {
            return !x.is_finite();
        }
        (f64::from(x) - t).abs() <= 1e-4 * t.abs().max(1.0)
    }

    /// The twin of `builtin` in f64 (the interpreter), at the pair's
    /// parameters and weight 1 -- for a twin that draws no random numbers,
    /// whose draws the interpreter's stream would not match.
    /// Built-ins that take a coordinate's remainder (`x - trunc(x / m) * m`):
    /// past 2^24 it is rounding on every side -- fused or not, f32 or f64 --
    /// so their pairs are not held at points that far. mobius_strip at
    /// [1e20, -1e20]: the built-in's 0, the twin's a remainder times a
    /// derived cos(90 degrees), the same expression in both WGSLs.
    const FAR_REMAINDER: &[&str] = &["mobius_strip"];

    /// The parts of a pair the built-in may be off its formula at where the
    /// twin is on it, before the twin is doubted (`every_variation_twin_runs_as_its_builtin`).
    const OFF_CAP: usize = 16;

    struct Judge {
        two: crate::lang::ir::Program,
        three: Option<crate::lang::ir::Program>,
        values: crate::lang::interp::ParamValues,
        /// The parameters' slots, which [`Self::spread`] moves.
        slots: Vec<f32>,
        render_3d: bool,
    }

    impl Judge {
        fn new(builtin: &str, params: &[(String, f32)], render_3d: bool) -> Option<Judge> {
            let src = source(builtin)?;
            if src.contains("rand()") {
                return None;
            }
            let two = crate::lang::rhai::transformation(src).ok()?;
            let three = crate::lang::rhai::has_function(src, "variation3").then(|| crate::lang::rhai::transformation3(src).ok()).flatten();
            let reg = super::super::global_registry();
            let defs = &reg.get(builtin)?.parameters;
            let slots: Vec<f32> = defs.iter().map(|d| params.iter().find(|(n, _)| *n == d.name).map_or(d.slot_default(), |(_, v)| *v)).collect();
            let values = crate::lang::interp::ParamValues::from_slots(&two, &slots);
            Some(Judge { two, three, values, slots, render_3d })
        }

        /// How far part `c` of the truth moves when the point, or one
        /// parameter, moves by about one part in a million, any way: a few
        /// f32 ulps. A parameter's move is a derived value's (`prepare`'s
        /// angle at an exact turn, 360 degrees, where its sine is zero).
        fn spread(&self, p: [f32; 3], c: usize) -> f64 {
            let base = self.at_f64(p.map(f64::from))[c];
            let mut far = 0.0f64;
            let mut take = |v: f64| {
                if v.is_finite() && base.is_finite() {
                    far = far.max((v - base).abs());
                } else if v.is_finite() != base.is_finite() {
                    far = f64::INFINITY;
                }
            };
            for axis in 0..3 {
                for s in [-1.0, 1.0] {
                    let mut q = p.map(f64::from);
                    // Not a round number: 1e20 ± 1e14 is still a multiple
                    // of 5, and a remainder by 5 would not move.
                    q[axis] += s * 1e-6 * std::f64::consts::SQRT_2 * q[axis].abs().max(1e-6);
                    take(self.at_f64(q)[c]);
                }
            }
            for i in 0..self.slots.len() {
                for s in [-1.0f32, 1.0] {
                    let mut slots = self.slots.clone();
                    slots[i] += s * 1e-6 * slots[i].abs().max(1e-6);
                    let values = crate::lang::interp::ParamValues::from_slots(&self.two, &slots);
                    take(self.with(&values, p.map(f64::from))[c]);
                }
            }
            far
        }

        fn at(&self, p: [f32; 3]) -> [f64; 4] {
            self.at_f64(p.map(f64::from))
        }

        fn at_f64(&self, p: [f64; 3]) -> [f64; 4] {
            self.with(&self.values, p)
        }

        fn with(&self, values: &crate::lang::interp::ParamValues, p: [f64; 3]) -> [f64; 4] {
            use crate::lang::builtins::C64;
            use crate::lang::interp::VariationRun;
            let (x, y, z) = (p[0], p[1], p[2]);
            // The colour from the probe's 0.5 (`main_template.wgsl`), which a
            // twin that writes none leaves.
            const PROBE_COLOR: f64 = 0.5;
            // ...and the probe transform's coefficients (`ctx.a` ..).
            fn run<'a>(p: &'a crate::lang::ir::Program, values: &'a crate::lang::interp::ParamValues) -> VariationRun<'a> {
                let a = crate::probe::flame::AFFINE;
                VariationRun::new(p, values, 1).with_color(PROBE_COLOR).with_coefs([0, 1, 2, 3, 4, 5].map(|k| f64::from(a[k]))).with_w(0.0)
            }
            if !self.render_3d {
                let (w, c) = run(&self.two, values).call(C64::new(x, y), C64::ZERO, 1.0);
                return [w.re, w.im, 0.0, c.unwrap_or(PROBE_COLOR)];
            }
            match &self.three {
                Some(p3) => {
                    let (v, c) = run(p3, values).call3([x, y, z], [0.0; 3], 1.0);
                    [v[0], v[1], v[2], c.unwrap_or(PROBE_COLOR)]
                }
                None => {
                    let (w, c) = run(&self.two, values).call(C64::new(x, y), C64::ZERO, 1.0);
                    [w.re, w.im, z, c.unwrap_or(PROBE_COLOR)]
                }
            }
        }
    }

    /// **Each twin runs as its built-in** (P7 step 3, V1): through the
    /// probe's pairs, the built-in and its twin at every point, in 2D and
    /// 3D, at the defaults and with each parameter at its range's ends,
    /// every output part and the colour it writes. A point past 2^24,
    /// where trig is garbage on every platform and differently
    /// (docs/accepted-divergences.txt), is not held where the built-in
    /// gives no finite answer either.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_variation_twin_runs_as_its_builtin() {
        use crate::probe::pairs::{run_pairs, Pair};
        let (device, queue) = pollster::block_on(crate::probe::run::open_device("variation twins")).expect("a GPU");
        let pts = points();
        let reg = super::super::global_registry().clone();
        let mut pairs = Vec::new();
        let mut carried = Vec::new();
        for (name, _) in chosen_twins() {
            let builtin = reg.get(name).unwrap();
            let twin = key(name).unwrap();
            carried.push(super::carried(name).unwrap());
            pairs.push((format!("{name} at its defaults"), Pair { builtin: name.to_string(), twin: twin.clone(), params: Vec::new() }));
            for d in &builtin.parameters {
                if let Some((lo, hi)) = d.kind.bounds() {
                    for v in [lo, hi] {
                        pairs.push((format!("{name} with {} at {v}", d.name), Pair { builtin: name.to_string(), twin: twin.clone(), params: vec![(d.name.to_string(), v as f32)] }));
                    }
                }
            }
        }
        drop(reg);
        // The compare tells a different variation apart: exp's twin set
        // beside julia, judged by exp's formula, as a twin of julia that
        // computed exp would be.
        pairs.push(("(control) julia beside exp's twin".into(), Pair { builtin: "julia".into(), twin: key("exp").unwrap(), params: Vec::new() }));
        // Each twin's built-in, by its key: the formula a pair is judged by is
        // the twin's that runs (the control's, exp's).
        let of_key: std::collections::HashMap<String, &str> = TWINS.iter().filter_map(|(n, _)| Some((key(n)?, *n))).collect();
        let mut failures = Vec::new();
        let mut counts = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        // In flames of at most 96 variations and 120 transforms (two a pair,
        // MAX_TRANSFORMS 128).
        let mut batches: Vec<Vec<usize>> = vec![Vec::new()];
        let mut names = std::collections::HashSet::new();
        for (i, (_, pair)) in pairs.iter().enumerate() {
            let new = [&pair.builtin, &pair.twin].iter().filter(|n| !names.contains(**n)).count();
            if names.len() + new > 96 || batches.last().unwrap().len() >= 60 {
                batches.push(Vec::new());
                names.clear();
            }
            names.insert(pair.builtin.clone());
            names.insert(pair.twin.clone());
            batches.last_mut().unwrap().push(i);
        }
        for render_3d in [false, true] {
            for batch in &batches {
                let only: Vec<Pair> = batch.iter().map(|&i| pairs[i].1.clone()).collect();
                let out = run_pairs(&device, &queue, &only, &carried, &pts, render_3d).expect("the pairs ran");
                for (&i, o) in batch.iter().zip(&out) {
                    let what = &pairs[i].0;
                    let builtin = &pairs[i].1.builtin;
                    let mut bad = Vec::new();
                    let mut exact = 0;
                    let mut builtin_off = 0;
                    let mut off_parts = Vec::new();
                    let mut builtin_nan = 0;
                    let mut ill = 0;
                    let judge = Judge::new(of_key[&pairs[i].1.twin], &pairs[i].1.params, render_3d);
                    for (k, (w, g)) in o.builtin.iter().zip(&o.twin).enumerate() {
                        let far = pts[k].iter().any(|c| c.abs() > 16_777_216.0);
                        let truth = judge.as_ref().map(|j| j.at(pts[k]));
                        for c in 0..4 {
                            exact += usize::from(w[c].to_bits() == g[c].to_bits());
                            if agrees(w[c], g[c]) || (far && !w[c].is_finite()) || (far && FAR_REMAINDER.contains(&builtin.as_str())) {
                                continue;
                            }
                            // The twin's own formula in f64: where the GPU's
                            // twin is it and the built-in is not, the built-in
                            // is the imprecise side (its hardware trig, its
                            // pow at zero).
                            let part = format!("    at {:?} part {c}: built-in {} twin {}{}", pts[k], w[c], g[c], truth.and_then(|t| t.get(c).map(|x| format!(" (f64 {x})"))).unwrap_or_default());
                            if let Some(t) = truth.and_then(|t| t.get(c).copied()) {
                                if near(g[c], t) && !near(w[c], t) {
                                    // No number where its formula has one
                                    // (WGSL's pow of a negative base, which
                                    // JWildfire's Math.pow takes): never the
                                    // sign of a twin that is another formula.
                                    if !w[c].is_finite() {
                                        builtin_nan += 1;
                                    } else {
                                        builtin_off += 1;
                                        off_parts.push(part);
                                    }
                                    continue;
                                }
                                // Both within f32's reach of the truth: the
                                // formula's precision, not a difference.
                                if near(g[c], t) && near(w[c], t) {
                                    continue;
                                }
                                // Ill-conditioned: the truth itself moves by as
                                // much when the point or a parameter moves by a
                                // few f32 ulps -- trig of a large value, a near
                                // cancellation, a sine at an exact turn -- so
                                // the two f32 roundings may part by that.
                                let spread = judge.as_ref().map_or(0.0, |j| j.spread(pts[k], c));
                                if w[c].is_finite() && g[c].is_finite() && f64::from((w[c] - g[c]).abs()) <= 4.0 * spread {
                                    ill += 1;
                                    continue;
                                }
                                // Both off it, the twin the nearer: the
                                // built-in's rounding the larger, past what
                                // the perturbations reach (a derived sine at
                                // an exact turn, divided into 1e20).
                                if g[c].is_finite() && (f64::from(g[c]) - t).abs() < (f64::from(w[c]) - t).abs() {
                                    builtin_off += 1;
                                    off_parts.push(part);
                                    continue;
                                }
                            }
                            bad.push(part);
                        }
                    }
                    // Off its own formula at a handful of points, the built-in
                    // is imprecise there (at most 9 of 432 parts, measured);
                    // at more, the twin may not be its formula -- judged by
                    // itself, a wrong twin would be "on" it everywhere.
                    if builtin_off > OFF_CAP {
                        bad.append(&mut off_parts);
                    } else if builtin_off > 0 {
                        counts.3 += 1;
                    }
                    if builtin_nan > 0 {
                        counts.6 += 1;
                    }
                    if ill > 0 {
                        counts.4 += 1;
                    }
                    let dim = if render_3d { "3D" } else { "2D" };
                    if what.starts_with("(control)") {
                        if bad.len() < pts.len() {
                            failures.push(format!("{what} ({dim}): the compare did not tell them apart ({} apart)", bad.len()));
                        }
                        continue;
                    }
                    counts.0 += 1;
                    counts.2 += exact;
                    // A twin that draws: the interpreter's stream is not the
                    // GPU's, so no truth to judge a point by -- its trig of a
                    // drawn angle, a sin hash's noise (circular's), apart by
                    // rounding. Its flame's density judges it
                    // (`every_variation_twin_draws_its_builtins_flame`).
                    if judge.is_none() && !bad.is_empty() {
                        counts.5 += 1;
                        println!("{what} ({dim}): drawing, {} of {} parts apart -- judged by its flame's density", bad.len(), pts.len() * 4);
                        continue;
                    }
                    if !bad.is_empty() {
                        counts.1 += 1;
                        println!("{what} ({dim}): {} of {} parts apart, {exact} bit for bit", bad.len(), pts.len() * 4);
                        failures.push(format!("{what} ({dim}):\n{}", bad.iter().take(4).cloned().collect::<Vec<_>>().join("\n")));
                    }
                }
            }
        }
        println!(
            "{} pairs, {} apart, {} with the built-in off its own formula where the twin is on it, {} with the built-in giving no number where its formula has one, {} ill-conditioned somewhere, {} drawing and apart (judged by density); {:.1}% of their parts bit for bit",
            counts.0,
            counts.1,
            counts.3,
            counts.6,
            counts.4,
            counts.5,
            100.0 * counts.2 as f64 / (counts.0 * pts.len() * 4).max(1) as f64
        );
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The twins a GPU gate or tool runs: those `VARIATION_TWINS=a,b`
    /// names, else every one.
    fn chosen_twins() -> impl Iterator<Item = &'static (&'static str, &'static str)> {
        let only: Option<Vec<String>> = std::env::var("VARIATION_TWINS").ok().map(|v| v.split(',').map(str::to_string).collect());
        TWINS.iter().filter(move |(n, _)| only.as_ref().is_none_or(|o| o.iter().any(|x| x == n)))
    }

    /// A 2D flame of three transforms: `variation` on a turning shrink,
    /// two linear shrinks keeping it bounded; a twin's flame carries its
    /// source, which nothing registers but the renderer. The variation's
    /// transform blends in the colour it writes by `direct`
    /// (`Transform::direct_color`).
    fn flame_coloured(variation: &str, carries: Option<CustomSource>, direct: f32) -> crate::config::FractalConfig {
        use crate::scene::transforms::{RenderMode, Transform};
        let mut cfg = crate::config::FractalConfig::default();
        cfg.render_mode = RenderMode::TwoD;
        cfg.flame.transforms.clear();
        for (i, (m, v)) in [((0.6f32, -0.3f32, 0.3f32, 0.6f32, 0.1f32, -0.2f32), variation), ((0.5, 0.0, 0.0, 0.5, 0.5, 0.3), "linear"), ((0.5, 0.0, 0.0, 0.5, -0.4, 0.4), "linear")]
            .into_iter()
            .enumerate()
        {
            let mut t = Transform::default();
            (t.a, t.b, t.c, t.d, t.e, t.f) = m;
            t.weight = 1.0;
            t.color = i as f32 / 2.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(v, 1.0);
            if i == 0 {
                t.direct_color = direct;
            }
            cfg.flame.transforms.push(t);
        }
        cfg.flame.custom_variations = carries.into_iter().collect();
        cfg.zoom = 1.6;
        cfg.levels_enabled = false;
        cfg
    }

    /// **Each twin draws its built-in's flame** (P7 step 3, V1): by
    /// density, block by block over replicates -- the chaos game parts two
    /// orbits at the first rounding, so they agree as measures, not pixel
    /// for pixel. And the measure tells two built-ins apart.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_variation_twin_draws_its_builtins_flame() {
        use crate::renderer::density_gates::{compare_blocks, print_compared, render_blocks, Run};
        const N: u32 = 256;
        let (device, queue) = pollster::block_on(crate::probe::run::open_device("variation twin flames")).expect("a GPU");
        let run = Run::default();
        // Twins whose flame is a different realisation of the same noise:
        // a `sin(...) * 43758.5453` hash places their cells, and the
        // language's range-reduced sin (the portable side, the same on
        // every platform, where the hardware's is not) hashes otherwise.
        // Held to drawing, not to the built-in's layout.
        // cut_jigsaw's pieces flip by `sin(dot(iv, (27, 57))) * 1e5`, and
        // cut_fingerprint, cut_hextruchetflow and cut_metaballs hash by
        // `fract(sin(x) * 43...)`: with the hardware's sin each flame is the
        // built-in's exactly, measured.
        const OTHER_NOISE: &[&str] = &["r_circleblur", "cut_jigsaw", "cut_fingerprint", "cut_hextruchetflow", "cut_metaballs"];
        // Twins whose flame sends points past 2^24, where an f32's tan is no
        // formula's on any side (docs/accepted-divergences.txt) and the
        // language's range-reduced one (the portable side) differs from
        // the hardware's. rays1: reduced below 1e5 its flame is the
        // built-in's exactly, measured; it parts past 2^24.
        const HUGE_TRIG: &[&str] = &["rays1"];
        let mut failures = Vec::new();
        let mut renders = Vec::new();
        // A twin that writes a colour is held to its built-in's colour too:
        // its transform blends it in whole, and each channel's light (colour
        // times density) compares as density does.
        let writes_colour = |name: &str| {
            super::super::global_registry().get(name).is_some_and(|v| v.has_feature(super::super::Feature::WritesColor) || v.has_feature(super::super::Feature::WritesRgb))
        };
        let off = |c: &crate::renderer::density_gates::Compared| c.worst_dense > 0.05 || c.worst_middle > 0.15;
        // A 4D twin's flame in 3D, where its 4th coordinate lives and feeds
        // back through every iteration (a 2D flame has none).
        let four_d = |name: &str| super::super::global_registry().get(name).is_some_and(|v| v.has_feature(super::super::Feature::NeedsW));
        // A twin reading the camera (`ctx.camera_x` .. `ctx.perspective`)
        // is drawn with it turned, moved and in perspective, every field
        // apart from the default's zero.
        let reads_camera = |name: &str| source(name).is_some_and(|s| s.contains("ctx.camera_") || s.contains("ctx.perspective"));
        // `VARIATION_TWINS_PARAMS=k=v,k=v`: the variation's parameters in
        // both flames, a mode its defaults do not reach (menger's 4-gap,
        // which reads its table).
        let params: Vec<(String, f32)> = std::env::var("VARIATION_TWINS_PARAMS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|kv| kv.split_once('=').and_then(|(k, v)| Some((k.to_string(), v.parse().ok()?))))
            .collect();
        let flame = |name: &str, twin_of: &str, carries: Option<CustomSource>, direct: f32, three: bool| {
            let mut cfg = flame_coloured(name, carries, direct);
            for t in cfg.flame.transforms.iter_mut().filter(|t| t.variations.contains_key(name)) {
                for (k, v) in &params {
                    t.variation_params.insert(format!("{name}.{k}"), *v);
                }
            }
            if three {
                cfg.render_mode = crate::scene::transforms::RenderMode::ThreeD;
                if reads_camera(twin_of) {
                    (cfg.camera_rotation_x, cfg.camera_rotation_y, cfg.camera_bank) = (0.35, 0.6, 0.15);
                    (cfg.camera_x, cfg.camera_y, cfg.camera_z) = (0.1, -0.05, 0.2);
                    cfg.perspective_strength = 0.25;
                }
            }
            cfg
        };
        // Each twin's flame in 2D and in 3D, its two bodies (a 4D twin's in
        // 3D alone); `VARIATION_TWINS_DIMS=2` or `3` draws one of them.
        let dims = std::env::var("VARIATION_TWINS_DIMS").unwrap_or_default();
        for (name, _) in chosen_twins() {
            let direct = if writes_colour(name) { 1.0 } else { 0.0 };
            let both: &[bool] = if four_d(name) { &[true] } else { &[false, true] };
            for &three in both.iter().filter(|&&t| dims.is_empty() || dims == if t { "3" } else { "2" }) {
                let (what, d) = if three { (format!("{name} (3D)"), " (3D)") } else { (name.to_string(), "") };
                let reference = render_blocks(&device, &queue, &flame(name, name, None, direct, three), N, &run);
                let twin = render_blocks(&device, &queue, &flame(&key(name).unwrap(), name, Some(super::carried(name).unwrap()), direct, three), N, &run);
                if direct > 0.0 {
                    for ch in 0..3 {
                        let c = compare_blocks(&twin.light[ch], &reference.light[ch], &reference.density);
                        if off(&c) {
                            print_compared(&format!("{what}'s twin against {name}, channel {ch}'s light"), &c);
                            failures.push(format!("{what}'s twin: its colour, channel {ch}: a block is {:.3} off (dense), {:.3} (middle)", c.worst_dense, c.worst_middle));
                        }
                    }
                }
                let c = compare_blocks(&twin.density, &reference.density, &reference.density);
                let (_, _, mx, _, my, _, z) = c.pooled[0];
                if mx <= 0.0 {
                    failures.push(format!("{what}'s twin drew nothing"));
                }
                if OTHER_NOISE.contains(name) {
                    println!("  {what}'s twin: its own noise, drawn");
                    continue;
                }
                if HUGE_TRIG.contains(name) {
                    println!("  {what}'s twin: its trig past 2^24 the portable side's, drawn");
                    continue;
                }
                print_compared(&format!("{what}'s twin against {name}{d}"), &c);
                if z.abs() > 4.0 && (mx / my - 1.0).abs() > 0.005 {
                    failures.push(format!("{what}'s twin: the whole view's density is {:.4} of the built-in's", mx / my));
                }
                if off(&c) {
                    failures.push(format!("{what}'s twin: a block is {:.3} off (dense), {:.3} (middle)", c.worst_dense, c.worst_middle));
                }
                if !three && ["julia", "exp"].contains(name) {
                    renders.push((name, reference));
                }
            }
        }
        // The measure tells two built-ins apart, or agreeing means nothing.
        if let [a, b] = &renders[..] {
            let c = compare_blocks(&a.1.density, &b.1.density, &b.1.density);
            if c.worst_dense < 0.2 {
                failures.push(format!("{} and {} look alike to this measure: {:.3}", a.0, b.0, c.worst_dense));
            }
        }
        // And the light tells a colour written from none: dc_linear's,
        // blended in and not.
        let (on, none) = (render_blocks(&device, &queue, &flame_coloured("dc_linear", None, 1.0), N, &run), render_blocks(&device, &queue, &flame_coloured("dc_linear", None, 0.0), N, &run));
        let c = compare_blocks(&on.light[0], &none.light[0], &none.density);
        if !off(&c) {
            failures.push(format!("dc_linear's colour, blended in and not, looks alike to this measure: {:.3}", c.worst_dense));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// A parameter of every kind written as a twin's `PARAMS` reads back
    /// as the same definition.
    #[test]
    fn params_rhai_reads_back_as_the_definitions() {
        use crate::params::Spec;
        let defs = [
            ParamDef::new("a", "A", "Free \"quoted\".", Spec::float(1.0, 0.01, 5.0).free()),
            ParamDef::new("b", "B", "", Spec::float(0.5, 1e-6, 1.0)),
            ParamDef::new("c", "C", "Log.", Spec::float(0.1, 0.001, 10.0).log()),
            ParamDef::new("d", "D", "", Spec::int(3, -20, 20).free()),
            ParamDef::new("e", "E", "", Spec::int(2, 1, 9)),
            ParamDef::new("f", "F", "", Spec::angle(30.0, -360.0, 360.0)),
            ParamDef::new("g", "G", "", Spec::bool(true)),
            ParamDef::new("h", "H", "", Spec::choice(1, &["One", "Two"])),
        ];
        let src = format!("{}const INFO = #{{ name: \"rhai_params_back\" }};\nfn variation(z, p) {{ z }}\n", params_rhai(&defs));
        let p = crate::lang::rhai::transformation(&src).unwrap_or_else(|d| panic!("{src}\n{:?}", d.iter().map(|x| x.message.clone()).collect::<Vec<_>>()));
        let back = p.param_defs(crate::lang::ir::ComplexParams::Split("_")).unwrap();
        for (t, b) in back.iter().zip(&defs) {
            assert_eq!((&t.name, &t.label, &t.hint, &t.kind), (&b.name, &b.label, &b.hint, &b.kind), "{}", b.name);
        }
        assert_eq!(back.len(), defs.len());
    }

    /// A tool for the twins' speed: each twin's flame dispatched against its
    /// built-in's -- the density compare's flame at 1024x1024, its
    /// variation's transform weighted 8 to the linears' 1 so most
    /// iterations run it -- on one renderer, the two loaded in turn, seven
    /// rounds of sixteen small dispatches, the median of the rounds'
    /// ratios; the slowest printed last. A twin whose one dispatch takes
    /// over 250 ms is skipped, short of the driver's watchdog.
    /// `VARIATION_TWINS=a,b` times those alone; `VARIATION_TWINS_SELF=1`
    /// times each built-in against itself, the measure's noise;
    /// `VARIATION_TWINS_3D=1` times every twin in a 3D flame, its 3D body
    /// (else a 4D twin's alone). `TWIN_WGSL=dir` runs a twin's WGSL from `dir/<name>-2d.wgsl` and
    /// `-3d.wgsl` in place of its compiled code, to try a change of the
    /// compiler's by its timing before making it (`TWIN_WGSL_WRITE=1`
    /// writes the compiled code there first, to start from). Measured,
    /// not held: a twin is what an edit starts from, and the built-in
    /// still draws an unedited one. Ignored: it times the GPU (the app
    /// closed).
    #[test]
    #[ignore = "times the GPU"]
    fn time_variation_twins() {
        const N: u32 = 1024;
        // Small dispatches, far under the driver's 2 s watchdog: a slow
        // variant must not reset the GPU (one did, taking the editor down,
        // 2026-11-06 -- a 370 ms dispatch made slower).
        const GROUPS: u32 = 64;
        const IPT: u32 = 256;
        let (device, queue) = pollster::block_on(crate::probe::run::open_device("variation twin timing")).expect("a GPU");
        let itself = std::env::var("VARIATION_TWINS_SELF").is_ok_and(|v| v == "1");
        let all_3d = std::env::var("VARIATION_TWINS_3D").is_ok_and(|v| v == "1");
        let wgsl_dir = std::env::var("TWIN_WGSL").ok().map(std::path::PathBuf::from);
        let write_wgsl = std::env::var("TWIN_WGSL_WRITE").is_ok_and(|v| v == "1");
        let wait = || {
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
        };
        let dispatch = |r: &mut crate::renderer::FlameRenderer, cfg: &crate::config::FractalConfig| {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("timed") });
            r.compute_pass(
                &mut enc, &queue, &device, GROUPS, IPT, 20, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, cfg.rotation, cfg.camera_rotation_x,
                cfg.camera_rotation_y, cfg.camera_bank, cfg.camera_x, cfg.camera_y, cfg.camera_z, cfg.speed_factor, true, false,
            );
            queue.submit(Some(enc.finish()));
        };
        let mut timed = Vec::new();
        for (name, _) in chosen_twins() {
            let three = super::super::global_registry().get(name).is_some_and(|v| v.has_feature(super::super::Feature::NeedsW));
            let twin = if itself { flame_coloured(name, None, 0.0) } else { flame_coloured(&key(name).unwrap(), Some(super::carried(name).unwrap()), 0.0) };
            let cfgs = [flame_coloured(name, None, 0.0), twin].map(|mut cfg| {
                cfg.deterministic_rng = false;
                cfg.flame.transforms[0].weight = 8.0;
                // `VARIATION_TWINS_PARAMS`, as the density gate takes it.
                let var = cfg.flame.transforms[0].variations.keys().next().cloned().unwrap_or_default();
                for kv in std::env::var("VARIATION_TWINS_PARAMS").unwrap_or_default().split(',') {
                    if let Some((k, v)) = kv.split_once('=').and_then(|(k, v)| Some((k, v.parse::<f32>().ok()?))) {
                        cfg.flame.transforms[0].variation_params.insert(format!("{var}.{k}"), v);
                    }
                }
                if three || all_3d {
                    cfg.render_mode = crate::scene::transforms::RenderMode::ThreeD;
                }
                cfg
            });
            // One renderer for both, each batch loading its flame: two
            // renderers' buffers land apart in memory, and an atomic-bound
            // flame's speed is its buffers' (the same built-in ran 82 to
            // 194 ms by instance, in creation order, run after run).
            // The twin's code from the files, registered over its own.
            if let (Some(dir), false) = (&wgsl_dir, itself) {
                let carried = super::carried(name).unwrap();
                let mut dl = super::super::custom::compile_as(&carried.source, &carried.entry).unwrap();
                let files = [(format!("{name}-2d.wgsl"), &mut dl.shader_2d), (format!("{name}-3d.wgsl"), &mut dl.shader_3d)];
                for (file, shader) in files {
                    let path = dir.join(file);
                    if write_wgsl {
                        std::fs::create_dir_all(dir).unwrap();
                        std::fs::write(&path, shader.as_deref().unwrap_or_default()).unwrap();
                    } else if let Ok(text) = std::fs::read_to_string(&path) {
                        *shader = Some(text);
                        println!("{name}: its {} from {}", if path.to_string_lossy().ends_with("2d.wgsl") { "2D body" } else { "3D body" }, path.display());
                    }
                }
                super::super::custom::ensure(&cfgs[1].flame);
                super::super::custom::register_into(&mut super::super::global_registry_mut(), &dl, &carried.source).unwrap();
            }
            let mut r = crate::renderer::FlameRenderer::with_palette_size(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, N, N, &cfgs[0].flame, cfgs[0].palette_size);
            // One dispatch of each first: past 250 ms, too near the
            // watchdog to time.
            let slow = (0..2).find_map(|i| {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("probe") });
                r.load_config(&device, &mut enc, &queue, &cfgs[i], &cfgs[i].palette, 1, 0);
                queue.submit(Some(enc.finish()));
                // The first dispatch also builds the pipeline: the second
                // is the one timed.
                dispatch(&mut r, &cfgs[i]);
                wait();
                let start = web_time::Instant::now();
                dispatch(&mut r, &cfgs[i]);
                wait();
                let ms = start.elapsed().as_secs_f64() * 1e3;
                (ms > 250.0).then_some((i, ms))
            });
            if let Some((i, ms)) = slow {
                println!("{name:28} skipped: one {} dispatch took {ms:.0} ms", if i == 0 { "built-in" } else { "twin" });
                continue;
            }
            let mut ratios = Vec::new();
            let mut ms = [0.0f64; 2];
            for round in 0..7 {
                for i in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
                    r.load_config(&device, &mut enc, &queue, &cfgs[i], &cfgs[i].palette, 1, 0);
                    queue.submit(Some(enc.finish()));
                    r.restart_orbits();
                    for _ in 0..4 {
                        dispatch(&mut r, &cfgs[i]);
                    }
                    wait();
                    let start = web_time::Instant::now();
                    for _ in 0..16 {
                        dispatch(&mut r, &cfgs[i]);
                    }
                    wait();
                    ms[i] = start.elapsed().as_secs_f64() * 1e3;
                }
                ratios.push(ms[1] / ms[0]);
            }
            ratios.sort_by(|a, b| a.total_cmp(b));
            let ratio = ratios[ratios.len() / 2];
            println!("{name:28} built-in {:8.2} ms   twin {:8.2} ms   median ratio {ratio:.3}", ms[0], ms[1]);
            timed.push((*name, ratio));
        }
        timed.sort_by(|a, b| a.1.total_cmp(&b.1));
        if timed.is_empty() {
            return;
        }
        let at = |q: f64| timed[((timed.len() - 1) as f64 * q).round() as usize].1;
        let over = |x: f64| timed.iter().filter(|t| t.1 > x).count();
        println!(
            "{} twins: median ratio {:.3}, quartiles {:.3} .. {:.3}; {} over 1.05, {} over 1.25, {} over 2; fastest {} {:.3}",
            timed.len(),
            at(0.5),
            at(0.25),
            at(0.75),
            over(1.05),
            over(1.25),
            over(2.0),
            timed[0].0,
            timed[0].1
        );
        for (name, ratio) in timed.iter().rev().take(20) {
            println!("  slowest: {name:28} {ratio:.3}");
        }
    }
}
