/// Wraps the plane into a grid of soft bubbles, each with its own internal
/// twist. Same shape as Pre Bwraps and Post Bwraps but applied in the
/// normal weighted-sum phase.

// BWraps, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cellsize: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cell Size",
        hint: "Width of each grid cell — the plane is divided into cells of this size, each becoming a bubble.",
    },
    space: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Space",
        hint: "Gap between cells. 0 = no gap; positive values push the bubbles apart.",
    },
    gain: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Gain",
        hint: "How strongly each bubble wraps its contents inward.",
    },
    inner_twist: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Inner Twist",
        hint: "Rotation (in degrees) applied at the center of each bubble.",
    },
    outer_twist: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Outer Twist",
        hint: "Rotation (in degrees) applied at the edge of each bubble.",
    },
};

const INFO = #{ title: "BWraps", name: "rhai_bwraps", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let cellsize = p.cellsize;
    let space = p.space;
    let gain = p.gain;
    let out = array(3, 0.0);
    if (cellsize == 0.0) {
        out[0] = 0.0;
        out[1] = 0.0;
        out[2] = 0.0;
    } else {
        let radius = 0.5 * (cellsize / (1.0 + space * space));
        let g2 = (gain * gain) / (radius + 1.0e-6) + 1.0e-6;
        let max_bubble = g2 * radius;
        if (max_bubble > 2.0) {
            max_bubble = 1.0;
        } else {
            max_bubble = max_bubble * (1.0 / ((max_bubble * max_bubble) / 4.0 + 1.0));
        }
        out[0] = g2;
        out[1] = radius * radius;
        out[2] = radius / max_bubble;
    }
    #{ g2: out[0], r2: out[1], rfactor: out[2] }
}

fn variation(z, p) {
    let cellsize = p.cellsize;
    let inner_twist = p.inner_twist;
    let outer_twist = p.outer_twist;
    let g2 = p.g2;
    let r2 = p.r2;
    let rfactor = p.rfactor;
    if (cellsize == 0.0) {
        return z;
    }
    let cx = (to_float(floor(z.re / cellsize)) + 0.5) * cellsize;
    let cy = (to_float(floor(z.im / cellsize)) + 0.5) * cellsize;
    let lx = z.re - cx;
    let ly = z.im - cy;
    if ((lx * lx + ly * ly) > r2) {
        return z;
    }
    lx = lx * g2;
    ly = ly * g2;
    let r_dist = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);
    lx = lx * r_dist;
    ly = ly * r_dist;
    let r_ratio = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - r_ratio) + outer_twist * r_ratio;
    let vx = cx + cos(theta) * lx + sin(theta) * ly;
    let vy = cy - sin(theta) * lx + cos(theta) * ly;
    complex(vx, vy)
}
