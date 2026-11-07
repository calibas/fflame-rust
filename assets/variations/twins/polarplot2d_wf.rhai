/// Polar curve plot extruded into a Z ribbon — samples a random angle `t`,
/// evaluates a preset radius formula `r = f(t)`, and emits
/// `(r·cos t, r·sin t, z)`. The 16 presets are JWildfire's stock polar
/// curves (Archimedes/hyperbolic/logarithmic spirals, rose curves,
/// cardioids, lemniscates; most take `param_a..c`). Custom JWF formulas
/// are not supported.
///
/// # Authors
/// - Andreas Maschke

// Polar-Plot 2D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    preset_id: #{
        value: 0, min: -1, max: 15,
        caption: "Preset ID",
        hint: "Which baked formula preset to plot (0-15). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as r = 0.",
    },
    tmin: #{
        value: -3.0, min: -100.0, max: 100.0, free: true,
        caption: "T Min",
        hint: "Lower bound of the sampled angle parameter (swapped with tmax if larger).",
    },
    tmax: #{
        value: 2.0, min: -100.0, max: 100.0, free: true,
        caption: "T Max",
        hint: "Upper bound of the sampled angle parameter.",
    },
    rmin: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "R Min",
        hint: "Lower R bound — only used to normalize the R color mode (the radius itself is unclamped).",
    },
    rmax: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "R Max",
        hint: "Upper R bound for the R color mode.",
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
        hint: "When on, writes the color register from the sample per Color Mode. Visible color requires the transform's Direct Color slider > 0.",
    },
    color_mode: #{
        value: 1, choices: ["Colormap (unsupported)", "T", "R"],
        caption: "Color Mode",
        hint: "Sample→palette mapping. T/R normalize the angle/radius over its range. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded.",
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
        hint: "Formula input `param_a` — used by most presets (JWF applies each preset's own default when selected; set it manually here).",
    },
    param_b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param B",
        hint: "Formula input `param_b`.",
    },
    param_c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param C",
        hint: "Formula input `param_c`.",
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

const INFO = #{ title: "Polar-Plot 2D WF", name: "rhai_polarplot2d_wf", category: "full_3d", phase: "any" };

fn prepare(p) {
    let out = array(6, 0.0);
    let tmin = p.tmin;
    let tmax = p.tmax;
    if (tmin > tmax) {
        let t = tmin;
        tmin = tmax;
        tmax = t;
    }
    out[0] = tmin;
    out[1] = tmax - tmin;
    let rmin = p.rmin;
    let rmax = p.rmax;
    if (rmin > rmax) {
        let t = rmin;
        rmin = rmax;
        rmax = t;
    }
    out[2] = rmin;
    out[3] = rmax - rmin;
    let zmin = p.zmin;
    let zmax = p.zmax;
    if (zmin > zmax) {
        let t = zmin;
        zmin = zmax;
        zmax = t;
    }
    out[4] = zmin;
    out[5] = zmax - zmin;
    #{ tmin_s: out[0], dt: out[1], rmin_s: out[2], dr: out[3], zmin_s: out[4], dz: out[5] }
}

fn polarplot2d_wf_sqr(v) {
    v * v
}

fn polarplot2d_wf_formula(id, t, param_a, param_b, param_c, param_d, param_e, param_f) {
    let pi = 3.14159265358979;
    if (id == 0) {
        return t * param_a;
    }
    if (id == 1) {
        return param_a / t;
    }
    if (id == 2) {
        return cos(t * param_a / param_b) + param_c;
    }
    if (id == 3) {
        return param_b + param_a * cos(t);
    }
    if (id == 4) {
        return sqrt(polarplot2d_wf_sqr(param_a) * sin(2.0 * t));
    }
    if (id == 5) {
        return sqrt((polarplot2d_wf_sqr(param_a) * polarplot2d_wf_sqr(sin(t)) - polarplot2d_wf_sqr(param_b) * polarplot2d_wf_sqr(cos(t))) / (polarplot2d_wf_sqr(sin(t)) - polarplot2d_wf_sqr(cos(t))));
    }
    if (id == 6) {
        return t * cos(param_a * t);
    }
    if (id == 7) {
        return sqrt(polarplot2d_wf_sqr(param_a) / t);
    }
    if (id == 8) {
        return sqrt(4.0 * param_b * (param_a - param_b * polarplot2d_wf_sqr(sin(t))));
    }
    if (id == 9) {
        return param_a * t + param_b;
    }
    if (id == 10) {
        return cos(t) * (4.0 * param_a * polarplot2d_wf_sqr(sin(t)) - param_b);
    }
    if (id == 11) {
        return param_a * exp(param_b * t);
    }
    if (id == 12) {
        return sqrt(polarplot2d_wf_sqr(param_a) * t);
    }
    if (id == 13) {
        return pow(sin(t), param_a) + pow(cos(t), param_b);
    }
    if (id == 14) {
        return param_a * sin(t) / t;
    }
    if (id == 15) {
        return param_a + tanh(param_b * sin(param_c * t)) / param_b;
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
    let tmin_s = p.tmin_s;
    let dt = p.dt;
    let rmin_s = p.rmin_s;
    let dr = p.dr;
    let zmin_s = p.zmin_s;
    let dz = p.dz;
    let rand_u = rand();
    let rand_v = rand();
    let t = tmin_s + rand_u * dt;
    let z = zmin_s + rand_v * dz;
    let r = polarplot2d_wf_formula(preset_id, t, pa, pb, pc, pd, pe, pf);
    let x = r * cos(t);
    let y = r * sin(t);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 2) {
            c = (r - rmin_s) / dr;
        } else if (color_mode != 0) {
            c = (t - tmin_s) / dt;
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
    let tmin_s = p.tmin_s;
    let dt = p.dt;
    let rmin_s = p.rmin_s;
    let dr = p.dr;
    let zmin_s = p.zmin_s;
    let dz = p.dz;
    let rand_u = rand();
    let rand_v = rand();
    let t = tmin_s + rand_u * dt;
    let z = zmin_s + rand_v * dz;
    let r = polarplot2d_wf_formula(preset_id, t, pa, pb, pc, pd, pe, pf);
    let x = r * cos(t);
    let y = r * sin(t);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 2) {
            c = (r - rmin_s) / dr;
        } else if (color_mode != 0) {
            c = (t - tmin_s) / dt;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: vec3(x, y, z), color: color }
}
