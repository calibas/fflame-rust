//! `mondrianomies` — L-system Mondrian skeletons, after Antonio
//! Sánchez Chinchón's "the-mondrianomies" R project
//! (<https://github.com/aschinchon/the-mondrianomies>, `mondrianomies.R`).
//!
//! See also: https://fronkonstin.com/2022/03/25/the-mondrianomies/
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
//! - The R rectangle-fill pass is relational post-processing over the
//!   whole segment set (per-thread infeasible); the Fill param is the
//!   chaos-game equivalent: a fill sample runs the descent TWICE and,
//!   when the two segments are parallel, overlapping in span, and
//!   within Fill Span of each other, fills the rectangle
//!   between them — both dimensions come from actually-drawn lines,
//!   so cells are true squares and longer rectangles, bounded by real
//!   lines on two sides (and only sometimes all four), with line
//!   samples drawing over the fills, as in the R output. The Cell
//!   color mode gives each rectangle one flat palette color (lines
//!   drop to palette position 0 — put black there); Fill Inset leaves
//!   the white gutters. The second segment is sampled as a SIBLING of
//!   the first (replaying its descent, re-randomizing only the bottom
//!   level or two) — uniform independent pairs almost never lie
//!   adjacent in a drawing of thousands of segments, siblings usually
//!   do.
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
/// - Antonio Sánchez Chinchón
/// - Fractals for All
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
        param!("drift", "Length Drift", float, 0.98, 0.9, 1.1, "Segment length = drift^d, where d counts segments drawn before this one (saved/restored with the bracket stack) — the source's ds = jitter(1). Exactly 1 gives the pure unit grid; small deviations make the grid drift and shear apart exponentially with drawing order."),
        param!("size", "Size", float, 0.3, 0.001, 2.0, "Output scale. Drawings span tens of units at unit segment length; 0.1 fits typical seeds near the frame."),
        param!("thickness", "Thickness", float, 0.0, 0.0, 0.5, "Jitter radius fattening the line skeleton into strokes (in un-scaled drawing units)."),
        param!("dc_mode", "Color Mode", enum, 4, &["Off", "Sequence", "Depth", "Angle", "Cell"], "Direct-color source. Sequence: palette position follows drawing order (the R script cycles its five Mondrian colors by rectangle id — use DC Scale for the cycle count). Depth: colors by the segment counter d (the length-drift driver). Angle: four flat colors by segment direction. Cell: each filled rectangle takes one flat color from a hash of its bounds — every sample agrees no matter which segment pair spanned it (the Mondrian mode; needs Fill > 0). DC Scale >= 2 quantizes to that many flat palette colors (5 = the classic Mondrian five, matching the R id %% 5); below 2 the hash is continuous. Line samples go to palette position 0 — put black at the palette start for the authentic black-lines-over-colored-rects look."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "Depth mode: palette advance per drawn segment (cyclic — wraps instead of saturating)."),
        param!("dc_scale", "DC Scale", float, 1.0, 0.0, 20.0, "Sequence mode: how many palette cycles across the whole drawing. Depth/Angle: extra multiplier on the palette position. Cell mode: >= 2 quantizes fills to that many flat palette colors (5 = the Mondrian five)."),
        param!("center", "Center", bool, true, "Recenter using the mean of the four axiom waypoints (the drawing starts at the turtle origin and wanders)."),
        param!("fill", "Fill", float, 1.0, 0.0, 1.0, "Fraction of samples that attempt a rectangle fill instead of drawing a line (0 = pure skeleton). A fill picks a SECOND independent segment; if the two are parallel, overlap in span, and sit within Fill Span of each other, the rectangle between them is filled — width = the drawn lines' actual overlap, height = their actual separation, so cells come out as true squares and longer rectangles bounded by real lines (mirroring the R script's detected rectangles). Non-qualifying pairs fall back to drawing a line, so effective fill density also depends on Depth (more segments = more parallel pairs)."),
        param!("inset", "Fill Inset", float, 0.04, 0.0, 0.2, "Shrinks each filled rectangle inward by this fraction of its span, leaving unpainted gutters along the bounding lines — the white borders of a Mondrian canvas."),
        param!("fill_span", "Fill Span", float, 2.0, 1.0, 6.0, "Pairs fill mode: how far apart (in segment lengths) two parallel drawn lines may be and still span a fill. 1 keeps only the tightest cells; larger values admit longer rectangles between more distant lines. Ignored in Exact mode."),
        param!("fill_mode", "Fill Mode", enum, 1, &["Pairs", "Exact (R)"], "How fills are found. Pairs: GPU sibling-pair sampling — a self-assembling square-cell mosaic, no rebuild cost (its own thing, not the R output). Exact (R): the drawing is expanded on the CPU and the R script's relational rectangle pass runs for real — segment endpoints rounded and deduped, rectangles of every aspect ratio found from aligned segment/point combinations, kept only when a drawn line connects them, painter-ordered with overlaps clipped, colored id %% 5 onto five evenly spaced palette stops. The rectangle table is baked into the shader, so changing Seed, Depth, or Length Drift triggers a shader rebuild (same pause as toggling a variation). Depth caps at 4 in Exact mode."),
        param!("line_ink", "Line Ink", float, 0.35, 0.0, 1.0, "Fraction of samples reserved for the line skeleton BEFORE fills are attempted — the line/fill brightness balance. In Exact mode every fill attempt succeeds, so Fill = 1 alone starves the lines to invisibility; in Pairs mode rejected fill attempts already fall back to lines, so this matters less there. The remaining share attempts fills at the Fill probability."),
    ],
    // Derived layout (base = 11 user params):
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

fn init_mondrianomies(user: array<f32, 14>) -> array<f32, 62> {
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
fn mnd_hash01(a: i32, b: i32) -> f32 {
    var h = u32(a) * 0x8da6b343u ^ u32(b) * 0xd8163841u;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 13u);
    return f32(h & 0xFFFFFFu) / 16777216.0;
}

fn mnd_rot(v: vec2<f32>, ang: u32) -> vec2<f32> {
    if (ang == 1u) { return vec2<f32>(-v.y, v.x); }
    if (ang == 2u) { return -v; }
    if (ang == 3u) { return vec2<f32>(v.y, -v.x); }
    return v;
}

// MND_EXACT_STUB_BEGIN
// Replaced by per-flame specialization (specialize_wgsl_2d/_3d below)
// with baked rectangle tables from the CPU run of the R relational
// pass. The stub keeps the static source valid; it reports "no table"
// so Exact mode falls back to the Pairs fill until specialization
// kicks in.
fn mnd_exact_pick(seed: f32, dep: f32, drift: f32, inset: f32, r1: f32, r2: f32, r3: f32) -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, -1.0);
}
// MND_EXACT_STUB_END

// One uniform random segment of the expanded L-system, by descent
// through the derivation tree (see the module docs): at each level
// pick a uniform target F among the rule's F's and compose the prefix
// transforms before it; level 1's target is the drawn segment.
// Returns [p0.x, p0.y, len, angle (0..3), bpos, d] — the shader
// builder's fn-splitter drops top-level structs, so a plain array.
// The rule and level tables come from thread-local caches (read from
// the param buffer once per mnd_point call), and the turtle state at
// the entry of every level is checkpointed into `ckpt` so sibling
// segments can resume mid-descent (mnd_seg_from) instead of paying a
// full descent per retry.
fn mnd_seg_full(rulec: ptr<function, array<u32, 34>>, lvt: ptr<function, array<vec4<f32>, 6>>, ckpt: ptr<function, array<vec4<f32>, 6>>, rng: ptr<function, RngState>, depth: u32, lnds: f32, rl: u32, ftot: u32) -> array<f32, 6> {
    var pp = vec2<f32>(0.0, 0.0);
    var ang = 0u;
    var d = 0.0;
    // Tree position in drawing order (Sequence color mode): string
    // order within the rule IS drawing order, so the mixed-radix
    // digits index segments in the order the pen visits them.
    var bpos = 0.0;
    var bden = 1.0;

    // Axiom F-F-F-F: pick which of the four arms, compose the prefix.
    let i0 = min(u32(rng_nextf(rng) * 4.0), 3u);
    bpos = f32(i0) * 0.25;
    bden = 4.0;
    {
        let lv = (*lvt)[depth];
        for (var j = 0u; j < i0; j = j + 1u) {
            let sc = exp(clamp(d * lnds, -12.0, 12.0));
            pp = pp + sc * mnd_rot(lv.xy, ang);
            ang = (ang + u32(lv.z) + 1u) & 3u;      // F then '-' (left)
            d = d + lv.w;
        }
    }

    for (var k = depth; k >= 1u; k = k - 1u) {
        (*ckpt)[k] = vec4<f32>(pp.x, pp.y, f32(ang), d);
        let tf = min(u32(rng_nextf(rng) * f32(ftot)), ftot - 1u);
        bpos = bpos + f32(tf) / (bden * f32(ftot));
        bden = bden * f32(ftot);
        let lv = (*lvt)[k - 1u];
        var sp = pp;
        var sang = ang;
        var sd = d;
        var cnt = 0u;
        for (var i = 0u; i < rl; i = i + 1u) {
            let sym = (*rulec)[i];
            if (sym == 0u) {
                if (cnt == tf) { break; }
                let sc = exp(clamp(d * lnds, -12.0, 12.0));
                pp = pp + sc * mnd_rot(lv.xy, ang);
                ang = (ang + u32(lv.z)) & 3u;
                d = d + lv.w;
                cnt = cnt + 1u;
            } else if (sym == 1u) { ang = (ang + 3u) & 3u; }
            else if (sym == 2u) { ang = (ang + 1u) & 3u; }
            else if (sym == 3u) { sp = pp; sang = ang; sd = d; }
            else { pp = sp; ang = sang; d = sd; }
        }
    }

    return array<f32, 6>(pp.x, pp.y, exp(clamp(d * lnds, -12.0, 12.0)), f32(ang), bpos, d);
}

// Sibling segment: resume the last full descent from its level-ll
// checkpoint with fresh choices below — the same distribution as
// replaying the recorded prefix, at a fraction of the cost (ll is
// usually 1, so most retries re-run a single level scan).
// Returns [p0.x, p0.y, len, angle].
fn mnd_seg_from(rulec: ptr<function, array<u32, 34>>, lvt: ptr<function, array<vec4<f32>, 6>>, ckpt: ptr<function, array<vec4<f32>, 6>>, rng: ptr<function, RngState>, ll: u32, lnds: f32, rl: u32, ftot: u32) -> array<f32, 4> {
    let c0 = (*ckpt)[ll];
    var pp = vec2<f32>(c0.x, c0.y);
    var ang = u32(c0.z);
    var d = c0.w;
    for (var k = ll; k >= 1u; k = k - 1u) {
        let tf = min(u32(rng_nextf(rng) * f32(ftot)), ftot - 1u);
        let lv = (*lvt)[k - 1u];
        var sp = pp;
        var sang = ang;
        var sd = d;
        var cnt = 0u;
        for (var i = 0u; i < rl; i = i + 1u) {
            let sym = (*rulec)[i];
            if (sym == 0u) {
                if (cnt == tf) { break; }
                let sc = exp(clamp(d * lnds, -12.0, 12.0));
                pp = pp + sc * mnd_rot(lv.xy, ang);
                ang = (ang + u32(lv.z)) & 3u;
                d = d + lv.w;
                cnt = cnt + 1u;
            } else if (sym == 1u) { ang = (ang + 3u) & 3u; }
            else if (sym == 2u) { ang = (ang + 1u) & 3u; }
            else if (sym == 3u) { sp = pp; sang = ang; sd = d; }
            else { pp = sp; ang = sang; d = sd; }
        }
    }
    return array<f32, 4>(pp.x, pp.y, exp(clamp(d * lnds, -12.0, 12.0)), f32(ang));
}

fn mnd_point(xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let bo = 14u;
    let depth = clamp(u32(get_param(xform_id, variation_id, 1u)), 1u, 5u);
    let ds = clamp(get_param(xform_id, variation_id, 2u), 0.5, 2.0);
    let size = get_param(xform_id, variation_id, 3u);
    let thickness = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let color_speed = get_param(xform_id, variation_id, 6u);
    let dc_scale = get_param(xform_id, variation_id, 7u);
    let centered = get_param(xform_id, variation_id, 8u) > 0.5;
    let fillp = get_param(xform_id, variation_id, 9u);
    let inset = clamp(get_param(xform_id, variation_id, 10u), 0.0, 0.45);
    let span = max(get_param(xform_id, variation_id, 11u), 1.0);
    let fill_mode = u32(get_param(xform_id, variation_id, 12u));
    let line_ink = clamp(get_param(xform_id, variation_id, 13u), 0.0, 1.0);
    let rl = u32(get_param(xform_id, variation_id, bo));
    let ftot = max(u32(get_param(xform_id, variation_id, bo + 35u)), 1u);
    let lnds = log(ds);

    // Thread-local caches: the descent and its sibling retries hammer
    // the rule symbols and level tables — reading them once per call
    // turns ~10^3 storage-buffer reads per sample into ~40.
    var rulec: array<u32, 34>;
    for (var i = 0u; i < rl; i = i + 1u) {
        rulec[i] = u32(get_param(xform_id, variation_id, bo + 1u + i));
    }
    var lvt: array<vec4<f32>, 6>;
    for (var k = 0u; k <= depth; k = k + 1u) {
        let bl = bo + 36u + k * 4u;
        lvt[k] = vec4<f32>(
            get_param(xform_id, variation_id, bl),
            get_param(xform_id, variation_id, bl + 1u),
            get_param(xform_id, variation_id, bl + 2u),
            get_param(xform_id, variation_id, bl + 3u));
    }
    var ckpt: array<vec4<f32>, 6>;

    let sa = mnd_seg_full(&rulec, &lvt, &ckpt, rng, depth, lnds, rl, ftot);
    let a_p0 = vec2<f32>(sa[0], sa[1]);
    let a_len = sa[2];
    let a_ang = u32(sa[3]);

    var q = vec2<f32>(0.0, 0.0);
    var filling = false;
    var exact_fill = false;
    var rvc = 0.0;
    // Line Ink reserves the low [0, line_ink) band of the draw for the
    // skeleton; the rest attempts a fill with probability fillp.
    let udraw = rng_nextf(rng);
    if (fillp > 0.0 && udraw >= line_ink && udraw < line_ink + (1.0 - line_ink) * fillp) {
        if (fill_mode == 1u) {
            // Exact (R) fills: sample the CPU-detected, painter-clipped
            // rectangle table baked into this shader (area-weighted).
            let seedp = floor(max(get_param(xform_id, variation_id, 0u), 0.0));
            let pk = mnd_exact_pick(seedp, f32(min(depth, 4u)), ds, inset,
                                    rng_nextf(rng), rng_nextf(rng), rng_nextf(rng));
            if (pk.w > 0.0) {
                filling = true;
                exact_fill = true;
                q = pk.xy;
                rvc = pk.z;
            }
        }
        // Rectangle fill, the chaos-game version of the R relational
        // pass: a second independent segment; if parallel, overlapping
        // in span, and within reach, fill the rectangle BETWEEN the
        // two drawn lines. Width = their actual overlap, height =
        // their actual separation -> true squares and longer
        // rectangles, bounded by real lines on two sides (only
        // sometimes on all four), lines drawing over the fills.
        // B = sibling of A: replay A's descent, re-randomizing only the
        // bottom level (sometimes two or three, for wider-reaching
        // rectangles). Uniform independent pairs almost never lie
        // adjacent in a drawing of thousands of segments.
        // Up to 8 sibling tries per sample: a single try succeeds only
        // at the pair-geometry acceptance rate (~15-30%), silently
        // handing most of the fill budget back to the lines. Retrying
        // makes the Fill slider mean what it says; if A truly has no
        // parallel neighbor, all tries fail and the sample draws a
        // line (correct - there is nothing to fill there).
        for (var att = 0u; att < 8u && !filling; att = att + 1u) {
        let lr = rng_nextf(rng);
        var rb = 1u;
        if (lr < 0.35) { rb = 2u; }
        if (lr < 0.08) { rb = 3u; }
        rb = min(rb, depth);
        let sb = mnd_seg_from(&rulec, &lvt, &ckpt, rng, rb, lnds, rl, ftot);
        let b_p0 = vec2<f32>(sb[0], sb[1]);
        let b_len = sb[2];
        let b_ang = u32(sb[3]);
        if ((a_ang & 1u) == (b_ang & 1u)) {
            let horiz = (a_ang & 1u) == 0u;
            let a1 = a_p0 + mnd_rot(vec2<f32>(a_len, 0.0), a_ang);
            let b1 = b_p0 + mnd_rot(vec2<f32>(b_len, 0.0), b_ang);
            var aa0: f32; var aa1: f32; var ca: f32;
            var ba0: f32; var ba1: f32; var cb: f32;
            if (horiz) {
                aa0 = min(a_p0.x, a1.x); aa1 = max(a_p0.x, a1.x); ca = a_p0.y;
                ba0 = min(b_p0.x, b1.x); ba1 = max(b_p0.x, b1.x); cb = b_p0.y;
            } else {
                aa0 = min(a_p0.y, a1.y); aa1 = max(a_p0.y, a1.y); ca = a_p0.x;
                ba0 = min(b_p0.y, b1.y); ba1 = max(b_p0.y, b1.y); cb = b_p0.x;
            }
            let lo = max(aa0, ba0);
            let hi = min(aa1, ba1);
            let sep = abs(cb - ca);
            if (hi - lo > 0.05 && sep > 0.05 && sep <= span * max(a_len, b_len)) {
                filling = true;
                let c0 = min(ca, cb);
                let c1 = max(ca, cb);
                let ia = inset * (hi - lo);
                let ic = inset * sep;
                let av = mix(lo + ia, hi - ia, rng_nextf(rng));
                let cv = mix(c0 + ic, c1 - ic, rng_nextf(rng));
                if (horiz) { q = vec2<f32>(av, cv); } else { q = vec2<f32>(cv, av); }
                // Stable per-rectangle color: hash the quantized bounds
                // so every sample of this rectangle agrees, whichever
                // segment pair spanned it.
                let qs = 4.0;
                rvc = mnd_hash01(
                    i32(round(lo * qs)) ^ (i32(round(hi * qs)) * 7919),
                    i32(round(c0 * qs)) ^ (i32(round(c1 * qs)) * 104729));
            }
        }
        }
    }
    if (!filling) {
        q = a_p0 + mnd_rot(vec2<f32>(a_len * rng_nextf(rng), 0.0), a_ang);
    }

    if (dc_mode == 1u) {
        *vc = fract(sa[4] * max(dc_scale, 1e-6));
    } else if (dc_mode == 2u) {
        // Cyclic in the segment counter (the length-drift driver).
        *vc = fract(sa[5] * color_speed * 0.1 * max(dc_scale, 1e-6));
    } else if (dc_mode == 3u) {
        *vc = fract((f32(a_ang) + 0.5) * 0.25 * max(dc_scale, 1e-6));
    } else if (dc_mode == 4u) {
        // Cell: one flat color per rectangle; lines to palette
        // position 0 (put black there).
        if (filling && exact_fill) {
            // Baked R color (id %% 5 on five evenly spaced stops).
            *vc = rvc;
        } else if (filling) {
            if (dc_scale >= 2.0) {
                // Quantize to n flat palette colors (5 = the classic
                // Mondrian five; the R script colors by id %% 5).
                // floor(h*n)/(n-1) lands EXACTLY on evenly spaced
                // palette stops instead of interpolating between them.
                let n = round(dc_scale);
                *vc = floor(rvc * n) / (n - 1.0);
            } else {
                *vc = rvc;
            }
        } else {
            *vc = 0.0;
        }
    }

    if (!filling && thickness > 0.0) {
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


// =============================================================================
// Exact-fill specialization: the R relational pass, on the CPU.
//
// A flame variation is a stateless per-sample function; the R script's fill
// is a global analysis of the finished drawing (round + dedup endpoints,
// find rectangles from aligned segment/point combinations, keep only those
// with a drawn connecting side, paint in order). Those are whole-set
// relational queries a per-thread chaos game cannot answer — so when a
// transform selects Fill Mode = Exact, we expand the L-system here, run the
// R algorithm, painter-clip the overlaps into disjoint pieces, and bake the
// result into the shader as const tables via the per-flame specialization
// hook (the `synth` mechanism). `specialization_key` makes the shader cache
// rebuild whenever a (seed, depth, drift) combo changes.
//
// Known divergences from the R script, all order-of-enumeration flavored:
// candidate order (and thus the id %% 5 color assignment and painter order)
// follows our deterministic point-major/segment-sorted order, not R's exact
// dplyr row order; R's area-percentile filter is reproduced as written
// (`A >= quantile(..)[1]` compares against the 0% quantile — a no-op);
// overlaps are clipped topmost-first into disjoint rectangles instead of
// overdrawn, which is painter-equivalent for the visible result.
// =============================================================================

use crate::scene::transforms::Flame;
use std::collections::{BTreeSet, HashMap, HashSet};

/// CPU mirror of the WGSL init's LCG — must stay in lockstep so the rule
/// (and therefore the line drawing the GPU samples) matches the rectangle
/// table baked here.
fn cpu_next(st: &mut u32) -> f32 {
    *st = st.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    ((*st >> 8) & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// CPU mirror of the WGSL init's rule generation (see WGSL_INIT).
fn cpu_rule(seed: u32) -> Vec<u8> {
    let mut st = seed.wrapping_mul(0x9E37_79B9).wrapping_add(0x85EB_CA6B);
    st ^= st >> 16;
    let s = 15 + (cpu_next(&mut st) * 11.999) as usize;
    let mut base = vec![0u8; s];
    for slot in base.iter_mut() {
        let r = cpu_next(&mut st) * 34.0;
        *slot = if r < 10.0 {
            0
        } else if r < 22.0 {
            1
        } else {
            2
        };
    }
    let mut posn = [0usize; 6];
    for i in 0..6 {
        let mut pp;
        let mut tries = 0;
        loop {
            pp = ((cpu_next(&mut st) * (s as f32 + 1.0)) as usize).min(s);
            let dup = posn[..i].contains(&pp);
            tries += 1;
            if !dup || tries > 64 {
                break;
            }
        }
        posn[i] = pp;
    }
    posn.sort_unstable();
    let mut rule: Vec<u8> = Vec::with_capacity(s + 6);
    let mut b = 0usize;
    for i in 0..=s {
        while b < 6 && posn[b] == i {
            rule.push(3 + (b as u8 & 1));
            b += 1;
        }
        if i < s {
            rule.push(base[i]);
        }
    }
    if !rule.contains(&0) {
        for c in rule.iter_mut() {
            if *c < 3 {
                *c = 0;
                break;
            }
        }
    }
    rule
}

fn deci(v: f64) -> i64 {
    (v * 10.0).round() as i64
}

/// Expand the L-system and run the turtle sequentially (R semantics: the
/// segment counter d advances per drawn segment and is saved/restored by
/// the bracket stack), then round endpoints to 0.1 and dedup in first-
/// occurrence order — the R script's `round(...,1) + distinct()`.
fn cpu_segments(rule: &[u8], depth: u32, ds: f64) -> Vec<[i64; 4]> {
    let mut acts: Vec<u8> = vec![0, 2, 0, 2, 0, 2, 0]; // F-F-F-F
    for _ in 0..depth {
        let mut next = Vec::with_capacity(acts.len() * rule.len());
        for &a in &acts {
            if a == 0 {
                next.extend_from_slice(rule);
            } else {
                next.push(a);
            }
        }
        acts = next;
        if acts.len() > 8_000_000 {
            break; // safety valve; depth is capped at 4 by the callers
        }
    }
    let (mut x, mut y) = (0f64, 0f64);
    let mut a: i64 = 0;
    let mut d: i64 = 0;
    // R initializes the stack with the origin state, so an unmatched `]`
    // restores the start of the drawing.
    let mut stack: Vec<(f64, f64, i64, i64)> = vec![(0.0, 0.0, 0, 0)];
    let mut seen: HashSet<[i64; 4]> = HashSet::new();
    let mut lines: Vec<[i64; 4]> = Vec::new();
    let ln = ds.ln();
    for &c in &acts {
        match c {
            0 => {
                let len = (d as f64 * ln).clamp(-27.6, 27.6).exp();
                let (dx, dy) = match a.rem_euclid(4) {
                    0 => (len, 0.0),
                    1 => (0.0, len),
                    2 => (-len, 0.0),
                    _ => (0.0, -len),
                };
                let (nx, ny) = (x + dx, y + dy);
                let k = [deci(x), deci(y), deci(nx), deci(ny)];
                if (k[0] != k[2] || k[1] != k[3])
                    && k.iter().all(|v| v.abs() < 100_000_000)
                    && seen.insert(k)
                {
                    lines.push(k);
                }
                x = nx;
                y = ny;
                d += 1;
            }
            1 => a -= 1, // '+' turns right
            2 => a += 1, // '-' turns left
            3 => stack.push((x, y, a, d)),
            4 => {
                if let Some(s0) = stack.pop() {
                    x = s0.0;
                    y = s0.1;
                    a = s0.2;
                    d = s0.3;
                }
            }
            _ => {}
        }
    }
    lines
}

/// The R rectangle detection: candidates are (segment, third point) with an
/// aligned coordinate; kept when a drawn line connects the point to one of
/// the segment's endpoints (directed, as in the R inner_joins) and the
/// three points are not collinear. Color = candidate id %% 5, with the id
/// counting per 10-point chunk exactly as the R script's chunked
/// row_number() does. Returned in painter order (join-group major, as the
/// R bind_rows produces): earlier = painted first = bottom.
fn cpu_rects(lines: &[[i64; 4]]) -> Vec<([i64; 4], u8)> {
    let mut pts: Vec<(i64, i64)> = Vec::new();
    let mut pseen: HashSet<(i64, i64)> = HashSet::new();
    for l in lines {
        if pseen.insert((l[0], l[1])) {
            pts.push((l[0], l[1]));
        }
    }
    for l in lines {
        if pseen.insert((l[2], l[3])) {
            pts.push((l[2], l[3]));
        }
    }
    let lineset: HashSet<[i64; 4]> = lines.iter().cloned().collect();
    let mut by_x: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut by_y: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, l) in lines.iter().enumerate() {
        by_x.entry(l[0]).or_default().push(i);
        if l[2] != l[0] {
            by_x.entry(l[2]).or_default().push(i);
        }
        by_y.entry(l[1]).or_default().push(i);
        if l[3] != l[1] {
            by_y.entry(l[3]).or_default().push(i);
        }
    }

    struct Cand {
        g: u8,
        ord: usize,
        bbox: [i64; 4],
        color: u8,
    }
    let mut cands: Vec<Cand> = Vec::new();
    let mut ord = 0usize;
    for chunk in pts.chunks(10) {
        let mut id = 0u32;
        for &(px, py) in chunk {
            let mut segids: Vec<usize> = Vec::new();
            if let Some(v) = by_x.get(&px) {
                segids.extend_from_slice(v);
            }
            if let Some(v) = by_y.get(&py) {
                segids.extend_from_slice(v);
            }
            segids.sort_unstable();
            segids.dedup();
            for &si in &segids {
                let l = lines[si];
                // Point must not coincide with either endpoint.
                if (l[0] == px && l[1] == py) || (l[2] == px && l[3] == py) {
                    continue;
                }
                id += 1; // R: row_number() over the filtered squares1 rows
                let color = (id % 5) as u8;
                // A drawn line must connect the point to an endpoint
                // (the four directed inner_joins, first match = group).
                let g = if lineset.contains(&[l[0], l[1], px, py]) {
                    1
                } else if lineset.contains(&[l[2], l[3], px, py]) {
                    2
                } else if lineset.contains(&[px, py, l[0], l[1]]) {
                    3
                } else if lineset.contains(&[px, py, l[2], l[3]]) {
                    4
                } else {
                    continue;
                };
                // Remove straight-line (collinear) triples.
                if (l[0] == l[2] && l[2] == px) || (l[1] == l[3] && l[3] == py) {
                    continue;
                }
                let bbox = [
                    l[0].min(l[2]).min(px),
                    l[1].min(l[3]).min(py),
                    l[0].max(l[2]).max(px),
                    l[1].max(l[3]).max(py),
                ];
                if bbox[2] - bbox[0] < 1 || bbox[3] - bbox[1] < 1 {
                    continue;
                }
                cands.push(Cand { g, ord, bbox, color });
                ord += 1;
            }
        }
    }
    cands.sort_by_key(|c| (c.g, c.ord));
    cands.into_iter().map(|c| (c.bbox, c.color)).collect()
}

/// Subtract rect `c` from rect `f`, returning the up-to-4 remainder strips.
fn rect_minus(f: [i64; 4], c: [i64; 4]) -> Vec<[i64; 4]> {
    if c[0] >= f[2] || c[2] <= f[0] || c[1] >= f[3] || c[3] <= f[1] {
        return vec![f];
    }
    let mut out = Vec::new();
    if c[0] > f[0] {
        out.push([f[0], f[1], c[0], f[3]]);
    }
    if c[2] < f[2] {
        out.push([c[2], f[1], f[2], f[3]]);
    }
    let mx0 = f[0].max(c[0]);
    let mx1 = f[2].min(c[2]);
    if c[1] > f[1] {
        out.push([mx0, f[1], mx1, c[1]]);
    }
    if c[3] < f[3] {
        out.push([mx0, c[3], mx1, f[3]]);
    }
    out
}

/// Painter's-order resolution: process rects topmost-first, keeping only
/// the parts not already covered — the visible result is exactly what the
/// R overdraw shows, as disjoint rectangles the shader can sample flatly.
/// Capped: once `cap` visible pieces exist we stop, dropping the deepest
/// (most-covered) rects.
fn cpu_clip(rects: &[([i64; 4], u8)], cap: usize) -> Vec<([i64; 4], u8)> {
    let mut covered: Vec<[i64; 4]> = Vec::new();
    let mut visible: Vec<([i64; 4], u8)> = Vec::new();
    for &(r, col) in rects.iter().rev() {
        let mut frags = vec![r];
        for c in &covered {
            if frags.is_empty() {
                break;
            }
            let mut next = Vec::new();
            for f in frags {
                next.extend(rect_minus(f, *c));
            }
            frags = next;
        }
        for f in frags {
            if f[2] - f[0] >= 1 && f[3] - f[1] >= 1 {
                visible.push((f, col));
                covered.push(f);
            }
        }
        if visible.len() >= cap {
            break;
        }
    }
    visible
}

/// Distinct (seed, depth, drift) combos of transforms running
/// mondrianomies with Fill Mode = Exact and Fill > 0, across normal,
/// linked, and final transforms. Sorted; capped at 4 (each combo bakes
/// its own rectangle table).
pub fn exact_combos(flame: &Flame) -> Vec<(u32, u32, f32)> {
    let mut set: BTreeSet<(u32, u32, u32)> = BTreeSet::new();
    let xforms = flame
        .transforms
        .iter()
        .chain(flame.linked_transforms.iter())
        .chain(flame.final_transforms.iter());
    for xform in xforms {
        if !xform.variations.contains_key("mondrianomies") {
            continue;
        }
        // Defaults come from the definition, never from literals here:
        // a config that leaves these unset still renders with them, so a
        // stale literal would bake the wrong table (or none at all, which
        // silently drops every fill sample into the 8-retry Pairs
        // fallback -- the Pairs picture at Exact's intended cost).
        let gp = |k: &str| xform.variation_param_or_default("mondrianomies", k);
        if gp("fill_mode") as u32 != 1 || gp("fill") <= 0.0 {
            continue;
        }
        let seed = gp("seed").max(0.0) as u32;
        let depth = (gp("depth") as u32).clamp(1, 4);
        let drift = gp("drift").clamp(0.5, 2.0);
        set.insert((seed, depth, drift.to_bits()));
    }
    set.into_iter()
        .take(4)
        .map(|(s, d, dr)| (s, d, f32::from_bits(dr)))
        .collect()
}

/// Cache key for the shader cache: changes whenever the baked tables would.
pub fn specialization_key(flame: &Flame) -> String {
    exact_combos(flame)
        .iter()
        .map(|(s, d, dr)| format!("{s}:{d}:{:08x}", dr.to_bits()))
        .collect::<Vec<_>>()
        .join(",")
}

/// Build the WGSL const tables + real `mnd_exact_pick` for the combos.
fn build_exact_wgsl(combos: &[(u32, u32, f32)]) -> String {
    let per_combo_cap = (3200 / combos.len().max(1)).max(400);
    let mut keys = String::new();
    let mut ranges = String::new();
    let mut rects_s = String::new();
    let mut metas = String::new();
    let mut total = 0usize;
    for &(seed, depth, drift) in combos {
        let rule = cpu_rule(seed);
        let lines = cpu_segments(&rule, depth, drift as f64);
        let rects = cpu_rects(&lines);
        let vis = cpu_clip(&rects, per_combo_cap);
        let offset = total;
        // Area-weighted CDF so a uniform r1 picks rectangles with density
        // proportional to area (flat perceived fill).
        let areas: Vec<f64> = vis
            .iter()
            .map(|(r, _)| ((r[2] - r[0]) as f64) * ((r[3] - r[1]) as f64))
            .collect();
        let asum: f64 = areas.iter().sum::<f64>().max(1e-9);
        let mut acc = 0f64;
        for (i, (r, col)) in vis.iter().enumerate() {
            acc += areas[i];
            let cdf = (acc / asum) as f32;
            rects_s.push_str(&format!(
                "vec4<f32>({:.4}, {:.4}, {:.4}, {:.4}), ",
                r[0] as f32 / 10.0,
                r[1] as f32 / 10.0,
                r[2] as f32 / 10.0,
                r[3] as f32 / 10.0
            ));
            metas.push_str(&format!(
                "vec2<f32>({:.7}, {:.2}), ",
                cdf,
                *col as f32 / 4.0
            ));
        }
        total += vis.len();
        keys.push_str(&format!(
            "vec4<f32>({:.1}, {:.1}, {:.7}, 0.0), ",
            seed as f32, depth as f32, drift
        ));
        ranges.push_str(&format!("vec2<u32>({offset}u, {}u), ", vis.len()));
    }
    if total == 0 {
        // All combos produced empty tables — keep the stub semantics.
        rects_s.push_str("vec4<f32>(0.0, 0.0, 0.0, 0.0), ");
        metas.push_str("vec2<f32>(1.0, 0.0), ");
        total = 1;
    }
    let c = combos.len();
    format!(
        r#"const MND_XKEY: array<vec4<f32>, {c}> = array<vec4<f32>, {c}>({keys});
const MND_XRANGE: array<vec2<u32>, {c}> = array<vec2<u32>, {c}>({ranges});
const MND_XRECT: array<vec4<f32>, {total}> = array<vec4<f32>, {total}>({rects_s});
const MND_XMETA: array<vec2<f32>, {total}> = array<vec2<f32>, {total}>({metas});
fn mnd_exact_pick(seed: f32, dep: f32, drift: f32, inset: f32, r1: f32, r2: f32, r3: f32) -> vec4<f32> {{
    var off = 0u;
    var cnt = 0u;
    for (var i = 0u; i < {c}u; i = i + 1u) {{
        let k = MND_XKEY[i];
        if (abs(k.x - seed) < 0.5 && abs(k.y - dep) < 0.5 && abs(k.z - drift) < 1e-6) {{
            off = MND_XRANGE[i].x;
            cnt = MND_XRANGE[i].y;
            break;
        }}
    }}
    if (cnt == 0u) {{ return vec4<f32>(0.0, 0.0, 0.0, -1.0); }}
    var lo = off;
    var hi = off + cnt - 1u;
    while (lo < hi) {{
        let mid = (lo + hi) >> 1u;
        if (MND_XMETA[mid].x < r1) {{ lo = mid + 1u; }} else {{ hi = mid; }}
    }}
    let rc = MND_XRECT[lo];
    let w = rc.z - rc.x;
    let h = rc.w - rc.y;
    let px = mix(rc.x + inset * w, rc.z - inset * w, r2);
    let py = mix(rc.y + inset * h, rc.w - inset * h, r3);
    return vec4<f32>(px, py, MND_XMETA[lo].y, 1.0);
}}
"#
    )
}

fn specialize(source: &str, flame: &Flame) -> String {
    let combos = exact_combos(flame);
    if combos.is_empty() {
        return source.to_string();
    }
    let (Some(b), Some(e)) = (
        source.find("// MND_EXACT_STUB_BEGIN"),
        source.find("// MND_EXACT_STUB_END"),
    ) else {
        return source.to_string();
    };
    let end = e + "// MND_EXACT_STUB_END".len();
    format!("{}{}{}", &source[..b], build_exact_wgsl(&combos), &source[end..])
}

/// 2D specialization entry point (see `variation_specialized_source` in
/// the shader builder). Returns the static source untouched when no
/// transform uses Exact fills — no rebuild churn.
pub fn specialize_wgsl_2d(flame: &Flame) -> String {
    specialize(MONDRIANOMIES.wgsl_2d, flame)
}

/// 3D specialization — same tables on the 3D body.
pub fn specialize_wgsl_3d(flame: &Flame) -> String {
    specialize(MONDRIANOMIES.wgsl_3d, flame)
}

// Reads the catalog's definition defaults.
#[cfg(feature = "engine-flame")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_pass_produces_rects_for_default_seed() {
        let rule = cpu_rule(12);
        assert!(rule.contains(&0), "rule has an F");
        let lines = cpu_segments(&rule, 3, 1.0);
        assert!(lines.len() > 50, "expanded drawing has segments: {}", lines.len());
        let rects = cpu_rects(&lines);
        assert!(!rects.is_empty(), "R pass finds rectangles");
        let vis = cpu_clip(&rects, 3200);
        assert!(!vis.is_empty());
        // Disjointness of the clipped pieces.
        for (i, (a, _)) in vis.iter().enumerate() {
            for (b, _) in vis.iter().skip(i + 1) {
                let overlap = a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3]);
                assert!(!overlap, "clipped rects overlap: {a:?} {b:?}");
            }
        }
    }

    /// The specializer must see a bare transform exactly as the GPU
    /// will: through the DEFINITION's defaults. Asserted against the
    /// definition rather than literals, so editing a default can never
    /// silently re-open the gap that made default-valued Exact flames
    /// bake no table and fall back to the slow Pairs path.
    #[test]
    fn exact_specialization_reads_definition_defaults() {
        use crate::scene::transforms::Transform;
        let def = |k: &str| {
            MONDRIANOMIES
                .parameters
                .iter()
                .find(|p| p.name == k)
                .unwrap_or_else(|| panic!("no param {k}"))
                .default_value
        };
        let mut flame = Flame::default();
        if flame.transforms.is_empty() {
            flame.transforms.push(Transform::default());
        }
        let xf = flame.transforms.get_mut(0).unwrap();
        xf.variations.insert("mondrianomies".into(), 1.0);
        xf.variation_params.clear(); // rely entirely on defaults

        let combos = exact_combos(&flame);
        if def("fill_mode") as u32 == 1 && def("fill") > 0.0 {
            assert_eq!(
                combos.len(),
                1,
                "defaults select Exact fills, so a bare transform must specialize"
            );
            let (seed, depth, drift) = combos[0];
            assert_eq!(seed, def("seed").max(0.0) as u32, "seed default");
            assert_eq!(depth, (def("depth") as u32).clamp(1, 4), "depth default");
            assert!(
                (drift - def("drift").clamp(0.5, 2.0)).abs() < 1e-6,
                "drift default: {drift} vs {}",
                def("drift")
            );
        } else {
            assert!(combos.is_empty(), "defaults do not select Exact fills");
        }
    }

    #[test]
    fn specialized_wgsl_replaces_stub() {
        use crate::scene::transforms::Transform;
        let mut flame = Flame::default();
        let xf = flame.transforms.get_mut(0);
        let xf = match xf {
            Some(x) => x,
            None => {
                flame.transforms.push(Transform::default());
                flame.transforms.get_mut(0).unwrap()
            }
        };
        xf.variations.insert("mondrianomies".into(), 1.0);
        xf.variation_params
            .insert("mondrianomies.fill_mode".into(), 1.0);
        xf.variation_params.insert("mondrianomies.fill".into(), 0.9);
        xf.variation_params.insert("mondrianomies.seed".into(), 12.0);
        let src = specialize_wgsl_2d(&flame);
        assert!(src.contains("MND_XRECT"), "tables baked");
        assert!(!src.contains("MND_EXACT_STUB_BEGIN"), "stub replaced");
    }
}
