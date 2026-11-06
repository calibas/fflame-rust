// Waves2 Radial, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    w2r_scalex: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X-axis sine amplitude.",
    },
    w2r_scaley: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y-axis sine amplitude.",
    },
    w2r_freqx: #{
        value: 7.0, min: -50.0, max: 50.0, free: true,
        caption: "Freq X",
        hint: "X-axis sine frequency.",
    },
    w2r_freqy: #{
        value: 13.0, min: -50.0, max: 50.0, free: true,
        caption: "Freq Y",
        hint: "Y-axis sine frequency.",
    },
    w2r_null: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Null",
        hint: "Inner radius — full effect below this distance from the origin.",
    },
    w2r_distance: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "Distance",
        hint: "Outer radius — zero effect above this distance.",
    },
};

const INFO = #{ title: "Waves2 Radial", name: "rhai_waves2_radial" };

fn variation(z, p) {
    let scalex = p.w2r_scalex;
    let scaley = p.w2r_scaley;
    let freqx = p.w2r_freqx;
    let freqy = p.w2r_freqy;
    let null_p = p.w2r_null;
    let distance = p.w2r_distance;
    let dist = sqrt(z.re * z.re + z.im * z.im);
    let denom = distance - null_p;
    let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let factor = (if dist < distance { (dist - null_p) / safe_denom } else { 1.0 });
    factor = (if dist < null_p { 0.0 } else { factor });
    complex(z.re + factor * sin(z.im * freqx) * scalex, z.im + factor * sin(z.re * freqy) * scaley)
}
