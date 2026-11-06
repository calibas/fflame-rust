/// Maps the plane into a rotated diamond shape using sine and cosine of the
/// polar angle and radius. Produces sharp diagonal symmetry.
///
/// # Authors
/// - Scott Draves

// Diamond, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Diamond", name: "rhai_diamond", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.re, z.im);
    complex(sin(theta) * cos(r), cos(theta) * sin(r))
}
