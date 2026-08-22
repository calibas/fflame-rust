//! Variation prose, read back out of the `///` doc comments in `defs/`.
//!
//! Descriptions and authors are the one part of a variation's metadata
//! that exists ONLY as source comments. They are invisible at runtime —
//! [`crate::storage::variation_catalog`] spells out the consequence:
//! prose reaches the app through the API catalog, never through the
//! registry, *including* for variations the app itself ships. So the
//! corpus the API imports has to recover them from the source text, and
//! this is where that happens.
//!
//! This takes source **text**, not paths. The caller supplies the
//! files, which keeps the parsing a pure function testable against
//! fixtures and leaves "the defs live under `CARGO_MANIFEST_DIR`" as
//! knowledge belonging to the export binary rather than to the shipped
//! library.
//!
//! The shape it relies on, verified across all 647 shipped defs:
//!
//! ```text
//! /// Prose, markdown, one or more lines.
//! ///
//! /// # Authors
//! /// - Someone
//! pub static SOMETHING: VariationDef = VariationDef {
//!     name: "something",
//! ```
//!
//! A doc block must sit immediately above its `pub static` (no blank
//! line, no attribute between), and the variation's wire name is the
//! `name:` field — NOT the static's identifier, which differs in case
//! for `popcorn2_3D` and friends.

use std::collections::BTreeMap;

/// One variation's prose, in the three shapes the wire format wants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VariationDoc {
    /// Markdown: the comment verbatim, minus the `# Authors` section.
    pub description: String,
    /// The same prose with markdown syntax removed, for clients with no
    /// markdown renderer — which includes this app.
    pub description_plain: String,
    /// Attribution, in the order the comment lists it. Empty is normal:
    /// 84 of the shipped defs claim no author.
    pub authors: Vec<String>,
}

/// Parse every documented `VariationDef` in one `defs/*.rs` file.
///
/// Returns `(wire name, doc)` pairs in file order. A `pub static` with
/// no doc block directly above it is skipped rather than reported —
/// the caller compares against the registry, which is the only place
/// that knows what SHOULD be present.
pub fn parse_source(src: &str) -> Vec<(String, VariationDoc)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        if !(line.starts_with("pub static ") && line.contains(": VariationDef")) {
            continue;
        }

        // Walk back over the contiguous `///` block.
        let mut start = i;
        while start > 0 && lines[start - 1].trim_start().starts_with("///") {
            start -= 1;
        }
        if start == i {
            continue; // undocumented
        }

        // The wire name is the `name:` field, found within this item's
        // body. Bounded by the item's closing `};` at column 0 so a
        // malformed file cannot run into the next definition.
        let mut name = None;
        for body in lines.iter().skip(i + 1) {
            if body.starts_with("};") {
                break;
            }
            if let Some(v) = field_str(body, "name") {
                name = Some(v);
                break;
            }
        }
        let Some(name) = name else { continue };

        out.push((name, parse_doc_block(&lines[start..i])));
    }

    out
}

/// Parse many files at once, keyed by wire name.
///
/// A name defined twice is a bug in the defs, not something to paper
/// over silently — the second one wins here, and the caller's coverage
/// check is what surfaces the mismatch.
pub fn parse_sources<'a>(
    files: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<String, VariationDoc> {
    let mut map = BTreeMap::new();
    for src in files {
        for (name, doc) in parse_source(src) {
            map.insert(name, doc);
        }
    }
    map
}

/// `    name: "value",` -> `value`, and only at the struct's own field
/// indent. Parameter tables nest their own `name:` two levels deeper;
/// matching those would hand back a parameter's name as the
/// variation's.
fn field_str(line: &str, field: &str) -> Option<String> {
    let rest = line.strip_prefix("    ")?;
    if rest.starts_with(' ') {
        return None;
    }
    let rest = rest.strip_prefix(field)?.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Split a `///` block into prose and attribution.
fn parse_doc_block(doc_lines: &[&str]) -> VariationDoc {
    let mut description: Vec<&str> = Vec::new();
    let mut authors = Vec::new();
    let mut in_authors = false;

    for raw in doc_lines {
        let text = raw.trim_start();
        // `/// text` and a bare `///` are the only two forms in the
        // tree; strip one space so indented markdown keeps its shape.
        let text = text.strip_prefix("///").unwrap_or(text);
        let text = text.strip_prefix(' ').unwrap_or(text);

        if text.starts_with('#') && text.trim_start_matches('#').trim() == "Authors" {
            in_authors = true;
            continue;
        }

        if in_authors {
            if let Some(a) = text.trim().strip_prefix("- ") {
                let a = a.trim();
                if !a.is_empty() {
                    authors.push(a.to_string());
                }
            }
        } else {
            description.push(text);
        }
    }

    // Trailing blank lines are the separator before `# Authors`, not
    // part of the prose.
    while description.last().is_some_and(|l| l.trim().is_empty()) {
        description.pop();
    }
    while description.first().is_some_and(|l| l.trim().is_empty()) {
        description.remove(0);
    }

    let description = description.join("\n");
    let description_plain = strip_markdown(&description);
    VariationDoc { description, description_plain, authors }
}

/// Remove markdown syntax, leaving the words.
///
/// Deliberately narrow — it handles what the defs actually use (inline
/// code, bold, links, headings) and nothing else. In particular it does
/// NOT treat `_` as emphasis: variation and parameter names are full of
/// underscores (`popcorn2_3D`, `pre_blur`), and "stripping" those would
/// corrupt the very identifiers the prose is explaining.
pub fn strip_markdown(md: &str) -> String {
    // Headings are line-scoped; inline syntax is NOT. A link's text can
    // wrap across lines — `waves_wf_family` writes
    // `[mathworld.wolfram.` / `com/DinisSurface](...)` over two — so
    // stripping line by line left that one half-converted, with a bare
    // `](url)` still sitting in the plain text. The inline pass runs
    // over the whole string instead, newlines preserved as ordinary
    // characters.
    let deheaded: Vec<&str> = md.lines().map(strip_heading).collect();
    strip_inline(&deheaded.join("\n"))
}

/// Drop a leading `#` marker, but only `#` followed by another `#` or a
/// space — a bare `#` mid-prose is text.
fn strip_heading(line: &str) -> &str {
    let trimmed = line.trim_end();
    let lead = trimmed.trim_start();
    match lead.strip_prefix('#') {
        Some(rest) if rest.starts_with('#') || rest.starts_with(' ') => {
            lead.trim_start_matches('#').trim_start()
        }
        _ => trimmed,
    }
}

fn strip_inline(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            // Inline code: the fences go, the code stays.
            '`' => i += 1,
            // Bold. A single `*` is left alone: it is multiplication far
            // more often than emphasis in this corpus.
            '*' if chars.get(i + 1) == Some(&'*') => i += 2,
            '[' => match take_link(&chars, i) {
                Some((text, next)) => {
                    out.push_str(&strip_inline(&text));
                    i = next;
                }
                None => {
                    out.push('[');
                    i += 1;
                }
            },
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// `[text](url)` starting at `open` -> the text, and the index past the
/// closing paren. The URL is dropped: most of them are rustdoc
/// intra-doc paths (`super::chladni`) that mean nothing outside the
/// crate, and a plain-text reader cannot follow any of them anyway.
fn take_link(chars: &[char], open: usize) -> Option<(String, usize)> {
    let close = (open + 1..chars.len()).find(|&j| chars[j] == ']')?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let paren = (close + 2..chars.len()).find(|&j| chars[j] == ')')?;
    Some((chars[open + 1..close].iter().collect(), paren + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r####"
// ---------------------------------------------------------------------------
// circleRand
// ---------------------------------------------------------------------------

/// Random-circle sampler — samples a random `(rx, ry)` point.
/// Capped at 32 iterations.
pub static CIRCLE_RAND: VariationDef = VariationDef {
    name: "circleRand",
    display_name: "Circle Rand",
    parameters: &[
        VariationParameter {
            name: "Sc",
        },
    ],
};

/// Normal-phase circle-crop — tests whether the input lies inside a
/// circle of radius `radius`.
///
/// # Authors
/// - Xyrus02
/// - Someone Else
pub static CIRCLECROP: VariationDef = VariationDef {
    name: "circlecrop",
};
"####;

    #[test]
    fn parses_description_and_authors() {
        let found = parse_source(SAMPLE);
        assert_eq!(found.len(), 2, "both defs found: {found:?}");

        let (name, doc) = &found[0];
        assert_eq!(name, "circleRand", "the wire name, not the static ident");
        assert_eq!(
            doc.description,
            "Random-circle sampler — samples a random `(rx, ry)` point.\nCapped at 32 iterations."
        );
        assert!(doc.authors.is_empty(), "no Authors section is normal");

        let (name, doc) = &found[1];
        assert_eq!(name, "circlecrop");
        assert_eq!(doc.authors, vec!["Xyrus02", "Someone Else"]);
        assert!(
            !doc.description.contains("Authors"),
            "the attribution section is not prose: {:?}",
            doc.description
        );
        assert!(
            doc.description.ends_with("radius`."),
            "trailing blank line before # Authors is trimmed: {:?}",
            doc.description
        );
    }

    /// A parameter's `name:` sits two levels deeper than the
    /// variation's. Reading the wrong one would mislabel the row, and
    /// `circleRand` above has exactly that shape.
    #[test]
    fn parameter_names_are_not_mistaken_for_the_variation_name() {
        assert_eq!(
            field_str("    name: \"circleRand\",", "name").as_deref(),
            Some("circleRand")
        );
        assert_eq!(field_str("            name: \"Sc\",", "name"), None);
        assert_eq!(field_str("    display_name: \"Circle Rand\",", "name"), None);
    }

    #[test]
    fn markdown_is_stripped_without_eating_identifiers() {
        assert_eq!(strip_markdown("uses `pre_blur` and **bold**"), "uses pre_blur and bold");
        assert_eq!(strip_markdown("see [`chladni`](super::chladni)"), "see chladni");
        assert_eq!(strip_markdown("[MathWorld](https://x/y)"), "MathWorld");
        // Underscores and lone asterisks survive: they are identifiers
        // and multiplication, not emphasis.
        assert_eq!(strip_markdown("popcorn2_3D scales x * y"), "popcorn2_3D scales x * y");
        // An unclosed bracket is text, not a swallowed line.
        assert_eq!(strip_markdown("range [-X, X] sampling"), "range [-X, X] sampling");
        // A link whose TEXT wraps across lines still resolves. The real
        // one in waves_wf_family does this, and a line-by-line stripper
        // left `](https://...)` sitting in the plain description.
        assert_eq!(
            strip_markdown("see [mathworld.wolfram.\ncom/DinisSurface](https://x/y)."),
            "see mathworld.wolfram.\ncom/DinisSurface."
        );
    }

    /// The real corpus: every variation the registry ships must have
    /// prose here. This is the guard that the exporter's output is
    /// complete — the corpus went out with `description: null` for all
    /// 647 rows because nothing checked.
    #[test]
    fn every_shipped_variation_has_a_description() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/variations/defs");
        let sources: Vec<String> = std::fs::read_dir(&dir)
            .expect("defs dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .collect();
        let docs = parse_sources(sources.iter().map(String::as_str));

        let registry = crate::variations::global_registry();
        let mut missing = Vec::new();
        for name in registry.names() {
            let Some(info) = registry.get(name) else { continue };
            if !info.provenance.is_builtin() {
                continue;
            }
            match docs.get(&info.name) {
                Some(d) if !d.description.trim().is_empty() => {}
                _ => missing.push(info.name.clone()),
            }
        }
        assert!(
            missing.is_empty(),
            "{} shipped variation(s) have no parsed description — the corpus \
             would export nulls for them: {:?}",
            missing.len(),
            &missing[..missing.len().min(15)]
        );
    }
}
