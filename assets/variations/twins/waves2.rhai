/// Like Waves, but the sine wave frequencies and amplitudes are exposed as
/// sliders instead of being baked into the affine. Independent control over
/// each axis.
///
/// # Authors
/// - Joel Faber

// Waves2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    freqx: #{
        value: 2.0, min: 0.0, max: 20.0, free: true,
        caption: "Freq X",
        hint: "Horizontal ripple frequency. More = tighter waves across the X axis.",
    },
    scalex: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "Scale X",
        hint: "Horizontal ripple amplitude. How far points get pushed sideways.",
    },
    freqy: #{
        value: 2.0, min: 0.0, max: 20.0, free: true,
        caption: "Freq Y",
        hint: "Vertical ripple frequency.",
    },
    scaley: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "Scale Y",
        hint: "Vertical ripple amplitude.",
    },
    freqz: #{
        value: 0.0, min: 0.0, max: 20.0, free: true,
        caption: "Freq Z",
        hint: "Depth ripple frequency (3D mode only).",
    },
    scalez: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Scale Z",
        hint: "Depth ripple amplitude (3D mode only).",
    },
};

const INFO = #{ title: "Waves2", name: "rhai_waves2", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let freqx = p.freqx;
    let scalex = p.scalex;
    let freqy = p.freqy;
    let scaley = p.scaley;
    let new_x = z.re + scalex * sin(z.im * freqx);
    let new_y = z.im + scaley * sin(z.re * freqy);
    complex(new_x, new_y)
}

fn variation3(v, p) {
    let freqx = p.freqx;
    let scalex = p.scalex;
    let freqy = p.freqy;
    let scaley = p.scaley;
    let freqz = p.freqz;
    let scalez = p.scalez;
    let r_xy = complex(v.x, v.y).abs();
    let new_x = v.x + scalex * sin(v.y * freqx);
    let new_y = v.y + scaley * sin(v.x * freqy);
    let new_z = v.z + scalez * sin(r_xy * freqz);
    vec3(new_x, new_y, new_z)
}
