/// Folds the plane along a heart-shaped curve. The output silhouette traces
/// a cardioid; classic Apophysis effect.
///
/// # Authors
/// - Scott Draves

// Heart, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Heart", name: "rhai_heart", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.re, z.im);
    let r_theta = r * theta;
    complex(r * sin(r_theta), -r * cos(r_theta))
}
