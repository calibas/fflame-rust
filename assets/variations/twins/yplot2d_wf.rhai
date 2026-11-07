/// 2D curve plot extruded into a Z ribbon — samples a random `x`,
/// evaluates a preset curve formula `y = f(x)`, samples `z` independently,
/// and emits `(x, y, z)`. The 14 presets are JWildfire's stock yplot2d
/// formulas (harmonics, chirps, power curves, square/triangle waves; some
/// take `param_a`/`param_b`). Custom JWF formulas are not supported.
///
/// # Authors
/// - Andreas Maschke
/// - Rick Sidwell

// Y-Plot 2D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    preset_id: #{
        value: 0, min: -1, max: 13,
        caption: "Preset ID",
        hint: "Which baked formula preset to plot (0-13). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as y = 0.",
    },
    xmin: #{
        value: -3.0, min: -10.0, max: 10.0, free: true,
        caption: "X Min",
        hint: "Lower X bound of the sampled interval (swapped with xmax if larger).",
    },
    xmax: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "X Max",
        hint: "Upper X bound of the sampled interval.",
    },
    ymin: #{
        value: -4.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Min",
        hint: "Lower Y bound — only used to normalize the Y color mode (the curve itself is unclamped).",
    },
    ymax: #{
        value: 4.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Max",
        hint: "Upper Y bound for the Y color mode.",
    },
    zmin: #{
        value: -2.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Min",
        hint: "Lower Z bound of the ribbon extrusion (swapped with zmax if larger).",
    },
    zmax: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Max",
        hint: "Upper Z bound of the ribbon extrusion.",
    },
    direct_color: #{
        value: true,
        caption: "Direct Color",
        hint: "When on, writes the color register from the sample position per Color Mode. Visible color requires the transform's Direct Color slider > 0.",
    },
    color_mode: #{
        value: 1, choices: ["Colormap (unsupported)", "X", "Y"],
        caption: "Color Mode",
        hint: "Position→palette mapping. X/Y normalize the corresponding coordinate over its range. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded.",
    },
    blend_colormap: #{
        value: 1, min: 0, max: 1,
        caption: "Blend Colormap",
        hint: "Unused — colormap images are unsupported. Preserved for JWF XML round-trip (JWF default 1).",
    },
    displ_amount: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Displ Amount",
        hint: "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 0.1).",
    },
    blend_displ_map: #{
        value: 1, min: 0, max: 1,
        caption: "Blend Displ Map",
        hint: "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 1).",
    },
    param_a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param A",
        hint: "Formula input `param_a` — used by presets 3, 4, 6-13 (JWF applies each preset's own default when selected; set it manually here).",
    },
    param_b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param B",
        hint: "Formula input `param_b` — used by presets 6, 8, 10-13.",
    },
    param_c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param C",
        hint: "Formula input `param_c` — unused by the stock presets.",
    },
    param_d: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param D",
        hint: "Formula input `param_d` — unused by the stock presets.",
    },
    param_e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param E",
        hint: "Formula input `param_e` — unused by the stock presets.",
    },
    param_f: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param F",
        hint: "Formula input `param_f` — unused by the stock presets.",
    },
};

const INFO = #{ title: "Y-Plot 2D WF", name: "rhai_yplot2d_wf", category: "full_3d", phase: "any" };

fn prepare(p) {
    let out = array(6, 0.0);
    let xmin = p.xmin;
    let xmax = p.xmax;
    if (xmin > xmax) {
        let t = xmin;
        xmin = xmax;
        xmax = t;
    }
    out[0] = xmin;
    out[1] = xmax - xmin;
    let ymin = p.ymin;
    let ymax = p.ymax;
    if (ymin > ymax) {
        let t = ymin;
        ymin = ymax;
        ymax = t;
    }
    out[2] = ymin;
    out[3] = ymax - ymin;
    let zmin = p.zmin;
    let zmax = p.zmax;
    if (zmin > zmax) {
        let t = zmin;
        zmin = zmax;
        zmax = t;
    }
    out[4] = zmin;
    out[5] = zmax - zmin;
    #{ xmin_s: out[0], dx: out[1], ymin_s: out[2], dy: out[3], zmin_s: out[4], dz: out[5] }
}

fn yplot2d_wf_sqr(v) {
    v * v
}

fn yplot2d_wf_formula(id, x, param_a, param_b, param_c, param_d, param_e, param_f) {
    let pi = 3.14159265358979;
    if (id == 0) {
        return (sin(x) + 2.0 * sin(2.0 * x) + 1.0 * sin(4.0 * x));
    }
    if (id == 1) {
        return sin(x) * cos(x);
    }
    if (id == 2) {
        return sin(2.0 * x * x);
    }
    if (id == 3) {
        return sin(param_a * x) / cos(x * x);
    }
    if (id == 4) {
        return sin(x + sin(x) / param_a);
    }
    if (id == 5) {
        return ln(abs(x));
    }
    if (id == 6) {
        return abs(sin(x) * param_a) + abs(cos(x) * param_b);
    }
    if (id == 7) {
        return (if x > 0.0 { pow(x, param_a) } else { -pow(-x, param_a) });
    }
    if (id == 8) {
        return param_a * x + param_b;
    }
    if (id == 9) {
        return pow(x, param_a);
    }
    if (id == 10) {
        return param_a * sin(param_b * x);
    }
    if (id == 11) {
        return param_a * pow(-1.0, to_float(floor(x * param_b)));
    }
    if (id == 12) {
        return 2.0 * param_a / param_b * abs(abs(x) % param_b - param_b / 2.0) - 2.0 * param_a / 4.0;
    }
    if (id == 13) {
        return to_float(floor(param_a * x)) / param_b;
    }
    0.0
}

fn variation(w, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let pa = p.param_a;
    let pb = p.param_b;
    let pc = p.param_c;
    let pd = p.param_d;
    let pe = p.param_e;
    let pf = p.param_f;
    let xmin_s = p.xmin_s;
    let dx = p.dx;
    let ymin_s = p.ymin_s;
    let dy = p.dy;
    let zmin_s = p.zmin_s;
    let dz = p.dz;
    let rand_u = rand();
    let rand_v = rand();
    let x = xmin_s + rand_u * dx;
    let z = zmin_s + rand_v * dz;
    let y = yplot2d_wf_formula(preset_id, x, pa, pb, pc, pd, pe, pf);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode != 0) {
            c = (x - xmin_s) / dx;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: complex(x, y), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let pa = p.param_a;
    let pb = p.param_b;
    let pc = p.param_c;
    let pd = p.param_d;
    let pe = p.param_e;
    let pf = p.param_f;
    let xmin_s = p.xmin_s;
    let dx = p.dx;
    let ymin_s = p.ymin_s;
    let dy = p.dy;
    let zmin_s = p.zmin_s;
    let dz = p.dz;
    let rand_u = rand();
    let rand_v = rand();
    let x = xmin_s + rand_u * dx;
    let z = zmin_s + rand_v * dz;
    let y = yplot2d_wf_formula(preset_id, x, pa, pb, pc, pd, pe, pf);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode != 0) {
            c = (x - xmin_s) / dx;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: vec3(x, y, z), color: color }
}
