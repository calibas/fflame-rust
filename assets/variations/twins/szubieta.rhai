/// SZubieta - Grid-of-polygons base shape.
///
/// # Authors
/// - Jesus Sosa

// SZubieta, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    width: #{
        value: 128, min: 32, max: 256, free: true,
        caption: "Width",
        hint: "Grid width in cells. Each cell hosts one 20-gon; the bitwise pattern formula reads i ∈ [0, width) for the color computation.",
    },
    height: #{
        value: 128, min: 32, max: 256, free: true,
        caption: "Height",
        hint: "Grid height in cells. Same role as `width` but for the j index.",
    },
    type: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Type",
        hint: "Pattern formula. 0 = CircleSquares (`|i² − 2·(i|j) + j²| mod 255`) — concentric square-circle interference. 1 = Square Tile Fractal (`|i & (j − 2·(i^j) + j) & i| mod 256`) — bitwise-AND fractal tiling.",
    },
    scale: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Scale",
        hint: "Size of each 20-gon relative to the grid cell. 0 collapses every polygon to its center; 1 makes them touch.",
    },
};

const INFO = #{ title: "SZubieta", name: "rhai_szubieta", category: "plugin", phase: "any" };

fn variation(z, p, ctx) {
    let vcolor = ctx.color;
    let width = to_int(to_float(p.width));
    let height = to_int(to_float(p.height));
    let pattern_type = u32(to_float(p.type));
    let scale = p.scale;
    let w_safe = max(width, 1);
    let h_safe = max(height, 1);
    let i = to_int(to_float(floor(rand() * to_float(w_safe))));
    let j = to_int(to_float(floor(rand() * to_float(h_safe))));
    let x_raw = 0;
    if (pattern_type == u32(0)) {
        x_raw = (i * i - 2 * (i | j) + j * j) % 255;
    } else {
        x_raw = (i & (j - 2 * (i ^ j) + j) & i) % 256;
    }
    let color = to_float(abs(x_raw)) / 255.0;
    vcolor = color;
    let n_sides = 20.0;
    let k = to_float(floor(rand() * n_sides));
    let t = rand();
    let two_pi = 6.28318530718;
    let angle_k = two_pi * k / n_sides;
    let angle_k1 = two_pi * (k + 1.0) / n_sides;
    let vx = (1.0 - t) * cos(angle_k) + t * cos(angle_k1);
    let vy = (1.0 - t) * sin(angle_k) + t * sin(angle_k1);
    let cx = to_float(i) - to_float(width) * 0.5;
    let cy = to_float(j) - to_float(height) * 0.5;
    #{ z: complex(0.1 * (vx * scale + cx), 0.1 * (vy * scale + cy)), color: vcolor }
}
