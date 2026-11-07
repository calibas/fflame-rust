/// Hexagonal Voronoi cell warp with Voronoi-distance direct color — tiles
/// the plane into hex cells, warps each by `power`/`scale`/`rotate`, and
/// writes the Voronoi distance to the colour register (`L·color_scale +
/// color_offset`, clamped). Produces coloured hexagonal-lattice textures.
///
/// # Authors
/// - slobo777
/// - Andreas Maschke

// DC Hexes WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cellsize: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Cell Size",
        hint: "Hex cell size. 0 disables the spatial warp entirely (Z still passes through).",
    },
    power: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Exponent on the Voronoi distance for the target radius: `trgL = (L1 + 1e-6)^power · scale`.",
    },
    rotate: #{
        value: 0.166, min: -10.0, max: 10.0, free: true,
        caption: "Rotate",
        hint: "Per-cell rotation, in turns (multiplied by 2π). 0.166 ≈ 1/6 turn aligns with the hex symmetry.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Per-cell scale factor applied to the warped radius.",
    },
    color_scale: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Color Scale",
        hint: "Multiplier on the Voronoi distance `L` for the colour index. Visible colour requires the transform's Direct Color slider > 0.",
    },
    color_offset: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color Offset",
        hint: "Offset added to `L·color_scale` before clamping the colour index to [0, 1].",
    },
};

const INFO = #{ title: "DC Hexes WF", name: "rhai_dc_hexes_wf", category: "plugin", phase: "any", z: "preserve_z" };

fn hexes_cell_centre(x, y, s) {
    let c = 0.86602540378443864;
    complex((1.5 * to_float(x) - 1.5 * to_float(y)) * s, (c * to_float(x) + c * to_float(y)) * s)
}

fn hexes_closest9(P, U) {
    let d2min = 1.0e32;
    let j = 0;
    for i in 0..9 {
        let d = P[i] - U;
        let d2 = d.norm_sqr();
        if (d2 < d2min) {
            d2min = d2;
            j = i;
        }
    }
    j
}

fn hexes_cell_choice(q) {
    [q / 3 - 1, q % 3 - 1]
}

fn hexes_vratio(P, Q, U) {
    let pmq = P - Q;
    if (pmq.re == 0.0 && pmq.im == 0.0) {
        return 1.0;
    }
    let t1 = U - Q;
    2.0 * (t1.re * pmq.re + t1.im * pmq.im) / pmq.norm_sqr()
}

fn hexes_voronoi7(P, U) {
    let ratiomax = -1.0e20;
    for i in 1..7 {
        let ratio = hexes_vratio(P[i], P[0], U);
        if (ratio > ratiomax) {
            ratiomax = ratio;
        }
    }
    ratiomax
}

fn hexes_warp(px, py, cellsize, power, rotate, scale, color_scale, color_offset) {
    let s = cellsize;
    let two_pi = 6.28318530717959;
    let rot_sin = sin(rotate * two_pi);
    let rot_cos = cos(rotate * two_pi);
    let a_hex = 1.0 / 3.0;
    let b_hex = 0.5773502691896258;
    let c_hex = -1.0 / 3.0;
    let d_hex = 0.5773502691896258;
    let U = complex(px, py);
    let hx = to_int(to_float(floor((a_hex * U.re + b_hex * U.im) / s)));
    let hy = to_int(to_float(floor((c_hex * U.re + d_hex * U.im) / s)));
    let P = array(9, complex(0.0, 0.0));
    let idx = 0;
    for di in -1..2 {
        for dj in -1..2 {
            P[idx] = hexes_cell_centre(hx + di, hy + dj, s);
            idx = idx + 1;
        }
    }
    let q = hexes_closest9(P, U);
    let ch = hexes_cell_choice(q);
    hx = hx + ch[0];
    hy = hy + ch[1];
    P[0] = hexes_cell_centre(hx, hy, s);
    P[1] = hexes_cell_centre(hx, hy + 1, s);
    P[2] = hexes_cell_centre(hx + 1, hy + 1, s);
    P[3] = hexes_cell_centre(hx + 1, hy, s);
    P[4] = hexes_cell_centre(hx, hy - 1, s);
    P[5] = hexes_cell_centre(hx - 1, hy - 1, s);
    P[6] = hexes_cell_centre(hx - 1, hy, s);
    let l1 = hexes_voronoi7(P, U);
    let dxo = U.re - P[0].re;
    let dyo = U.im - P[0].im;
    let trg_l = pow(max(1.0e-06, l1 + 1.0e-06), power) * scale;
    let vx = dxo * rot_cos + dyo * rot_sin;
    let vy = -dxo * rot_sin + dyo * rot_cos;
    let u2 = complex(vx + P[0].re, vy + P[0].im);
    let l2 = hexes_voronoi7(P, u2);
    let l = max(l1, l2);
    let r = 0.0;
    if (l < 0.5) {
        r = trg_l / l1;
    } else if (l > 0.8) {
        r = trg_l / l2;
    } else {
        r = ((trg_l / l1) * (0.8 - l) + (trg_l / l2) * (l - 0.5)) / 0.3;
    }
    vx = vx * r + P[0].re;
    vy = vy * r + P[0].im;
    this = clamp(l * color_scale + color_offset, 0.0, 1.0);
    complex(vx, vy)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let cellsize = p.cellsize;
    if (cellsize == 0.0) {
        return #{ z: complex(0.0, 0.0), color: color };
    }
    let power = p.power;
    let rotate = p.rotate;
    let scale = p.scale;
    let color_scale = p.color_scale;
    let color_offset = p.color_offset;
    #{ z: color.hexes_warp(z.re, z.im, cellsize, power, rotate, scale, color_scale, color_offset), color: color }
}

fn variation3(q, p, ctx) {
    let color = ctx.color;
    let cellsize = p.cellsize;
    if (cellsize == 0.0) {
        return #{ z: vec3(0.0, 0.0, q.z), color: color };
    }
    let power = p.power;
    let rotate = p.rotate;
    let scale = p.scale;
    let color_scale = p.color_scale;
    let color_offset = p.color_offset;
    let v = color.hexes_warp(q.x, q.y, cellsize, power, rotate, scale, color_scale, color_offset);
    #{ z: vec3(v.re, v.im, q.z), color: color }
}
