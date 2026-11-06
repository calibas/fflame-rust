// Helix, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    frequency: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Frequency",
        hint: "How many full turns the helix makes per unit of Z.",
    },
    width: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Width",
        hint: "Radius of the helical winding around the Z axis.",
    },
};

const INFO = #{ title: "Helix", name: "rhai_helix" };

fn variation(z, p) {
    let width = p.width;
    complex(z.re + width, z.im)
}

fn variation3(v, p) {
    let two_pi = 6.28318530717959;
    let frequency = p.frequency;
    let width = p.width;
    let phase = v.z * two_pi * frequency;
    vec3(v.x + cos(phase) * width, v.y + sin(phase) * width, v.z)
}
