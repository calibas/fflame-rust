/// Random heart-shape blur — picks a uniform random point in `[-2, 2]²` and
/// runs it through the `xheart` Möbius-style heart warp (rotated by
/// `angle`, with Y-stretch controlled by `ratio`). Produces a scattered
/// heart-silhouette splat that's independent of the iteration's input
/// position.

// X-Heart Blur WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Angle",
        hint: "Rotation angle of the heart shape (scaled by π/8 internally and offset from π/4).",
    },
    ratio: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Ratio",
        hint: "Y-axis stretch factor (added to a base of 6 to control heart roundness, same as `xheart`).",
    },
};

const INFO = #{ title: "X-Heart Blur WF", name: "rhai_xheart_blur_wf", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi_4 = 0.7853981633974483;
    let ang = pi_4 + 0.5 * pi_4 * p.angle;
    #{ sina: sin(ang), cosa: cos(ang), rat: 6.0 + 2.0 * p.ratio }
}

fn variation(z, p) {
    let sina = p.sina;
    let cosa = p.cosa;
    let rat = p.rat;
    let dx = 2.0 - rand() * 4.0;
    let dy = 2.0 - rand() * 4.0;
    let r2_4 = dx * dx + dy * dy + 4.0;
    if (abs(r2_4) < 1.0e-30) {
        r2_4 = 1.0;
    }
    let bx = 4.0 / r2_4;
    let by = rat / r2_4;
    let x = cosa * (bx * dx) - sina * (by * dy);
    let y = sina * (bx * dx) + cosa * (by * dy);
    if (x > 0.0) {
        return complex(x, y);
    }
    complex(x, -y)
}
