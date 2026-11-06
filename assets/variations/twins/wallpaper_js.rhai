// Wallpaper (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.156, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Wallpaper parameter `a` — Y-output offset (`y' = a − x`).",
    },
    b: #{
        value: -0.28, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Wallpaper parameter `b` — coefficient on x inside the sqrt term.",
    },
    c: #{
        value: 21.288, min: -100.0, max: 100.0, free: true,
        caption: "C",
        hint: "Wallpaper parameter `c` — constant subtracted inside the sqrt term.",
    },
};

const INFO = #{ title: "Wallpaper (JS)", name: "rhai_wallpaper_js" };

fn wallpaper_signum(v) {
    if (v < 0.0) {
        return -1.0;
    }
    if (v > 0.0) {
        return 1.0;
    }
    0.0
}

fn variation(z, p, ctx) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    if (rand() < 0.5) {
        let trunc_x = to_float(trunc(z.re));
        let s = wallpaper_signum(trunc_x);
        let nx = z.im - s * sqrt(abs(b * z.re - c));
        let ny = a - z.re;
        return complex(nx * inv_w, ny * inv_w);
    }
    complex(z.re * inv_w, z.im * inv_w)
}
