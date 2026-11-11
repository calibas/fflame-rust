//! The audio analysers' FFT: an iterative radix-2 complex transform.
//!
//! The analyser (`analyzer.rs`) and live capture (`capture_common.rs`)
//! transform one power-of-two window at a time (1024 or 2048 samples), which
//! a textbook radix-2 FFT does in microseconds. It replaced `rustfft`, ~680
//! KiB of code in the app for this one use (docs/projects/size-study.md §5);
//! `rustfft` stays as a dev-dependency to hold this to it
//! (`matches_rustfft`), and to check the GPU FFT (`sim::app_repro_test`).
//!
//! Forward, unnormalised, `X[k] = sum x[n] e^(-2 pi i k n / N)`: rustfft's
//! convention.

/// A complex number: what the analysers need of one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

impl Complex {
    pub const fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    /// The magnitude.
    pub fn norm(self) -> f32 {
        self.re.hypot(self.im)
    }

    fn mul(self, o: Self) -> Self {
        Self::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

/// A forward FFT of one size, its twiddles computed once.
#[derive(Debug, Clone)]
pub struct Fft {
    n: usize,
    /// `e^(-2 pi i k / n)` for `k < n / 2`, computed in f64.
    twiddles: Vec<Complex>,
}

impl Fft {
    /// A forward transform of `n` points. `n` must be a power of two.
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two(), "the audio FFT takes a power-of-two size, not {n}");
        let twiddles = (0..n / 2)
            .map(|k| {
                let a = -2.0 * std::f64::consts::PI * k as f64 / n as f64;
                Complex::new(a.cos() as f32, a.sin() as f32)
            })
            .collect();
        Self { n, twiddles }
    }

    /// Transform `buf` in place; `buf.len()` must be the size this was made for.
    pub fn process(&self, buf: &mut [Complex]) {
        let n = self.n;
        assert_eq!(buf.len(), n, "the FFT was made for {n} points");
        if n < 2 {
            return;
        }
        // Bit-reversal permutation.
        let bits = n.trailing_zeros();
        for i in 0..n {
            let j = i.reverse_bits() >> (usize::BITS - bits);
            if i < j {
                buf.swap(i, j);
            }
        }
        // Butterflies, doubling the span each pass.
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let stride = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let w = self.twiddles[k * stride];
                    let a = buf[start + k];
                    let b = buf[start + k + half].mul(w);
                    buf[start + k] = Complex::new(a.re + b.re, a.im + b.im);
                    buf[start + k + half] = Complex::new(a.re - b.re, a.im - b.im);
                }
            }
            len *= 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(n: usize, seed: u64) -> Vec<f32> {
        // A deterministic mix of tones and noise.
        let mut s = seed;
        (0..n)
            .map(|i| {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let noise = ((s >> 40) as f32 / (1u64 << 24) as f32) - 0.5;
                let t = i as f32 / n as f32;
                (2.0 * std::f32::consts::PI * 5.0 * t).sin() + 0.3 * (2.0 * std::f32::consts::PI * 97.0 * t).cos() + noise
            })
            .collect()
    }

    #[test]
    fn matches_rustfft() {
        use rustfft::{num_complex::Complex as RC, FftPlanner};
        let mut planner = FftPlanner::<f32>::new();
        for n in [1usize, 2, 4, 8, 64, 1024, 2048, 4096] {
            let x = signal(n, n as u64);
            let mut ours: Vec<Complex> = x.iter().map(|&v| Complex::new(v, 0.5 * v)).collect();
            let mut theirs: Vec<RC<f32>> = x.iter().map(|&v| RC::new(v, 0.5 * v)).collect();
            Fft::new(n).process(&mut ours);
            planner.plan_fft_forward(n).process(&mut theirs);
            // Relative to the spectrum's size: f32 rounding grows ~log n.
            let scale = theirs.iter().map(|c| c.norm()).fold(1.0f32, f32::max);
            for (k, (a, b)) in ours.iter().zip(&theirs).enumerate() {
                let err = (a.re - b.re).hypot(a.im - b.im) / scale;
                assert!(err < 1e-5, "n={n} bin {k}: {a:?} vs {b:?} (err {err})");
            }
        }
    }

    #[test]
    fn a_tone_lands_in_its_bin() {
        let n = 1024;
        let fft = Fft::new(n);
        let mut buf: Vec<Complex> = (0..n)
            .map(|i| Complex::new((2.0 * std::f32::consts::PI * 37.0 * i as f32 / n as f32).cos(), 0.0))
            .collect();
        fft.process(&mut buf);
        let peak = (0..=n / 2).max_by(|&a, &b| buf[a].norm().total_cmp(&buf[b].norm())).unwrap();
        assert_eq!(peak, 37);
        assert!((buf[37].norm() - n as f32 / 2.0).abs() < 1e-2);
    }

    #[test]
    #[should_panic(expected = "power-of-two")]
    fn refuses_other_sizes() {
        Fft::new(1000);
    }
}
