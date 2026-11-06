/// Random-mirror tile blur — shifts the input along one axis by ±space
/// (direction chosen by coin flip). With `reversal = 1` the same axis is
/// mirrored as well. `vertical` selects which axis gets the tiling
/// treatment.
///
/// # Authors
/// - Whittaker Courtney

// Tile Reverse, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    space: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Space",
        hint: "Tile offset along the active axis.",
    },
    reversal: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Reversal",
        hint: "When equal to 1.0, the active axis is also mirrored. Any other value passes through unchanged.",
    },
    vertical: #{
        value: false,
        caption: "Vertical",
        hint: "When on, tile along the vertical axis. When off, horizontal.",
    },
};

const INFO = #{ title: "Tile Reverse", name: "rhai_tile_reverse", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let space = p.space;
    let reversal = p.reversal;
    let vertical = to_float(p.vertical);
    let rev = (if reversal == 1.0 { -1.0 } else { 1.0 });
    let n = rand();
    let sign_v = (if n < 0.5 { 1.0 } else { -1.0 });
    if (vertical == 1.0) {
        return complex(z.re, rev * z.im + sign_v * space);
    }
    complex(rev * z.re + sign_v * space, z.im)
}
