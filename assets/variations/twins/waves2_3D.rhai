/// 3D version of Waves2 — adds `scale · sin(freq · avg(x, y))` to the Z
/// coordinate alongside the standard 2D Waves2 X/Y displacement.
/// 
/// # Authors
/// - Larry Berlin

// Waves2 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    freq: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Freq",
        hint: "Wave frequency on all three axes.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Wave amplitude — how strongly points get displaced.",
    },
};

const INFO = #{ title: "Waves2 3D", name: "rhai_waves2_3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let freq = p.freq;
    let scale = p.scale;
    complex(z.re + scale * sin(z.im * freq), z.im + scale * sin(z.re * freq))
}

fn variation3(v, p) {
    let freq = p.freq;
    let scale = p.scale;
    let avgxy = (v.x + v.y) / 2.0;
    vec3(v.x + scale * sin(v.y * freq), v.y + scale * sin(v.x * freq), v.z + scale * sin(avgxy * freq))
}
