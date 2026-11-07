/// 3D piecewise erf / 1/r² — per-axis, if `|coord| ≥ 2` the output is
/// `coord / r²` (spherical inversion); otherwise it's `erf(coord)`.
/// Combines sigmoid saturation near the origin with spherical inversion far
/// away.
///
/// # Authors
/// - zephyrtronium
/// - DarkBeam

// X Erf, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "X Erf", name: "rhai_xerf", category: "full_3d", phase: "any" };

fn xerf_erf(x) {
    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let s = (if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 });
    let ax = abs(x);
    let t = 1.0 / (1.0 + p * ax);
    let poly = ((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t;
    s * (1.0 - poly * exp(-ax * ax))
}

fn variation(z, p) {
    let r2 = max(1.0e-30, z.re * z.re + z.im * z.im);
    let fx = (if abs(z.re) >= 2.0 { z.re / r2 } else { xerf_erf(z.re) });
    let fy = (if abs(z.im) >= 2.0 { z.im / r2 } else { xerf_erf(z.im) });
    complex(fx, fy)
}

fn variation3(v, p) {
    let r2 = max(1.0e-30, v.x * v.x + v.y * v.y + v.z * v.z);
    let fx = (if abs(v.x) >= 2.0 { v.x / r2 } else { xerf_erf(v.x) });
    let fy = (if abs(v.y) >= 2.0 { v.y / r2 } else { xerf_erf(v.y) });
    let fz = (if abs(v.z) >= 2.0 { v.z / r2 } else { xerf_erf(v.z) });
    vec3(fx, fy, fz)
}
