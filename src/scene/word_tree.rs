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

/// **Removals** (§5): pieces of the picture a user took out, each a
/// pattern of maps matched against a word's last-applied maps. Written
/// in a config as text -- `"t1a1 t1a0"`, transform and arm, in the order
/// the chaos game applies them, so the map nearest the view is last --
/// and parsed here into the walk's symbols (`transform | arm << 8`).
pub fn parse_pattern(text: &str) -> Option<Vec<u32>> {
    let pattern: Option<Vec<u32>> = text
        .split_whitespace()
        .map(|m| {
            let m = m.strip_prefix('t')?;
            let (t, arm) = match m.split_once('a') {
                Some((t, a)) => (t.parse::<u32>().ok()?, a.parse::<u32>().ok()?),
                None => (m.parse::<u32>().ok()?, 0),
            };
            (t < 256 && arm < (1 << 24)).then_some(t | arm << 8)
        })
        .collect();
    pattern.filter(|p| !p.is_empty())
}

/// A pattern's text, as [`parse_pattern`] reads it.
pub fn pattern_text(pattern: &[u32]) -> String {
    pattern.iter().map(|&s| format!("t{}a{}", s & 0xff, s >> 8)).collect::<Vec<_>>().join(" ")
}

/// A config's removals, parsed; a pattern that does not parse is left
/// out.
pub fn parse_removals(list: &[String]) -> Vec<Vec<u32>> {
    list.iter().filter_map(|t| parse_pattern(t)).collect()
}

/// Whether `word`'s piece was removed: it ends with a removed pattern.
pub fn removed(removals: &[Vec<u32>], word: &[u32]) -> bool {
    removals.iter().any(|p| word.ends_with(p))
}

/// Whether `word`'s piece HOLDS a removed one: it is a proper suffix of
/// a removed pattern, so some of the words it would be refined into are
/// removed and some not. The inverse walk refines such a word rather
/// than keeping it whole.
pub fn holds_removed(removals: &[Vec<u32>], word: &[u32]) -> bool {
    removals.iter().any(|p| p.len() > word.len() && p.ends_with(word))
}

/// The plan without the words whose piece was removed. The inverse walk
/// never makes them; this is for the planners that do not know about
/// removals, and for a plan made before a removal, until its replan.
pub fn remove(plan: &Cylinders, removals: &[Vec<u32>]) -> Cylinders {
    if removals.is_empty() {
        return plan.clone();
    }
    let keep: Vec<usize> = (0..plan.words.len()).filter(|&i| !removed(removals, &plan.words[i].word)).collect();
    if keep.len() == plan.words.len() {
        return plan.clone();
    }
    subset(plan, &keep)
}

/// **Solo** (§6): only the words whose piece is `pattern`'s -- what a
/// branch is, drawn alone.
pub fn solo(plan: &Cylinders, pattern: &[u32]) -> Cylinders {
    let keep: Vec<usize> = (0..plan.words.len()).filter(|&i| plan.words[i].word.ends_with(pattern)).collect();
    subset(plan, &keep)
}

/// A branch of a plan's word tree, for the Words panel (§6): the words
/// that share `pattern` as their last-applied maps.
#[derive(Clone, Debug)]
pub struct Branch {
    /// Its maps, in application order: the last is nearest the view.
    /// Removing the branch removes this pattern.
    pub pattern: Vec<u32>,
    /// Its share of the view: the sum of its words' probability times
    /// efficiency. Zero where nothing in it was measured.
    pub share: f64,
    /// The sum of its words' probability.
    pub prob: f64,
    pub words: usize,
    /// None of its words is drawn: trim took them all.
    pub trimmed: bool,
    /// One level further from the view, the largest share first. A word
    /// that ends at this branch counts in it but is no child.
    pub children: Vec<Branch>,
}

/// What the Words panel shows of the plan on screen (§6).
#[derive(Clone, Debug)]
pub struct Tree {
    /// The branches, from the view inward, the largest first.
    pub branches: Vec<Branch>,
    /// The whole tree's share of the view and probability: what a
    /// branch's are a fraction of.
    pub share: f64,
    pub prob: f64,
    pub words: usize,
    /// The words drawn after trim.
    pub drawn: usize,
}

impl Tree {
    /// How many levels the tree is built to. A plan of a few thousand
    /// words builds eight in a millisecond or two; the widest plans
    /// (hundreds of thousands) get fewer, since it is rebuilt with every
    /// move of the trim slider.
    fn levels_for(words: usize) -> usize {
        if words > 50_000 {
            4
        } else {
            8
        }
    }

    /// The tree of `full` -- a plan as made -- with `removals` taken out
    /// and marked by what `trim` at `trim_levels` keeps: exactly what
    /// the renderer draws from it.
    pub fn of(full: &Cylinders, removals: &[Vec<u32>], trim: f64, trim_levels: usize) -> Tree {
        let plan = remove(full, removals);
        let mut drawn = vec![false; plan.words.len()];
        let kept = if trim > 0.0 && trim_levels > 0 { kept_to(&plan, trim.min(1.0), trim_levels) } else { (0..plan.words.len()).collect() };
        for &i in &kept {
            drawn[i] = true;
        }
        Tree {
            branches: tree(&plan, &drawn, Self::levels_for(plan.words.len())),
            share: plan.words.iter().map(|w| w.prob * w.eff).sum(),
            prob: plan.mass,
            words: plan.words.len(),
            drawn: kept.len(),
        }
    }
}

/// The tree of `plan`'s words to `levels` levels from the view, each
/// level's branches the largest share first (then the most probable,
/// for branches nothing in which was measured). `drawn[i]` says whether
/// word `i` is drawn after trim.
pub fn tree(plan: &Cylinders, drawn: &[bool], levels: usize) -> Vec<Branch> {
    let mut idx: Vec<usize> = (0..plan.words.len()).collect();
    grow(plan, drawn, &mut idx, &[], levels)
}

fn grow(plan: &Cylinders, drawn: &[bool], idx: &mut [usize], suffix: &[u32], levels: usize) -> Vec<Branch> {
    if levels == 0 {
        return Vec::new();
    }
    let d = suffix.len();
    let key = |i: usize| -> Option<u32> {
        let w = &plan.words[i].word;
        (w.len() > d).then(|| w[w.len() - 1 - d])
    };
    idx.sort_by_key(|&i| key(i));
    let mut out = Vec::new();
    let mut start = 0;
    while start < idx.len() {
        let k = key(idx[start]);
        let mut end = start;
        while end < idx.len() && key(idx[end]) == k {
            end += 1;
        }
        if let Some(sym) = k {
            let group = &mut idx[start..end];
            let mut pattern = Vec::with_capacity(d + 1);
            pattern.push(sym);
            pattern.extend_from_slice(suffix);
            let children = grow(plan, drawn, group, &pattern, levels - 1);
            out.push(Branch {
                share: group.iter().map(|&i| share(plan, i)).sum(),
                prob: group.iter().map(|&i| plan.words[i].prob).sum(),
                words: group.len(),
                trimmed: group.iter().all(|&i| !drawn.get(i).copied().unwrap_or(true)),
                pattern,
                children,
            });
        }
        start = end;
    }
    out.sort_by(|a, b| b.share.total_cmp(&a.share).then(b.prob.total_cmp(&a.prob)));
    out
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

    /// A pattern reads back as it was written, arm 0 may be left out,
    /// and anything else is refused.
    #[test]
    fn patterns_round_trip() {
        let p = parse_pattern("t1a1 t1a0").expect("parses");
        assert_eq!(p, vec![1 | 1 << 8, 1]);
        assert_eq!(pattern_text(&p), "t1a1 t1a0");
        assert_eq!(parse_pattern("t3"), Some(vec![3]));
        for bad in ["", "  ", "x1", "t", "t1a", "ta0", "t300", "t1 q2"] {
            assert_eq!(parse_pattern(bad), None, "{bad:?}");
        }
    }

    /// A removal takes the words ending with it; a shorter word it ends
    /// with holds it, and is not taken.
    #[test]
    fn removals_take_words_ending_with_them() {
        let r = vec![vec![5, 1]];
        assert!(removed(&r, &[5, 1]));
        assert!(removed(&r, &[9, 5, 1]));
        assert!(!removed(&r, &[1]));
        assert!(!removed(&r, &[6, 1]));
        assert!(holds_removed(&r, &[1]));
        assert!(holds_removed(&r, &[]));
        assert!(!holds_removed(&r, &[5, 1]));
        assert!(!holds_removed(&r, &[2]));
        let p = plan(vec![word(&[5, 1], 0.1, 1.0), word(&[9, 5, 1], 0.1, 1.0), word(&[6, 1], 0.2, 1.0), word(&[1], 0.3, 1.0)]);
        let q = remove(&p, &r);
        assert_eq!(q.words.len(), 2);
        assert!((q.mass - 0.5).abs() < 1e-12);
    }

    /// The tree groups by last maps, largest share first, marks what trim
    /// took, and solo keeps one branch.
    #[test]
    fn the_tree_groups_by_last_maps() {
        let p = plan(vec![
            word(&[7, 3], 0.40, 1.0),
            word(&[8, 3], 0.30, 1.0),
            word(&[3], 0.02, 0.5),
            word(&[5, 1], 0.03, 1.0),
            word(&[6, 1], 0.01, 1.0),
        ]);
        let drawn = [true, true, true, false, false];
        let t = tree(&p, &drawn, 2);
        assert_eq!(t.iter().map(|b| b.pattern.clone()).collect::<Vec<_>>(), vec![vec![3], vec![1]]);
        assert_eq!(t[0].words, 3);
        assert!((t[0].share - 0.71).abs() < 1e-12);
        assert!(!t[0].trimmed && t[1].trimmed);
        assert_eq!(t[0].children.iter().map(|b| b.pattern.clone()).collect::<Vec<_>>(), vec![vec![7, 3], vec![8, 3]]);
        assert!(t[0].children[0].children.is_empty(), "two levels only");
        let s = solo(&p, &[3]);
        assert_eq!(s.words.len(), 3);
        assert!((s.mass - 0.72).abs() < 1e-12);
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
