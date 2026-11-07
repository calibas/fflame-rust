/// Kleinian limit-set chaos game from Mumford, Series & Wright's *Indra's
/// Pearls: The Vision of Felix Klein* (2002). Each iteration picks one of
/// the four group generators `{a, b, A=a⁻¹, B=b⁻¹}` at random, applies the
/// corresponding Möbius transformation `(α·z + β)/(γ·z + δ)`, and plots the
/// result. `avoid_reversal` skips immediately-cancelling pairs (`aA, bB,
/// Aa, Bb`) so the trajectory drifts through the limit set instead of
/// bouncing back. Seven recipes are available for constructing the
/// generator pair from the complex parameters `(a_re + i·a_im)` and `(b_re
/// + i·b_im)`: Grandma's standard parabolic-commutator construction (0),
/// Maskit's μ slice (1), Jørgensen's trace-based parameterization (2),
/// Riley's c parameter (3), Riley-modified with extra `b` (4), Maskit-
/// modified with extra `b` (5), and Jos Leys' n-fold Maskit variant (6).
/// Inverses are computed on-the-fly via the `SL(2,ℂ)` shortcut `[d, -b, -c,
/// a]` since all generator matrices have determinant 1 by construction.
///
/// # Authors
/// - CozyG

// Klein Group, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a_re: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "A (real)",
        hint: "Real part of the first complex parameter. Interpretation depends on `recipe`: trace of generator A (recipes 0 GRANDMA, 2 JORGENSEN), Maskit's μ parameter (1, 5, 6), or Riley's c parameter (3, 4).",
    },
    a_im: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A (imag)",
        hint: "Imaginary part of the first complex parameter. See `a_re` for recipe-dependent role.",
    },
    b_re: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "B (real)",
        hint: "Real part of the second complex parameter. Interpretation depends on `recipe`: trace of generator B (0, 2), `b1` translation coefficient (4, 5), the `n` in Leys' `2·cos(π/n)` formula (6). Unused in recipes 1 (MASKIT_MU) and 3 (RILEY) — those have a hardcoded `b = 2`.",
    },
    b_im: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B (imag)",
        hint: "Imaginary part of the second complex parameter. See `b_re` for recipe-dependent role.",
    },
    recipe: #{
        value: 0, choices: ["Grandma", "Maskit Mu", "Jorgensen", "Riley", "Riley+", "Maskit+", "Maskit-Leys"],
        caption: "Recipe",
        hint: "Generator-pair construction recipe. Grandma is the parabolic commutator from *Indra's Pearls* Ch. 6. Maskit Mu is the Maskit slice (single complex μ). Jorgensen uses Troels Jørgensen's trace-based parameterization. Riley uses Robert Riley's single complex c. The `+` variants add an extra `b1`. Maskit-Leys is Jos Leys' n-fold variant.",
    },
    avoid_reversal: #{
        value: true,
        caption: "Avoid Reversal",
        hint: "When on, the next-matrix pick excludes the inverse of the previous matrix (no `aA`, `Aa`, `bB`, or `Bb` cancellations), so the trajectory drifts through the limit set instead of bouncing back-and-forth on a small subset. When off, all four matrices are equally likely each iteration.",
    },
};

const INFO = #{ title: "Klein Group", name: "rhai_klein_group", category: "advanced_2d", z: "preserve_z" };

fn cadd(a, b) {
    a + b
}

fn csub(a, b) {
    a - b
}

fn cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn cmul_real(a, s) {
    a * s
}

fn cconj(a) {
    complex(a.re, -a.im)
}

fn cmag2(a) {
    a.re * a.re + a.im * a.im
}

fn cdiv(a, b) {
    let denom = cmag2(b);
    let safe_denom = (if denom < 1.0e-30 { 1.0e-30 } else { denom });
    cmul(a, cconj(b)) / safe_denom
}

fn csquare(a) {
    complex(a.re * a.re - a.im * a.im, 2.0 * a.re * a.im)
}

fn csqrt(a) {
    let mag = sqrt(cmag2(a));
    let real_part = sqrt(max(0.0, 0.5 * (mag + a.re)));
    let imag_mag = sqrt(max(0.0, 0.5 * (mag - a.re)));
    let imag_part = (if a.im >= 0.0 { imag_mag } else { -imag_mag });
    complex(real_part, imag_part)
}

fn prepare(p) {
    let a_re = p.a_re;
    let a_im = p.a_im;
    let b_re = p.b_re;
    let b_im = p.b_im;
    let recipe = to_int(to_float(p.recipe));
    let zero = complex(0.0, 0.0);
    let re1 = complex(1.0, 0.0);
    let re2 = complex(2.0, 0.0);
    let re4 = complex(4.0, 0.0);
    let im1 = complex(0.0, 1.0);
    let im2 = complex(0.0, 2.0);
    let im4 = complex(0.0, 4.0);
    let neg1 = complex(-1.0, 0.0);
    let mat_a0 = re1;
    let mat_a1 = zero;
    let mat_a2 = zero;
    let mat_a3 = re1;
    let mat_b0 = re1;
    let mat_b1 = zero;
    let mat_b2 = zero;
    let mat_b3 = re1;
    if (recipe == 3) {
        let c = complex(a_re, a_im);
        mat_a0 = re1;
        mat_a1 = zero;
        mat_a2 = c;
        mat_a3 = re1;
        mat_b0 = re1;
        mat_b1 = re2;
        mat_b2 = zero;
        mat_b3 = re1;
    } else if (recipe == 4) {
        let c = complex(a_re, a_im);
        let b1 = complex(b_re, b_im);
        mat_a0 = re1;
        mat_a1 = zero;
        mat_a2 = c;
        mat_a3 = re1;
        mat_b0 = re1;
        mat_b1 = b1;
        mat_b2 = zero;
        mat_b3 = re1;
    } else if (recipe == 1) {
        let mu = complex(a_re, a_im);
        mat_a0 = cmul(cmul_real(mu, -1.0), im1);
        mat_a1 = cmul_real(im1, -1.0);
        mat_a2 = cmul_real(im1, -1.0);
        mat_a3 = zero;
        mat_b0 = re1;
        mat_b1 = re2;
        mat_b2 = zero;
        mat_b3 = re1;
    } else if (recipe == 5) {
        let mu = complex(a_re, a_im);
        let b1 = complex(b_re, b_im);
        mat_a0 = cmul(cmul_real(mu, -1.0), im1);
        mat_a1 = cmul_real(im1, -1.0);
        mat_a2 = cmul_real(im1, -1.0);
        mat_a3 = zero;
        mat_b0 = re1;
        mat_b1 = b1;
        mat_b2 = zero;
        mat_b3 = re1;
    } else if (recipe == 6) {
        let mu = complex(a_re, a_im);
        let pi = 3.14159265358979;
        let safe_br = (if abs(b_re) < 1.0e-30 { 1.0e-30 } else { b_re });
        let b1 = complex(2.0 * cos(pi / safe_br), b_im);
        mat_a0 = cmul(cmul_real(mu, -1.0), im1);
        mat_a1 = cmul_real(im1, -1.0);
        mat_a2 = cmul_real(im1, -1.0);
        mat_a3 = zero;
        mat_b0 = re1;
        mat_b1 = b1;
        mat_b2 = zero;
        mat_b3 = re1;
    } else if (recipe == 2) {
        let trA = complex(a_re, a_im);
        let trB = complex(b_re, b_im);
        let bb = cmul_real(cmul(trA, trB), -1.0);
        let cc = cadd(csquare(trA), csquare(trB));
        let bsq = csquare(bb);
        let ac4 = cmul_real(cc, 4.0);
        let disc_root = csqrt(csub(bsq, ac4));
        let trAB = cmul_real(csub(cmul_real(bb, -1.0), disc_root), 0.5);
        let safe_trAB = complex((if abs(trAB.re) + abs(trAB.im) < 1.0e-15 { 1.0e-15 } else { trAB.re }), trAB.im);
        let a0 = csub(trA, cdiv(trB, safe_trAB));
        let a1 = cdiv(trA, csquare(safe_trAB));
        let a2 = trA;
        let a3 = cdiv(trB, safe_trAB);
        let b0 = csub(trB, cdiv(trA, safe_trAB));
        let b1 = cmul_real(cdiv(trB, csquare(safe_trAB)), -1.0);
        let b2 = cmul_real(trB, -1.0);
        let b3 = cdiv(trA, safe_trAB);
        mat_a0 = a0;
        mat_a1 = a1;
        mat_a2 = a2;
        mat_a3 = a3;
        mat_b0 = b0;
        mat_b1 = b1;
        mat_b2 = b2;
        mat_b3 = b3;
    } else {
        let trA = complex(a_re, a_im);
        let trB = complex(b_re, b_im);
        let bb = cmul_real(cmul(trA, trB), -1.0);
        let cc = cadd(csquare(trA), csquare(trB));
        let bsq = csquare(bb);
        let ac4 = cmul_real(cc, 4.0);
        let disc_root = csqrt(csub(bsq, ac4));
        let trAB = cmul_real(csub(cmul_real(bb, -1.0), disc_root), 0.5);
        let safe_trAB = complex((if abs(trAB.re) + abs(trAB.im) < 1.0e-15 { 1.0e-15 } else { trAB.re }), trAB.im);
        let znum = cmul(csub(trAB, re2), trB);
        let zdenom = cadd(csub(cmul(trB, safe_trAB), cmul_real(trA, 2.0)), cmul(safe_trAB, im2));
        let z0 = cdiv(znum, zdenom);
        let a0 = cmul_real(trA, 0.5);
        let a1num = cadd(csub(cmul(trA, safe_trAB), cmul_real(trB, 2.0)), im4);
        let a1denom = cmul(cadd(cmul_real(safe_trAB, 2.0), re4), z0);
        let a1 = cdiv(a1num, a1denom);
        let a2num = cmul(csub(csub(cmul(trA, safe_trAB), cmul_real(trB, 2.0)), im4), z0);
        let a2denom = csub(cmul_real(safe_trAB, 2.0), re4);
        let a2 = cdiv(a2num, a2denom);
        let a3 = cmul_real(trA, 0.5);
        let b0 = cmul_real(csub(trB, im2), 0.5);
        let b1 = cmul_real(trB, 0.5);
        let b2 = cmul_real(trB, 0.5);
        let b3 = cmul_real(cadd(trB, im2), 0.5);
        mat_a0 = a0;
        mat_a1 = a1;
        mat_a2 = a2;
        mat_a3 = a3;
        mat_b0 = b0;
        mat_b1 = b1;
        mat_b2 = b2;
        mat_b3 = b3;
    }
    #{ d0: mat_a0.re, d1: mat_a0.im, d2: mat_a1.re, d3: mat_a1.im, d4: mat_a2.re, d5: mat_a2.im, d6: mat_a3.re, d7: mat_a3.im, d8: mat_b0.re, d9: mat_b0.im, d10: mat_b1.re, d11: mat_b1.im, d12: mat_b2.re, d13: mat_b2.im, d14: mat_b3.re, d15: mat_b3.im }
}

fn init(p) {
    let r = rand();
    #{ st0: to_float(floor(r * 4.0)) }
}

fn cmat2_make(a, b, c, d) {
    let m = array(4, complex(0.0, 0.0));
    m[0] = a;
    m[1] = b;
    m[2] = c;
    m[3] = d;
    m
}

fn cmat2_apply(m, z) {
    let num = cadd(cmul(m[0], z), m[1]);
    let den = cadd(cmul(m[2], z), m[3]);
    cdiv(num, den)
}

fn cmat2_inverse_sl2(m) {
    cmat2_make(m[3], complex(-m[1].re, -m[1].im), complex(-m[2].re, -m[2].im), m[0])
}

fn klein_load_mat_a(p) {
    cmat2_make(complex(p.d0, p.d1), complex(p.d2, p.d3), complex(p.d4, p.d5), complex(p.d6, p.d7))
}

fn klein_load_mat_b(p) {
    cmat2_make(complex(p.d8, p.d9), complex(p.d10, p.d11), complex(p.d12, p.d13), complex(p.d14, p.d15))
}

fn klein_load_mat_a_3d(p) {
    cmat2_make(complex(p.d0, p.d1), complex(p.d2, p.d3), complex(p.d4, p.d5), complex(p.d6, p.d7))
}

fn klein_load_mat_b_3d(p) {
    cmat2_make(complex(p.d8, p.d9), complex(p.d10, p.d11), complex(p.d12, p.d13), complex(p.d14, p.d15))
}

fn variation_klein_group_2d(pv, p, ctx) {
    let avoid_reversal = to_int(to_float(p.avoid_reversal));
    let prev = to_int(this.st0);
    let chosen = 0;
    if (avoid_reversal != 0) {
        let r3 = clamp(to_int(to_float(floor(rand() * 3.0))), 0, 2);
        if (prev == 0) {
            if (r3 == 0) {
                chosen = 0;
            } else if (r3 == 1) {
                chosen = 2;
            } else {
                chosen = 3;
            }
        } else if (prev == 1) {
            if (r3 == 0) {
                chosen = 1;
            } else if (r3 == 1) {
                chosen = 2;
            } else {
                chosen = 3;
            }
        } else if (prev == 2) {
            if (r3 == 0) {
                chosen = 0;
            } else if (r3 == 1) {
                chosen = 1;
            } else {
                chosen = 2;
            }
        } else if (r3 == 0) {
            chosen = 0;
        } else if (r3 == 1) {
            chosen = 1;
        } else {
            chosen = 3;
        }
    } else {
        chosen = clamp(to_int(to_float(floor(rand() * 4.0))), 0, 3);
    }
    let mat_a = klein_load_mat_a_3d(p);
    let mat_b = klein_load_mat_b_3d(p);
    let m = array(4, complex(0.0, 0.0));
    if (chosen == 0) {
        m = mat_a;
    } else if (chosen == 1) {
        m = cmat2_inverse_sl2(mat_a);
    } else if (chosen == 2) {
        m = mat_b;
    } else {
        m = cmat2_inverse_sl2(mat_b);
    }
    this.st0 = to_float(chosen);
    let weight = ctx.weight;
    let safe_w = (if abs(weight) < 1.0e-30 { 1.0e-30 } else { weight });
    let z = complex(pv.re / safe_w, pv.im / safe_w);
    cmat2_apply(m, z)
}

fn variation(w, p, ctx) {
    let avoid_reversal = to_int(to_float(p.avoid_reversal));
    let prev = to_int(this.st0);
    let chosen = 0;
    if (avoid_reversal != 0) {
        let r = to_int(to_float(floor(rand() * 3.0)));
        let r3 = clamp(r, 0, 2);
        if (prev == 0) {
            if (r3 == 0) {
                chosen = 0;
            } else if (r3 == 1) {
                chosen = 2;
            } else {
                chosen = 3;
            }
        } else if (prev == 1) {
            if (r3 == 0) {
                chosen = 1;
            } else if (r3 == 1) {
                chosen = 2;
            } else {
                chosen = 3;
            }
        } else if (prev == 2) {
            if (r3 == 0) {
                chosen = 0;
            } else if (r3 == 1) {
                chosen = 1;
            } else {
                chosen = 2;
            }
        } else if (r3 == 0) {
            chosen = 0;
        } else if (r3 == 1) {
            chosen = 1;
        } else {
            chosen = 3;
        }
    } else {
        chosen = to_int(to_float(floor(rand() * 4.0)));
        chosen = clamp(chosen, 0, 3);
    }
    let mat_a = klein_load_mat_a(p);
    let mat_b = klein_load_mat_b(p);
    let m = array(4, complex(0.0, 0.0));
    if (chosen == 0) {
        m = mat_a;
    } else if (chosen == 1) {
        m = cmat2_inverse_sl2(mat_a);
    } else if (chosen == 2) {
        m = mat_b;
    } else {
        m = cmat2_inverse_sl2(mat_b);
    }
    this.st0 = to_float(chosen);
    let weight = ctx.weight;
    let safe_w = (if abs(weight) < 1.0e-30 { 1.0e-30 } else { weight });
    let z = complex(w.re / safe_w, w.im / safe_w);
    let out = cmat2_apply(m, z);
    out
}

fn variation3(v, p, ctx) {
    let r2 = this.variation_klein_group_2d(complex(v.x, v.y), p, ctx);
    vec3(r2.re, r2.im, v.z)
}
