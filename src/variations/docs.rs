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

    let description = reflow(&description);
    // ONE markdown stripper in this codebase, and it is the script
    // module's: it handles links, images, escapes and the emphasis
    // flanking rules that keep `snake_case` and `2 * 3` intact, and
    // `shipped_script_prose_is_not_corrupted` holds it to a real
    // corpus. A second implementation here would be a second thing to
    // keep in agreement, writing to the same API field.
    let description_plain = crate::script::strip_markdown(&description);
    VariationDoc { description, description_plain, authors }
}

/// Join hard-wrapped lines back into paragraphs.
///
/// The defs wrap prose at ~72 columns to suit Rust source, and that
/// wrapping belongs to the file, not to the sentence: in markdown a
/// single newline inside a paragraph is a SPACE, so any renderer
/// already flows these back together. Doing it here buys two things.
///
/// It keeps the plain text matching what the markdown renders, instead
/// of baking a 72-column shape into a field nobody wrapped on purpose.
///
/// And it is what lets the shared stripper work at all. That stripper
/// pairs delimiters within a line, deliberately — so inline code
/// carried across a wrap, as in
/// `` `sin²/cos = sin ·`` + ``tan` ``, never closes. 256 doc lines in
/// the defs do this, and the corpus went out with the backticks still
/// in `description_plain` until they were flowed first.
///
/// Blank lines and list items stay as boundaries. Nothing else in the
/// defs is indentation-sensitive: there are no fenced code blocks, and
/// every indented line is the continuation of the one above it.
fn reflow(lines: &[&str]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut open = false;
    for line in lines {
        let text = line.trim();
        if text.is_empty() {
            out.push(String::new());
            open = false;
        } else if !open || is_list_item(text) {
            out.push(text.to_string());
            open = true;
        } else {
            let last = out.last_mut().expect("open implies a line to extend");
            last.push(' ');
            last.push_str(text);
        }
    }
    out.join("\n")
}

/// `- item`, `* item` or `3. item` — a line that begins a new block
/// rather than continuing the previous one.
fn is_list_item(text: &str) -> bool {
    if text.starts_with("- ") || text.starts_with("* ") {
        return true;
    }
    match text.split_once(". ") {
        Some((n, _)) => !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
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
        // One paragraph, so one line: the source wrap was Rust's, not
        // the sentence's.
        assert_eq!(
            doc.description,
            "Random-circle sampler — samples a random `(rx, ry)` point. \
             Capped at 32 iterations."
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

    /// The stripping itself belongs to `script::strip_markdown` and is
    /// tested there; this checks only that a parsed description is
    /// actually run through it, on the constructs the defs use.
    #[test]
    fn descriptions_carry_a_stripped_twin() {
        let doc = &parse_source(
            "/// Uses `pre_blur`, see [`chladni`](super::chladni).\n\
             pub static X: VariationDef = VariationDef {\n    name: \"x\",\n};\n",
        )[0]
        .1;
        assert_eq!(doc.description, "Uses `pre_blur`, see [`chladni`](super::chladni).");
        assert_eq!(doc.description_plain, "Uses pre_blur, see chladni.");
    }

    /// `description_plain` must contain no markdown left over. The
    /// direct invariant, and it catches a whole class the earlier
    /// version of this file shipped: a delimiter pair split across the
    /// source's 72-column wrap. Reflowing closes almost all of them,
    /// but not one whose continuation line opens a new block — `xtrb`
    /// wrapped a code span onto a line starting `- w²`, which any
    /// markdown parser reads as a list item, so the span never closed.
    /// The prose has to be written closable; this says when it is not.
    #[test]
    fn shipped_descriptions_strip_clean() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/variations/defs");
        let sources: Vec<String> = std::fs::read_dir(&dir)
            .expect("defs dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .collect();

        let mut offenders = Vec::new();
        for (name, doc) in parse_sources(sources.iter().map(String::as_str)) {
            let leftover = doc.description_plain.contains('`')
                || doc.description_plain.contains("](");
            if leftover {
                let line = doc
                    .description_plain
                    .lines()
                    .find(|l| l.contains('`') || l.contains("]("))
                    .unwrap_or_default();
                offenders.push(format!("{name}: {line}"));
            }
        }
        assert!(
            offenders.is_empty(),
            "{} description(s) still carry markdown after stripping — a \
             delimiter pair is split across a line break in the source:\n  {}",
            offenders.len(),
            offenders.join("\n  ")
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
