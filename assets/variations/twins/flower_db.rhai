/// Flower with stem — emits a flower-shaped XY pattern (radius modulated by
/// `|spread + sin(petals·t)| · cos(petal_split·petals·t)`) plus a stem Z
/// that grows downward with thickness controlled by `stem_thickness`.
/// Optionally folds the petal Z upward outside a radius, and caps the stem
/// at a maximum length.
///
/// # Authors
/// - CozyG
/// - DarkBeam

// Flower DB, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    petals: #{
        value: 6.0, min: -50.0, max: 50.0, free: true,
        caption: "Petals",
        hint: "Number of petal lobes (angular frequency of the sin modulator).",
    },
    petal_split: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Petal Split",
        hint: "Frequency multiplier on the inner cosine that creates per-petal sub-divisions.",
    },
    petal_spread: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Petal Spread",
        hint: "Constant offset added to the sin modulator. Controls how filled the petals appear.",
    },
    stem_thickness: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Stem Thickness",
        hint: "Magnitude of the Z output. Negative values point the stem downward.",
    },
    stem_length: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Stem Length",
        hint: "Maximum stem length. 0 disables the cap (stem extends without bound).",
    },
    petal_fold_strength: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Petal Fold Strength",
        hint: "Z-axis fold strength applied to petals outside `petal_fold_radius`.",
    },
    petal_fold_radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Petal Fold Radius",
        hint: "Radial threshold beyond which the petal fold kicks in.",
    },
};

const INFO = #{ title: "Flower DB", name: "rhai_flower_db", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let petals = p.petals;
    let petal_split = p.petal_split;
    let petal_spread = p.petal_spread;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let r = w * sqrt(z.re * z.re + z.im * z.im);
    let t = atan2(z.im, z.re);
    r = r * abs((petal_spread + sin(petals * t)) * cos(petal_split * petals * t));
    let fx = sin(t) * r;
    let fy = cos(t) * r;
    complex(fx * inv_w, fy * inv_w)
}

fn variation3(v, p, ctx) {
    let petals = p.petals;
    let petal_split = p.petal_split;
    let petal_spread = p.petal_spread;
    let stem_thickness = p.stem_thickness;
    let stem_length = p.stem_length;
    let petal_fold_strength = p.petal_fold_strength;
    let petal_fold_radius = p.petal_fold_radius;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let r = w * sqrt(v.x * v.x + v.y * v.y);
    let t = atan2(v.y, v.x);
    r = r * abs((petal_spread + sin(petals * t)) * cos(petal_split * petals * t));
    let safe_r = (if abs(r) < 1.0e-30 { 1.0e-30 } else { r });
    let fx = sin(t) * r;
    let fy = cos(t) * r;
    let fz = -stem_thickness * (2.0 / safe_r - 1.0);
    let rnew = sqrt(fx * fx + fy * fy);
    if (rnew > petal_fold_radius) {
        fz = fz + (rnew - petal_fold_radius) * petal_fold_strength;
    }
    if (stem_length != 0.0 && fz <= -stem_length) {
        fz = -stem_length;
    }
    vec3(fx * inv_w, fy * inv_w, fz * inv_w)
}
