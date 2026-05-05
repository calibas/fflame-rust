//! klein_group — Kleinian limit-set chaos game
//!
//! Discrete subgroup of Möbius transformations on the Riemann sphere,
//! rendered via the chaos-game algorithm from *Indra's Pearls: The
//! Vision of Felix Klein* (Mumford, Series & Wright 2002): pick one of
//! the four group generators `{a, b, A=a⁻¹, B=b⁻¹}` at random, apply
//! the Möbius transformation, plot, repeat. `avoid_reversal` skips
//! immediately-cancelling pairs (`aA, bB, Aa, Bb`) so the trajectory
//! drifts through the limit set instead of bouncing back.
//!
//! Supports 7 recipes for constructing the generator pair `(mat_a,
//! mat_b)` from two complex parameters:
//!   0: GRANDMA_STANDARD — Mumford/Series/Wright's parabolic
//!      commutator construction
//!   1: MASKIT_MU — single complex parameter μ (the famous Maskit
//!      slice)
//!   2: JORGENSEN — Troels Jørgensen's parameterization
//!   3: RILEY — Robert Riley's parameterization (single complex c)
//!   4: RILEY_MODIFIED — Riley with extra b parameter
//!   5: MASKIT_MU_MODIFIED — Maskit with extra b parameter
//!   6: MASKIT_LEYS_MODIFIED — Jos Leys' n-fold variant
//!
//! Per-thread state: 1 slot for `prev_matrix` index (0..3 = a, A, b,
//! B). Custom `wgsl_state_init` seeds it uniformly from rng.
//!
//! Init computes `mat_a` and `mat_b` (8 floats each = 16 init slots)
//! from the recipe + (a_re, a_im, b_re, b_im). Inverses (mat_A,
//! mat_B) computed on-the-fly via the SL(2,ℂ) shortcut
//! `[d, -b, -c, a]` since all generator matrices are det=1 by
//! construction.
//!
//! Body drops cpp's `VVAR` factor — framework's outer multiplier
//! supplies it. cpp does `z = (FTx + i·FTy) / VVAR` then
//! `out = (a·z + b)/(c·z + d)` then `FPx/y += VVAR · out.re/im`.
//! Net effect: framework multiplies our return by weight, so we
//! return `mobius(p / weight)`.
//!
//! Source: `output/jwildfire-vars/output/klein_group.cpp`.
//! Reference: *Indra's Pearls* Ch. 6 (Grandma's recipe), Ch. 8 (Maskit
//! slice), Ch. 9 (Jørgensen).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static KLEIN_GROUP: VariationDef = VariationDef {
    name: "klein_group",
    display_name: "Klein Group",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("a_re", "A (real)", unlimited_float, 2.0, -10.0, 10.0),
        param!("a_im", "A (imag)", unlimited_float, 0.0, -10.0, 10.0),
        param!("b_re", "B (real)", unlimited_float, 2.0, -10.0, 10.0),
        param!("b_im", "B (imag)", unlimited_float, 0.0, -10.0, 10.0),
        param!("recipe", "Recipe", int, 0.0, 0.0, 6.0),
        param!("avoid_reversal", "Avoid Reversal", int, 1.0, 0.0, 1.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 16,
    wgsl_init: Some(r#"
fn init_klein_group(user: array<f32, 6>) -> array<f32, 16> {
    let a_re = user[0];
    let a_im = user[1];
    let b_re = user[2];
    let b_im = user[3];
    let recipe = i32(user[4]);

    let zero = vec2<f32>(0.0, 0.0);
    let re1 = vec2<f32>(1.0, 0.0);
    let re2 = vec2<f32>(2.0, 0.0);
    let re4 = vec2<f32>(4.0, 0.0);
    let im1 = vec2<f32>(0.0, 1.0);
    let im2 = vec2<f32>(0.0, 2.0);
    let im4 = vec2<f32>(0.0, 4.0);
    let neg1 = vec2<f32>(-1.0, 0.0);

    var mat_a0 = re1; var mat_a1 = zero; var mat_a2 = zero; var mat_a3 = re1;
    var mat_b0 = re1; var mat_b1 = zero; var mat_b2 = zero; var mat_b3 = re1;

    if (recipe == 3) {
        // RILEY: mat_a = [[1, 0], [c, 1]], mat_b = [[1, 2], [0, 1]]; c = (a_re, a_im)
        let c = vec2<f32>(a_re, a_im);
        mat_a0 = re1; mat_a1 = zero; mat_a2 = c; mat_a3 = re1;
        mat_b0 = re1; mat_b1 = re2; mat_b2 = zero; mat_b3 = re1;
    } else if (recipe == 4) {
        // RILEY_MODIFIED: same as RILEY but mat_b uses (b_re, b_im) instead of 2
        let c = vec2<f32>(a_re, a_im);
        let b1 = vec2<f32>(b_re, b_im);
        mat_a0 = re1; mat_a1 = zero; mat_a2 = c; mat_a3 = re1;
        mat_b0 = re1; mat_b1 = b1; mat_b2 = zero; mat_b3 = re1;
    } else if (recipe == 1) {
        // MASKIT_MU: mat_a = [[-μ·i, -i], [-i, 0]], mat_b = [[1, 2], [0, 1]]
        let mu = vec2<f32>(a_re, a_im);
        mat_a0 = cmul(cmul_real(mu, -1.0), im1);
        mat_a1 = cmul_real(im1, -1.0);
        mat_a2 = cmul_real(im1, -1.0);
        mat_a3 = zero;
        mat_b0 = re1; mat_b1 = re2; mat_b2 = zero; mat_b3 = re1;
    } else if (recipe == 5) {
        // MASKIT_MU_MODIFIED: as MASKIT_MU but mat_b uses (b_re, b_im)
        let mu = vec2<f32>(a_re, a_im);
        let b1 = vec2<f32>(b_re, b_im);
        mat_a0 = cmul(cmul_real(mu, -1.0), im1);
        mat_a1 = cmul_real(im1, -1.0);
        mat_a2 = cmul_real(im1, -1.0);
        mat_a3 = zero;
        mat_b0 = re1; mat_b1 = b1; mat_b2 = zero; mat_b3 = re1;
    } else if (recipe == 6) {
        // MASKIT_LEYS_MODIFIED: as MASKIT_MU but mat_b's b1 = 2·cos(π/b_re) + b_im·i
        let mu = vec2<f32>(a_re, a_im);
        let pi = 3.14159265358979;
        let safe_br = select(b_re, 1e-30, abs(b_re) < 1e-30);
        let b1 = vec2<f32>(2.0 * cos(pi / safe_br), b_im);
        mat_a0 = cmul(cmul_real(mu, -1.0), im1);
        mat_a1 = cmul_real(im1, -1.0);
        mat_a2 = cmul_real(im1, -1.0);
        mat_a3 = zero;
        mat_b0 = re1; mat_b1 = b1; mat_b2 = zero; mat_b3 = re1;
    } else if (recipe == 2) {
        // JORGENSEN: traceA, traceB given; solve quadratic for traceAB,
        //   then derive matrix entries from the trace identities.
        let trA = vec2<f32>(a_re, a_im);
        let trB = vec2<f32>(b_re, b_im);
        let bb = cmul_real(cmul(trA, trB), -1.0);
        let cc = cadd(csquare(trA), csquare(trB));
        let bsq = csquare(bb);
        let ac4 = cmul_real(cc, 4.0);
        let disc_root = csqrt(csub(bsq, ac4));
        // trABminus = (-bb - sqrt(bsq - 4·cc)) / 2
        let trAB = cmul_real(csub(cmul_real(bb, -1.0), disc_root), 0.5);
        let safe_trAB = vec2<f32>(
            select(trAB.x, 1e-15, abs(trAB.x) + abs(trAB.y) < 1e-15),
            trAB.y
        );

        let a0 = csub(trA, cdiv(trB, safe_trAB));
        let a1 = cdiv(trA, csquare(safe_trAB));
        let a2 = trA;
        let a3 = cdiv(trB, safe_trAB);

        let b0 = csub(trB, cdiv(trA, safe_trAB));
        let b1 = cmul_real(cdiv(trB, csquare(safe_trAB)), -1.0);
        let b2 = cmul_real(trB, -1.0);
        let b3 = cdiv(trA, safe_trAB);

        mat_a0 = a0; mat_a1 = a1; mat_a2 = a2; mat_a3 = a3;
        mat_b0 = b0; mat_b1 = b1; mat_b2 = b2; mat_b3 = b3;
    } else {
        // GRANDMA_STANDARD (recipe 0 or any unknown):
        //   parabolic commutator construction from Indra's Pearls Ch. 6
        let trA = vec2<f32>(a_re, a_im);
        let trB = vec2<f32>(b_re, b_im);
        let bb = cmul_real(cmul(trA, trB), -1.0);
        let cc = cadd(csquare(trA), csquare(trB));
        let bsq = csquare(bb);
        let ac4 = cmul_real(cc, 4.0);
        let disc_root = csqrt(csub(bsq, ac4));
        // trABminus = (-bb - sqrt(bsq - 4·cc)) / 2  (cpp picks the − branch)
        let trAB = cmul_real(csub(cmul_real(bb, -1.0), disc_root), 0.5);
        let safe_trAB = vec2<f32>(
            select(trAB.x, 1e-15, abs(trAB.x) + abs(trAB.y) < 1e-15),
            trAB.y
        );

        // z0 = ((trAB - 2)·trB) / (trB·trAB - 2·trA + 2i·trAB)
        let znum = cmul(csub(trAB, re2), trB);
        let zdenom = cadd(csub(cmul(trB, safe_trAB), cmul_real(trA, 2.0)),
                          cmul(safe_trAB, im2));
        let z0 = cdiv(znum, zdenom);

        let a0 = cmul_real(trA, 0.5);
        // a1num = trA·trAB - 2·trB + 4i
        let a1num = cadd(csub(cmul(trA, safe_trAB), cmul_real(trB, 2.0)), im4);
        // a1denom = (2·trAB + 4) · z0
        let a1denom = cmul(cadd(cmul_real(safe_trAB, 2.0), re4), z0);
        let a1 = cdiv(a1num, a1denom);
        // a2num = (trA·trAB - 2·trB - 4i) · z0
        let a2num = cmul(csub(csub(cmul(trA, safe_trAB), cmul_real(trB, 2.0)), im4), z0);
        // a2denom = 2·trAB - 4
        let a2denom = csub(cmul_real(safe_trAB, 2.0), re4);
        let a2 = cdiv(a2num, a2denom);
        let a3 = cmul_real(trA, 0.5);

        let b0 = cmul_real(csub(trB, im2), 0.5);
        let b1 = cmul_real(trB, 0.5);
        let b2 = cmul_real(trB, 0.5);
        let b3 = cmul_real(cadd(trB, im2), 0.5);

        mat_a0 = a0; mat_a1 = a1; mat_a2 = a2; mat_a3 = a3;
        mat_b0 = b0; mat_b1 = b1; mat_b2 = b2; mat_b3 = b3;
    }

    var out: array<f32, 16>;
    // mat_a entries (real, imag) packed
    out[0]  = mat_a0.x; out[1]  = mat_a0.y;
    out[2]  = mat_a1.x; out[3]  = mat_a1.y;
    out[4]  = mat_a2.x; out[5]  = mat_a2.y;
    out[6]  = mat_a3.x; out[7]  = mat_a3.y;
    // mat_b entries
    out[8]  = mat_b0.x; out[9]  = mat_b0.y;
    out[10] = mat_b1.x; out[11] = mat_b1.y;
    out[12] = mat_b2.x; out[13] = mat_b2.y;
    out[14] = mat_b3.x; out[15] = mat_b3.y;
    return out;
}
"#),
    state_count: 1,
    wgsl_state_init: Some(
        // Initialize prev_matrix index uniformly from {0, 1, 2, 3}.
        "        let r = rng_nextf(&rng);\n\
         \x20       set_state(xform_id, variation_id, 0u, floor(r * 4.0));"
    ),
    needs_accum: false,
    wgsl_2d: r#"
fn klein_load_mat_a(xform_id: u32, variation_id: u32) -> CMat2 {
    return cmat2_make(
        vec2<f32>(get_param(xform_id, variation_id, 6u),  get_param(xform_id, variation_id, 7u)),
        vec2<f32>(get_param(xform_id, variation_id, 8u),  get_param(xform_id, variation_id, 9u)),
        vec2<f32>(get_param(xform_id, variation_id, 10u), get_param(xform_id, variation_id, 11u)),
        vec2<f32>(get_param(xform_id, variation_id, 12u), get_param(xform_id, variation_id, 13u))
    );
}

fn klein_load_mat_b(xform_id: u32, variation_id: u32) -> CMat2 {
    return cmat2_make(
        vec2<f32>(get_param(xform_id, variation_id, 14u), get_param(xform_id, variation_id, 15u)),
        vec2<f32>(get_param(xform_id, variation_id, 16u), get_param(xform_id, variation_id, 17u)),
        vec2<f32>(get_param(xform_id, variation_id, 18u), get_param(xform_id, variation_id, 19u)),
        vec2<f32>(get_param(xform_id, variation_id, 20u), get_param(xform_id, variation_id, 21u))
    );
}

fn variation_klein_group(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let avoid_reversal = i32(get_param(xform_id, variation_id, 5u));
    let prev = i32(get_state(xform_id, variation_id, 0u));

    // Allowed-matrix subset depends on previous matrix (avoid_reversal):
    //   if prev=a (0): not_A = {A=1, b=2, B=3}  (can't pick a's inverse A=1? wait — see cpp)
    // Actually cpp does the opposite: if prev was `a`, exclude `A` (the inverse).
    //   prev=mat_a (0)     → mtransforms = not_A = {a, b, B}        but cpp says not_A = {a, b, B}?
    // Re-reading cpp:
    //   if (prev == mat_a)     mtransforms = not_A;  // not_A = {mat_a, mat_b, mat_inv_b}
    //   if (prev == mat_inv_a) mtransforms = not_a;  // not_a = {mat_inv_a, mat_b, mat_inv_b}
    //   if (prev == mat_b)     mtransforms = not_B;  // not_B = {mat_a, mat_inv_a, mat_b}
    //   if (prev == mat_inv_b) mtransforms = not_b;  // not_b = {mat_a, mat_inv_a, mat_inv_b}
    // So: if previous was X, exclude X's inverse from the next pick. (prevents
    // immediate aA/Aa/bB/Bb cancellations.)
    //
    // Encoding:  index 0 = a, 1 = A (=a⁻¹), 2 = b, 3 = B (=b⁻¹).
    //   prev=0 → exclude 1 (A); choose from {0, 2, 3}
    //   prev=1 → exclude 0 (a); choose from {1, 2, 3}
    //   prev=2 → exclude 3 (B); choose from {0, 1, 2}
    //   prev=3 → exclude 2 (b); choose from {0, 1, 3}

    var chosen: i32 = 0;
    if (avoid_reversal != 0) {
        let r = i32(floor(rng_nextf(rng) * 3.0));
        let r3 = clamp(r, 0, 2);
        if (prev == 0) {
            // {0, 2, 3}
            if (r3 == 0) { chosen = 0; } else if (r3 == 1) { chosen = 2; } else { chosen = 3; }
        } else if (prev == 1) {
            if (r3 == 0) { chosen = 1; } else if (r3 == 1) { chosen = 2; } else { chosen = 3; }
        } else if (prev == 2) {
            if (r3 == 0) { chosen = 0; } else if (r3 == 1) { chosen = 1; } else { chosen = 2; }
        } else {
            if (r3 == 0) { chosen = 0; } else if (r3 == 1) { chosen = 1; } else { chosen = 3; }
        }
    } else {
        chosen = i32(floor(rng_nextf(rng) * 4.0));
        chosen = clamp(chosen, 0, 3);
    }

    let mat_a = klein_load_mat_a(xform_id, variation_id);
    let mat_b = klein_load_mat_b(xform_id, variation_id);
    var m: CMat2;
    if (chosen == 0) {
        m = mat_a;
    } else if (chosen == 1) {
        m = cmat2_inverse_sl2(mat_a);
    } else if (chosen == 2) {
        m = mat_b;
    } else {
        m = cmat2_inverse_sl2(mat_b);
    }

    set_state(xform_id, variation_id, 0u, f32(chosen));

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);
    let z = vec2<f32>(p.x / safe_w, p.y / safe_w);
    let out = cmat2_apply(m, z);
    return out;
}
"#,
    wgsl_3d: Some(r#"
fn variation_klein_group(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let r2 = variation_klein_group_2d(p.xy, xform_id, variation_id, rng);
    return vec3<f32>(r2.x, r2.y, p.z);
}

fn klein_load_mat_a_3d(xform_id: u32, variation_id: u32) -> CMat2 {
    return cmat2_make(
        vec2<f32>(get_param(xform_id, variation_id, 6u),  get_param(xform_id, variation_id, 7u)),
        vec2<f32>(get_param(xform_id, variation_id, 8u),  get_param(xform_id, variation_id, 9u)),
        vec2<f32>(get_param(xform_id, variation_id, 10u), get_param(xform_id, variation_id, 11u)),
        vec2<f32>(get_param(xform_id, variation_id, 12u), get_param(xform_id, variation_id, 13u))
    );
}

fn klein_load_mat_b_3d(xform_id: u32, variation_id: u32) -> CMat2 {
    return cmat2_make(
        vec2<f32>(get_param(xform_id, variation_id, 14u), get_param(xform_id, variation_id, 15u)),
        vec2<f32>(get_param(xform_id, variation_id, 16u), get_param(xform_id, variation_id, 17u)),
        vec2<f32>(get_param(xform_id, variation_id, 18u), get_param(xform_id, variation_id, 19u)),
        vec2<f32>(get_param(xform_id, variation_id, 20u), get_param(xform_id, variation_id, 21u))
    );
}

fn variation_klein_group_2d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let avoid_reversal = i32(get_param(xform_id, variation_id, 5u));
    let prev = i32(get_state(xform_id, variation_id, 0u));

    var chosen: i32 = 0;
    if (avoid_reversal != 0) {
        let r3 = clamp(i32(floor(rng_nextf(rng) * 3.0)), 0, 2);
        if (prev == 0) {
            if (r3 == 0) { chosen = 0; } else if (r3 == 1) { chosen = 2; } else { chosen = 3; }
        } else if (prev == 1) {
            if (r3 == 0) { chosen = 1; } else if (r3 == 1) { chosen = 2; } else { chosen = 3; }
        } else if (prev == 2) {
            if (r3 == 0) { chosen = 0; } else if (r3 == 1) { chosen = 1; } else { chosen = 2; }
        } else {
            if (r3 == 0) { chosen = 0; } else if (r3 == 1) { chosen = 1; } else { chosen = 3; }
        }
    } else {
        chosen = clamp(i32(floor(rng_nextf(rng) * 4.0)), 0, 3);
    }

    let mat_a = klein_load_mat_a_3d(xform_id, variation_id);
    let mat_b = klein_load_mat_b_3d(xform_id, variation_id);
    var m: CMat2;
    if (chosen == 0) { m = mat_a; }
    else if (chosen == 1) { m = cmat2_inverse_sl2(mat_a); }
    else if (chosen == 2) { m = mat_b; }
    else { m = cmat2_inverse_sl2(mat_b); }

    set_state(xform_id, variation_id, 0u, f32(chosen));

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);
    let z = vec2<f32>(p.x / safe_w, p.y / safe_w);
    return cmat2_apply(m, z);
}
"#),
};
