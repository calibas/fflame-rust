/// Chrysanthemum curve — Sosa's flower-like parametric curve. Plots `r` as
/// a function of a random angle, producing dense overlapping petal
/// patterns.
///
/// # Authors
/// - Jesus Sosa

// Chrysanthemum, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Chrysanthemum", name: "rhai_chrysanthemum", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let u = 21.0 * pi * rand();
    let p4_b = sin(17.0 * u / 3.0);
    let p4 = p4_b * p4_b * p4_b * p4_b;
    let p8_b = sin(2.0 * cos(3.0 * u) - 28.0 * u);
    let p8_2 = p8_b * p8_b;
    let p8_4 = p8_2 * p8_2;
    let p8 = p8_4 * p8_4;
    let r = 5.0 * (1.0 + sin(11.0 * u / 5.0)) - 4.0 * p4 * p8;
    complex(r * cos(u), r * sin(u))
}
