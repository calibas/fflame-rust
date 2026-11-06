// Exp 2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Exp 2", name: "rhai_exp2" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let a = exp(z.re * pi);
    complex(a * cos(z.im * pi), a * sin(z.im * pi))
}
