// Line integral convolution: noise smeared along the field's contours
// (or its gradient), so the picture is the flow's texture.
//
// The Rhai twin of the built-in `lic` colouring (src/sim/colorings.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters and the same
// picture.
//
// From each cell a streamline walks both ways, a cell a step, up to
// `length` cells, summing a noise fixed per cell; the mean, stretched by
// `contrast`, is the palette's place.

// WGSL's `round`: half to even, as the built-in reads its length.

const PARAMS = #{
    channel: #{
        value: 0, choices: ["A / x", "B / y", "Age / z", "Spare / w"], caption: "Channel",
        hint: "Which channel's gradient gives the direction. For fingering, y (the pressure) points along the flow.",
    },
    direction: #{
        value: 0, choices: ["Contours", "Gradient"], caption: "Follow",
        hint: "Contours runs along lines of equal value — the shape of the pattern. Gradient runs across them, up the slope — the flow.",
    },
    length: #{
        value: 8.0, min: 1.0, max: 32.0, caption: "Length (cells)",
        hint: "How far each way the noise is smeared. Longer lines are smoother and cost proportionally more.",
    },
    contrast: #{
        value: 1.0, min: 0.1, max: 4.0, caption: "Contrast",
        hint: "Stretches the averaged noise about its mean. The average of many noise values is close to a half; this pulls it back out to the palette.",
    },
};
const INFO = #{
    title: "Flow lines (LIC)",
    id: "lic",
    description: "Noise smeared along the field's flow lines, so the streamlines themselves are the picture. Line integral convolution.",
};

fn round_even(x) {
    let r = floor(x + 0.5);
    if r - x == 0.5 && r % 2.0 != 0.0 { r - 1.0 } else { r }
}

// The noise at the cell (x, y): the framework's PCG of its index and the
// run's seed, fixed for the run.
fn lic_noise(x, y, w, lo, hi) {
    let h = pcg(pcg((y * w + x) ^ lo) ^ hi ^ 0x11c);
    to_float(h >> 8) * (1.0 / 16777216.0)
}

fn cell_color(cell, p, ctx) {
    let c = clamp(p.channel, 0, 3);
    let along = p.direction == 0;
    let len = to_int(round_even(clamp(p.length, 1.0, 32.0)));
    let px = to_int(ctx.cell.re);
    let py = to_int(ctx.cell.im);
    let w = ctx.width;
    let h = ctx.height;
    let acc = lic_noise(px, py, w, ctx.seed_lo, ctx.seed_hi);
    let n = 1.0;
    for leg in 0..2 {
        let pos_x = to_float(px) + 0.5;
        let pos_y = to_float(py) + 0.5;
        let prev_x = 0.0;
        let prev_y = 0.0;
        for i in 0..len {
            let qx = to_int(floor(pos_x));
            let qy = to_int(floor(pos_y));
            if qx < 0 || qy < 0 || qx >= w || qy >= h {
                break;
            }
            // The field's gradient at q, by central differences, turned
            // a quarter for the contours.
            let r = read(qx - px + 1, qy - py);
            let l = read(qx - px - 1, qy - py);
            let u = read(qx - px, qy - py + 1);
            let d = read(qx - px, qy - py - 1);
            let gx = r.x - l.x;
            let gy = u.x - d.x;
            if c == 1 {
                gx = r.y - l.y;
                gy = u.y - d.y;
            } else if c == 2 {
                gx = r.z - l.z;
                gy = u.z - d.z;
            } else if c == 3 {
                gx = r.w - l.w;
                gy = u.w - d.w;
            }
            let vx = 0.0;
            let vy = 0.0;
            let size = sqrt(gx * gx + gy * gy);
            if size >= 0.000000001 {
                vx = gx / size;
                vy = gy / size;
                if along {
                    let t = vx;
                    vx = -vy;
                    vy = t;
                }
            }
            if leg == 1 {
                vx = -vx;
                vy = -vy;
            }
            // The line keeps its heading: a direction's sign is free.
            if vx * prev_x + vy * prev_y < 0.0 {
                vx = -vx;
                vy = -vy;
            }
            if vx * vx + vy * vy < 0.5 {
                break;
            }
            prev_x = vx;
            prev_y = vy;
            pos_x = pos_x + vx;
            pos_y = pos_y + vy;
            let sx = to_int(floor(pos_x));
            let sy = to_int(floor(pos_y));
            if sx < 0 || sy < 0 || sx >= w || sy >= h {
                break;
            }
            acc = acc + lic_noise(sx, sy, w, ctx.seed_lo, ctx.seed_hi);
            n = n + 1.0;
        }
    }
    let mean = acc / n;
    let col = palette(clamp(0.5 + (mean - 0.5) * sqrt(n) * p.contrast, 0.0, 1.0));
    rgba(col.x, col.y, col.z, 1.0)
}
