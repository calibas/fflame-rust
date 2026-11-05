// Decomposition, the built-in colouring's twin (Ultra Fractal, Fraktaler
// 3): the angle of z where it escaped, as a palette position. With
// sectors, the angle is cut into that many flat bands -- 2 is binary
// decomposition, the sign of Im z, whose cells trace the external rays.

const PARAMS = #{
    sectors: #{
        value: 0.0, min: 0.0, max: 16.0,
        caption: "Sectors",
        hint: "0 draws the angle itself; 2 or more cuts it into that many bands -- 2 is binary decomposition.",
    },
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette turns per full turn of the angle.",
    },
};

const INFO = #{ title: "Decomposition", region: "outside" };

fn finish(s, z, p) {
    // An escaped z is far from the origin: no zero pair reaches the angle.
    let a = z.arg() * 0.15915494 + 0.5;
    let m = round_even(p.sectors);
    let v = if m >= 1.5 { floor(a * m) / m } else { a };
    v * p.scale
}
