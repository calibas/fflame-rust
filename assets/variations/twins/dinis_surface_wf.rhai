// Dini's Surface WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 0.8, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Radial scale of the surface (controls XY radius and Z magnitude).",
    },
    b: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Helical twist coefficient — multiplies the input U to add a linear helical Z offset.",
    },
};

const INFO = #{ title: "Dini's Surface WF", name: "rhai_dinis_surface_wf" };

fn variation(z, p) {
    let a = p.a;
    let u = z.re;
    let v = z.im;
    let sinv = sin(v);
    complex(a * cos(u) * sinv, a * sin(u) * sinv)
}

fn variation3(q, p) {
    let a = p.a;
    let b = p.b;
    let u = q.x;
    let v = q.y;
    let sinv = sin(v);
    let tan_half = tan(v * 0.5);
    let log_tan = ln(max(abs(tan_half), 1.0e-30));
    vec3(a * cos(u) * sinv, a * sin(u) * sinv, -(a * (cos(v) + log_tan) + b * u))
}
