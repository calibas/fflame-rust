/// Bends the plane into an N-sided polygon outline. Configurable side
/// count, corner sharpness, and how circle-vs-polygon the shape feels.
///
/// # Authors
/// - Scott Draves

// Ngon, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sides: #{
        value: 5, min: -10, max: 10, free: true,
        caption: "Sides",
        hint: "Number of sides of the polygon (e.g. 5 = pentagon, 6 = hexagon).",
    },
    power: #{
        value: 3.0, min: -2.0, max: 20.0, free: true,
        caption: "Power",
        hint: "Radial exponent. Stretches or compresses the polygon shape outward.",
    },
    circle: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "Circle",
        hint: "Blend between polygon and circle. 0 = pure circle, higher = sharper corners.",
    },
    corners: #{
        value: 2.0, min: -20.0, max: 20.0, free: true,
        caption: "Corners",
        hint: "Horizontal output offset. Useful for tiling the polygon outward.",
    },
};

const INFO = #{ title: "Ngon", name: "rhai_ngon", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let sides = to_float(p.sides);
    let power = p.power;
    let circle = p.circle;
    let corners = p.corners;
    let theta = atan2(z.im, z.re);
    let phi = theta - (PI() * 2.0 / sides) * to_float(floor(sides * theta / (PI() * 2.0)));
    let phi_adj = (if phi > PI() / sides { phi - 2.0 * PI() / sides } else { phi });
    let amp = cos(phi_adj) * pow(1.0 / (cos(phi_adj * sides / 2.0) + 1.0e-10), circle);
    let r = pow(z.norm_sqr(), power * 0.5);
    complex(amp * r * cos(theta) + corners, amp * r * sin(theta))
}
