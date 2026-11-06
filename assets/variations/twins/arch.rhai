// Arch, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Arch", name: "rhai_arch" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let ang = rand() * w * pi;
    let sinr = sin(ang);
    let cosr = cos(ang);
    let safe_cosr = (if abs(cosr) < 1.0e-30 { 1.0e-30 } else { cosr });
    let fx = w * sinr;
    let fy = w * sinr * sinr / safe_cosr;
    complex(fx * inv_w, fy * inv_w)
}
