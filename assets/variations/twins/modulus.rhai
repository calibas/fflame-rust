/// Boundary-bounded fmod warp — leaves points inside the `±x, ±y`
/// rectangle alone; outside the rectangle, wraps the coordinate back
/// inside via fmod. Produces a tiled pattern in which the central
/// rectangle is preserved while the surrounding plane is repeated.
///
/// # Authors
/// - Apophysis Plugin Pack

// Modulus, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis half-width — points outside ±x get mod-wrapped back inside.",
    },
    y: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis half-width.",
    },
};

const INFO = #{ title: "Modulus", name: "rhai_modulus", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ xr: 2.0 * p.x, yr: 2.0 * p.y }
}

fn variation(z, p) {
    let xp = p.x;
    let yp = p.y;
    let xr = p.xr;
    let yr = p.yr;
    let safe_xr = (if abs(xr) < 1.0e-30 { 1.0e-30 } else { xr });
    let safe_yr = (if abs(yr) < 1.0e-30 { 1.0e-30 } else { yr });
    let ox = 0.0;
    if (z.re > xp) {
        let v = z.re + xp;
        ox = -xp + (v - to_float(floor(v / safe_xr)) * safe_xr);
    } else if (z.re < -xp) {
        let v = xp - z.re;
        ox = xp - (v - to_float(floor(v / safe_xr)) * safe_xr);
    } else {
        ox = z.re;
    }
    let oy = 0.0;
    if (z.im > yp) {
        let v = z.im + yp;
        oy = -yp + (v - to_float(floor(v / safe_yr)) * safe_yr);
    } else if (z.im < -yp) {
        let v = yp - z.im;
        oy = yp - (v - to_float(floor(v / safe_yr)) * safe_yr);
    } else {
        oy = z.im;
    }
    complex(ox, oy)
}
