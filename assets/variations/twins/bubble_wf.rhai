/// Bubble inversion with random Z bump — applies the standard bubble
/// inversion `(x, y) / (1 + r²/4)` to XY, plus a random `±(2/r − 1)` Z bump
/// (sign chosen by coin flip). The XY portion matches the classic `bubble`
/// variation; Z gets per-iteration random spheres above and below the
/// bubble.

// Bubble WF, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Bubble WF", name: "rhai_bubble_wf", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let r = (z.re * z.re + z.im * z.im) * 0.25 + 1.0;
    let safe_r = (if abs(r) < 1.0e-30 { 1.0e-30 } else { r });
    let t = 1.0 / safe_r;
    complex(t * z.re, t * z.im)
}

fn variation3(v, p) {
    let r = (v.x * v.x + v.y * v.y) * 0.25 + 1.0;
    let safe_r = (if abs(r) < 1.0e-30 { 1.0e-30 } else { r });
    let t = 1.0 / safe_r;
    let z_bump = 2.0 / safe_r - 1.0;
    let z = (if rand() < 0.5 { -z_bump } else { z_bump });
    vec3(t * v.x, t * v.y, z)
}
