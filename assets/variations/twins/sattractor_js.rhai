// S-Attractor (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    m: #{
        value: 10, min: 1, max: 12,
        caption: "M",
        hint: "Number of vertex angles (1-12). Each iteration samples one of `m` evenly-spaced angles around the unit circle.",
    },
};

const INFO = #{ title: "S-Attractor (JS)", name: "rhai_sattractor_js" };

fn variation(z, p) {
    let m = max(to_int(to_float(p.m)), 1);
    let mf = to_float(m);
    let two_pi = 6.28318530717959;
    let l = to_int(rand() * mf) + 1;
    let l_clamped = clamp(l, 1, m);
    let theta = two_pi * to_float(l_clamped) / mf;
    let a = cos(theta);
    let b = sin(theta);
    let x = 0.0;
    let y = 0.0;
    if (rand() < 0.5) {
        x = z.re * 0.5 + a;
        y = z.im * 0.5 + b;
    } else {
        x = z.re * a + z.im * b + z.re * z.re * b;
        y = z.im * a - z.re * b + z.re * z.re * a;
    }
    complex(x * 0.5, y * 0.5)
}
