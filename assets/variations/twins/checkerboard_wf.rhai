/// Textured checker-grid blur — picks a random (u, v) on a unit
/// square then maps it to an axis-aligned plane (like [`PLANE_WF`]),
/// with the color and Z-displacement of the resulting point read
/// from a checkerboard pattern of size `checker_size`. Even cells
/// take `checker_color1` and (optionally) `displ_amount` of Z
/// thickness; odd cells take `checker_color2` and zero Z. When
/// `with_sides = 1` the variation occasionally snaps one coordinate
/// to a grid line and uses `side_color` instead — these are the
/// "side walls" of the checkerboard tiles, painting the vertical
/// faces of each raised cell. Side-frequency scales automatically
/// with `displ_amount` (precomputed `_side_prob = 4d / (1 + 4d)`).
///
/// Stubbed vs the JWildfire reference: no SupportsGPU code path
/// (we emit our own WGSL), no displacement-map input (the
/// `displ_amount` here is just a constant z-thickness, not a sampled
/// map).
///
/// # Authors
/// - Andreas Maschke

// Checkerboard WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    position: #{
        value: 3.0, min: -100.0, max: 100.0, free: true,
        caption: "Position",
        hint: "Fixed coordinate along the axis the plane is perpendicular to. Sets the offset of the whole checkerboard along the axis it's mapped to.",
    },
    size: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "Size",
        hint: "Plane edge length. The two free axes get `(u − 0.5) × size` / `(v − 0.5) × size` and the displaced axis is `(z × size + position)`.",
    },
    axis: #{
        value: 2, choices: ["XY (fix Z)", "YZ (fix X)", "ZX (fix Y)"],
        caption: "Axis",
        hint: "Which two axes form the plane; the third is the displaced / fixed axis. Default ZX matches JWildfire's default.",
    },
    checker_size: #{
        value: 0.1, min: 0.001, max: 1.0, free: true,
        caption: "Checker Size",
        hint: "Side length of one checker square in (u, v) coordinates. Smaller values give finer grids; the init step computes `_max_checks = floor(1 / checker_size)` so a smaller checker_size also means more potential side-wall lines.",
    },
    displ_amount: #{
        value: 0.05, min: -1.0, max: 1.0, free: true,
        caption: "Displacement Amount",
        hint: "Z-thickness of the raised (even-cell) checkers. 0 disables the displacement entirely (and forces `with_sides` off — flat checkerboard).",
    },
    checker_color1: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Checker Color 1",
        hint: "Palette position written to `vc` for points landing on even checker cells.",
    },
    checker_color2: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Checker Color 2",
        hint: "Palette position written to `vc` for points landing on odd checker cells.",
    },
    side_color: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Side Color",
        hint: "Palette position written to `vc` for side-wall points (when `with_sides = 1` and a grid line is sampled).",
    },
    with_sides: #{
        value: true,
        caption: "With Sides",
        hint: "When on (and `displ_amount` is non-zero), occasionally snap one coordinate to a grid line and treat it as a side wall — paints the vertical faces of raised cells. Frequency scales with `displ_amount`.",
    },
};

const INFO = #{ title: "Checkerboard WF", name: "rhai_checkerboard_wf", category: "full_3d", phase: "any" };

fn prepare(p) {
    let out = array(2, 0.0);
    let displ_amount = p.displ_amount;
    let checker_size = p.checker_size;
    let side_area = 4.0 * displ_amount;
    out[0] = side_area / (1.0 + side_area);
    let safe_size = (if abs(checker_size) < 1.0e-30 { 1.0e-30 } else { checker_size });
    let max_checks = to_float(floor(1.0 / safe_size));
    if (max_checks * safe_size >= 1.0) {
        max_checks = max_checks - 1.0;
    }
    out[1] = max_checks;
    #{ side_prob: out[0], max_checks: out[1] }
}

fn variation(w, p, ctx) {
    let vcolor = ctx.color;
    let position = p.position;
    let size = p.size;
    let axis = to_int(to_float(p.axis));
    let checker_size = p.checker_size;
    let displ_amount = p.displ_amount;
    let checker_color1 = p.checker_color1;
    let checker_color2 = p.checker_color2;
    let side_color = p.side_color;
    let with_sides = to_int(to_float(p.with_sides));
    let side_prob = p.side_prob;
    let max_checks = p.max_checks;
    let safe_checker = (if abs(checker_size) < 1.0e-30 { 1.0e-30 } else { checker_size });
    let x = 0.0;
    let y = 0.0;
    let z = 0.0;
    let color = 0.0;
    let use_sides = with_sides != 0 && abs(displ_amount) >= 1.0e-6 && max_checks > 0.0 && rand() < side_prob;
    if (use_sides) {
        color = side_color;
        let max_int = max_checks + 1.0;
        if (rand() < 0.5) {
            x = to_float(floor(rand() * max_int)) * checker_size;
            y = rand();
        } else {
            x = rand();
            y = to_float(floor(rand() * max_int)) * checker_size;
        }
        z = displ_amount * rand();
    } else {
        x = rand();
        y = rand();
        let cell = to_float(floor(x / safe_checker)) + to_float(floor(y / safe_checker));
        let is_even = (cell - 2.0 * to_float(floor(cell * 0.5))) < 1.0;
        z = (if is_even { displ_amount } else { 0.0 });
        color = (if is_even { checker_color1 } else { checker_color2 });
    }
    vcolor = clamp(color, 0.0, 1.0);
    x = (x - 0.5) * size;
    y = (y - 0.5) * size;
    z = z * size + position;
    if (axis == 0) {
        return #{ z: complex(x, y), color: vcolor };
    }
    if (axis == 1) {
        return #{ z: complex(z, x), color: vcolor };
    }
    #{ z: complex(y, z), color: vcolor }
}

fn variation3(v, p, ctx) {
    let vcolor = ctx.color;
    let position = p.position;
    let size = p.size;
    let axis = to_int(to_float(p.axis));
    let checker_size = p.checker_size;
    let displ_amount = p.displ_amount;
    let checker_color1 = p.checker_color1;
    let checker_color2 = p.checker_color2;
    let side_color = p.side_color;
    let with_sides = to_int(to_float(p.with_sides));
    let side_prob = p.side_prob;
    let max_checks = p.max_checks;
    let safe_checker = (if abs(checker_size) < 1.0e-30 { 1.0e-30 } else { checker_size });
    let x = 0.0;
    let y = 0.0;
    let z = 0.0;
    let color = 0.0;
    let use_sides = with_sides != 0 && abs(displ_amount) >= 1.0e-6 && max_checks > 0.0 && rand() < side_prob;
    if (use_sides) {
        color = side_color;
        let max_int = max_checks + 1.0;
        if (rand() < 0.5) {
            x = to_float(floor(rand() * max_int)) * checker_size;
            y = rand();
        } else {
            x = rand();
            y = to_float(floor(rand() * max_int)) * checker_size;
        }
        z = displ_amount * rand();
    } else {
        x = rand();
        y = rand();
        let cell = to_float(floor(x / safe_checker)) + to_float(floor(y / safe_checker));
        let is_even = (cell - 2.0 * to_float(floor(cell * 0.5))) < 1.0;
        z = (if is_even { displ_amount } else { 0.0 });
        color = (if is_even { checker_color1 } else { checker_color2 });
    }
    vcolor = clamp(color, 0.0, 1.0);
    x = (x - 0.5) * size;
    y = (y - 0.5) * size;
    z = z * size + position;
    if (axis == 0) {
        return #{ z: vec3(x, y, z), color: vcolor };
    }
    if (axis == 1) {
        return #{ z: vec3(z, x, y), color: vcolor };
    }
    #{ z: vec3(y, z, x), color: vcolor }
}
