//! Analytic-blur kernel metadata — the host-side counterpart to
//! `Feature::AnalyticBlur`. See `docs/projects/analytic-blur-buffer.md`.
//!
//! Each input-independent blur variation provides a Rust **offset sampler**
//! that mirrors its WGSL offset formula (the random displacement it adds to
//! the point, in variation-output space at weight 1). The renderer draws a
//! large batch of these offsets, maps each through a transform's pixel-space
//! linear map, and bins them into a small normalized convolution kernel —
//! so the analytic kernel matches the stochastic distribution by
//! construction (it *is* that distribution, sampled).
//!
//! Only variations whose fuzz is genuinely input-independent appear here.
//! Input-DEPENDENT blurs (`radial_blur`, `farblur`, `post_rblur`, `exblur`)
//! are intentionally absent — they have no entry and stay fully stochastic.

const TWO_PI: f32 = 6.283_185_307_18;

/// Is `name` an analytic-blur variation (one with an offset sampler here)?
/// Mirrors `Feature::AnalyticBlur`; both must agree. These are the opt-in
/// `analytic_*` variations (see `defs/analytic_blurs.rs`), NOT the
/// stochastic originals `blur` / `gaussian_blur`.
pub fn is_analytic_blur(name: &str) -> bool {
    matches!(name, "analytic_blur" | "analytic_gaussian_blur")
}

/// Draw one random offset (variation-output space, weight 1) for the named
/// analytic-blur variation, using `rng` as the uniform-[0,1) source. The
/// distribution must match the variation's WGSL body exactly; the specific
/// RNG sequence does not matter (we sample the distribution, not a seed).
///
/// Returns `None` if `name` is not an analytic blur.
pub fn sample_offset(name: &str, rng: &mut impl FnMut() -> f32) -> Option<(f32, f32)> {
    match name {
        // theta = rand*2π; r = rand;  (uniform radius → areal density ∝ 1/r)
        "analytic_blur" => {
            let theta = rng() * TWO_PI;
            let r = rng();
            Some((r * theta.cos(), r * theta.sin()))
        }
        // theta = rand*2π; r = (Σ4 rand) − 2;  (Irwin-Hall(4) bell radius)
        "analytic_gaussian_blur" => {
            let theta = rng() * TWO_PI;
            let r = rng() + rng() + rng() + rng() - 2.0;
            Some((r * theta.cos(), r * theta.sin()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A tiny deterministic LCG so the test doesn't pull in rand.
    fn lcg(state: &mut u64) -> f32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*state >> 33) as f32) / (1u64 << 31) as f32
    }

    #[test]
    fn analytic_blur_offsets_are_mean_zero_isotropic() {
        // Both blur variations are centered (mean 0) and isotropic with
        // variance 1/6 per axis at weight 1 — the kernel builder relies on
        // this. Verify by Monte-Carlo.
        for name in ["analytic_blur", "analytic_gaussian_blur"] {
            let mut s = 0x1234_5678u64;
            let mut rng = || lcg(&mut s);
            let (mut sx, mut sy, mut sxx, mut syy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let n = 200_000;
            for _ in 0..n {
                let (x, y) = sample_offset(name, &mut rng).unwrap();
                sx += x as f64; sy += y as f64;
                sxx += (x * x) as f64; syy += (y * y) as f64;
            }
            let nf = n as f64;
            let (mx, my) = (sx / nf, sy / nf);
            let (vx, vy) = (sxx / nf, syy / nf);
            assert!(mx.abs() < 0.01 && my.abs() < 0.01, "{name}: mean not ~0: ({mx},{my})");
            // Var per axis ≈ 1/6 ≈ 0.1667 for both.
            assert!((vx - 1.0 / 6.0).abs() < 0.01, "{name}: var_x {vx} != 1/6");
            assert!((vy - 1.0 / 6.0).abs() < 0.01, "{name}: var_y {vy} != 1/6");
        }
    }

    #[test]
    fn non_analytic_blur_returns_none() {
        let mut s = 1u64;
        let mut rng = || lcg(&mut s);
        assert!(sample_offset("radial_blur", &mut rng).is_none());
        assert!(sample_offset("blur", &mut rng).is_none()); // stochastic original
        assert!(!is_analytic_blur("radial_blur"));
        assert!(!is_analytic_blur("blur")); // original stays stochastic
        assert!(is_analytic_blur("analytic_blur"));
    }
}
