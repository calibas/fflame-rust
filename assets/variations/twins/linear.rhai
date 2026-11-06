/// The identity transform — passes the point through unchanged. Acts as
/// a baseline shape; mix it with other variations to soften their
/// effect, or use a small weight as a placeholder.
///
/// # Authors
/// - Scott Draves

// Linear, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Linear", name: "rhai_linear", category: "basic_2d", phase: "any" };

fn variation(z, p) {
    z
}
