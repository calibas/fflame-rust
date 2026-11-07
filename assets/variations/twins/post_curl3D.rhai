/// 3D version of Post Curl — applies a complex polynomial twist along all
/// three axes after all other variations have run. Each axis has its own
/// twist coefficient.

// Post Curl 3D, the built-in variation's twin, translated from its WGSL.

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

const INFO = #{ title: "Post Curl 3D", name: "rhai_post_curl3d", category: "full_3d", phase: "post" };

fn variation(z, p) {
    z
}

fn variation3(v, p) {
    let cx = p.cx;
    let cy = p.cy;
    let cz = p.cz;
    let x = clamp(v.x, -1.0e30, 1.0e30);
    let y = clamp(v.y, -1.0e30, 1.0e30);
    let z = clamp(v.z, -1.0e30, 1.0e30);
    let r2 = x * x + y * y + z * z;
    let c2 = cx * cx + cy * cy + cz * cz;
    let denom = r2 * c2 + 2.0 * cx * x - 2.0 * cy * y + 2.0 * cz * z + 1.0;
    let r = 1.0 / denom;
    vec3(r * (x + cx * r2), r * (y - cy * r2), r * (z + cz * r2))
}
