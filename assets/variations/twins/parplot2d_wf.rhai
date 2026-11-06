/// Parametric surface plot — evaluates `(x, y, z) = F(u, v)` for one of 47
/// baked JWildfire presets (tori, shells, springs, Klein-style surfaces;
/// many take `param_a..f`). `solid` picks random `(u, v)` per iteration
/// (default) vs. mapping the input point into parameter space. Custom JWF
/// formulas are not supported.
///
/// # Authors
/// - Andreas Maschke
/// - Frank Baumann and other contributors (preset formulas)

// Par-Plot 2D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    preset_id: #{
        value: 0, min: -1, max: 46,
        caption: "Preset ID",
        hint: "Which baked formula preset to plot (0-46). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders at the origin.",
    },
    umin: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "U Min",
        hint: "Lower U bound of the parameter rectangle (swapped with umax if larger).",
    },
    umax: #{
        value: 6.2831855, min: -100.0, max: 100.0, free: true,
        caption: "U Max",
        hint: "Upper U bound. JWF default 2π.",
    },
    vmin: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "V Min",
        hint: "Lower V bound of the parameter rectangle.",
    },
    vmax: #{
        value: 6.2831855, min: -100.0, max: 100.0, free: true,
        caption: "V Max",
        hint: "Upper V bound. JWF default 2π.",
    },
    direct_color: #{
        value: true,
        caption: "Direct Color",
        hint: "When on, writes the color register from the (u, v) position per Color Mode. Visible color requires the transform's Direct Color slider > 0.",
    },
    color_mode: #{
        value: 3, choices: ["Colormap (unsupported)", "U", "V", "U×V"],
        caption: "Color Mode",
        hint: "Parameter→palette mapping. U/V normalize the corresponding parameter over its range; U×V multiplies the two. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded.",
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
    solid: #{
        value: true,
        caption: "Solid",
        hint: "When on (JWF default), draws random (u, v) each iteration — a solid surface. When off, maps the input point into parameter space (u from x, v from y) so the chaos game traverses the surface.",
    },
    param_a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param A",
        hint: "Formula input `param_a` — used by many presets (JWF applies each preset's own default when selected; set it manually here).",
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
        hint: "Formula input `param_d`.",
    },
    param_e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param E",
        hint: "Formula input `param_e`.",
    },
    param_f: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Param F",
        hint: "Formula input `param_f`.",
    },
};

const INFO = #{ title: "Par-Plot 2D WF", name: "rhai_parplot2d_wf", category: "full_3d", phase: "any" };

fn prepare(p) {
    let out = array(4, 0.0);
    let umin = p.umin;
    let umax = p.umax;
    if (umin > umax) {
        let t = umin;
        umin = umax;
        umax = t;
    }
    out[0] = umin;
    out[1] = umax - umin;
    let vmin = p.vmin;
    let vmax = p.vmax;
    if (vmin > vmax) {
        let t = vmin;
        vmin = vmax;
        vmax = t;
    }
    out[2] = vmin;
    out[3] = vmax - vmin;
    #{ umin_s: out[0], du: out[1], vmin_s: out[2], dv: out[3] }
}

fn parplot2d_wf_sqr(v) {
    v * v
}

fn parplot2d_wf_formula(id, u, v, param_a, param_b, param_c, param_d, param_e, param_f) {
    let pi = 3.14159265358979;
    if (id == 0) {
        return vec3(cos(u) * (4.0 + cos(v)), sin(u) * (4.0 + cos(v)), 4.0 * sin(2.0 * u) + sin(v) * (1.2 - sin(v)));
    }
    if (id == 1) {
        return vec3(cos(v) * sin(2.0 * u), sin(v) * sin(2.0 * u), sin(2.0 * v) * parplot2d_wf_sqr((cos(u))));
    }
    if (id == 2) {
        return vec3(cos(u) * (exp(u / 10.0) - 1.0) * (cos(v) + 0.8), sin(u) * (exp(u / 10.0) - 1.0) * (cos(v) + 0.8), (exp(u / 10.0) - 1.0) * sin(v));
    }
    if (id == 3) {
        return vec3(cos(v) * (2.0 + sin(u + v / 3.0)), sin(v) * (2.0 + sin(u + v / 3.0)), cos(u + v / 3.0));
    }
    if (id == 4) {
        return vec3(cos(u) * (2.0 + cos(v)), sin(u) * (2.0 + cos(v)), (u - 2.0 * pi) + sin(v));
    }
    if (id == 5) {
        return vec3(u * cos(v), u * sin(v), parplot2d_wf_sqr(cos(4.0 * u)) * exp(0.0 - u));
    }
    if (id == 6) {
        return vec3(cos(u) * (2.0 + parplot2d_wf_sqr(cos(u / 2.0)) * sin(v)), sin(u) * (2.0 + parplot2d_wf_sqr(cos(u / 2.0)) * sin(v)), parplot2d_wf_sqr(cos(u / 2.0)) * cos(v));
    }
    if (id == 7) {
        return vec3(cos(u) * (4.0 + cos(v)), sin(u) * (4.0 + cos(v)), 3.0 * sin(u) + (sin(3.0 * v) * (1.2 + sin(3.0 * v))));
    }
    if (id == 8) {
        return vec3(u * cos(v), v * cos(u), u * v * sin(u) * sin(v));
    }
    if (id == 9) {
        return vec3(cos(u) * sin(v * v * v / (pi * pi)), sin(u) * sin(v), cos(v));
    }
    if (id == 10) {
        return vec3(cos(u) * ((cos(3.0 * u) + 2.0) * sin(v) + 0.5), sin(u) * ((cos(3.0 * u) + 2.0) * sin(v) + 0.5), (cos(3.0 * u) + 2.0) * cos(v));
    }
    if (id == 11) {
        return vec3(sin(u) * sin(v) + 0.05 * cos(20.0 * v), cos(u) * sin(v) + 0.05 * cos(20.0 * u), cos(v));
    }
    if (id == 12) {
        return vec3(2.0 * (1.0 - exp(u / (6.0 * pi))) * cos(u) * parplot2d_wf_sqr(cos(v / 2.0)), 2.0 * (-1.0 + exp(u / (6.0 * pi))) * sin(u) * parplot2d_wf_sqr(cos(v / 2.0)), 1.0 - exp(u / (3.0 * pi)) - sin(v) + exp(u / (6.0 * pi)) * sin(v));
    }
    if (id == 13) {
        return vec3((6.0 + 2.0 * cos(u * v)) * cos(u), (6.0 + 2.0 * cos(u * v)) * sin(u), (2.0 * u + 2.0 * sin(u * v)));
    }
    if (id == 14) {
        return vec3((1.0 + 0.25 * cos(75.0 * u)) * cos(u), (1.0 + 0.25 * cos(75.0 * u)) * sin(u), u + sin(75.0 * u));
    }
    if (id == 15) {
        return vec3(7.83 * cos((v - pi) / 2.0) * (cos(16.4 * v)), 7.83 * cos((v - pi) / 2.0) * (sin(16.4 * v)), 7.83 * sin((v - pi) / 2.0));
    }
    if (id == 16) {
        return vec3((2.0 + sin(7.0 * u + 5.0 * v)) * cos(u) * sin(v), (2.0 + sin(7.0 * u + 5.0 * v)) * sin(u) * sin(v), (2.0 + sin(7.0 * u + 5.0 * v)) * cos(v));
    }
    if (id == 17) {
        return vec3(sin(u) * sin(v), cos(v) * cos(u), sin(sin(u) + cos(v)));
    }
    if (id == 18) {
        return vec3((2.0 * v * cos(u)), 2.0 * v * (sin(u)) + v * abs(cos(u)), cos(3.0 * v) * sin(3.0 * v));
    }
    if (id == 19) {
        return vec3(v * sin(abs(u)), u * sin(abs(v)), u + abs(sin(v * u)));
    }
    if (id == 20) {
        return vec3(cos(u) * (6.0 - (5.0 / 4.0 + sin(3.0 - v)) * sin(v - 3.0 - u)), (6.0 - (5.0 / 4.0 + sin(3.0 * v)) * sin(v - 3.0 * u)) * sin(u), -cos(v - 3.0 * u) * (5.0 / 4.0 + sin(3.0 * v)));
    }
    if (id == 21) {
        return vec3((4.0 + (sin(4.0 * (v + 2.0 * u)) + 1.25) * cos(v)) * cos(u), (4.0 + (sin(4.0 * (v + 2.0 * u)) + 1.25) * cos(v)) * sin(u), ((sin(4.0 * (v + 2.0 * u)) + 1.25) * sin(v)));
    }
    if (id == 22) {
        return vec3(u, sin(v) * (u * u * u + 2.0 * u * u - 2.0 * u + 2.0) / 5.0, cos(v) * (u * u * u + 2.0 * u * u - 2.0 * u + 2.0) / 5.0);
    }
    if (id == 23) {
        return vec3(-0.8 * u + (2.0 * 0.75 * cosh(0.5 * u) * sinh(0.5 * u)) / (0.5 * ((sqrt(0.75) * parplot2d_wf_sqr(cosh(0.5 * u))) + parplot2d_wf_sqr(0.5 * sin(sqrt(0.75) * v)))), (2.0 * sqrt(0.75) * cosh(0.5 * u) * (-(sqrt(0.75) * cos(v) * cos(sqrt(0.75) * v)) - sin(v) * sin(sqrt(0.75) * v))) / (0.5 * parplot2d_wf_sqr((sqrt(0.75) * cosh(0.5 * u)) + parplot2d_wf_sqr(0.5 * sin(sqrt(0.75) * v)))), (2.0 * sqrt(0.75) * cosh(0.5 * u) * (-(sqrt(0.75) * sin(v) * cos(sqrt(0.75) * v)) + cos(v) * sin(sqrt(0.75) * v))) / (0.5 * parplot2d_wf_sqr((sqrt(0.75) * cosh(0.5 * u)) + parplot2d_wf_sqr(0.5 * sin(sqrt(0.75) * v)))));
    }
    if (id == 24) {
        return vec3(cos(u + 0.0) + 0.06 * sin(1.0 * v), cos(15.0 * u + 0.0) - 0.6 * cos(1.0 * v), sin(12.0 * u + 0.0) + 0.06 * sin(1.0 * v));
    }
    if (id == 25) {
        return vec3((cos(2.0 * u)) / (sqrt(2.0) + sin(2.0 * v)), sin(2.0 * u) / (sqrt(2.0) + sin(2.0 * v)), v / (sqrt(5.0) + cos(2.0 * v)));
    }
    if (id == 26) {
        return vec3(2.0 * sin(3.0 * u) / (2.0 + cos(v)), 2.0 * (sin(u) + 2.0 * sin(2.0 * u)) / (2.0 + cos(v + 2.0 * pi / 3.0)), (cos(u) - 2.0 * cos(2.0 * u)) * (2.0 + cos(v)) * (2.0 + cos(v + 2.0 * pi / 3.0)) / 4.0);
    }
    if (id == 27) {
        return vec3(pow(1.2, u) * (1.0 + cos(v)) * cos(u), pow(1.2, u) * (1.0 + cos(v)) * sin(u), pow(1.2, u) * sin(v) - 1.5 * pow(1.2, u));
    }
    if (id == 28) {
        return vec3(u * cos(u) * (cos(v) + 1.0), u * sin(u) * (cos(v) + 1.0), u * sin(v) - ((u + 3.0) / 8.0 * pi) * u / 3.0);
    }
    if (id == 29) {
        return vec3(cos(u) * cos(v) + 3.0 * cos(u) * (1.5 + sin(u * 5.0 / 3.0) / 2.0), sin(u) * cos(v) + 3.0 * sin(u) * (1.5 + sin(u * 5.0 / 3.0) / 2.0), sin(v) + 2.0 * cos(u * 5.0 / 3.0));
    }
    if (id == 30) {
        return vec3(0.1 * cos(u), -0.1 * sin(u), v + 0.1 * sin(u));
    }
    if (id == 31) {
        return vec3((u / (pi + pi)) * (1.0 - 2.0 * v * v) * cos(u), (u / (pi + pi)) * (1.0 - 2.0 * v * v) * sin(u), v);
    }
    if (id == 32) {
        return vec3((3.0 + 2.0 * cos(v)) * cos(u), (3.0 + 2.0 * cos(v)) * sin(u), u + 2.0 * sin(v));
    }
    if (id == 33) {
        return vec3(u + (1.0 / 10.0) * sin(10.0 * v), ((2.0 * v) / 3.0) * (1.2 - (1.0 / (1.0 + u * u))), sin(pi * v) / (2.0 * pi * v));
    }
    if (id == 34) {
        return vec3((v / 3.0) * cos(u - (pi + pi) / 3.0), (v / 3.0) * sin(u - (pi + pi) / 3.0), u / 10.0 + (v * v) / 2.0);
    }
    if (id == 35) {
        return vec3(u * cos(v), u * sin(v), exp(-u * u) * (sin(param_a * pi * (u)) - u * cos(param_b * v)));
    }
    if (id == 36) {
        return vec3(u * param_a, v * param_b, u * param_c + v * param_d);
    }
    if (id == 37) {
        return vec3(u * param_a, v * param_b, sin(v * param_c) * param_d);
    }
    if (id == 38) {
        return vec3(cos(u * param_a) * sin(v * param_b), sin(u * param_c) * sin(v * param_d), sin(v * param_e));
    }
    if (id == 39) {
        return vec3(cos(u * param_a) * sin(v * param_b), sin(u * param_c) * sin(v * param_d), cos(v * param_e));
    }
    if (id == 40) {
        return vec3(u * cos(v * param_a) - u * param_b, v * cos(u * param_c) - v * param_d, u * v * sin(u * param_e) * sin(v * param_f) - u / v);
    }
    if (id == 41) {
        return vec3(cos(u * param_a) * sin(u * param_b) - u * param_c, sin(u * param_d) * cos(v * param_e) - u * param_f, cos(u) * sin(u));
    }
    if (id == 42) {
        return vec3(cos(u * param_a) * sin(u * param_b) - u * param_c, (v / param_d) * cos(u * param_e) - v * param_f, cos(u) * sin(u));
    }
    if (id == 43) {
        return vec3(cos(v * param_a) + sin(v * param_b) - u * param_c, (v / param_d) * cos(u * param_e) - v * param_f, cos(u) - sin(u));
    }
    if (id == 44) {
        return vec3(cos(u * param_a) * sin(v * param_b) - u - v, sin(v * param_c) * cos(v * param_d) - u * param_e, cos(u * param_f) * sin(u * param_f));
    }
    if (id == 45) {
        return vec3(cos(u) * sin(u) - v, (v / 3.0) * cos(u) - v, cos(u) * sin(u));
    }
    if (id == 46) {
        return vec3(cos(u) * sin(v) + (u * v), sin(v) * cos(v) * (u + v), cos(u) * sin(u));
    }
    vec3(0.0, 0.0, 0.0)
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let solid = to_int(to_float(p.solid));
    let pa = p.param_a;
    let pb = p.param_b;
    let pc = p.param_c;
    let pd = p.param_d;
    let pe = p.param_e;
    let pf = p.param_f;
    let umin_s = p.umin_s;
    let du = p.du;
    let vmin_s = p.vmin_s;
    let dv = p.dv;
    let rand_u = 0.0;
    let rand_v = 0.0;
    if (solid == 0) {
        rand_u = z.re;
        rand_v = z.im;
    } else {
        rand_u = rand();
        rand_v = rand();
    }
    let u = umin_s + rand_u * du;
    let v = vmin_s + rand_v * dv;
    let xyz = parplot2d_wf_formula(preset_id, u, v, pa, pb, pc, pd, pe, pf);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 1) {
            c = (u - umin_s) / du;
        } else if (color_mode == 2) {
            c = (v - vmin_s) / dv;
        } else if (color_mode != 0) {
            c = (v - vmin_s) / dv * (u - umin_s) / du;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: complex(xyz.x, xyz.y), color: color }
}

fn variation3(q, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let solid = to_int(to_float(p.solid));
    let pa = p.param_a;
    let pb = p.param_b;
    let pc = p.param_c;
    let pd = p.param_d;
    let pe = p.param_e;
    let pf = p.param_f;
    let umin_s = p.umin_s;
    let du = p.du;
    let vmin_s = p.vmin_s;
    let dv = p.dv;
    let rand_u = 0.0;
    let rand_v = 0.0;
    if (solid == 0) {
        rand_u = q.x;
        rand_v = q.y;
    } else {
        rand_u = rand();
        rand_v = rand();
    }
    let u = umin_s + rand_u * du;
    let v = vmin_s + rand_v * dv;
    let xyz = parplot2d_wf_formula(preset_id, u, v, pa, pb, pc, pd, pe, pf);
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 1) {
            c = (u - umin_s) / du;
        } else if (color_mode == 2) {
            c = (v - vmin_s) / dv;
        } else if (color_mode != 0) {
            c = (v - vmin_s) / dv * (u - umin_s) / du;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: xyz, color: color }
}
