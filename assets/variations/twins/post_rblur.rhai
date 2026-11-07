/// Post-phase radial blur — jitters the accumulator point by an amount
/// proportional to its distance from `(center_x, center_y)` minus `offset`,
/// scaled by `2·strength`. Two uniform-random jitter offsets per axis.
/// Inside the `offset` radius the jitter is suppressed (clamped to 0).
///
/// # Authors
/// - Xyrus02

// Post R-Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    strength: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Strength",
        hint: "Jitter intensity (multiplied by 2 internally and by the radial distance).",
    },
    offset: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset",
        hint: "Inner radius — jitter is suppressed to 0 inside this distance from the center.",
    },
    center_x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Center X",
        hint: "X center of the radial blur.",
    },
    center_y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Center Y",
        hint: "Y center of the radial blur.",
    },
};

const INFO = #{ title: "Post R-Blur", name: "rhai_post_rblur", category: "advanced_2d", phase: "post" };

fn prepare(p) {
    #{ s2: 2.0 * p.strength }
}

fn variation(z, p, ctx) {
    let offset = p.offset;
    let cx = p.center_x;
    let cy = p.center_y;
    let s2 = p.s2;
    let w = ctx.weight;
    let dx = z.re - cx;
    let dy = z.im - cy;
    let r = sqrt(dx * dx + dy * dy) - offset;
    if (r < 0.0) {
        r = 0.0;
    }
    r = r * s2;
    complex(w * (z.re + (rand() - 0.5) * r), w * (z.im + (rand() - 0.5) * r))
}
