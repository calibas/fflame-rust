// Helicoid, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    frequency: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Frequency",
        hint: "How fast the (x, y) plane rotates as Z increases. Larger = tighter spiral.",
    },
};

const INFO = #{ title: "Helicoid", name: "rhai_helicoid" };

fn variation(z, p) {
    let _f = p.frequency;
    z
}

fn variation3(v, p) {
    let two_pi = 6.28318530717959;
    let frequency = p.frequency;
    let range = sqrt(v.x * v.x + v.y * v.y);
    let phase = v.z * two_pi * frequency + atan2(v.y, v.x);
    vec3(cos(phase) * range, sin(phase) * range, v.z)
}
