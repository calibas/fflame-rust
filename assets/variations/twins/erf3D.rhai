// Erf 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Erf 3D", name: "rhai_erf3d" };

fn variation(z, p) {
    let pp = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let t1 = z.re;
    let sx = (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 });
    let ax = abs(z.re);
    let tx = 1.0 / (1.0 + pp * ax);
    let polyx = ((((a5 * tx + a4) * tx + a3) * tx + a2) * tx + a1) * tx;
    let fx = sx * (1.0 - polyx * exp(-ax * ax));
    let t2 = z.im;
    let sy = (if t2 > 0.0 { 1.0 } else if t2 < 0.0 { -1.0 } else { 0.0 });
    let ay = abs(z.im);
    let ty = 1.0 / (1.0 + pp * ay);
    let polyy = ((((a5 * ty + a4) * ty + a3) * ty + a2) * ty + a1) * ty;
    let fy = sy * (1.0 - polyy * exp(-ay * ay));
    complex(fx, fy)
}

fn variation3(v, p) {
    let pp = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let t1 = v.x;
    let sx = (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 });
    let ax = abs(v.x);
    let tx = 1.0 / (1.0 + pp * ax);
    let polyx = ((((a5 * tx + a4) * tx + a3) * tx + a2) * tx + a1) * tx;
    let fx = sx * (1.0 - polyx * exp(-ax * ax));
    let t2 = v.y;
    let sy = (if t2 > 0.0 { 1.0 } else if t2 < 0.0 { -1.0 } else { 0.0 });
    let ay = abs(v.y);
    let ty = 1.0 / (1.0 + pp * ay);
    let polyy = ((((a5 * ty + a4) * ty + a3) * ty + a2) * ty + a1) * ty;
    let fy = sy * (1.0 - polyy * exp(-ay * ay));
    let t3 = v.z;
    let sz = (if t3 > 0.0 { 1.0 } else if t3 < 0.0 { -1.0 } else { 0.0 });
    let az = abs(v.z);
    let tz = 1.0 / (1.0 + pp * az);
    let polyz = ((((a5 * tz + a4) * tz + a3) * tz + a2) * tz + a1) * tz;
    let fz = sz * (1.0 - polyz * exp(-az * az));
    vec3(fx, fy, fz)
}
