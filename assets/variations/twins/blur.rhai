// Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Blur", name: "rhai_blur" };

fn variation(z, p) {
    let theta = rand() * 6.28318530718;
    let r = rand();
    complex(r * cos(theta), r * sin(theta))
}
