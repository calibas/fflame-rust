//! Descriptions are markdown -- a built-in's, a script's, a palette's, a
//! fractal's (docs/main/CATALOG.md, *Descriptions*). This is the format's
//! one definition in the app: what is parsed (CommonMark with tables and
//! strikethrough, through `pulldown-cmark`), and the plain text of it, for
//! searching and for a one-line tooltip. Drawing it is `ui::markdown`.
//!
//! Raw HTML is never interpreted: it reads as the text it is. Images show
//! their alternative text.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// The extensions descriptions may use beyond CommonMark.
pub fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// The parsed events of `md`.
pub fn parse(md: &str) -> Parser<'_> {
    Parser::new_ext(md, options())
}

/// The text of `md` without its markup: blocks a blank line apart, list
/// items a line each, table cells a tab apart.
pub fn plain(md: &str) -> String {
    let mut out = String::new();
    for event in parse(md) {
        match event {
            Event::Text(t) | Event::Code(t) | Event::Html(t) | Event::InlineHtml(t) => out.push_str(&t),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            Event::Start(Tag::Item) => {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("- ");
            }
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Table | TagEnd::BlockQuote(_)) => {
                out.push_str("\n\n")
            }
            Event::End(TagEnd::TableCell) => out.push('\t'),
            Event::End(TagEnd::TableRow | TagEnd::TableHead) => out.push('\n'),
            Event::End(TagEnd::List(_)) => out.push('\n'),
            Event::Rule => out.push_str("\n\n"),
            _ => {}
        }
    }
    // Tidy the joins: no run of blank lines, no trailing space.
    let mut tidy = String::new();
    let mut blank = 0;
    for line in out.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !tidy.is_empty() {
            tidy.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        tidy.push_str(line);
        blank = 0;
    }
    tidy
}

/// The first paragraph's plain text, for a tooltip.
pub fn first_paragraph(md: &str) -> String {
    plain(md).split("\n\n").next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plain text keeps the words and drops the markup; HTML is text.
    #[test]
    fn plain_text_drops_the_markup() {
        let md = "The **classic**: `z -> z^2 + c`, see [the docs](https://example.com).\n\n- one\n- *two*\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n<b>raw</b>";
        let p = plain(md);
        assert_eq!(first_paragraph(md), "The classic: z -> z^2 + c, see the docs.");
        assert!(p.contains("- one\n- two"), "{p:?}");
        assert!(p.contains("a\tb"), "{p:?}");
        assert!(p.contains("<b>raw</b>"), "HTML reads as text: {p:?}");
        assert_eq!(plain("plain words"), "plain words");
    }
}
