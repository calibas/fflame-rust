// Blur Zoom, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    length: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Length",
        hint: "Maximum zoom distance. Larger values streak points further outward from the center.",
    },
    x: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "X",
        hint: "X coordinate of the zoom center.",
    },
    y: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "Y",
        hint: "Y coordinate of the zoom center.",
    },
};

const INFO = #{ title: "Blur Zoom", name: "rhai_blur_zoom" };

fn variation(w, p) {
    let length = p.length;
    let zoom_x = p.x;
    let zoom_y = p.y;
    let z = 1.0 + length * rand();
    complex((w.re - zoom_x) * z + zoom_x, (w.im - zoom_y) * z + zoom_y)
}
