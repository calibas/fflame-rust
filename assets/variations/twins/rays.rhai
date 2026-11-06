// Rays, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Rays", name: "rhai_rays" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let ang = w * rand() * pi;
    let r = w / (z.re * z.re + z.im * z.im + 1.0e-6);
    let tanr = w * tan(ang) * r;
    complex(tanr * cos(z.re) * inv_w, tanr * sin(z.im) * inv_w)
}
