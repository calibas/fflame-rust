// Circle Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Circle Blur", name: "rhai_circleblur" };

fn variation(z, p) {
    let rad = sqrt(rand());
    let ang = rand() * 6.28318530717959;
    complex(cos(ang) * rad, sin(ang) * rad)
}
