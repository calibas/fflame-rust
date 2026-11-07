/// 3D version of Curl — applies a complex polynomial twist along all three
/// axes. Each axis has its own twist coefficient.

// Curl3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cx: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "CX",
        hint: "Twist strength along the X axis.",
    },
    cy: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "CY",
        hint: "Twist strength along the Y axis.",
    },
    cz: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "CZ",
        hint: "Twist strength along the Z axis.",
    },
};

const INFO = #{ title: "Curl3D", name: "rhai_curl3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let cx = p.cx;
    let cy = p.cy;
    let r2 = z.re * z.re + z.im * z.im;
    let c2 = cx * cx + cy * cy;
    let denom = r2 * c2 + 2.0 * cx * z.re - 2.0 * cy * z.im + 1.0;
    let r = 1.0 / denom;
    complex(r * (z.re + cx * r2), r * (z.im - cy * r2))
}

fn variation3(v, p) {
    let cx = p.cx;
    let cy = p.cy;
    let cz = p.cz;
    let r2 = v.x * v.x + v.y * v.y + v.z * v.z;
    let c2 = cx * cx + cy * cy + cz * cz;
    let denom = r2 * c2 + 2.0 * cx * v.x - 2.0 * cy * v.y + 2.0 * cz * v.z + 1.0;
    let r = 1.0 / denom;
    vec3(r * (v.x + cx * r2), r * (v.y - cy * r2), r * (v.z + cz * r2))
}
