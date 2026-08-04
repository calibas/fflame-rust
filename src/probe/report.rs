//! The committed artefact and the comparison over it.
//!
//! # Why a text format and not JSON
//!
//! The report's whole job is to be diffed — between platforms, and
//! between builds. One line per variation per dimension, fixed column
//! order, means `git diff` and `diff` both localise a change to the
//! variation that moved without any tooling at all. JSON would
//! reformat, reorder, and bury the signal in punctuation.
//!
//! # Why timings are not in it
//!
//! Timing varies by machine, by thermal state, and by run. Putting it
//! in the diffable body would mean every comparison shows hundreds of
//! changed lines, and a report that is always noisy is a report nobody
//! reads. Timings go to a separate file that is not compared line by
//! line — see [`Timings`].

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Bumped when the format or the input grid changes, because either
/// makes old reports incomparable. Comparing across a schema change is
/// refused rather than silently producing nonsense.
pub const SCHEMA: u32 = 1;

/// Which experiment a report describes.
///
/// The two reports share a format and a schema, so without this nothing
/// stops `compare base.txt sweep.txt` — which parses fine and emits
/// thousands of "present in only one" findings, every one of them
/// meaningless. Refusing the comparison is the same reflex as the schema
/// check: two files that describe different experiments should not be
/// diffed just because they happen to have the same columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Every variation at its default parameters.
    Defaults,
    /// Every parameter of every variation, one at a time.
    Sweep,
}

impl Kind {
    pub fn tag(self) -> &'static str {
        match self {
            Kind::Defaults => "defaults",
            Kind::Sweep => "sweep",
        }
    }

    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "defaults" => Some(Kind::Defaults),
            "sweep" => Some(Kind::Sweep),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Variation name.
    pub name: String,
    /// `2d` or `3d`.
    pub dim: &'static str,
    /// One glyph per output component, in input order. The hard signal.
    pub classes: String,
    /// Quantised-magnitude digest. The soft signal.
    pub digest: u64,
}

/// What the run was, so a comparison can say what it compared.
#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub app_version: String,
    pub git_hash: String,
    pub adapter: String,
    pub backend: String,
    pub driver: String,
    pub os: String,
    pub input_labels: Vec<String>,
    /// Extra header lines describing this particular report's format.
    /// The base and sweep reports share a renderer but not a column
    /// meaning, and a reader who cannot tell them apart will misread
    /// one of them.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub schema: u32,
    pub kind: Kind,
    pub meta: Meta,
    pub entries: Vec<Entry>,
    /// Variations that could not be probed at all, and why. Recorded
    /// rather than dropped: a variation silently missing from the
    /// report reads as "fine" when it may be the most interesting one.
    pub skipped: Vec<(String, String)>,
}

impl Report {
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Fractal Art Editor — variation math probe");
        let _ = writeln!(s, "# schema {}", self.schema);
        let _ = writeln!(s, "# kind {}", self.kind.tag());
        let _ = writeln!(s, "#");
        let _ = writeln!(s, "# This file is generated. See src/probe/ for what it means.");
        let _ = writeln!(
            s,
            "# Compare two of these with:  cargo run --release --bin variation_probe \\"
        );
        let _ = writeln!(s, "#                                 -- compare OLD NEW");
        let _ = writeln!(s, "#");
        let _ = writeln!(s, "# build    {} ({})", self.meta.app_version, self.meta.git_hash);
        let _ = writeln!(s, "# os       {}", self.meta.os);
        let _ = writeln!(s, "# adapter  {}", self.meta.adapter);
        let _ = writeln!(s, "# backend  {}", self.meta.backend);
        let _ = writeln!(s, "# driver   {}", self.meta.driver);
        let _ = writeln!(s, "#");
        if !self.meta.input_labels.is_empty() {
            let _ = writeln!(s, "# inputs   {}", self.meta.input_labels.join(" "));
        }
        let _ = writeln!(
            s,
            "# glyphs   0=+0  o=-0  z=near-zero  p=+  m=-  H=+huge  h=-huge  I=+inf  i=-inf  n=nan"
        );
        for note in &self.meta.notes {
            let _ = writeln!(s, "# {note}");
        }
        let _ = writeln!(s, "#");
        let _ = writeln!(s, "# NOT covered: the accumulate and tonemap passes, and how");
        let _ = writeln!(s, "# variations compose. Those are the visual regression suite's.");
        let _ = writeln!(s, "#");

        if !self.skipped.is_empty() {
            let _ = writeln!(s, "# skipped ({}):", self.skipped.len());
            for (name, why) in &self.skipped {
                let _ = writeln!(s, "#   {name}: {why}");
            }
            let _ = writeln!(s, "#");
        }

        // Entries that produced nothing. Some are correct — a Z-only
        // variation contributes (0, 0) in 2D by design — but a line of
        // zeros carries no signal either way, and a reader comparing two
        // reports would otherwise count them as agreement. Naming them
        // is the difference between "646 variations verified" and "638
        // verified, 8 that produce no output to verify".
        let silent: Vec<&Entry> = self
            .entries
            .iter()
            // `.` is the sweep's "class not present" marker, so a mask
            // of only zeros and dots means the same thing a base entry
            // of only zeros does. Requiring at least one real glyph
            // keeps an entirely-empty mask from counting as a result.
            .filter(|e| {
                e.classes.chars().any(|c| c != crate::probe::classify::ABSENT)
                    && e.classes
                        .chars()
                        .all(|c| c == '0' || c == 'o' || c == crate::probe::classify::ABSENT)
            })
            .collect();
        if !silent.is_empty() {
            let _ = writeln!(s, "# no observable output ({}) — these lines carry no signal:", silent.len());
            let mut line = String::from("#  ");
            for e in &silent {
                let item = format!(" {}({})", e.name, e.dim);
                if line.len() + item.len() > 76 {
                    let _ = writeln!(s, "{line}");
                    line = String::from("#  ");
                }
                line.push_str(&item);
            }
            if line.trim() != "#" {
                let _ = writeln!(s, "{line}");
            }
            let _ = writeln!(s, "#");
        }

        let _ = writeln!(s, "# name dim classes digest");
        for e in &self.entries {
            let _ = writeln!(s, "{} {} {} {:016x}", e.name, e.dim, e.classes, e.digest);
        }
        s
    }

    pub fn parse(text: &str) -> Result<Report, String> {
        // Establish that this is a probe report before trying to read
        // entries out of it. Otherwise an unrelated text file fails on
        // whichever of its lines happens to look most like an entry,
        // and the error talks about dimensions rather than about the
        // file being the wrong file.
        if !text.lines().any(|l| l.trim_start_matches('#').trim().starts_with("schema ")) {
            return Err("no schema line — is this a probe report?".into());
        }

        let mut schema = 0;
        let mut kind = Kind::Defaults;
        let mut meta = Meta::default();
        let mut entries = Vec::new();
        let mut skipped = Vec::new();

        // Skipped entries are recognised by being indented under their
        // header rather than by shape alone, so a comment that happens
        // to contain a colon cannot be mistaken for one.
        let mut in_skipped = false;

        for line in text.lines() {
            let line = line.trim_end();
            if let Some(raw) = line.strip_prefix('#') {
                let rest = raw.trim();
                if in_skipped && raw.starts_with("   ") {
                    if let Some((name, why)) = rest.split_once(':') {
                        skipped.push((name.trim().to_string(), why.trim().to_string()));
                    }
                    continue;
                }
                in_skipped = rest.starts_with("skipped (");

                if let Some(v) = rest.strip_prefix("schema ") {
                    schema = v.trim().parse().map_err(|_| format!("bad schema: {v}"))?;
                } else if let Some(v) = rest.strip_prefix("kind ") {
                    kind = Kind::from_tag(v.trim())
                        .ok_or_else(|| format!("unknown report kind `{}`", v.trim()))?;
                } else if let Some(v) = rest.strip_prefix("adapter ") {
                    meta.adapter = v.trim().to_string();
                } else if let Some(v) = rest.strip_prefix("backend ") {
                    meta.backend = v.trim().to_string();
                } else if let Some(v) = rest.strip_prefix("driver ") {
                    meta.driver = v.trim().to_string();
                } else if let Some(v) = rest.strip_prefix("os ") {
                    meta.os = v.trim().to_string();
                } else if let Some(v) = rest.strip_prefix("build ") {
                    meta.app_version = v.trim().to_string();
                } else if let Some(v) = rest.strip_prefix("inputs ") {
                    meta.input_labels = v.split_whitespace().map(str::to_string).collect();
                }
                continue;
            }
            if line.is_empty() {
                continue;
            }

            let mut f = line.split_whitespace();
            let (Some(name), Some(dim), Some(classes), Some(digest)) =
                (f.next(), f.next(), f.next(), f.next())
            else {
                return Err(format!("malformed entry: {line}"));
            };
            entries.push(Entry {
                name: name.to_string(),
                dim: match dim {
                    "2d" => "2d",
                    "3d" => "3d",
                    other => return Err(format!("unknown dimension `{other}`")),
                },
                classes: classes.to_string(),
                digest: u64::from_str_radix(digest, 16)
                    .map_err(|_| format!("bad digest `{digest}`"))?,
            });
        }

        if schema == 0 {
            return Err("no schema line — is this a probe report?".into());
        }
        Ok(Report {
            schema,
            kind,
            meta,
            entries,
            skipped,
        })
    }
}

/// How two reports differ for one variation.
#[derive(Debug, Clone, PartialEq)]
pub enum Divergence {
    /// The hard signal moved: a value changed kind. Not tolerance —
    /// something behaves differently.
    Class {
        name: String,
        dim: &'static str,
        /// The input labels whose class changed, so the finding names
        /// the point rather than just the variation.
        at: Vec<String>,
        before: String,
        after: String,
    },
    /// Only the soft signal moved: same kinds, different magnitudes
    /// beyond the quantisation tolerance.
    Magnitude { name: String, dim: &'static str },
    /// Present in one report and not the other.
    OnlyIn {
        name: String,
        dim: &'static str,
        which: &'static str,
    },
}

impl Divergence {
    /// Whether this is the kind that means a real behavioural
    /// difference, as opposed to a number that moved.
    pub fn is_hard(&self) -> bool {
        !matches!(self, Divergence::Magnitude { .. })
    }
}

/// Compare two reports, hard signal first.
///
/// `labels` names the input points; it comes from whichever report has
/// them, so a class difference can be attributed to a specific input.
pub fn compare(a: &Report, b: &Report) -> Result<Vec<Divergence>, String> {
    if a.schema != b.schema {
        return Err(format!(
            "schema {} vs {} — the input grid or format changed between these \
             two runs, so they describe different experiments and comparing \
             them would be meaningless. Regenerate both.",
            a.schema, b.schema
        ));
    }

    if a.kind != b.kind {
        return Err(format!(
            "one report is `{}` and the other is `{}` — these describe different \
             experiments and share only their columns. Compare a defaults report \
             with a defaults report, and a sweep with a sweep.",
            a.kind.tag(),
            b.kind.tag()
        ));
    }

    let labels = if a.meta.input_labels.is_empty() {
        &b.meta.input_labels
    } else {
        &a.meta.input_labels
    };

    let key = |e: &Entry| (e.name.clone(), e.dim);
    let ma: BTreeMap<_, _> = a.entries.iter().map(|e| (key(e), e)).collect();
    let mb: BTreeMap<_, _> = b.entries.iter().map(|e| (key(e), e)).collect();

    let mut out = Vec::new();
    for (k, ea) in &ma {
        match mb.get(k) {
            None => out.push(Divergence::OnlyIn {
                name: k.0.clone(),
                dim: k.1,
                which: "first",
            }),
            Some(eb) => {
                if ea.classes != eb.classes {
                    out.push(Divergence::Class {
                        name: k.0.clone(),
                        dim: k.1,
                        at: differing_inputs(&ea.classes, &eb.classes, labels),
                        before: ea.classes.clone(),
                        after: eb.classes.clone(),
                    });
                } else if ea.digest != eb.digest {
                    out.push(Divergence::Magnitude {
                        name: k.0.clone(),
                        dim: k.1,
                    });
                }
            }
        }
    }
    for k in mb.keys() {
        if !ma.contains_key(k) {
            out.push(Divergence::OnlyIn {
                name: k.0.clone(),
                dim: k.1,
                which: "second",
            });
        }
    }

    // Hard findings first: a class change is a bug, a magnitude change
    // is a question.
    out.sort_by_key(|d| !d.is_hard());
    Ok(out)
}

/// Map differing glyph positions back to input labels.
///
/// Each input contributes a fixed number of consecutive glyphs (2 for
/// 2D, 3 for 3D), so the label is recoverable by division — provided
/// the strings are the length the labels imply. When they are not, the
/// position is reported raw rather than guessing at an attribution that
/// might be wrong.
fn differing_inputs(a: &str, b: &str, labels: &[String]) -> Vec<String> {
    let ga: Vec<char> = a.chars().collect();
    let gb: Vec<char> = b.chars().collect();
    let positions: Vec<usize> = (0..ga.len().max(gb.len()))
        .filter(|&i| ga.get(i) != gb.get(i))
        .collect();

    if labels.is_empty() || ga.len() != gb.len() || ga.len() % labels.len() != 0 {
        return positions.iter().map(|i| format!("glyph {i}")).collect();
    }

    let per_input = ga.len() / labels.len();
    let mut seen = Vec::new();
    for i in positions {
        let label = &labels[i / per_input];
        if !seen.iter().any(|s: &String| s == label) {
            seen.push(label.clone());
        }
    }
    seen
}

/// Below this, a timing ratio says more about scheduler jitter than
/// about the work. Set well under a driver compile (tens of ms at
/// minimum) and well over a probe dispatch (under a millisecond).
pub const NOISE_FLOOR_MS: f64 = 50.0;

/// The phase an entry belongs to: whatever follows the last `.`.
///
/// Labels with no phase share one group rather than each becoming a
/// group of one — a singleton group is its own median, so nothing in it
/// could ever read as an outlier.
fn phase_of(label: &str) -> &str {
    label.rsplit_once('.').map_or("", |(_, phase)| phase)
}

/// Per-batch timings, kept out of the diffable report.
///
/// The signal wanted here is not "how many milliseconds" — that is
/// machine-specific — but "is anything wildly out of line with its
/// peers", which is a within-run comparison and stays valid on any
/// hardware.
#[derive(Debug, Clone, Default)]
pub struct Timings {
    /// `(what, milliseconds)`, in run order.
    pub entries: Vec<(String, f64)>,
}

impl Timings {
    pub fn push(&mut self, what: impl Into<String>, millis: f64) {
        self.entries.push((what.into(), millis));
    }

    /// Entries far above the median *of their own phase*, as multiples
    /// of it.
    ///
    /// Median rather than mean because one pathological entry is
    /// exactly what is being looked for, and it would drag a mean up
    /// enough to hide itself.
    ///
    /// Grouped by the suffix after the last `.` — `compile` against
    /// `compile`, `dispatch` against `dispatch`. Pooling them was
    /// actively misleading: driver compiles run two orders of magnitude
    /// longer than the dispatches they set up, so a single median sat
    /// between the two populations and flagged every compile as an
    /// outlier while a genuinely pathological one hid in the crowd.
    ///
    /// Entries under [`NOISE_FLOOR_MS`] are never flagged however large
    /// the ratio: a 4 ms dispatch against a 0.8 ms median is a 5x
    /// "outlier" and also just scheduling jitter. A report that cries
    /// wolf at millisecond noise is one that gets skipped when it
    /// finally has something to say.
    pub fn outliers(&self, factor: f64) -> Vec<(String, f64, f64)> {
        let mut groups: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        for (what, ms) in &self.entries {
            groups.entry(phase_of(what)).or_default().push(*ms);
        }
        for values in groups.values_mut() {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        }

        let mut out = Vec::new();
        for (what, ms) in &self.entries {
            let values = &groups[phase_of(what)];
            let median = values[values.len() / 2];
            if median > 0.0 && *ms > median * factor && *ms >= NOISE_FLOOR_MS {
                out.push((what.clone(), *ms, ms / median));
            }
        }
        out
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Fractal Art Editor — variation probe timings");
        let _ = writeln!(s, "#");
        let _ = writeln!(s, "# Absolute values are machine-specific and are NOT compared");
        let _ = writeln!(s, "# across runs. What matters is the spread within one run.");
        let _ = writeln!(s, "#");
        for (what, ms) in &self.entries {
            let _ = writeln!(s, "{what} {ms:.2}ms");
        }
        let outliers = self.outliers(4.0);
        if !outliers.is_empty() {
            let _ = writeln!(s, "#");
            let _ = writeln!(s, "# outliers (>4x the median FOR THEIR PHASE):");
            for (what, ms, x) in outliers {
                let _ = writeln!(s, "#   {what}: {ms:.2}ms ({x:.1}x)");
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, dim: &'static str, classes: &str, digest: u64) -> Entry {
        Entry {
            name: name.to_string(),
            dim,
            classes: classes.to_string(),
            digest,
        }
    }

    fn report(entries: Vec<Entry>) -> Report {
        Report {
            schema: SCHEMA,
            kind: Kind::Defaults,
            meta: Meta {
                input_labels: vec!["a".into(), "b".into(), "c".into()],
                ..Default::default()
            },
            entries,
            skipped: Vec::new(),
        }
    }

    #[test]
    fn round_trips_through_text() {
        let r = report(vec![
            entry("linear", "2d", "pp00mm", 0xdead_beef_1234_5678),
            entry("linear", "3d", "ppz00zmmz", 1),
        ]);
        let back = Report::parse(&r.render()).unwrap();
        assert_eq!(back.entries, r.entries);
        assert_eq!(back.schema, SCHEMA);
        assert_eq!(back.meta.input_labels, r.meta.input_labels);
    }

    #[test]
    fn a_class_change_is_hard_and_names_the_input() {
        // Two glyphs per input, three inputs. The middle input's y
        // moved from a positive zero to a positive value — the shape of
        // the atan2 bug.
        let a = report(vec![entry("npolar", "2d", "pp00pp", 1)]);
        let b = report(vec![entry("npolar", "2d", "pp0ppp", 2)]);

        let d = compare(&a, &b).unwrap();
        assert_eq!(d.len(), 1);
        assert!(d[0].is_hard());
        match &d[0] {
            Divergence::Class { name, at, .. } => {
                assert_eq!(name, "npolar");
                assert_eq!(at, &["b".to_string()], "should name the input that moved");
            }
            other => panic!("expected a class divergence, got {other:?}"),
        }
    }

    #[test]
    fn a_digest_change_alone_is_soft() {
        let a = report(vec![entry("sinusoidal", "2d", "pppppp", 1)]);
        let b = report(vec![entry("sinusoidal", "2d", "pppppp", 2)]);
        let d = compare(&a, &b).unwrap();
        assert_eq!(d.len(), 1);
        assert!(!d[0].is_hard(), "same classes means tolerance, not a bug");
    }

    #[test]
    fn identical_reports_diverge_nowhere() {
        let a = report(vec![entry("linear", "2d", "pppppp", 7)]);
        assert!(compare(&a, &a).unwrap().is_empty());
    }

    #[test]
    fn hard_findings_sort_before_soft_ones() {
        let a = report(vec![
            entry("soft", "2d", "pppppp", 1),
            entry("hard", "2d", "pppppp", 1),
        ]);
        let b = report(vec![
            entry("soft", "2d", "pppppp", 2),
            entry("hard", "2d", "nnnnnn", 1),
        ]);
        let d = compare(&a, &b).unwrap();
        assert!(d[0].is_hard() && !d[1].is_hard());
    }

    #[test]
    fn comparing_a_defaults_report_with_a_sweep_is_refused() {
        // They share every column, so nothing else would stop this —
        // and it would emit one meaningless finding per entry.
        let a = report(vec![entry("linear", "2d", "pppppp", 1)]);
        let mut b = report(vec![entry("linear.power", "2d", "pppppp", 1)]);
        b.kind = Kind::Sweep;
        let err = compare(&a, &b).unwrap_err();
        assert!(err.contains("defaults") && err.contains("sweep"), "{err}");
    }

    #[test]
    fn a_file_that_is_not_a_report_says_so() {
        let err = Report::parse("# Some other document

some linear text here
").unwrap_err();
        assert!(
            err.contains("probe report"),
            "should name the real problem, not the first line that looks like an entry: {err}"
        );
    }

    #[test]
    fn the_kind_survives_the_round_trip() {
        let mut r = report(vec![]);
        r.kind = Kind::Sweep;
        assert_eq!(Report::parse(&r.render()).unwrap().kind, Kind::Sweep);
    }

    #[test]
    fn comparing_across_a_schema_change_is_refused_not_guessed() {
        let a = report(vec![]);
        let mut b = report(vec![]);
        b.schema = SCHEMA + 1;
        let err = compare(&a, &b).unwrap_err();
        assert!(err.contains("schema"), "{err}");
    }

    #[test]
    fn a_variation_present_in_only_one_report_is_reported() {
        let a = report(vec![entry("gone", "2d", "pppppp", 1)]);
        let b = report(vec![entry("new", "2d", "pppppp", 1)]);
        let d = compare(&a, &b).unwrap();
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|x| x.is_hard()));
    }

    #[test]
    fn attribution_falls_back_rather_than_guessing_when_lengths_disagree() {
        // 5 glyphs cannot be split evenly over 3 labels, so naming an
        // input would be a fabrication.
        let at = differing_inputs("ppppp", "ppppm", &["a".into(), "b".into(), "c".into()]);
        assert_eq!(at, vec!["glyph 4"]);
    }

    #[test]
    fn entries_with_no_output_are_named_in_the_header() {
        let r = report(vec![
            entry("silent", "2d", "000000", 1),
            entry("speaks", "2d", "pppppp", 2),
        ]);
        let text = r.render();
        assert!(text.contains("no observable output (1)"), "{text}");
        assert!(text.contains("silent(2d)"), "{text}");
        assert!(!text.contains("speaks(2d)"), "a real result must not be listed as silent");
        // And it must still parse back — the section is commentary.
        assert_eq!(Report::parse(&text).unwrap().entries, r.entries);
    }

    #[test]
    fn skipped_variations_survive_the_round_trip() {
        let mut r = report(vec![]);
        r.skipped.push(("weird".into(), "no 3d body".into()));
        let back = Report::parse(&r.render()).unwrap();
        assert_eq!(back.skipped, r.skipped);
    }

    #[test]
    fn timing_outliers_use_the_median_so_one_bad_entry_cannot_hide_itself() {
        let mut t = Timings::default();
        for i in 0..9 {
            t.push(format!("batch{i}"), 10.0);
        }
        t.push("pathological", 500.0);
        // 500ms clears NOISE_FLOOR_MS, so the ratio is the only thing
        // deciding this.

        let out = t.outliers(4.0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "pathological");
        assert!((out[0].2 - 50.0).abs() < 1e-9, "50x the median");
    }

    #[test]
    fn millisecond_jitter_is_not_reported_as_an_outlier() {
        let mut t = Timings::default();
        for i in 0..9 {
            t.push(format!("batch{i}.dispatch"), 0.8);
        }
        // 5x the median, and completely meaningless.
        t.push("batch9.dispatch", 4.0);
        assert!(t.outliers(4.0).is_empty(), "4ms against 0.8ms is jitter, not a finding");

        // The same ratio at a scale where it means something.
        let mut big = Timings::default();
        for i in 0..9 {
            big.push(format!("batch{i}.compile"), 100.0);
        }
        big.push("batch9.compile", 500.0);
        assert_eq!(big.outliers(4.0).len(), 1);
    }

    #[test]
    fn phases_are_compared_only_against_their_own_kind() {
        // Compiles run ~100x longer than dispatches. Pooled, the median
        // lands between the two populations, every compile reads as an
        // outlier, and the one compile that is genuinely pathological is
        // indistinguishable from its healthy peers.
        let mut t = Timings::default();
        for i in 0..9 {
            t.push(format!("batch{i}.compile"), 1000.0);
            t.push(format!("batch{i}.dispatch"), 1.0);
        }
        t.push("batch9.compile", 90_000.0);
        t.push("batch9.dispatch", 1.0);

        let out = t.outliers(4.0);
        assert_eq!(
            out.iter().map(|(w, _, _)| w.as_str()).collect::<Vec<_>>(),
            vec!["batch9.compile"],
            "only the compile that is slow *for a compile* should be flagged"
        );
    }

    #[test]
    fn unphased_labels_share_a_group() {
        assert_eq!(phase_of("batch3-2d.compile"), "compile");
        assert_eq!(phase_of("plain"), phase_of("also_plain"));
    }
}
