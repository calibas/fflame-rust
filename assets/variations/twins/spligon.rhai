/// Polygonal spike-tiler — snaps the input angle to the nearest of `sides`
/// polygon spokes (with a per-spoke offset from `i`), then adds a unit-
/// length spike at that angle to the input. Creates a tiling of spikes
/// radiating at regular angles.
///
/// # Authors
/// - DarkBeam

// Spligon, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sides: #{
        value: 3.0, min: 1.0, max: 50.0, free: true,
        caption: "Sides",
        hint: "Number of polygon spokes.",
    },
    i: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "I",
        hint: "Spike angular offset — rotates the spike direction within its spoke sector.",
    },
};

const INFO = #{ title: "Spligon", name: "rhai_spligon", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let sides = p.sides;
    let i_val = p.i;
    let two_pi = 6.28318530717959;
    let pi = 3.14159265358979;
    let safe_sides = (if sides == 0.0 { 1.0e-30 } else { sides });
    let th = safe_sides / two_pi;
    #{ th: th, thi: 1.0 / th, j: pi * i_val / (-2.0 * safe_sides) }
}

fn variation(z, p) {
    let th = p.th;
    let thi = p.thi;
    let j = p.j;
    let t = thi * to_float(floor(atan2(z.re, z.im) * th)) + j;
    complex(z.re + cos(t), z.im + sin(t))
}
