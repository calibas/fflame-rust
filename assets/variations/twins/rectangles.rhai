// Rectangles, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: 0.01, max: 5.0, free: true,
        caption: "X",
        hint: "Width of each rectangular tile.",
    },
    y: #{
        value: 1.0, min: 0.01, max: 5.0, free: true,
        caption: "Y",
        hint: "Height of each rectangular tile.",
    },
};

const INFO = #{ title: "Rectangles", name: "rhai_rectangles" };

fn variation(z, p) {
    let rect_x = p.x;
    let rect_y = p.y;
    complex((2.0 * to_float(floor(z.re / rect_x)) + 1.0) * rect_x - z.re, (2.0 * to_float(floor(z.im / rect_y)) + 1.0) * rect_y - z.im)
}
