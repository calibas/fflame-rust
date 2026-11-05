// Visions of Chaos Traps, the built-in colouring's twin (Softology's
// orbit-trap shaders): the orbit stops at the FIRST iterate a trap
// catches, and a caught pixel is grey by how close that iterate came;
// one that escaped uncaught takes the CPM smooth colouring through a
// 256-entry palette indexed mod 255; the interior is black.

const PARAMS = #{
    shape: #{
        value: 0, choices: ["Circles", "Crosses", "Rings", "Squares", "Stalks"],
        caption: "Trap",
        hint: "Which of Visions of Chaos's orbit traps: circles, crosses, rings, squares or stalks.",
    },
    trap_x: #{
        value: 0.0, min: -4.0, max: 4.0,
        caption: "Trap X",
        hint: "The trap's centre, as Visions of Chaos names it. Circles and rings read it with X and Y swapped, as the original does, so the default puts them at (0.5, 0) and squares at (0, 0.5). Crosses and stalks do not use it.",
    },
    trap_y: #{
        value: 0.5, min: -4.0, max: 4.0,
        caption: "Trap Y",
        hint: "The trap's centre, as Visions of Chaos names it (see Trap X).",
    },
    circle_size: #{
        value: 0.5, min: 0.001, max: 4.0,
        caption: "Circle radius",
        hint: "Circles: how far from the centre an iterate is caught.",
    },
    cross_size: #{
        value: 0.05, min: 0.001, max: 4.0,
        caption: "Cross width",
        hint: "Crosses: how near an axis an iterate is caught.",
    },
    ring_min: #{
        value: 0.4, min: 0.0, max: 4.0,
        caption: "Ring inner radius",
        hint: "Rings: the inner edge of the band that catches.",
    },
    ring_max: #{
        value: 0.5, min: 0.001, max: 4.0,
        caption: "Ring outer radius",
        hint: "Rings: the outer edge of the band that catches.",
    },
    square_size: #{
        value: 0.4, min: 0.001, max: 4.0,
        caption: "Square half-size",
        hint: "Squares: half the side of the square that catches.",
    },
    stalk_radius: #{
        value: 0.05, min: 0.001, max: 4.0,
        caption: "Stalk width",
        hint: "Stalks: how near |z| must come to |c| to be caught.",
    },
};

const INFO = #{ title: "Visions of Chaos Traps" };

// caught (1) or not, the grey at the catch, the loop index of this step
// (0 for z1).
fn start(z, p) { #{ caught: 0.0, grey: 0.0, i: 0.0 } }

fn step(s, z, p, ctx) {
    let i = s.i;
    s.i = s.i + 1.0;
    // Caught already: frozen, as the original stops there. The escaping
    // iterate is never tested: the original breaks before it.
    if !(s.caught > 0.5 || z.norm_sqr() > ctx.bailout) {
        let tx = p.trap_x;
        let ty = p.trap_y;
        let caught = false;
        let grey = 0.0;
        if p.shape == 0 {
            // Circles, from (ty, tx) as the listing has it.
            let d = sqrt((z.re - ty) * (z.re - ty) + (z.im - tx) * (z.im - tx));
            if d < p.circle_size {
                caught = true;
                grey = 1.0 - d / p.circle_size;
            }
        } else if p.shape == 1 {
            // Crosses: the real axis's band first.
            let size = p.cross_size;
            if abs(z.re) < size {
                caught = true;
                grey = 1.0 - abs(z.re) / size;
            } else if abs(z.im) < size {
                caught = true;
                grey = 1.0 - abs(z.im) / size;
            }
        } else if p.shape == 2 {
            // Rings, from (ty, tx); a triangle across the band.
            let lo = p.ring_min;
            let hi = p.ring_max;
            let d = sqrt((z.re - ty) * (z.re - ty) + (z.im - tx) * (z.im - tx));
            if d < hi && d > lo {
                caught = true;
                let q = (d - lo) / (hi - lo);
                let tri = if q > 0.5 { 0.5 - (q - 0.5) } else { q };
                grey = tri * 2.0;
            }
        } else if p.shape == 3 {
            // Squares, about (tx, ty) as named.
            let size = p.square_size;
            if z.re > tx - size && z.re < tx + size && z.im > ty - size && z.im < ty + size {
                caught = true;
                grey = 1.0 - ((abs(z.re - tx) + abs(z.im - ty)) / 2.0) / size;
            }
        } else {
            // Stalks: |z| within the radius of |c|, from z3 on.
            let rad = p.stalk_radius;
            let m = z.abs();
            let rc = ctx.step_c.abs();
            if m <= rc + rad && m >= rc - rad && i > 1.0 {
                caught = true;
                grey = min(1.0, sqrt(1.0 - abs(m - rc) / rad));
            }
        }
        if caught {
            s.caught = 1.0;
            s.grey = grey;
        }
    }
    s
}

// Visions of Chaos's smooth count, i + 1 - log2(ln|z|), i the loop index
// the escape broke on -- this renderer's n less one.
fn smooth_count(z, n) {
    let r = max(z.abs(), 1.0000001);
    to_float(n) - log2(ln(r))
}

// Caught: the grey. Escaped uncaught: the smooth palette position, for
// the relief and auto contrast to read.
fn finish(s, z, p, ctx) {
    if s.caught > 0.5 {
        s.grey
    } else {
        let real = smooth_count(z, ctx.numiter);
        (real - 255.0 * to_float(floor(real / 255.0))) / 256.0
    }
}

fn color(s, z, p, ctx) {
    if s.caught > 0.5 {
        let g = pow(clamp(s.grey, 0.0, 1.0), 2.2);
        vec3(g, g, g)
    } else if !ctx.escaped {
        vec3(0.0, 0.0, 0.0)
    } else {
        let real = smooth_count(z, ctx.numiter);
        let m = real - 255.0 * to_float(floor(real / 255.0));
        let colval = to_float(floor(m));
        let colval2 = if colval + 1.0 >= 255.0 { 0.0 } else { colval + 1.0 };
        let tween = real - to_float(trunc(real));
        // Palette entries at their texel centres, blended by hand: entry
        // 254 blends into entry 0, which a single sample cannot do.
        let a = palette_srgb((colval + 0.5) / 256.0);
        let b = palette_srgb((colval2 + 0.5) / 256.0);
        vec3(
            pow(max(a.x + (b.x - a.x) * tween, 0.0), 2.2),
            pow(max(a.y + (b.y - a.y) * tween, 0.0), 2.2),
            pow(max(a.z + (b.z - a.z) * tween, 0.0), 2.2)
        )
    }
}
