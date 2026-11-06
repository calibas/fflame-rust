/// 3D kaleidoscopic-fold (Kaliset) stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Snowflake, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1000, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire; ignored by the renderer.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Animates the Z component of the fold seed.",
    },
    level: #{
        value: 10, min: 1, max: 15,
        caption: "Level",
        hint: "Number of Kaliset fold iterations.",
    },
    zoom: #{
        value: 10.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Snowflake", name: "rhai_cut_snowflake", category: "plugin", phase: "any", z: "preserve_z", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
    let level = to_int(to_float(p.level));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let uv = complex(x * zoom, y * zoom);
    let s = 0.25;
    let q = vec3(uv.re, uv.im, sin(time * 0.4) * 0.5 - 0.5) * s;
    let k = 0.0;
    for i in 0..level {
        let t1 = abs(q) / dot(q, q);
        q = vec3(t1.x - 1.65, t1.y - 1.65, t1.z - 1.65);
        k = length(q);
        let t2 = q * k;
        q = vec3(t2.x + k, t2.y + k, t2.z + k);
    }
    let color = dot(q, q) * s;
    let hidden = false;
    if (invert == 0) {
        if (color > 0.5) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.5) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}

fn variation3(v, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
    let level = to_int(to_float(p.level));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = v.x;
        y = v.y;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let uv = complex(x * zoom, y * zoom);
    let s = 0.25;
    let q = vec3(uv.re, uv.im, sin(time * 0.4) * 0.5 - 0.5) * s;
    let k = 0.0;
    for i in 0..level {
        let t1 = abs(q) / dot(q, q);
        q = vec3(t1.x - 1.65, t1.y - 1.65, t1.z - 1.65);
        k = length(q);
        let t2 = q * k;
        q = vec3(t2.x + k, t2.y + k, t2.z + k);
    }
    let color = dot(q, q) * s;
    let hidden = false;
    if (invert == 0) {
        if (color > 0.5) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.5) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: vec3(x, y, v.z), hide: hide_v }
}
