/// Vogel-spiral sampler — places points at integer-indexed angles `i ·
/// 2π/φ²` from a golden-angle spiral. Produces the iconic sunflower-seed
/// pattern.
///
/// # Authors
/// - Victor Ganora

// Vogel, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 20, min: 1, max: 1000,
        caption: "N",
        hint: "Number of seed positions (1-1000). Larger = more points around the spiral.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Mixes the radial distance with the input coordinates. 0 = pure spiral; higher = blended with input.",
    },
};

const INFO = #{ title: "Vogel", name: "rhai_vogel", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let phi = 1.618033988749895;
    let two_pi = 6.28318530717959;
    #{ two_pi_over_phi2: two_pi / (phi * phi) }
}

fn variation(z, p) {
    let n = to_float(p.n);
    let scale = p.scale;
    let two_pi_over_phi2 = p.two_pi_over_phi2;
    let n_safe = max(1.0, n);
    let i = to_float(floor(rand() * n_safe)) + 1.0;
    let a = i * two_pi_over_phi2;
    let r = sqrt(z.re * z.re + z.im * z.im) + 1.0e-6 + sqrt(max(0.0, i));
    complex(r * (cos(a) + scale * z.re), r * (sin(a) + scale * z.im))
}

fn variation3(v, p) {
    let n = to_float(p.n);
    let scale = p.scale;
    let two_pi_over_phi2 = p.two_pi_over_phi2;
    let n_safe = max(1.0, n);
    let i = to_float(floor(rand() * n_safe)) + 1.0;
    let a = i * two_pi_over_phi2;
    let r = sqrt(v.x * v.x + v.y * v.y) + 1.0e-6 + sqrt(max(0.0, i));
    vec3(r * (cos(a) + scale * v.x), r * (sin(a) + scale * v.y), v.z)
}
