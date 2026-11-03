// Two coupled Brusselators: a pair of Turing systems in one field,
// exchanging both species (Yang, Dolnik, Zhabotinsky and Epstein's
// superlattices come from such a coupling).
//
// The Rhai twin of the built-in `brusselator2` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its stable time step, and the same field.
//
// Channels: x = u1, y = v1 (the first system), z = u2, w = v2 (the
// second), on the 5-point Laplacian.

const PARAMS = #{
    a: #{
        value: 3.0, min: 0.5, max: 8.0, caption: "a",
        hint: "The Brusselator's feed. The fixed point of both layers is (a, b/a).",
    },
    b: #{
        value: 9.0, min: 1.0, max: 20.0, caption: "b",
        hint: "The Brusselator's control parameter. Turing patterns need b above 1 + a²·D_u/D_v for the layer; Hopf oscillations above 1 + a².",
    },
    du1: #{
        value: 1.85, min: 0.1, max: 100.0, caption: "Layer 1 D_u",
        hint: "Activator diffusion of layer 1, in cells² per unit time.",
    },
    dv1: #{
        value: 5.66, min: 0.1, max: 400.0, caption: "Layer 1 D_v",
        hint: "Inhibitor diffusion of layer 1. The ratio to D_u sets the layer's wavelength.",
    },
    du2: #{
        value: 50.6, min: 0.1, max: 100.0, caption: "Layer 2 D_u",
        hint: "Activator diffusion of layer 2.",
    },
    dv2: #{
        value: 186.0, min: 0.1, max: 400.0, caption: "Layer 2 D_v",
        hint: "Inhibitor diffusion of layer 2. The largest diffusion sets the time step: 0.25 / D_max on this stencil, 0.0013 at 186.",
    },
    coupling: #{
        value: 1, choices: ["Linear", "Cubic"], caption: "Coupling",
        hint: "Linear is diffusion between the layers, q·(u₂ − u₁): the strength hardly matters and the patterns superimpose. Cubic is an active middle layer, q·u₁·u₂·(u₂ − u₁): the strength selects the pattern.",
    },
    q: #{
        value: 0.15, min: 0.0, max: 1.0, caption: "Strength (q)",
        hint: "The coupling strength, the same for both morphogens. Under cubic coupling at the default parameters: 0.01–0.05 superimposed stripes, 0.09 spots with inner structure, 0.15 boats, 0.19 elongated boats, 0.27–0.29 long stripes.",
    },
};
const INFO = #{
    title: "Two-Layer Brusselator",
    id: "brusselator2",
    description: "Two Brusselator layers with different wavelengths, coupled linearly or cubically. Cubic coupling of the right strength makes patterns that are neither spots nor stripes nor their superposition — Kyttä, Kaski & Barrio's boats and beans.",
    dt: 0.001,
    max_dt: 0.005,
    steps: 200000,
    presets: [
        #{
            name: "boats",
            title: "Boats",
            params: #{ a: 3.0, b: 9.0, du1: 1.85, dv1: 5.66, du2: 50.6, dv2: 186.0, coupling: 1.0, q: 0.15 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.0, wrap: 0.0 },
        },
        #{
            name: "spots_inside",
            title: "Spots with inner structure",
            params: #{ a: 3.0, b: 9.0, du1: 1.85, dv1: 5.66, du2: 50.6, dv2: 186.0, coupling: 1.0, q: 0.09 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.0, wrap: 0.0 },
        },
        #{
            name: "superimposed_stripes",
            title: "Superimposed stripes",
            params: #{ a: 3.0, b: 9.0, du1: 1.85, dv1: 5.66, du2: 50.6, dv2: 186.0, coupling: 1.0, q: 0.03 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.0, wrap: 0.0 },
        },
        #{
            name: "long_stripes",
            title: "Long stripes",
            params: #{ a: 3.0, b: 9.0, du1: 1.85, dv1: 5.66, du2: 50.6, dv2: 186.0, coupling: 1.0, q: 0.27 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.0, wrap: 0.0 },
        },
        #{
            name: "superposition",
            title: "Superposition",
            params: #{ a: 4.5, b: 11.0, du1: 1.85, dv1: 16.66, du2: 25.741, dv2: 196.0, coupling: 1.0, q: 0.1 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.75, wrap: 0.0 },
        },
        #{
            name: "beans",
            title: "Beans",
            params: #{ a: 4.5, b: 11.0, du1: 7.5, dv1: 32.5, du2: 27.5, dv2: 121.5, coupling: 1.0, q: 0.01 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.75, wrap: 0.0 },
        },
        #{
            name: "linear_weak",
            title: "Linear, weak",
            params: #{ a: 3.0, b: 9.0, du1: 1.85, dv1: 5.66, du2: 50.6, dv2: 186.0, coupling: 0.0, q: 0.1 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.0, wrap: 0.0 },
        },
        #{
            name: "linear_strong",
            title: "Linear, strong",
            params: #{ a: 3.0, b: 9.0, du1: 1.85, dv1: 5.66, du2: 50.6, dv2: 186.0, coupling: 0.0, q: 1.0 },
            steps: 200000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: -1.0, wrap: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    let up = read(0, -1);
    let dn = read(0, 1);
    let lf = read(-1, 0);
    let rt = read(1, 0);
    let lap_u1 = up.x + dn.x + lf.x + rt.x - 4.0 * cell.x;
    let lap_v1 = up.y + dn.y + lf.y + rt.y - 4.0 * cell.y;
    let lap_u2 = up.z + dn.z + lf.z + rt.z - 4.0 * cell.z;
    let lap_v2 = up.w + dn.w + lf.w + rt.w - 4.0 * cell.w;
    let dt = ctx.dt;
    let u1 = cell.x;
    let v1 = cell.y;
    let u2 = cell.z;
    let v2 = cell.w;
    let f1 = p.a - (p.b + 1.0) * u1 + u1 * u1 * v1;
    let g1 = p.b * u1 - u1 * u1 * v1;
    let f2 = p.a - (p.b + 1.0) * u2 + u2 * u2 * v2;
    let g2 = p.b * u2 - u2 * u2 * v2;
    // The exchange: linear, or cubic (each species' product).
    let cu = p.q * (u2 - u1);
    let cv = p.q * (v2 - v1);
    if p.coupling == 1 {
        cu = cu * u1 * u2;
        cv = cv * v1 * v2;
    }
    vec4(
        max(u1 + dt * (p.du1 * lap_u1 + (f1 + cu)), 0.0),
        max(v1 + dt * (p.dv1 * lap_v1 + (g1 + cv)), 0.0),
        max(u2 + dt * (p.du2 * lap_u2 + (f2 - cu)), 0.0),
        max(v2 + dt * (p.dv2 * lap_v2 + (g2 - cv)), 0.0),
    )
}

// Both systems at the fixed point (a, b / a), each species with a draw
// of its own.
fn cell_seed(inside, noise, p) {
    let r = p.b / max(p.a, 0.001);
    vec4(
        max(p.a + (rand(0x91) - 0.5) * 0.1, 0.0),
        max(r + (rand(0x92) - 0.5) * 0.1, 0.0),
        max(p.a + (rand(0x93) - 0.5) * 0.1, 0.0),
        max(r + (rand(0x94) - 0.5) * 0.1, 0.0),
    )
}

// Explicit Euler on the 5-point Laplacian: dt D < 1/4 for the fastest
// species.
fn max_dt(p) {
    let dmax = max(max(max(max(0.0, p.du1), p.dv1), p.du2), p.dv2);
    0.25 / max(dmax, 0.001)
}
