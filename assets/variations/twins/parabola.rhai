// Parabola, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    width: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Width",
        hint: "Horizontal scaling of the parabolic envelope.",
    },
    height: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Height",
        hint: "Vertical scaling of the parabolic envelope.",
    },
};

const INFO = #{ title: "Parabola", name: "rhai_parabola" };

fn variation(z, p) {
    let width = p.width;
    let height = p.height;
    let r = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let sr = sin(r);
    let cr = cos(r);
    complex(height * sr * sr * rand(), width * cr * rand())
}
