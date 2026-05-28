//! hexnix3D (Larry Berlin, September 2009)
//!
//! Sister of `hexaplay3D` — built specifically for animated flame
//! designing. Adds:
//!  - `smooth` factor: when |weight| ≤ 0.5, smoothly fades between
//!    pass-through and full hex effect.
//!  - Three majplane modes (0=single plate, 1=transition, 2=split
//!    planes) instead of hexaplay's two, with extra negative-majp
//!    handling that flips Z for animation transitions.
//!  - `3side` parameter scaling the inscribed triangle independently.
//!  - Different vertex layouts: seg60y signs are mirrored vs hexaplay
//!    and seg120 is rotated 30° via cos(7π/6)/cos(11π/6).
//!  - Random vertex selection (vs hexaplay's sequential), reducing
//!    artifact-displaced look.
//!  - X/Y formula uses a rotation-and-blend mixing FPx/FPy with FTx/FTy
//!    rotated by the chosen vertex (rather than hexaplay's pure
//!    replacement-with-vertex-offset).
//!
//! 4 user params (majp, scale, zlift, 3side) + 3 state slots (rswtch,
//! fcycle, bcycle). Custom thread-init seeds rswtch from rng.
//!
//! cpp uses replacement-style FPx/FPy/FPz writes; we use the
//! `(desired - accum) / weight` workaround with `needs_accum` and
//! `needs_transform`.
//!
//! Source: `output/jwildfire-vars/output/hexnix3D.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Animation-friendly sister of `hexaplay3D`. Adds: a `smooth` factor that
/// fades between pass-through and full hex effect when `|weight| ≤ 0.5`;
/// three majplane modes (`|majp| ≤ 1` single plate / `1 < |majp| < 2`
/// transition / `|majp| ≥ 2` split planes with boost `(|majp| - 2) · 0.5`)
/// with extra negative-`majp` branches that flip Z for animation
/// transitions; a `3side` parameter that scales the inscribed triangle
/// vertices independently; randomized (rather than sequential) vertex
/// selection to reduce stepping artifacts; and a rotation-and-blend X/Y
/// formula (rather than hexaplay's translate-to-vertex). Vertex layouts
/// also differ — `seg60` y-signs mirrored and `seg120` rotated 30°.
///
/// # Authors
/// - Larry Berlin
pub static HEXNIX_3D: VariationDef = VariationDef {
    name: "hexnix3D",
    aliases: &[],
    display_name: "Hexnix 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("majp", "Major Plane", unlimited_float, 1.0, -10.0, 10.0, "Major-plane threshold for Z behavior (3 modes based on `|majp|`): ≤ 1 = single plate (Z from `zlift` only); 1 < |majp| < 2 = transition mode (with extra negative-`majp` Z-flip branches for animation); ≥ 2 = split planes with boost = `(|majp| - 2) · 0.5`. Negative `majp` adds Z-flip branches in modes 1 and 2 that mirror the accumulator across Z=0 for animation transitions. Unused in 2D mode."),
        param!("scale", "Scale", unlimited_float, 0.25, -10.0, 10.0, "Input-blend scale for the rotation-and-blend X/Y formula. The body mixes `(accum + p)` rotated by the chosen vertex angle with the vertex itself, scaled by `scale`."),
        param!("zlift", "Z Lift", unlimited_float, 0.0, -10.0, 10.0, "Z input scale. Combined with `scale` and `p.z` to set the baseline Z output. Unused in 2D mode."),
        param!("3side", "Triangle Scale", unlimited_float, 0.667, -10.0, 10.0, "Inscribed-triangle vertex scale. Multiplies the 3-vertex (triangle) branch's `weight · vertex` term, letting the triangle and hexagon contributions be scaled independently. Hexagon branch ignores this param."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 3,
    wgsl_state_init: Some(
        "        let r = rng_nextf(&rng);\n\
         \x20       set_state(xform_id, variation_id, 0u, floor(r * 3.0));\n\
         \x20       set_state(xform_id, variation_id, 1u, 0.0);\n\
         \x20       set_state(xform_id, variation_id, 2u, 0.0);"
    ),
    needs_accum: true,
    wgsl_2d: r#"
fn hexnix_seg60(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(0.5, -hlift); }
        case 2u: { return vec2<f32>(-0.5, -hlift); }
        case 3u: { return vec2<f32>(-1.0, 0.0); }
        case 4u: { return vec2<f32>(-0.5, hlift); }
        default: { return vec2<f32>(0.5, hlift); }
    }
}

fn hexnix_seg120(loc: u32) -> vec2<f32> {
    // cos(7π/6) = -√3/2 ≈ -0.86602540, cos(11π/6) = √3/2 ≈ 0.86602540
    let c76 = -0.86602540378443864;
    let c116 = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(0.0, -1.0); }
        case 1u: { return vec2<f32>(c76, 0.5); }
        default: { return vec2<f32>(c116, 0.5); }
    }
}

fn variation_hexnix3D(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let scale = get_param(xform_id, variation_id, 1u);
    let scale3 = get_param(xform_id, variation_id, 3u);

    var rswtch = get_state(xform_id, variation_id, 0u);
    var fcycle = get_state(xform_id, variation_id, 1u);
    var bcycle = get_state(xform_id, variation_id, 2u);

    if (fcycle > 5.0) {
        fcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }
    if (bcycle > 2.0) {
        bcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);
    var sm = 1.0;
    if (abs(weight) <= 0.5) { sm = weight * 2.0; }

    var ox = 0.0;
    var oy = 0.0;
    if (rswtch <= 1.0) {
        let loc = u32(floor(rng_nextf(rng) * 6.0));
        let v = hexnix_seg60(min(loc, 5u));
        // desired_x = accum.x*(1-smooth) + smooth*[scale*((accum.x+p.x)*v.x - (accum.y+p.y)*v.y) + weight*v.x]
        // body = (desired - accum.x) / weight
        let dx = accum.x * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.x - (accum.y + p.y) * v.y) + weight * v.x);
        let dy = accum.y * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.y + (accum.y + p.y) * v.x) + weight * v.y);
        ox = (dx - accum.x) / safe_w;
        oy = (dy - accum.y) / safe_w;
        fcycle = fcycle + 1.0;
    } else {
        let loc = u32(floor(rng_nextf(rng) * 3.0));
        let v = hexnix_seg120(min(loc, 2u));
        let dx = accum.x * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.x - (accum.y + p.y) * v.y) + weight * scale3 * v.x);
        let dy = accum.y * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.y + (accum.y + p.y) * v.x) + weight * scale3 * v.y);
        ox = (dx - accum.x) / safe_w;
        oy = (dy - accum.y) / safe_w;
        bcycle = bcycle + 1.0;
    }

    set_state(xform_id, variation_id, 0u, rswtch);
    set_state(xform_id, variation_id, 1u, fcycle);
    set_state(xform_id, variation_id, 2u, bcycle);
    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: Some(r#"
fn hexnix_seg60_3d(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(0.5, -hlift); }
        case 2u: { return vec2<f32>(-0.5, -hlift); }
        case 3u: { return vec2<f32>(-1.0, 0.0); }
        case 4u: { return vec2<f32>(-0.5, hlift); }
        default: { return vec2<f32>(0.5, hlift); }
    }
}

fn hexnix_seg120_3d(loc: u32) -> vec2<f32> {
    let c76 = -0.86602540378443864;
    let c116 = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(0.0, -1.0); }
        case 1u: { return vec2<f32>(c76, 0.5); }
        default: { return vec2<f32>(c116, 0.5); }
    }
}

fn variation_hexnix3D(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let majp = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);
    let zlift = get_param(xform_id, variation_id, 2u);
    let scale3 = get_param(xform_id, variation_id, 3u);

    var rswtch = get_state(xform_id, variation_id, 0u);
    var fcycle = get_state(xform_id, variation_id, 1u);
    var bcycle = get_state(xform_id, variation_id, 2u);

    if (fcycle > 5.0) {
        fcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }
    if (bcycle > 2.0) {
        bcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);
    var sm = 1.0;
    if (abs(weight) <= 0.5) { sm = weight * 2.0; }

    var pos_neg = 1.0;
    if (rng_nextf(rng) < 0.5) { pos_neg = -1.0; }

    // 3-mode majplane logic
    let abmajp = abs(majp);
    var majplane: i32 = 0;
    var boost = 0.0;
    if (abmajp <= 1.0) {
        majplane = 0;
    } else if (abmajp < 2.0) {
        majplane = 1;
    } else {
        majplane = 2;
        boost = (abmajp - 2.0) * 0.5;
    }

    // Z computation. cpp branches:
    //  majplane=0: FPz += smooth*p.z*scale*zlift
    //  majplane=1 + majp<0 + posNeg<0: FPz = -2*smooth*FPz*gentleZ + FPz (replacement)
    //  majplane=2 + majp<0 + posNeg>0: FPz += smooth*(p.z*scale*zlift + boost)
    //  majplane=2 + majp<0 + posNeg<0: FPz = (FPz - 2*smooth*FPz) + smooth*posNeg*(p.z*scale*zlift+boost)
    //  else (majp >= 0): FPz += smooth*(p.z*scale*zlift + posNeg*boost)
    var oz = 0.0;
    if (majplane == 0) {
        oz = sm * p.z * scale * zlift / safe_w;
    } else if (majplane == 1 && majp < 0.0) {
        var gentle_z = 1.0;
        if (majp < -1.0 && majp >= -2.0) { gentle_z = abmajp - 1.0; }
        if (pos_neg < 0.0) {
            // desired = accum.z + (-2 * accum.z * gentle_z) = accum.z * (1 - 2*gentle_z)
            let desired = accum.z * (1.0 - 2.0 * gentle_z);
            oz = (desired - accum.z) / safe_w;
        }
    } else if (majplane == 2 && majp < 0.0) {
        if (pos_neg > 0.0) {
            oz = sm * (p.z * scale * zlift + boost) / safe_w;
        } else {
            // desired = (accum.z - 2*smooth*accum.z) + smooth*posNeg*(p.z*scale*zlift+boost)
            let desired = accum.z * (1.0 - 2.0 * sm)
                        + sm * pos_neg * (p.z * scale * zlift + boost);
            oz = (desired - accum.z) / safe_w;
        }
    } else {
        oz = sm * (p.z * scale * zlift + pos_neg * boost) / safe_w;
    }

    var ox = 0.0;
    var oy = 0.0;
    if (rswtch <= 1.0) {
        let loc = u32(floor(rng_nextf(rng) * 6.0));
        let v = hexnix_seg60_3d(min(loc, 5u));
        let dx = accum.x * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.x - (accum.y + p.y) * v.y) + weight * v.x);
        let dy = accum.y * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.y + (accum.y + p.y) * v.x) + weight * v.y);
        ox = (dx - accum.x) / safe_w;
        oy = (dy - accum.y) / safe_w;
        fcycle = fcycle + 1.0;
    } else {
        let loc = u32(floor(rng_nextf(rng) * 3.0));
        let v = hexnix_seg120_3d(min(loc, 2u));
        let dx = accum.x * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.x - (accum.y + p.y) * v.y) + weight * scale3 * v.x);
        let dy = accum.y * (1.0 - sm)
               + sm * (scale * ((accum.x + p.x) * v.y + (accum.y + p.y) * v.x) + weight * scale3 * v.y);
        ox = (dx - accum.x) / safe_w;
        oy = (dy - accum.y) / safe_w;
        bcycle = bcycle + 1.0;
    }

    set_state(xform_id, variation_id, 0u, rswtch);
    set_state(xform_id, variation_id, 1u, fcycle);
    set_state(xform_id, variation_id, 2u, bcycle);
    return vec3<f32>(ox, oy, oz);
}
"#),
};
