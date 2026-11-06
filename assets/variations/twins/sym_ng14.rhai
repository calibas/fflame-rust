// Sym NG14, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "radius",
        hint: "Network-symmetry parameter (see JWildfire SymNetG14).",
    },
    stepx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "stepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG14).",
    },
    stepy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "stepy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG14).",
    },
};

const INFO = #{ title: "Sym NG14", name: "rhai_sym_ng14" };

fn variation(z, p) {
    let radius = p.radius;
    let stepx = p.stepx;
    let stepy = p.stepy;
    let sx = stepx / 2.0;
    let sy = stepy / 2.0;
    let spacex = sqrt(radius * radius / 2.0);
    let spacey = spacex;
    let zx = z.re + (spacex);
    let zy = z.im + (spacex);
    let idx = to_int(to_float(floor(rand() * 12.0)));
    let a = 0.0;
    let b = 0.0;
    let c = 0.0;
    let d = 0.0;
    let e = 0.0;
    let ff = 0.0;
    let t1 = (idx);
    if t1 == 0 {
        a = 1.0;
        b = 0.0;
        c = -sx;
        d = 0.0;
        e = 1.0;
        ff = -sy;
    } else if t1 == 1 {
        a = 1.0;
        b = 0.0;
        c = -sx;
        d = 0.0;
        e = -1.0;
        ff = -sy;
    } else if t1 == 2 {
        a = -0.5;
        b = -0.866;
        c = -sx;
        d = 0.866;
        e = -0.50;
        ff = -sy;
    } else if t1 == 3 {
        a = -0.5;
        b = 0.866;
        c = -sx;
        d = -0.866;
        e = -0.50;
        ff = -sy;
    } else if t1 == 4 {
        a = -0.5;
        b = -0.866;
        c = -sx;
        d = -0.866;
        e = 0.50;
        ff = -sy;
    } else if t1 == 5 {
        a = -0.5;
        b = 0.866;
        c = -sx;
        d = 0.866;
        e = 0.50;
        ff = -sy;
    } else if t1 == 6 {
        a = 1.0;
        b = 0.0;
        c = sx;
        d = 0.0;
        e = 1.0;
        ff = sy;
    } else if t1 == 7 {
        a = 1.0;
        b = 0.0;
        c = sx;
        d = 0.0;
        e = -1.0;
        ff = sy;
    } else if t1 == 8 {
        a = -0.5;
        b = -0.866;
        c = sx;
        d = 0.866;
        e = -0.50;
        ff = sy;
    } else if t1 == 9 {
        a = -0.5;
        b = 0.866;
        c = sx;
        d = -0.866;
        e = -0.50;
        ff = sy;
    } else if t1 == 10 {
        a = -0.5;
        b = -0.866;
        c = sx;
        d = -0.866;
        e = 0.50;
        ff = sy;
    } else {
        a = -0.5;
        b = 0.866;
        c = sx;
        d = 0.866;
        e = 0.50;
        ff = sy;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
