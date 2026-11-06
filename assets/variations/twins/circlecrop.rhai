/// Normal-phase circle-crop — tests whether the input lies inside a circle
/// of radius `radius` centered at `(x, y)`. Inside, the input passes
/// through scaled by weight. Outside, behavior depends on `zero`: if 1, the
/// point is hidden (collapsed to origin); if 0, it scatters onto the circle
/// boundary with `scatter_area` randomization.
///
/// # Authors
/// - Xyrus02

// Circle Crop, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Crop circle radius.",
    },
    x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X center of the crop circle.",
    },
    y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y center of the crop circle.",
    },
    scatter_area: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Scatter Area",
        hint: "Random scatter band along the circle boundary. 0 = snap to boundary; ±1 = scatter across full half-radius.",
    },
    zero: #{
        value: true,
        caption: "Zero",
        hint: "When on, points outside the circle collapse to the origin. When off, they scatter onto the boundary.",
    },
};

const INFO = #{ title: "Circle Crop", name: "rhai_circlecrop", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ ca: clamp(p.scatter_area, -1.0, 1.0) }
}

fn variation(z, p, ctx) {
    let hidden = ctx.hide;
    let cr = p.radius;
    let x0 = p.x;
    let y0 = p.y;
    let zero = to_int(to_float(p.zero));
    let ca = p.ca;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let dx = z.re - x0;
    let dy = z.im - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rand() * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;
    if (cr0 && esc) {
        hidden = true;
        return #{ z: complex(0.0, 0.0), hide: hidden };
    }
    if (!cr0 && esc) {
        return #{ z: complex((w * rdc * cos(ang) + x0) * inv_w, (w * rdc * sin(ang) + y0) * inv_w), hide: hidden };
    }
    #{ z: complex((w * dx + x0) * inv_w, (w * dy + y0) * inv_w), hide: hidden }
}
