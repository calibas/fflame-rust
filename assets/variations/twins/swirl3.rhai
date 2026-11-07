/// Log-spiral swirl — emits `(rad · cos(ang), rad · sin(ang))` where `rad =
/// sqrt(x²+y²) + ε` and `ang = atan2(y, x) + log(rad) · shift`. The
/// `log(rad)` term makes the angular offset grow linearly with the log of
/// radius, producing a logarithmic-spiral swirl.
///
/// # Authors
/// - Zy0rg

// Swirl 3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shift: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Shift",
        hint: "Logarithmic spiral coefficient. Larger = tighter spiral.",
    },
};

const INFO = #{ title: "Swirl 3", name: "rhai_swirl3", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let shift = p.shift;
    let rad = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let ang = atan2(z.im, z.re) + ln(rad) * shift;
    complex(rad * cos(ang), rad * sin(ang))
}
