// Noise, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Noise", name: "rhai_noise" };

fn variation(z, p) {
    let theta = rand() * 6.28318530718;
    let r = rand();
    complex(z.re * r * cos(theta), z.im * r * sin(theta))
}
