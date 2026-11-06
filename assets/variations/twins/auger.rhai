// Auger, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    freq: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Frequency",
        hint: "Ripple frequency. How many waves go across the surface.",
    },
    weight: #{
        value: 0.5, min: -2.0, max: 2.0, free: true,
        caption: "Weight",
        hint: "How strongly the waves displace points. 0 = no effect.",
    },
    scale: #{
        value: 0.5, min: -2.0, max: 2.0, free: true,
        caption: "Scale",
        hint: "Cross-coupling between X and Y waves. Tunes the diagonal texture.",
    },
    sym: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Symmetry",
        hint: "Blend back toward the input. 0 = full displacement, 1 = no displacement.",
    },
};

const INFO = #{ title: "Auger", name: "rhai_auger" };

fn variation(z, p) {
    let freq = p.freq;
    let weight = p.weight;
    let scale = p.scale;
    let sym = p.sym;
    let s = sin(freq * z.re);
    let t = sin(freq * z.im);
    let dx = z.re + weight * (0.5 * scale * t + abs(z.re) * t);
    let dy = z.im + weight * (0.5 * scale * s + abs(z.im) * s);
    complex(z.re + sym * (dx - z.re), dy)
}
