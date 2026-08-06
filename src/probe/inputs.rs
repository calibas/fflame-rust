//! The points every variation is evaluated at.
//!
//! A grid of pleasant values would prove very little. The failures this
//! exists to catch live at the inputs that are easy not to think about:
//! `npolar` broke because it reaches exactly `(0, 0)`, and it reaches it
//! on *every* call at default parity — an input a hand-written test
//! would likely include but a random sweep would essentially never hit.
//!
//! So the grid is deliberately adversarial, and fixed. Fixed matters:
//! the report is compared across machines and across time, so the
//! inputs cannot drift or be random. Appending a point at the end
//! extends every class string by one character and leaves the existing
//! characters aligned, which keeps old reports readable against new
//! ones.

/// A point to evaluate at, with a label for the report.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub label: &'static str,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

const fn p(label: &'static str, x: f32, y: f32, z: f32) -> Point {
    Point { label, x, y, z }
}

/// Just past the `1e32` bad-value threshold, so a variation that merely
/// passes its input through lands in the `Huge` class and one that
/// amplifies goes infinite.
const PAST_THRESHOLD: f32 = 1e33;

/// The smallest positive normal `f32`. Below this the hardware may
/// flush to zero — and Metal's fast-math does, which is itself worth
/// seeing in the report.
const SMALLEST_NORMAL: f32 = 1.175_494_4e-38;

/// A subnormal. Flush-to-zero turns this into a signed zero, which the
/// classifier distinguishes.
const SUBNORMAL: f32 = 1e-40;

/// The evaluation grid. Order is part of the report format — append
/// only.
pub fn probe_inputs() -> &'static [Point] {
    &POINTS
}

static POINTS: [Point; 27] = [
        // The four signed zeros. `atan2` is specified by the sign of a
        // zero, so these four are separate points rather than one: on
        // Metal all four collapse to pi/4, and the report shows all
        // four moving together.
        p("+0+0", 0.0, 0.0, 0.0),
        p("+0-0", 0.0, -0.0, 0.0),
        p("-0+0", -0.0, 0.0, 0.0),
        p("-0-0", -0.0, -0.0, 0.0),
        // Axes — where a division by one component is a division by
        // zero.
        p("+x", 1.0, 0.0, 0.0),
        p("-x", -1.0, 0.0, 0.0),
        p("+y", 0.0, 1.0, 0.0),
        p("-y", 0.0, -1.0, 0.0),
        // Diagonals, all four sign quadrants.
        p("q1", 1.0, 1.0, 0.5),
        p("q2", -1.0, 1.0, -0.5),
        p("q3", -1.0, -1.0, 0.5),
        p("q4", 1.0, -1.0, -0.5),
        // Unremarkable values, where a variation should simply work.
        // If these move, something structural changed.
        p("mid", 0.5, 0.3, 0.2),
        p("odd", 0.123_456_79, -0.987_654_3, 0.456_789),
        p("big", 2.5, -1.5, 3.25),
        p("pi_e", std::f32::consts::PI, std::f32::consts::E, -std::f32::consts::LN_2),
        // The unit circle exactly, where `sqrt(1 - x*x - y*y)` is zero
        // and rounding decides whether it is a small negative — a NaN
        // under `sqrt` on one platform and not the other.
        p("unit", 1.0, 0.0, 0.0),
        p("half_unit", 0.707_106_77, 0.707_106_77, 0.0),
        // Small, then smaller than normal, then subnormal.
        p("tiny", 1e-20, -1e-20, 1e-20),
        p("normal_min", SMALLEST_NORMAL, SMALLEST_NORMAL, SMALLEST_NORMAL),
        p("subnormal", SUBNORMAL, -SUBNORMAL, SUBNORMAL),
        // Large, then either side of the bad-value threshold. A
        // variation whose output crosses `1e32` is one the renderer
        // would respawn, so the class boundary is where behaviour
        // actually changes.
        p("large", 1e20, -1e20, 1e20),
        p("near_threshold", 9e31, -9e31, 9e31),
        p("past_threshold", PAST_THRESHOLD, -PAST_THRESHOLD, PAST_THRESHOLD),
        // Asymmetric magnitudes, where a `hypot`-style computation can
        // overflow in the square even though the result is fine.
        p("lopsided", 1e30, 1e-30, 1.0),
        // Negative zero paired with a real value: `atan2(y, -0.0)` is
        // the case the `npolar` guard has to get right, and getting it
        // wrong by flattening to zero moved the render on Vulkan.
        p("y_neg0x", 1.0, -0.0, 0.0),
    p("neg0y_x", -0.0, 1.0, 0.0),
];

/// How many components each point yields. 2D writes x and y; 3D also
/// writes z. Colour-writing variations contribute one more.
pub const COMPONENTS_2D: usize = 2;
pub const COMPONENTS_3D: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_signed_zeros_are_present_and_distinct() {
        let zeros: Vec<&Point> = probe_inputs()
            .iter()
            .filter(|p| p.x == 0.0 && p.y == 0.0)
            .collect();
        assert_eq!(zeros.len(), 4, "the atan2(0,0) case needs all four signs");

        let bits: std::collections::HashSet<(u32, u32)> = zeros
            .iter()
            .map(|p| (p.x.to_bits(), p.y.to_bits()))
            .collect();
        assert_eq!(bits.len(), 4, "two of the four are the same bit pattern");
    }

    #[test]
    fn the_grid_straddles_the_bad_value_threshold() {
        let below = probe_inputs().iter().any(|p| p.x.abs() > 1e30 && p.x.abs() < 1e32);
        let above = probe_inputs().iter().any(|p| p.x.abs() > 1e32 && p.x.is_finite());
        assert!(below && above, "need points either side of 1e32 to see the boundary");
    }

    #[test]
    fn labels_are_unique_so_the_report_can_name_a_failing_point() {
        let mut seen = std::collections::HashSet::new();
        for point in probe_inputs() {
            assert!(seen.insert(point.label), "duplicate label `{}`", point.label);
        }
    }

    #[test]
    fn no_input_is_itself_nan_or_infinite() {
        // The probe tests what variations do with representable
        // inputs. Feeding in a NaN would test the *renderer's* recovery
        // path, which `main_template.wgsl` handles and which belongs to
        // the visual suite.
        for point in probe_inputs() {
            for (name, v) in [("x", point.x), ("y", point.y), ("z", point.z)] {
                assert!(v.is_finite(), "{} {name} is not finite", point.label);
            }
        }
    }
}
