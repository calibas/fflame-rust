/// 3D log-tiled spreader — each axis is independently shifted by a random
/// integer drawn from `log(uniform)` rounded to the nearest integer.
/// Produces a stamped tile effect with geometric falloff.
///
/// # Authors
/// - Zy0rg

// Log Tile2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    spreadx: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread X",
        hint: "X-axis tile spacing. The random integer from the log-of-uniform draw is multiplied by this value.",
    },
    spready: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread Y",
        hint: "Y-axis tile spacing.",
    },
    spreadz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread Z",
        hint: "Z-axis tile spacing (3D mode only).",
    },
};

const INFO = #{ title: "Log Tile2", name: "rhai_log_tile2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let sx = p.spreadx;
    let sy = p.spready;
    let spreadx = (if rand() < 0.5 { sx } else { -sx });
    let spready = (if rand() < 0.5 { sy } else { -sy });
    let lx = round_even(ln(max(1.0e-30, rand())));
    let ly = round_even(ln(max(1.0e-30, rand())));
    complex(z.re + spreadx * lx, z.im + spready * ly)
}

fn variation3(v, p) {
    let sx = p.spreadx;
    let sy = p.spready;
    let sz = p.spreadz;
    let spreadx = (if rand() < 0.5 { sx } else { -sx });
    let spready = (if rand() < 0.5 { sy } else { -sy });
    let spreadz = (if rand() < 0.5 { sz } else { -sz });
    let lx = round_even(ln(max(1.0e-30, rand())));
    let ly = round_even(ln(max(1.0e-30, rand())));
    let lz = round_even(ln(max(1.0e-30, rand())));
    vec3(v.x + spreadx * lx, v.y + spready * ly, v.z + spreadz * lz)
}
