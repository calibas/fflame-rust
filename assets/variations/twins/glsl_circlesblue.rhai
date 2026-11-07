/// Animated bubble field. Renders `bubles` circular bubbles with
/// per-bubble pseudo-random position, size, and color, all driven by
/// `time`. Bubbles fall from the top of the unit square with vertical
/// scrolling.
///
/// 6 params: standard 4 (sans gradient — `seed` and `time` are
/// included) + `radius` (bubble base radius) + `bubles` (count).
/// 
/// # Authors
/// - Jesus Sosa

// GLSL CirclesBlue, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    density_pixels: #{
        value: 1000000, min: 100, max: 10000000, free: true,
        caption: "Density Pixels",
        hint: "Discrete grid resolution.",
    },
    seed: #{
        value: 10000, min: 0, max: 10000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only — set `time` directly.",
    },
    time: #{
        value: 1.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Time",
        hint: "Animation parameter. Drives the vertical scrolling (`time/5 · 0.1` per step) and the per-bubble color/size modulation.",
    },
    radius: #{
        value: 0.04, min: 0.0, max: 1.0, free: true,
        caption: "Radius",
        hint: "Base bubble radius — each bubble's actual radius adds `sin(i)·0.12 + 0.08` on top. JWildfire's UI label is 'Radiusy' (typo in source kept for round-trip).",
    },
    bubles: #{
        value: 40, min: 1, max: 200, free: true,
        caption: "Bubles",
        hint: "Bubble count. **GPU-clamped to ≤ 200.** JWildfire's UI label has the typo 'Bubles'; kept for round-trip.",
    },
    gradient: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Gradient",
        hint: "JWildfire color output mode. **Only mode 0 honored.**",
    },
};

const INFO = #{ title: "GLSL CirclesBlue", name: "rhai_glsl_circlesblue", category: "plugin", phase: "any", slow: ["bubles"], z: "never" };

fn variation(z, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let f_radius = p.radius;
    let bubles = min(u32(to_float(p.bubles)), u32(200));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);
    let color = vec3(0.0, 0.0, 0.0);
    let col_a = vec3(0.1, 0.2, 0.8);
    let col_b = vec3(0.2, 0.8, 0.6);
    for i in 0..to_int(bubles) {
        let i_fl = to_float(i);
        let pha = tan(i_fl * 6.0 + 1.0) * 0.5 + 0.5;
        let siz = pow(cos(i_fl * 2.4 + 5.0) * 0.5 + 0.5, 4.0);
        let pox = cos(i_fl * 3.55 + 4.1);
        let rad = f_radius + sin(i_fl) * 0.12 + 0.08;
        let scroll = pha + 0.1 * (time / 5.0) * (0.2 + 0.8 * siz);
        let scroll_mod = scroll - to_float(floor(scroll));
        let pos = complex(pox + sin(time / 15.0 + pha + siz), -1.0 - rad + (2.0 + 2.0 * rad) * scroll_mod);
        let dis = (uv - pos).abs();
        let t2 = 0.5 + 0.5 * sin(i_fl * sin(time * pox * 0.03) + 1.9);
        let t1 = vec3(t2, t2, t2);
        let col = vec3(mix(col_a.x, col_b.x, t1.x), mix(col_a.y, col_b.y, t1.y), mix(col_a.z, col_b.z, t1.z));
        let t3 = rad * (0.65 + 0.20 * sin(pox * time));
        let t4 = clamp((dis - t3) / (rad - t3), 0.0, 1.0);
        color = color + col * (1.0 - (t4 * t4 * (3.0 - 2.0 * t4))) * (1.0 - cos(pox * time));
    }
    rgb = color * 0.3;
    #{ z: complex(i_f / resolution - 0.5, j_f / resolution - 0.5), rgb: rgb }
}

fn variation3(v, p, ctx) {
    let rgb = ctx.rgb;
    let resolution = max(100.0, to_float(p.density_pixels));
    let time = p.time;
    let f_radius = p.radius;
    let bubles = min(u32(to_float(p.bubles)), u32(200));
    let i_f = to_float(floor(rand() * resolution));
    let j_f = to_float(floor(rand() * resolution));
    let uv = complex(2.0 * (i_f + 0.5) / resolution - 1.0, 2.0 * (j_f + 0.5) / resolution - 1.0);
    let color = vec3(0.0, 0.0, 0.0);
    let col_a = vec3(0.1, 0.2, 0.8);
    let col_b = vec3(0.2, 0.8, 0.6);
    for i in 0..to_int(bubles) {
        let i_fl = to_float(i);
        let pha = tan(i_fl * 6.0 + 1.0) * 0.5 + 0.5;
        let siz = pow(cos(i_fl * 2.4 + 5.0) * 0.5 + 0.5, 4.0);
        let pox = cos(i_fl * 3.55 + 4.1);
        let rad = f_radius + sin(i_fl) * 0.12 + 0.08;
        let scroll = pha + 0.1 * (time / 5.0) * (0.2 + 0.8 * siz);
        let scroll_mod = scroll - to_float(floor(scroll));
        let pos = complex(pox + sin(time / 15.0 + pha + siz), -1.0 - rad + (2.0 + 2.0 * rad) * scroll_mod);
        let dis = (uv - pos).abs();
        let t2 = 0.5 + 0.5 * sin(i_fl * sin(time * pox * 0.03) + 1.9);
        let t1 = vec3(t2, t2, t2);
        let col = vec3(mix(col_a.x, col_b.x, t1.x), mix(col_a.y, col_b.y, t1.y), mix(col_a.z, col_b.z, t1.z));
        let t3 = rad * (0.65 + 0.20 * sin(pox * time));
        let t4 = clamp((dis - t3) / (rad - t3), 0.0, 1.0);
        color = color + col * (1.0 - (t4 * t4 * (3.0 - 2.0 * t4))) * (1.0 - cos(pox * time));
    }
    rgb = color * 0.3;
    #{ z: vec3(i_f / resolution - 0.5, j_f / resolution - 0.5, v.z), rgb: rgb }
}
