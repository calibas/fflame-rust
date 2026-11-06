// Sinusoidal, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sinusoidal", name: "rhai_sinusoidal" };

fn variation(z, p) {
    complex(sin(z.re), sin(z.im))
}
