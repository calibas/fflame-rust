/// Julia + hemisphere mix forming a cone — combines a Julia-style angular
/// pick `π·floor(weight · rand)·radius2 + atan2(y,x)·radius1` with a
/// hemisphere-style radial term `r = size2 / sqrt(x²·warp + y² + size1)`,
/// plus a configurable `height` Z output. The result traces a cone-shaped
/// surface in 3D.
///
/// # Authors
/// - Brad Stefanov

// Cone, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius1: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Radius 1",
        hint: "Inner-radius multiplier on the input angle.",
    },
    radius2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius 2",
        hint: "Outer-radius multiplier on the random branch offset.",
    },
    size1: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Size 1",
        hint: "Squared-radius offset in the denominator.",
    },
    size2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Size 2",
        hint: "Output radius scale (numerator of `r`).",
    },
    ywave: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Wave",
        hint: "Y-axis frequency multiplier in `sin(xx·ywave)`.",
    },
    xwave: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Wave",
        hint: "X-axis frequency multiplier in `cos(xx·xwave)`.",
    },
    height: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Height",
        hint: "Z output scale (3D only — controls cone height).",
    },
    warp: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Warp",
        hint: "X² weight in the denominator. Controls the aspect ratio of the cone.",
    },
    weight: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Weight",
        hint: "Number of random angular branches — `floor(weight · rand)` picks the branch.",
    },
};

const INFO = #{ title: "Cone", name: "rhai_cone", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let radius1 = p.radius1;
    let radius2 = p.radius2;
    let size1 = p.size1;
    let size2 = p.size2;
    let ywave = p.ywave;
    let xwave = p.xwave;
    let warp = p.warp;
    let weight = p.weight;
    let pi = 3.14159265358979;
    let denom = sqrt(max(z.re * z.re * warp + z.im * z.im + size1, 1.0e-30));
    let r = size2 / denom;
    let xx = atan2(z.im, z.re) * radius1 + pi * to_float(floor(weight * rand())) * radius2;
    complex(r * cos(xx * xwave), r * sin(xx * ywave))
}

fn variation3(v, p) {
    let radius1 = p.radius1;
    let radius2 = p.radius2;
    let size1 = p.size1;
    let size2 = p.size2;
    let ywave = p.ywave;
    let xwave = p.xwave;
    let height = p.height;
    let warp = p.warp;
    let weight = p.weight;
    let pi = 3.14159265358979;
    let denom = sqrt(max(v.x * v.x * warp + v.y * v.y + size1, 1.0e-30));
    let r = size2 / denom;
    let xx = atan2(v.y, v.x) * radius1 + pi * to_float(floor(weight * rand())) * radius2;
    vec3(r * cos(xx * xwave), r * sin(xx * ywave), r * height)
}
