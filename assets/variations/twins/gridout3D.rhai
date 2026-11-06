/// Eight-region grid offset — partitions the XY plane into 8 wedges based
/// on `round(p.x)·xx` and `round(p.y)·yy` (the rounding turns the param
/// space into a coarse grid), then adds or subtracts a per-region offset
/// triplet to `(x, y, z)`. The 8 regions split each quadrant along its
/// diagonal; for `y ≤ 0` (regions A-D) the Z offset is added, for `y > 0`
/// (regions E-H) the Z offset is multiplied into Z instead.
///
/// Region table (input X/Y signs after gate-rounding by `xx`/`yy`):
///
/// | Region | Condition (`y`, `x` are gate-rounded) | X op | Y op | Z op |
/// |---|---|---|---|---|
/// | A | `y ≤ 0`, `x > 0`, `-y ≥ x`  | `+xa` | `+ya` | `+za` |
/// | B | `y ≤ 0`, `x > 0`, `-y < x`  | `+xb` | `+yb` | `+zb` |
/// | C | `y ≤ 0`, `x ≤ 0`, `y ≤ x`   | `+xc` | `+yc` | `+zc` |
/// | D | `y ≤ 0`, `x ≤ 0`, `y > x`   | `+xd` | `-yd` | `+zd` |
/// | E | `y > 0`, `x > 0`, `y ≥ x`   | `-xe` | `+ye` | `·ze` |
/// | F | `y > 0`, `x > 0`, `y < x`   | `+xf` | `+yf` | `·zf` |
/// | G | `y > 0`, `x ≤ 0`, `y ≥ -x`  | `-xg` | `+yg` | `·zg` |
/// | H | `y > 0`, `x ≤ 0`, `y < -x`  | `+xh` | `-yh` | `·zh` |
///
/// Sign flips on `-yd`, `-xe`, `-xg`, `-yh` and the add-vs-multiply Z split
/// between A-D and E-H are preserved verbatim from upstream.
///
/// # Authors
/// - Michael Faber
/// - Joel Faber
/// - DarkBeam
/// - Brad Stefanov

// Gridout 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xx: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Gate",
        hint: "X gate scale: input `x` is rounded then multiplied by `xx` before the region comparison. Setting to 0 collapses all regions onto `x = 0` (only the y-half decision still applies).",
    },
    yy: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Gate",
        hint: "Y gate scale: input `y` is rounded then multiplied by `yy` before the region comparison. Setting to 0 collapses all regions onto `y = 0` (only the x-sign decision still applies).",
    },
    xa: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "XA",
        hint: "Region A (y ≤ 0, x > 0, -y ≥ x) X offset, added to X.",
    },
    xb: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "XB",
        hint: "Region B (y ≤ 0, x > 0, -y < x) X offset, added to X.",
    },
    xc: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "XC",
        hint: "Region C (y ≤ 0, x ≤ 0, y ≤ x) X offset, added to X.",
    },
    xd: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "XD",
        hint: "Region D (y ≤ 0, x ≤ 0, y > x) X offset, added to X.",
    },
    xe: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "XE",
        hint: "Region E (y > 0, x > 0, y ≥ x) X offset, subtracted from X.",
    },
    xf: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "XF",
        hint: "Region F (y > 0, x > 0, y < x) X offset, added to X.",
    },
    xg: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "XG",
        hint: "Region G (y > 0, x ≤ 0, y ≥ -x) X offset, subtracted from X.",
    },
    xh: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "XH",
        hint: "Region H (y > 0, x ≤ 0, y < -x) X offset, added to X.",
    },
    ya: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "YA",
        hint: "Region A (y ≤ 0, x > 0, -y ≥ x) Y offset, added to Y.",
    },
    yb: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "YB",
        hint: "Region B (y ≤ 0, x > 0, -y < x) Y offset, added to Y.",
    },
    yc: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "YC",
        hint: "Region C (y ≤ 0, x ≤ 0, y ≤ x) Y offset, added to Y.",
    },
    yd: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "YD",
        hint: "Region D (y ≤ 0, x ≤ 0, y > x) Y offset, subtracted from Y.",
    },
    ye: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "YE",
        hint: "Region E (y > 0, x > 0, y ≥ x) Y offset, added to Y.",
    },
    yf: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "YF",
        hint: "Region F (y > 0, x > 0, y < x) Y offset, added to Y.",
    },
    yg: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "YG",
        hint: "Region G (y > 0, x ≤ 0, y ≥ -x) Y offset, added to Y.",
    },
    yh: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "YH",
        hint: "Region H (y > 0, x ≤ 0, y < -x) Y offset, subtracted from Y.",
    },
    za: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "ZA",
        hint: "Region A (y ≤ 0, x > 0, -y ≥ x) Z offset, added to Z.",
    },
    zb: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "ZB",
        hint: "Region B (y ≤ 0, x > 0, -y < x) Z offset, added to Z.",
    },
    zc: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "ZC",
        hint: "Region C (y ≤ 0, x ≤ 0, y ≤ x) Z offset, added to Z.",
    },
    zd: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "ZD",
        hint: "Region D (y ≤ 0, x ≤ 0, y > x) Z offset, added to Z.",
    },
    ze: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "ZE",
        hint: "Region E (y > 0, x > 0, y ≥ x) Z offset, multiplied into Z (regions E-H multiply; A-D add).",
    },
    zf: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "ZF",
        hint: "Region F (y > 0, x > 0, y < x) Z offset, multiplied into Z (regions E-H multiply; A-D add).",
    },
    zg: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "ZG",
        hint: "Region G (y > 0, x ≤ 0, y ≥ -x) Z offset, multiplied into Z (regions E-H multiply; A-D add).",
    },
    zh: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "ZH",
        hint: "Region H (y > 0, x ≤ 0, y < -x) Z offset, multiplied into Z (regions E-H multiply; A-D add).",
    },
};

const INFO = #{ title: "Gridout 3D", name: "rhai_gridout3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let xx = p.xx;
    let yy = p.yy;
    let xa = p.xa;
    let xb = p.xb;
    let xc = p.xc;
    let xd = p.xd;
    let xe = p.xe;
    let xf = p.xf;
    let xg = p.xg;
    let xh = p.xh;
    let ya = p.ya;
    let yb = p.yb;
    let yc = p.yc;
    let yd = p.yd;
    let ye = p.ye;
    let yf = p.yf;
    let yg = p.yg;
    let yh = p.yh;
    let x = round_even(z.re) * xx;
    let y = round_even(z.im) * yy;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) {
                return complex(z.re + xa, z.im + ya);
            }
            return complex(z.re + xb, z.im + yb);
        }
        if (y <= x) {
            return complex(z.re + xc, z.im + yc);
        }
        return complex(z.re + xd, z.im - yd);
    }
    if (x > 0.0) {
        if (y >= x) {
            return complex(z.re - xe, z.im + ye);
        }
        return complex(z.re + xf, z.im + yf);
    }
    if (y >= -x) {
        return complex(z.re - xg, z.im + yg);
    }
    complex(z.re + xh, z.im - yh)
}

fn variation3(v, p) {
    let xx = p.xx;
    let yy = p.yy;
    let xa = p.xa;
    let xb = p.xb;
    let xc = p.xc;
    let xd = p.xd;
    let xe = p.xe;
    let xf = p.xf;
    let xg = p.xg;
    let xh = p.xh;
    let ya = p.ya;
    let yb = p.yb;
    let yc = p.yc;
    let yd = p.yd;
    let ye = p.ye;
    let yf = p.yf;
    let yg = p.yg;
    let yh = p.yh;
    let za = p.za;
    let zb = p.zb;
    let zc = p.zc;
    let zd = p.zd;
    let ze = p.ze;
    let zf = p.zf;
    let zg = p.zg;
    let zh = p.zh;
    let x = round_even(v.x) * xx;
    let y = round_even(v.y) * yy;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) {
                return vec3(v.x + xa, v.y + ya, v.z + za);
            }
            return vec3(v.x + xb, v.y + yb, v.z + zb);
        }
        if (y <= x) {
            return vec3(v.x + xc, v.y + yc, v.z + zc);
        }
        return vec3(v.x + xd, v.y - yd, v.z + zd);
    }
    if (x > 0.0) {
        if (y >= x) {
            return vec3(v.x - xe, v.y + ye, v.z * ze);
        }
        return vec3(v.x + xf, v.y + yf, v.z * zf);
    }
    if (y >= -x) {
        return vec3(v.x - xg, v.y + yg, v.z * zg);
    }
    vec3(v.x + xh, v.y - yh, v.z * zh)
}
