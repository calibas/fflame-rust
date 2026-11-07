/// Symmetric Icon Attractor — implements one of 17 baked-in presets from
/// Field & Golubitsky's *Symmetry in Chaos*. Each preset is a `(degree, a,
/// b, g, o, l)` tuple selected by `preset_id`. The body computes the
/// standard symmetric-icon iteration: `p = a·|z|² + l + b·Re(z^degree)`,
/// then `out = p·z + g·Re(z^(degree-1)) − o·rot90(z)`. The complex-power
/// iteration is bounded at 24 to keep WGSL happy.
///
/// # Authors
/// - Jesus Sosa

// Icon Attractor (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    preset_id: #{
        value: 0, min: 0, max: 16,
        caption: "Preset ID",
        hint: "Which Field & Golubitsky preset to use (0-16). Each corresponds to a different symmetric icon attractor shape from the original book.",
    },
    centerx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Center X",
        hint: "Color register X center — offsets the squared-distance color formula. Visible color requires the transform's Direct Color slider > 0.",
    },
    centery: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Center Y",
        hint: "Color register Y center — offsets the squared-distance color formula.",
    },
    scale: #{
        value: 5.0, min: -100.0, max: 100.0, free: true,
        caption: "Scale",
        hint: "Color register scale — the color formula multiplies the squared distance by `1/scale` (precomputed at init as `_bdcs`).",
    },
};

const INFO = #{ title: "Icon Attractor (JS)", name: "rhai_iconattractor_js", category: "advanced_2d", phase: "any", replace: true };

fn prepare(p) {
    let scale = p.scale;
    let safe_scale = (if abs(scale) < 1.0e-30 { 1.0e-5 } else { scale });
    #{ bdcs: 1.0 / safe_scale }
}

fn ic_preset(id) {
    let out = array(6, 0.0);
    let i = clamp(id, 0, 16);
    let deg = 5.0;
    let a = 5.0;
    let b = -1.9;
    let g = 1.0;
    let o = 0.188;
    let l = -2.5;
    if (i == 1) {
        deg = 3.0;
        a = -1.0;
        b = 0.1;
        g = -0.82;
        o = 0.12;
        l = 1.56;
    } else if (i == 2) {
        deg = 5.0;
        a = 1.806;
        b = 0.0;
        g = 1.0;
        o = 0.0;
        l = -1.806;
    } else if (i == 3) {
        deg = 3.0;
        a = 10.0;
        b = -12.0;
        g = 1.0;
        o = 0.0;
        l = -2.195;
    } else if (i == 4) {
        deg = 3.0;
        a = -2.5;
        b = 0.0;
        g = 0.9;
        o = 0.0;
        l = 2.5;
    } else if (i == 5) {
        deg = 9.0;
        a = 3.0;
        b = -16.79;
        g = 1.0;
        o = 0.0;
        l = -2.05;
    } else if (i == 6) {
        deg = 6.0;
        a = 5.0;
        b = 1.5;
        g = 1.0;
        o = 0.0;
        l = -2.7;
    } else if (i == 7) {
        deg = 23.0;
        a = -2.5;
        b = 0.0;
        g = 0.9;
        o = 0.0;
        l = 2.409;
    } else if (i == 8) {
        deg = 7.0;
        a = 1.0;
        b = -0.1;
        g = 0.167;
        o = 0.0;
        l = -2.08;
    } else if (i == 9) {
        deg = 5.0;
        a = 2.32;
        b = 0.0;
        g = 0.75;
        o = 0.0;
        l = -2.32;
    } else if (i == 10) {
        deg = 5.0;
        a = -2.0;
        b = 0.0;
        g = -0.5;
        o = 0.0;
        l = 2.6;
    } else if (i == 11) {
        deg = 5.0;
        a = 2.0;
        b = 0.2;
        g = 0.1;
        o = 0.0;
        l = -2.34;
    } else if (i == 12) {
        deg = 4.0;
        a = 2.0;
        b = 0.0;
        g = 1.0;
        o = 0.1;
        l = -1.86;
    } else if (i == 13) {
        deg = 3.0;
        a = -1.0;
        b = 0.1;
        g = -0.82;
        o = 0.0;
        l = 1.56;
    } else if (i == 14) {
        deg = 3.0;
        a = -1.0;
        b = 0.1;
        g = -0.805;
        o = 0.0;
        l = 1.5;
    } else if (i == 15) {
        deg = 3.0;
        a = -1.0;
        b = 0.03;
        g = -0.80;
        o = 0.0;
        l = 1.455;
    } else if (i == 16) {
        deg = 16.0;
        a = -2.5;
        b = -0.1;
        g = 0.90;
        o = -0.15;
        l = 2.39;
    }
    out[0] = deg;
    out[1] = a;
    out[2] = b;
    out[3] = g;
    out[4] = o;
    out[5] = l;
    out
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let preset_id = to_int(to_float(p.preset_id));
    let preset = ic_preset(preset_id);
    let degree = to_int(preset[0]);
    let a = preset[1];
    let b = preset[2];
    let g = preset[3];
    let o = preset[4];
    let l = preset[5];
    let zzbar = z.re * z.re + z.im * z.im;
    let pp = a * zzbar + l;
    let zreal = z.re;
    let zimag = z.im;
    let max_loop = max(degree - 2, 0);
    for i in 0..24 {
        if (i >= max_loop) {
            break;
        }
        let za = zreal * z.re - zimag * z.im;
        let zb = zimag * z.re + zreal * z.im;
        zreal = za;
        zimag = zb;
    }
    let zn = z.re * zreal - z.im * zimag;
    pp = pp + b * zn;
    let nx = pp * z.re + g * zreal - o * z.im;
    let ny = pp * z.im - g * zimag + o * z.re;
    let centerx = p.centerx;
    let centery = p.centery;
    let bdcs = p.bdcs;
    let w = ctx.weight;
    let fpx = w * nx + centerx;
    let fpy = w * ny + centery;
    let craw = abs(bdcs * (fpx * fpx + fpy * fpy));
    color = craw - to_float(floor(craw));
    #{ z: complex(nx, ny), color: color }
}
