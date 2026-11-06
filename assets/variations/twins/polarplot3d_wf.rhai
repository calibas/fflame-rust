/// 3D polar surface plot — samples random angles `(t, u)`, evaluates a
/// preset radius formula `r = f(t, u)`, and emits the point in spherical
/// (default) or cylindrical coordinates. The 10 presets are JWildfire's
/// stock polar surfaces (all take `param_a..c`). Custom JWF formulas are
/// not supported.
///
/// # Authors
/// - Andreas Maschke

// Polar-Plot 3D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    preset_id: #{
        value: 0, min: -1, max: 9,
        caption: "Preset ID",
        hint: "Which baked formula preset to plot (0-9). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as r = 0.",
    },
    tmin: #{
        value: -3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "T Min",
        hint: "Lower bound of the azimuthal parameter t (swapped with tmax if larger). JWF default -π.",
    },
    tmax: #{
        value: 3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "T Max",
        hint: "Upper bound of the azimuthal parameter t. JWF default π.",
    },
    umin: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "U Min",
        hint: "Lower bound of the polar parameter u.",
    },
    umax: #{
        value: 3.1415927, min: -100.0, max: 100.0, free: true,
        caption: "U Max",
        hint: "Upper bound of the polar parameter u. JWF default π.",
    },
    rmin: #{
        value: -2.0, min: -10.0, max: 10.0, free: true,
        caption: "R Min",
        hint: "Lower R bound — only used to normalize the R color mode (the radius itself is unclamped).",
    },
    rmax: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "R Max",
        hint: "Upper R bound for the R color mode.",
    },
    cylindrical: #{
        value: false,
        caption: "Cylindrical",
        hint: "When off (JWF default), spherical mapping: `(r·sin u·cos t, r·sin u·sin t, r·cos u)`. When on, cylindrical: `(r·cos t, r·sin t, u)`.",
    },
    direct_color: #{
        value: true,
        caption: "Direct Color",
        hint: "When on, writes the color register from the sample per Color Mode. Visible color requires the transform's Direct Color slider > 0.",
    },
    color_mode: #{
        value: 2, choices: ["Colormap (unsupported)", "T", "U", "R", "T×U"],
        caption: "Color Mode",
        hint: "Sample→palette mapping. T/U/R normalize the corresponding value over its range; T×U multiplies the two angles. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded.",
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
        hint: "Formula input `param_a` — used by all presets (JWF applies each preset's own default when selected; set it manually here).",
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

const INFO = #{ title: "Polar-Plot 3D WF", name: "rhai_polarplot3d_wf", category: "full_3d", phase: "any" };

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
    let umin = p.umin;
    let umax = p.umax;
    if (umin > umax) {
        let t = umin;
        umin = umax;
        umax = t;
    }
    out[2] = umin;
    out[3] = umax - umin;
    let rmin = p.rmin;
    let rmax = p.rmax;
    if (rmin > rmax) {
        let t = rmin;
        rmin = rmax;
        rmax = t;
    }
    out[4] = rmin;
    out[5] = rmax - rmin;
    #{ tmin_s: out[0], dt: out[1], umin_s: out[2], du: out[3], rmin_s: out[4], dr: out[5] }
}

fn polarplot3d_wf_sqr(v) {
    v * v
}

fn polarplot3d_wf_formula(id, t, u, param_a, param_b, param_c, param_d, param_e, param_f) {
    let pi = 3.14159265358979;
    if (id == 0) {
        return sin(param_a * u) + param_b;
    }
    if (id == 1) {
        return sin(param_a * u) + param_b;
    }
    if (id == 2) {
        return u;
    }
    if (id == 3) {
        return (sin(param_a * t) + cos(param_b * u)) + param_c;
    }
    if (id == 4) {
        return (sin(param_a * t + param_b * u)) + param_c;
    }
    if (id == 5) {
        return (sin(param_a * t + param_b * u)) + param_c;
    }
    if (id == 6) {
        return sin(param_a * t) + param_b * u;
    }
    if (id == 7) {
        return t / (param_a + u) + param_b;
    }
    if (id == 8) {
        return polarplot3d_wf_sqr(u) + param_a;
    }
    if (id == 9) {
        return cos(param_a * t + sin(param_b * u)) + param_c;
    }
    0.0
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let cylindrical = to_int(to_float(p.cylindrical));
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
    let umin_s = p.umin_s;
    let du = p.du;
    let rmin_s = p.rmin_s;
    let dr = p.dr;
    let rand_t = rand();
    let rand_u = rand();
    let t = tmin_s + rand_t * dt;
    let u = umin_s + rand_u * du;
    let r = polarplot3d_wf_formula(preset_id, t, u, pa, pb, pc, pd, pe, pf);
    let x = 0.0;
    let y = 0.0;
    if (cylindrical == 0) {
        x = r * sin(u) * cos(t);
        y = r * sin(u) * sin(t);
    } else {
        x = r * cos(t);
        y = r * sin(t);
    }
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 1) {
            c = (t - tmin_s) / dt;
        } else if (color_mode == 3) {
            c = (r - rmin_s) / dr;
        } else if (color_mode == 4) {
            c = (t - tmin_s) / dt * (u - umin_s) / du;
        } else if (color_mode != 0) {
            c = (u - umin_s) / du;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: complex(x, y), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let cylindrical = to_int(to_float(p.cylindrical));
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
    let umin_s = p.umin_s;
    let du = p.du;
    let rmin_s = p.rmin_s;
    let dr = p.dr;
    let rand_t = rand();
    let rand_u = rand();
    let t = tmin_s + rand_t * dt;
    let u = umin_s + rand_u * du;
    let r = polarplot3d_wf_formula(preset_id, t, u, pa, pb, pc, pd, pe, pf);
    let x = 0.0;
    let y = 0.0;
    let z = 0.0;
    if (cylindrical == 0) {
        x = r * sin(u) * cos(t);
        y = r * sin(u) * sin(t);
        z = r * cos(u);
    } else {
        x = r * cos(t);
        y = r * sin(t);
        z = u;
    }
    if (direct_color > 0) {
        let c = color;
        if (color_mode == 1) {
            c = (t - tmin_s) / dt;
        } else if (color_mode == 3) {
            c = (r - rmin_s) / dr;
        } else if (color_mode == 4) {
            c = (t - tmin_s) / dt * (u - umin_s) / du;
        } else if (color_mode != 0) {
            c = (u - umin_s) / du;
        }
        color = clamp(c, 0.0, 1.0);
    }
    #{ z: vec3(x, y, z), color: color }
}
