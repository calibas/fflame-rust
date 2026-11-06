// Hole Square, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Hole Square", name: "rhai_holesq" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let x = w * z.re;
    let y = w * z.im;
    let fax = abs(x);
    let fay = abs(y);
    let fx = 0.0;
    let fy = 0.0;
    if (fax + fay > 1.0) {
        fx = x;
        fy = y;
    } else if (fax > fay) {
        let t = 0.0;
        if (x < 0.0) {
            t = (x + fay - 1.0) * 0.5;
        } else {
            t = (x - fay + 1.0) * 0.5;
        }
        fx = t;
        fy = y;
    } else {
        let t = 0.0;
        if (y < 0.0) {
            t = (y + fax - 1.0) * 0.5;
        } else {
            t = (y - fax + 1.0) * 0.5;
        }
        fx = x;
        fy = t;
    }
    complex(fx * inv_w, fy * inv_w)
}
