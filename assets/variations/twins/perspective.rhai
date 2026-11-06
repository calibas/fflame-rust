/// 2D perspective projection — applies a perspective foreshortening with `t
/// = 1 / (dist − y · sin(angle·π/2))`, then emits `(dist · x · t, dist ·
/// cos(angle·π/2) · y · t)`. The `angle` parameter tilts the projection
/// plane; `dist` sets the viewing distance.
///
/// # Authors
/// - Scott Draves

// Perspective, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 0.62, min: -10.0, max: 10.0, free: true,
        caption: "Angle",
        hint: "Tilt of the projection plane (in units of π/2). 0 = parallel (no perspective); 1 = perpendicular.",
    },
    dist: #{
        value: 2.2, min: -10.0, max: 10.0, free: true,
        caption: "Dist",
        hint: "Viewing distance from the projection plane. Larger = milder perspective effect.",
    },
};

const INFO = #{ title: "Perspective", name: "rhai_perspective", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi_2 = 1.5707963267948966;
    let ang = p.angle * pi_2;
    #{ vsin: sin(ang), vfcos: p.dist * cos(ang) }
}

fn variation(z, p) {
    let dist = p.dist;
    let vsin = p.vsin;
    let vfcos = p.vfcos;
    let d = dist - z.im * vsin;
    let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
    let t = 1.0 / safe_d;
    complex(dist * z.re * t, vfcos * z.im * t)
}
