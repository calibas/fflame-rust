//! A unit quaternion for camera orientation: the one place every 3D
//! camera's turning composes (docs/projects/camera-unification.md, C2).
//!
//! It stands for the camera's WORLD → CAMERA rotation, the matrix whose
//! rows are the camera's axes in world coordinates (row 0 screen-right,
//! row 1 screen-DOWN, row 2 the camera's +z, which it looks away from) --
//! the rows `chain::rows` builds and every renderer's camera uses. A turn
//! about an axis given in CAMERA coordinates is a left product,
//! `Quat::from_axis_angle(axis, angle) * q`; one about a WORLD axis is a
//! right product.
//!
//! f64 throughout: an orientation is composed from many small turns, and
//! the decomposition back to stored angles reads it near its poles.

use std::ops::Mul;

/// `w + xi + yj + zk`. Kept unit-length by the operations that make one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A 3x3 matrix, row-major: `m[row][col]`.
pub type Mat3 = [[f64; 3]; 3];

impl Quat {
    pub const IDENTITY: Quat = Quat { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    /// The rotation by `angle` radians about `axis` (any length; a zero
    /// axis is no rotation).
    pub fn from_axis_angle(axis: [f64; 3], angle: f64) -> Quat {
        let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if !(len > 0.0) || !angle.is_finite() {
            return Quat::IDENTITY;
        }
        let (s, c) = (angle * 0.5).sin_cos();
        let k = s / len;
        Quat { w: c, x: axis[0] * k, y: axis[1] * k, z: axis[2] * k }
    }

    /// The quaternion of a rotation matrix (Shepperd's method: the
    /// largest of the four squared components is taken from the
    /// diagonal, so no division is by something near zero).
    pub fn from_rows(m: &Mat3) -> Quat {
        let t = m[0][0] + m[1][1] + m[2][2];
        let q = if t > 0.0 {
            let s = (t + 1.0).sqrt() * 2.0;
            Quat { w: 0.25 * s, x: (m[2][1] - m[1][2]) / s, y: (m[0][2] - m[2][0]) / s, z: (m[1][0] - m[0][1]) / s }
        } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
            let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
            Quat { w: (m[2][1] - m[1][2]) / s, x: 0.25 * s, y: (m[0][1] + m[1][0]) / s, z: (m[0][2] + m[2][0]) / s }
        } else if m[1][1] > m[2][2] {
            let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
            Quat { w: (m[0][2] - m[2][0]) / s, x: (m[0][1] + m[1][0]) / s, y: 0.25 * s, z: (m[1][2] + m[2][1]) / s }
        } else {
            let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
            Quat { w: (m[1][0] - m[0][1]) / s, x: (m[0][2] + m[2][0]) / s, y: (m[1][2] + m[2][1]) / s, z: 0.25 * s }
        };
        q.normalized()
    }

    /// The rotation matrix, row-major.
    pub fn to_rows(&self) -> Mat3 {
        let Quat { w, x, y, z } = self.normalized();
        [
            [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)],
            [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)],
            [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)],
        ]
    }

    /// The same rotation at unit length (identity for a degenerate one).
    pub fn normalized(&self) -> Quat {
        let n = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if !(n > 0.0) || !n.is_finite() {
            return Quat::IDENTITY;
        }
        Quat { w: self.w / n, x: self.x / n, y: self.y / n, z: self.z / n }
    }

    /// The inverse rotation (for a unit quaternion, the conjugate).
    pub fn conjugate(&self) -> Quat {
        Quat { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }

    /// `v` rotated.
    pub fn rotate(&self, v: [f64; 3]) -> [f64; 3] {
        let m = self.to_rows();
        [
            m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
            m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
            m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
        ]
    }
}

impl Mul for Quat {
    type Output = Quat;
    /// The rotation `rhs` first, then `self` -- as the matrices multiply.
    fn mul(self, r: Quat) -> Quat {
        let l = self;
        Quat {
            w: l.w * r.w - l.x * r.x - l.y * r.y - l.z * r.z,
            x: l.w * r.x + l.x * r.w + l.y * r.z - l.z * r.y,
            y: l.w * r.y - l.x * r.z + l.y * r.w + l.z * r.x,
            z: l.w * r.z + l.x * r.y - l.y * r.x + l.z * r.w,
        }
        .normalized()
    }
}

/// `a · b`, row-major.
pub fn mat_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    out
}

/// The transpose -- for a rotation, its inverse.
pub fn transpose(m: &Mat3) -> Mat3 {
    [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]]
}

#[cfg(test)]
pub(crate) fn max_diff(a: &Mat3, b: &Mat3) -> f64 {
    let mut d = 0.0f64;
    for i in 0..3 {
        for j in 0..3 {
            d = d.max((a[i][j] - b[i][j]).abs());
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axes() -> Vec<[f64; 3]> {
        vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 2.0, -0.5], [-0.3, 0.1, 0.9]]
    }

    /// A rotation survives the trip to a quaternion and back, from every
    /// branch of Shepperd's method (angles past π/2 reach the
    /// negative-trace ones).
    #[test]
    fn rows_round_trip() {
        for axis in axes() {
            for k in -12..=12 {
                let angle = k as f64 * 0.27;
                let m = Quat::from_axis_angle(axis, angle).to_rows();
                let back = Quat::from_rows(&m).to_rows();
                assert!(max_diff(&m, &back) < 1e-12, "{axis:?} {angle}");
            }
        }
    }

    /// The product of quaternions is the product of their matrices, in
    /// the same order.
    #[test]
    fn product_is_the_matrix_product() {
        let a = Quat::from_axis_angle([1.0, 2.0, -0.5], 0.7);
        let b = Quat::from_axis_angle([-0.3, 0.1, 0.9], -1.9);
        let m = mat_mul(&a.to_rows(), &b.to_rows());
        assert!(max_diff(&(a * b).to_rows(), &m) < 1e-12);
    }

    /// The matrix is the right-handed rotation about the axis: a quarter
    /// turn about +z takes +x to +y.
    #[test]
    fn a_quarter_turn_about_z() {
        let v = Quat::from_axis_angle([0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2).rotate([1.0, 0.0, 0.0]);
        assert!((v[0]).abs() < 1e-12 && (v[1] - 1.0).abs() < 1e-12 && v[2].abs() < 1e-12, "{v:?}");
    }
}
