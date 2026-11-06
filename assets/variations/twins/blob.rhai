// Blob, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    high: #{
        value: 1.0, min: -3.0, max: 3.0, free: true,
        caption: "High",
        hint: "Outer radius - how far the bumps reach at their peaks.",
    },
    low: #{
        value: 1.0, min: -3.0, max: 3.0, free: true,
        caption: "Low",
        hint: "Inner radius - how close the bumps recede in the troughs.",
    },
    waves: #{
        value: 6, min: 1, max: 20, free: true,
        caption: "Waves",
        hint: "How many bumps go around the perimeter. More waves = finer-grained edge.",
    },
};

const INFO = #{ title: "Blob", name: "rhai_blob" };

fn variation(z, p) {
    let p1 = p.high;
    let p2 = p.low;
    let p3 = to_float(p.waves);
    let r = z.abs();
    let theta = atan2(z.re, z.im);
    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));
    complex(scale * cos(theta), scale * sin(theta))
}
