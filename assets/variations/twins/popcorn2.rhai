// Popcorn2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis displacement strength.",
    },
    y: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis displacement strength.",
    },
    c: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Frequency of the tan-sine wave that drives the displacement on both axes.",
    },
};

const INFO = #{ title: "Popcorn2", name: "rhai_popcorn2" };

fn variation(z, p) {
    let x_p = p.x;
    let y_p = p.y;
    let c = p.c;
    let tan_y = tan(z.im * c);
    tan_y = (if abs(tan_y) <= 1.0e32 { tan_y } else { 0.0 });
    let tan_x = tan(z.re * c);
    tan_x = (if abs(tan_x) <= 1.0e32 { tan_x } else { 0.0 });
    complex(z.re + x_p * sin(tan_y), z.im + y_p * sin(tan_x))
}
