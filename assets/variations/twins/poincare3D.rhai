/// Poincaré-disc-style 3D hyperbolic-tiling generator — projects the input
/// through a Möbius inversion centered at `c = (-r·cos(a·π/2)·cos(b·π/2),
/// r·sin(a·π/2)·cos(b·π/2), -r·sin(b·π/2))`. The `r, a, b` parameters
/// control the center's distance from origin and its angular position on a
/// sphere of radius `r`.
///
/// # Authors
/// - Zueuk

// Poincare 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "R",
        hint: "Center distance from origin (radius of the inversion center).",
    },
    a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Azimuthal angle of the inversion center (in units of π/2).",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Polar angle of the inversion center (in units of π/2).",
    },
};

const INFO = #{ title: "Poincare 3D", name: "rhai_poincare3d", category: "full_3d", phase: "any" };

fn prepare(p) {
    let pi_2 = 1.5707963267948966;
    let r = p.r;
    let a = p.a;
    let b = p.b;
    let cb = cos(b * pi_2);
    let cx = -r * cos(a * pi_2) * cb;
    let cy = r * sin(a * pi_2) * cb;
    let cz = -r * sin(b * pi_2);
    let cx2 = cx * cx;
    let cy2 = cy * cy;
    let cz2 = cz * cz;
    #{ cx: cx, cy: cy, cz: cz, c2: cx2 + cy2 + cz2, c2x: 2.0 * cx, c2y: 2.0 * cy, c2z: 2.0 * cz, s2x: cx2 - cy2 - cz2 + 1.0, s2y: cy2 - cx2 - cz2 + 1.0, s2z: cz2 - cy2 - cx2 + 1.0 }
}

fn variation(z, p) {
    let cx = p.cx;
    let cy = p.cy;
    let c2 = p.c2;
    let c2x = p.c2x;
    let c2y = p.c2y;
    let s2x = p.s2x;
    let s2y = p.s2y;
    let r2 = z.re * z.re + z.im * z.im;
    let x2cx = c2x * z.re;
    let y2cy = c2y * z.im;
    let denom = c2 * r2 - x2cx - y2cy + 1.0;
    let d = 1.0 / (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let fx = d * (z.re * s2x + cx * (y2cy - r2 - 1.0));
    let fy = d * (z.im * s2y + cy * (x2cx - r2 - 1.0));
    complex(fx, fy)
}

fn variation3(v, p) {
    let cx = p.cx;
    let cy = p.cy;
    let cz = p.cz;
    let c2 = p.c2;
    let c2x = p.c2x;
    let c2y = p.c2y;
    let c2z = p.c2z;
    let s2x = p.s2x;
    let s2y = p.s2y;
    let s2z = p.s2z;
    let r2 = v.x * v.x + v.y * v.y + v.z * v.z;
    let x2cx = c2x * v.x;
    let y2cy = c2y * v.y;
    let z2cz = c2z * v.z;
    let denom = c2 * r2 - x2cx - y2cy - z2cz + 1.0;
    let d = 1.0 / (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let fx = d * (v.x * s2x + cx * (y2cy + z2cz - r2 - 1.0));
    let fy = d * (v.y * s2y + cy * (x2cx + z2cz - r2 - 1.0));
    let fz = d * (v.z * s2z + cz * (y2cy + x2cx - r2 - 1.0));
    vec3(fx, fy, fz)
}
