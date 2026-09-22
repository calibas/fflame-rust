//! Cylinder planning by the inverse walk.
//!
//! `docs/projects/inversive-targeting.md` §24. The forward planner in
//! [`cylinder`](super::cylinder) pushes a disc through a word's maps
//! and asks whether it meets the view; for a flame like
//! `grand-julian` that cannot work, and the reason is structural
//! rather than a matter of tightness. Its dominant map is
//! `z^(-1/2)`, which EXPANDS over the part of the plane the attractor
//! occupies, and a ball bound must take the worst derivative over the
//! ball -- so it cannot contract until the ball is already small and
//! cannot get small without contracting. Measured: the bound for the
//! measure-typical word sat at 0.2 for depths 3--5 while the true
//! cylinder was 1e-5, then died.
//!
//! The same flame, walked BACKWARDS, is the ideal case. Every map is
//! `julian ∘ rotation`, whose inverse is single-valued; for a given
//! output point only one arm of each transform can have produced it,
//! since the arms' images are disjoint sectors. Walked from eleven
//! orbit points to depth 24, the word whose cylinder contains a point
//! was UNIQUE at every depth. A deep view is reached by one word.
//!
//! So this planner pulls the VIEW back through the inverse maps
//! rather than pushing the attractor forward through the word. The
//! machinery is the one [`ifs_analysis`](super::ifs_analysis) already
//! builds for the inverse walks -- [`analyse_2d`] gives every
//! transform's inverse, its exact Jacobian by dual numbers, and one
//! map per preimage branch -- and [`ifs_estimate::estimate_measure`]
//! is the same walk with a different stop rule. This one stops where
//! the pulled-back region covers the attractor, which is where the
//! word's cylinder fits the view, and every stop is VERIFIED by
//! replaying the word forward on a sample of the attractor: the
//! fraction that lands in the frame is the word's efficiency, exact
//! for the sample and immune to the first-order region's distortion.
//!
//! **What is approximate, and what it costs.** The pulled-back region
//! is carried to first order (the composed inverse Jacobian applied
//! to the view disc), and "meets the attractor" is asked of a sample.
//! A region pruned wrongly is a HOLE in the picture -- measure the
//! view has that no forced sample delivers -- so the prune keeps a
//! margin and the picture gate is the test that counts. A cut taken
//! early is only WASTE: forced samples that land outside the frame,
//! which the efficiency reports and the kernel discards.

use super::cylinder::{sym_of, Cylinder, Cylinders, NoCylinders, View, MAX_DEPTH, MAX_WORDS};
use super::ifs_analysis::{analyse_2d, Ifs2, IfsMap, Kernel, Map2};
use super::transforms::Flame;
use crate::variations::VariationRegistry;

/// The frontier beam. Wider than family M's 96 because a node here
/// costs one inverse evaluation and one Jacobian, not a cover push;
/// narrower than "everything" because a flame whose measure does not
/// concentrate would otherwise grow the frontier like the branching
/// factor to the depth, which is the wall §15 measured.
pub const BEAM: usize = 256;

/// How many points of the attractor are sampled, once per plan.
pub const SAMPLE: usize = 20_000;

/// How many of them a cut is verified against. The verification
/// replays the word forward on each, so this times the depth is the
/// cost of one cut.
pub const VERIFY: usize = 600;

/// The fraction of replayed sample points that must land in the
/// frame for a word to be cut there rather than walked deeper.
///
/// Below this the word's cylinder still spills outside the view and
/// forcing it wastes the spill; a symbol deeper the cylinder is
/// smaller and the spill less. It is a quality knob with a monotone
/// trade -- longer words cost replay time per sample, shorter ones
/// waste samples -- and 0.9 is where a forced sample is almost always
/// a frame sample.
pub const CUT_EFFICIENCY: f64 = 0.9;

/// The margin on the first-order region before a branch is pruned:
/// the region's outer radius is multiplied by this before asking
/// whether any sample point lies within it.
///
/// **This is the soundness margin.** The composed Jacobian describes
/// the pulled-back region exactly only where it is small; where an
/// inversion has stretched it, the true region reaches further than
/// the ellipse, and pruning on the ellipse alone would drop a word
/// that does reach the view. Two is generous where the region is
/// small and costs nothing there -- the frontier stays a handful --
/// and where the region is large the prune is skipped outright (see
/// `PRUNE_SKIP`).
pub const PRUNE_SLACK: f64 = 2.0;

/// A region whose outer radius is this fraction of the attractor's
/// extent is not asked whether it meets the sample at all: a region
/// that big certainly does, and asking would cost a search over most
/// of the grid.
pub const PRUNE_SKIP: f64 = 0.25;

/// A node whose probability is below this fraction of the mass
/// already kept is dropped and charged to `lost`.
///
/// **The tail is what made a shallow view slow.** Measured at zoom
/// 1e2 on `grand-julian`: the typical word cuts by depth 6, and the
/// beam then carried 256 words of probability 1e-40 to the depth cap
/// and replayed every one of them -- fifteen seconds for `lost` of
/// 1e-38. Nothing below this floor can change the picture, and the
/// bound on what is charged is `BEAM × MAX_DEPTH × MEASURE_FLOOR`
/// of the kept mass.
pub const MEASURE_FLOOR: f64 = 1e-8;

/// The inscribed disc of the pulled-back region need only reach this
/// fraction of the way to covering the attractor before the word is
/// replayed to see whether it fits. The replay decides; this only
/// says when it is worth asking. Waiting for full first-order cover
/// walked words twice as long as the exact cut depth, because the
/// region's smallest singular value under-reads an anisotropic
/// region.
pub const COVER_TRIGGER: f64 = 0.3;

/// A node that was replayed and found not to fit is not replayed
/// again until its inscribed radius has grown by this factor.
pub const RETRY_GROWTH: f64 = 1.5;

/// The attractor, sampled and gridded, with the maps to walk it.
pub struct Backward {
    ifs: Ifs2,
    /// One entry per distinct transform: its index, its selection
    /// probability, how many forward arms it has, and the index into
    /// `ifs.maps` of one map that carries its forward.
    transforms: Vec<TransformInfo>,
    /// Per `ifs.maps` entry: the probability the chaos game selects
    /// this transform AND, for a many-valued forward, this arm. A
    /// map that is one of several preimage branches of the same
    /// transform carries the transform's whole probability, because
    /// its branches partition the preimage rather than the draw.
    prob: Vec<f64>,
    sample: Vec<[f64; 2]>,
    centre: [f64; 2],
    extent: f64,
    grid: Grid,
}

struct TransformInfo {
    index: usize,
    weight: f64,
    arms: u32,
    map: usize,
}

/// A hash grid over the sample, for "is any point within `r` of `q`".
struct Grid {
    cell: f64,
    cells: std::collections::HashMap<(i64, i64), Vec<[f64; 2]>>,
}

impl Grid {
    fn of(pts: &[[f64; 2]], cell: f64) -> Self {
        let mut cells: std::collections::HashMap<(i64, i64), Vec<[f64; 2]>> =
            std::collections::HashMap::new();
        for p in pts {
            cells.entry(Self::key(*p, cell)).or_default().push(*p);
        }
        Self { cell, cells }
    }

    fn key(p: [f64; 2], cell: f64) -> (i64, i64) {
        ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64)
    }

    /// Whether some sample point lies within `r` of `q`.
    fn any_within(&self, q: [f64; 2], r: f64) -> bool {
        let reach = (r / self.cell).ceil() as i64;
        let (cx, cy) = Self::key(q, self.cell);
        let r2 = r * r;
        for dx in -reach..=reach {
            for dy in -reach..=reach {
                if let Some(v) = self.cells.get(&(cx + dx, cy + dy)) {
                    if v.iter().any(|s| {
                        let (ex, ey) = (s[0] - q[0], s[1] - q[1]);
                        ex * ex + ey * ey <= r2
                    }) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// How many arms a map's FORWARD has -- the multiplicity of the draw
/// the kernel makes, which is what the replay has to force. A root's
/// `|n|`; everything else is single-valued going forward, whatever
/// its inverse's branch count.
fn forward_arms(map: &IfsMap<Map2>) -> u32 {
    let root = |k: &Kernel| match *k {
        Kernel::Root { n, .. } => n.unsigned_abs().max(1),
        _ => 1,
    };
    match &map.forward {
        Map2::Nonlinear(n) => root(&n.kernel),
        Map2::Sum(s) => root(&s.kernel),
        _ => 1,
    }
}

/// The forward map along arm `k`.
fn forward(map: &IfsMap<Map2>, x: [f64; 2], k: u32) -> [f64; 2] {
    match &map.forward {
        Map2::Nonlinear(n) => n.apply_branch(x, k),
        Map2::Sum(s) => s.apply_branch(x, k),
        other => other.apply(x),
    }
}

fn finite(p: [f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

/// Singular values of a 2×2, largest first.
fn singular_values(m: [[f64; 2]; 2]) -> (f64, f64) {
    let (a, b, c, d) = (m[0][0], m[0][1], m[1][0], m[1][1]);
    let s1 = a * a + b * b + c * c + d * d;
    let det = a * d - b * c;
    let disc = (s1 * s1 - 4.0 * det * det).max(0.0).sqrt();
    let hi = ((s1 + disc) / 2.0).max(0.0).sqrt();
    let lo = ((s1 - disc) / 2.0).max(0.0).sqrt();
    (hi, lo)
}

fn mat_mul(a: [[f64; 2]; 2], b: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    [
        [a[0][0] * b[0][0] + a[0][1] * b[1][0], a[0][0] * b[0][1] + a[0][1] * b[1][1]],
        [a[1][0] * b[0][0] + a[1][1] * b[1][0], a[1][0] * b[0][1] + a[1][1] * b[1][1]],
    ]
}

impl Backward {
    /// Analyse the flame and sample its attractor. `Err` names what
    /// the inverse walk cannot do for this flame.
    pub fn read(flame: &Flame, registry: &VariationRegistry) -> Result<Self, String> {
        let ifs = analyse_2d(flame, registry).map_err(|errs| {
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
        })?;
        if ifs.final_map.is_some() {
            return Err("a final transform is not pulled back yet".into());
        }
        if ifs.maps.is_empty() {
            return Err("no maps".into());
        }
        let mut transforms: Vec<TransformInfo> = Vec::new();
        for (mi, m) in ifs.maps.iter().enumerate() {
            if transforms.iter().any(|t| t.index == m.transform_index) {
                continue;
            }
            let weight = flame
                .transforms
                .get(m.transform_index)
                .map_or(0.0, |t| (t.weight as f64).max(0.0));
            transforms.push(TransformInfo {
                index: m.transform_index,
                weight,
                arms: forward_arms(m),
                map: mi,
            });
        }
        let total: f64 = transforms.iter().map(|t| t.weight).sum();
        if !(total > 0.0) {
            return Err("no weight".into());
        }
        let prob: Vec<f64> = ifs
            .maps
            .iter()
            .map(|m| {
                let t = transforms.iter().find(|t| t.index == m.transform_index).unwrap();
                t.weight / total / t.arms as f64
            })
            .collect();

        // The attractor, by the chaos game on the analysed maps. A
        // fixed stream, so two plans of one flame agree.
        let mut st = 0x9E37_79B9_7F4A_7C15u64;
        let mut lcg = move || {
            st = st
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut x = [0.31f64, 0.17];
        let burn = SAMPLE / 20;
        let mut sample = Vec::with_capacity(SAMPLE);
        for k in 0..SAMPLE + burn {
            let mut u = lcg() * total;
            let mut t = &transforms[transforms.len() - 1];
            for c in &transforms {
                if u < c.weight {
                    t = c;
                    break;
                }
                u -= c.weight;
            }
            let arm = ((lcg() * t.arms as f64) as u32).min(t.arms - 1);
            let y = forward(&ifs.maps[t.map], x, arm);
            if finite(y) && y[0].abs() < 1e12 && y[1].abs() < 1e12 {
                x = y;
            } else {
                x = [0.31, 0.17];
                continue;
            }
            if k >= burn {
                sample.push(x);
            }
        }
        if sample.len() < 1000 {
            return Err("the orbit did not settle".into());
        }
        let n = sample.len() as f64;
        let centre = [
            sample.iter().map(|p| p[0]).sum::<f64>() / n,
            sample.iter().map(|p| p[1]).sum::<f64>() / n,
        ];
        let extent = sample
            .iter()
            .map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt())
            .fold(0.0, f64::max);
        if !(extent > 0.0) || !extent.is_finite() {
            return Err("the attractor has no extent".into());
        }
        let grid = Grid::of(&sample, extent / 64.0);
        Ok(Self { ifs, transforms, prob, sample, centre, extent, grid })
    }

    /// How far the sample reaches from its centre.
    pub fn extent(&self) -> f64 {
        self.extent
    }

    /// Which arm of `map` sends `x` to `p`: the one whose forward
    /// image is nearest. A many-valued map's arms have disjoint
    /// images, so the answer is unambiguous away from a boundary and
    /// either arm is right on one.
    fn arm_of(&self, map: &IfsMap<Map2>, x: [f64; 2], p: [f64; 2]) -> Option<u32> {
        let arms = forward_arms(map);
        let mut best = None;
        let mut best_d = f64::INFINITY;
        for k in 0..arms {
            let y = forward(map, x, k);
            if !finite(y) {
                continue;
            }
            let d = (y[0] - p[0]).hypot(y[1] - p[1]);
            if d < best_d {
                best_d = d;
                best = Some(k);
            }
        }
        // An inverse that does not invert is a point with no
        // preimage on this map, whatever the arithmetic returned.
        let scale = 1.0 + p[0].abs().max(p[1].abs());
        (best_d <= 1e-6 * scale).then_some(best?)
    }

    /// Replay a word forward on the verification sample: the
    /// fraction landing in the view, and where the landed points sit.
    fn replay(&self, word: &[u32], view: View) -> (f64, [f64; 2], f64) {
        let stride = (self.sample.len() / VERIFY).max(1);
        let mut hit = 0usize;
        let mut total = 0usize;
        let mut sum = [0.0f64; 2];
        let mut landed: Vec<[f64; 2]> = Vec::new();
        for p0 in self.sample.iter().step_by(stride) {
            let mut p = *p0;
            let mut ok = true;
            for &sym in word {
                let t = super::cylinder::sym_transform(sym) as usize;
                let arm = super::cylinder::sym_arm(sym);
                let Some(info) = self.transforms.iter().find(|i| i.index == t) else {
                    ok = false;
                    break;
                };
                p = forward(&self.ifs.maps[info.map], p, arm);
                if !finite(p) {
                    ok = false;
                    break;
                }
            }
            total += 1;
            if ok && (p[0] - view.centre[0]).hypot(p[1] - view.centre[1]) <= view.radius {
                hit += 1;
                sum[0] += p[0];
                sum[1] += p[1];
                landed.push(p);
            }
        }
        if hit == 0 {
            return (0.0, view.centre, view.radius);
        }
        let c = [sum[0] / hit as f64, sum[1] / hit as f64];
        let r = landed
            .iter()
            .map(|p| (p[0] - c[0]).hypot(p[1] - c[1]))
            .fold(0.0, f64::max);
        (hit as f64 / total.max(1) as f64, c, r)
    }

    /// Plan the antichain for `view`.
    pub fn plan(&self, view: View) -> Result<Cylinders, NoCylinders> {
        struct Node {
            word: Vec<u32>,
            p: [f64; 2],
            jac: [[f64; 2]; 2],
            prob: f64,
            /// The inscribed radius at the last replay that found
            /// the word did not fit, or zero.
            tried_at: f64,
        }
        let mut frontier = vec![Node {
            word: Vec::new(),
            p: view.centre,
            jac: [[1.0, 0.0], [0.0, 1.0]],
            prob: 1.0,
            tried_at: 0.0,
        }];
        let mut kept: Vec<(Cylinder, f64)> = Vec::new();
        let mut kept_mass = 0.0f64;
        let mut lost = 0.0f64;
        let skip_r = PRUNE_SKIP * self.extent;

        for depth in 1..=MAX_DEPTH {
            if frontier.is_empty() {
                break;
            }
            let mut next: Vec<Node> = Vec::new();
            for node in frontier.drain(..) {
                for (mi, map) in self.ifs.maps.iter().enumerate() {
                    let q = map.inverse.apply(node.p);
                    if !finite(q) || q[0].abs() > 1e12 || q[1].abs() > 1e12 {
                        // No preimage on this map: the point is not in
                        // its image, so no word through it reaches
                        // the view. Nothing is lost.
                        continue;
                    }
                    let Some(ji) = map.inverse.jacobian(node.p) else {
                        continue;
                    };
                    let Some(arm) = self.arm_of(map, q, node.p) else {
                        continue;
                    };
                    let jac = mat_mul(ji, node.jac);
                    let (smax, smin) = singular_values(jac);
                    if !smax.is_finite() {
                        continue;
                    }
                    let r_out = view.radius * smax;
                    let r_in = view.radius * smin;
                    let prob = node.prob * self.prob[mi];
                    // **Prune.** The pulled-back region, to first
                    // order and with margin, holds no point of the
                    // attractor -- so the word's cylinder misses the
                    // view. Only asked while the region is small
                    // enough for first order to mean something.
                    if r_out < skip_r && !self.grid.any_within(q, PRUNE_SLACK * r_out + self.grid.cell)
                    {
                        continue;
                    }
                    // **The floor.** Below it a word cannot change
                    // the picture, and walking it to the depth cap
                    // is what made shallow views slow.
                    if prob < MEASURE_FLOOR * kept_mass {
                        lost += prob;
                        continue;
                    }
                    let mut word = Vec::with_capacity(node.word.len() + 1);
                    word.push(sym_of(map.transform_index as u32, arm));
                    word.extend_from_slice(&node.word);
                    // **Cut.** The inscribed disc of the region has
                    // grown far enough that the cylinder may fit the
                    // view; the replay is what decides, and a word
                    // that did not fit is not asked again until the
                    // region has grown.
                    let need = self.extent + (q[0] - self.centre[0]).hypot(q[1] - self.centre[1]);
                    let last = depth == MAX_DEPTH;
                    let mut tried_at = node.tried_at;
                    if last || (r_in >= COVER_TRIGGER * need && r_in >= RETRY_GROWTH * node.tried_at) {
                        let (eff, c, r) = self.replay(&word, view);
                        if eff >= CUT_EFFICIENCY || last {
                            kept_mass += prob;
                            kept.push((Cylinder { word, prob, centre: c, radius: r }, eff));
                            if kept.len() > MAX_WORDS {
                                return Err(NoCylinders::TooManyWords(kept.len()));
                            }
                            continue;
                        }
                        tried_at = r_in;
                    }
                    next.push(Node { word, p: q, jac, prob, tried_at });
                }
            }
            if next.len() > BEAM {
                next.sort_by(|a, b| b.prob.partial_cmp(&a.prob).unwrap_or(std::cmp::Ordering::Equal));
                for n in next.drain(BEAM..) {
                    lost += n.prob;
                }
            }
            frontier = next;
        }

        // Several preimage branches of one transform share a word;
        // the draw wants each word once, carrying the transform's
        // probability once, with the branches' efficiencies summed.
        kept.sort_by(|a, b| a.0.word.cmp(&b.0.word));
        let mut merged: Vec<(Cylinder, f64)> = Vec::new();
        for (c, e) in kept {
            match merged.last_mut() {
                Some((m, me)) if m.word == c.word => {
                    *me = (*me + e).min(1.0);
                    m.radius = m.radius.max(c.radius);
                }
                _ => merged.push((c, e)),
            }
        }
        if merged.is_empty() {
            return Err(NoCylinders::ViewIsEmpty);
        }
        let mass: f64 = merged.iter().map(|(c, _)| c.prob).sum();
        let delivered: f64 = merged.iter().map(|(c, e)| c.prob * e).sum();
        let depth = merged.iter().map(|(c, _)| c.word.len()).max().unwrap_or(0);
        Ok(Cylinders {
            words: merged.into_iter().map(|(c, _)| c).collect(),
            mass,
            lost,
            sampling_leak: 0.0,
            efficiency: if mass > 0.0 { delivered / mass } else { 0.0 },
            depth,
            composable: false,
            view_centre: view.centre,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **What the inverse walk plans for the family-J flames**, at
    /// every zoom the forward walk could not reach.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_the_inverse_walk_plans() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc", "random1"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let t0 = std::time::Instant::now();
            let b = match Backward::read(&cfg.flame, reg) {
                Ok(b) => b,
                Err(why) => {
                    println!("== {name}: refused — {why}");
                    continue;
                }
            };
            let read_ms = t0.elapsed().as_secs_f64() * 1e3;
            // A view on the set: a sample point well into the orbit.
            let q = b.sample[b.sample.len() * 3 / 4];
            println!(
                "== {name}: read in {read_ms:.0} ms, {} maps, extent {:.3e}, view at [{:.4}, {:.4}]",
                b.ifs.maps.len(),
                b.extent,
                q[0],
                q[1]
            );
            for zoom in [1e2f64, 1e3, 1e4, 1e6, 1e8] {
                let view = View::of(zoom, q, 512, 512);
                let t0 = std::time::Instant::now();
                let r = b.plan(view);
                let ms = t0.elapsed().as_secs_f64() * 1e3;
                match r {
                    Ok(c) => {
                        let shortest = c.words.iter().map(|w| w.word.len()).min().unwrap_or(0);
                        println!(
                            "   zoom {zoom:>6.0e} {ms:>7.1} ms  {:>4} words, depth {:>2}..{:>2}, mass {:.2e}, \
                             efficiency {:.2}, speedup {:.2e}, lost {:.2e}",
                            c.words.len(),
                            shortest,
                            c.depth,
                            c.mass,
                            c.efficiency,
                            c.speedup(),
                            c.lost
                        );
                    }
                    Err(e) => println!("   zoom {zoom:>6.0e} {ms:>7.1} ms  {e:?}"),
                }
            }
        }
    }
}
