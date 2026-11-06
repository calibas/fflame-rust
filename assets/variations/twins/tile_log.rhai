// Tile Log, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    spread: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread",
        hint: "Tile spacing along X. The random integer from the log-of-uniform draw is multiplied by this value.",
    },
};

const INFO = #{ title: "Tile Log", name: "rhai_tile_log" };

fn variation(z, p) {
    let sp = p.spread;
    let x = (if rand() < 0.5 { sp } else { -sp });
    complex(z.re + round_even(x * ln(max(rand(), 1.0e-30))), z.im)
}

fn variation3(v, p) {
    let sp = p.spread;
    let x = (if rand() < 0.5 { sp } else { -sp });
    vec3(v.x + round_even(x * ln(max(rand(), 1.0e-30))), v.y, v.z)
}
