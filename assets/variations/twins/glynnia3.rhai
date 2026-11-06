/// Glynnia with 4 user-tunable knobs — exposes the radius/distance scaling
/// and the branch threshold as parameters.
///
/// # Authors
/// - Michael Faber
/// - Maulana Randa
/// - CozyG

// Glynnia 3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    rscale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "R scale",
        hint: "Radial scaling on the input distance.",
    },
    dscale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D scale",
        hint: "Scaling on the `d = r + x` term used by both branches.",
    },
    rthresh: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "R threshold",
        hint: "Radius threshold for the inside-vs-outside branch split.",
    },
    ythresh: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y threshold",
        hint: "Y threshold added to the branch condition — only points with `y > ythresh` take the outer branch.",
    },
};

const INFO = #{ title: "Glynnia 3", name: "rhai_glynnia3", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let rscale = p.rscale;
    let dscale = p.dscale;
    let rthresh = p.rthresh;
    let ythresh = p.ythresh;
    let half_sqrt2 = 0.7071067811865476;
    let r = rscale * sqrt(z.re * z.re + z.im * z.im);
    if (r > rthresh && z.im > ythresh) {
        if (rand() > 0.5) {
            let d = dscale * sqrt(max(r + z.re, 0.0));
            if (d == 0.0) {
                return complex(0.0, 0.0);
            }
            return complex(half_sqrt2 * d, -half_sqrt2 / d * z.im);
        }
        let d = dscale * (r + z.re);
        let dx = sqrt(max(r * (z.im * z.im + d * d), 0.0));
        if (dx == 0.0) {
            return complex(0.0, 0.0);
        }
        return complex(d / dx, z.im / dx);
    }
    if (rand() > 0.5) {
        let d = dscale * sqrt(max(r + z.re, 0.0));
        if (d == 0.0) {
            return complex(0.0, 0.0);
        }
        return complex(-half_sqrt2 * d, -half_sqrt2 / d * z.im);
    }
    let d = dscale * (r + z.re);
    let dx = sqrt(max(r * (z.im * z.im + d * d), 0.0));
    if (dx == 0.0) {
        return complex(0.0, 0.0);
    }
    complex(-d / dx, z.im / dx)
}
