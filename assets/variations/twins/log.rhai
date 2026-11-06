/// Polar log transform - the output X is the logarithm of the squared
/// distance, the output Y is the angle. Produces a spiral log-scale view.
///

// Log, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    base: #{
        value: 2.7182817, min: 1.01, max: 100.0, free: true,
        caption: "Base",
        hint: "Logarithm base. Default `e` (natural log); larger compresses the output, smaller stretches it out.",
    },
};

const INFO = #{ title: "Log", name: "rhai_log", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ denom: 0.5 / ln(max(1.000001, p.base)) }
}

fn variation(z, p) {
    let denom = p.denom;
    let r2 = z.norm_sqr();
    complex(ln(r2) * denom, atan2(z.im, z.re))
}
