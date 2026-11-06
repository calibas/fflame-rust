// Gaussian Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Gaussian Blur", name: "rhai_gaussian_blur" };

fn variation(z, p) {
    let theta = rand() * 6.28318530718;
    let r = rand() + rand() + rand() + rand() - 2.0;
    complex(r * cos(theta), r * sin(theta))
}
