// Blur 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Blur 3D", name: "rhai_blur3d" };

fn variation(z, p) {
    let theta = rand() * 6.28318530718;
    let phi = rand() * 3.14159265359;
    let r = rand() + rand() + rand() + rand() - 2.0;
    complex(r * sin(phi) * cos(theta), r * sin(phi) * sin(theta))
}

fn variation3(v, p) {
    let theta = rand() * 6.28318530718;
    let phi = rand() * 3.14159265359;
    let r = rand() + rand() + rand() + rand() - 2.0;
    vec3(r * sin(phi) * cos(theta), r * sin(phi) * sin(theta), r * cos(phi))
}
