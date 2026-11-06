// Flip Circle, the built-in variation's twin: the points inside the circle of the weight's radius flipped across the real axis.

const INFO = #{ title: "Flip Circle", name: "rhai_flipcircle" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inside = (z.re * z.re + z.im * z.im) <= w * w;
    if inside { complex(z.re, -z.im) } else { z }
}
