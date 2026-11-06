/// Radial radius scale — points inside the unit circle (`r ≤ 1`) are scaled
/// outward by `scale`; points outside are scaled inward by `1/scale`.
/// Creates a deliberate discontinuity at the unit circle.
///
/// # Authors
/// - Michael Faber

// Circus, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.001, max: 10.0, free: true,
        caption: "Scale",
        hint: "Inner-disc (`r ≤ 1`) scaling factor. Outside the disc, the reciprocal `1/scale` is used.",
    },
};

const INFO = #{ title: "Circus", name: "rhai_circus", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let scale = max(1.0e-6, p.scale);
    #{ inv_scale: 1.0 / scale }
}

fn variation(z, p) {
    let scale = p.scale;
    let inv_scale = p.inv_scale;
    let r = sqrt(z.re * z.re + z.im * z.im);
    let a = atan2(z.im, z.re);
    let r_out = (if r <= 1.0 { r * scale } else { r * inv_scale });
    complex(r_out * cos(a), r_out * sin(a))
}
