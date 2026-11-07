/// 3D version of Pie — same pie-slice splatter as Pie but adds `z' =
/// r·sin(r)` for depth modulation.

// Pie 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    slices: #{
        value: 7.0, min: 1.0, max: 64.0,
        caption: "Slices",
        hint: "Number of pie wedges (1-64).",
    },
    rotation: #{
        value: 0.0, min: 0.0, max: 360.0, type: "angle",
        caption: "Rotation",
        hint: "Rotation angle of the whole pie in degrees.",
    },
    thickness: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Thickness",
        hint: "Wedge thickness within its slice. 0 = razor-thin spokes, 1 = wedges fill their entire slice.",
    },
};

const INFO = #{ title: "Pie 3D", name: "rhai_pie3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let two_pi = 6.28318530717959;
    let slices = max(1.0, p.slices);
    let rotation = p.rotation;
    let thickness = p.thickness;
    let sl = to_float(floor(rand() * slices + 0.5));
    let a = rotation + two_pi * (sl + rand() * thickness) / slices;
    let r = rand();
    complex(r * cos(a), r * sin(a))
}

fn variation3(v, p) {
    let two_pi = 6.28318530717959;
    let slices = max(1.0, p.slices);
    let rotation = p.rotation;
    let thickness = p.thickness;
    let sl = to_float(floor(rand() * slices + 0.5));
    let a = rotation + two_pi * (sl + rand() * thickness) / slices;
    let r = rand();
    vec3(r * cos(a), r * sin(a), r * sin(r))
}
