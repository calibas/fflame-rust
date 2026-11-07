/// Random row-or-column tile blur — flips a coin to decide whether to tile
/// along rows or columns. On a row pass, X is shifted by `xtilesize ·
/// round(±xwidth · log(rand))` to a random nearby tile, while Y gets
/// piecewise fmod-folded around `xmod1`. Columns work analogously. Produces
/// grid-like tile-shifted blur patterns.
///
/// # Authors
/// - Brad Stefanov

// Intersection, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xwidth: #{
        value: 5.0, min: -50.0, max: 50.0, free: true,
        caption: "X Width",
        hint: "Row-branch X-shift magnitude (multiplied by `log(rand)`).",
    },
    xtilesize: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "X Tile",
        hint: "Row-branch X tile size — final X is `xtilesize · (x + shift)`.",
    },
    xmod1: #{
        value: 0.3, min: -10.0, max: 10.0, free: true,
        caption: "X Mod 1",
        hint: "Row-branch Y fold threshold.",
    },
    xmod2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Mod 2",
        hint: "Row-branch Y fold period (multiplied with `xmod1` to form the modulus).",
    },
    xheight: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "X Height",
        hint: "Row-branch Y output scale.",
    },
    yheight: #{
        value: 5.0, min: -50.0, max: 50.0, free: true,
        caption: "Y Height",
        hint: "Column-branch Y-shift magnitude.",
    },
    ytilesize: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y Tile",
        hint: "Column-branch Y tile size.",
    },
    ymod1: #{
        value: 0.3, min: -10.0, max: 10.0, free: true,
        caption: "Y Mod 1",
        hint: "Column-branch X fold threshold.",
    },
    ymod2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Mod 2",
        hint: "Column-branch X fold period.",
    },
    ywidth: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y Width",
        hint: "Column-branch X output scale.",
    },
};

const INFO = #{ title: "Intersection", name: "rhai_intersection", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ xr1: p.xmod2 * p.xmod1, yr1: p.ymod2 * p.ymod1 }
}

fn variation(z, p, ctx) {
    let xwidth = p.xwidth;
    let xtilesize = p.xtilesize;
    let xmod1 = p.xmod1;
    let xheight = p.xheight;
    let yheight = p.yheight;
    let ytilesize = p.ytilesize;
    let ymod1 = p.ymod1;
    let ywidth = p.ywidth;
    let xr1 = p.xr1;
    let yr1 = p.yr1;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let safe_xr1 = (if abs(xr1) < 1.0e-30 { 1.0e-30 } else { xr1 });
    let safe_yr1 = (if abs(yr1) < 1.0e-30 { 1.0e-30 } else { yr1 });
    let fx = 0.0;
    let fy = 0.0;
    if (rand() < 0.5) {
        let x_sign_choice = (if rand() < 0.5 { xwidth } else { -xwidth });
        let log_rand = ln(max(1.0e-30, rand()));
        fx = xtilesize * (z.re + round_even(x_sign_choice * log_rand));
        if (z.im > xmod1) {
            let v = z.im + xmod1;
            fy = xheight * (-xmod1 + (v - to_float(floor(v / safe_xr1)) * safe_xr1));
        } else if (z.im < -xmod1) {
            let v = xmod1 - z.im;
            fy = xheight * (xmod1 - (v - to_float(floor(v / safe_xr1)) * safe_xr1));
        } else {
            fy = xheight * z.im;
        }
    } else {
        let y_sign_choice = (if rand() < 0.5 { yheight } else { -yheight });
        let log_rand = ln(max(1.0e-30, rand()));
        fy = ytilesize * (z.im + round_even(y_sign_choice * log_rand));
        if (z.re > ymod1) {
            let v = z.re + ymod1;
            fx = ywidth * (-ymod1 + (v - to_float(floor(v / safe_yr1)) * safe_yr1));
        } else if (z.re < -ymod1) {
            let v = ymod1 - z.re;
            fx = ywidth * (ymod1 - (v - to_float(floor(v / safe_yr1)) * safe_yr1));
        } else {
            fx = ywidth * z.re;
        }
    }
    complex(fx * inv_w, fy * inv_w)
}
