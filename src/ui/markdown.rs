//! Drawing a markdown description (`crate::markdown`) in egui: paragraphs
//! that wrap, emphasis, strong, strikethrough, inline code, code blocks,
//! headings, lists (nested, numbered or not), block quotes, rules, tables
//! and links. A link opens in the browser only when it is `http(s)`; any
//! other is drawn as its text. Raw HTML reads as the text it is, and an
//! image as its alternative text.

use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};

/// A run of inline text in one style.
#[derive(Debug, Clone, Default, PartialEq)]
struct Span {
    text: String,
    strong: bool,
    emph: bool,
    code: bool,
    strike: bool,
    link: Option<String>,
    /// A hard line break, not text.
    newline: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Para(Vec<Span>),
    Heading(u8, Vec<Span>),
    Code(String),
    List { start: Option<u64>, items: Vec<Vec<Block>> },
    Quote(Vec<Block>),
    Rule,
    Table { head: Vec<Vec<Span>>, rows: Vec<Vec<Vec<Span>>> },
}

/// A container being built.
enum Frame {
    Blocks(Vec<Block>),
    Quote(Vec<Block>),
    List { start: Option<u64>, items: Vec<Vec<Block>> },
    Item(Vec<Block>),
    Table { head: Vec<Vec<Span>>, rows: Vec<Vec<Vec<Span>>>, row: Vec<Vec<Span>> },
}

/// What the inline text being gathered becomes.
enum Inline {
    Para,
    Heading(u8),
    Cell,
}

/// `md` as blocks.
fn blocks_of(md: &str) -> Vec<Block> {
    let mut stack = vec![Frame::Blocks(Vec::new())];
    let mut inline: Option<(Inline, Vec<Span>)> = None;
    let mut code: Option<String> = None;
    let (mut strong, mut emph, mut strike) = (0u32, 0u32, 0u32);
    let mut links: Vec<String> = Vec::new();

    fn push_block(stack: &mut [Frame], b: Block) {
        match stack.last_mut() {
            Some(Frame::Blocks(v) | Frame::Quote(v) | Frame::Item(v)) => v.push(b),
            // A block straight in a list (not an item) or a table: kept in
            // the nearest container that holds blocks.
            _ => {
                if let Some(Frame::Blocks(v) | Frame::Quote(v) | Frame::Item(v)) =
                    stack.iter_mut().rev().find(|f| matches!(f, Frame::Blocks(_) | Frame::Quote(_) | Frame::Item(_)))
                {
                    v.push(b);
                }
            }
        }
    }
    fn flush(stack: &mut Vec<Frame>, inline: &mut Option<(Inline, Vec<Span>)>) {
        if let Some((kind, spans)) = inline.take() {
            match kind {
                Inline::Para => push_block(stack, Block::Para(spans)),
                Inline::Heading(l) => push_block(stack, Block::Heading(l, spans)),
                Inline::Cell => {
                    if let Some(Frame::Table { row, .. }) = stack.last_mut() {
                        row.push(spans);
                    }
                }
            }
        }
    }

    for event in crate::markdown::parse(md) {
        let mut text = |t: &str, is_code: bool, inline: &mut Option<(Inline, Vec<Span>)>| {
            if let Some(c) = code.as_mut() {
                c.push_str(t);
                return;
            }
            let (_, spans) = inline.get_or_insert_with(|| (Inline::Para, Vec::new()));
            spans.push(Span {
                text: t.to_string(),
                strong: strong > 0,
                emph: emph > 0,
                code: is_code,
                strike: strike > 0,
                link: links.last().cloned(),
                newline: false,
            });
        };
        match event {
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => text(&t, false, &mut inline),
            Event::Code(t) => text(&t, true, &mut inline),
            Event::SoftBreak => text(" ", false, &mut inline),
            Event::HardBreak => {
                if let Some((_, spans)) = inline.as_mut() {
                    spans.push(Span { newline: true, ..Default::default() });
                }
            }
            Event::Rule => {
                flush(&mut stack, &mut inline);
                push_block(&mut stack, Block::Rule);
            }
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    flush(&mut stack, &mut inline);
                    inline = Some((Inline::Para, Vec::new()));
                }
                Tag::Heading { level, .. } => {
                    flush(&mut stack, &mut inline);
                    let l = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        _ => 3,
                    };
                    inline = Some((Inline::Heading(l), Vec::new()));
                }
                Tag::CodeBlock(_) => {
                    flush(&mut stack, &mut inline);
                    code = Some(String::new());
                }
                Tag::BlockQuote(_) => {
                    flush(&mut stack, &mut inline);
                    stack.push(Frame::Quote(Vec::new()));
                }
                Tag::List(start) => {
                    flush(&mut stack, &mut inline);
                    stack.push(Frame::List { start, items: Vec::new() });
                }
                Tag::Item => {
                    flush(&mut stack, &mut inline);
                    stack.push(Frame::Item(Vec::new()));
                }
                Tag::Table(_) => {
                    flush(&mut stack, &mut inline);
                    stack.push(Frame::Table { head: Vec::new(), rows: Vec::new(), row: Vec::new() });
                }
                Tag::TableCell => inline = Some((Inline::Cell, Vec::new())),
                Tag::Emphasis => emph += 1,
                Tag::Strong => strong += 1,
                Tag::Strikethrough => strike += 1,
                Tag::Link { dest_url, .. } => links.push(dest_url.to_string()),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::Heading(_) => flush(&mut stack, &mut inline),
                TagEnd::CodeBlock => {
                    if let Some(c) = code.take() {
                        push_block(&mut stack, Block::Code(c.trim_end_matches('\n').to_string()));
                    }
                }
                TagEnd::BlockQuote(_) => {
                    flush(&mut stack, &mut inline);
                    if let Some(Frame::Quote(v)) = stack.pop() {
                        push_block(&mut stack, Block::Quote(v));
                    }
                }
                TagEnd::Item => {
                    flush(&mut stack, &mut inline);
                    if let Some(Frame::Item(v)) = stack.pop() {
                        if let Some(Frame::List { items, .. }) = stack.last_mut() {
                            items.push(v);
                        }
                    }
                }
                TagEnd::List(_) => {
                    flush(&mut stack, &mut inline);
                    if let Some(Frame::List { start, items }) = stack.pop() {
                        push_block(&mut stack, Block::List { start, items });
                    }
                }
                TagEnd::TableCell => flush(&mut stack, &mut inline),
                TagEnd::TableHead => {
                    if let Some(Frame::Table { head, row, .. }) = stack.last_mut() {
                        *head = std::mem::take(row);
                    }
                }
                TagEnd::TableRow => {
                    if let Some(Frame::Table { rows, row, .. }) = stack.last_mut() {
                        rows.push(std::mem::take(row));
                    }
                }
                TagEnd::Table => {
                    if let Some(Frame::Table { head, rows, .. }) = stack.pop() {
                        push_block(&mut stack, Block::Table { head, rows });
                    }
                }
                TagEnd::Emphasis => emph = emph.saturating_sub(1),
                TagEnd::Strong => strong = strong.saturating_sub(1),
                TagEnd::Strikethrough => strike = strike.saturating_sub(1),
                TagEnd::Link => {
                    links.pop();
                }
                _ => {}
            },
            _ => {}
        }
    }
    flush(&mut stack, &mut inline);
    match stack.into_iter().next() {
        Some(Frame::Blocks(v)) => v,
        _ => Vec::new(),
    }
}

/// Draw the markdown `md`.
pub fn show(ui: &mut egui::Ui, md: &str) {
    draw_blocks(ui, &blocks_of(md));
}

fn draw_blocks(ui: &mut egui::Ui, blocks: &[Block]) {
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            ui.add_space(4.0);
        }
        match b {
            Block::Para(spans) => draw_inline(ui, spans, None),
            Block::Heading(l, spans) => draw_inline(ui, spans, Some(*l)),
            Block::Code(text) => {
                egui::Frame::new().fill(ui.visuals().code_bg_color).inner_margin(egui::Margin::same(4)).corner_radius(2.0).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap_mode(egui::TextWrapMode::Extend));
                });
            }
            Block::List { start, items } => {
                for (n, item) in items.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        let marker = match start {
                            Some(s) => format!("{}.", s + n as u64),
                            None => "•".to_string(),
                        };
                        ui.label(marker);
                        ui.vertical(|ui| draw_blocks(ui, item));
                    });
                }
            }
            Block::Quote(inner) => {
                ui.horizontal_top(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(3.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 1.0, ui.visuals().weak_text_color());
                    ui.vertical(|ui| draw_blocks(ui, inner));
                });
            }
            Block::Rule => {
                ui.separator();
            }
            Block::Table { head, rows } => {
                egui::Grid::new(ui.next_auto_id()).striped(true).show(ui, |ui| {
                    if !head.is_empty() {
                        for cell in head {
                            let strong: Vec<Span> = cell.iter().map(|s| Span { strong: true, ..s.clone() }).collect();
                            draw_inline(ui, &strong, None);
                        }
                        ui.end_row();
                    }
                    for row in rows {
                        for cell in row {
                            draw_inline(ui, cell, None);
                        }
                        ui.end_row();
                    }
                });
            }
        }
    }
}

/// Inline text that wraps, a word at a time, so a link can sit inside it.
fn draw_inline(ui: &mut egui::Ui, spans: &[Span], heading: Option<u8>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let base = ui.text_style_height(&egui::TextStyle::Body);
        for span in spans {
            if span.newline {
                ui.end_row();
                continue;
            }
            // Words with their trailing space, so wrapping falls between
            // them; inline code stays whole.
            let pieces: Vec<&str> = if span.code { vec![span.text.as_str()] } else { span.text.split_inclusive(' ').collect() };
            for piece in pieces {
                let mut rich = egui::RichText::new(piece);
                if let Some(l) = heading {
                    rich = rich.strong().size(base * if l == 1 { 1.35 } else if l == 2 { 1.18 } else { 1.05 });
                }
                if span.strong {
                    rich = rich.strong();
                }
                if span.emph {
                    rich = rich.italics();
                }
                if span.strike {
                    rich = rich.strikethrough();
                }
                if span.code {
                    rich = rich.code();
                }
                match &span.link {
                    Some(url) if url.starts_with("https://") || url.starts_with("http://") => {
                        ui.hyperlink_to(rich, url).on_hover_text(url);
                    }
                    _ => {
                        ui.label(rich);
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Span {
        Span { text: text.into(), ..Default::default() }
    }

    /// The blocks a description reads as: paragraphs with their styles,
    /// a tight list's items, a table, a code block; raw HTML as text.
    #[test]
    fn a_description_reads_as_its_blocks() {
        let b = blocks_of("The **classic**: `z^2`.\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```\n\n<i>x</i>");
        assert_eq!(b.len(), 5, "{b:?}");
        let Block::Para(p) = &b[0] else { panic!("{b:?}") };
        assert_eq!(p[0], s("The "));
        assert!(p[1].strong && p[1].text == "classic");
        assert!(p[3].code && p[3].text == "z^2");
        let Block::List { start: None, items } = &b[1] else { panic!("{b:?}") };
        assert_eq!(items.len(), 2);
        assert_eq!(items[1], vec![Block::Para(vec![s("two")])]);
        let Block::Table { head, rows } = &b[2] else { panic!("{b:?}") };
        assert_eq!((head.len(), rows.len()), (2, 1));
        assert_eq!(b[3], Block::Code("code".into()));
        let Block::Para(html) = &b[4] else { panic!("{b:?}") };
        assert_eq!(html.iter().map(|s| s.text.as_str()).collect::<String>(), "<i>x</i>");
    }

    /// A link carries its target on its words; nested lists nest.
    #[test]
    fn links_and_nested_lists() {
        let b = blocks_of("See [the paper](https://example.com/p) now.\n\n1. first\n   - inner\n2. second");
        let Block::Para(p) = &b[0] else { panic!("{b:?}") };
        assert_eq!(p[1].link.as_deref(), Some("https://example.com/p"));
        assert_eq!(p[2].link, None);
        let Block::List { start: Some(1), items } = &b[1] else { panic!("{b:?}") };
        assert!(matches!(items[0].last(), Some(Block::List { start: None, .. })), "{:?}", items[0]);
    }
}
