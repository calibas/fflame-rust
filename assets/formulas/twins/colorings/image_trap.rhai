// Image Trap, the built-in colouring's twin (Ultra Fractal's
// `ColorTrapImage`, drawn as its Direct Orbit Traps draws a colour trap,
// with `TrapTransform` for the trap's position): each iterate, moved
// through the trap's position, reads the config's texture over (-1,-1)
// to (1,1) -- transparent outside it -- and merges into the colour so
// far with UF's FullMerge. The colour carries its alpha, the pixel's
// coverage; `step`, as UF's loop section, sees only the iterates that
// did not bail out.

const PARAMS = #{
    trap_re: #{
        value: 0.0, min: -4.0, max: 4.0,
        caption: "Trap center (re)",
        hint: "Where the image sits in the complex plane: its centre.",
    },
    trap_im: #{
        value: 0.0, min: -4.0, max: 4.0,
        caption: "Trap center (im)",
        hint: "Where the image sits in the complex plane: its centre.",
    },
    trap_scale: #{
        value: 1.0, min: 0.01, max: 10.0,
        caption: "Trap scale",
        hint: "The image's size. At 1 it spans -1 to 1; smaller shrinks it.",
    },
    trap_rotation: #{
        value: 0.0, min: -180.0, max: 180.0,
        caption: "Rotation",
        hint: "The image's angle, in degrees.",
    },
    trap_aspect: #{
        value: 1.0, min: 0.1, max: 10.0,
        caption: "Aspect ratio",
        hint: "Squeezes the image vertically above 1, stretches it below.",
    },
    trap_skew: #{
        value: 0.0, min: -80.0, max: 80.0,
        caption: "Skew",
        hint: "Slants the image's vertical axis, in degrees.",
    },
    proportions: #{
        value: 1, choices: ["Stretch to square", "Keep proportions"],
        caption: "Proportions",
        hint: "Keep the image's own proportions, or stretch it to a square.",
    },
    opacity: #{
        value: 0.2, min: 0.0, max: 1.0,
        caption: "Trap merge opacity",
        hint: "How strongly each iterate's colour merges. Low values stack many translucent copies; 1 lets one copy cover the rest.",
    },
    order: #{
        value: 0, choices: ["Bottom-up", "Top-down"],
        caption: "Trap merge order",
        hint: "Bottom-up merges each new iterate's colour on top of the ones before; top-down merges it underneath, so the earliest stays on top. Top-down leaves the colour partly transparent where iterates miss the image, and the background shows.",
    },
    merge: #{
        value: 0, choices: ["Normal", "Multiply", "Screen", "Darken", "Lighten", "Difference", "Addition", "Subtraction", "Red", "Green", "Blue"],
        caption: "Trap color merge",
        hint: "How an iterate's colour combines with the colour so far, as Ultra Fractal's layer merge modes do.",
    },
    base_r: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Base color red",
        hint: "The colour the iterates' colours merge onto.",
    },
    base_g: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Base color green",
        hint: "The colour the iterates' colours merge onto.",
    },
    base_b: #{
        value: 0.0, min: 0.0, max: 1.0,
        caption: "Base color blue",
        hint: "The colour the iterates' colours merge onto.",
    },
};

const INFO = #{ title: "Image Trap", skips_escaping: true };

// TrapTransform: (z - centre) * rotation / scale, then the aspect on the
// imaginary part, then the skew on the real part.
fn trap_position(z, p) {
    let r = p.trap_rotation * 0.017453292;
    let cr = cos(r);
    let sr = sin(r);
    let dx = z.re - p.trap_re;
    let dy = z.im - p.trap_im;
    let x = (dx * cr - dy * sr) / p.trap_scale;
    let y = (dx * sr + dy * cr) / p.trap_scale * p.trap_aspect;
    let k = p.trap_skew * 0.017453292;
    complex(x * cos(k) - y * sin(k), y)
}

// Image.getColor: the texture over (-1,-1)..(1,1), (-1,-1) its
// bottom-left corner, transparent outside; ImageWrapper.NormalizePixel
// first, for Keep proportions. The negated test lets a non-finite point
// miss.
fn trap_colour(q, p) {
    let size = texture_size();
    let x = q.re;
    let y = q.im;
    if p.proportions == 1 {
        if size.re > size.im {
            y = y * size.re / size.im;
        } else {
            x = x * size.im / size.re;
        }
    }
    if !(abs(x) <= 1.0 && abs(y) <= 1.0) {
        rgba(0.0, 0.0, 0.0, 0.0)
    } else {
        texture(complex((x + 1.0) * 0.5, (1.0 - y) * 0.5))
    }
}

// mergeX(bottom, top): UF's layer merge modes, the ones its help defines
// exactly, on the three parts at once.
fn merged(mode, br, bg, bb, tr, tg, tb) {
    if mode == 1 {
        vec3(br * tr, bg * tg, bb * tb)
    } else if mode == 2 {
        vec3(1.0 - (1.0 - br) * (1.0 - tr), 1.0 - (1.0 - bg) * (1.0 - tg), 1.0 - (1.0 - bb) * (1.0 - tb))
    } else if mode == 3 {
        vec3(min(br, tr), min(bg, tg), min(bb, tb))
    } else if mode == 4 {
        vec3(max(br, tr), max(bg, tg), max(bb, tb))
    } else if mode == 5 {
        vec3(abs(br - tr), abs(bg - tg), abs(bb - tb))
    } else if mode == 6 {
        vec3(min(br + tr, 1.0), min(bg + tg, 1.0), min(bb + tb, 1.0))
    } else if mode == 7 {
        vec3(max(br - tr, 0.0), max(bg - tg, 0.0), max(bb - tb, 0.0))
    } else if mode == 8 {
        vec3(tr, bg, bb)
    } else if mode == 9 {
        vec3(br, tg, bb)
    } else if mode == 10 {
        vec3(br, bg, tb)
    } else {
        vec3(tr, tg, tb)
    }
}

// ColorMerge.FullMerge: compose(b, blend(t, mergeX(b, t), alpha(b)), o)
// -- the merged colour (its alpha the top's) mixed into the top by the
// bottom's alpha, then over the bottom at the top's alpha times o,
// alphas counted.
fn full_merge(mode, br, bg, bb, ba, tr, tg, tb, ta, o) {
    let x = merged(mode, br, bg, bb, tr, tg, tb);
    let mr = tr + (x.x - tr) * ba;
    let mg = tg + (x.y - tg) * ba;
    let mb = tb + (x.z - tb) * ba;
    let ma = ta + (ta - ta) * ba;
    let at = ma * o;
    let a = at + ba * (1.0 - at);
    let under = ba * (1.0 - at);
    if a > 0.0 {
        let n = max(a, 1e-30);
        rgba((mr * at + br * under) / n, (mg * at + bg * under) / n, (mb * at + bb * under) / n, a)
    } else {
        rgba(0.0, 0.0, 0.0, a)
    }
}

// The base colour, opaque: UF's `rgb()`.
fn start(z, p) { #{ r: p.base_r, g: p.base_g, b: p.base_b, a: 1.0 } }

// Bottom-up puts each new colour on top; top-down underneath.
fn step(s, z, p) {
    let c = trap_colour(trap_position(z, p), p);
    let o = clamp(p.opacity, 0.0, 1.0);
    let top_down = p.order == 1;
    let bottom = if top_down { c } else { rgba(s.r, s.g, s.b, s.a) };
    let top = if top_down { rgba(s.r, s.g, s.b, s.a) } else { c };
    let m = full_merge(p.merge, bottom.r, bottom.g, bottom.b, bottom.a, top.r, top.g, top.b, top.a, o);
    s.r = m.r;
    s.g = m.g;
    s.b = m.b;
    s.a = m.a;
    s
}

// The value the relief and auto contrast read: the colour's luminance.
fn finish(s, z, p) {
    s.r * 0.2126 + s.g * 0.7152 + s.b * 0.0722
}

// UF's colour is in display values; the output is linear light, its
// alpha the coverage.
fn color(s, z, p) {
    rgba(pow(clamp(s.r, 0.0, 1.0), 2.2), pow(clamp(s.g, 0.0, 1.0), 2.2), pow(clamp(s.b, 0.0, 1.0), 2.2), s.a)
}
