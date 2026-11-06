// Foci 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Foci 3D", name: "rhai_foci_3d" };

fn variation(z, p) {
    let expx = exp(z.re) * 0.5;
    let expnx = 0.25 / max(expx, 1.0e-30);
    let boot = atan2(z.im, z.re);
    let cosy = cos(z.im);
    let cosz = cos(boot);
    let denom = expx + expnx - cosy * cosz;
    let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let tmp = 1.0 / safe_denom;
    let fx = (expx - expnx) * tmp;
    let fy = sin(z.im) * tmp;
    complex(fx, fy)
}

fn variation3(v, p) {
    let expx = exp(v.x) * 0.5;
    let expnx = 0.25 / max(expx, 1.0e-30);
    let boot = v.z;
    if (boot == 0.0) {
        boot = atan2(v.y, v.x);
    }
    let cosy = cos(v.y);
    let cosz = cos(boot);
    let denom = expx + expnx - cosy * cosz;
    let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let tmp = 1.0 / safe_denom;
    let fx = (expx - expnx) * tmp;
    let fy = sin(v.y) * tmp;
    let fz = sin(boot) * tmp;
    vec3(fx, fy, fz)
}
