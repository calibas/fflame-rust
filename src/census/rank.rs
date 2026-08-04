//! `rank` — the join that turns a probe diff into a worklist.
//!
//! The probe compare says *what differs* (variation × input label); the
//! census report says *what real renders feed variations*. This maps
//! every diverging probe input to its census class and asks whether the
//! corpus ever delivered that class to that variation. Output: the
//! divergences that matter, ranked; the texture that doesn't, counted.

use super::classify;
use crate::probe::{inputs::probe_inputs, report};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Reachability verdict for one (variation, input) pair.
#[derive(Debug, Clone, PartialEq)]
enum Reach {
    /// The input is (normal, normal) and the corpus exercised the
    /// variation — ordinary points are the chaos game's bread and
    /// butter; the census deliberately doesn't count them.
    Ordinary,
    /// The census observed exactly this input class arriving, with the
    /// report's frequency bucket and reproducer flame.
    Observed(String, String),
    /// The corpus exercised the variation and never delivered this
    /// class. Not proof of unreachability — proof the corpus didn't.
    Unobserved,
    /// The corpus never exercised the variation at all.
    NotExercised,
}

struct CensusData {
    exercised: HashSet<String>,
    /// (variation, class-name as rendered by the census report) →
    /// (bucket, worst flame). Both "in" and "pp" rows — a pre/post
    /// variation's inputs live in the chained table.
    observed: HashMap<(String, String), (String, String)>,
}

fn parse_census(text: &str) -> CensusData {
    let mut exercised = HashSet::new();
    let mut observed = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let (Some(who), Some(kind), Some(class), Some(bucket)) =
            (it.next(), it.next(), it.next(), it.next())
        else {
            continue;
        };
        let worst = it.next().unwrap_or("?");
        match kind {
            "use" => {
                exercised.insert(who.to_string());
            }
            "in" | "pp" => {
                observed
                    .entry((who.to_string(), class.to_string()))
                    .or_insert((bucket.to_string(), worst.to_string()));
            }
            _ => {}
        }
    }
    CensusData { exercised, observed }
}

/// The census report's rendering of a pair class — must match
/// `run::pair_name` for the join to hold; pinned by a test below.
fn pair_class_name(x: f32, y: f32) -> String {
    format!(
        "({},{})",
        super::component_class_name(classify(x)),
        super::component_class_name(classify(y))
    )
}

fn z_class_name(z: f32) -> String {
    format!("z:{}", super::component_class_name(classify(z)))
}

fn reach_for(census: &CensusData, who: &str, label: &str, dim: &str) -> Reach {
    let Some(point) = probe_inputs().iter().find(|p| p.label == label) else {
        return Reach::Unobserved; // unknown label — newer probe than census tooling
    };
    let pair = pair_class_name(point.x, point.y);
    let z = z_class_name(point.z);
    let ordinary =
        pair == "(normal,normal)" && (dim != "3d" || z == "z:normal");

    if !census.exercised.contains(who) {
        return Reach::NotExercised;
    }
    if ordinary {
        return Reach::Ordinary;
    }
    if let Some((bucket, worst)) = census.observed.get(&(who.to_string(), pair)) {
        return Reach::Observed(bucket.clone(), worst.clone());
    }
    if dim == "3d" && z != "z:normal" {
        if let Some((bucket, worst)) = census.observed.get(&(who.to_string(), z)) {
            return Reach::Observed(bucket.clone(), worst.clone());
        }
    }
    Reach::Unobserved
}

/// Severity of the worst glyph transition in a class divergence.
/// NaN-involved transitions outrank everything: a value that becomes —
/// or stops being — NaN changes control flow (bad-value recovery),
/// not just a number.
fn severity(before: &str, after: &str) -> (u32, &'static str) {
    let mut worst = (0u32, "none");
    for (a, b) in before.chars().zip(after.chars()) {
        if a == b {
            continue;
        }
        let zero = |c: char| matches!(c, '0' | 'o' | 'z');
        let inf = |c: char| matches!(c, 'I' | 'i');
        let s = if a == 'n' || b == 'n' {
            (4, "nan")
        } else if inf(a) || inf(b) {
            (3, "inf")
        } else if zero(a) != zero(b) {
            (2, "zero<->finite")
        } else {
            (1, "sign/magnitude")
        };
        if s.0 > worst.0 {
            worst = s;
        }
    }
    worst
}

pub fn run_cli(old: &Path, new: &Path, census_path: &Path) -> i32 {
    let read = |p: &Path| -> Result<report::Report, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        report::Report::parse(&text)
    };
    let (ra, rb) = match (read(old), read(new)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("rank: {e}");
            return 2;
        }
    };
    let census_text = match std::fs::read_to_string(census_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "rank: {}: {e}\n      run `variation_probe census --corpus` first",
                census_path.display()
            );
            return 2;
        }
    };
    let census = parse_census(&census_text);
    let accepted = super::accepted::Accepted::load(std::path::Path::new(
        super::accepted::DEFAULT_PATH,
    ));
    let divergences = match report::compare(&ra, &rb) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("rank: {e}");
            return 2;
        }
    };

    // Every (variation, input) from the hard signal, judged.
    struct Row {
        score: f64,
        name: String,
        dim: &'static str,
        label: String,
        sev: &'static str,
        reach: Reach,
    }
    let mut rows = Vec::new();
    let mut soft = 0usize;
    let mut accepted_sites = 0usize;
    for d in &divergences {
        match d {
            report::Divergence::Class { name, dim, at, before, after } => {
                let (sev_rank, sev) = severity(before, after);
                for label in at {
                    // Dispositioned sites come off the worklist — the file
                    // carries the reason; the count below keeps them visible.
                    if accepted.covers(name, dim, label) {
                        accepted_sites += 1;
                        continue;
                    }
                    let reach = reach_for(&census, name, label, dim);
                    let reach_w = match &reach {
                        Reach::Observed(bucket, _) => match bucket.as_str() {
                            "dominant" => 8.0,
                            "common" => 6.0,
                            "rare" => 4.0,
                            _ => 3.0,
                        },
                        Reach::Ordinary => 5.0,
                        Reach::Unobserved => 1.0,
                        Reach::NotExercised => 0.5,
                    };
                    rows.push(Row {
                        score: sev_rank as f64 * reach_w,
                        name: name.clone(),
                        dim,
                        label: label.clone(),
                        sev,
                        reach,
                    });
                }
            }
            report::Divergence::Magnitude { .. } => soft += 1,
            report::Divergence::OnlyIn { .. } => {}
        }
    }
    rows.sort_by(|a, b| b.score.total_cmp(&a.score));

    let reachable = rows
        .iter()
        .filter(|r| matches!(r.reach, Reach::Observed(..) | Reach::Ordinary))
        .count();
    let unobserved = rows.iter().filter(|r| r.reach == Reach::Unobserved).count();
    let unexercised = rows
        .iter()
        .filter(|r| r.reach == Reach::NotExercised)
        .count();

    println!(
        "# rank — {} hard divergence sites: {} REACHABLE, {} unobserved-by-corpus, {} not-exercised ({} accepted via {}; {} value-only entries ignored)",
        rows.len(),
        reachable,
        unobserved,
        unexercised,
        accepted_sites,
        super::accepted::DEFAULT_PATH,
        soft
    );
    println!(
        "{:<6} {:<24} {:<4} {:<16} {:<15} {}",
        "score", "variation", "dim", "input", "severity", "reachability"
    );
    for r in rows.iter().take(60) {
        let reach = match &r.reach {
            Reach::Ordinary => "REACHABLE (ordinary input)".to_string(),
            Reach::Observed(bucket, worst) => format!("REACHABLE ({bucket}) — {worst}"),
            Reach::Unobserved => "corpus never delivered this class".to_string(),
            Reach::NotExercised => "variation not in corpus".to_string(),
        };
        println!(
            "{:<6.1} {:<24} {:<4} {:<16} {:<15} {}",
            r.score, r.name, r.dim, r.label, r.sev, reach
        );
    }
    if rows.len() > 60 {
        println!("... {} more (same format)", rows.len() - 60);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The join is string-matched against the census report's class
    /// rendering; if `run::pair_name` changes shape, this names it.
    #[test]
    fn class_names_match_the_report_format() {
        assert_eq!(pair_class_name(1.0, 1.0), "(normal,normal)");
        assert_eq!(pair_class_name(0.0, -0.0), "(+0,-0)");
        assert_eq!(pair_class_name(1e-40, 2e32), "(subnormal,huge)");
        assert_eq!(z_class_name(f32::NAN), "z:nan");
    }

    #[test]
    fn severity_orders_nan_above_everything() {
        assert!(severity("0", "n").0 > severity("0", "p").0);
        assert!(severity("I", "p").0 > severity("p", "m").0);
        assert_eq!(severity("p0", "p0").0, 0);
    }

    #[test]
    fn census_parse_reads_use_in_and_pp_rows() {
        let c = parse_census(
            "# header\n\
             npolar                   use  (exercised)              dominant  preset:X\n\
             npolar                   in   (+0,-0)                  common    preset:X\n\
             foo                      pp   (subnormal,normal)       rare      random:3\n",
        );
        assert!(c.exercised.contains("npolar"));
        assert_eq!(
            c.observed.get(&("npolar".into(), "(+0,-0)".into())).unwrap().0,
            "common"
        );
        assert!(c.observed.contains_key(&("foo".into(), "(subnormal,normal)".into())));
    }
}
