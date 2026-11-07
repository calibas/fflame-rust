/// Pre-phase 3D Gaussian-sphere blur — adds a random offset along a unit-
/// sphere direction with Gaussian-distributed radius. The radius is `(sum
/// of 6 uniforms) − 3` (central-limit Gaussian approximation); direction is
/// picked uniformly on the sphere via `(sin b · cos a, sin b · sin a, cos
/// b)` with `a ∈ [0, 2π], b ∈ [0, π]`. Effectively a 3D Gaussian blur of
/// the input.

// Pre Blur 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre Blur 3D", name: "rhai_pre_blur3d", category: "full_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p, ctx) {
    let two_pi = 6.28318530717959;
    let pi = 3.14159265358979;
    let w = ctx.weight;
    let gauss = rand() + rand() + rand() + rand() + rand() + rand() - 3.0;
    let r = w * gauss;
    let a = rand() * two_pi;
    let b = rand() * pi;
    let sb = sin(b);
    complex(z.re + r * sb * cos(a), z.im + r * sb * sin(a))
}

fn variation3(v, p, ctx) {
    let two_pi = 6.28318530717959;
    let pi = 3.14159265358979;
    let w = ctx.weight;
    let gauss = rand() + rand() + rand() + rand() + rand() + rand() - 3.0;
    let r = w * gauss;
    let a = rand() * two_pi;
    let b = rand() * pi;
    let sb = sin(b);
    vec3(v.x + r * sb * cos(a), v.y + r * sb * sin(a), v.z + r * cos(b))
}
