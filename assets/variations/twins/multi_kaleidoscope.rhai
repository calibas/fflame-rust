/// Multi-emit kaleidoscope: all N sector copies plotted per iteration —
/// dihedral mirrors, log-spiral copy scaling, twist shear, movable
/// center; mono-equivalent brightness at any order.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Multi Kaleidoscope, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    order: #{
        value: 6, min: 2, max: 16,
        caption: "Order",
        hint: "Number of sector copies plotted per iteration (all emitted at once; the un-copied center plot is suppressed). Total brightness stays mono-equivalent at any order — each copy carries weight Brightness/Order.",
    },
    angle: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Phase of the whole fan — rotates every sector together about the center.",
    },
    twist: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Twist",
        hint: "Extra rotation added PER COPY on top of the even 360/Order spacing. Nonzero shears the fan out of exact symmetry into a swirl — the thing a rigid symmetry group cannot do. Animates beautifully.",
    },
    spiral: #{
        value: 1.0, min: 0.5, max: 1.5,
        caption: "Spiral",
        hint: "Per-copy scale factor about the center: copy k is scaled by Spiral^k. 1 = flat kaleidoscope; below 1 the sectors telescope inward into a logarithmic spiral, above 1 outward.",
    },
    mirror: #{
        value: false,
        caption: "Mirror",
        hint: "Dihedral mode: odd copies are reflected before rotation, giving the alternating mirror-image sectors of a physical two-mirror kaleidoscope (D_n symmetry instead of C_n).",
    },
    cx: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Center X",
        hint: "Kaleidoscope center x — copies rotate and scale about this point.",
    },
    cy: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Center Y",
        hint: "Kaleidoscope center y.",
    },
    brightness: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Brightness",
        hint: "Total density multiplier. 1 = mono-equivalent (each copy at 1/Order weight). Per-copy weights below ~0.01 floor to zero in the histogram, so very high Order plus very low Brightness can vanish.",
    },
};

const INFO = #{ title: "Multi Kaleidoscope", name: "rhai_multi_kaleidoscope", category: "advanced_2d", z: "preserve_z", emits: 16 };

fn variation(z, p) {
    let order = clamp(u32(to_float(p.order)), u32(2), u32(16));
    let angle = p.angle * 0.01745329252;
    let twist = p.twist * 0.01745329252;
    let spiral = p.spiral;
    let mirror = to_float(p.mirror) > 0.5;
    let c = complex(p.cx, p.cy);
    let brightness = p.brightness;
    suppress_main();
    let v0 = z - c;
    let w = brightness / to_float(order);
    let step = 6.28318530718 / to_float(order);
    let scale = 1.0;
    for k_n in 0..to_int(order) {
        let k = u32(k_n);
        let v = v0;
        if (mirror && (k & u32(1)) == u32(1)) {
            v = complex(v.re, -v.im);
        }
        let a = angle + to_float(k) * (step + twist);
        let ca = cos(a);
        let sa = sin(a);
        v = complex(ca * v.re - sa * v.im, sa * v.re + ca * v.im) * scale;
        emit(c + v, w);
        scale = scale * spiral;
    }
    z
}

fn variation3(q, p) {
    let order = clamp(u32(to_float(p.order)), u32(2), u32(16));
    let angle = p.angle * 0.01745329252;
    let twist = p.twist * 0.01745329252;
    let spiral = p.spiral;
    let mirror = to_float(p.mirror) > 0.5;
    let c = complex(p.cx, p.cy);
    let brightness = p.brightness;
    suppress_main();
    let v0 = complex(q.x, q.y) - c;
    let w = brightness / to_float(order);
    let step = 6.28318530718 / to_float(order);
    let scale = 1.0;
    for k_n in 0..to_int(order) {
        let k = u32(k_n);
        let v = v0;
        if (mirror && (k & u32(1)) == u32(1)) {
            v = complex(v.re, -v.im);
        }
        let a = angle + to_float(k) * (step + twist);
        let ca = cos(a);
        let sa = sin(a);
        v = complex(ca * v.re - sa * v.im, sa * v.re + ca * v.im) * scale;
        let t1 = c + v;
        emit(vec3(t1.re, t1.im, q.z), w);
        scale = scale * spiral;
    }
    q
}
