/// Linear shred grid — splits each axis into tiles of size `distance`, then
/// within each tile compresses the position by `width` and shifts by `(0.5
/// − sign_offset) · (1 − width)`. Produces a discontinuous shred-like grid
/// pattern.
///
/// # Authors
/// - Zy0rg

// Shredlin, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xdistance: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X distance",
        hint: "X-axis tile size.",
    },
    xwidth: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "X width",
        hint: "X-axis intra-tile compression. 1 = no shred; 0 = collapse to tile center.",
    },
    ydistance: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y distance",
        hint: "Y-axis tile size.",
    },
    ywidth: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y width",
        hint: "Y-axis intra-tile compression.",
    },
};

const INFO = #{ title: "Shredlin", name: "rhai_shredlin", category: "advanced_2d", phase: "any", z: "preserve_z", replace: true };

fn variation(z, p) {
    let sxd = p.xdistance;
    let sxw = p.xwidth;
    let syd = p.ydistance;
    let syw = p.ywidth;
    let safe_sxd = (if abs(sxd) < 1.0e-30 { 1.0e-30 } else { sxd });
    let safe_syd = (if abs(syd) < 1.0e-30 { 1.0e-30 } else { syd });
    let xpos = (if z.re < 0.0 { 1.0 } else { 0.0 });
    let ypos = (if z.im < 0.0 { 1.0 } else { 0.0 });
    let xrng = z.re / safe_sxd;
    let yrng = z.im / safe_syd;
    let xrng_f = to_float(trunc(xrng));
    let yrng_f = to_float(trunc(yrng));
    complex(sxd * ((xrng - xrng_f) * sxw + xrng_f + (0.5 - xpos) * (1.0 - sxw)), syd * ((yrng - yrng_f) * syw + yrng_f + (0.5 - ypos) * (1.0 - syw)))
}

fn variation3(v, p) {
    let sxd = p.xdistance;
    let sxw = p.xwidth;
    let syd = p.ydistance;
    let syw = p.ywidth;
    let safe_sxd = (if abs(sxd) < 1.0e-30 { 1.0e-30 } else { sxd });
    let safe_syd = (if abs(syd) < 1.0e-30 { 1.0e-30 } else { syd });
    let xpos = (if v.x < 0.0 { 1.0 } else { 0.0 });
    let ypos = (if v.y < 0.0 { 1.0 } else { 0.0 });
    let xrng = v.x / safe_sxd;
    let yrng = v.y / safe_syd;
    let xrng_f = to_float(trunc(xrng));
    let yrng_f = to_float(trunc(yrng));
    vec3(sxd * ((xrng - xrng_f) * sxw + xrng_f + (0.5 - xpos) * (1.0 - sxw)), syd * ((yrng - yrng_f) * syw + yrng_f + (0.5 - ypos) * (1.0 - syw)), v.z)
}
