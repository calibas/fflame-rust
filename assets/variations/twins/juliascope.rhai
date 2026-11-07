/// Kaleidoscope variant of Julia — splits the angle into `power` branches
/// with random sign-flipping, producing symmetric mirror-like patterns.
///
/// # Authors
/// - Scott Draves

// JuliaScope, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 2, min: -20, max: 20, free: true,
        caption: "Power",
        hint: "Number of mirror branches. Higher = more reflections; negative values invert the rotation.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Radial scaling factor. 1.0 is balanced; larger values push arms outward.",
    },
};

const INFO = #{ title: "JuliaScope", name: "rhai_juliascope", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let power = to_int(to_float(p.power));
    let dist = p.dist;
    let r2 = z.norm_sqr();
    let absp = abs(power);
    let rnd = to_int(to_float(absp) * rand());
    let phi = atan2(z.im, z.re);
    let a = (2.0 * PI() * to_float(rnd) + (if (rnd & 1) == 0 { phi } else { -phi })) / to_float(power);
    let cn = dist / to_float(power) * 0.5;
    let r = pow(r2, cn);
    complex(r * cos(a), r * sin(a))
}

fn variation3(v, p) {
    let PI = 3.14159265359;
    let power = to_int(to_float(p.power));
    let dist = p.dist;
    let r2 = v.x * v.x + v.y * v.y;
    let absp = abs(power);
    let rnd = to_int(to_float(absp) * rand());
    let phi = atan2(v.y, v.x);
    let a = (2.0 * PI() * to_float(rnd) + (if (rnd & 1) == 0 { phi } else { -phi })) / to_float(power);
    let cn = dist / to_float(power) * 0.5;
    let r = pow(r2, cn);
    vec3(r * cos(a), r * sin(a), v.z)
}
