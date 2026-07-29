//! The colour value scripts work in.
//!
//! Stored as linear-ish RGB in 0..1 — the same triple `ColorStop` and
//! the app's colour pickers use, so nothing is converted on the way in
//! or out. What this module adds is the HSV view of it.
//!
//! HSV earns its place because the relationships colour theory NAMES are
//! coordinates in it: complementary is `h + 180`, triadic `h ± 120`,
//! analogous `h ± 30`, and monochromatic is "hold h, vary s and v".
//! None of those is simple arithmetic in RGB, and interpolating between
//! two hues in RGB passes through desaturated mud. The same applies to
//! deliberately roughening a palette: jittering H, S and V separately is
//! meaningful, jittering R, G and B separately is not.
//!
//! Known limit: HSV is not perceptually uniform, so equal steps in hue
//! do not look equally different — yellows and greens crowd together
//! while blues spread out. If generated palettes come out uneven, OKLCH
//! would slot in beside this with the same shape of API.

/// A colour a script can hold, in RGB 0..1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScriptColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl ScriptColor {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
        }
    }

    pub fn from_rgb(rgb: [f32; 3]) -> Self {
        Self::new(rgb[0], rgb[1], rgb[2])
    }

    pub fn to_rgb(self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }

    /// From hue in DEGREES (wrapped), saturation and value in 0..1.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = h.rem_euclid(360.0);
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Self::new(r + m, g + m, b + m)
    }

    /// Hue in degrees 0..360. Grey has no hue; report 0 rather than NaN
    /// so a script can keep doing arithmetic with it.
    pub fn hue(self) -> f32 {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let d = max - min;
        if d <= f32::EPSILON {
            return 0.0;
        }
        let h = if max == self.r {
            60.0 * (((self.g - self.b) / d) % 6.0)
        } else if max == self.g {
            60.0 * ((self.b - self.r) / d + 2.0)
        } else {
            60.0 * ((self.r - self.g) / d + 4.0)
        };
        h.rem_euclid(360.0)
    }

    pub fn saturation(self) -> f32 {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        if max <= f32::EPSILON {
            0.0
        } else {
            (max - min) / max
        }
    }

    pub fn value(self) -> f32 {
        self.r.max(self.g).max(self.b)
    }

    pub fn with_hue(self, h: f32) -> Self {
        Self::from_hsv(h, self.saturation(), self.value())
    }

    pub fn with_saturation(self, s: f32) -> Self {
        Self::from_hsv(self.hue(), s, self.value())
    }

    pub fn with_value(self, v: f32) -> Self {
        Self::from_hsv(self.hue(), self.saturation(), v)
    }

    pub fn rotate_hue(self, degrees: f32) -> Self {
        self.with_hue(self.hue() + degrees)
    }

    /// Straight RGB blend. Mixing in HSV would take the long way round
    /// the wheel half the time; a script that wants a hue path should
    /// rotate the hue instead.
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
        )
    }

    /// `#rrggbb`, with or without the hash, and the `#rgb` short form.
    pub fn from_hex(text: &str) -> Result<Self, String> {
        let hex = text.trim().trim_start_matches('#');
        let byte = |i: usize| -> Result<f32, String> {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map(|v| v as f32 / 255.0)
                .map_err(|_| format!("`{text}` is not a colour — expected something like \"#ff8800\""))
        };
        match hex.len() {
            6 => Ok(Self::new(byte(0)?, byte(2)?, byte(4)?)),
            3 => {
                let nib = |i: usize| -> Result<f32, String> {
                    u8::from_str_radix(&hex[i..i + 1], 16)
                        .map(|v| (v * 17) as f32 / 255.0)
                        .map_err(|_| {
                            format!("`{text}` is not a colour — expected something like \"#ff8800\"")
                        })
                };
                Ok(Self::new(nib(0)?, nib(1)?, nib(2)?))
            }
            _ => Err(format!(
                "`{text}` is not a colour — expected \"#ff8800\" or \"#f80\""
            )),
        }
    }

    pub fn to_hex(self) -> String {
        let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", ch(self.r), ch(self.g), ch(self.b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn hsv_round_trips() {
        // Every hue sextant, plus the boundaries between them.
        for h in [0.0, 45.0, 60.0, 120.0, 180.0, 240.0, 300.0, 359.0] {
            for s in [0.25, 1.0] {
                for v in [0.4, 1.0] {
                    let c = ScriptColor::from_hsv(h, s, v);
                    assert!(close(c.hue(), h), "hue {h} -> {}", c.hue());
                    assert!(close(c.saturation(), s), "sat {s} -> {}", c.saturation());
                    assert!(close(c.value(), v), "val {v} -> {}", c.value());
                }
            }
        }
    }

    /// The point of having HSV at all: colour-theory relationships are
    /// hue arithmetic, and they have to survive the round trip through
    /// RGB storage.
    #[test]
    fn colour_theory_relationships_are_hue_arithmetic() {
        let base = ScriptColor::from_hsv(30.0, 0.8, 0.9);
        assert!(close(base.rotate_hue(180.0).hue(), 210.0), "complementary");
        assert!(close(base.rotate_hue(120.0).hue(), 150.0), "triadic");
        assert!(close(base.rotate_hue(-60.0).hue(), 330.0), "wraps below zero");
        assert!(close(base.rotate_hue(400.0).hue(), 70.0), "wraps above 360");
        // A rotation must not disturb the other two axes, or a scheme
        // drifts in brightness as it goes round the wheel.
        let spun = base.rotate_hue(137.0);
        assert!(close(spun.saturation(), base.saturation()));
        assert!(close(spun.value(), base.value()));
    }

    #[test]
    fn grey_has_no_hue_but_still_computes() {
        let grey = ScriptColor::new(0.5, 0.5, 0.5);
        assert_eq!(grey.hue(), 0.0, "no NaN to poison later arithmetic");
        assert_eq!(grey.saturation(), 0.0);
        assert!(close(grey.value(), 0.5));
        // Black is the other degenerate case.
        let black = ScriptColor::new(0.0, 0.0, 0.0);
        assert_eq!(black.saturation(), 0.0);
        assert_eq!(black.value(), 0.0);
    }

    #[test]
    fn hex_round_trips_both_forms() {
        assert_eq!(ScriptColor::from_hex("#ff8800").unwrap().to_hex(), "#ff8800");
        assert_eq!(ScriptColor::from_hex("ff8800").unwrap().to_hex(), "#ff8800");
        assert_eq!(ScriptColor::from_hex("#f80").unwrap().to_hex(), "#ff8800");
        for bad in ["", "#12345", "nonsense", "#gg0000"] {
            assert!(ScriptColor::from_hex(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn mixing_moves_between_the_ends() {
        let a = ScriptColor::new(1.0, 0.0, 0.0);
        let b = ScriptColor::new(0.0, 0.0, 1.0);
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        let half = a.mix(b, 0.5);
        assert!(close(half.r, 0.5) && close(half.b, 0.5));
    }
}
