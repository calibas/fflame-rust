// Swirl 3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shift: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Shift",
        hint: "Logarithmic spiral coefficient. Larger = tighter spiral.",
    },
};

const INFO = #{ title: "Swirl 3", name: "rhai_swirl3" };

fn variation(z, p) {
    let shift = p.shift;
    let rad = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let ang = atan2(z.im, z.re) + ln(rad) * shift;
    complex(rad * cos(ang), rad * sin(ang))
}
