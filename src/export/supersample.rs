//! 2× supersampled export: antialiasing that works identically for 2D,
//! 3D and solid-3D because the PIPELINE NEVER KNOWS IT'S HAPPENING.
//!
//! The whole existing render (histogram → accumulate → occlusion →
//! shadows → shade → DoF → density effects → tonemap) runs at 2W×2H,
//! then the final TONEMAPPED image is box-filtered 2×2 down to the
//! requested size. Downsampling after the tonemap is the load-bearing
//! choice: the tonemap is logarithmic, so averaging histogram/
//! accumulator densities instead (mean-then-log vs log-then-mean)
//! shifts brightness in a density-dependent way — dense cores dim,
//! haze brightens — and the AA render stops matching the 1× look
//! (the failure mode of the earlier attempt at this feature).
//!
//! The downsample kernel doubles as the FIREFLY CLAMP: each output
//! pixel sees exactly its 4 source samples, so any sample whose
//! luminance towers over the quad's median is scaled down before
//! averaging. A firefly is a single lucky high-density bucket —
//! legitimate bright detail spans the quad and keeps its median high,
//! so it passes untouched.
//!
//! Cost model: iterations scale ×4 automatically (see
//! `scale_config_for_supersample`) so each 2× bucket holds the same
//! sample density as a 1× pixel — exact brightness parity with the 1×
//! render, and the downsample averages away noise instead of averaging
//! away the concave-log bias. Memory is 4× pixels; the export panel
//! budget accounts for it and the oversized-render fallbacks (tiled /
//! CPU histogram) engage as usual.

use crate::config::FractalConfig;

/// Luminance ratio over the quad median above which a sample is
/// considered a firefly and clamped down.
const FIREFLY_RATIO: f32 = 4.0;
/// Absolute luminance headroom (8-bit sum scale) added to the clamp so
/// near-black quads with ordinary speckle noise aren't crushed.
const FIREFLY_OFFSET: f32 = 64.0;

/// Per-parameter config adjustments for the 2× render.
///
/// - `filter_radius` is in HISTOGRAM PIXELS — doubles to keep the same
///   physical footprint on the final image.
/// - `max_iterations` QUADRUPLES: 4× the buckets must each hold the
///   same sample density as a 1× pixel, or the concave log tonemap
///   turns the extra per-bucket noise into a brightness LOSS (Jensen's
///   inequality — field-reported as unacceptable dimming, worst in dim
///   regions where buckets are noisiest). With ×4 iterations each 2×
///   bucket is statistically identical to a 1× pixel, so brightness
///   matches exactly and the downsample averages away noise instead of
///   averaging away bias. This IS the advertised ~4× cost of 2× AA.
///
/// Everything else is world/relative units and resolution-independent.
/// Callers must embed the ORIGINAL config in PNG metadata, not this
/// scaled copy.
pub fn scale_config_for_supersample(config: &FractalConfig) -> FractalConfig {
    let mut c = config.clone();
    c.filter_radius *= 2.0;
    c.max_iterations = c.max_iterations.saturating_mul(4);
    c
}

/// Box-filter an RGBA8 image from (2w, 2h) down to (w, h) with the
/// per-quad firefly clamp. Alpha averages untouched (it's coverage, not
/// energy); RGB samples are clamped to `FIREFLY_RATIO ×` the quad's
/// median luminance first.
pub fn downsample_2x_firefly(rgba: &[u8], out_width: u32, out_height: u32) -> Vec<u8> {
    let w2 = (out_width * 2) as usize;
    let ow = out_width as usize;
    let oh = out_height as usize;
    assert_eq!(rgba.len(), w2 * (out_height as usize * 2) * 4, "supersampled buffer size mismatch");

    let mut out = vec![0u8; ow * oh * 4];

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        out.par_chunks_mut(ow * 4)
            .enumerate()
            .for_each(|(oy, row)| downsample_row(rgba, w2, ow, oy, row));
    }
    #[cfg(target_arch = "wasm32")]
    {
        for (oy, row) in out.chunks_mut(ow * 4).enumerate() {
            downsample_row(rgba, w2, ow, oy, row);
        }
    }
    out
}

fn downsample_row(rgba: &[u8], w2: usize, ow: usize, oy: usize, row: &mut [u8]) {
    for ox in 0..ow {
        let mut samples = [[0.0f32; 4]; 4];
        let mut lums = [0.0f32; 4];
        for (i, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].iter().enumerate() {
            let sx = ox * 2 + dx;
            let sy = oy * 2 + dy;
            let base = (sy * w2 + sx) * 4;
            let s = [
                rgba[base] as f32,
                rgba[base + 1] as f32,
                rgba[base + 2] as f32,
                rgba[base + 3] as f32,
            ];
            // Quick luminance proxy (r + 2g + b, 0..1020).
            lums[i] = s[0] + 2.0 * s[1] + s[2];
            samples[i] = s;
        }
        // Median of 4 = mean of the two middle values.
        let mut sorted = lums;
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = 0.5 * (sorted[1] + sorted[2]);
        let cap = median * FIREFLY_RATIO + FIREFLY_OFFSET;

        let mut acc = [0.0f32; 4];
        for (i, s) in samples.iter().enumerate() {
            let scale = if lums[i] > cap { cap / lums[i] } else { 1.0 };
            acc[0] += s[0] * scale;
            acc[1] += s[1] * scale;
            acc[2] += s[2] * scale;
            // Alpha is coverage — never firefly-clamped.
            acc[3] += s[3];
        }
        let base = ox * 4;
        for c in 0..4 {
            row[base + c] = (acc[c] * 0.25).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_image(quads: &[[[u8; 4]; 4]], w: u32) -> Vec<u8> {
        // Lay out `quads` left to right on a (2w × 2) image.
        let w2 = (w * 2) as usize;
        let mut img = vec![0u8; w2 * 2 * 4];
        for (q, quad) in quads.iter().enumerate() {
            for (i, px) in quad.iter().enumerate() {
                let (dx, dy) = [(0, 0), (1, 0), (0, 1), (1, 1)][i];
                let base = (dy * w2 + q * 2 + dx) * 4;
                img[base..base + 4].copy_from_slice(px);
            }
        }
        img
    }

    #[test]
    fn uniform_quad_averages_exactly() {
        let img = quad_image(&[[[100, 150, 200, 255]; 4]], 1);
        let out = downsample_2x_firefly(&img, 1, 1);
        assert_eq!(&out, &[100, 150, 200, 255]);
    }

    #[test]
    fn plain_average_when_no_outlier() {
        let img = quad_image(
            &[[
                [100, 100, 100, 255],
                [120, 120, 120, 255],
                [140, 140, 140, 255],
                [160, 160, 160, 255],
            ]],
            1,
        );
        let out = downsample_2x_firefly(&img, 1, 1);
        assert_eq!(&out[..3], &[130, 130, 130]);
    }

    #[test]
    fn firefly_is_clamped() {
        // Three near-black samples + one blown-out white: without the
        // clamp the average is ~64; with it the firefly is scaled to
        // ~4× the (tiny) median + offset.
        let img = quad_image(
            &[[
                [2, 2, 2, 255],
                [2, 2, 2, 255],
                [2, 2, 2, 255],
                [255, 255, 255, 255],
            ]],
            1,
        );
        let out = downsample_2x_firefly(&img, 1, 1);
        assert!(out[0] < 25, "firefly leaked through: {}", out[0]);
        // Alpha must NOT be clamped.
        assert_eq!(out[3], 255);
    }

    #[test]
    fn legitimate_bright_detail_survives() {
        // A bright edge spanning the quad (2 bright + 2 dark) keeps its
        // median high — no clamping, plain average.
        let img = quad_image(
            &[[
                [10, 10, 10, 255],
                [10, 10, 10, 255],
                [250, 250, 250, 255],
                [250, 250, 250, 255],
            ]],
            1,
        );
        let out = downsample_2x_firefly(&img, 1, 1);
        assert_eq!(&out[..3], &[130, 130, 130]);
    }

    #[test]
    fn filter_radius_and_iterations_scale() {
        let mut c = FractalConfig::default();
        c.filter_radius = 1.5;
        c.max_iterations = 1_000_000;
        let s = scale_config_for_supersample(&c);
        assert_eq!(s.filter_radius, 3.0);
        assert_eq!(s.max_iterations, 4_000_000);
    }
}
