// Inv Polar, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inv Polar", name: "rhai_invpolar" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let ny = 1.0 + z.im;
    complex(ny * sin(z.re * pi), ny * cos(z.re * pi))
}
