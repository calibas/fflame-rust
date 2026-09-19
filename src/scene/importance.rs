//! Biased transform selection and the correction that makes it
//! unbiased — stage 1 of
//! [flame-deep-zoom.md](../../docs/projects/flame-deep-zoom.md).
//!
//! The chaos game samples the invariant measure over the whole
//! attractor, so the fraction of samples landing in a deep viewport
//! falls off polynomially with the zoom: the image starves three to
//! four decades before anything numerical breaks. Boosting a
//! transform's selection weight redirects iterations toward where the
//! camera is looking — and on its own that is importance sampling
//! with the correction term dropped, which is why it works and
//! changes the picture. The attractor's SUPPORT does not depend on
//! the weights (any strictly positive weights give the same point
//! set); what changes is the density on it, and in a flame the
//! density is the image.
//!
//! The correction is the likelihood ratio of the orbit's recent
//! choices. Deposit `w = ∏ p(choice)/q(choice)` instead of 1 and the
//! rendered measure is exactly the true one, however aggressive the
//! bias.
//!
//! This module is the CPU half: it turns a bias vector into the two
//! tables the kernel reads. It knows nothing about how the vector was
//! chosen — by hand, slaved to the zoom, or from stage 2's cylinder
//! measure — which is the mechanism/policy split the plan asks for.

use crate::config::fractal_config::ImportanceSettings;
use crate::scene::transforms::Flame;

/// The packed table the shader reads at `@binding(11)`, for `n`
/// transforms:
///
/// ```text
/// [0, n)          q[i]              the BIASED selection weight
/// [n, n + n·n)    r[prev·n + i]     p(prev→i) / q(prev→i)
/// ```
///
/// `q[i]` is `bias[i] · weight[i]`, unnormalised: the kernel's
/// selection loop sums it and picks against the sum, exactly as it
/// sums `weight[i]` today, so the normaliser never has to be
/// transmitted.
///
/// The ratio is a MATRIX because under xaos both probabilities are
/// row-conditional and their row normalisers differ. Without xaos
/// every row is identical and the kernel reads row zero — which is
/// why its non-xaos arm needs no `prev` and the feature costs that
/// arm nothing but a multiply.
///
/// ### The ratio
///
/// With `x[prev][i]` the xaos entry (1 everywhere when there is no
/// xaos), the true and biased probabilities of choosing `i` after
/// `prev` are
///
/// ```text
/// p(prev→i) = w_i·x[prev][i] / Σ_j w_j·x[prev][j]
/// q(prev→i) = b_i·w_i·x[prev][i] / Σ_j b_j·w_j·x[prev][j]
/// ```
///
/// so the ratio is `(1/b_i) · (Σ_j b_j·w_j·x[prev][j]) / (Σ_j
/// w_j·x[prev][j])` — the per-transform factor undone, times the row
/// normalisers' own ratio. The `x[prev][i]` cancels, which is what
/// makes one formula serve both arms.
///
/// ### The sparsity pattern is preserved by construction
///
/// `q` is positive exactly where `w_i·x[prev][i]` is, because every
/// bias factor is strictly positive ([`ImportanceSettings::factor`]
/// forces it). Creating an edge xaos forbids, or destroying one it
/// allows, would change the attractor's support — the one thing this
/// may not do — and neither is reachable from a positive factor.
pub fn build_table(flame: &Flame, settings: &ImportanceSettings) -> Vec<f32> {
    let n = flame.transforms.len();
    if n == 0 {
        return vec![1.0];
    }
    let w: Vec<f64> = flame.transforms.iter().map(|t| t.weight.max(0.0) as f64).collect();
    let b: Vec<f64> = (0..n).map(|i| settings.factor(i)).collect();

    let mut out = vec![0.0f32; n + n * n];
    for i in 0..n {
        out[i] = (b[i] * w[i]) as f32;
    }

    // `xaos_flat()` is `None` when every entry is 1, which is the
    // common case and the one where every row comes out the same.
    let xaos = flame.xaos_flat();
    let x = |prev: usize, i: usize| -> f64 {
        match &xaos {
            Some(flat) => flat.get(prev * n + i).copied().unwrap_or(1.0) as f64,
            None => 1.0,
        }
    };

    for prev in 0..n {
        let mut zp = 0.0f64;
        let mut zq = 0.0f64;
        for j in 0..n {
            let xj = x(prev, j).max(0.0);
            zp += w[j] * xj;
            zq += b[j] * w[j] * xj;
        }
        // A row with no admissible successor is one the walk can
        // never be in, so its ratios are never read; neutral is the
        // answer that cannot corrupt anything if they are.
        let norm = if zp > 0.0 && zq > 0.0 { zq / zp } else { 1.0 };
        for i in 0..n {
            // A transform of zero weight is never chosen under either
            // distribution, so its ratio is not a number the walk
            // uses. Neutral rather than a division by zero.
            let r = if w[i] > 0.0 { norm / b[i] } else { 1.0 };
            out[n + prev * n + i] = r as f32;
        }
    }
    out
}

/// The largest `|ln r|` the table holds among transitions that can
/// actually occur — how far one choice can move the window's product.
///
/// A window of `m` choices can move the product by at most `m` times
/// this in log space, so it is what says whether a bias is usable at
/// a given window: the deposit's useful range is bounded below by the
/// histogram's own resolution, `1/color_scale`, and above by nothing
/// in particular.
///
/// Reported rather than enforced. Clamping the product would bias the
/// estimator in exactly the way it exists to avoid, and clamping the
/// BIAS to keep a window of 16 in band would leave ratios in
/// [0.84, 1.19] — too weak to redirect anything. The deposit rounds
/// stochastically instead, which keeps a sub-resolution weight alive
/// in expectation, and what is left is variance rather than bias.
pub fn max_log_ratio(flame: &Flame, settings: &ImportanceSettings) -> f64 {
    let n = flame.transforms.len();
    if n == 0 {
        return 0.0;
    }
    let table = build_table(flame, settings);
    let xaos = flame.xaos_flat();
    let mut worst = 0.0f64;
    for prev in 0..n {
        for i in 0..n {
            let reachable = flame.transforms[i].weight > 0.0
                && match &xaos {
                    Some(flat) => flat.get(prev * n + i).copied().unwrap_or(1.0) > 0.0,
                    None => true,
                };
            if !reachable {
                continue;
            }
            let r = table[n + prev * n + i] as f64;
            if r > 0.0 && r.is_finite() {
                worst = worst.max(r.ln().abs());
            }
        }
    }
    worst
}

/// The gates that need a GPU: `q ≡ p` renders what the feature off
/// renders, and a real bias renders the same measure as no bias.
#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::config::FractalConfig;
    use crate::config::fractal_config::ImportanceSettings;
    use crate::scene::transforms::Transform;

    /// A device with the flame renderer's own limits: its compute
    /// layout wants more storage buffers than the default allows.
    fn device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        let adapter_limits = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage =
            adapter_limits.max_storage_buffers_per_shader_stage;
        limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
        limits.max_buffer_size = adapter_limits.max_buffer_size;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("importance test"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            ..Default::default()
        }))
        .expect("device")
    }

    /// A Sierpinski gasket: three equal halves, an attractor that
    /// fills its triangle, and the set every other measure gate in
    /// this project uses.
    fn gasket() -> FractalConfig {
        let mut cfg = FractalConfig::default();
        cfg.flame.transforms.clear();
        for (i, (e, f)) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)].into_iter().enumerate() {
            let mut t = Transform::default();
            t.a = 0.5;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 0.5;
            t.e = e;
            t.f = f;
            t.weight = 1.0;
            t.color = i as f32 / 2.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            cfg.flame.transforms.push(t);
        }
        cfg.deterministic_rng = true;
        cfg.zoom = 1.6;
        cfg.pan_x = -0.25;
        cfg.pan_y = -0.25;
        cfg
    }

    fn render(cfg: &FractalConfig, n: u32, iters: u64) -> Vec<u8> {
        let (device, queue) = device();
        let job = crate::renderer::RenderJob::new(cfg, n, n).with_iterations(iters);
        pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            job,
            &mut crate::renderer::NoProgress,
        ))
        .expect("render")
        .rgba_data
    }

    /// `q ≡ p` is the mechanism running neutrally, and it must render
    /// what the feature off renders -- to the BIT.
    ///
    /// Every piece is exercised: the biased selection functions run,
    /// the window accumulates, the warm-up gate fires, the deposit
    /// rounds stochastically. What makes it identical is that each
    /// one is neutral at `q ≡ p` -- the ratios are exactly 1, so the
    /// product is exactly 1, so the deposit's scale is exactly 100
    /// and its fractional part is exactly 0.
    ///
    /// **Except the warm-up**, which is not neutral: it suppresses
    /// the first `m` plots of every epoch whatever the ratios are. So
    /// this runs at `window = 0`, clamped to 1 by the uploader, where
    /// the gate passes on the first iteration and the epoch resets
    /// every second one. That isolates the arithmetic, which is what
    /// the gate is about; the warm-up's own effect is measured by
    /// `the_correction_renders_the_true_measure` below, where it is
    /// on and the answer still has to be right.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_neutral_bias_renders_what_the_feature_off_renders() {
        let base = gasket();
        let mut neutral = base.clone();
        neutral.importance = ImportanceSettings { enabled: true, bias: Vec::new(), window: 0 };

        const N: u32 = 128;
        const ITERS: u64 = 4_000_000;
        let off = render(&base, N, ITERS);
        let on = render(&neutral, N, ITERS);

        let differing = off.iter().zip(&on).filter(|(a, b)| a != b).count();
        let lit = off.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).count();
        println!("  q≡p: {differing} of {} bytes differ, {lit} lit pixels", off.len());
        // A gasket is measure zero, so a thin figure is the right
        // answer: 820 of 16384 at this zoom and iteration count.
        // The bar is only "not blank".
        assert!(lit > 500, "the fixture rendered almost nothing ({lit} lit)");
        assert_eq!(
            differing, 0,
            "the mechanism is not neutral at q ≡ p: {differing} bytes differ, so a ratio, the \
             window product or the deposit's rounding is not exactly 1 where it has to be"
        );
    }

    /// PROBE: what the bias BUYS — and on this fixture it buys
    /// nothing, which is the finding.
    ///
    /// The mechanism is correct: the three gates around this one show
    /// the correction recovers 95% of an imposed bias and is
    /// bit-neutral at `q ≡ p`. This asks the separate question of
    /// whether it is WORTH anything, and the answer here is no.
    ///
    /// **It does not make a starved render brighter, and the first
    /// attempt to measure it assumed otherwise.** Counting lit pixels
    /// found a ratio of 1.01 and was right to: the correction
    /// restores the true measure exactly, so the deposited density —
    /// and so the brightness at a given exposure — is identical by
    /// construction. The extra samples landing in view each carry a
    /// proportionally smaller weight. What could improve is NOISE, so
    /// this renders each configuration twice on different RNG streams
    /// and measures how far the two disagree.
    ///
    /// Measured on the gasket at the origin — `S₀`'s fixed point, so
    /// the boosted transform is exactly the one the camera is looking
    /// at, which is the most favourable case there is — 8M iterations
    /// at 96², mean |a − b| over pixels lit in either:
    ///
    /// ```text
    ///   zoom    unbiased   biased 4x   noise ratio
    ///   2^2       0.0023      0.0137        6.10
    ///   2^4       0.0027      0.0113        4.23
    ///   2^6      nothing lit on either side
    /// ```
    ///
    /// **Four to six times NOISIER**, not quieter. The weight's own
    /// variance — the `exp(m·E_q[ln r])` spread that
    /// `what_a_bias_costs_the_window` tabulates — costs more than the
    /// redirection saves, at every zoom where the comparison can be
    /// made at all. And past 2^6 it cannot be made: the view holds so
    /// little of the measure that both sides render black, and
    /// compensating the exposure by the gasket's own dimension does
    /// not bring it back.
    ///
    /// **So stage 1 ships its mechanism with its payoff
    /// undemonstrated**, and that is worth saying plainly rather than
    /// leaving for someone to discover. Three readings of it:
    ///
    /// - a fixed per-transform bias is still a POLYNOMIAL share of
    ///   the orbit — it moves the constant, and the constant it moves
    ///   is swamped by the weight variance. Stage 2's forced prefix
    ///   changes the asymptotics instead, and it carries the prefix's
    ///   own `∏p` as the weight rather than a product of ratios, so
    ///   it has no window and no accumulated variance at all. That is
    ///   the item this measurement argues for bringing forward;
    /// - the policy may simply be wrong. A 4× boost on one transform
    ///   is a guess; the admissible-prefix measure of stage 2 is the
    ///   principled bias, and the deep-zoom plan's open question 1
    ///   says so. This fixture cannot distinguish "the mechanism does
    ///   not pay" from "nobody has chosen a good `q`";
    /// - the fixture may be too kind to the unbiased game. A gasket
    ///   is self-similar, so every neighbourhood is reachable by a
    ///   short address; a real flame's deep view may need a long and
    ///   improbable one, which is where redirection is worth most.
    ///
    /// Left as a probe rather than a gate because it asserts nothing
    /// that ought to hold. It is the demand signal the plan asked
    /// stage 1 for, and what it signals is: go to stage 2.
    #[test]
    #[ignore = "a survey; needs a GPU"]
    fn probe_what_the_bias_buys() {
        const N: u32 = 96;
        const ITERS: u64 = 8_000_000;

        // Mean disagreement between two renders of one config on
        // different RNG streams, over pixels either one lit: the
        // render's own noise and nothing else.
        let noise = |cfg: &FractalConfig| -> f64 {
            let mut c = cfg.clone();
            c.deterministic_rng = false;
            let a = render(&c, N, ITERS);
            let b = render(&c, N, ITERS);
            let (mut acc, mut n) = (0.0f64, 0.0f64);
            for (pa, pb) in a.chunks(4).zip(b.chunks(4)) {
                let sa = pa[0] as u32 + pa[1] as u32 + pa[2] as u32;
                let sb = pb[0] as u32 + pb[1] as u32 + pb[2] as u32;
                if sa <= 24 && sb <= 24 {
                    continue;
                }
                for k in 0..3 {
                    acc += (pa[k] as f64 - pb[k] as f64).abs() / 255.0;
                }
                n += 3.0;
            }
            if n < 300.0 {
                return f64::NAN;
            }
            acc / n
        };

        println!("  zoom    unbiased   biased 4x   noise ratio");
        for zoom_pow in [2.0f32, 4.0, 6.0, 8.0] {
            let mut base = gasket();
            // `world_to_pixel` is `(p − pan) · zoom`, so `pan` IS the
            // view centre: the origin, which `S₀(p) = p/2` fixes.
            base.zoom = 2f32.powf(zoom_pow);
            base.pan_x = 0.0;
            base.pan_y = 0.0;
            // A deep view holds a tiny share of the measure and the
            // tone map normalises by the whole frame's mean, so the
            // picture goes black before it goes noisy. Compensating by
            // the gasket's own dimension is an attempt to keep the
            // brightness comparable; it is not enough past 2^6, which
            // is itself part of the finding.
            let dim = 3f64.ln() / 2f64.ln();
            base.exposure *= 2f64.powf(zoom_pow as f64 * dim) as f32;

            let mut boosted = base.clone();
            boosted.importance =
                ImportanceSettings { enabled: true, bias: vec![4.0, 1.0, 1.0], window: 8 };

            let (a, b) = (noise(&base), noise(&boosted));
            if !a.is_finite() || !b.is_finite() {
                println!("  2^{:<5} nothing lit on either side", zoom_pow as u32);
                continue;
            }
            println!(
                "  2^{:<5} {a:>9.4}   {b:>9.4}   {:>11.2}",
                zoom_pow as u32,
                b / a
            );
        }
    }

    /// The claim the whole stage rests on: with the correction, a
    /// BIASED render is the same measure as an unbiased one.
    ///
    /// Not the same picture — the estimators agree in expectation,
    /// not per pixel. What is compared is the rendered COLOUR,
    /// because colour is exactly what the selection distribution
    /// controls and nothing else here is: the palette index is folded
    /// along the orbit as `c ← c·(1+s)/2 + colour(xform)·(1−s)/2`, so
    /// the image's mean colour is a weighted average over how often
    /// each transform fires. Boost one and the mean moves toward its
    /// colour. Colour also survives the tone map, which total
    /// brightness does not — the tone map normalises by
    /// `total_iters / pixel_count`, so a brightness comparison would
    /// be measuring the normalisation.
    ///
    /// **Three renders, because two would not separate two effects.**
    /// `neutral` is the feature on at `q ≡ p`: nothing is biased, so
    /// it isolates what the DEPOSIT RULE alone changes. `corrected`
    /// is the bias with its correction. `uncorrected` is the same
    /// bias folded into the weights with nothing deposited — the
    /// discovered trick, and the control that gives the tolerance its
    /// meaning, asserted to FAIL the bar the corrected render passes.
    ///
    /// Measured at a 4× boost on one of three equal transforms,
    /// 24M iterations at 96², mean colour against the unbiased truth:
    ///
    /// ```text
    ///   neutral (q ≡ p)      0.005
    ///   corrected, m = 8     0.010
    ///   uncorrected          0.207
    /// ```
    ///
    /// The correction recovers 95% of the bias, and what is left is
    /// within a hair of what the deposit rule moves on its own.
    ///
    /// **The window has an OPTIMUM, which is not what the plan
    /// says.** §2 of the deep-zoom plan gives a lower bound on `m`
    /// from contraction and treats larger as simply more correct.
    /// Measured here, larger is worse: at 24M iterations the error
    /// runs 0.010 at `m = 8`, 0.017 at 16 and 0.119 at 32, and
    /// SIXTEEN times the samples brings 32 only to 0.056 while 8 sits
    /// still at 0.010. The reason is the variance the window exists
    /// to cap: the typical product is `exp(m·E_q[ln r])`, which at
    /// this bias is 0.16 at `m = 8` and 6e-4 at `m = 32` — so by 32
    /// the typical deposit is far under the histogram's own
    /// resolution and the estimate rides on a rare heavy tail. The
    /// lower bound is a lower bound; the upper one is variance, and
    /// nothing in the plan bounds it.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_correction_renders_the_true_measure() {
        const N: u32 = 96;
        const ITERS: u64 = 24_000_000;
        let base = gasket();

        // The mean colour over LIT pixels. Unlit ones are background
        // and say nothing about the selection.
        let mean_colour = |rgba: &[u8]| -> [f64; 3] {
            let (mut acc, mut n) = ([0.0f64; 3], 0.0f64);
            for px in rgba.chunks(4) {
                if px[0] as u32 + px[1] as u32 + px[2] as u32 <= 24 {
                    continue;
                }
                for k in 0..3 {
                    acc[k] += px[k] as f64;
                }
                n += 1.0;
            }
            assert!(n > 100.0, "only {n} lit pixels to average");
            [acc[0] / n / 255.0, acc[1] / n / 255.0, acc[2] / n / 255.0]
        };
        let truth = mean_colour(&render(&base, N, ITERS));
        let err = |a: [f64; 3]| (0..3).fold(0.0f64, |m, i| m.max((a[i] - truth[i]).abs()));

        let biased = |bias: Vec<f32>, window: u32| {
            let mut c = base.clone();
            c.importance = ImportanceSettings { enabled: true, bias, window };
            mean_colour(&render(&c, N, ITERS))
        };

        // What the deposit rule alone moves, with nothing biased.
        let neutral = biased(Vec::new(), 8);
        // The correction, at the window the sweep below finds best.
        let corrected = biased(vec![4.0, 1.0, 1.0], 8);
        // The discovered trick: the same factor in the WEIGHTS.
        let mut uncorrected_cfg = base.clone();
        uncorrected_cfg.flame.transforms[0].weight = 4.0;
        let uncorrected = mean_colour(&render(&uncorrected_cfg, N, ITERS));

        println!("  true        {truth:.4?}");
        println!("  neutral     {neutral:.4?}  off by {:.4}", err(neutral));
        println!("  corrected   {corrected:.4?}  off by {:.4}", err(corrected));
        println!("  uncorrected {uncorrected:.4?}  off by {:.4}", err(uncorrected));

        // The control has teeth.
        assert!(
            err(uncorrected) > 0.05,
            "the uncorrected bias moved the mean colour by only {:.4}, so this fixture cannot \
             tell a correction from its absence",
            err(uncorrected)
        );
        // The correction recovers nearly all of it.
        assert!(
            err(corrected) < err(uncorrected) * 0.15,
            "the corrected render is off by {:.4} against the uncorrected {:.4} -- the \
             correction recovered under 85% of the bias",
            err(corrected),
            err(uncorrected)
        );
        // ...and what is left is the deposit rule's own move, not the
        // correction failing. This is the assertion with teeth: it
        // holds the RESIDUAL against a render where nothing is biased
        // at all.
        assert!(
            (err(corrected) - err(neutral)).abs() < 0.02,
            "the corrected residual {:.4} is not the deposit rule's own {:.4} -- something \
             other than the rounding is biasing the correction",
            err(corrected),
            err(neutral)
        );

        // And where the window's optimum sits, reported. Both ends
        // are real: too short and the deposited weight covers fewer
        // choices than the position needs, too long and the typical
        // product falls under the histogram's resolution.
        println!("  window sweep at a 4x bias:");
        let mut best = (u32::MAX, f64::INFINITY);
        for w in [4u32, 8, 16, 32] {
            let e = err(biased(vec![4.0, 1.0, 1.0], w));
            println!("    m = {w:>3}: off by {e:.4}");
            if e < best.1 {
                best = (w, e);
            }
        }
        assert!(
            best.0 <= 16,
            "the best window is {} -- if the optimum has moved out past 16 the variance \
             analysis above no longer describes this estimator",
            best.0
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::Transform;

    fn flame_of(weights: &[f32]) -> Flame {
        let mut f = Flame::default();
        f.transforms.clear();
        for (i, &w) in weights.iter().enumerate() {
            let mut t = Transform::default();
            t.weight = w;
            t.color = i as f32 / weights.len() as f32;
            f.transforms.push(t);
        }
        f
    }

    fn settings(bias: &[f32]) -> ImportanceSettings {
        ImportanceSettings { enabled: true, bias: bias.to_vec(), window: 16 }
    }

    /// `q ≡ p` is the identity, and the gate that says the mechanism
    /// is neutral before anything is asked of it.
    #[test]
    fn a_neutral_bias_is_the_true_weights_and_ratios_of_one() {
        let f = flame_of(&[1.0, 2.0, 0.5]);
        let t = build_table(&f, &settings(&[]));
        assert_eq!(&t[..3], &[1.0, 2.0, 0.5]);
        for r in &t[3..] {
            assert_eq!(*r, 1.0, "a neutral bias made a ratio of {r}");
        }
        assert_eq!(max_log_ratio(&f, &settings(&[])), 0.0);
    }

    /// The estimator's defining property, checked on the numbers
    /// rather than on a render: `Σ_i q_i · r_i = 1`, and more
    /// strongly `q_i · r_i = p_i` for every `i`, which is what says
    /// the deposit's expectation is the TRUE measure and not merely
    /// normalised.
    #[test]
    fn the_ratio_takes_the_biased_distribution_back_to_the_true_one() {
        for weights in [
            vec![1.0f32, 1.0, 1.0],
            vec![6.0, 1.0, 1.0],
            vec![0.3, 2.5, 1.1, 0.7],
        ] {
            for bias in [
                vec![],
                vec![4.0f32, 1.0, 1.0],
                vec![100.0, 1.0, 1.0],
                vec![0.1, 10.0, 1.0],
            ] {
                let f = flame_of(&weights);
                let n = f.transforms.len();
                let s = settings(&bias);
                let t = build_table(&f, &s);
                let zq: f64 = (0..n).map(|i| t[i] as f64).sum();
                let zp: f64 = weights.iter().map(|w| *w as f64).sum();
                for i in 0..n {
                    let q = t[i] as f64 / zq;
                    let p = weights[i] as f64 / zp;
                    let r = t[n + i] as f64;
                    assert!(
                        (q * r - p).abs() < 1e-6 * p.max(1e-9),
                        "weights {weights:?} bias {bias:?}: q·r = {} but p = {p}",
                        q * r
                    );
                }
            }
        }
    }

    /// Under xaos the ratio has to be ROW-CONDITIONAL, and a single
    /// vector cannot serve: the row normalisers differ because each
    /// row admits a different subset. This fixture is built so they
    /// differ — row 0 reaches only transform 1, row 1 reaches all
    /// three — and asserts the same `q·r = p` identity per row.
    #[test]
    fn under_xaos_each_row_gets_its_own_normaliser() {
        let mut f = flame_of(&[1.0, 2.0, 3.0]);
        f.xaos = Some(vec![
            vec![0.0, 1.0, 0.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 0.0, 1.0],
        ]);
        let s = settings(&[5.0, 1.0, 1.0]);
        let t = build_table(&f, &s);
        let n = 3usize;
        let flat = f.xaos_flat().expect("a non-trivial xaos");

        let mut rows_differ = false;
        for prev in 0..n {
            let zq: f64 = (0..n).map(|j| t[j] as f64 * flat[prev * n + j] as f64).sum();
            let zp: f64 =
                (0..n).map(|j| f.transforms[j].weight as f64 * flat[prev * n + j] as f64).sum();
            for i in 0..n {
                if flat[prev * n + i] == 0.0 {
                    continue;
                }
                let q = t[i] as f64 * flat[prev * n + i] as f64 / zq;
                let p = f.transforms[i].weight as f64 * flat[prev * n + i] as f64 / zp;
                let r = t[n + prev * n + i] as f64;
                assert!(
                    (q * r - p).abs() < 1e-6 * p.max(1e-9),
                    "row {prev}, i {i}: q·r = {} but p = {p}",
                    q * r
                );
            }
            if prev > 0 && (t[n + prev * n] - t[n]).abs() > 1e-6 {
                rows_differ = true;
            }
        }
        assert!(
            rows_differ,
            "every row came out the same, so this fixture is not testing the row conditioning"
        );
    }

    /// The support may not move. `q` must be positive exactly where
    /// the true selection is: an edge created where xaos forbids one
    /// adds points to the attractor, and an edge destroyed removes
    /// them — neither is a density change the correction could undo.
    #[test]
    fn the_bias_neither_creates_nor_destroys_an_admissible_edge() {
        let mut f = flame_of(&[1.0, 2.0, 0.0, 3.0]);
        f.xaos = Some(vec![
            vec![1.0, 0.0, 1.0, 1.0],
            vec![0.0, 1.0, 0.0, 1.0],
            vec![1.0, 1.0, 1.0, 0.0],
            vec![1.0, 1.0, 1.0, 1.0],
        ]);
        let n = 4usize;
        let flat = f.xaos_flat().expect("a non-trivial xaos");
        // Including the pathological factors `factor()` is there to
        // neutralise: zero, negative, and not a number.
        let s = settings(&[50.0, 0.02, 0.0, -1.0]);
        let t = build_table(&f, &s);
        for prev in 0..n {
            for i in 0..n {
                let truth = f.transforms[i].weight > 0.0 && flat[prev * n + i] > 0.0;
                let biased = t[i] > 0.0 && flat[prev * n + i] > 0.0;
                assert_eq!(
                    truth, biased,
                    "row {prev} → {i}: the true edge is {truth} and the biased one {biased}"
                );
            }
        }
    }

    /// The typical deposit against the worst one, which is the number
    /// that decides whether a bias is usable at a given window.
    ///
    /// The window's product can move by `m·max_log_ratio` in log
    /// space, and the histogram's own resolution is `1/color_scale` =
    /// 0.01 — so the WORST case goes under the floor even at a mild
    /// bias. Measured, over a window of 16 on three equal transforms:
    ///
    /// ```text
    ///   bias            max|ln r|   worst range         typical
    ///   [2, 1, 1]       0.405       [1.5e-3, 6.6e2]     3.9e-1
    ///   [4, 1, 1]       0.693       [1.5e-5, 6.6e4]     2.5e-2
    ///   [10, 1, 1]      1.386       [2.3e-10, 4.3e9]    2.0e-4
    ///   [100, 1, 1]     3.526       [3.1e-25, 3.2e24]   1.4e-7
    /// ```
    ///
    /// The typical column is `exp(m·E_q[ln r])`, and `E_q[ln r]` is
    /// `−KL(q‖p)`: the weight an orbit that follows the bias actually
    /// carries, as against the tail where it happened not to. Two
    /// things follow, and both are why the deposit rounds
    /// stochastically rather than truncating.
    ///
    /// **A mild bias is fine and a strong one is not, and the typical
    /// column is what says where the line is.** At `[2,1,1]` the
    /// typical deposit is 0.39 and only the rare tail dips under the
    /// resolution. At `[4,1,1]` it is 0.025, already AT the floor. By
    /// `[10,1,1]` it is 2e-4 and every sample is under it, so
    /// truncation would throw the whole render away and keep only the
    /// tail — which is the shape of the corruption the uncorrected
    /// trick produces, arriving by a different route. **A boost of
    /// about two, at a window of sixteen, is where this stops being
    /// free**, and stage 2 is what raises that ceiling: a forced
    /// prefix's weight is the prefix's own `∏p` rather than a product
    /// of ratios, so it has no window and no accumulation at all.
    ///
    /// **The floor cannot be fixed by clamping.** Clamping the
    /// product reintroduces exactly the bias the ratio exists to
    /// remove, and clamping the BIAS so a window of 16 stays inside
    /// [1/16, 16] — the plan's v1 suggestion — leaves ratios in
    /// [0.84, 1.19], which redirects nothing. Stochastic rounding
    /// keeps a sub-resolution weight alive in expectation instead, so
    /// what is left is variance rather than bias.
    #[test]
    fn what_a_bias_costs_the_window() {
        let f = flame_of(&[1.0, 1.0, 1.0]);
        let n = 3usize;
        /// The u32 histogram's own resolution: `color_scale` is 100,
        /// so a deposit below this truncates to nothing.
        const FLOOR: f64 = 0.01;
        const M: f64 = 16.0;
        println!("  bias             max|ln r|  worst range         typical");
        let mut mild_typical = 0.0;
        for bias in [
            vec![2.0f32, 1.0, 1.0],
            vec![4.0, 1.0, 1.0],
            vec![10.0, 1.0, 1.0],
            vec![100.0, 1.0, 1.0],
        ] {
            let s = settings(&bias);
            let l = max_log_ratio(&f, &s);
            let span = (M * l).exp();
            // E_q[ln r] = −KL(q‖p): the log-weight a typical orbit
            // carries per choice.
            let t = build_table(&f, &s);
            let zq: f64 = (0..n).map(|i| t[i] as f64).sum();
            let mean_log: f64 =
                (0..n).map(|i| (t[i] as f64 / zq) * (t[n + i] as f64).ln()).sum();
            let typical = (M * mean_log).exp();
            println!(
                "  {:<16} {l:.3}   [{:.1e}, {:.1e}]   {typical:.2e}",
                format!("{bias:?}"),
                1.0 / span,
                span
            );
            assert!(l > 0.0, "{bias:?} biases nothing");
            assert!(
                mean_log < 0.0,
                "{bias:?}: the typical log-weight is {mean_log}, but a likelihood ratio has \
                 mean 1 and so a NEGATIVE mean log unless it is constant"
            );
            if bias[0] == 2.0 {
                mild_typical = typical;
            }
        }

        // The worst case goes under the histogram's resolution even at
        // the mildest bias here — which is the finding, and the reason
        // the deposit is stochastic.
        let mild = max_log_ratio(&f, &settings(&[2.0, 1.0, 1.0]));
        assert!(
            (-M * mild).exp() < FLOOR,
            "a 2x boost's worst window product is {:.2e}, above the histogram's own {FLOOR} -- \
             if that is now true the stochastic deposit has less to do than this says",
            (-M * mild).exp()
        );
        // ...and the TYPICAL deposit at that bias is comfortably above
        // it, which is what makes a mild bias usable at all.
        assert!(
            mild_typical > 10.0 * FLOOR,
            "a 2x boost's typical deposit is {mild_typical:.2e}, at the histogram's own floor -- \
             the usable bias band is narrower than this test assumes"
        );
    }
}
