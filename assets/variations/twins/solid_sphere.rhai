/// Uniform spherical-shell sampler. IGNORES the incoming point (like
/// the classic blur family) and emits a volumetrically uniform random
/// point inside the shell `radius·[1−thickness, 1]` around a
/// configurable center. Mix at a low weight into a transform (or give
/// it its own low-weight transform) to guarantee baseline density over
/// an entire sphere — the textured variations still dominate wherever
/// they have density; this fills what they leave empty.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Solid Sphere, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Outer radius of the shell in world units.",
    },
    thickness: #{
        value: 0.1, min: 0.0, max: 1.0,
        caption: "Thickness",
        hint: "Shell depth as a fraction of the radius: samples land volumetrically uniform between radius·(1−thickness) and radius. 0 = infinitely thin surface, 1 = solid ball.",
    },
    cx: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Center X",
        hint: "World-space X of the sphere center.",
    },
    cy: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Center Y",
        hint: "World-space Y of the sphere center.",
    },
    cz: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Center Z",
        hint: "World-space Z of the sphere center.",
    },
};

const INFO = #{ title: "Solid Sphere", name: "rhai_solid_sphere", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let radius = p.radius;
    let thickness = clamp(p.thickness, 0.0, 1.0);
    let cx = p.cx;
    let cy = p.cy;
    let theta = rand() * 6.28318530718;
    let ri2 = (1.0 - thickness) * (1.0 - thickness);
    let r = radius * sqrt(mix(ri2, 1.0, rand()));
    complex(cx + r * cos(theta), cy + r * sin(theta))
}

fn variation3(v, p) {
    let radius = p.radius;
    let thickness = clamp(p.thickness, 0.0, 1.0);
    let cx = p.cx;
    let cy = p.cy;
    let cz = p.cz;
    let z = rand() * 2.0 - 1.0;
    let phi = rand() * 6.28318530718;
    let rxy = sqrt(max(0.0, 1.0 - z * z));
    let dir = vec3(rxy * cos(phi), rxy * sin(phi), z);
    let ri = 1.0 - thickness;
    let ri3 = ri * ri * ri;
    let r = radius * pow(mix(ri3, 1.0, rand()), 0.33333333);
    vec3(cx, cy, cz) + dir * r
}
