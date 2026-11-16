// Bloom: the bright parts of the picture glow, near and wide, as light
// does through a lens.
//
// A built-in effect written in Rhai, with no WGSL behind it
// (docs/main/EFFECTS.md, Rhai effects): the app runs this file -- on
// desktop the copy beside the app, so an edit shows on the next start.
// A multi-pass effect over two pyramids (docs/projects/multipass-effects.md):
// the bright parts thresholded and halved, level by level, down to a
// sixty-fourth of the picture's size; then back up, each level its own
// glow plus the wider ones below it; then added over the picture.
// Each level costs a few taps a pixel however wide the glow, where one
// blur that wide would cost thousands.

const PARAMS = #{
    threshold: #{ value: 0.45, min: 0.0, max: 1.0, caption: "Threshold", hint: "How bright a pixel is before it glows." },
    softness: #{ value: 0.3, min: 0.0, max: 1.0, caption: "Softness", hint: "How gently the glow starts around the threshold: 0 a hard cut." },
    intensity: #{ value: 1.5, min: 0.0, max: 4.0, caption: "Intensity" },
    spread: #{ value: 0.7, min: 0.0, max: 1.0, caption: "Spread", hint: "How much each wider glow adds: 0 a tight halo, 1 a broad haze." },
    tint: #{ value: rgba(1.0, 1.0, 1.0, 1.0), caption: "Tint" },
};
const INFO = #{
    title: "Bloom",
    description: "The bright parts of the picture glow, near and wide, as light does through a lens.",
    id: "bloom",
    buffers: #{ down: #{ scale: 0.5, levels: 7 }, up: #{ scale: 0.5, levels: 7 } },
    passes: [
        #{ run: "down", to: "down", repeat: "levels" },
        #{ run: "up", to: "up", repeat: "levels", order: "up" },
        #{ run: "effect" },
    ],
};

// What of a colour glows: its brightest channel past the threshold, eased
// in over the softness (a quadratic knee), the colour kept.
fn bright(c, threshold, softness) {
    let l = max(c.r, max(c.g, c.b));
    let knee = max(threshold * softness, 0.00001);
    let soft = clamp(l - threshold + knee, 0.0, 2.0 * knee);
    let w = max(soft * soft / (4.0 * knee), l - threshold) / max(l, 0.00001);
    c * w
}

// Down: level 0 the picture's bright parts, each later level the one
// before halved -- the centre and four corners a half texel away, each
// corner a 2 x 2 average by the filtered read (the dual filter).
fn down(c, p, ctx) {
    let dx = 0.5 / to_float(ctx.width);
    let dy = 0.5 / to_float(ctx.height);
    let a = complex(-dx, -dy);
    let b = complex(dx, -dy);
    let d = complex(-dx, dy);
    let e = complex(dx, dy);
    if ctx.level == 0 {
        let s = bright(input(ctx.uv + a), p.threshold, p.softness)
            + bright(input(ctx.uv + b), p.threshold, p.softness)
            + bright(input(ctx.uv + d), p.threshold, p.softness)
            + bright(input(ctx.uv + e), p.threshold, p.softness);
        s * 0.25
    } else {
        let s = buffer("down", ctx.uv) * 4.0
            + buffer("down", ctx.uv + a)
            + buffer("down", ctx.uv + b)
            + buffer("down", ctx.uv + d)
            + buffer("down", ctx.uv + e);
        s * 0.125
    }
}

// Up, coarsest first: each level its own glow plus the wider ones below
// it -- drawn up by a 3 x 3 tent -- weighed by the spread: the sum of
// blurs, each wider than the last, that reads as light.
fn up(c, p, ctx) {
    let here = buffer("down", ctx.uv);
    if ctx.level == ctx.levels - 1 {
        here
    } else {
        let dx = 1.0 / to_float(ctx.width);
        let dy = 1.0 / to_float(ctx.height);
        let tent = buffer("up", ctx.uv) * 4.0
            + (buffer("up", ctx.uv + complex(-dx, 0.0)) + buffer("up", ctx.uv + complex(dx, 0.0))
                + buffer("up", ctx.uv + complex(0.0, -dy)) + buffer("up", ctx.uv + complex(0.0, dy))) * 2.0
            + buffer("up", ctx.uv + complex(-dx, -dy)) + buffer("up", ctx.uv + complex(dx, -dy))
            + buffer("up", ctx.uv + complex(-dx, dy)) + buffer("up", ctx.uv + complex(dx, dy));
        here + tent * (0.0625 * p.spread)
    }
}

fn effect(c, p, ctx) {
    let glow = buffer("up", ctx.uv);
    rgba(clamp(c.rgb + glow.rgb * p.tint.rgb * p.intensity, 0.0, 1.0), c.a)
}
