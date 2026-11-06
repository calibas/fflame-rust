// Root Basin, the built-in colouring's twin: for a convergent formula
// over z^p - 1, which root the orbit landed on (the final angle,
// bucketed) shaded by convergence speed; the General key folds in log|z|
// for roots that share an angle.

const PARAMS = #{
    roots: #{
        value: 3.0, min: 2.0, max: 12.0,
        caption: "Root count",
        hint: "Number of basins to bucket the final angle into - match the formula's power.",
    },
    speed: #{
        value: 0.01, min: 0.0, max: 0.2,
        caption: "Speed shading",
        hint: "Palette offset per iteration of convergence time, shading within each basin.",
    },
    key: #{
        value: 0, choices: ["Angle buckets", "General"],
        caption: "Basin key",
        hint: "0: Angle buckets - the true basin index, but only when the roots are evenly spaced on a circle (z^p - 1). 1: General - folds in log|z| so roots that share an angle still separate; use it for every other function.",
    },
    key_scale: #{
        value: 0.25, min: -2.0, max: 2.0,
        caption: "Key scale",
        hint: "How strongly the General key weighs log|z| against the angle. Tune until neighbouring basins stop sharing a colour.",
    },
};

const INFO = #{ title: "Root Basin" };

// The shading by convergence time is added in one expression, as the
// built-in's `t + n * speed`, which the GPU may fuse: written apart it
// rounds once more, and a position on a whole number wraps.
fn finish(s, z, p, ctx) {
    let r2 = z.re * z.re + z.im * z.im;
    // The origin has no angle.
    if r2 > 1e-30 {
        let ang = fract(z.arg() / 6.28318530718 + 1.0);
        if p.key == 0 {
            // A converged orbit on a root of z^p - 1: the bucket is the
            // basin.
            let roots = min(max(p.roots, 2.0), 12.0);
            floor(ang * roots + 0.5) / roots + to_float(ctx.numiter) * p.speed
        } else if ctx.converged {
            let lr = 0.5 * ln(max(r2, 1e-30));
            fract(ang + lr * p.key_scale) + to_float(ctx.numiter) * p.speed
        } else {
            // No root reached: one flat colour of its own.
            0.0
        }
    } else {
        0.0 + to_float(ctx.numiter) * p.speed
    }
}
