/// Quarter-arc Truchet tile pattern. Each cell in a `tiles_per_row ×
/// tiles_per_column` grid hosts a randomly-tilted quarter-arc, and the
/// variation samples points uniformly inside each cell's arc band
/// (width controlled by `thickness`). Produces interlocking
/// curve-and-corner tile patterns reminiscent of an Escher-style maze.
///
/// # Authors
/// - Jesus Sosa

// Arctruchet, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 10000, min: 0, max: 100000, free: true,
        caption: "Seed",
        hint: "PRNG seed feeding the per-cell tilt hash. JWildfire seeds a Java `Random` at init to build a static per-cell tilt array; on GPU we hash `(i, j, seed)` per call instead. Visually equivalent random Truchet pattern, not bit-identical per-cell to JWildfire's output. Same range as JWF.",
    },
    thickness: #{
        value: 0.025, min: 0.0, max: 1.0, free: true,
        caption: "Thickness",
        hint: "Arc band thickness in cell units (`radius = 0.25` is hardcoded). Wider = chunkier arcs. JWildfire clamps to [0, 1]; we follow suit at runtime.",
    },
    tiles_per_row: #{
        value: 10, min: 1, max: 100, free: true,
        caption: "Tiles Per Row",
        hint: "Grid columns (matches JWF's `TilesPerRow` label). Higher = denser pattern. **GPU-clamped to [1, 100]** matching JWF.",
    },
    tiles_per_column: #{
        value: 10, min: 1, max: 100, free: true,
        caption: "Tiles Per Column",
        hint: "Grid rows. **GPU-clamped to [1, 100]** matching JWF.",
    },
};

const INFO = #{ title: "Arctruchet", name: "rhai_arctruchet", category: "plugin", phase: "any" };

fn arctruchet_cell_tilt(i, j, seed) {
    let h = i * u32(374761393) + j * u32(668265263) + seed * u32(1274126177);
    h = (h ^ (h >> u32(13))) * u32(1274126177);
    h = h ^ (h >> u32(16));
    h % u32(4)
}

fn variation(z, p) {
    let seed_f = to_float(p.seed);
    let thickness = clamp(p.thickness, 0.0, 1.0);
    let tpr_f = clamp(to_float(p.tiles_per_row), 1.0, 100.0);
    let tpc_f = clamp(to_float(p.tiles_per_column), 1.0, 100.0);
    let tpr = u32(tpr_f);
    let tpc = u32(tpc_f);
    let radius = 0.25;
    let tile_size = 0.5;
    let i = u32(rand() * tpr_f);
    let j = u32(rand() * tpc_f);
    let cx = to_float(i) * tile_size + tile_size * 0.5;
    let cy = to_float(j) * tile_size + tile_size * 0.5;
    let tilt_idx = arctruchet_cell_tilt(i, j, u32(seed_f));
    let ang = to_float(tilt_idx) * 1.57079632679;
    let phi1 = (if rand() < 0.5 { 0.0 } else { 180.0 });
    let phi2 = phi1 + 90.0;
    let radio = (if phi1 == 0.0 { radius } else { -radius });
    let phi10 = phi1 * 0.01745329251;
    let phi20 = phi2 * 0.01745329251;
    let delta = phi20 - phi10;
    let denom = radius + thickness;
    let gamma = thickness * (2.0 * radius + thickness) / max(1.0e-32, denom);
    let r = radius + thickness - gamma * rand();
    let phi = phi10 + delta * rand();
    let xp = r * cos(phi);
    let yp = r * sin(phi);
    let ca = cos(ang);
    let sa = sin(ang);
    let prx = xp * ca + yp * sa;
    let pry = -xp * sa + yp * ca;
    let dx = radio * ca + radio * sa;
    let dy = -radio * sa + radio * ca;
    let local_x = prx - dx;
    let local_y = pry - dy;
    let half_w = tile_size * to_float(tpr) * 0.5;
    let half_h = tile_size * to_float(tpc) * 0.5;
    complex(local_x + cx - half_w, local_y + cy - half_h)
}
