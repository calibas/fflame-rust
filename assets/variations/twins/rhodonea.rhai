/// Rose curves (rhodonea) — `r = cos(k·θ) + radial_offset` with `k =
/// knumer/kdenom`. Maps incoming points onto the rose with 7 inner modes
/// (when `|rin| ≤ |r|`) and 7 outer modes (when `|rin| > |r|`), giving 49
/// spread/mask combinations. The init logic computes the minimum cycle
/// count needed to close the curve, handling integer `k` (1 or ½ cycle
/// depending on parity), rational `k` (Euclidean gcd reduction), and
/// irrational `k` (16-cycle fallback). `metacycle_expansion` distorts
/// amplitude across cycles for spiral-like radial growth; `fill` adds
/// uniform random radial jitter.
///
/// # Authors
/// - CozyG

// Rhodonea, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    knumer: #{
        value: 3.0, min: -50.0, max: 50.0, free: true,
        caption: "K Numer",
        hint: "Rose-curve `k = knumer/kdenom` numerator. Integer `k` produces `k` petals (even) or `2k` petals (odd); rational `k` produces nested petal patterns with `cycles_to_close = kdenom` cycles.",
    },
    kdenom: #{
        value: 4.0, min: -50.0, max: 50.0, free: true,
        caption: "K Denom",
        hint: "Rose-curve `k = knumer/kdenom` denominator.",
    },
    radial_offset: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Radial Offset",
        hint: "Additive offset to the radial formula: `r = cos(k·θ) + radial_offset`. Non-zero values create an inner loop or distort the petals away from the origin.",
    },
    inner_mode: #{
        value: 1, choices: ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Hide", "Pass Through"],
        caption: "Inner Mode",
        hint: "Behavior for points inside the rose curve (`|rin| ≤ |r|`).",
    },
    outer_mode: #{
        value: 1, choices: ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Pass Through", "Hide"],
        caption: "Outer Mode",
        hint: "Behavior for points outside the rose curve (`|rin| > |r|`). Modes 0-4 mirror the spread formulas of `inner_mode` (using `outer_spread` instead). The mask modes (Pass Through / Hide) have the opposite ordering from `inner_mode` — each label describes what happens at *this* side, so the underlying inversion is invisible to the user.",
    },
    inner_spread: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Inner Spread",
        hint: "Spread amplitude for inner modes 1-4. Scales how strongly the input position influences the displacement from the rose curve.",
    },
    outer_spread: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Outer Spread",
        hint: "Spread amplitude for outer modes 1-4.",
    },
    inner_spread_ratio: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Inner Spread Ratio",
        hint: "X/Y asymmetry for the inner spread. 1.0 = symmetric; other values stretch the spread along X relative to Y.",
    },
    outer_spread_ratio: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Outer Spread Ratio",
        hint: "X/Y asymmetry for the outer spread.",
    },
    spread_split: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread Split",
        hint: "Scale factor for the input-radius comparison: `rin = spread_split · |p|`. Larger values make the curve appear smaller (more inputs classify as outside).",
    },
    cycles: #{
        value: 0.0, min: 0.0, max: 100.0, free: true,
        caption: "Cycles",
        hint: "Number of θ cycles to traverse per iteration. 0 = auto-compute as `cycles_to_close × metacycles`; non-zero overrides the auto-compute.",
    },
    cycle_offset: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cycle Offset",
        hint: "Phase offset for θ in fractions of 2π. Rotates the rose orientation.",
    },
    metacycle_expansion: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Metacycle Expansion",
        hint: "Per-metacycle radial expansion factor. Non-zero values cause the rose amplitude to grow linearly across subsequent metacycles, creating spiral patterns.",
    },
    metacycles: #{
        value: 1.0, min: 0.0, max: 100.0, free: true,
        caption: "Metacycles",
        hint: "Number of metacycles when `cycles == 0`. Each metacycle is one `cycles_to_close` worth of θ.",
    },
    fill: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Fill",
        hint: "Random radial jitter amplitude. Adds `fill · (rng - 0.5)` to `r` per iteration, filling in the rose shape with noise.",
    },
};

const INFO = #{ title: "Rhodonea", name: "rhai_rhodonea", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let knumer = p.knumer;
    let kdenom = p.kdenom;
    let radial_offset = p.radial_offset;
    let inner_spread = p.inner_spread;
    let outer_spread = p.outer_spread;
    let cycles_param = p.cycles;
    let metacycles = p.metacycles;
    let fill = p.fill;
    let kn = knumer;
    let kd = kdenom;
    let safe_kd = (if abs(kd) < 1.0e-30 { 1.0e-30 } else { kd });
    let k = kn / safe_kd;
    let cycles_to_close = 0.0;
    let k_is_integer = abs(k - to_float(floor(k + 0.5))) < 1.0e-6;
    let kn_is_integer = abs(kn - to_float(floor(kn + 0.5))) < 1.0e-6;
    let kd_is_integer = abs(kd - to_float(floor(kd + 0.5))) < 1.0e-6;
    let k_int = to_float(floor(k + 0.5));
    if (k_is_integer) {
        let k_even = abs(k_int - to_float(floor(k_int * 0.5)) * 2.0) < 0.5;
        if (k_even) {
            cycles_to_close = 1.0;
        } else {
            let has_offset = (radial_offset != 0.0) || (inner_spread != 0.0) || (outer_spread != 0.0) || (fill != 0.0);
            if (has_offset) {
                cycles_to_close = 1.0;
            } else {
                cycles_to_close = 0.5;
            }
        }
    } else if (kn_is_integer && kd_is_integer) {
        let a = abs(to_float(floor(kn + 0.5)));
        let b = abs(to_float(floor(kd + 0.5)));
        for i in 0..64 {
            if (b < 0.5) {
                break;
            }
            let t = b;
            b = a - to_float(floor(a / b)) * b;
            a = t;
        }
        let gcd = max(1.0, a);
        if (gcd > 1.5) {
            kn = kn / gcd;
            kd = kd / gcd;
        }
        let kn_even = abs(kn - to_float(floor(kn * 0.5)) * 2.0) < 0.5;
        let kd_even = abs(kd - to_float(floor(kd * 0.5)) * 2.0) < 0.5;
        if (kn_even || kd_even) {
            cycles_to_close = kd;
        } else {
            cycles_to_close = kd * 0.5;
        }
    } else {
        cycles_to_close = 2.0 * kn * kd;
        if (cycles_param < 16.0) {
            cycles_to_close = 16.0;
        }
    }
    let cycles_final = 0.0;
    if (cycles_param == 0.0) {
        cycles_final = cycles_to_close * metacycles;
    } else {
        cycles_final = cycles_param;
    }
    #{ d0: kn, d1: kd, k: k, cycles_to_close: cycles_to_close, d4: cycles_final }
}

fn variation_rhodonea_2d(pv, p, ctx) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let radial_offset = p.radial_offset;
    let inner_mode = to_int(to_float(p.inner_mode));
    let outer_mode = to_int(to_float(p.outer_mode));
    let inner_spread = p.inner_spread;
    let outer_spread = p.outer_spread;
    let inner_ratio = p.inner_spread_ratio;
    let outer_ratio = p.outer_spread_ratio;
    let spread_split = p.spread_split;
    let cycle_offset = p.cycle_offset;
    let meta_expand = p.metacycle_expansion;
    let fill = p.fill;
    let k = p.k;
    let cycles_to_close = p.cycles_to_close;
    let cycles = p.d4;
    let weight = ctx.weight;
    let safe_w = (if abs(weight) < 1.0e-30 { 1.0e-30 } else { weight });
    let rin = spread_split * pv.abs();
    let tin = atan2(pv.im, pv.re);
    let t = cycles * (tin + cycle_offset * two_pi);
    let r = cos(k * t) + radial_offset;
    if (fill != 0.0) {
        r = r + fill * (rand() - 0.5);
    }
    let x_curve = r * cos(t);
    let y_curve = r * sin(t);
    let safe_close = (if abs(cycles_to_close) < 1.0e-30 { 1.0e-30 } else { cycles_to_close * two_pi });
    let expansion = to_float(floor((cycles * (tin + pi)) / safe_close));
    let amt_factor = 1.0 + expansion * meta_expand / safe_w;
    let ox = 0.0;
    let oy = 0.0;
    if (abs(rin) > abs(r)) {
        if (outer_mode == 0) {
            ox = amt_factor * x_curve;
            oy = amt_factor * y_curve;
        } else if (outer_mode == 1) {
            let rinx = rin * outer_spread * outer_ratio - outer_spread * outer_ratio + 1.0;
            let riny = rin * outer_spread - outer_spread + 1.0;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 2) {
            let xin = abs(pv.re);
            let yin = abs(pv.im);
            if (x_curve < 0.0) {
                xin = -xin;
            }
            if (y_curve < 0.0) {
                yin = -yin;
            }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * (xin - x_curve));
            oy = amt_factor * (y_curve + outer_spread * (yin - y_curve));
        } else if (outer_mode == 3) {
            let xin = abs(pv.re);
            let yin = abs(pv.im);
            if (x_curve < 0.0) {
                xin = -xin;
            }
            if (y_curve < 0.0) {
                yin = -yin;
            }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * xin);
            oy = amt_factor * (y_curve + outer_spread * yin);
        } else if (outer_mode == 4) {
            let rinx = 0.5 * rin + outer_spread * outer_ratio;
            let riny = 0.5 * rin + outer_spread;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 5) {
            ox = pv.re / safe_w;
            oy = pv.im / safe_w;
        } else if (outer_mode == 6) {
            this = true;
        }
    } else if (inner_mode == 0) {
        ox = amt_factor * x_curve;
        oy = amt_factor * y_curve;
    } else if (inner_mode == 1) {
        let rinx = rin * inner_spread * inner_ratio - inner_spread * inner_ratio + 1.0;
        let riny = rin * inner_spread - inner_spread + 1.0;
        ox = amt_factor * rinx * x_curve;
        oy = amt_factor * riny * y_curve;
    } else if (inner_mode == 2) {
        let xin = abs(pv.re);
        let yin = abs(pv.im);
        if (x_curve < 0.0) {
            xin = -xin;
        }
        if (y_curve < 0.0) {
            yin = -yin;
        }
        ox = amt_factor * (x_curve - inner_spread * inner_ratio * (x_curve - xin));
        oy = amt_factor * (y_curve - inner_spread * (y_curve - yin));
    } else if (inner_mode == 3) {
        let xin = abs(pv.re);
        let yin = abs(pv.im);
        if (x_curve < 0.0) {
            xin = -xin;
        }
        if (y_curve < 0.0) {
            yin = -yin;
        }
        ox = amt_factor * (x_curve - inner_spread * inner_ratio * xin);
        oy = amt_factor * (y_curve - inner_spread * yin);
    } else if (inner_mode == 4) {
        let rinx = 0.5 * rin + inner_spread * inner_ratio;
        let riny = 0.5 * rin + inner_spread;
        ox = amt_factor * rinx * x_curve;
        oy = amt_factor * riny * y_curve;
    } else if (inner_mode == 6) {
        ox = pv.re / safe_w;
        oy = pv.im / safe_w;
    } else if (inner_mode == 5) {
        this = true;
    }
    complex(ox, oy)
}

fn variation(z, p, ctx) {
    let hidden = ctx.hide;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let radial_offset = p.radial_offset;
    let inner_mode = to_int(to_float(p.inner_mode));
    let outer_mode = to_int(to_float(p.outer_mode));
    let inner_spread = p.inner_spread;
    let outer_spread = p.outer_spread;
    let inner_ratio = p.inner_spread_ratio;
    let outer_ratio = p.outer_spread_ratio;
    let spread_split = p.spread_split;
    let cycle_offset = p.cycle_offset;
    let meta_expand = p.metacycle_expansion;
    let fill = p.fill;
    let k = p.k;
    let cycles_to_close = p.cycles_to_close;
    let cycles = p.d4;
    let weight = ctx.weight;
    let safe_w = (if abs(weight) < 1.0e-30 { 1.0e-30 } else { weight });
    let rin = spread_split * z.abs();
    let tin = atan2(z.im, z.re);
    let t = cycles * (tin + cycle_offset * two_pi);
    let r = cos(k * t) + radial_offset;
    if (fill != 0.0) {
        r = r + fill * (rand() - 0.5);
    }
    let x_curve = r * cos(t);
    let y_curve = r * sin(t);
    let safe_close = (if abs(cycles_to_close) < 1.0e-30 { 1.0e-30 } else { cycles_to_close * two_pi });
    let expansion = to_float(floor((cycles * (tin + pi)) / safe_close));
    let amt_factor = 1.0 + expansion * meta_expand / safe_w;
    let ox = 0.0;
    let oy = 0.0;
    if (abs(rin) > abs(r)) {
        if (outer_mode == 0) {
            ox = amt_factor * x_curve;
            oy = amt_factor * y_curve;
        } else if (outer_mode == 1) {
            let rinx = rin * outer_spread * outer_ratio - outer_spread * outer_ratio + 1.0;
            let riny = rin * outer_spread - outer_spread + 1.0;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 2) {
            let xin = abs(z.re);
            let yin = abs(z.im);
            if (x_curve < 0.0) {
                xin = -xin;
            }
            if (y_curve < 0.0) {
                yin = -yin;
            }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * (xin - x_curve));
            oy = amt_factor * (y_curve + outer_spread * (yin - y_curve));
        } else if (outer_mode == 3) {
            let xin = abs(z.re);
            let yin = abs(z.im);
            if (x_curve < 0.0) {
                xin = -xin;
            }
            if (y_curve < 0.0) {
                yin = -yin;
            }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * xin);
            oy = amt_factor * (y_curve + outer_spread * yin);
        } else if (outer_mode == 4) {
            let rinx = 0.5 * rin + outer_spread * outer_ratio;
            let riny = 0.5 * rin + outer_spread;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 5) {
            ox = z.re / safe_w;
            oy = z.im / safe_w;
        } else if (outer_mode == 6) {
            hidden = true;
        }
    } else if (inner_mode == 0) {
        ox = amt_factor * x_curve;
        oy = amt_factor * y_curve;
    } else if (inner_mode == 1) {
        let rinx = rin * inner_spread * inner_ratio - inner_spread * inner_ratio + 1.0;
        let riny = rin * inner_spread - inner_spread + 1.0;
        ox = amt_factor * rinx * x_curve;
        oy = amt_factor * riny * y_curve;
    } else if (inner_mode == 2) {
        let xin = abs(z.re);
        let yin = abs(z.im);
        if (x_curve < 0.0) {
            xin = -xin;
        }
        if (y_curve < 0.0) {
            yin = -yin;
        }
        ox = amt_factor * (x_curve - inner_spread * inner_ratio * (x_curve - xin));
        oy = amt_factor * (y_curve - inner_spread * (y_curve - yin));
    } else if (inner_mode == 3) {
        let xin = abs(z.re);
        let yin = abs(z.im);
        if (x_curve < 0.0) {
            xin = -xin;
        }
        if (y_curve < 0.0) {
            yin = -yin;
        }
        ox = amt_factor * (x_curve - inner_spread * inner_ratio * xin);
        oy = amt_factor * (y_curve - inner_spread * yin);
    } else if (inner_mode == 4) {
        let rinx = 0.5 * rin + inner_spread * inner_ratio;
        let riny = 0.5 * rin + inner_spread;
        ox = amt_factor * rinx * x_curve;
        oy = amt_factor * riny * y_curve;
    } else if (inner_mode == 6) {
        ox = z.re / safe_w;
        oy = z.im / safe_w;
    } else if (inner_mode == 5) {
        hidden = true;
    }
    #{ z: complex(ox, oy), hide: hidden }
}

fn variation3(v, p, ctx) {
    let hidden = ctx.hide;
    let r2 = hidden.variation_rhodonea_2d(complex(v.x, v.y), p, ctx);
    #{ z: vec3(r2.re, r2.im, v.z), hide: hidden }
}
