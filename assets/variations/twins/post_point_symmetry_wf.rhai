/// Post-phase N-fold rotational symmetry — each iteration picks a random
/// rotation index in `[0, order)` and rotates the accumulator point by `idx
/// · 2π/order` around `(centre_x, centre_y)`. Produces N-fold rotational
/// symmetry around a configurable center.

// Post Point Symmetry WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    centre_x: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Centre X",
        hint: "X center of the rotational symmetry.",
    },
    centre_y: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Centre Y",
        hint: "Y center of the rotational symmetry.",
    },
    order: #{
        value: 3, min: 1, max: 16,
        caption: "Order",
        hint: "Number of rotational-symmetry orders (≥ 1). 3 = 3-fold rotation, 4 = 4-fold, etc.",
    },
    colorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Color Shift",
        hint: "Color-register shift per symmetry copy: `color = fmod(color + idx·colorshift, 1)` where `idx` is the randomly chosen rotation index. Visible color requires the transform's Direct Color slider > 0.",
    },
};

const INFO = #{ title: "Post Point Symmetry WF", name: "rhai_post_point_symmetry_wf", category: "advanced_2d", phase: "post" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let cx = p.centre_x;
    let cy = p.centre_y;
    let order = max(to_int(to_float(p.order)), 1);
    let colorshift = p.colorshift;
    let order_f = to_float(order);
    let w = ctx.weight;
    let two_pi = 6.28318530717959;
    let dx = (z.re - cx) * w;
    let dy = (z.im - cy) * w;
    let idx = to_int(rand() * order_f);
    let idx_clamped = clamp(idx, 0, order - 1);
    let angle = to_float(idx_clamped) * two_pi / order_f;
    let cosa = cos(angle);
    let sina = sin(angle);
    let craw = color + to_float(idx_clamped) * colorshift;
    color = craw - to_float(trunc(craw));
    #{ z: complex(cx + dx * cosa + dy * sina, cy + dy * cosa - dx * sina), color: color }
}
