// The Swift-Hohenberg equation: a field that selects one wavelength,
// drawing stripes or, with the quadratic term, hexagons.
//
// The Rhai twin of the built-in `swift_hohenberg` model
// (src/sim/models.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters, presets and working values, its stable time step, and the
// same field.
//
//   du/dt = r u - (q0^2 + lap)^2 u + g u^2 - u^3
//
// Two passes: the first writes w = (q0^2 + lap) u into `.y`, the second
// applies the operator again to w, so the biharmonic needs only the
// 5-point stencil. Channels: x = u, y = w, z = the step u last changed
// appreciably, w spare.

// The 5-point Laplacian of `.x` or `.y`, the cell's value `c`.

const PARAMS = #{
    wavelength: #{
        value: 16.0, min: 6.0, max: 64.0, caption: "Wavelength (cells)",
        hint: "The spacing the equation selects, in cells — it is 2π/q₀, and the measurement agrees with it to 3%. Larger wavelengths need proportionally more steps, because the pattern grows on a 1/r timescale and r falls as the fourth power of 1/wavelength.",
    },
    drive: #{
        value: 2.0, min: 0.05, max: 4.0, caption: "Drive (r / q₀⁴)",
        hint: "How hard the pattern is driven, in units of the band's own selectivity — the literal r is this times q₀⁴. Low values grow slowly and give a faint, very regular pattern; above about 4 the uniform mode grows nearly as fast as the pattern and the field phase-separates into blobs instead.",
    },
    asymmetry: #{
        value: 0.0, min: 0.0, max: 0.35, caption: "Asymmetry (g)",
        hint: "Breaks the symmetry between peaks and troughs. At 0 there is no reason to prefer either and the field makes a labyrinth of equal stripes; raising it pinches the stripes into spots. Past about 0.35 the quadratic term overwhelms the cubic and the field goes uniform — measured, so the slider stops below it.",
    },
};
const INFO = #{
    title: "Swift–Hohenberg",
    id: "swift_hohenberg",
    description: "The canonical pattern-forming equation: one wavelength is amplified and every other damped, giving a labyrinth with a definite spacing.",
    dt: 0.03,
    max_dt: 0.05,
    steps: 5000,
    presets: [
        #{
            name: "labyrinth",
            title: "Labyrinth",
            params: #{ wavelength: 16.0, drive: 2.0, asymmetry: 0.0 },
            steps: 5000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.2, offset: 0.5, wrap: 0.0 },
        },
        #{
            name: "spots",
            title: "Spots",
            params: #{ wavelength: 16.0, drive: 2.0, asymmetry: 0.25 },
            steps: 6000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.2, offset: 0.5, wrap: 0.0 },
        },
    ],
};

fn lap_x(c) {
    read(0, -1).x + read(0, 1).x + read(-1, 0).x + read(1, 0).x - 4.0 * c
}

fn lap_y(c) {
    read(0, -1).y + read(0, 1).y + read(-1, 0).y + read(1, 0).y - 4.0 * c
}

// The wavenumber the wavelength names, in cells.
fn q0(wavelength) {
    6.28318530718 / max(wavelength, 1.0)
}

fn cell_step(cell, p, ctx) {
    let q = q0(p.wavelength);
    let w = q * q * cell.x + lap_x(cell.x);
    vec4(cell.x, w, cell.z, 0.0)
}

fn cell_step2(cell, p, ctx) {
    let q = q0(p.wavelength);
    let q4 = q * q * q * q;
    let r = p.drive * q4;
    let dt = ctx.dt;
    let u = cell.x;
    let bi = q * q * cell.y + lap_y(cell.y);
    let nu = u + dt * (r * u - bi + p.asymmetry * u * u - u * u * u);
    let cl = clamp(nu, -10.0, 10.0);
    vec4(cl, cell.y, if abs(cl - u) > 0.00001 { ctx.step } else { cell.z }, 0.0)
}

// Small noise about zero.
fn cell_seed(inside, noise) {
    vec4((noise - 0.5) * 0.2, 0.0, 0.0, 0.0)
}

// Explicit Euler on the operator's stiffest mode, the Nyquist one, where
// the 5-point Laplacian's eigenvalue is -8.
fn max_dt(p) {
    let q = 6.2831853 / max(p.wavelength, 1.0);
    let q2 = q * q;
    let stiff = (8.0 - q2) * (8.0 - q2) - p.drive * q2 * q2;
    2.0 / max(stiff, 0.001)
}
