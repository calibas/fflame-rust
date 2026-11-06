/// Cellular tiling — divides the plane into cells of the given size and
/// rearranges them in an interleaved pattern. Produces a checkered,
/// displaced look.
///
/// # Authors
/// - Apophysis Plugin Pack

// Cell, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    size: #{
        value: 0.6, min: 0.01, max: 10.0, free: true,
        caption: "Size",
        hint: "Width of each cell in the grid.",
    },
};

const INFO = #{ title: "Cell", name: "rhai_cell", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let size = max(p.size, 1.0e-6);
    let inv_cell_size = 1.0 / size;
    let x_i = to_float(floor(z.re * inv_cell_size));
    let y_i = to_float(floor(z.im * inv_cell_size));
    let dx = z.re - x_i * size;
    let dy = z.im - y_i * size;
    let ix = x_i;
    let iy = y_i;
    if (y_i >= 0.0) {
        if (x_i >= 0.0) {
            iy = y_i * 2.0;
            ix = x_i * 2.0;
        } else {
            iy = y_i * 2.0;
            ix = -(2.0 * x_i + 1.0);
        }
    } else if (x_i >= 0.0) {
        iy = -(2.0 * y_i + 1.0);
        ix = x_i * 2.0;
    } else {
        iy = -(2.0 * y_i + 1.0);
        ix = -(2.0 * x_i + 1.0);
    }
    complex(dx + ix * size, -(dy + iy * size))
}
