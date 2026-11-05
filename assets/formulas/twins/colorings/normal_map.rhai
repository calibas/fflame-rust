// Normal Map, the built-in colouring's twin: the "fake 3D" relief
// lit from the derivative, as Wikimedia Commons' reference does it --
// the normal u = z / dz, normalised, against a light at an angle and a
// height: (u . v + h) / (1 + h).

const PARAMS = #{
    angle: #{
        value: 0.125, min: 0.0, max: 1.0,
        caption: "Light angle",
        hint: "Direction the light comes from, in TURNS (0.125 = 45°), measured counter-clockwise from the +x axis.",
    },
    height: #{
        value: 1.5, min: 0.0, max: 8.0,
        caption: "Light height",
        hint: "How high the light sits above the plane. Low values rake across the surface and exaggerate relief; high values flatten it toward even illumination.",
    },
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of reflection (reflection runs 0..1).",
    },
};

const INFO = #{ title: "Normal Map (3D relief)", region: "outside", bounded: true, bailout: 1.0e4 };

// Without a derivative `ctx.dz` is its seed, z / dz is z, and the
// shading a smooth function of arg z that encodes nothing about the
// surface: flat light instead.
fn finish(s, z, p, ctx) {
    let dz = ctx.dz;
    let dzl = dz.re * dz.re + dz.im * dz.im;
    if ctx.has_derivative && !(dzl < 1e-30) {
        // u = z / dz, the division written out.
        let inv = 1.0 / dzl;
        let u = complex((z.re * dz.re + z.im * dz.im) * inv, (z.im * dz.re - z.re * dz.im) * inv);
        let ul = u.abs();
        if ul < 1e-30 {
            p.scale
        } else {
            let a = p.angle * 6.283185307;
            let reflection = (u.re / ul * cos(a) + u.im / ul * sin(a) + p.height) / (1.0 + p.height);
            max(reflection, 0.0) * p.scale
        }
    } else {
        p.scale
    }
}
