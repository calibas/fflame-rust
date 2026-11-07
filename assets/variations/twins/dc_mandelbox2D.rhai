/// 2D Mandelbox escape colouring — runs a sampled point through a 20-iter
/// Mandelbox at `zoom` and writes the escape-derived RGB to the colour
/// register. `ColorOnly` samples the input point instead of a random one;
/// the `DC_BaseFunc` `scale_z`/`offset_z`/`reset_z` drive an optional Z
/// output.
///
/// # Authors
/// - Jesus Sosa

// DC MandelBox2D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    zoom: #{
        value: 7.0, min: 0.1, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Scale applied to the sample before the Mandelbox iteration. Higher zoom shows finer escape structure.",
    },
    seed: #{
        value: 10000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only: seeds a Java Random and assigns `time` as a side effect. Not honored here — set `time` directly. Accepted for round-trip.",
    },
    time: #{
        value: 0.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter, embedded as `O.z = -5·cos(time·0.1)` in the iteration. Animate for a morphing pattern.",
    },
    ColorOnly: #{
        value: false,
        caption: "Color Only",
        hint: "When on, samples the input point (colours it by the Mandelbox at that position) instead of scattering a random sample.",
    },
    Gradient: #{
        value: 0, choices: ["Direct RGB", "Nearest Palette", "Greyscale Index"],
        caption: "Gradient",
        hint: "Colour output mode. Only mode 0 (direct RGB) is honored; modes 1/2 fall back to RGB (framework limitation — see module doc). Round-trips regardless.",
    },
    scale_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Z",
        hint: "Z = greyscale(rgb)·scale_z + offset_z. 0 (default) gives a flat z = offset_z.",
    },
    offset_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset Z",
        hint: "Constant added to the Z output.",
    },
    reset_z: #{
        value: true,
        caption: "Reset Z",
        hint: "When on (default), the variation's Z replaces the running z with `dz`; when off, it adds `dz`.",
    },
};

const INFO = #{ title: "DC MandelBox2D", name: "rhai_dc_mandelbox2d", category: "plugin", phase: "any" };

fn dc_mbox2d_rgb(i_in, time) {
    let I = i_in;
    let O = vec4(I.re, I.im, -5.0 * cos(time * 0.1), 1.0);
    let d = 1.0;
    for k in 0..20 {
        let t2 = -1.0;
        let t1 = complex(t2, t2);
        let t3 = complex(1.0, 1.0);
        I = complex(clamp(I.re, t1.re, t3.re), clamp(I.im, t1.im, t3.im)) * 2.0 - I;
        O.w = I.abs();
        let b = 0.0;
        if (O.w < 0.5) {
            b = 4.0;
        } else if (O.w < 1.0) {
            b = 1.0 / O.w;
        } else {
            b = 1.0;
        }
        I = I * (O.z * b) + complex(O.x, O.y);
        d = b * d * abs(O.z) + 1.0;
    }
    let df = pow(I.abs() / d, 0.1) * 5.0;
    let t4 = vec3(cos(df), sin(10.0 * df + 1.0), cos(3.0 * df + 1.0)) * 0.5;
    vec3(t4.x + 0.5, t4.y + 0.5, t4.z + 0.5)
}

fn dc_mbox2d_grey(color) {
    let ri = clamp(to_float(floor(color.r * 256.0)), 0.0, 255.0);
    let gi = clamp(to_float(floor(color.g * 256.0)), 0.0, 255.0);
    let bi = clamp(to_float(floor(color.b * 256.0)), 0.0, 255.0);
    (to_float(floor(ri * 0.299)) + to_float(floor(gi * 0.587)) + to_float(floor(bi * 0.114))) / 255.0
}

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let zoom = p.zoom;
    let time = p.time;
    let color_only = to_int(to_float(p.ColorOnly));
    let uv = complex(0.0, 0.0);
    if (color_only == 1) {
        uv = z;
    } else {
        uv = complex(rand() - 0.5, rand() - 0.5);
    }
    rgb = dc_mbox2d_rgb(uv * zoom, time);
    #{ z: uv, rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let zoom = p.zoom;
    let time = p.time;
    let color_only = to_int(to_float(p.ColorOnly));
    let scale_z = p.scale_z;
    let offset_z = p.offset_z;
    let uv = complex(0.0, 0.0);
    if (color_only == 1) {
        uv = complex(v.x, v.y);
    } else {
        uv = complex(rand() - 0.5, rand() - 0.5);
    }
    let color = dc_mbox2d_rgb(uv * zoom, time);
    rgb = color;
    let z = dc_mbox2d_grey(color);
    let dz = z * scale_z + offset_z;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    #{ z: vec3(uv.re, uv.im, dz * inv_w), rgb: rgb }
}
