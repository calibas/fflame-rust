// Exp, the built-in variation's twin: the complex exponential.

const INFO = #{ title: "Exp", name: "rhai_exp" };

fn variation(z, p) {
    let e = exp(z.re);
    complex(e * cos(z.im), e * sin(z.im))
}
