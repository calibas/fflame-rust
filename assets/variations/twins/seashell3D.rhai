// Seashell 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    final_radius: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Final Radius",
        hint: "Final-spiral radius (outer-edge radius coefficient).",
    },
    height: #{
        value: 3.5, min: -20.0, max: 20.0, free: true,
        caption: "Height",
        hint: "Height ramp coefficient on Z.",
    },
    inner_radius: #{
        value: 0.4, min: -10.0, max: 10.0, free: true,
        caption: "Inner Radius",
        hint: "Inner-spiral radial offset added to X and Y.",
    },
    nSpirals: #{
        value: 3, min: 1, max: 50,
        caption: "N Spirals",
        hint: "Number of spiral turns.",
    },
};

const INFO = #{ title: "Seashell 3D", name: "rhai_seashell3d" };

fn variation(z, p) {
    let a = p.final_radius;
    let c = p.inner_radius;
    let n = to_float(p.nSpirals);
    let two_pi = 6.28318530717959;
    let inv_2pi = 0.15915494309189535;
    let t = two_pi * rand();
    let s = two_pi * rand();
    let one_minus_t = 1.0 - t * inv_2pi;
    let cos_s = cos(s);
    let cos_nt = cos(n * t);
    let sin_nt = sin(n * t);
    let x = a * one_minus_t * cos_nt * (1.0 + cos_s) + c * cos_nt;
    let y = a * one_minus_t * sin_nt * (1.0 + cos_s) + c * sin_nt;
    complex(x, y)
}

fn variation3(v, p) {
    let a = p.final_radius;
    let b = p.height;
    let c = p.inner_radius;
    let n = to_float(p.nSpirals);
    let two_pi = 6.28318530717959;
    let inv_2pi = 0.15915494309189535;
    let t = two_pi * rand();
    let s = two_pi * rand();
    let one_minus_t = 1.0 - t * inv_2pi;
    let cos_s = cos(s);
    let cos_nt = cos(n * t);
    let sin_nt = sin(n * t);
    let x = a * one_minus_t * cos_nt * (1.0 + cos_s) + c * cos_nt;
    let y = a * one_minus_t * sin_nt * (1.0 + cos_s) + c * sin_nt;
    let z = b * t * inv_2pi + a * one_minus_t * sin(s);
    vec3(x, y, z)
}
