// Distance Field: the rendered picture as a seed mask, jump-flooded into
// the distance to its nearest bright pixel, then shaded -- a glow, contour
// bands, or each pixel filled with its nearest seed's colour. Every
// distance-estimator look, for any flame, no invertibility required.
//
// The built-in `distance_field` effect (docs/main/EFFECTS.md, Rhai effects):
// the app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/distance_field.wgsl,
// whose pixels, parameters and order it keeps, so a config cannot tell
// which it got. A multi-pass effect (docs/projects/multipass-effects.md):
// a seed pass, ~log2(the larger side) flood passes, and the composite.

const PARAMS = #{
    threshold: #{ value: 0.15, min: 0.0, max: 1.0, caption: "Seed Threshold", hint: "Pixels brighter than this seed the distance field." },
    spread: #{ value: 0.25, min: 0.01, max: 1.0, caption: "Spread", hint: "Field range as a fraction of the image's larger dimension." },
    mode: #{ value: 0, caption: "Mode", hint: "0 = glow, 1 = contour bands, 2 = nearest-color fill.", choices: ["Glow", "Contours", "Nearest fill"] },
    intensity: #{ value: 1.0, min: 0.0, max: 2.0, caption: "Intensity" },
    band_count: #{ value: 12.0, min: 1.0, max: 64.0, caption: "Band Count", hint: "Contour rings across the range (mode 1)." },
};
const INFO = #{
    title: "Distance Field",
    description: "The rendered picture as a seed mask, jump-flooded into the distance to its nearest bright pixel, then shaded -- a glow, contour bands, or each pixel filled with its nearest seed's colour. Every distance-estimator look, for any flame, no invertibility required.",
    id: "distance_field",
    buffers: #{ coords: #{} },
    passes: [
        #{ run: "seed", to: "coords" },
        #{ run: "flood", to: "coords", repeat: "log2" },
        #{ run: "effect" },
    ],
};

fn luminance(c) {
    dot(c.rgb, vec3(0.2126, 0.7152, 0.0722))
}

// The shader's `smoothstep`, spelled out.
fn smooth_step(lo, hi, x) {
    let t = clamp((x - lo) / (hi - lo), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// A pixel above the threshold seeds the field with its own position;
// every other carries the (-1, -1) sentinel.
fn seed(c, p, ctx) {
    let here = input_at(ctx.xy.re, ctx.xy.im);
    if luminance(here) > p.threshold {
        rgba(ctx.xy.re, ctx.xy.im, 0.0, 1.0)
    } else {
        rgba(-1.0, -1.0, 0.0, 1.0)
    }
}

// The jump flood: of the seeds recorded `ctx.step` pixels away in each
// direction and here, the nearest.
fn flood(c, p, ctx) {
    let x = to_int(ctx.xy.re);
    let y = to_int(ctx.xy.im);
    let best = complex(-1.0, -1.0);
    let best_d = 1.0e30;
    for dy in -1..2 {
        for dx in -1..2 {
            let qx = x + dx * ctx.step;
            let qy = y + dy * ctx.step;
            if qx >= 0 && qy >= 0 && qx < ctx.width && qy < ctx.height {
                let s = buffer_at("coords", qx, qy);
                if s.r >= 0.0 {
                    let d = (complex(s.r, s.g) - ctx.xy).abs();
                    if d < best_d {
                        best_d = d;
                        best = complex(s.r, s.g);
                    }
                }
            }
        }
    }
    rgba(best.re, best.im, 0.0, 1.0)
}

// The field shaded over the image, by the colour at the nearest seed.
fn effect(c, p, ctx) {
    let x = to_int(ctx.xy.re);
    let y = to_int(ctx.xy.im);
    let base = input_at(x, y);
    let s = buffer_at("coords", x, y);
    // No seed reached this pixel (an empty image): left alone.
    let reached = s.r >= 0.0;
    let at = complex(s.r, s.g);
    let range_px = max(p.spread, 0.01) * to_float(max(ctx.width, ctx.height));
    let d = (at - ctx.xy).abs();
    let t = clamp(d / range_px, 0.0, 1.0);
    let seed_color = input_at(s.r, s.g).rgb;
    let off = if d > 0.5 { 1.0 } else { 0.0 };
    let shaded = if p.mode == 0 {
        // Glow: the seed's colour, falling off exponentially.
        base.rgb + seed_color * exp(-4.0 * t) * off * p.intensity
    } else if p.mode == 1 {
        // Contour bands: rings of equal distance in the seed's colour.
        let phase = fract(t * max(p.band_count, 1.0));
        let ring = smooth_step(0.0, 0.15, phase) * (1.0 - smooth_step(0.35, 0.5, phase));
        base.rgb + seed_color * ring * (1.0 - t) * p.intensity * off
    } else {
        // Nearest fill: each pixel its nearest seed's colour, faded by
        // distance (a Voronoi-style flat).
        let fill = mix(base.rgb, seed_color, clamp(p.intensity, 0.0, 1.0) * (1.0 - t));
        if d <= 0.5 { base.rgb } else { fill }
    };
    if reached { rgba(shaded, base.a) } else { base }
}
