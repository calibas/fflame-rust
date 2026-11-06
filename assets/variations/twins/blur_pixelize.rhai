/// Snaps points to a grid of pixel-sized cells, then adds random offset
/// within each cell. Produces a mosaic effect.

// Blur Pixelize, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    size: #{
        value: 0.1, min: 1.0e-7, max: 10.0, free: true, exponential: false,
        caption: "Size",
        hint: "Pixel cell size. Smaller = finer grid; larger = chunkier pixels.",
    },
    scale: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "Scale",
        hint: "How much each point can jitter within its cell. 0 = points snap to cell centers; 1 = points scatter across the cell.",
    },
};

const INFO = #{ title: "Blur Pixelize", name: "rhai_blur_pixelize", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let size = p.size;
    let scale = p.scale;
    let inv_size = 1.0 / size;
    let x = to_float(floor(z.re * inv_size));
    let y = to_float(floor(z.im * inv_size));
    complex(size * (x + scale * (rand() - 0.5) + 0.5), size * (y + scale * (rand() - 0.5) + 0.5))
}
