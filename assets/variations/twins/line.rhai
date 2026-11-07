/// Project to a random spot on a 3D line — picks a random distance `r =
/// rand · w` and emits along a unit vector `(cos(δπ)·cos(φπ),
/// sin(δπ)·cos(φπ), sin(φπ))`. The 2D output drops the Z component.
///
/// # Authors
/// - chronologicaldot

// Line, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    delta: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Delta",
        hint: "Azimuthal angle of the line direction (in units of π).",
    },
    phi: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Phi",
        hint: "Polar angle of the line direction (in units of π). When `δ = φ = 0`, the line projects onto the X axis.",
    },
};

const INFO = #{ title: "Line", name: "rhai_line", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let delta = p.delta;
    let phi = p.phi;
    let pi = 3.14159265358979;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let cphi = cos(phi * pi);
    let ux = cos(delta * pi) * cphi;
    let uy = sin(delta * pi) * cphi;
    let r = rand() * w;
    complex(ux * r * inv_w, uy * r * inv_w)
}

fn variation3(v, p, ctx) {
    let delta = p.delta;
    let phi = p.phi;
    let pi = 3.14159265358979;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let cphi = cos(phi * pi);
    let ux = cos(delta * pi) * cphi;
    let uy = sin(delta * pi) * cphi;
    let uz = sin(phi * pi);
    let r = rand() * w;
    vec3(ux * r * inv_w, uy * r * inv_w, uz * r * inv_w)
}
