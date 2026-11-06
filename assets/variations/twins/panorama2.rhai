// Panorama 2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Panorama 2", name: "rhai_panorama2" };

fn variation(z, p) {
    let aux = 1.0 / (sqrt(z.re * z.re + z.im * z.im) + 1.0);
    let x1 = z.re * aux;
    let y1 = z.im * aux;
    let r1 = sqrt(x1 * x1 + y1 * y1);
    complex(atan2(x1, y1) * 0.31830988618379, r1 - 0.5)
}
