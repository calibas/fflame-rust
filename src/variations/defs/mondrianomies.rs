//! `mondrianomies` — L-system Mondrian skeletons, after Antonio
//! Sánchez Chinchón's "the-mondrianomies" R project
//! (<https://github.com/aschinchon/the-mondrianomies>, `mondrianomies.R`).
//!
//! An L-system (Lindenmayer 1968) turtle drawing with the source's
//! exact grammar: axiom `F-F-F-F`, turning angle 90°, and ONE random
//! rule of 15–26 symbols drawn from {F, +, −} with weights 10/12/12,
//! into which three balanced (non-nested) `[` `]` pairs are inserted
//! at distinct sorted positions. The rule substitutes `F` `depth`
//! times (source: 3 or 4). Segment length is `drift^d` where `d`
//! counts segments drawn so far and is saved/restored by the bracket
//! stack (source: `ds = jitter(1)`, i.e. ≈1). The result is the
//! rectilinear scaffolding of a Mondrian painting; use a Mondrian
//! palette (white / black / red / yellow / blue) with the Sequence
//! color mode for the authentic look.
//!
//! GPU adaptation — the R script draws sequentially and then runs a
//! relational pass to find and fill enclosed rectangles; neither fits
//! a per-thread chaos game, so:
//! - **Uniform derivation-tree descent** instead of sequential
//!   drawing: every level-k `F` expands to the same rule, so its net
//!   effect is a composable turtle transform (displacement `drift^d ·
//!   R(angle) · v_k`, net angle `A_k`, net segment-count advance
//!   `m_k`, all precomputed per level in `wgsl_init`). A thread picks
//!   one of the `4·f^depth` segments uniformly by choosing a random
//!   branch at each level and composing prefix transforms — ~depth ·
//!   |rule| steps, no expansion stored.
//! - The rectangle-fill pass is omitted (it is relational
//!   post-processing over the whole segment set); the variation
//!   renders the line skeleton and the flame palette supplies the
//!   Mondrian colors.
//! - The rule is generated deterministically from the Seed param (the
//!   R script uses session randomness). Bracket insertion is clean;
//!   the R insertion loop `c(v1[1:k], b, v1[k:n])` duplicates one
//!   symbol per insertion, which we do not reproduce — either way the
//!   rule is a random string, but seeds are not comparable to R runs.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Mondrianomies: random 90°-L-system (axiom F-F-F-F, one random
/// bracketed rule) sampled by uniform derivation-tree descent —
/// Mondrian-painting scaffolding from a seed.
///
/// # Authors
/// - Antonio Sánchez Chinchón (the-mondrianomies R project)
/// - Claude Fable 5
pub static MONDRIANOMIES: VariationDef = VariationDef {
    name: "mondrianomies",
    aliases: &[],
    display_name: "Mondrianomies",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    parameters: &[
        param!("seed", "Seed", int, 1.0, 0.0, 99999.0, "Seed for the random rule. Each seed is a different Mondrianomy: rule length 15-26, symbols from {F, +, -} with the source's 10/12/12 weights, three balanced bracket pairs at random positions."),
        param!("depth", "Depth", int, 3.0, 1.0, 5.0, "How many times the rule substitutes F, starting from the axiom F-F-F-F (the R source picks 3 or 4). Segment count multiplies by the rule's F-count per level."),
        param!("drift", "Length Drift", float, 1.0, 0.9, 1.1, "Segment length = drift^d, where d counts segments drawn before this one (saved/restored with the bracket stack) — the source's ds = jitter(1). Exactly 1 gives the pure unit grid; small deviations make the grid drift and shear apart exponentially with drawing order."),
        param!("size", "Size", float, 0.1, 0.001, 2.0, "Output scale. Drawings span tens of units at unit segment length; 0.1 fits typical seeds near the frame."),
        param!("thickness", "Thickness", float, 0.0, 0.0, 0.5, "Jitter radius fattening the line skeleton into strokes (in un-scaled drawing units)."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Sequence", "Depth", "Angle"], "Direct-color source. Sequence: palette position follows drawing order (the R script cycles its five Mondrian colors by rectangle id — use DC Scale for the cycle count). Depth: colors by the segment counter d (the length-drift driver). Angle: four flat colors by segment direction."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "Depth mode: palette advance per drawn segment (cyclic — wraps instead of saturating)."),
        param!("dc_scale", "DC Scale", float, 1.0, 0.0, 20.0, "Sequence mode: how many palette cycles across the whole drawing. Depth/Angle: extra multiplier on the palette position."),
        param!("center", "Center", bool, true, "Recenter using the mean of the four axiom waypoints (the drawing starts at the turtle origin and wanders)."),
    ],
    // Derived layout (base = 9 user params):
    //   +0        rule length (≤ 32)
    //   +1..+34   rule symbols: 0=F 1=+ 2=- 3=[ 4=]
    //   +35       F-count in the rule
    //   +36+4k..  per level k=0..5: v_k.x, v_k.y, A_k, m_k
    //             (net displacement / angle quarter-turns / segment
    //             count of a level-k F starting from angle 0, d 0)
    //   +60,+61   center offset (mean of axiom waypoints at `depth`)
    init_param_count: 62,
    wgsl_init: Some(WGSL_INIT),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_INIT: &str = r#"
fn mnd_next(st: ptr<function, u32>) -> f32 {
    *st = *st * 1664525u + 1013904223u;
    return f32((*st >> 8u) & 0xFFFFFFu) / 16777216.0;
}

fn mnd_rot_i(v: vec2<f32>, ang: u32) -> vec2<f32> {
    if (ang == 1u) { return vec2<f32>(-v.y, v.x); }
    if (ang == 2u) { return -v; }
    if (ang == 3u) { return vec2<f32>(v.y, -v.x); }
    return v;
}

fn init_mondrianomies(user: array<f32, 9>) -> array<f32, 62> {
    var out: array<f32, 62>;
    let depth = clamp(u32(user[1]), 1u, 5u);
    let ds = clamp(user[2], 0.5, 2.0);
    let lnds = log(ds);

    // ---- generate the rule from the seed ----
    var st: u32 = u32(max(user[0], 0.0)) * 0x9E3779B9u + 0x85EBCA6Bu;
    st = st ^ (st >> 16u);
    let s = 15u + u32(mnd_next(&st) * 11.999);          // 15..26 symbols
    var base: array<u32, 26>;
    for (var i = 0u; i < s; i = i + 1u) {
        // F : + : - with the source's 10 : 12 : 12 weights.
        let r = mnd_next(&st) * 34.0;
        base[i] = select(select(2u, 1u, r < 22.0), 0u, r < 10.0);
    }
    // Six distinct insertion positions in 0..s, sorted -> three
    // balanced, non-nested [ ] pairs in reading order.
    var posn: array<u32, 6>;
    for (var i = 0u; i < 6u; i = i + 1u) {
        var pp = 0u;
        var tries = 0u;
        loop {
            pp = min(u32(mnd_next(&st) * f32(s + 1u)), s);
            var dup = false;
            for (var j = 0u; j < i; j = j + 1u) {
                if (posn[j] == pp) { dup = true; }
            }
            tries = tries + 1u;
            if (!dup || tries > 64u) { break; }
        }
        posn[i] = pp;
    }
    for (var i = 1u; i < 6u; i = i + 1u) {              // insertion sort
        let v = posn[i];
        var j = i;
        while (j > 0u && posn[j - 1u] > v) { posn[j] = posn[j - 1u]; j = j - 1u; }
        posn[j] = v;
    }
    var rule: array<u32, 32>;
    var rl = 0u;
    var b = 0u;
    for (var i = 0u; i <= s; i = i + 1u) {
        while (b < 6u && posn[b] == i) {
            rule[rl] = 3u + (b & 1u);                   // [, ], [, ], [, ]
            rl = rl + 1u;
            b = b + 1u;
        }
        if (i < s) { rule[rl] = base[i]; rl = rl + 1u; }
    }
    // A rule with no F draws nothing after one substitution — force one
    // (replace the first non-bracket symbol to keep the pairs balanced).
    var ftot = 0u;
    for (var i = 0u; i < rl; i = i + 1u) {
        if (rule[i] == 0u) { ftot = ftot + 1u; }
    }
    if (ftot == 0u) {
        for (var i = 0u; i < rl; i = i + 1u) {
            if (rule[i] < 3u) { rule[i] = 0u; break; }
        }
        ftot = 1u;
    }

    out[0] = f32(rl);
    for (var i = 0u; i < 34u; i = i + 1u) {
        out[1u + i] = f32(select(0u, rule[i], i < rl));
    }
    out[35] = f32(ftot);

    // ---- per-level net transforms ----
    // T_0: a bare segment. v=(1,0), net angle 0, advances d by 1.
    out[36] = 1.0; out[37] = 0.0; out[38] = 0.0; out[39] = 1.0;
    for (var k = 1u; k <= depth; k = k + 1u) {
        let bl = 36u + (k - 1u) * 4u;
        let vk = vec2<f32>(out[bl], out[bl + 1u]);
        let ak = u32(out[bl + 2u]);
        let mk = out[bl + 3u];
        var pp = vec2<f32>(0.0, 0.0);
        var ang = 0u;
        var d = 0.0;
        var sp = vec2<f32>(0.0, 0.0);
        var sang = 0u;
        var sd = 0.0;
        for (var i = 0u; i < rl; i = i + 1u) {
            let sym = rule[i];
            if (sym == 0u) {
                let sc = exp(clamp(d * lnds, -12.0, 12.0));
                pp = pp + sc * mnd_rot_i(vk, ang);
                ang = (ang + ak) & 3u;
                d = d + mk;
            } else if (sym == 1u) { ang = (ang + 3u) & 3u; }    // + = right
            else if (sym == 2u) { ang = (ang + 1u) & 3u; }      // - = left
            else if (sym == 3u) { sp = pp; sang = ang; sd = d; }
            else { pp = sp; ang = sang; d = sd; }
        }
        out[36u + k * 4u] = pp.x;
        out[37u + k * 4u] = pp.y;
        out[38u + k * 4u] = f32(ang);
        out[39u + k * 4u] = d;
    }

    // ---- center: mean of the five axiom waypoints ----
    let bl = 36u + depth * 4u;
    let vk = vec2<f32>(out[bl], out[bl + 1u]);
    let ak = u32(out[bl + 2u]);
    let mk = out[bl + 3u];
    var pp = vec2<f32>(0.0, 0.0);
    var ang = 0u;
    var d = 0.0;
    var sum = vec2<f32>(0.0, 0.0);
    for (var j = 0u; j < 4u; j = j + 1u) {
        let sc = exp(clamp(d * lnds, -12.0, 12.0));
        pp = pp + sc * mnd_rot_i(vk, ang);
        ang = (ang + ak + 1u) & 3u;                     // F then '-'
        d = d + mk;
        sum = sum + pp;
    }
    out[60] = sum.x / 5.0;
    out[61] = sum.y / 5.0;
    return out;
}
"#;

// Body: uniform random descent through the derivation tree. State is
// the turtle (position, quarter-turn angle, segment counter d); a full
// level-k F composes as p += drift^d · R(ang)·v_k, ang += A_k, d += m_k.
// Brackets need only depth-1 save slots per level (the pairs are
// balanced and non-nested within the rule). Shared between the 2D and
// 3D bodies via macro + concat! (only one is compiled per flame).
macro_rules! mnd_body {
    () => {
        r#"
fn mnd_rot(v: vec2<f32>, ang: u32) -> vec2<f32> {
    if (ang == 1u) { return vec2<f32>(-v.y, v.x); }
    if (ang == 2u) { return -v; }
    if (ang == 3u) { return vec2<f32>(v.y, -v.x); }
    return v;
}

fn mnd_point(xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let bo = 9u;
    let depth = clamp(u32(get_param(xform_id, variation_id, 1u)), 1u, 5u);
    let ds = clamp(get_param(xform_id, variation_id, 2u), 0.5, 2.0);
    let size = get_param(xform_id, variation_id, 3u);
    let thickness = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let color_speed = get_param(xform_id, variation_id, 6u);
    let dc_scale = get_param(xform_id, variation_id, 7u);
    let centered = get_param(xform_id, variation_id, 8u) > 0.5;
    let rl = u32(get_param(xform_id, variation_id, bo));
    let ftot = max(u32(get_param(xform_id, variation_id, bo + 35u)), 1u);
    let lnds = log(ds);

    var pp = vec2<f32>(0.0, 0.0);
    var ang = 0u;
    var d = 0.0;
    // Tree position in drawing order (for the Sequence color mode):
    // string order within the rule IS drawing order, so the mixed-radix
    // digits (i0, t_k...) index segments in the order the pen visits them.
    var bpos = 0.0;
    var bden = 1.0;

    // Axiom F-F-F-F: pick which of the four arms, compose the prefix.
    let i0 = min(u32(rng_nextf(rng) * 4.0), 3u);
    bpos = f32(i0) * 0.25;
    bden = 4.0;
    {
        let bl = bo + 36u + depth * 4u;
        let vk = vec2<f32>(get_param(xform_id, variation_id, bl), get_param(xform_id, variation_id, bl + 1u));
        let ak = u32(get_param(xform_id, variation_id, bl + 2u));
        let mk = get_param(xform_id, variation_id, bl + 3u);
        for (var j = 0u; j < i0; j = j + 1u) {
            let sc = exp(clamp(d * lnds, -12.0, 12.0));
            pp = pp + sc * mnd_rot(vk, ang);
            ang = (ang + ak + 1u) & 3u;                 // F then '-' (left)
            d = d + mk;
        }
    }

    // Descend: at each level pick a uniform target F among the rule's
    // F's and compose the prefix before it; the state lands at the
    // start of that F's expansion. Level 1's target is the segment.
    for (var k = depth; k >= 1u; k = k - 1u) {
        let tf = min(u32(rng_nextf(rng) * f32(ftot)), ftot - 1u);
        bpos = bpos + f32(tf) / (bden * f32(ftot));
        bden = bden * f32(ftot);
        let bl = bo + 36u + (k - 1u) * 4u;
        let vk = vec2<f32>(get_param(xform_id, variation_id, bl), get_param(xform_id, variation_id, bl + 1u));
        let ak = u32(get_param(xform_id, variation_id, bl + 2u));
        let mk = get_param(xform_id, variation_id, bl + 3u);
        var sp = pp;
        var sang = ang;
        var sd = d;
        var cnt = 0u;
        for (var i = 0u; i < rl; i = i + 1u) {
            let sym = u32(get_param(xform_id, variation_id, bo + 1u + i));
            if (sym == 0u) {
                if (cnt == tf) { break; }
                let sc = exp(clamp(d * lnds, -12.0, 12.0));
                pp = pp + sc * mnd_rot(vk, ang);
                ang = (ang + ak) & 3u;
                d = d + mk;
                cnt = cnt + 1u;
            } else if (sym == 1u) { ang = (ang + 3u) & 3u; }
            else if (sym == 2u) { ang = (ang + 1u) & 3u; }
            else if (sym == 3u) { sp = pp; sang = ang; sd = d; }
            else { pp = sp; ang = sang; d = sd; }
        }
    }

    // Draw the level-0 segment: from pp, direction ang, length drift^d.
    let len = exp(clamp(d * lnds, -12.0, 12.0));
    let t = rng_nextf(rng);
    var q = pp + mnd_rot(vec2<f32>(len * t, 0.0), ang);

    if (dc_mode == 1u) {
        *vc = fract(bpos * max(dc_scale, 1e-6));
    } else if (dc_mode == 2u) {
        // Cyclic in the segment counter (the length-drift driver).
        *vc = fract(d * color_speed * 0.1 * max(dc_scale, 1e-6));
    } else if (dc_mode == 3u) {
        *vc = fract((f32(ang) + 0.5) * 0.25 * max(dc_scale, 1e-6));
    }

    if (thickness > 0.0) {
        q = q + thickness * vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * 2.0;
    }
    if (centered) {
        q = q - vec2<f32>(get_param(xform_id, variation_id, bo + 60u), get_param(xform_id, variation_id, bo + 61u));
    }
    return q * size;
}
"#
    };
}

const WGSL_2D: &str = concat!(
    mnd_body!(),
    r#"
fn variation_mondrianomies(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    return mnd_point(xform_id, variation_id, rng, vc);
}
"#
);

const WGSL_3D: &str = concat!(
    mnd_body!(),
    r#"
fn variation_mondrianomies(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    return vec3<f32>(mnd_point(xform_id, variation_id, rng, vc), p.z);
}
"#
);
