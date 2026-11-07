/// Lorenz attractor (Euler-step IFS) — integrates one Euler step of the
/// Lorenz system: `dx/dt = a·(y − x)`, `dy/dt = x·(b − z) − y`, `dz/dt =
/// x·y − c·z`. With the classic parameters `a = 10, b = 28, c = 8/3` and
/// small `h`, produces the iconic butterfly attractor.
///
/// # Authors
/// - Jesus Sosa

// Lorenz (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "A",
        hint: "Lorenz parameter `a` (Prandtl number; classic value 10).",
    },
    b: #{
        value: 28.0, min: -100.0, max: 100.0, free: true,
        caption: "B",
        hint: "Lorenz parameter `b` (Rayleigh number; classic value 28).",
    },
    c: #{
        value: 1.66, min: -100.0, max: 100.0, free: true,
        caption: "C",
        hint: "Lorenz parameter `c` (geometric ratio; classic value 8/3 ≈ 2.67).",
    },
    h: #{
        value: 1.0e-5, min: -1.0, max: 1.0, free: true,
        caption: "H (step)",
        hint: "Euler integration step size.",
    },
    centerx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Center X",
        hint: "Color register X center — offsets the squared-distance color formula. Visible color requires the transform's Direct Color slider > 0.",
    },
    centery: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Center Y",
        hint: "Color register Y center — offsets the squared-distance color formula.",
    },
    scale: #{
        value: 1000.0, min: -10000.0, max: 10000.0, free: true,
        caption: "Scale",
        hint: "Color register scale — the color formula multiplies the squared distance by `1/scale` (precomputed at init as `_bdcs`).",
    },
};

const INFO = #{ title: "Lorenz (JS)", name: "rhai_lorenz_js", category: "full_3d" };

fn prepare(p) {
    let scale = p.scale;
    let safe_scale = (if abs(scale) < 1.0e-30 { 1.0e-5 } else { scale });
    #{ bdcs: 1.0 / safe_scale }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let h = p.h;
    let x = z.re + h * a * (z.im - z.re);
    let y = z.im + h * (z.re * b - z.im);
    let centerx = p.centerx;
    let centery = p.centery;
    let bdcs = p.bdcs;
    let w = ctx.weight;
    let fpx = ctx.accum.re + w * x + centerx;
    let fpy = ctx.accum.im + w * y + centery;
    let craw = abs(bdcs * (fpx * fpx + fpy * fpy));
    color = craw - to_float(floor(craw));
    #{ z: complex(x, y), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let h = p.h;
    let x = v.x + h * a * (v.y - v.x);
    let y = v.y + h * (v.x * (b - v.z) - v.y);
    let z = v.z + h * (v.x * v.y - c * v.z);
    let centerx = p.centerx;
    let centery = p.centery;
    let bdcs = p.bdcs;
    let w = ctx.weight;
    let fpx = ctx.accum.x + w * x + centerx;
    let fpy = ctx.accum.y + w * y + centery;
    let craw = abs(bdcs * (fpx * fpx + fpy * fpy));
    color = craw - to_float(floor(craw));
    #{ z: vec3(x, y, z), color: color }
}
