// Direct Orbit Traps, the built-in colouring's twin (techmatt's
// `direct_trap` modes): no value, but every iterate that comes within the
// threshold of the shape takes a sample from the palette and composites
// it into the pixel, in the order the orbit made them -- the stack of
// every near miss. The palette is read in the loop, so a palette edit
// re-iterates.

const PARAMS = #{
    shape: #{
        value: 1, choices: ["Point", "Ring", "Cross", "Hypercross", "Diamond", "Box", "Astroid", "Lines"],
        caption: "Trap shape",
        hint: "The shape each iterate is measured against, at the origin: a point (beads), a ring of the given radius (overlapping scales), the axes' cross, the cross with its diagonals, diamond and square contours, a four-pointed astroid, or the real axis alone (horizontal bands).",
    },
    radius: #{
        value: 1.0, min: 0.01, max: 4.0,
        caption: "Ring radius",
        hint: "The ring's radius (the other shapes do not use it).",
    },
    threshold: #{
        value: 0.0597, min: 0.0, max: 4.0,
        caption: "Threshold",
        hint: "How near an iterate must come to paint. 0 takes the shape's own default, calibrated so every shape paints about the same share of a frame: point 0.6, ring 0.078, cross 0.1, hypercross 0.074, diamond 0.8, box 0.55, astroid 1.06, lines 0.127.",
    },
    opacity: #{
        value: 0.45, min: 0.0, max: 1.0,
        caption: "Opacity",
        hint: "How strongly each near miss paints, before its feathering by closeness. A Screen cross is held to 0.15, where it would otherwise blow out to white.",
    },
    blend: #{
        value: 2, choices: ["Normal", "Multiply", "Screen", "Overlay", "Min", "Add"],
        caption: "Blend",
        hint: "How each sample merges with the colour so far: Screen brightens (start from black), Multiply darkens (start from white), Overlay does both, Min clamps, Add lifts by the same amount everywhere, Normal replaces.",
    },
    order: #{
        value: 0, choices: ["Bottom up", "Top down"],
        caption: "Order",
        hint: "Bottom up: each near miss goes over the ones before it. Top down: the earliest stays on top.",
    },
    start: #{
        value: 0, choices: ["Black", "White"],
        caption: "Start",
        hint: "The colour the samples land on, where no near miss paints.",
    },
    transform: #{
        value: 0, choices: ["Linear", "Square root", "Log", "S-curve"],
        caption: "Curve",
        hint: "A curve on the closeness before it picks the sample and its feathering.",
    },
};

// techmatt's escape radius, 2^16, squared.
const INFO = #{ title: "Direct Orbit Traps", bailout: 4294967296.0 };

// The distance from an iterate to the shape, at the origin.
fn distance(shape, z, radius) {
    let re = abs(z.re);
    let im = abs(z.im);
    if shape == 0 {
        z.abs()
    } else if shape == 1 {
        abs(z.abs() - radius)
    } else if shape == 3 {
        let diagonal = min(abs(z.re - z.im), abs(z.re + z.im)) * 0.70710678;
        min(min(re, im), diagonal)
    } else if shape == 4 {
        re + im
    } else if shape == 5 {
        max(re, im)
    } else if shape == 6 {
        // (|x|^(2/3) + |y|^(2/3))^(3/2), the zeros written out rather
        // than handed to pow.
        let a = if re > 0.0 { pow(re, 2.0 / 3.0) } else { 0.0 };
        let b = if im > 0.0 { pow(im, 2.0 / 3.0) } else { 0.0 };
        if a + b > 0.0 { pow(a + b, 1.5) } else { 0.0 }
    } else if shape == 7 {
        im
    } else {
        min(re, im)
    }
}

// techmatt's per-shape thresholds, each calibrated to paint about the
// share of a frame the cross does at 0.1.
fn default_threshold(shape) {
    if shape == 0 { 0.60 } else if shape == 1 { 0.078 } else if shape == 3 { 0.074 } else if shape == 4 { 0.80 } else if shape == 5 { 0.55 } else if shape == 6 { 1.06 } else if shape == 7 { 0.127 } else { 0.10 }
}

// One part of the blend of `over` onto `under`.
fn blend_part(blend, under, over) {
    if blend == 0 {
        over
    } else if blend == 1 {
        under * over
    } else if blend == 3 {
        if under < 0.5 { 2.0 * under * over } else { 1.0 - 2.0 * (1.0 - under) * (1.0 - over) }
    } else if blend == 4 {
        min(under, over)
    } else if blend == 5 {
        min(under + over, 1.0)
    } else {
        1.0 - (1.0 - under) * (1.0 - over)
    }
}

// One part of the colour after a near miss: the sample blended over it
// (or under it, top down), mixed in by alpha.
fn painted(blend, top_down, col, sample, alpha) {
    let blended = if top_down { blend_part(blend, sample, col) } else { blend_part(blend, col, sample) };
    blended * alpha + col * (1.0 - alpha)
}

// The colour so far, in linear light: black or white to start.
fn start(z, p) {
    let w = if p.start == 1 { 1.0 } else { 0.0 };
    #{ r: w, g: w, b: w }
}

fn step(s, z, p) {
    let shape = p.shape;
    let blend = p.blend;
    let threshold = if p.threshold > 0.0 { p.threshold } else { default_threshold(shape) };
    let opacity = clamp(p.opacity, 0.0, 1.0);
    // A Screen cross held where it would blow out to white.
    if blend == 2 && shape == 2 {
        opacity = min(opacity, 0.15);
        threshold = min(threshold, 0.08);
    }
    threshold = max(threshold, 1e-12);
    let d = distance(shape, z, p.radius);
    // Not negated: a non-finite distance paints nothing.
    if d < threshold {
        let key = clamp(d / threshold, 0.0, 1.0);
        if p.transform == 1 {
            key = sqrt(key);
        } else if p.transform == 2 {
            key = log2(1.0 + key);
        } else if p.transform == 3 {
            key = key * key * (3.0 - 2.0 * key);
        }
        // The palette at the key, CLAMPED as techmatt's colormap lookup
        // is, decoded to linear light.
        let srgb = palette_table(palette_curve(key));
        let alpha = opacity * (1.0 - key);
        let top_down = p.order == 1;
        s.r = painted(blend, top_down, s.r, pow(max(srgb.x, 0.0), 2.2), alpha);
        s.g = painted(blend, top_down, s.g, pow(max(srgb.y, 0.0), 2.2), alpha);
        s.b = painted(blend, top_down, s.b, pow(max(srgb.z, 0.0), 2.2), alpha);
    }
    s
}

// The value the relief and auto contrast read: the colour's luminance.
fn finish(s, z, p) {
    s.r * 0.2126 + s.g * 0.7152 + s.b * 0.0722
}

fn color(s, z, p) {
    vec3(max(s.r, 0.0), max(s.g, 0.0), max(s.b, 0.0))
}
