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

/// Largest kernel half-extent (in pixels) we'll build. A blur whose mapped
/// support exceeds this is truncated + renormalized (and warned about); the
/// convolution cost is `(2*half+1)^2` taps per pixel, so this bounds it.
pub const MAX_KERNEL_HALF: u32 = 64;

/// Maximum offset radius (variation-output space, weight 1) — the support of
/// the offset sampler. Used host-side to pick the convolution downscale so the
/// low-res kernel half stays small. Returns 0 for non-analytic names.
pub fn max_offset_radius(name: &str) -> f32 {
    match name {
        "analytic_blur" => 1.0,           // uniform disc, r ≤ 1
        "analytic_gaussian_blur" => 2.0,  // Irwin-Hall(4) − 2 ∈ [-2, 2]
        _ => 0.0,
    }
}

/// A normalized 2D convolution kernel: an analytic-blur variation's offset
/// distribution sampled and pushed through a transform's pixel-space linear
/// map. Square, `(2*half+1)` on a side, row-major, weights sum to 1.0. A
/// `half == 0` kernel is the 1×1 identity (sub-pixel blur — no spread).
#[derive(Clone, Debug)]
pub struct BlurKernel {
    pub half: u32,
    pub weights: Vec<f32>,
}

impl BlurKernel {
    #[inline]
    pub fn size(&self) -> usize {
        (2 * self.half as usize) + 1
    }
}

/// Monte-Carlo a [`BlurKernel`] for `name` whose offsets are mapped to pixel
/// space by the 2×2 `linear` map (`[m00, m01, m10, m11]`, applied as
/// `px = m00·ox + m01·oy`, `py = m10·ox + m11·oy`). `linear` is the caller's
/// full pixel-space linear part: `world→pixel scale · variation weight ·
/// transform post-affine linear`. `samples` offsets are drawn from `rng`
/// (uniform [0,1)) and bilinearly splatted, so the kernel matches the
/// stochastic distribution by construction (anisotropy included).
///
/// Returns `None` for a non-analytic `name`. A degenerate/sub-pixel map
/// yields the 1×1 identity kernel.
pub fn build_kernel(
    name: &str,
    linear: [f32; 4],
    samples: usize,
    rng: &mut impl FnMut() -> f32,
) -> Option<BlurKernel> {
    if !is_analytic_blur(name) {
        return None;
    }
    let [m00, m01, m10, m11] = linear;
    let map = |ox: f32, oy: f32| (m00 * ox + m01 * oy, m10 * ox + m11 * oy);

    // Draw offsets once, mapping to pixel space; track the support extent.
    let mut pts: Vec<(f32, f32)> = Vec::with_capacity(samples);
    let (mut max_abs_x, mut max_abs_y) = (0.0f32, 0.0f32);
    for _ in 0..samples {
        let (ox, oy) = sample_offset(name, rng).unwrap();
        let (px, py) = map(ox, oy);
        max_abs_x = max_abs_x.max(px.abs());
        max_abs_y = max_abs_y.max(py.abs());
        pts.push((px, py));
    }

    // Half-extent covers the support (both distributions are bounded).
    // `ceil` already covers the bilinear reach of an edge sample at the
    // support (a point at extent `e` splats no further than `ceil(e)`).
    let needed = max_abs_x.max(max_abs_y).ceil() as i64;
    let half = needed.clamp(0, MAX_KERNEL_HALF as i64) as u32;
    if needed > MAX_KERNEL_HALF as i64 {
        log::warn!(
            "analytic blur '{name}': kernel support {needed}px exceeds MAX_KERNEL_HALF \
             {MAX_KERNEL_HALF}; truncating",
        );
    }
    let size = (2 * half as usize) + 1;
    let mut weights = vec![0.0f32; size * size];
    let center = half as f32;

    // Bilinear splat each mapped offset (continuous grid coord = pt + center).
    for &(px, py) in &pts {
        let gx = px + center;
        let gy = py + center;
        let x0 = gx.floor();
        let y0 = gy.floor();
        let fx = gx - x0;
        let fy = gy - y0;
        let x0 = x0 as i64;
        let y0 = y0 as i64;
        let mut add = |xi: i64, yi: i64, w: f32| {
            if xi >= 0 && yi >= 0 && (xi as usize) < size && (yi as usize) < size {
                weights[(yi as usize) * size + xi as usize] += w;
            }
        };
        add(x0, y0, (1.0 - fx) * (1.0 - fy));
        add(x0 + 1, y0, fx * (1.0 - fy));
        add(x0, y0 + 1, (1.0 - fx) * fy);
        add(x0 + 1, y0 + 1, fx * fy);
    }

    // Normalize to sum 1 (truncation/clamp may have dropped a little mass).
    let total: f32 = weights.iter().sum();
    if total > 0.0 {
        let inv = 1.0 / total;
        for w in &mut weights {
            *w *= inv;
        }
    } else {
        // Fully degenerate (shouldn't happen): fall back to identity.
        return Some(BlurKernel { half: 0, weights: vec![1.0] });
    }
    Some(BlurKernel { half, weights })
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
    fn build_kernel_covariance_matches_linear_map() {
        // The offset covariance is (1/6)·I, so the pixel-space kernel
        // covariance must be (1/6)·M·Mᵀ — anisotropy and cross-term included.
        let mut s = 0xDEAD_BEEFu64;
        let mut rng = || lcg(&mut s);
        let m = [8.0f32, 2.0, 0.0, 6.0]; // [m00, m01, m10, m11] — scale + shear
        let k = build_kernel("analytic_blur", m, 400_000, &mut rng).unwrap();
        let size = k.size();
        let c = k.half as f64;

        let mut mx = 0.0; let mut my = 0.0;
        for j in 0..size { for i in 0..size {
            let w = k.weights[j * size + i] as f64;
            mx += w * (i as f64 - c);
            my += w * (j as f64 - c);
        }}
        let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
        for j in 0..size { for i in 0..size {
            let w = k.weights[j * size + i] as f64;
            let dx = (i as f64 - c) - mx;
            let dy = (j as f64 - c) - my;
            cxx += w * dx * dx; cyy += w * dy * dy; cxy += w * dx * dy;
        }}
        // M·Mᵀ = [[68,12],[12,36]] → ×(1/6).
        let (exp_xx, exp_yy, exp_xy) = (68.0 / 6.0, 36.0 / 6.0, 12.0 / 6.0);
        assert!((cxx - exp_xx).abs() < 0.7, "cxx {cxx} vs {exp_xx}");
        assert!((cyy - exp_yy).abs() < 0.7, "cyy {cyy} vs {exp_yy}");
        assert!((cxy - exp_xy).abs() < 0.7, "cxy {cxy} vs {exp_xy}");
        let sum: f32 = k.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "kernel not normalized: {sum}");
    }

    #[test]
    fn build_kernel_subpixel_is_near_identity() {
        // A blur much smaller than a pixel yields a tiny kernel whose center
        // cell holds nearly all the weight (≈ identity, but still a faithful
        // sub-pixel spread — not forced to exact identity).
        let mut s = 7u64;
        let mut rng = || lcg(&mut s);
        let k = build_kernel("analytic_blur", [0.05, 0.0, 0.0, 0.05], 50_000, &mut rng).unwrap();
        assert!(k.half <= 1, "sub-pixel blur should be a tiny kernel, got half={}", k.half);
        let size = k.size();
        let center = k.weights[k.half as usize * size + k.half as usize];
        assert!(center > 0.85, "center weight {center} should dominate a sub-pixel kernel");
        let sum: f32 = k.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
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
