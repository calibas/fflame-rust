//! Cylinder targeting — stage 2 of
//! [flame-deep-zoom.md](../../docs/projects/flame-deep-zoom.md).
//!
//! Stage 1 biases the chaos game's transform selection and corrects
//! with a likelihood ratio. Its mechanism is exact and its payoff was
//! not there ([§11](../../docs/projects/flame-deep-zoom.md)): a fixed
//! per-transform bias is still a polynomial share of the orbit, so it
//! moves a constant that the weight's own variance swamps. This is
//! the stage where the asymptotics change.
//!
//! # The identity
//!
//! The invariant measure satisfies `μ = Σ_i p_i (S_i)_* μ`. Unrolled
//! along any **complete antichain** `A` — a prefix-free set of words
//! that every infinite path passes through exactly once —
//!
//! ```text
//! μ = Σ_{a ∈ A} p_a · (S_a)_* μ,     p_a = ∏ p(symbol)
//! ```
//!
//! with `S_a = S_{a_k} ∘ … ∘ S_{a_1}` for a word stored oldest-first,
//! which is the order the chaos game applies them in.
//!
//! Restricted to a viewport `V`, every word whose image misses `V`
//! contributes nothing, so
//!
//! ```text
//! μ|_V = Σ_{a ∈ A_V} p_a · (S_a)_* μ |_V,   A_V = {a ∈ A : S_a(B) ∩ V ≠ ∅}
//! ```
//!
//! **That is exact, not an approximation.** Sampling `a` from `A_V`
//! with probability `p_a / P(A_V)`, drawing `x ~ μ` by running the
//! true chaos game, and plotting `S_a(x)` with weight `P(A_V)`
//! therefore has expectation `μ|_V` — for any `A_V` built this way,
//! however loose the bound that built it. A loose bound admits words
//! whose image does not really reach `V`; their samples land outside
//! and are simply not seen. Efficiency, never correctness.
//!
//! # Why this is different from stage 1
//!
//! The weight is `P(A_V)` for **every** sample — one number for the
//! whole view, not a product accumulated along an orbit. There is no
//! window, no epoch, and no weight variance at all. And every sample
//! lands in the viewport by construction, where the unbiased game's
//! share falls off polynomially with the zoom. The cost is the
//! prefix's length, which grows like `log(1/zoom)`.
//!
//! # What this module does not do
//!
//! **Affine transforms only, and no xaos.** Both are deliberate and
//! both are about the bound rather than the identity:
//!
//! - the image bound needs a Lipschitz constant per map. For an
//!   affine one it is `σ_max` of the 2×2, exact and free. For a
//!   variation it is the deep-zoom plan's §7 item 1, the shared piece
//!   the escape-time plan also wants, and it is not built. Without it
//!   a nonlinear map has no honest bound and an enumeration built on
//!   a guessed one would be wrong rather than loose;
//! - under xaos the first symbol's probability is conditioned on
//!   whatever transform the burn-in ended on, so the sampling table
//!   is per-predecessor rather than one table. The identity holds
//!   unchanged; the bookkeeping does not, and it can follow.
//!
//! [`plan`](Cylinders::plan) reports which of these turned a flame
//! away, so the panel can say so rather than silently rendering the
//! ordinary way.

use crate::scene::transforms::Flame;
use crate::scene::ifs_analysis::Affine2;

/// How far a word may be expanded before the enumeration gives up.
///
/// A word's image shrinks geometrically, so the depth needed is
/// `log(R/V)/log(1/λ)` — about 60 at a zoom of 2^40 with a
/// contraction of a half. The cap is a guard against a flame whose
/// contraction is so weak that the enumeration would run away, not a
/// budget: a word that reaches it is kept as it is, which is sound
/// (its image is merely larger than the viewport, so its samples
/// spread wider than they need to).
pub const MAX_DEPTH: usize = 96;

/// How many words the enumeration may keep.
///
/// A viewport is covered by a handful of cylinders at its own scale,
/// so this is not normally approached. It bounds the pathological
/// case — a viewport straddling many pieces of a set with a large
/// branching factor — where the right answer is to render the
/// ordinary way rather than to enumerate ten thousand words.
pub const MAX_WORDS: usize = 4096;

/// Why a flame cannot be cylinder-targeted.
#[derive(Debug, Clone, PartialEq)]
pub enum NoCylinders {
    /// A transform is not affine, so it has no exact image bound and
    /// no Lipschitz constant is available yet (the deep-zoom plan's
    /// §7 item 1). Names the first one.
    NotAffine(usize),
    /// The flame carries a non-trivial xaos matrix: the first
    /// symbol's probability is conditional on the burn-in's last
    /// transform, which this table does not carry.
    Xaos,
    /// A transform expands (σ_max ≥ 1), so a word's image does not
    /// shrink and the enumeration has no stopping rule.
    NotContractive(usize),
    /// The flame has no transforms, or every weight is zero.
    Empty,
    /// No word's image reaches the viewport: the view is off the
    /// attractor entirely, and there is nothing to target.
    ViewIsEmpty,
    /// The enumeration hit [`MAX_WORDS`] — the viewport straddles too
    /// many pieces for targeting to be worth it.
    TooManyWords(usize),
}

/// The viewport, in world coordinates.
///
/// `world_to_pixel` is `(p − pan) · zoom` mapped so the SHORTER
/// screen axis spans ±2, so the view is the rectangle of half-extent
/// `2/zoom` in that axis and `2·aspect/zoom` in the other, centred on
/// the pan. Carried as a centre and a half-diagonal because every
/// test here is against a disc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub centre: [f64; 2],
    /// Half the diagonal: the radius of a disc containing the frame.
    pub radius: f64,
}

impl View {
    /// The view a config frames, from its zoom, pan and aspect.
    pub fn of(zoom: f64, pan: [f64; 2], width: u32, height: u32) -> Self {
        let short = width.min(height).max(1) as f64;
        let hx = 2.0 / zoom * (width as f64 / short);
        let hy = 2.0 / zoom * (height as f64 / short);
        Self { centre: pan, radius: (hx * hx + hy * hy).sqrt() }
    }
}

/// One enumerated word: the transforms to force, oldest first, and
/// the probability of the chaos game having chosen them.
#[derive(Debug, Clone, PartialEq)]
pub struct Cylinder {
    /// The symbols in the order the chaos game applies them, so a
    /// replay is a forward loop. The composed map is
    /// `S_{last} ∘ … ∘ S_{first}`.
    pub word: Vec<u32>,
    /// `p_a = ∏ p(symbol)`, the true probability of this word.
    pub prob: f64,
    /// Where the ball lands under this word, and how big it is — the
    /// bound the enumeration stopped on.
    pub centre: [f64; 2],
    pub radius: f64,
}

/// The enumerated antichain, restricted to the words that reach the
/// view.
#[derive(Debug, Clone, PartialEq)]
pub struct Cylinders {
    pub words: Vec<Cylinder>,
    /// `P(A_V) = Σ p_a` over the kept words: the weight every forced
    /// sample carries, and the share of the invariant measure the
    /// viewport holds.
    pub mass: f64,
    /// The deepest word kept, which is what the prefix costs per
    /// plotted sample.
    pub depth: usize,
}

impl Cylinders {
    /// How much work one plotted sample costs against the unbiased
    /// game's, as a ratio of useful samples per map application.
    ///
    /// The unbiased game applies one map per iteration and a
    /// `mass` fraction of its samples land in view. Forcing applies
    /// `depth` maps plus one free-orbit step and every sample lands
    /// in view, so the ratio is `1 / (mass · (depth + 1))`.
    ///
    /// This is the number the whole stage is for, and it grows
    /// without bound as the view shrinks: `mass` falls like a power
    /// of the zoom while `depth` grows like its logarithm.
    pub fn speedup(&self) -> f64 {
        if self.mass <= 0.0 {
            return f64::INFINITY;
        }
        1.0 / (self.mass * (self.depth as f64 + 1.0))
    }

    /// Enumerate the words whose image reaches `view`, or say why the
    /// flame cannot be targeted.
    pub fn plan(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
        view: View,
    ) -> Result<Self, NoCylinders> {
        let n = flame.transforms.len();
        if n == 0 {
            return Err(NoCylinders::Empty);
        }
        if flame.has_xaos() {
            return Err(NoCylinders::Xaos);
        }

        // Every transform as an affine, with its selection
        // probability and its Lipschitz constant. `σ_max` of the 2×2
        // is EXACT for an affine: the image of a disc of radius r is
        // an ellipse inside a disc of radius `σ_max · r`, so the
        // bound is tight in the worst direction and never wrong.
        let mut maps: Vec<(Affine2, f64, f64)> = Vec::with_capacity(n);
        let mut total_w = 0.0f64;
        for (i, t) in flame.transforms.iter().enumerate() {
            let a = crate::scene::ifs_analysis::transform_affine_2d(t, registry)
                .map_err(|_| NoCylinders::NotAffine(i))?;
            let (_, hi) = a.singular_values();
            if !(hi < 1.0) {
                return Err(NoCylinders::NotContractive(i));
            }
            let w = (t.weight as f64).max(0.0);
            total_w += w;
            maps.push((a, w, hi));
        }
        if !(total_w > 0.0) {
            return Err(NoCylinders::Empty);
        }

        // The ball every map sends into itself: the enumeration's
        // root, and what `S_a(B)` is the image of.
        let ifs = crate::scene::ifs_analysis::analyse_2d(flame, registry)
            .map_err(|_| NoCylinders::NotAffine(0))?;
        let (root_c, root_r) = (ifs.ball.centre, ifs.ball.radius);

        // Breadth-first, because the frontier is then ordered by
        // depth and the antichain comes out sorted — which makes the
        // completeness check below a statement about levels rather
        // than about a traversal.
        struct Node {
            word: Vec<u32>,
            prob: f64,
            centre: [f64; 2],
            radius: f64,
        }
        let mut frontier = vec![Node {
            word: Vec::new(),
            prob: 1.0,
            centre: root_c,
            radius: root_r,
        }];
        let mut kept: Vec<Cylinder> = Vec::new();

        for _depth in 0..MAX_DEPTH {
            if frontier.is_empty() {
                break;
            }
            let mut next: Vec<Node> = Vec::new();
            for node in frontier.drain(..) {
                for (i, (a, w, lip)) in maps.iter().enumerate() {
                    if !(*w > 0.0) {
                        continue;
                    }
                    let centre = a.apply(node.centre);
                    let radius = node.radius * lip;
                    // Disjoint from the view: this word and every
                    // word extending it miss, because a child's image
                    // is a SUBSET of its parent's. Dropping it keeps
                    // the antichain complete -- the word is still a
                    // member of `A`, it simply contributes nothing to
                    // `V` and so is never sampled.
                    let d = ((centre[0] - view.centre[0]).powi(2)
                        + (centre[1] - view.centre[1]).powi(2))
                    .sqrt();
                    if d > radius + view.radius {
                        continue;
                    }
                    let mut word = node.word.clone();
                    word.push(i as u32);
                    let prob = node.prob * (*w / total_w);
                    // Small enough: the image fits the view, so
                    // forcing this word puts a sample in the frame
                    // and subdividing further would only lengthen the
                    // prefix. This is where the antichain is cut.
                    if radius <= view.radius {
                        kept.push(Cylinder { word, prob, centre, radius });
                        if kept.len() > MAX_WORDS {
                            return Err(NoCylinders::TooManyWords(kept.len()));
                        }
                    } else {
                        next.push(Node { word, prob, centre, radius });
                    }
                }
            }
            frontier = next;
        }
        // Anything still on the frontier hit the depth cap. Keeping
        // it is sound: its image is larger than the view, so its
        // samples spread wider than they need to, which costs
        // efficiency and not correctness.
        for node in frontier {
            kept.push(Cylinder {
                word: node.word,
                prob: node.prob,
                centre: node.centre,
                radius: node.radius,
            });
        }

        if kept.is_empty() {
            return Err(NoCylinders::ViewIsEmpty);
        }
        let mass: f64 = kept.iter().map(|c| c.prob).sum();
        let depth = kept.iter().map(|c| c.word.len()).max().unwrap_or(0);
        Ok(Self { words: kept, mass, depth })
    }
}

/// How many floats one packed word occupies: the composed affine
/// (six), the colour fold's two coefficients, the cumulative
/// probability, and three to round the stride to a `vec4` multiple.
pub const WORD_FLOATS: usize = 12;

/// Pack the enumeration for the kernel.
///
/// **A word becomes ONE affine, not a sequence of symbols.** Every
/// map is affine here, so `S_a = S_{a_k} ∘ … ∘ S_{a_1}` composes to a
/// single 2×2 and a translation on the CPU — so forcing a prefix of
/// eighteen transforms costs the kernel one matrix multiply, not
/// eighteen. That is the deep-zoom plan's stage 3 note arriving
/// early: "compose the camera zoom with the (contracting) forced
/// prefix at f64 on the CPU into one well-conditioned map".
///
/// The COLOUR folds the same way. flam3's rule is
/// `c ← c·h + g` per transform with `h = (1+s)/2` and
/// `g = colour·(1−s)/2`, an affine map of `c`, so a whole word is
/// `c ← c·H + G` with
///
/// ```text
/// H = ∏ h_j          G = Σ_i g_i · ∏_{j>i} h_j
/// ```
///
/// the inner product running over the maps applied AFTER `i`, since
/// those are the ones that still act on `g_i`. Two numbers per word,
/// and the plotted colour is exact rather than approximated.
///
/// Layout, `WORD_FLOATS` per word:
///
/// ```text
/// 0..4   m00, m01, m10, m11      the composed 2×2
/// 4..6   t0, t1                  its translation
/// 6..8   H, G                    the colour fold
/// 8      cdf                     cumulative p_a / mass, ascending
/// 9..12  spare (stride)
/// ```
///
/// The CDF is cumulative and its last entry is exactly 1, so the
/// kernel's search cannot fall off the end however the floats
/// rounded.
pub fn pack(cyl: &Cylinders, flame: &Flame, registry: &crate::variations::VariationRegistry) -> Vec<f32> {
    let maps: Vec<Affine2> = flame
        .transforms
        .iter()
        .map(|t| {
            crate::scene::ifs_analysis::transform_affine_2d(t, registry)
                .unwrap_or(Affine2 { m: [[1.0, 0.0], [0.0, 1.0]], t: [0.0, 0.0] })
        })
        .collect();

    let mut out = vec![0.0f32; cyl.words.len() * WORD_FLOATS];
    let mut acc = 0.0f64;
    for (w, c) in cyl.words.iter().enumerate() {
        // Compose oldest-first: the word is stored in the order the
        // chaos game applies it, so each new map goes on the OUTSIDE.
        let mut m = Affine2 { m: [[1.0, 0.0], [0.0, 1.0]], t: [0.0, 0.0] };
        let mut h_prod = 1.0f64;
        let mut g_acc = 0.0f64;
        for &sym in &c.word {
            let i = sym as usize;
            m = maps[i].then_after(&m);
            let t = &flame.transforms[i];
            let s = t.color_speed as f64;
            let h = (1.0 + s) * 0.5;
            let g = t.color as f64 * (1.0 - s) * 0.5;
            // `g_acc` is the sum so far; this map acts on all of it,
            // then adds its own term.
            g_acc = g_acc * h + g;
            h_prod *= h;
        }
        acc += c.prob / cyl.mass.max(f64::MIN_POSITIVE);
        let base = w * WORD_FLOATS;
        out[base] = m.m[0][0] as f32;
        out[base + 1] = m.m[0][1] as f32;
        out[base + 2] = m.m[1][0] as f32;
        out[base + 3] = m.m[1][1] as f32;
        out[base + 4] = m.t[0] as f32;
        out[base + 5] = m.t[1] as f32;
        out[base + 6] = h_prod as f32;
        out[base + 7] = g_acc as f32;
        out[base + 8] = acc as f32;
    }
    // The last entry is exactly one, whatever the sum rounded to, so
    // a uniform draw always finds a word.
    if let Some(last) = cyl.words.len().checked_sub(1) {
        out[last * WORD_FLOATS + 8] = 1.0;
    }
    out
}

/// The gates that need a GPU: a targeted render is the untargeted
/// render, and it gets there with far fewer wasted samples.
#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::config::FractalConfig;
    use crate::scene::transforms::Transform;

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
        let al = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
        limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
        limits.max_buffer_size = al.max_buffer_size;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cylinder test"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            ..Default::default()
        }))
        .expect("device")
    }

    fn gasket_config() -> FractalConfig {
        let mut cfg = FractalConfig::default();
        cfg.flame.transforms.clear();
        for (i, (e, f)) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)].into_iter().enumerate() {
            let mut t = Transform::default();
            t.a = 0.5;
            t.d = 0.5;
            t.e = e;
            t.f = f;
            t.weight = 1.0;
            // NOT `i / 2`, which gives transform 0 the colour 0.0.
            // flam3's colour rule drives the coordinate toward the
            // colour of whatever transform ran last, so a word of many
            // consecutive S0s -- which is exactly what a deep zoom
            // toward S0's fixed point selects for -- lands at 0.0, the
            // BLACK end of the palette. The render then has full
            // density everywhere and is invisible anyway, which reads
            // exactly like starvation and is not (see
            // `auto_exposure_makes_a_deep_view_visible`).
            t.color = 0.5 + i as f32 / 4.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            cfg.flame.transforms.push(t);
        }
        cfg.deterministic_rng = true;
        // The origin is S0's fixed point, so the view is on the set at
        // every scale.
        cfg.pan_x = 0.0;
        cfg.pan_y = 0.0;
        cfg
    }

    fn render_out(cfg: &FractalConfig, n: u32, iters: u64) -> crate::renderer::RenderOutput {
        let (device, queue) = device();
        let job = crate::renderer::RenderJob::new(cfg, n, n).with_iterations(iters);
        pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            job,
            &mut crate::renderer::NoProgress,
        ))
        .expect("render")
    }

    fn render(cfg: &FractalConfig, n: u32, iters: u64) -> Vec<u8> {
        render_out(cfg, n, iters).rgba_data
    }

    /// What share of the gasket's chaos game a zoomed view holds, on
    /// the CPU: the number the GPU counters are supposed to agree
    /// with. Same three maps, same equal weights, a square frame of
    /// half-extent `2/zoom` about the origin.
    fn cpu_coverage(zoom: f64, pan: f64, n: usize) -> f64 {
        let maps = [[0.0f64, 0.0], [0.5, 0.0], [0.25, 0.5]];
        let mut st = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 11) as f64 / (1u64 << 53) as f64
        };
        let half = 2.0 / zoom;
        let (mut p, mut hits) = ([0.0f64, 0.0], 0usize);
        for i in 0..n + 200 {
            let m = maps[((rnd() * 3.0) as usize).min(2)];
            p = [0.5 * p[0] + m[0], 0.5 * p[1] + m[1]];
            if i >= 200 && (p[0] - pan).abs() <= half && (p[1] - pan).abs() <= half {
                hits += 1;
            }
        }
        hits as f64 / n as f64
    }

    /// Auto exposure makes a deep view visible, and the number it uses
    /// is the real one.
    ///
    /// The shipped tone map normalises by `total_iters / pixel_count`,
    /// which assumes the frame holds all the work. A zoomed view does
    /// not: most of the attractor is off-screen, so the samples that
    /// DID land get divided by a count dominated by ones that did not,
    /// and the picture fades as the zoom deepens.
    ///
    /// Two things are asserted, because the feature is worth nothing
    /// unless both hold:
    ///
    /// - the measured coverage matches an independent CPU chaos game,
    ///   so the counters mean what they claim;
    /// - a view that has gone dark without it is exposed with it.
    ///
    /// Measured on a gasket at a generic point of the set, 96x96,
    /// 64M iterations:
    ///
    /// ```text
    ///   zoom   coverage gpu / cpu      max off / on   mean off / on
    ///   2^2     7.22e-1 /  7.22e-1        234 / 252   0.0499 / 0.0539
    ///   2^4     1.12e-1 /  1.11e-1        143 / 235   0.0322 / 0.0539
    ///   2^6     8.28e-3 /  8.25e-3        101 / 255   0.0132 / 0.0394
    ///   2^8     9.02e-4 /  9.03e-4         60 / 255   0.0080 / 0.0393
    ///   2^10    1.04e-4 /  1.03e-4         37 / 255   0.0047 / 0.0374
    ///   2^12    8.28e-6 /  8.22e-6         28 / 255   0.0015 / 0.0176
    /// ```
    ///
    /// **A GENERIC point of the set, deliberately.** Aimed at `S₀`'s
    /// fixed point instead, the deep view is all-black for a reason
    /// that has nothing to do with exposure: the cylinder a corner
    /// zoom selects is the all-`S₀` word, flam3's colour rule walks
    /// the colour coordinate to transform 0's colour, and at the
    /// fixture's original `0.0` that is the black end of the palette.
    /// Full density, no light. That is what
    /// `gasket_config`'s colours are now chosen to avoid, and it is
    /// worth knowing because it mimics starvation exactly -- max
    /// channel zero, unmoved by any exposure -- while being a
    /// completely different fault.
    #[test]
    #[ignore = "needs a GPU"]
    fn auto_exposure_makes_a_deep_view_visible() {
        const N: u32 = 96;
        const ITERS: u64 = 64_000_000;
        // A generic point of the set rather than a fixed point of one
        // of the maps, so the view keeps sampling the whole attractor
        // as it descends.
        const PAN: f32 = 0.25;

        let stat = |rgba: &[u8]| -> (u8, f64) {
            let mx = rgba.chunks(4).map(|p| p[0].max(p[1]).max(p[2])).max().unwrap_or(0);
            let sum: u64 = rgba.chunks(4).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum();
            (mx, sum as f64 / 765.0 / (N * N) as f64)
        };

        println!("  zoom   coverage gpu / cpu      max off / on   mean off / on");
        let mut rescued = 0;
        for zoom_pow in [2i32, 4, 6, 8, 10, 12] {
            let mut off = gasket_config();
            off.zoom = 2f32.powi(zoom_pow);
            off.pan_x = PAN;
            off.pan_y = PAN;
            let mut on = off.clone();
            on.auto_exposure = true;

            let r_off = render_out(&off, N, ITERS);
            let r_on = render_out(&on, N, ITERS);
            let (mx_off, mean_off) = stat(&r_off.rgba_data);
            let (mx_on, mean_on) = stat(&r_on.rgba_data);
            let cpu = cpu_coverage(2f64.powi(zoom_pow), PAN as f64, 4_000_000);

            println!(
                "  2^{zoom_pow:<4} {:>9.2e} / {cpu:>9.2e}     {mx_off:>4} / {mx_on:<4}   {mean_off:.4} / {mean_on:.4}",
                r_on.frame_coverage
            );

            // Off must measure nothing: the counters are not compiled
            // in, so the fraction stays at its 1.0 identity.
            assert_eq!(
                r_off.frame_coverage, 1.0,
                "2^{zoom_pow}: a render with auto exposure off reported a coverage -- the \
                 counters are running when the flag says they are not"
            );
            assert!(
                r_on.frame_coverage <= 1.0,
                "2^{zoom_pow}: coverage {:.2e} exceeds 1 -- more attempts landed than were made",
                r_on.frame_coverage
            );

            // The measurement has to be the real one. Loose, because
            // the two use different RNGs and different burn-ins; this
            // is catching an off-by-a-factor, not a rounding
            // difference. Only where the CPU estimate itself has
            // enough hits to be worth comparing against.
            if cpu * 4_000_000.0 > 1000.0 {
                let ratio = r_on.frame_coverage as f64 / cpu;
                assert!(
                    (0.8..1.25).contains(&ratio),
                    "2^{zoom_pow}: GPU coverage {:.2e} against an independent CPU chaos game's \
                     {cpu:.2e} ({ratio:.2}x) -- the counters are not measuring what they claim",
                    r_on.frame_coverage
                );
            }

            // The rescue: a view the shipped normalisation has taken
            // well below full scale comes back to it.
            if mx_off < 96 && mx_on > 224 && mean_on > mean_off * 2.0 {
                rescued += 1;
            }
        }

        assert!(
            rescued >= 3,
            "auto exposure rescued {rescued} of the deep zooms -- either the views are no \
             longer being dimmed by the iteration-count normalisation, or the measured \
             coverage is not reaching the tone map"
        );
    }

    /// A targeted render is the untargeted render — the same picture,
    /// from a fraction of the samples.
    ///
    /// This is the estimator's claim, end to end: sampling a word
    /// with probability `p_a/P(A_V)`, carrying a point of the free
    /// orbit through it and plotting there has expectation `μ|_V`, so
    /// the picture must be the one the ordinary chaos game draws.
    ///
    /// Compared as a PICTURE rather than per pixel — the two draw
    /// from the same distribution by different routes and each has
    /// its own noise — so: which pixels are lit, and how bright they
    /// are on average. The untargeted reference gets sixteen times
    /// the iterations, because at these zooms it is starved and a
    /// fair-budget comparison would be against noise rather than
    /// against the answer.
    ///
    /// Measured on a gasket at `S₀`'s fixed point:
    ///
    /// ```text
    ///   zoom   P(A_V)    speedup   lit ref / tgt   overlap   brightness
    ///   2^2    1.00e0        0.5    594 /  603     100.0%   0.514 / 0.508
    ///   2^4    1.11e-1       3.0    595 /  603      99.8%   0.266 / 0.263
    ///   2^6    1.23e-2      16.2    600 /  594      99.0%   0.153 / 0.154
    ///   2^8    1.37e-3     104.1    596 /  594      99.7%   0.093 / 0.093
    ///   2^10   1.52e-4     729.0    588 /  594      99.5%   0.056 / 0.057
    /// ```
    ///
    /// The same picture at seven hundred times the rate.
    ///
    /// **Why it stops at 2^10: the REFERENCE gives out, not the
    /// target.** At 2^12 the unbiased render lights 252 pixels to the
    /// targeted one's 378 and the overlap falls to 78% — the targeted
    /// render is drawing structure its starved reference never
    /// reaches, which is the direction the whole stage exists to
    /// produce and is also exactly what makes it unverifiable. There
    /// is no comparison past the point where nothing else can draw
    /// the picture.
    ///
    /// **An earlier version of this gate stopped at 2^4 and blamed
    /// the tone map. That was wrong.** The renders past 2^4 came out
    /// with max channel zero, unmoved by four thousand times the
    /// exposure, which reads exactly like starvation — and it was the
    /// PALETTE. A zoom toward `S₀`'s fixed point selects the all-`S₀`
    /// word, flam3's colour rule walks the colour coordinate to
    /// transform 0's colour, and the fixture gave transform 0 the
    /// colour `0.0`: the black end of the palette. Full density, no
    /// light. `gasket_config` now starts its colours at 0.5, and six
    /// zoom levels that were thought to be out of reach were there
    /// the whole time. The iteration-count normalisation IS a real
    /// limit on deep views — that is what `auto_exposure` addresses —
    /// but it was not what made this gate dark.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_targeted_render_is_the_untargeted_render() {
        const N: u32 = 96;
        let stats = |rgba: &[u8]| -> (Vec<bool>, f64) {
            let lit: Vec<bool> = rgba
                .chunks(4)
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .collect();
            let (mut acc, mut n) = (0.0f64, 0.0f64);
            for (p, l) in rgba.chunks(4).zip(&lit) {
                if !*l {
                    continue;
                }
                acc += (p[0] as f64 + p[1] as f64 + p[2] as f64) / 765.0;
                n += 1.0;
            }
            (lit, if n > 0.0 { acc / n } else { 0.0 })
        };

        let reg = crate::variations::global_registry();
        println!("  zoom   P(A_V)    speedup   lit ref / tgt   overlap   brightness");
        for zoom_pow in [2i32, 4, 6, 8, 10] {
            let mut base = gasket_config();
            base.zoom = 2f32.powi(zoom_pow);
            let plan = Cylinders::plan(
                &base.flame,
                &reg,
                View::of(base.zoom as f64, [base.pan_x as f64, base.pan_y as f64], N, N),
            )
            .expect("a gasket is affine");
            let mut tgt = base.clone();
            tgt.cylinder_targeting = true;

            let (lit_r, bright_r) = stats(&render(&base, N, 64_000_000));
            let (lit_t, bright_t) = stats(&render(&tgt, N, 4_000_000));

            let nr = lit_r.iter().filter(|b| **b).count();
            let nt = lit_t.iter().filter(|b| **b).count();
            let both = lit_r.iter().zip(&lit_t).filter(|(a, b)| **a && **b).count();
            let overlap = both as f64 / nr.max(1) as f64;
            println!(
                "  2^{zoom_pow:<4} {:.2e}  {:>7.1}   {nr:>4} / {nt:>4}   {:>6.1}%   {bright_r:.3} / {bright_t:.3}",
                plan.mass,
                plan.speedup(),
                overlap * 100.0
            );

            assert!(nr > 100 && nt > 100, "2^{zoom_pow}: {nr} / {nt} lit -- nothing to compare");
            assert!(
                overlap > 0.95,
                "2^{zoom_pow}: the targeted render covers only {:.1}% of the reference's lit \
                 pixels -- it is drawing a different set, so the forced prefix is not the \
                 word the enumeration named",
                overlap * 100.0
            );
            assert!(
                (bright_t / bright_r - 1.0).abs() < 0.2,
                "2^{zoom_pow}: the targeted render is {:.3}x the reference's brightness -- the \
                 iteration count is not being inflated by 1/P(A_V), so the deposit's weight is \
                 wrong",
                bright_t / bright_r
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::Transform;
    use crate::variations::global_registry;

    fn affine(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, w: f32) -> Transform {
        let mut t = Transform::default();
        t.a = a;
        t.b = b;
        t.c = c;
        t.d = d;
        t.e = e;
        t.f = f;
        t.weight = w;
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("linear", 1.0);
        t
    }

    fn flame_of(ts: Vec<Transform>) -> Flame {
        let mut f = Flame::default();
        f.transforms = ts;
        f
    }

    /// A point ON the attractor: `S₀(p) = p/2` fixes the origin, so
    /// it is in the set at every scale. A generic point is not — the
    /// gasket is measure zero, and centring on (0.25, 0.25) made the
    /// view empty past 2^16, which is what found this.
    const ON_SET: [f64; 2] = [0.0, 0.0];

    fn gasket() -> Flame {
        flame_of(vec![
            affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 1.0),
            affine(0.5, 0.0, 0.0, 0.5, 0.5, 0.0, 1.0),
            affine(0.5, 0.0, 0.0, 0.5, 0.25, 0.5, 1.0),
        ])
    }

    /// A chaos-game sample of the attractor, for checking that what
    /// the enumeration claims about the measure is true of the set.
    fn sample(flame: &Flame, n: usize) -> Vec<[f64; 2]> {
        let reg = global_registry();
        let maps: Vec<(Affine2, f64)> = flame
            .transforms
            .iter()
            .map(|t| {
                (
                    crate::scene::ifs_analysis::transform_affine_2d(t, &reg).expect("affine"),
                    t.weight as f64,
                )
            })
            .collect();
        let total: f64 = maps.iter().map(|m| m.1).sum();
        let mut st = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = [0.0f64, 0.0];
        let mut out = Vec::with_capacity(n);
        for i in 0..n + 200 {
            let u = rnd() * total;
            let mut acc = 0.0;
            let mut k = maps.len() - 1;
            for (j, m) in maps.iter().enumerate() {
                acc += m.1;
                if u <= acc {
                    k = j;
                    break;
                }
            }
            p = maps[k].0.apply(p);
            if i >= 200 {
                out.push(p);
            }
        }
        out
    }

    /// The identity the stage rests on: the kept words' probability
    /// is the share of the invariant measure the viewport holds.
    ///
    /// `P(A_V) = Σ p_a` over the words whose image reaches the view,
    /// and the chaos game's own share of samples landing in the view
    /// has to agree. It is an upper bound rather than an equality --
    /// a word is kept when its image OVERLAPS the view, and an
    /// overlapping image may put only part of its measure inside --
    /// so this checks that the sampled share is under it and of the
    /// right order.
    #[test]
    fn the_kept_mass_bounds_the_measure_in_view() {
        let reg = global_registry();
        let flame = gasket();
        let pts = sample(&flame, 400_000);
        println!("  zoom   words  depth   P(A_V)    sampled    ratio   speedup");
        for zoom in [2.0f64, 4.0, 8.0, 16.0, 32.0, 64.0] {
            let view = View::of(zoom, ON_SET, 96, 96);
            let cyl = Cylinders::plan(&flame, &reg, view).expect("a gasket is affine");
            // The chaos game's own share of the view, over the disc
            // the enumeration tested against.
            let inside = pts
                .iter()
                .filter(|p| {
                    (p[0] - view.centre[0]).hypot(p[1] - view.centre[1]) <= view.radius
                })
                .count();
            let sampled = inside as f64 / pts.len() as f64;
            println!(
                "  {zoom:<6} {:<6} {:<6} {:.3e}  {sampled:.3e}  {:>6.2}  {:>7.1}x",
                cyl.words.len(),
                cyl.depth,
                cyl.mass,
                cyl.mass / sampled.max(1e-12),
                cyl.speedup()
            );
            assert!(
                cyl.mass >= sampled * 0.95,
                "at 2^{zoom} the kept mass {:.3e} is under the sampled share {sampled:.3e} -- \
                 the enumeration is missing words the chaos game reaches, which would leave \
                 part of the view unrendered",
                cyl.mass
            );
            assert!(
                cyl.mass <= sampled * 12.0 + 1e-6,
                "at 2^{zoom} the kept mass {:.3e} is {:.1}x the sampled share -- the bound has \
                 gone so loose that most forced samples would miss the view",
                cyl.mass,
                cyl.mass / sampled.max(1e-12)
            );
        }
    }

    /// The antichain is PREFIX-FREE, which is what makes the unrolled
    /// identity a sum over disjoint events rather than a double
    /// count.
    ///
    /// If one kept word were a prefix of another, the orbit passing
    /// through the shorter would also pass through the longer and its
    /// measure would be counted twice.
    #[test]
    fn the_kept_words_are_prefix_free() {
        let reg = global_registry();
        for zoom in [2.0f64, 8.0, 32.0] {
            let cyl = Cylinders::plan(&gasket(), &reg, View::of(zoom, ON_SET, 96, 96))
                .expect("affine");
            for (i, a) in cyl.words.iter().enumerate() {
                for (j, b) in cyl.words.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    assert!(
                        !b.word.starts_with(&a.word),
                        "at 2^{zoom} word {:?} is a prefix of {:?}, so the measure through it \
                         is counted twice",
                        a.word,
                        b.word
                    );
                }
            }
        }
    }

    /// A word's stored image is the image its own composed map
    /// produces, and that image really does contain where the
    /// attractor's points go.
    ///
    /// The bound is `σ_max` per step, which for an affine map is
    /// exact in the worst direction — so a point of the ball has to
    /// land inside, and the check is against the chaos-game sample
    /// pushed through the same word.
    #[test]
    fn a_words_image_contains_where_its_points_land() {
        let reg = global_registry();
        let flame = gasket();
        let maps: Vec<Affine2> = flame
            .transforms
            .iter()
            .map(|t| crate::scene::ifs_analysis::transform_affine_2d(t, &reg).expect("affine"))
            .collect();
        let pts = sample(&flame, 20_000);
        let cyl = Cylinders::plan(&flame, &reg, View::of(16.0, ON_SET, 96, 96))
            .expect("affine");
        let mut checked = 0usize;
        for c in &cyl.words {
            for p in pts.iter().step_by(37) {
                // The word is stored oldest-first, which is the order
                // the chaos game applies it in.
                let mut q = *p;
                for &s in &c.word {
                    q = maps[s as usize].apply(q);
                }
                let d = (q[0] - c.centre[0]).hypot(q[1] - c.centre[1]);
                assert!(
                    d <= c.radius * 1.0000001,
                    "word {:?}: a point of the attractor lands {d:.6e} from the image centre, \
                     outside the claimed radius {:.6e}",
                    c.word,
                    c.radius
                );
                checked += 1;
            }
        }
        assert!(checked > 500, "only {checked} points checked");
    }

    /// Every kept word's image really does reach the view, and its
    /// radius really is under the view's — the two stopping rules,
    /// asserted rather than assumed.
    #[test]
    fn every_kept_word_reaches_the_view_and_fits_it() {
        let reg = global_registry();
        for zoom in [4.0f64, 16.0, 64.0] {
            let view = View::of(zoom, ON_SET, 96, 96);
            let cyl = Cylinders::plan(&gasket(), &reg, view).expect("affine");
            for c in &cyl.words {
                let d = (c.centre[0] - view.centre[0]).hypot(c.centre[1] - view.centre[1]);
                assert!(
                    d <= c.radius + view.radius,
                    "at 2^{zoom} word {:?} was kept but its image is {d:.4e} away, past \
                     {:.4e} + {:.4e}",
                    c.word,
                    c.radius,
                    view.radius
                );
                assert!(
                    c.radius <= view.radius || c.word.len() >= MAX_DEPTH,
                    "at 2^{zoom} word {:?} was cut at radius {:.4e} against a view of {:.4e} \
                     without reaching the depth cap",
                    c.word,
                    c.radius,
                    view.radius
                );
            }
        }
    }

    /// What the stage is for, as a number: the deeper the view, the
    /// more the forced prefix is worth.
    ///
    /// `speedup` is `1 / (mass · (depth + 1))` — every sample lands
    /// in view at a cost of `depth + 1` map applications, against the
    /// unbiased game's one application of which a `mass` fraction is
    /// useful. The point is the SHAPE: `mass` falls like a power of
    /// the zoom while `depth` grows like its logarithm, so the ratio
    /// grows without bound.
    ///
    /// Measured on the gasket at the origin — `S₀`'s fixed point, so
    /// the view is on the set at every scale:
    ///
    /// ```text
    ///   zoom   words  depth    P(A_V)    speedup
    ///   2^1        3      1   1.00e0        0.5x
    ///   2^3        1      1   3.33e-1       1.5x
    ///   2^6        1      4   1.23e-2      16.2x
    ///   2^9        1      7   4.57e-4     273.4x
    ///   2^12       1     10   1.69e-5     5,368.1x
    ///   2^16       1     14   2.09e-7   318,864.6x
    ///   2^20       1     18   2.58e-9  20,390,552x
    /// ```
    ///
    /// Roughly doubling per octave, which is `1/mass` growing like
    /// `zoom^D` against `depth` growing like `log zoom`. **Below 2^2
    /// it is a LOSS** — the whole attractor fits the view, every
    /// sample is already useful, and the prefix is pure overhead.
    /// That is the right answer and the reason the renderer has to be
    /// able to decline: targeting is for depth, and at no depth there
    /// is nothing to target.
    ///
    /// Only ONE word survives from 2^3 on, because a view that small
    /// sits inside a single cylinder. That is the ideal case and it
    /// is the common one: the enumeration's branching is transient,
    /// dying out as soon as the view stops straddling pieces.
    #[test]
    fn the_speedup_grows_with_the_zoom() {
        let reg = global_registry();
        let flame = gasket();
        println!("  zoom   words  depth    P(A_V)    speedup");
        let mut prev = 0.0f64;
        let mut first_win = None;
        for zoom_pow in [1i32, 3, 6, 9, 12, 16, 20] {
            let zoom = 2f64.powi(zoom_pow);
            let view = View::of(zoom, ON_SET, 96, 96);
            let cyl = Cylinders::plan(&flame, &reg, view).expect("affine");
            let up = cyl.speedup();
            println!(
                "  2^{zoom_pow:<4} {:<6} {:<6} {:.2e}  {up:>10.1}x",
                cyl.words.len(),
                cyl.depth,
                cyl.mass
            );
            // The depth must track the zoom: a prefix that stopped
            // growing would mean the enumeration had stopped
            // resolving, and the speedup would be an artefact of the
            // mass alone.
            assert!(
                cyl.depth >= (zoom_pow as usize).saturating_sub(2),
                "at 2^{zoom_pow} the deepest word is only {} symbols -- the enumeration is not \
                 following the zoom",
                cyl.depth
            );
            if zoom_pow > 1 {
                assert!(
                    up > prev,
                    "at 2^{zoom_pow} the speedup {up:.2} did not beat the previous {prev:.2} -- \
                     a targeting whose value does not rise with depth is not answering \
                     starvation"
                );
            }
            if first_win.is_none() && up > 1.0 {
                first_win = Some(zoom_pow);
            }
            prev = up;
        }
        assert_eq!(
            first_win,
            Some(3),
            "targeting starts paying at a different zoom than it did -- below it the prefix is \
             pure overhead, and the renderer's decision to decline is keyed on that"
        );
        assert!(
            prev > 1e6,
            "by 2^20 the speedup is only {prev:.1e}x -- the asymptotics are not what this \
             stage exists for"
        );
    }

    /// The packed affine IS the word replayed, and the packed colour
    /// fold IS the colour folded per transform.
    ///
    /// This is the error-prone part of the stage: a composition taken
    /// in the wrong order still produces a plausible picture, of a
    /// different flame. So both are checked against a direct replay
    /// rather than against a derivation — the point, pushed through
    /// the maps one at a time, against the one matrix the kernel
    /// will use.
    ///
    /// The colour is checked the same way because it folds the same
    /// way: flam3's `c ← c·h + g` per transform is an affine map of
    /// `c`, so a word is `c ← c·H + G`, and getting `G`'s inner
    /// product over the WRONG end of the word is the same class of
    /// mistake with the same plausible-looking result.
    #[test]
    fn the_packed_word_is_the_word_replayed() {
        let reg = global_registry();
        let mut flame = gasket();
        // Distinct colours and a non-zero speed, so a fold taken the
        // wrong way round cannot come out right by symmetry.
        for (i, t) in flame.transforms.iter_mut().enumerate() {
            t.color = [0.1f32, 0.55, 0.9][i];
            t.color_speed = [0.0f32, 0.3, 0.6][i];
        }
        let maps: Vec<Affine2> = flame
            .transforms
            .iter()
            .map(|t| crate::scene::ifs_analysis::transform_affine_2d(t, &reg).expect("affine"))
            .collect();

        let probes = [[0.3f64, 0.2], [-0.4, 0.7], [0.0, 0.0], [0.62, 0.11]];
        let mut checked = 0usize;
        // Shallow, where the view straddles pieces and there are
        // several short words, through deep, where there is one word
        // of eighteen symbols -- a composition order that is wrong
        // shows up at length and a CDF that is wrong shows up at
        // breadth.
        for zoom_pow in [1i32, 2, 4, 8, 16, 20] {
        let cyl = Cylinders::plan(&flame, &reg, View::of(2f64.powi(zoom_pow), ON_SET, 96, 96))
            .expect("affine");
        let packed = pack(&cyl, &flame, &reg);
        assert_eq!(packed.len(), cyl.words.len() * WORD_FLOATS);

        for (w, c) in cyl.words.iter().enumerate() {
            let b = w * WORD_FLOATS;
            let m = Affine2 {
                m: [
                    [packed[b] as f64, packed[b + 1] as f64],
                    [packed[b + 2] as f64, packed[b + 3] as f64],
                ],
                t: [packed[b + 4] as f64, packed[b + 5] as f64],
            };
            let (big_h, big_g) = (packed[b + 6] as f64, packed[b + 7] as f64);

            for p in probes {
                // Replay: apply the word's maps one at a time, in the
                // order the chaos game would.
                let mut q = p;
                for &sym in &c.word {
                    q = maps[sym as usize].apply(q);
                }
                let got = m.apply(p);
                let e = (got[0] - q[0]).hypot(got[1] - q[1]);
                assert!(
                    e < 1e-6,
                    "word {:?}: the composed affine puts {p:?} at {got:?} where the replay \
                     puts it at {q:?} ({e:.2e})",
                    c.word
                );
                checked += 1;
            }

            for c0 in [0.0f64, 0.25, 0.5, 1.0] {
                let mut col = c0;
                for &sym in &c.word {
                    let t = &flame.transforms[sym as usize];
                    let s = t.color_speed as f64;
                    col = col * ((1.0 + s) * 0.5) + t.color as f64 * (1.0 - s) * 0.5;
                }
                let got = c0 * big_h + big_g;
                assert!(
                    (got - col).abs() < 1e-6,
                    "word {:?}: the folded colour from {c0} is {got} where the replay gives \
                     {col} -- H and G are over the wrong end of the word",
                    c.word
                );
                checked += 1;
            }
        }
        // The CDF is ascending and ends at exactly one, so a uniform
        // draw always finds a word however the floats rounded.
        let mut prev = 0.0f32;
        for w in 0..cyl.words.len() {
            let v = packed[w * WORD_FLOATS + 8];
            assert!(v >= prev, "the CDF went backwards at word {w}: {v} after {prev}");
            prev = v;
        }
        assert_eq!(prev, 1.0, "the CDF ends at {prev}, so a draw near one finds nothing");
        }
        // Six zooms, one to three words each, eight comparisons a word.
        assert!(checked >= 80, "only {checked} comparisons");
    }

    /// The refusals are refusals, not silent wrong answers.
    #[test]
    fn a_flame_that_cannot_be_targeted_says_so() {
        let reg = global_registry();
        let view = View::of(8.0, ON_SET, 96, 96);

        // Nonlinear: no Lipschitz constant exists yet.
        let mut curved = gasket();
        curved.transforms[1].set_variation("spherical", 1.0);
        assert_eq!(
            Cylinders::plan(&curved, &reg, view),
            Err(NoCylinders::NotAffine(1)),
            "a spherical transform has no image bound and must be refused by index"
        );

        // Xaos: the first symbol's probability is conditional.
        let mut x = gasket();
        x.xaos = Some(vec![
            vec![1.0, 0.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);
        assert_eq!(Cylinders::plan(&x, &reg, view), Err(NoCylinders::Xaos));

        // Expanding: a word's image does not shrink, so there is no
        // stopping rule.
        let mut big = gasket();
        big.transforms[2] = affine(1.2, 0.0, 0.0, 1.2, 0.0, 0.0, 1.0);
        assert!(matches!(
            Cylinders::plan(&big, &reg, view),
            Err(NoCylinders::NotContractive(2)) | Err(NoCylinders::NotAffine(_))
        ));

        // A view nowhere near the attractor.
        let far = View::of(8.0, [40.0, 40.0], 96, 96);
        assert_eq!(Cylinders::plan(&gasket(), &reg, far), Err(NoCylinders::ViewIsEmpty));
    }
}
