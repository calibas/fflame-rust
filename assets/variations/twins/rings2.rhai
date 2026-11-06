/// Carves the plane into concentric ring bands at the chosen spacing. Each
/// ring inverts the radial position within its band.
///

// Rings2, the built-in variation's twin: concentric rings, spaced by the value squared.

const PARAMS = #{
    val: #{
        value: 1.0, min: 0.01, max: 5.0, free: true,
        caption: "Value",
        hint: "Ring spacing. Smaller packs more rings closer together; larger spreads them out.",
    },
};

const INFO = #{ title: "Rings2", name: "rhai_rings2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let dx = p.val * p.val + 1e-10;
    let rho = sqrt(z.norm_sqr());
    let r = 2.0 - dx * (to_float(floor((rho / dx + 1.0) / 2.0)) * 2.0 / rho + 1.0);
    complex(z.re * r, z.im * r)
}
