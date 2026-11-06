/// Warps points via the Gielis super-formula — a generalized rose/star
/// curve controlled by 5 shape parameters. Produces flower-like patterns.

// Super Shape, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    rnd: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "RND",
        hint: "Mix between random sampling and the input radius. 0 = pure input radius, 1 = uniform random.",
    },
    m: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "M",
        hint: "Number of symmetry petals — sets how many lobes the super-shape has.",
    },
    n1: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "N1",
        hint: "Outer exponent — controls overall puffiness or sharpness.",
    },
    n2: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "N2",
        hint: "First inner exponent — controls one half of each lobe shape.",
    },
    n3: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "N3",
        hint: "Second inner exponent — controls the other half of each lobe shape.",
    },
    holes: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Holes",
        hint: "Radial offset that punches a hole in the center.",
    },
};

const INFO = #{ title: "Super Shape", name: "rhai_super_shape", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let rnd = p.rnd;
    let m = p.m;
    let n1 = p.n1;
    let n2 = p.n2;
    let n3 = p.n3;
    let holes = p.holes;
    let pi_4 = 0.7853981633974483;
    let theta = m * 0.25 * atan2(z.re, z.im) + pi_4;
    let t1 = pow(max(abs(cos(theta)), 1.0e-30), n2);
    let t2 = pow(max(abs(sin(theta)), 1.0e-30), n3);
    let len = sqrt(z.re * z.re + z.im * z.im) + 1.0e-6;
    let safe_n1 = (if n1 == 0.0 { 1.0e-30 } else { n1 });
    let r = ((rnd * rand() + (1.0 - rnd) * len) - holes) * pow(max(t1 + t2, 1.0e-30), -1.0 / safe_n1) / len;
    complex(r * z.re, r * z.im)
}
