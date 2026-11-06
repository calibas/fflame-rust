// R Circle Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 1.0, min: 0.1, max: 50.0, free: true,
        caption: "N",
        hint: "Band width — radius is truncated modulo this value.",
    },
    seed: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Seed",
        hint: "Random seed for the per-cell hash — change to vary the pattern.",
    },
    dist: #{
        value: 0.5, min: 0.0, max: 5.0, free: true,
        caption: "Dist",
        hint: "Per-cell jitter strength.",
    },
    min: #{
        value: 0.1, min: 0.0, max: 1.0,
        caption: "Min",
        hint: "Minimum cell-disc radius (fraction of cell).",
    },
    max: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "Max",
        hint: "Maximum cell-disc radius (fraction of cell).",
    },
};

const INFO = #{ title: "R Circle Blur", name: "rhai_r_circleblur" };

fn variation(z, p) {
    let n = p.n;
    let seed = p.seed;
    let dist = p.dist;
    let min_v = p.min;
    let max_v = p.max;
    let rcn = max(abs(n), 1.0e-6);
    let angle = atan2(z.im, z.re);
    let rad = sqrt(z.re * z.re + z.im * z.im);
    rad = rad - to_float(floor(rad / rcn)) * rcn;
    let by = sin(angle + rad);
    let bx = cos(angle + rad);
    by = round_even(by * rad);
    bx = round_even(bx * rad);
    let rad2 = sqrt(rand()) * 0.5;
    let angle2 = rand() * 6.28318530717959;
    let a1 = sin(bx * 127.1 + by * 311.7 + seed) * 43758.5453;
    a1 = a1 - to_float(trunc(a1));
    let a2 = sin(bx * 269.5 + by * 183.3 + seed) * 43758.5453;
    a2 = a2 - to_float(trunc(a2));
    let a3 = sin(by * 12.9898 + bx * 78.233 + seed) * 43758.5453;
    a3 = a3 - to_float(trunc(a3));
    a3 = a3 * (max_v - min_v) + min_v;
    let rad3 = rad2 * a3;
    by = by + rad3 * sin(angle2) + a1 * dist;
    bx = bx + rad3 * cos(angle2) + a2 * dist;
    complex(bx, by)
}
