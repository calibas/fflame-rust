/// 3D version of Blob — same wavy boundary as Blob, plus a Z component that
/// modulates with the same waves pattern.

// Blob 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    low: #{
        value: 0.3, min: -5.0, max: 5.0, free: true,
        caption: "Low",
        hint: "Inner radius — how close the bumps recede in the troughs.",
    },
    high: #{
        value: 1.2, min: -5.0, max: 5.0, free: true,
        caption: "High",
        hint: "Outer radius — how far the bumps reach at their peaks.",
    },
    waves: #{
        value: 6.0, min: 0.0, max: 30.0, free: true,
        caption: "Waves",
        hint: "Number of bumps around the perimeter.",
    },
};

const INFO = #{ title: "Blob 3D", name: "rhai_blob3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let low = p.low;
    let high = p.high;
    let waves = p.waves;
    let a = atan2(z.re, z.im);
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let r = r0 * (low + (high - low) * (0.5 + 0.5 * sin(waves * a)));
    complex(sin(a) * r, cos(a) * r)
}

fn variation3(v, p) {
    let low = p.low;
    let high = p.high;
    let waves = p.waves;
    let a = atan2(v.x, v.y);
    let r0 = sqrt(v.x * v.x + v.y * v.y);
    let r = r0 * (low + (high - low) * (0.5 + 0.5 * sin(waves * a)));
    vec3(sin(a) * r, cos(a) * r, sin(waves * a) * r)
}
