/// Multi-emit stochastic blur: count jittered copies per iteration
/// (Gaussian / Disc / Ring / Streak kernels, elliptical aspect,
/// orientable), mono-equivalent brightness, count× faster convergence
/// than a one-sample blur.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Spray Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    count: #{
        value: 8, min: 1, max: 16,
        caption: "Count",
        hint: "Jittered copies plotted per iteration. The blur fills in Count× faster than a one-sample stochastic blur at the same total density (each copy carries weight Brightness/Count). Mixes distortion-free with other variations (post phase, identity pass-through); copies center on the final plotted position (post-affine and Final chains included) and the trajectory continues un-blurred. A transform with ONLY spray needs a companion linear 1 (the post phase acts on the normal-phase result).",
    },
    radius: #{
        value: 0.15, min: 0.0, max: 2.0,
        caption: "Radius",
        hint: "Kernel size in world units: Gaussian scale, Disc/Ring radius, Streak half-length.",
    },
    shape: #{
        value: 0, choices: ["Gaussian", "Disc", "Ring", "Streak"],
        caption: "Shape",
        hint: "Jitter kernel. Gaussian: soft falloff (the classic gaussian_blur distribution). Disc: uniform fill — flat bokeh. Ring: exact circle — bokeh rings / neon doubling. Streak: uniform line segment along Angle — motion blur.",
    },
    angle: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Kernel orientation: the Streak direction, and the major-axis direction of an elliptical (Aspect ≠ 1) Gaussian/Disc/Ring.",
    },
    aspect: #{
        value: 1.0, min: 0.05, max: 4.0,
        caption: "Aspect",
        hint: "Kernel y-scale before orientation: below 1 squashes the kernel into an ellipse along Angle — directional bokeh without a full streak.",
    },
    brightness: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Brightness",
        hint: "Total density multiplier. 1 = mono-equivalent (each copy at 1/Count weight). Per-copy weights below ~0.01 floor to zero in the histogram.",
    },
};

const INFO = #{ title: "Spray Blur", name: "rhai_spray_blur", category: "advanced_2d", phase: "post", slow: ["count"], z: "preserve_z", emits: 16 };

fn spray_blur_offset(shape, radius) {
    if (shape == u32(0)) {
        let theta = rand() * 6.28318530718;
        let r = rand() + rand() + rand() + rand() - 2.0;
        return radius * r * complex(cos(theta), sin(theta));
    }
    if (shape == u32(1)) {
        let theta = rand() * 6.28318530718;
        let r = radius * sqrt(rand());
        return r * complex(cos(theta), sin(theta));
    }
    if (shape == u32(2)) {
        let theta = rand() * 6.28318530718;
        return radius * complex(cos(theta), sin(theta));
    }
    complex(radius * (rand() * 2.0 - 1.0), 0.0)
}

fn variation(z, p) {
    let count = clamp(u32(to_float(p.count)), u32(1), u32(16));
    let radius = p.radius;
    let shape = u32(to_float(p.shape));
    let angle = p.angle * 0.01745329252;
    let aspect = p.aspect;
    let brightness = p.brightness;
    suppress_main();
    let w = brightness / to_float(count);
    let ca = cos(angle);
    let sa = sin(angle);
    for k_n in 0..to_int(count) {
        let k = u32(k_n);
        let o = spray_blur_offset(shape, radius);
        o = complex(o.re, o.im * aspect);
        o = complex(ca * o.re - sa * o.im, sa * o.re + ca * o.im);
        emit_offset(o, w);
    }
    z
}

fn variation3(v, p) {
    let count = clamp(u32(to_float(p.count)), u32(1), u32(16));
    let radius = p.radius;
    let shape = u32(to_float(p.shape));
    let angle = p.angle * 0.01745329252;
    let aspect = p.aspect;
    let brightness = p.brightness;
    suppress_main();
    let w = brightness / to_float(count);
    let ca = cos(angle);
    let sa = sin(angle);
    for k_n in 0..to_int(count) {
        let k = u32(k_n);
        let o = spray_blur_offset(shape, radius);
        o = complex(o.re, o.im * aspect);
        o = complex(ca * o.re - sa * o.im, sa * o.re + ca * o.im);
        emit_offset(vec3(o.re, o.im, 0.0), w);
    }
    v
}
