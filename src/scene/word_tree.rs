//! **A plan's words as a tree** (`docs/projects/word-editing.md`): grouped
//! by their last-applied maps -- depth 1 by the last map, depth 2 by the
//! last two -- which is the tree the inverse walk grows from the view
//! inward. A branch's share of the view is the sum over its words of
//! probability times efficiency: how much of the picture it is.

use super::cylinder::Cylinders;

/// A word's share of the view.
fn share(plan: &Cylinders, i: usize) -> f64 {
    plan.words[i].prob * plan.words[i].eff
}

/// **Trim** (§4): drop the branches whose share of the view is under
/// `trim` times their largest sibling's, at every depth from the view
/// inward. Against the largest sibling, not the parent, so an even split
/// -- a power-15 julian's arms -- is never trimmed, while a sliver beside
/// a dominant branch is. `trim <= 0` keeps everything.
///
/// The kept words' mass is the tone map's scale, so what remains keeps
/// its brightness.
pub fn trim(plan: &Cylinders, trim: f64) -> Cylinders {
    trim_to(plan, trim, usize::MAX)
}

/// [`trim`], within the first `levels` levels from the view -- which is
/// how the renderer trims (`FractalConfig::cylinder_trim_levels`). The
/// largest branch at every level always clears its own bar, so a trim
/// never empties a plan.
pub fn trim_to(plan: &Cylinders, trim: f64, levels: usize) -> Cylinders {
    let trim = trim.min(1.0);
    if !(trim > 0.0) || levels == 0 || plan.words.is_empty() {
        return plan.clone();
    }
    subset(plan, &kept_to(plan, trim, levels))
}

/// The indices of the words [`trim`] keeps, in plan order.
pub fn kept(plan: &Cylinders, trim: f64) -> Vec<usize> {
    kept_to(plan, trim, usize::MAX)
}

/// [`kept`], trimming only the first `levels` levels from the view.
///
/// A word whose replays landed nothing has not been MEASURED, not been
/// measured as nothing -- the inverse walk's own rule, which forces such
/// words rather than dropping them. A branch is judged by the words in
/// it that were measured, and the rest go with it; a branch in which
/// nothing was measured is not judged, and kept. The true Grand
/// Julian's renewal words are the case: a blurred bubble's replay
/// misses at depth, so each measures zero, and they are a quarter of the
/// probability and visible glow. Ranked as zero, any trim took them.
pub fn kept_to(plan: &Cylinders, trim: f64, levels: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..plan.words.len()).collect();
    let mut out = Vec::with_capacity(idx.len());
    descend(plan, &mut idx, 0, trim, levels, &mut out);
    out.sort_unstable();
    out
}

/// The branch of the words in `idx`, which share their last `depth` maps:
/// split it by the map before those, trim, and go on into what is kept.
fn descend(plan: &Cylinders, idx: &mut [usize], depth: usize, trim: f64, levels: usize, out: &mut Vec<usize>) {
    if depth >= levels {
        out.extend_from_slice(idx);
        return;
    }
    // The map `depth` from the end, or None for a word that ends here --
    // a leaf, its own child.
    let key = |i: usize| -> Option<u32> {
        let w = &plan.words[i].word;
        (w.len() > depth).then(|| w[w.len() - 1 - depth])
    };
    idx.sort_by_key(|&i| key(i));
    let mut children: Vec<(usize, usize, f64)> = Vec::new();
    let mut start = 0;
    while start < idx.len() {
        let k = key(idx[start]);
        let mut end = start;
        let mut s = 0.0;
        while end < idx.len() && key(idx[end]) == k {
            s += share(plan, idx[end]);
            end += 1;
        }
        children.push((start, end, s));
        start = end;
    }
    let largest = children.iter().map(|c| c.2).fold(0.0f64, f64::max);
    for (a, b, s) in children {
        // Nothing in it measured: not judged (see `kept_to`).
        if !(s > 0.0) {
            out.extend_from_slice(&idx[a..b]);
            continue;
        }
        if s < trim * largest {
            continue;
        }
        if key(idx[a]).is_none() {
            out.extend_from_slice(&idx[a..b]);
        } else {
            descend(plan, &mut idx[a..b], depth + 1, trim, levels, out);
        }
    }
}

/// The plan with only the words at `keep` (plan order), its mass,
/// efficiency and depth recomputed and its references kept beside them.
pub fn subset(plan: &Cylinders, keep: &[usize]) -> Cylinders {
    let words: Vec<_> = keep.iter().map(|&i| plan.words[i].clone()).collect();
    let refs = if plan.refs.len() == plan.words.len() {
        keep.iter().map(|&i| plan.refs[i].clone()).collect()
    } else {
        plan.refs.clone()
    };
    let mass: f64 = words.iter().map(|w| w.prob).sum();
    let delivered: f64 = words.iter().map(|w| w.prob * w.eff).sum();
    Cylinders {
        depth: words.iter().map(|w| w.word.len()).max().unwrap_or(0),
        efficiency: if mass > 0.0 { delivered / mass } else { 0.0 },
        mass,
        words,
        refs,
        lost: plan.lost,
        sampling_leak: plan.sampling_leak,
        composable: plan.composable,
        view_centre: plan.view_centre,
        offset_rows: plan.offset_rows.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::cylinder::Cylinder;

    fn word(w: &[u32], prob: f64, eff: f64) -> Cylinder {
        Cylinder { word: w.to_vec(), prob, centre: [0.0; 2], radius: 1.0, seeds: Vec::new(), eff }
    }

    fn plan(words: Vec<Cylinder>) -> Cylinders {
        let mass = words.iter().map(|w| w.prob).sum();
        Cylinders {
            words,
            mass,
            lost: 0.0,
            sampling_leak: 0.0,
            efficiency: 1.0,
            depth: 3,
            composable: false,
            view_centre: [0.0; 2],
            refs: Vec::new(),
            offset_rows: Vec::new(),
        }
    }

    /// Unmeasured words (efficiency 0) go with their branch; a branch of
    /// nothing but unmeasured words is kept.
    #[test]
    fn trim_judges_a_branch_by_what_was_measured() {
        let p = plan(vec![
            word(&[7, 3], 0.40, 1.0),
            word(&[8, 3], 0.001, 1.0),
            word(&[9, 3], 0.30, 0.0),
            word(&[4], 0.20, 0.0),
            word(&[6, 1], 0.001, 1.0),
            word(&[5, 1], 0.05, 0.0),
        ]);
        // [8, 3] is a sliver beside [7, 3]; [9, 3] and [4] measured
        // nothing and stay; branch 1 is a sliver as measured, and its
        // unmeasured [5, 1] goes with it.
        assert_eq!(kept(&p, 0.1), vec![0, 2, 3]);
    }

    /// A sliver beside a dominant branch goes; an even split never does;
    /// a word that ends where its siblings go on is a branch of its own.
    #[test]
    fn trim_drops_slivers_and_keeps_even_splits() {
        // Last maps: 3 (dominant), 1 (sliver); under 3, arms 7 and 8
        // split evenly; one word is just [3].
        let p = plan(vec![
            word(&[7, 3], 0.40, 1.0),
            word(&[8, 3], 0.40, 1.0),
            word(&[3], 0.02, 0.5),
            word(&[5, 1], 0.03, 1.0),
            word(&[6, 1], 0.01, 1.0),
        ]);
        assert_eq!(kept(&p, 0.0), vec![0, 1, 2, 3, 4], "zero trim keeps all");
        // The sliver branch (share 0.04 against 0.81) goes at 0.1; the
        // lone [3] (0.01 against 0.40) goes too; the even split stays.
        assert_eq!(kept(&p, 0.1), vec![0, 1]);
        // At 0.03 the sliver branch (0.04 against 0.81) stays, with both
        // its words; the lone [3] (0.01 against 0.40) goes.
        assert_eq!(kept(&p, 0.03), vec![0, 1, 3, 4]);
        // At 0.02 everything clears its bar.
        assert_eq!(kept(&p, 0.02), vec![0, 1, 2, 3, 4]);
        let t = trim(&p, 0.1);
        assert_eq!(t.words.len(), 2);
        assert!((t.mass - 0.8).abs() < 1e-12, "the kept mass is the scale");
    }
}
