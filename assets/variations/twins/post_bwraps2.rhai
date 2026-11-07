/// Bubble Wrap warp with a third `g2` formula — same Bubble Wrap algorithm
/// as `bwraps`/`bwraps7` (each grid cell hosts a bubble; inside the bubble
/// the input twists toward the cell center, outside it passes through), but
/// with `g2 = gain² / cellsize + ε` (where `bwraps` divides by `radius` and
/// `bwraps7` doesn't divide at all). Applied at post-phase, on the
/// accumulated output.
///
/// # Authors
/// - Xyrus02

// Post BWraps 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cellsize: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cell Size",
        hint: "Grid cell size.",
    },
    space: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Space",
        hint: "Inter-cell spacing factor. 0 = no space; larger = smaller bubble relative to cell.",
    },
    gain: #{
        value: 2.0, min: -5.0, max: 5.0, free: true,
        caption: "Gain",
        hint: "Bubble strength — controls both bubble size and twist intensity.",
    },
    inner_twist: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Inner Twist",
        hint: "Rotation amount applied at the bubble center.",
    },
    outer_twist: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Outer Twist",
        hint: "Rotation amount applied at the bubble edge. Interpolated radially with `inner_twist`.",
    },
};

const INFO = #{ title: "Post BWraps 2", name: "rhai_post_bwraps2", category: "advanced_2d", phase: "post" };

fn prepare(p) {
    let cellsize = p.cellsize;
    let space = p.space;
    let gain = p.gain;
    let out = array(3, 0.0);
    if (abs(cellsize) < 1.0e-30) {
        out[0] = 0.0;
        out[1] = 0.0;
        out[2] = 0.0;
    } else {
        let radius = 0.5 * (cellsize / (1.0 + space * space));
        let g2 = gain * gain / cellsize + 1.0e-6;
        let max_bubble = g2 * radius;
        if (max_bubble > 2.0) {
            max_bubble = 1.0;
        } else {
            max_bubble = max_bubble * (1.0 / ((max_bubble * max_bubble) * 0.25 + 1.0));
        }
        out[0] = g2;
        out[1] = radius * radius;
        out[2] = radius / max(1.0e-30, max_bubble);
    }
    #{ g2: out[0], r2: out[1], rfactor: out[2] }
}

fn variation(z, p, ctx) {
    let cellsize = p.cellsize;
    let inner_twist = p.inner_twist;
    let outer_twist = p.outer_twist;
    let g2 = p.g2;
    let r2 = p.r2;
    let rfactor = p.rfactor;
    let w = ctx.weight;
    if (abs(cellsize) < 1.0e-30) {
        return complex(w * z.re, w * z.im);
    }
    let cx = (to_float(floor(z.re / cellsize)) + 0.5) * cellsize;
    let cy = (to_float(floor(z.im / cellsize)) + 0.5) * cellsize;
    let lx = z.re - cx;
    let ly = z.im - cy;
    if ((lx * lx + ly * ly) > r2) {
        return complex(w * z.re, w * z.im);
    }
    lx = lx * g2;
    ly = ly * g2;
    let rb = rfactor / ((lx * lx + ly * ly) * 0.25 + 1.0);
    lx = lx * rb;
    ly = ly * rb;
    let rr = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - rr) + outer_twist * rr;
    let st = sin(theta);
    let ct = cos(theta);
    complex(w * (cx + ct * lx + st * ly), w * (cy - st * lx + ct * ly))
}
