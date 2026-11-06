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

const INFO = #{ title: "Log Tile2", name: "rhai_log_tile2" };

fn variation(z, p) {
    let sx = p.spreadx;
    let sy = p.spready;
    let spreadx = (if rand() < 0.5 { sx } else { -sx });
    let spready = (if rand() < 0.5 { sy } else { -sy });
    let lx = round_even(ln(max(rand(), 1.0e-30)));
    let ly = round_even(ln(max(rand(), 1.0e-30)));
    complex(z.re + spreadx * lx, z.im + spready * ly)
}

fn variation3(v, p) {
    let sx = p.spreadx;
    let sy = p.spready;
    let sz = p.spreadz;
    let spreadx = (if rand() < 0.5 { sx } else { -sx });
    let spready = (if rand() < 0.5 { sy } else { -sy });
    let spreadz = (if rand() < 0.5 { sz } else { -sz });
    let lx = round_even(ln(max(rand(), 1.0e-30)));
    let ly = round_even(ln(max(rand(), 1.0e-30)));
    let lz = round_even(ln(max(rand(), 1.0e-30)));
    vec3(v.x + spreadx * lx, v.y + spready * ly, v.z + spreadz * lz)
}
