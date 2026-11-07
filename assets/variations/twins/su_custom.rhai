/// SU(n) Möbius groups with live reduction tensors dialed from the
/// Reduce sliders.
///
/// # Authors
/// - Roger Bagula
/// - Fractals for All
/// - Claude Fable 5

// SU(n) Custom, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    group: #{
        value: 0, choices: ["SU(2) Custom", "SU(3) Custom", "SU(4) Custom"],
        caption: "Group",
        hint: "Which SU(n) algebra to reduce LIVE from the Reduce A/B/C/D + Plug sliders (init pass). SU(2) Custom: 6 generators. SU(3) Custom: the 8 Gell-Mann matrices reduced 3×3→2×2 (16 generators) — Bagula's family. SU(4) Custom: 30 generators (the D slider becomes live). Dial the reduction tensor and the pole-plug to explore the infinite family and the circle packing.",
    },
    avoid_reversal: #{
        value: true,
        caption: "Avoid Reversal",
        hint: "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset. Off = all generators equally likely every call.",
    },
    conj_angle: #{
        value: 45.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Elliptic rotation θ in the conjugator qf = rotate(θ + iη). SU(3)'s triquasiconformal '45'. Sweeping it deforms the whole limit set.",
    },
    conj_hyper: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Hyper Angle",
        hint: "HYPERBOLIC rotation η (imaginary angle) in qf = rotate(θ + iη) — the 'hyper' in SU(2)'s hypertriquasiconformal. 45° with Angle 0 is Bagula's SU(2) 6-group; nonzero η bends the group loxodromically, compacting lattices toward Apollonian disks.",
    },
    qc_strength: #{
        value: 1.0, min: 0.0, max: 2.0,
        caption: "QC Strength",
        hint: "Quasiconformal deformation δ in dk = [[1+iδ,1],[1,1−iδ]]. 1 = Bagula's groups; toward 0 the generators lose their quasiconformal stretch and the limit set collapses; > 1 exaggerates it.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Generator"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Generator: each group element has its own palette position, blended through a persistent color register at Color Speed.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the Generator color (wrapped with fract).",
    },
    color_speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Color Speed",
        hint: "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment.",
    },
    space: #{
        value: 0, choices: ["Euclidean", "Hyperbolic H3"],
        caption: "Space",
        hint: "3D render mode only. Euclidean: the ordinary complex Möbius map in the xy plane (z passes through) — the flat limit set. Hyperbolic H3: the Poincaré extension — SL(2,C) is exactly the isometry group of hyperbolic 3-space, so each generator acts on upper-half-space (the point as a quaternion x+yi+t·j, t = height) via (Aq+B)(Cq+D)⁻¹. The limit set fills 3D: SU(3)'s three disks become three spheres, SU(2)'s Apollonian disk an Apollonian sphere-packing. The z-slice at t→0 is the 2D picture.",
    },
    symmetry: #{
        value: 0, choices: ["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"],
        caption: "Symmetry",
        hint: "4-Fold applies a random one of {z, −z, conj z, −conj z} to the output each call — 2-Fold Point = {z,−z} (180° rotation), 2-Fold Mirror = {z,conj z} (reflection), 4-Fold = both (Bagula's SU(5) orbit-symmetrization for the 'symmetry enhanced' elliptical picture). Usable on any group.",
    },
    red_a_re: #{
        value: 2.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce A re",
        hint: "Real part of the reduction-tensor entry a (SU(3): tt = [[1,0,1],[a,b,c]]). The reduction is a free design choice — each (a,b,c) gives a different fractal in the SU(3) family. Computed live via the init pass.",
    },
    red_a_im: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce A im",
        hint: "Imag part of reduction entry a.",
    },
    red_b_re: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce B re",
        hint: "Real part of reduction entry b.",
    },
    red_b_im: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce B im",
        hint: "Imag part of reduction entry b.",
    },
    red_c_re: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce C re",
        hint: "Real part of reduction entry c.",
    },
    red_c_im: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce C im",
        hint: "Imag part of reduction entry c.",
    },
    red_d_re: #{
        value: -1.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce D re",
        hint: "SU(4) Custom: real part of reduction entry d (the 4th tensor column). Unused by SU(2)/SU(3) Custom.",
    },
    red_d_im: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Reduce D im",
        hint: "Imag part of reduction entry d.",
    },
    red_plug: #{
        value: 2.0, min: 0.0, max: 4.0,
        caption: "Plug",
        hint: "Trace added to the traceless reduced matrices to plug their Möbius poles. 2 makes them parabolic → Apollonian circle packing; 0 leaves them as pole-y involutions; the circles live near 2.",
    },
};

const INFO = #{ title: "SU(n) Custom", name: "rhai_su_custom", category: "advanced_2d" };

fn csub(a, b) {
    a - b
}

fn cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
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

fn csqrt(a) {
    let mag = sqrt(cmag2(a));
    let real_part = sqrt(max(0.0, 0.5 * (mag + a.re)));
    let imag_mag = sqrt(max(0.0, 0.5 * (mag - a.re)));
    let imag_part = (if a.im >= 0.0 { imag_mag } else { -imag_mag });
    complex(real_part, imag_part)
}

fn prepare(p) {
    let group = u32(to_float(p.group));
    let a = complex(p.red_a_re, p.red_a_im);
    let b = complex(p.red_b_re, p.red_b_im);
    let c = complex(p.red_c_re, p.red_c_im);
    let d = complex(p.red_d_re, p.red_d_im);
    let plug = p.red_plug;
    let out = array(259, 0.0);
    out[0] = to_float(p.group);
    out[1] = to_float(p.avoid_reversal);
    out[2] = p.conj_angle;
    out[3] = p.conj_hyper;
    out[4] = p.qc_strength;
    out[5] = to_float(p.dc_mode);
    out[6] = p.dc_scale;
    out[7] = p.color_speed;
    out[8] = to_float(p.space);
    out[9] = to_float(p.symmetry);
    out[10] = p.red_a_re;
    out[11] = p.red_a_im;
    out[12] = p.red_b_re;
    out[13] = p.red_b_im;
    out[14] = p.red_c_re;
    out[15] = p.red_c_im;
    out[16] = p.red_d_re;
    out[17] = p.red_d_im;
    out[18] = p.red_plug;
    if (group == u32(1)) {
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(1.000000, 0.000000), b);
        let wc = cmul(complex(1.000000, 0.000000), b);
        let wd = cmul(complex(2.000000, 0.000000), cmul(a, b));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(0))] = na.re;
        out[19 + to_int(u32(1))] = na.im;
        out[19 + to_int(u32(2))] = nb.re;
        out[19 + to_int(u32(3))] = nb.im;
        out[19 + to_int(u32(4))] = nc.re;
        out[19 + to_int(u32(5))] = nc.im;
        out[19 + to_int(u32(6))] = nd.re;
        out[19 + to_int(u32(7))] = nd.im;
        out[19 + to_int(u32(64))] = nd.re;
        out[19 + to_int(u32(65))] = nd.im;
        out[19 + to_int(u32(66))] = -nb.re;
        out[19 + to_int(u32(67))] = -nb.im;
        out[19 + to_int(u32(68))] = -nc.re;
        out[19 + to_int(u32(69))] = -nc.im;
        out[19 + to_int(u32(70))] = na.re;
        out[19 + to_int(u32(71))] = na.im;
        let wa = complex(2.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), c) + cmul(complex(1.000000, 0.000000), a);
        let wc = cmul(complex(1.000000, 0.000000), c) + cmul(complex(1.000000, 0.000000), a);
        let wd = cmul(complex(2.000000, 0.000000), cmul(a, c));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(8))] = na.re;
        out[19 + to_int(u32(9))] = na.im;
        out[19 + to_int(u32(10))] = nb.re;
        out[19 + to_int(u32(11))] = nb.im;
        out[19 + to_int(u32(12))] = nc.re;
        out[19 + to_int(u32(13))] = nc.im;
        out[19 + to_int(u32(14))] = nd.re;
        out[19 + to_int(u32(15))] = nd.im;
        out[19 + to_int(u32(72))] = nd.re;
        out[19 + to_int(u32(73))] = nd.im;
        out[19 + to_int(u32(74))] = -nb.re;
        out[19 + to_int(u32(75))] = -nb.im;
        out[19 + to_int(u32(76))] = -nc.re;
        out[19 + to_int(u32(77))] = -nc.im;
        out[19 + to_int(u32(78))] = na.re;
        out[19 + to_int(u32(79))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(1.000000, 0.000000), b);
        let wc = cmul(complex(1.000000, 0.000000), b);
        let wd = cmul(complex(2.000000, 0.000000), cmul(b, c));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(16))] = na.re;
        out[19 + to_int(u32(17))] = na.im;
        out[19 + to_int(u32(18))] = nb.re;
        out[19 + to_int(u32(19))] = nb.im;
        out[19 + to_int(u32(20))] = nc.re;
        out[19 + to_int(u32(21))] = nc.im;
        out[19 + to_int(u32(22))] = nd.re;
        out[19 + to_int(u32(23))] = nd.im;
        out[19 + to_int(u32(80))] = nd.re;
        out[19 + to_int(u32(81))] = nd.im;
        out[19 + to_int(u32(82))] = -nb.re;
        out[19 + to_int(u32(83))] = -nb.im;
        out[19 + to_int(u32(84))] = -nc.re;
        out[19 + to_int(u32(85))] = -nc.im;
        out[19 + to_int(u32(86))] = na.re;
        out[19 + to_int(u32(87))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), b);
        let wc = cmul(complex(0.000000, 1.000000), b);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(24))] = na.re;
        out[19 + to_int(u32(25))] = na.im;
        out[19 + to_int(u32(26))] = nb.re;
        out[19 + to_int(u32(27))] = nb.im;
        out[19 + to_int(u32(28))] = nc.re;
        out[19 + to_int(u32(29))] = nc.im;
        out[19 + to_int(u32(30))] = nd.re;
        out[19 + to_int(u32(31))] = nd.im;
        out[19 + to_int(u32(88))] = nd.re;
        out[19 + to_int(u32(89))] = nd.im;
        out[19 + to_int(u32(90))] = -nb.re;
        out[19 + to_int(u32(91))] = -nb.im;
        out[19 + to_int(u32(92))] = -nc.re;
        out[19 + to_int(u32(93))] = -nc.im;
        out[19 + to_int(u32(94))] = na.re;
        out[19 + to_int(u32(95))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), c) + cmul(complex(0.000000, 1.000000), a);
        let wc = cmul(complex(0.000000, 1.000000), c) + cmul(complex(0.000000, -1.000000), a);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(32))] = na.re;
        out[19 + to_int(u32(33))] = na.im;
        out[19 + to_int(u32(34))] = nb.re;
        out[19 + to_int(u32(35))] = nb.im;
        out[19 + to_int(u32(36))] = nc.re;
        out[19 + to_int(u32(37))] = nc.im;
        out[19 + to_int(u32(38))] = nd.re;
        out[19 + to_int(u32(39))] = nd.im;
        out[19 + to_int(u32(96))] = nd.re;
        out[19 + to_int(u32(97))] = nd.im;
        out[19 + to_int(u32(98))] = -nb.re;
        out[19 + to_int(u32(99))] = -nb.im;
        out[19 + to_int(u32(100))] = -nc.re;
        out[19 + to_int(u32(101))] = -nc.im;
        out[19 + to_int(u32(102))] = na.re;
        out[19 + to_int(u32(103))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, 1.000000), b);
        let wc = cmul(complex(0.000000, -1.000000), b);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(40))] = na.re;
        out[19 + to_int(u32(41))] = na.im;
        out[19 + to_int(u32(42))] = nb.re;
        out[19 + to_int(u32(43))] = nb.im;
        out[19 + to_int(u32(44))] = nc.re;
        out[19 + to_int(u32(45))] = nc.im;
        out[19 + to_int(u32(46))] = nd.re;
        out[19 + to_int(u32(47))] = nd.im;
        out[19 + to_int(u32(104))] = nd.re;
        out[19 + to_int(u32(105))] = nd.im;
        out[19 + to_int(u32(106))] = -nb.re;
        out[19 + to_int(u32(107))] = -nb.im;
        out[19 + to_int(u32(108))] = -nc.re;
        out[19 + to_int(u32(109))] = -nc.im;
        out[19 + to_int(u32(110))] = na.re;
        out[19 + to_int(u32(111))] = na.im;
        let wa = complex(1.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), a);
        let wc = cmul(complex(1.000000, 0.000000), a);
        let wd = cmul(complex(-1.000000, 0.000000), cmul(b, b)) + cmul(complex(1.000000, 0.000000), cmul(a, a));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(48))] = na.re;
        out[19 + to_int(u32(49))] = na.im;
        out[19 + to_int(u32(50))] = nb.re;
        out[19 + to_int(u32(51))] = nb.im;
        out[19 + to_int(u32(52))] = nc.re;
        out[19 + to_int(u32(53))] = nc.im;
        out[19 + to_int(u32(54))] = nd.re;
        out[19 + to_int(u32(55))] = nd.im;
        out[19 + to_int(u32(112))] = nd.re;
        out[19 + to_int(u32(113))] = nd.im;
        out[19 + to_int(u32(114))] = -nb.re;
        out[19 + to_int(u32(115))] = -nb.im;
        out[19 + to_int(u32(116))] = -nc.re;
        out[19 + to_int(u32(117))] = -nc.im;
        out[19 + to_int(u32(118))] = na.re;
        out[19 + to_int(u32(119))] = na.im;
        let wa = complex(-0.577350, 0.000000);
        let wb = cmul(complex(-1.154701, 0.000000), c) + cmul(complex(0.577350, 0.000000), a);
        let wc = cmul(complex(-1.154701, 0.000000), c) + cmul(complex(0.577350, 0.000000), a);
        let wd = cmul(complex(-1.154701, 0.000000), cmul(c, c)) + cmul(complex(0.577350, 0.000000), cmul(b, b)) + cmul(complex(0.577350, 0.000000), cmul(a, a));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(56))] = na.re;
        out[19 + to_int(u32(57))] = na.im;
        out[19 + to_int(u32(58))] = nb.re;
        out[19 + to_int(u32(59))] = nb.im;
        out[19 + to_int(u32(60))] = nc.re;
        out[19 + to_int(u32(61))] = nc.im;
        out[19 + to_int(u32(62))] = nd.re;
        out[19 + to_int(u32(63))] = nd.im;
        out[19 + to_int(u32(120))] = nd.re;
        out[19 + to_int(u32(121))] = nd.im;
        out[19 + to_int(u32(122))] = -nb.re;
        out[19 + to_int(u32(123))] = -nb.im;
        out[19 + to_int(u32(124))] = -nc.re;
        out[19 + to_int(u32(125))] = -nc.im;
        out[19 + to_int(u32(126))] = na.re;
        out[19 + to_int(u32(127))] = na.im;
    }
    if (group == u32(0)) {
        let wa = complex(-2.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), b) + cmul(complex(-1.000000, 0.000000), a);
        let wc = cmul(complex(1.000000, 0.000000), b) + cmul(complex(-1.000000, 0.000000), a);
        let wd = cmul(complex(2.000000, 0.000000), cmul(a, b));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(0))] = na.re;
        out[19 + to_int(u32(1))] = na.im;
        out[19 + to_int(u32(2))] = nb.re;
        out[19 + to_int(u32(3))] = nb.im;
        out[19 + to_int(u32(4))] = nc.re;
        out[19 + to_int(u32(5))] = nc.im;
        out[19 + to_int(u32(6))] = nd.re;
        out[19 + to_int(u32(7))] = nd.im;
        out[19 + to_int(u32(24))] = nd.re;
        out[19 + to_int(u32(25))] = nd.im;
        out[19 + to_int(u32(26))] = -nb.re;
        out[19 + to_int(u32(27))] = -nb.im;
        out[19 + to_int(u32(28))] = -nc.re;
        out[19 + to_int(u32(29))] = -nc.im;
        out[19 + to_int(u32(30))] = na.re;
        out[19 + to_int(u32(31))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), b) + cmul(complex(0.000000, -1.000000), a);
        let wc = cmul(complex(0.000000, 1.000000), b) + cmul(complex(0.000000, 1.000000), a);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(8))] = na.re;
        out[19 + to_int(u32(9))] = na.im;
        out[19 + to_int(u32(10))] = nb.re;
        out[19 + to_int(u32(11))] = nb.im;
        out[19 + to_int(u32(12))] = nc.re;
        out[19 + to_int(u32(13))] = nc.im;
        out[19 + to_int(u32(14))] = nd.re;
        out[19 + to_int(u32(15))] = nd.im;
        out[19 + to_int(u32(32))] = nd.re;
        out[19 + to_int(u32(33))] = nd.im;
        out[19 + to_int(u32(34))] = -nb.re;
        out[19 + to_int(u32(35))] = -nb.im;
        out[19 + to_int(u32(36))] = -nc.re;
        out[19 + to_int(u32(37))] = -nc.im;
        out[19 + to_int(u32(38))] = na.re;
        out[19 + to_int(u32(39))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(1.000000, 0.000000), b) + cmul(complex(1.000000, 0.000000), a);
        let wc = cmul(complex(1.000000, 0.000000), b) + cmul(complex(1.000000, 0.000000), a);
        let wd = cmul(complex(-1.000000, 0.000000), cmul(b, b)) + cmul(complex(1.000000, 0.000000), cmul(a, a));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(16))] = na.re;
        out[19 + to_int(u32(17))] = na.im;
        out[19 + to_int(u32(18))] = nb.re;
        out[19 + to_int(u32(19))] = nb.im;
        out[19 + to_int(u32(20))] = nc.re;
        out[19 + to_int(u32(21))] = nc.im;
        out[19 + to_int(u32(22))] = nd.re;
        out[19 + to_int(u32(23))] = nd.im;
        out[19 + to_int(u32(40))] = nd.re;
        out[19 + to_int(u32(41))] = nd.im;
        out[19 + to_int(u32(42))] = -nb.re;
        out[19 + to_int(u32(43))] = -nb.im;
        out[19 + to_int(u32(44))] = -nc.re;
        out[19 + to_int(u32(45))] = -nc.im;
        out[19 + to_int(u32(46))] = na.re;
        out[19 + to_int(u32(47))] = na.im;
    }
    if (group == u32(2)) {
        let wa = complex(2.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), b) + cmul(complex(1.000000, 0.000000), a);
        let wc = cmul(complex(1.000000, 0.000000), b) + cmul(complex(1.000000, 0.000000), a);
        let wd = cmul(complex(2.000000, 0.000000), cmul(a, b));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(0))] = na.re;
        out[19 + to_int(u32(1))] = na.im;
        out[19 + to_int(u32(2))] = nb.re;
        out[19 + to_int(u32(3))] = nb.im;
        out[19 + to_int(u32(4))] = nc.re;
        out[19 + to_int(u32(5))] = nc.im;
        out[19 + to_int(u32(6))] = nd.re;
        out[19 + to_int(u32(7))] = nd.im;
        out[19 + to_int(u32(120))] = nd.re;
        out[19 + to_int(u32(121))] = nd.im;
        out[19 + to_int(u32(122))] = -nb.re;
        out[19 + to_int(u32(123))] = -nb.im;
        out[19 + to_int(u32(124))] = -nc.re;
        out[19 + to_int(u32(125))] = -nc.im;
        out[19 + to_int(u32(126))] = na.re;
        out[19 + to_int(u32(127))] = na.im;
        let wa = complex(-2.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), c) + cmul(complex(-1.000000, 0.000000), a);
        let wc = cmul(complex(1.000000, 0.000000), c) + cmul(complex(-1.000000, 0.000000), a);
        let wd = cmul(complex(2.000000, 0.000000), cmul(a, c));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(8))] = na.re;
        out[19 + to_int(u32(9))] = na.im;
        out[19 + to_int(u32(10))] = nb.re;
        out[19 + to_int(u32(11))] = nb.im;
        out[19 + to_int(u32(12))] = nc.re;
        out[19 + to_int(u32(13))] = nc.im;
        out[19 + to_int(u32(14))] = nd.re;
        out[19 + to_int(u32(15))] = nd.im;
        out[19 + to_int(u32(128))] = nd.re;
        out[19 + to_int(u32(129))] = nd.im;
        out[19 + to_int(u32(130))] = -nb.re;
        out[19 + to_int(u32(131))] = -nb.im;
        out[19 + to_int(u32(132))] = -nc.re;
        out[19 + to_int(u32(133))] = -nc.im;
        out[19 + to_int(u32(134))] = na.re;
        out[19 + to_int(u32(135))] = na.im;
        let wa = complex(-2.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), d) + cmul(complex(-1.000000, 0.000000), a);
        let wc = cmul(complex(1.000000, 0.000000), d) + cmul(complex(-1.000000, 0.000000), a);
        let wd = cmul(complex(2.000000, 0.000000), cmul(a, d));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(16))] = na.re;
        out[19 + to_int(u32(17))] = na.im;
        out[19 + to_int(u32(18))] = nb.re;
        out[19 + to_int(u32(19))] = nb.im;
        out[19 + to_int(u32(20))] = nc.re;
        out[19 + to_int(u32(21))] = nc.im;
        out[19 + to_int(u32(22))] = nd.re;
        out[19 + to_int(u32(23))] = nd.im;
        out[19 + to_int(u32(136))] = nd.re;
        out[19 + to_int(u32(137))] = nd.im;
        out[19 + to_int(u32(138))] = -nb.re;
        out[19 + to_int(u32(139))] = -nb.im;
        out[19 + to_int(u32(140))] = -nc.re;
        out[19 + to_int(u32(141))] = -nc.im;
        out[19 + to_int(u32(142))] = na.re;
        out[19 + to_int(u32(143))] = na.im;
        let wa = complex(-2.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), c) + cmul(complex(-1.000000, 0.000000), b);
        let wc = cmul(complex(1.000000, 0.000000), c) + cmul(complex(-1.000000, 0.000000), b);
        let wd = cmul(complex(2.000000, 0.000000), cmul(b, c));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(24))] = na.re;
        out[19 + to_int(u32(25))] = na.im;
        out[19 + to_int(u32(26))] = nb.re;
        out[19 + to_int(u32(27))] = nb.im;
        out[19 + to_int(u32(28))] = nc.re;
        out[19 + to_int(u32(29))] = nc.im;
        out[19 + to_int(u32(30))] = nd.re;
        out[19 + to_int(u32(31))] = nd.im;
        out[19 + to_int(u32(144))] = nd.re;
        out[19 + to_int(u32(145))] = nd.im;
        out[19 + to_int(u32(146))] = -nb.re;
        out[19 + to_int(u32(147))] = -nb.im;
        out[19 + to_int(u32(148))] = -nc.re;
        out[19 + to_int(u32(149))] = -nc.im;
        out[19 + to_int(u32(150))] = na.re;
        out[19 + to_int(u32(151))] = na.im;
        let wa = complex(-2.000000, 0.000000);
        let wb = cmul(complex(1.000000, 0.000000), d) + cmul(complex(-1.000000, 0.000000), b);
        let wc = cmul(complex(1.000000, 0.000000), d) + cmul(complex(-1.000000, 0.000000), b);
        let wd = cmul(complex(2.000000, 0.000000), cmul(b, d));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(32))] = na.re;
        out[19 + to_int(u32(33))] = na.im;
        out[19 + to_int(u32(34))] = nb.re;
        out[19 + to_int(u32(35))] = nb.im;
        out[19 + to_int(u32(36))] = nc.re;
        out[19 + to_int(u32(37))] = nc.im;
        out[19 + to_int(u32(38))] = nd.re;
        out[19 + to_int(u32(39))] = nd.im;
        out[19 + to_int(u32(152))] = nd.re;
        out[19 + to_int(u32(153))] = nd.im;
        out[19 + to_int(u32(154))] = -nb.re;
        out[19 + to_int(u32(155))] = -nb.im;
        out[19 + to_int(u32(156))] = -nc.re;
        out[19 + to_int(u32(157))] = -nc.im;
        out[19 + to_int(u32(158))] = na.re;
        out[19 + to_int(u32(159))] = na.im;
        let wa = complex(2.000000, 0.000000);
        let wb = cmul(complex(-1.000000, 0.000000), d) + cmul(complex(-1.000000, 0.000000), c);
        let wc = cmul(complex(-1.000000, 0.000000), d) + cmul(complex(-1.000000, 0.000000), c);
        let wd = cmul(complex(2.000000, 0.000000), cmul(c, d));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(40))] = na.re;
        out[19 + to_int(u32(41))] = na.im;
        out[19 + to_int(u32(42))] = nb.re;
        out[19 + to_int(u32(43))] = nb.im;
        out[19 + to_int(u32(44))] = nc.re;
        out[19 + to_int(u32(45))] = nc.im;
        out[19 + to_int(u32(46))] = nd.re;
        out[19 + to_int(u32(47))] = nd.im;
        out[19 + to_int(u32(160))] = nd.re;
        out[19 + to_int(u32(161))] = nd.im;
        out[19 + to_int(u32(162))] = -nb.re;
        out[19 + to_int(u32(163))] = -nb.im;
        out[19 + to_int(u32(164))] = -nc.re;
        out[19 + to_int(u32(165))] = -nc.im;
        out[19 + to_int(u32(166))] = na.re;
        out[19 + to_int(u32(167))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), b) + cmul(complex(0.000000, 1.000000), a);
        let wc = cmul(complex(0.000000, 1.000000), b) + cmul(complex(0.000000, -1.000000), a);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(48))] = na.re;
        out[19 + to_int(u32(49))] = na.im;
        out[19 + to_int(u32(50))] = nb.re;
        out[19 + to_int(u32(51))] = nb.im;
        out[19 + to_int(u32(52))] = nc.re;
        out[19 + to_int(u32(53))] = nc.im;
        out[19 + to_int(u32(54))] = nd.re;
        out[19 + to_int(u32(55))] = nd.im;
        out[19 + to_int(u32(168))] = nd.re;
        out[19 + to_int(u32(169))] = nd.im;
        out[19 + to_int(u32(170))] = -nb.re;
        out[19 + to_int(u32(171))] = -nb.im;
        out[19 + to_int(u32(172))] = -nc.re;
        out[19 + to_int(u32(173))] = -nc.im;
        out[19 + to_int(u32(174))] = na.re;
        out[19 + to_int(u32(175))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), c) + cmul(complex(0.000000, -1.000000), a);
        let wc = cmul(complex(0.000000, 1.000000), c) + cmul(complex(0.000000, 1.000000), a);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(56))] = na.re;
        out[19 + to_int(u32(57))] = na.im;
        out[19 + to_int(u32(58))] = nb.re;
        out[19 + to_int(u32(59))] = nb.im;
        out[19 + to_int(u32(60))] = nc.re;
        out[19 + to_int(u32(61))] = nc.im;
        out[19 + to_int(u32(62))] = nd.re;
        out[19 + to_int(u32(63))] = nd.im;
        out[19 + to_int(u32(176))] = nd.re;
        out[19 + to_int(u32(177))] = nd.im;
        out[19 + to_int(u32(178))] = -nb.re;
        out[19 + to_int(u32(179))] = -nb.im;
        out[19 + to_int(u32(180))] = -nc.re;
        out[19 + to_int(u32(181))] = -nc.im;
        out[19 + to_int(u32(182))] = na.re;
        out[19 + to_int(u32(183))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), d) + cmul(complex(0.000000, -1.000000), a);
        let wc = cmul(complex(0.000000, 1.000000), d) + cmul(complex(0.000000, 1.000000), a);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(64))] = na.re;
        out[19 + to_int(u32(65))] = na.im;
        out[19 + to_int(u32(66))] = nb.re;
        out[19 + to_int(u32(67))] = nb.im;
        out[19 + to_int(u32(68))] = nc.re;
        out[19 + to_int(u32(69))] = nc.im;
        out[19 + to_int(u32(70))] = nd.re;
        out[19 + to_int(u32(71))] = nd.im;
        out[19 + to_int(u32(184))] = nd.re;
        out[19 + to_int(u32(185))] = nd.im;
        out[19 + to_int(u32(186))] = -nb.re;
        out[19 + to_int(u32(187))] = -nb.im;
        out[19 + to_int(u32(188))] = -nc.re;
        out[19 + to_int(u32(189))] = -nc.im;
        out[19 + to_int(u32(190))] = na.re;
        out[19 + to_int(u32(191))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), c) + cmul(complex(0.000000, -1.000000), b);
        let wc = cmul(complex(0.000000, 1.000000), c) + cmul(complex(0.000000, 1.000000), b);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(72))] = na.re;
        out[19 + to_int(u32(73))] = na.im;
        out[19 + to_int(u32(74))] = nb.re;
        out[19 + to_int(u32(75))] = nb.im;
        out[19 + to_int(u32(76))] = nc.re;
        out[19 + to_int(u32(77))] = nc.im;
        out[19 + to_int(u32(78))] = nd.re;
        out[19 + to_int(u32(79))] = nd.im;
        out[19 + to_int(u32(192))] = nd.re;
        out[19 + to_int(u32(193))] = nd.im;
        out[19 + to_int(u32(194))] = -nb.re;
        out[19 + to_int(u32(195))] = -nb.im;
        out[19 + to_int(u32(196))] = -nc.re;
        out[19 + to_int(u32(197))] = -nc.im;
        out[19 + to_int(u32(198))] = na.re;
        out[19 + to_int(u32(199))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, -1.000000), d) + cmul(complex(0.000000, -1.000000), b);
        let wc = cmul(complex(0.000000, 1.000000), d) + cmul(complex(0.000000, 1.000000), b);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(80))] = na.re;
        out[19 + to_int(u32(81))] = na.im;
        out[19 + to_int(u32(82))] = nb.re;
        out[19 + to_int(u32(83))] = nb.im;
        out[19 + to_int(u32(84))] = nc.re;
        out[19 + to_int(u32(85))] = nc.im;
        out[19 + to_int(u32(86))] = nd.re;
        out[19 + to_int(u32(87))] = nd.im;
        out[19 + to_int(u32(200))] = nd.re;
        out[19 + to_int(u32(201))] = nd.im;
        out[19 + to_int(u32(202))] = -nb.re;
        out[19 + to_int(u32(203))] = -nb.im;
        out[19 + to_int(u32(204))] = -nc.re;
        out[19 + to_int(u32(205))] = -nc.im;
        out[19 + to_int(u32(206))] = na.re;
        out[19 + to_int(u32(207))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(0.000000, 1.000000), d) + cmul(complex(0.000000, -1.000000), c);
        let wc = cmul(complex(0.000000, -1.000000), d) + cmul(complex(0.000000, 1.000000), c);
        let wd = complex(0.0, 0.0);
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(88))] = na.re;
        out[19 + to_int(u32(89))] = na.im;
        out[19 + to_int(u32(90))] = nb.re;
        out[19 + to_int(u32(91))] = nb.im;
        out[19 + to_int(u32(92))] = nc.re;
        out[19 + to_int(u32(93))] = nc.im;
        out[19 + to_int(u32(94))] = nd.re;
        out[19 + to_int(u32(95))] = nd.im;
        out[19 + to_int(u32(208))] = nd.re;
        out[19 + to_int(u32(209))] = nd.im;
        out[19 + to_int(u32(210))] = -nb.re;
        out[19 + to_int(u32(211))] = -nb.im;
        out[19 + to_int(u32(212))] = -nc.re;
        out[19 + to_int(u32(213))] = -nc.im;
        out[19 + to_int(u32(214))] = na.re;
        out[19 + to_int(u32(215))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(-1.000000, 0.000000), b) + cmul(complex(1.000000, 0.000000), a);
        let wc = cmul(complex(-1.000000, 0.000000), b) + cmul(complex(1.000000, 0.000000), a);
        let wd = cmul(complex(-1.000000, 0.000000), cmul(b, b)) + cmul(complex(1.000000, 0.000000), cmul(a, a));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(96))] = na.re;
        out[19 + to_int(u32(97))] = na.im;
        out[19 + to_int(u32(98))] = nb.re;
        out[19 + to_int(u32(99))] = nb.im;
        out[19 + to_int(u32(100))] = nc.re;
        out[19 + to_int(u32(101))] = nc.im;
        out[19 + to_int(u32(102))] = nd.re;
        out[19 + to_int(u32(103))] = nd.im;
        out[19 + to_int(u32(216))] = nd.re;
        out[19 + to_int(u32(217))] = nd.im;
        out[19 + to_int(u32(218))] = -nb.re;
        out[19 + to_int(u32(219))] = -nb.im;
        out[19 + to_int(u32(220))] = -nc.re;
        out[19 + to_int(u32(221))] = -nc.im;
        out[19 + to_int(u32(222))] = na.re;
        out[19 + to_int(u32(223))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(1.154701, 0.000000), c) + cmul(complex(0.577350, 0.000000), b) + cmul(complex(0.577350, 0.000000), a);
        let wc = cmul(complex(1.154701, 0.000000), c) + cmul(complex(0.577350, 0.000000), b) + cmul(complex(0.577350, 0.000000), a);
        let wd = cmul(complex(-1.154701, 0.000000), cmul(c, c)) + cmul(complex(0.577350, 0.000000), cmul(b, b)) + cmul(complex(0.577350, 0.000000), cmul(a, a));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(104))] = na.re;
        out[19 + to_int(u32(105))] = na.im;
        out[19 + to_int(u32(106))] = nb.re;
        out[19 + to_int(u32(107))] = nb.im;
        out[19 + to_int(u32(108))] = nc.re;
        out[19 + to_int(u32(109))] = nc.im;
        out[19 + to_int(u32(110))] = nd.re;
        out[19 + to_int(u32(111))] = nd.im;
        out[19 + to_int(u32(224))] = nd.re;
        out[19 + to_int(u32(225))] = nd.im;
        out[19 + to_int(u32(226))] = -nb.re;
        out[19 + to_int(u32(227))] = -nb.im;
        out[19 + to_int(u32(228))] = -nc.re;
        out[19 + to_int(u32(229))] = -nc.im;
        out[19 + to_int(u32(230))] = na.re;
        out[19 + to_int(u32(231))] = na.im;
        let wa = complex(0.0, 0.0);
        let wb = cmul(complex(1.224745, 0.000000), d) + cmul(complex(-0.408248, 0.000000), c) + cmul(complex(0.408248, 0.000000), b) + cmul(complex(0.408248, 0.000000), a);
        let wc = cmul(complex(1.224745, 0.000000), d) + cmul(complex(-0.408248, 0.000000), c) + cmul(complex(0.408248, 0.000000), b) + cmul(complex(0.408248, 0.000000), a);
        let wd = cmul(complex(-1.224745, 0.000000), cmul(d, d)) + cmul(complex(0.408248, 0.000000), cmul(c, c)) + cmul(complex(0.408248, 0.000000), cmul(b, b)) + cmul(complex(0.408248, 0.000000), cmul(a, a));
        let tr = wa + wd;
        if (tr.re * tr.re + tr.im * tr.im < 0.09) {
            wa = wa + complex(plug, 0.0);
        }
        let dtm = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(dtm);
        let na = cdiv(wa, sd);
        let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd);
        let nd = cdiv(wd, sd);
        out[19 + to_int(u32(112))] = na.re;
        out[19 + to_int(u32(113))] = na.im;
        out[19 + to_int(u32(114))] = nb.re;
        out[19 + to_int(u32(115))] = nb.im;
        out[19 + to_int(u32(116))] = nc.re;
        out[19 + to_int(u32(117))] = nc.im;
        out[19 + to_int(u32(118))] = nd.re;
        out[19 + to_int(u32(119))] = nd.im;
        out[19 + to_int(u32(232))] = nd.re;
        out[19 + to_int(u32(233))] = nd.im;
        out[19 + to_int(u32(234))] = -nb.re;
        out[19 + to_int(u32(235))] = -nb.im;
        out[19 + to_int(u32(236))] = -nc.re;
        out[19 + to_int(u32(237))] = -nc.im;
        out[19 + to_int(u32(238))] = na.re;
        out[19 + to_int(u32(239))] = na.im;
    }
    #{ slots: out }
}

fn init(p) {
    #{ st0: 0.0, creg: 0.0 }
}

fn su_cmul(x, y) {
    complex(x.re * y.re - x.im * y.im, x.re * y.im + x.im * y.re)
}

fn su_cdiv(x, y) {
    let dn = y.norm_sqr() + 1.0e-30;
    complex(x.re * y.re + x.im * y.im, x.im * y.re - x.re * y.im) / dn
}

fn su_matinv(P) {
    let det = su_cmul(P[0], P[3]) - su_cmul(P[1], P[2]);
    [su_cdiv(P[3], det), su_cdiv(-P[1], det), su_cdiv(-P[2], det), su_cdiv(P[0], det)]
}

fn su_matmul(P, Q) {
    [su_cmul(P[0], Q[0]) + su_cmul(P[1], Q[2]), su_cmul(P[0], Q[1]) + su_cmul(P[1], Q[3]), su_cmul(P[2], Q[0]) + su_cmul(P[3], Q[2]), su_cmul(P[2], Q[1]) + su_cmul(P[3], Q[3])]
}

fn su_conjugator(theta, eta, delta) {
    let ch = cosh(eta);
    let sh = sinh(eta);
    let ct = cos(theta);
    let st = sin(theta);
    let ca = complex(ct * ch, -st * sh);
    let sa = complex(st * ch, ct * sh);
    let qf = [ca, complex(-sa.re, -sa.im), sa, ca];
    let dk = [complex(1.0, delta), complex(1.0, 0.0), complex(1.0, 0.0), complex(1.0, -delta)];
    let s0 = [complex(vec4(0.7071068, 0.0, 0.0, -0.7071068).x, vec4(0.7071068, 0.0, 0.0, -0.7071068).y), complex(vec4(0.7071068, 0.0, 0.0, -0.7071068).z, vec4(0.7071068, 0.0, 0.0, -0.7071068).w), complex(vec4(0.0, -0.7071068, 0.7071068, 0.0).x, vec4(0.0, -0.7071068, 0.7071068, 0.0).y), complex(vec4(0.0, -0.7071068, 0.7071068, 0.0).z, vec4(0.0, -0.7071068, 0.7071068, 0.0).w)];
    su_matmul(su_matmul(dk, s0), qf)
}

fn su_apply_m(base, z, cj, cji) {
    let m = su_matmul(su_matmul(cj, base), cji);
    su_cdiv(su_cmul(m[0], z) + m[1], su_cmul(m[2], z) + m[3])
}

fn su_qmul(a, b) {
    vec4(a.x * b.x - a.y * b.y - a.z * b.z - a.w * b.w, a.x * b.y + a.y * b.x + a.z * b.w - a.w * b.z, a.x * b.z - a.y * b.w + a.z * b.x + a.w * b.y, a.x * b.w + a.y * b.z - a.z * b.y + a.w * b.x)
}

fn su_qinv(a) {
    let n = (a.x * a.x + a.y * a.y + a.z * a.z + a.w * a.w) + 1.0e-30;
    vec4(a.x, -a.y, -a.z, -a.w) / n
}

fn su_apply_m3(base, p3, cj, cji) {
    let m = su_matmul(su_matmul(cj, base), cji);
    let t1 = m[0];
    let qa = vec4(t1.re, t1.im, 0.0, 0.0);
    let t2 = m[1];
    let qb = vec4(t2.re, t2.im, 0.0, 0.0);
    let t3 = m[2];
    let qc = vec4(t3.re, t3.im, 0.0, 0.0);
    let t4 = m[3];
    let qd = vec4(t4.re, t4.im, 0.0, 0.0);
    let q = vec4(p3.x, p3.y, abs(p3.z) + 1.0e-4, 0.0);
    let r = su_qmul(su_qmul(qa, q) + qb, su_qinv(su_qmul(qc, q) + qd));
    vec3(r.x, r.y, r.z)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let group = u32(to_float(p.group));
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let theta = p.conj_angle * 0.01745329252;
    let eta = p.conj_hyper * 0.01745329252;
    let delta = p.qc_strength;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let cnt = u32(16);
    if (group == u32(0)) {
        cnt = u32(6);
    } else if (group == u32(2)) {
        cnt = u32(30);
    }
    let half = cnt / u32(2);
    let cj = su_conjugator(theta, eta, delta);
    let cji = su_matinv(cj);
    let k = min(u32(rand() * to_float(cnt)), cnt - u32(1));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < cnt && k == (prev + half) % cnt) {
            k = (k + u32(1)) % cnt;
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let bo = u32(19) + k * u32(8);
    let ba = [complex(p.slots[to_int(bo)], p.slots[to_int(bo + u32(1))]), complex(p.slots[to_int(bo + u32(2))], p.slots[to_int(bo + u32(3))]), complex(p.slots[to_int(bo + u32(4))], p.slots[to_int(bo + u32(5))]), complex(p.slots[to_int(bo + u32(6))], p.slots[to_int(bo + u32(7))])];
    let out = su_apply_m(ba, z, cj, cji);
    let symm = u32(to_float(p.symmetry));
    if (symm == u32(1)) {
        if (rand() < 0.5) {
            out = -out;
        }
    } else if (symm == u32(2)) {
        if (rand() < 0.5) {
            out = complex(out.re, -out.im);
        }
    } else if (symm == u32(3)) {
        let r = min(u32(rand() * 4.0), u32(3));
        if (r == u32(1)) {
            out = -out;
        } else if (r == u32(2)) {
            out = complex(out.re, -out.im);
        } else if (r == u32(3)) {
            out = complex(-out.re, out.im);
        }
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let group = u32(to_float(p.group));
    let avoid = to_float(p.avoid_reversal) > 0.5;
    let theta = p.conj_angle * 0.01745329252;
    let eta = p.conj_hyper * 0.01745329252;
    let delta = p.qc_strength;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let color_speed = p.color_speed;
    let cnt = u32(16);
    if (group == u32(0)) {
        cnt = u32(6);
    } else if (group == u32(2)) {
        cnt = u32(30);
    }
    let half = cnt / u32(2);
    let cj = su_conjugator(theta, eta, delta);
    let cji = su_matinv(cj);
    let k = min(u32(rand() * to_float(cnt)), cnt - u32(1));
    if (avoid) {
        let prev = u32(this.st0);
        if (prev < cnt && k == (prev + half) % cnt) {
            k = (k + u32(1)) % cnt;
        }
    }
    this.st0 = to_float(k);
    if (dc_mode == u32(1)) {
        let creg = this.creg;
        creg = mix(creg, fract((to_float(k) + 0.5) / to_float(cnt) * dc_scale), color_speed);
        this.creg = creg;
        color = creg;
    }
    let bo = u32(19) + k * u32(8);
    let ba = [complex(p.slots[to_int(bo)], p.slots[to_int(bo + u32(1))]), complex(p.slots[to_int(bo + u32(2))], p.slots[to_int(bo + u32(3))]), complex(p.slots[to_int(bo + u32(4))], p.slots[to_int(bo + u32(5))]), complex(p.slots[to_int(bo + u32(6))], p.slots[to_int(bo + u32(7))])];
    let space = u32(to_float(p.space));
    let out = vec3(0.0, 0.0, 0.0);
    if (space == u32(1)) {
        out = su_apply_m3(ba, v, cj, cji);
    } else {
        let t1 = su_apply_m(ba, complex(v.x, v.y), cj, cji);
        out = vec3(t1.re, t1.im, v.z);
    }
    let symm = u32(to_float(p.symmetry));
    if (symm == u32(1)) {
        if (rand() < 0.5) {
            out = vec3(-out.x, -out.y, out.z);
        }
    } else if (symm == u32(2)) {
        if (rand() < 0.5) {
            out = vec3(out.x, -out.y, out.z);
        }
    } else if (symm == u32(3)) {
        let r = min(u32(rand() * 4.0), u32(3));
        if (r == u32(1)) {
            out = vec3(-out.x, -out.y, out.z);
        } else if (r == u32(2)) {
            out = vec3(out.x, -out.y, out.z);
        } else if (r == u32(3)) {
            out = vec3(-out.x, out.y, out.z);
        }
    }
    #{ z: out, color: color }
}
