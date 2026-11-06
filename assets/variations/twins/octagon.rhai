// Octagon, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis sign-shift amount added at the end (signed by `sign(x)`).",
    },
    y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis sign-shift amount.",
    },
    z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z",
        hint: "Z-axis sign-shift amount (3D only).",
    },
};

const INFO = #{ title: "Octagon", name: "rhai_octagon" };

fn variation(z, p) {
    let xp = p.x;
    let yp = p.y;
    let denom_r = max(z.re * z.re * z.re * z.re + z.im * z.im * z.im * z.im, 1.0e-30);
    let r = 1.0 / denom_r;
    let fpx = 0.0;
    let fpy = 0.0;
    if (r < 2.0) {
        fpx = z.re * r;
        fpy = z.im * r;
    } else {
        fpx = z.re;
        fpy = z.im;
    }
    let denom_t = max(sqrt(abs(z.re * z.re)) + sqrt(abs(z.im * z.im)), 1.0e-30);
    let t = 1.0 / denom_t;
    if (r >= 0.0) {
        fpx = fpx + z.re * t;
        fpy = fpy + z.im * t;
    } else {
        fpx = fpx + z.re;
        fpy = fpy + z.im;
    }
    if (z.re >= 0.0) {
        fpx = fpx + z.re + xp;
    } else {
        fpx = fpx + z.re - xp;
    }
    if (z.im >= 0.0) {
        fpy = fpy + z.im + yp;
    } else {
        fpy = fpy + z.im - yp;
    }
    complex(fpx, fpy)
}

fn variation3(v, p) {
    let xp = p.x;
    let yp = p.y;
    let zp = p.z;
    let denom_r = max(v.x * v.x * v.x * v.x + v.z * v.z + v.y * v.y * v.y * v.y + v.z * v.z, 1.0e-30);
    let r = 1.0 / denom_r;
    let fpx = 0.0;
    let fpy = 0.0;
    let fpz = 0.0;
    if (r < 2.0) {
        fpx = v.x * r;
        fpy = v.y * r;
        fpz = v.z * r;
    } else {
        fpx = v.x;
        fpy = v.y;
        fpz = v.z;
    }
    let denom_t = max(sqrt(abs(v.x * v.x)) + sqrt(abs(v.z)) + sqrt(abs(v.y * v.y)) + sqrt(abs(v.z)), 1.0e-30);
    let t = 1.0 / denom_t;
    if (r >= 0.0) {
        fpx = fpx + v.x * t;
        fpy = fpy + v.y * t;
        fpz = fpz + v.z * t;
    } else {
        fpx = fpx + v.x;
        fpy = fpy + v.y;
        fpz = fpz + v.z;
    }
    if (v.x >= 0.0) {
        fpx = fpx + v.x + xp;
    } else {
        fpx = fpx + v.x - xp;
    }
    if (v.y >= 0.0) {
        fpy = fpy + v.y + yp;
    } else {
        fpy = fpy + v.y - yp;
    }
    if (v.z >= 0.0) {
        fpz = fpz + v.z + zp;
    } else {
        fpz = fpz + v.z - zp;
    }
    vec3(fpx, fpy, fpz)
}
