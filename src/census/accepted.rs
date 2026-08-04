//! The accepted-divergences list — hand-curated dispositions that
//! `rank` subtracts from its worklist.
//!
//! See `docs/accepted-divergences.txt` for the format and, more
//! importantly, the bar an entry has to clear. This module only parses
//! and matches; the judgment lives in the file, with reasons and
//! evidence commits, under review like any other change.

use std::path::Path;

pub const DEFAULT_PATH: &str = "docs/accepted-divergences.txt";

#[derive(Debug, Clone)]
pub struct Entry {
    pub variation: String,
    pub dims: Vec<String>,
    /// Probe input labels; empty means `*` (all inputs).
    pub inputs: Vec<String>,
    pub reason: String,
}

pub struct Accepted {
    entries: Vec<Entry>,
}

impl Accepted {
    /// Parse the file; a missing file is an empty list, not an error —
    /// rank must work in a checkout that has not curated anything yet.
    pub fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.split_whitespace();
            let (Some(variation), Some(dims), Some(inputs)) = (it.next(), it.next(), it.next())
            else {
                continue;
            };
            let reason = it.collect::<Vec<_>>().join(" ");
            entries.push(Entry {
                variation: variation.to_string(),
                dims: dims.split(',').map(str::to_string).collect(),
                inputs: if inputs == "*" {
                    Vec::new()
                } else {
                    inputs.split(',').map(str::to_string).collect()
                },
                reason,
            });
        }
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Is this (variation, dim, input) site dispositioned?
    pub fn covers(&self, variation: &str, dim: &str, input: &str) -> bool {
        self.entries.iter().any(|e| {
            e.variation == variation
                && e.dims.iter().any(|d| d == dim)
                && (e.inputs.is_empty() || e.inputs.iter().any(|i| i == input))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# comment
exp2   2d,3d  big,large   exp overflow; consequence-identical (abc123)
funnel 2d     *           everything about funnel 2d
";

    #[test]
    fn parses_and_matches() {
        let a = Accepted::parse(SAMPLE);
        assert_eq!(a.len(), 2);
        assert!(a.covers("exp2", "2d", "big"));
        assert!(a.covers("exp2", "3d", "large"));
        assert!(!a.covers("exp2", "2d", "q1"), "input not listed");
        assert!(!a.covers("exp2", "4d", "big"), "dim not listed");
        assert!(!a.covers("exp3", "2d", "big"), "different variation");
        // wildcard inputs
        assert!(a.covers("funnel", "2d", "anything"));
        assert!(!a.covers("funnel", "3d", "anything"));
    }

    #[test]
    fn missing_file_is_empty() {
        let a = Accepted::load(Path::new("/definitely/not/here.txt"));
        assert!(a.is_empty());
    }

    /// The committed file must stay parseable and its entries must
    /// reference real probe input labels — a typo in a label would
    /// silently accept nothing.
    #[test]
    fn committed_file_labels_are_real_probe_inputs() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_PATH);
        let a = Accepted::load(&path);
        if a.is_empty() {
            return; // nothing curated yet — fine
        }
        let labels: Vec<&str> = crate::probe::inputs::probe_inputs()
            .iter()
            .map(|p| p.label)
            .collect();
        for e in &a.entries {
            for i in &e.inputs {
                assert!(
                    labels.contains(&i.as_str()),
                    "accepted-divergences entry for `{}` names unknown input `{}`",
                    e.variation,
                    i
                );
            }
            for d in &e.dims {
                assert!(d == "2d" || d == "3d", "unknown dim `{d}`");
            }
            assert!(
                !e.reason.is_empty(),
                "entry for `{}` has no reason — the file's whole point",
                e.variation
            );
        }
    }
}
