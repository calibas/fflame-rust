//! Syntax highlighting for the code editors: Rhai (the Scripts panel,
//! and the Formula Editor's `.rhai` files) and the Ultra Fractal /
//! Fractint formula language (the Formula Editor's `.ufm`, `.ucl` and
//! `.frm` files).
//!
//! No external crate. egui's `TextEdit::layouter` hook hands us the
//! buffer and expects a laid-out `Galley` back, so highlighting is
//! "build a `LayoutJob` of coloured runs" — which needs a tokenizer and
//! nothing else. `syntect` (what `egui_extras` reaches for) is a
//! grammar engine and a multi-megabyte dependency; Rhai's lexical
//! surface is small enough to walk directly.
//!
//! **Function calls are detected structurally** — an identifier
//! followed by `(` — rather than matched against a list of the API's
//! names. A list would duplicate `script/api.rs` and rot the moment
//! someone registers a function; the grammar rule cannot.
//!
//! The tokenizers never fail. Unterminated strings and comments run to
//! the end of the buffer and colour as themselves, which is what you
//! want while typing: the editor shows a half-written string as a
//! string, not as an error.
//!
//! The formula language's own lexer (`escape::lang::lexer`) is no use
//! here: it drops comments, whitespace and directives, and lexes an
//! entry's body at a time. Its vocabulary is what the second tokenizer
//! below knows.

use std::cell::RefCell;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

/// Colours, chosen against the app's dark editor background (the app
/// pins `Visuals::dark()`). Deliberately low-saturation: this sits
/// behind a lot of reading, and a rainbow is tiring.
mod color {
    use egui::Color32;
    pub const COMMENT: Color32 = Color32::from_rgb(106, 135, 89);
    pub const STRING: Color32 = Color32::from_rgb(206, 145, 120);
    pub const NUMBER: Color32 = Color32::from_rgb(181, 206, 168);
    pub const KEYWORD: Color32 = Color32::from_rgb(197, 134, 192);
    pub const CALL: Color32 = Color32::from_rgb(220, 220, 170);
    pub const PLAIN: Color32 = Color32::from_rgb(212, 212, 212);
    /// A formula's predefined symbols (`#pixel`) and parameters (`@power`).
    pub const SYMBOL: Color32 = Color32::from_rgb(156, 220, 254);
}

/// Which language a buffer is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Rhai,
    /// Ultra Fractal's formula language, and Fractint's (the same
    /// lexical surface: `;` comments, `#` and `@` symbols, sections).
    Formula,
}

/// The formula language's words, matched without regard to case: its
/// sections (followed by `:`), its block words and its types.
const FORMULA_KEYWORDS: &[&str] = &[
    "global", "builtin", "init", "loop", "bailout", "perturbinit", "perturbloop", "final", "transform", "default",
    "switch", "if", "elseif", "else", "endif", "while", "endwhile", "repeat", "until", "param", "endparam", "func",
    "endfunc", "heading", "endheading", "bool", "int", "float", "complex", "color", "true", "false", "const",
];

/// Rhai's reserved words, plus the literals worth colouring like them.
const KEYWORDS: &[&str] = &[
    "let", "const", "if", "else", "switch", "do", "while", "until", "loop", "for", "in",
    "continue", "break", "return", "fn", "private", "import", "export", "as", "global",
    "try", "catch", "throw", "true", "false",
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tok {
    Plain,
    Comment,
    Str,
    Num,
    Keyword,
    Call,
    Symbol,
}

impl Tok {
    fn color(self) -> Color32 {
        match self {
            Tok::Plain => color::PLAIN,
            Tok::Comment => color::COMMENT,
            Tok::Str => color::STRING,
            Tok::Num => color::NUMBER,
            Tok::Keyword => color::KEYWORD,
            Tok::Call => color::CALL,
            Tok::Symbol => color::SYMBOL,
        }
    }
}

/// Split `src` into (byte range, kind) runs. Total coverage: the ranges
/// tile the whole input in order, so the caller can append them all and
/// reproduce the source exactly.
fn tokenize(src: &str) -> Vec<(usize, usize, Tok)> {
    let b = src.as_bytes();
    let n = b.len();
    let mut out: Vec<(usize, usize, Tok)> = Vec::new();
    let mut i = 0usize;
    // Start of the current run of un-classified (plain) bytes.
    let mut plain_start = 0usize;

    // Close the pending plain run, then push a classified one.
    macro_rules! emit {
        ($start:expr, $end:expr, $kind:expr) => {{
            if plain_start < $start {
                out.push((plain_start, $start, Tok::Plain));
            }
            out.push(($start, $end, $kind));
            plain_start = $end;
        }};
    }

    while i < n {
        let c = b[i];

        // Line comment.
        if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            let start = i;
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            emit!(start, i, Tok::Comment);
            continue;
        }

        // Block comment. Unterminated runs to EOF — correct while typing.
        if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < n && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = if i + 1 < n { i + 2 } else { n };
            emit!(start, i, Tok::Comment);
            continue;
        }

        // String or char literal, with backslash escapes.
        if c == b'"' || c == b'\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < n {
                if b[i] == b'\\' {
                    // Skip the escaped byte; a trailing backslash at EOF
                    // must not step past the end.
                    i = (i + 2).min(n);
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                // A newline ends a single-quoted char literal — treating
                // an unclosed `'` as a string to EOF would paint the
                // rest of the file, and apostrophes appear in comments.
                if quote == b'\'' && b[i] == b'\n' {
                    break;
                }
                i += 1;
            }
            emit!(start, i, Tok::Str);
            continue;
        }

        // Number: a digit, or a dot directly followed by a digit that
        // isn't a field access (`t.5` is not valid Rhai anyway).
        if c.is_ascii_digit() {
            let start = i;
            while i < n
                && (b[i].is_ascii_alphanumeric()   // covers hex digits and the `x` in 0x
                    || b[i] == b'_'
                    || b[i] == b'.'
                    // exponent sign, only right after an e/E
                    || ((b[i] == b'+' || b[i] == b'-')
                        && i > start
                        && (b[i - 1] | 0x20) == b'e'))
            {
                i += 1;
            }
            emit!(start, i, Tok::Num);
            continue;
        }

        // Identifier / keyword / call.
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &src[start..i];
            let kind = if KEYWORDS.contains(&word) {
                Tok::Keyword
            } else {
                // A call is an identifier whose next non-space byte is
                // `(`. Structural, so it never goes stale against the
                // registered API.
                let mut j = i;
                while j < n && (b[j] == b' ' || b[j] == b'\t') {
                    j += 1;
                }
                if j < n && b[j] == b'(' {
                    Tok::Call
                } else {
                    Tok::Plain
                }
            };
            if kind == Tok::Plain {
                // Leave it in the plain run rather than splitting it.
                continue;
            }
            emit!(start, i, kind);
            continue;
        }

        i += 1;
    }

    if plain_start < n {
        out.push((plain_start, n, Tok::Plain));
    }
    out
}

/// The formula language's runs, as [`tokenize`] gives Rhai's: total
/// coverage, never failing. `;` starts a comment to the end of the line;
/// a string ends at its quote or the line's end; `#name` and `@name` are
/// symbols; a `$directive` is a keyword; words are matched without case.
fn tokenize_formula(src: &str) -> Vec<(usize, usize, Tok)> {
    let b = src.as_bytes();
    let n = b.len();
    let mut out: Vec<(usize, usize, Tok)> = Vec::new();
    let mut i = 0usize;
    let mut plain_start = 0usize;
    macro_rules! emit {
        ($start:expr, $end:expr, $kind:expr) => {{
            if plain_start < $start {
                out.push((plain_start, $start, Tok::Plain));
            }
            out.push(($start, $end, $kind));
            plain_start = $end;
        }};
    }
    let word_end = |mut j: usize| {
        while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'.') {
            j += 1;
        }
        j
    };
    while i < n {
        let c = b[i];
        if c == b';' {
            let start = i;
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            emit!(start, i, Tok::Comment);
            continue;
        }
        if c == b'"' {
            let start = i;
            i += 1;
            while i < n && b[i] != b'"' && b[i] != b'\n' {
                i += 1;
            }
            if i < n && b[i] == b'"' {
                i += 1;
            }
            emit!(start, i, Tok::Str);
            continue;
        }
        if (c == b'#' || c == b'@') && i + 1 < n && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_') {
            let start = i;
            i = word_end(i + 1);
            emit!(start, i, Tok::Symbol);
            continue;
        }
        if c == b'$' && i + 1 < n && b[i + 1].is_ascii_alphabetic() {
            let start = i;
            i = word_end(i + 1);
            emit!(start, i, Tok::Keyword);
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && i + 1 < n && b[i + 1].is_ascii_digit()) {
            let start = i;
            while i < n
                && (b[i].is_ascii_digit()
                    || b[i] == b'.'
                    || (b[i] | 0x20) == b'e'
                    || ((b[i] == b'+' || b[i] == b'-') && i > start && (b[i - 1] | 0x20) == b'e'))
            {
                i += 1;
            }
            // An imaginary literal's `i`.
            if i < n && (b[i] | 0x20) == b'i' && !(i + 1 < n && (b[i + 1].is_ascii_alphanumeric() || b[i + 1] == b'_')) {
                i += 1;
            }
            emit!(start, i, Tok::Num);
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = src[start..i].to_ascii_lowercase();
            let mut j = i;
            while j < n && (b[j] == b' ' || b[j] == b'\t') {
                j += 1;
            }
            let kind = if FORMULA_KEYWORDS.contains(&word.as_str()) {
                Tok::Keyword
            } else if j < n && (b[j] == b'(' || b[j] == b'{') {
                // A call, or an entry's name before its body.
                Tok::Call
            } else {
                Tok::Plain
            };
            if kind == Tok::Plain {
                continue;
            }
            emit!(start, i, kind);
            continue;
        }
        i += 1;
    }
    if plain_start < n {
        out.push((plain_start, n, Tok::Plain));
    }
    out
}

/// Build the coloured layout for `src`.
fn build_job(src: &str, font_id: FontId, lang: Lang) -> LayoutJob {
    let mut job = LayoutJob::default();
    let runs = match lang {
        Lang::Rhai => tokenize(src),
        Lang::Formula => tokenize_formula(src),
    };
    for (start, end, kind) in runs {
        job.append(
            &src[start..end],
            0.0,
            TextFormat {
                font_id: font_id.clone(),
                color: kind.color(),
                ..Default::default()
            },
        );
    }
    job
}

thread_local! {
    /// A one-entry memo for each language. The layouter runs every frame,
    /// and re-tokenizing a few KB per frame is wasted work even though it
    /// is fast. Keyed by the text and the font, so a theme or size change
    /// re-highlights; a slot a language, so the Scripts panel and the
    /// Formula Editor open together do not evict each other every frame.
    /// egui caches the *galley* for an identical job, so this is the only
    /// per-frame cost that was ours to remove.
    static MEMO: RefCell<[Option<(u64, LayoutJob)>; 2]> = const { RefCell::new([None, None]) };
}

fn key(src: &str, font_id: &FontId) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    font_id.size.to_bits().hash(&mut h);
    h.finish()
}

/// The `LayoutJob` for Rhai `src`, memoized. Wrap width is the caller's
/// to set — it changes with panel width and must not invalidate the memo.
pub fn highlight(src: &str, font_id: FontId) -> LayoutJob {
    highlight_lang(src, font_id, Lang::Rhai)
}

/// [`highlight`] in `lang`.
pub fn highlight_lang(src: &str, font_id: FontId, lang: Lang) -> LayoutJob {
    let k = key(src, &font_id);
    let slot = lang as usize;
    MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        if let Some((cached_key, job)) = memo[slot].as_ref() {
            if *cached_key == k {
                return job.clone();
            }
        }
        let job = build_job(src, font_id, lang);
        memo[slot] = Some((k, job.clone()));
        job
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reassembling the runs must reproduce the source exactly — a
    /// tokenizer that drops or duplicates a byte would silently corrupt
    /// what the editor DISPLAYS while the buffer stays correct, which
    /// is a maddening bug to chase.
    fn assert_lossless(src: &str) {
        let joined: String = tokenize(src)
            .iter()
            .map(|&(s, e, _)| &src[s..e])
            .collect();
        assert_eq!(joined, src, "tokenizer lost or duplicated bytes");
    }

    fn kinds(src: &str) -> Vec<(&str, Tok)> {
        tokenize(src).into_iter().map(|(s, e, k)| (&src[s..e], k)).collect()
    }

    fn find(src: &str, needle: &str) -> Option<Tok> {
        kinds(src).into_iter().find(|(t, _)| *t == needle).map(|(_, k)| k)
    }

    #[test]
    fn covers_every_byte_of_real_scripts() {
        // The shipped starters are the realistic corpus.
        for src in crate::script::library::EMBEDDED {
            assert_lossless(src.1);
        }
    }

    #[test]
    fn classifies_the_basics() {
        let src = r#"let x = 42; // note
flame.add_transform();
"#;
        assert_eq!(find(src, "let"), Some(Tok::Keyword));
        assert_eq!(find(src, "42"), Some(Tok::Num));
        assert_eq!(find(src, "// note"), Some(Tok::Comment));
        assert_eq!(find(src, "add_transform"), Some(Tok::Call));
        // A plain identifier is not painted as a call.
        assert!(kinds(src).iter().all(|(t, k)| *t != "flame" || *k == Tok::Plain));
    }

    /// The classic tokenizer traps: delimiters that appear inside other
    /// delimiters must not start anything.
    #[test]
    fn delimiters_inside_delimiters_are_inert() {
        let s1 = r#"let url = "http://example.com";"#;
        assert_eq!(find(s1, r#""http://example.com""#), Some(Tok::Str));
        assert!(
            !kinds(s1).iter().any(|(_, k)| *k == Tok::Comment),
            "the // inside a string must not start a comment"
        );

        let s2 = r#"// a "quote" in a comment"#;
        assert_eq!(find(s2, s2), Some(Tok::Comment));
        assert!(!kinds(s2).iter().any(|(_, k)| *k == Tok::Str));

        let s3 = r#"let s = "she said \"hi\"";"#;
        assert_lossless(s3);
        assert!(
            kinds(s3).iter().any(|(t, k)| *k == Tok::Str && t.contains("hi")),
            "escaped quotes must not end the string early"
        );
    }

    /// Half-typed input is the normal state of an editor. Nothing may
    /// panic, and everything must still tile the input.
    #[test]
    fn unterminated_input_is_safe() {
        for src in [
            "let s = \"unclosed",
            "/* unclosed block",
            "let c = 'x",
            "let s = \"trailing escape \\",
            "",
            "\"",
            "/",
            "/*",
            "0x",
            "1e",
            "1e+",
        ] {
            assert_lossless(src);
        }
    }

    /// An apostrophe in a comment must not paint the rest of the file.
    #[test]
    fn an_apostrophe_does_not_run_away() {
        let src = "// don't do this\nlet x = 1;\n";
        assert_eq!(find(src, "let"), Some(Tok::Keyword));
        assert_eq!(find(src, "1"), Some(Tok::Num));
    }

    #[test]
    fn numbers_keep_their_shape() {
        for (src, want) in [
            ("1.5", "1.5"),
            ("0x1f", "0x1f"),
            ("1_000", "1_000"),
            ("1e-9", "1e-9"),
            ("2.0e+3", "2.0e+3"),
        ] {
            assert_eq!(find(src, want), Some(Tok::Num), "{src}");
        }
    }

    /// Non-ASCII must not panic or split a char (the starters contain
    /// em-dashes and degree signs in comments).
    #[test]
    fn utf8_is_handled() {
        let src = "// ripple — 3° and ✓\nlet x = 1;\n";
        assert_lossless(src);
        assert_eq!(find(src, "let"), Some(Tok::Keyword));
    }

    // ---- the formula language ----------------------------------------

    fn formula_kinds(src: &str) -> Vec<(&str, Tok)> {
        tokenize_formula(src).into_iter().map(|(s, e, k)| (&src[s..e], k)).collect()
    }

    fn formula_find(src: &str, needle: &str) -> Option<Tok> {
        formula_kinds(src).into_iter().find(|(t, _)| *t == needle).map(|(_, k)| k)
    }

    /// Every byte of the Formula Editor's examples, in both languages,
    /// comes back -- the `.rhai` files through the Rhai tokenizer.
    #[test]
    fn covers_every_byte_of_the_formula_examples() {
        for (file, src) in crate::storage::formula_store::EXAMPLES {
            let runs = if file.ends_with(".rhai") { tokenize(src) } else { tokenize_formula(src) };
            let joined: String = runs.iter().map(|&(s, e, _)| &src[s..e]).collect();
            assert_eq!(&joined, src, "{file}: the tokenizer lost or duplicated bytes");
        }
    }

    #[test]
    fn classifies_a_formula() {
        let src = "Mandel {  ; a comment\ninit:\n  z = 0\nLOOP:\n  z = sqr(z) + #pixel * @power + (1.5e-3, 2i)\nbailout:\n  |z| <= @bailout\ndefault:\n  title = \"Mandel\"\n  float param power\n    default = 2\n  endparam\n  $define DEBUG\n}\n";
        assert_eq!(formula_find(src, "Mandel"), Some(Tok::Call), "an entry's name");
        assert_eq!(formula_find(src, "; a comment"), Some(Tok::Comment));
        assert_eq!(formula_find(src, "init"), Some(Tok::Keyword));
        assert_eq!(formula_find(src, "LOOP"), Some(Tok::Keyword), "words match without case");
        assert_eq!(formula_find(src, "sqr"), Some(Tok::Call));
        assert_eq!(formula_find(src, "#pixel"), Some(Tok::Symbol));
        assert_eq!(formula_find(src, "@power"), Some(Tok::Symbol));
        assert_eq!(formula_find(src, "1.5e-3"), Some(Tok::Num));
        assert_eq!(formula_find(src, "2i"), Some(Tok::Num), "an imaginary literal");
        assert_eq!(formula_find(src, "\"Mandel\""), Some(Tok::Str));
        assert_eq!(formula_find(src, "param"), Some(Tok::Keyword));
        assert_eq!(formula_find(src, "$define"), Some(Tok::Keyword));
        assert!(formula_kinds(src).iter().all(|(t, k)| *t != "z" || *k == Tok::Plain), "a variable is plain");
    }

    /// A `;` inside a string is not a comment; a `"` in a comment starts
    /// nothing; half-typed input tiles the input.
    #[test]
    fn formula_delimiters_and_half_typed_input() {
        let s1 = "title = \"a; b\"";
        assert_eq!(formula_find(s1, "\"a; b\""), Some(Tok::Str));
        assert!(!formula_kinds(s1).iter().any(|(_, k)| *k == Tok::Comment));
        let s2 = "; a \"quote\"\nz = 1";
        assert!(!formula_kinds(s2).iter().any(|(_, k)| *k == Tok::Str));
        for src in ["\"unclosed", "#", "@", "$", "1e", "1e+", ".", "z = 3i", "x = .5", "é ; ✓"] {
            let joined: String = tokenize_formula(src).iter().map(|&(s, e, _)| &src[s..e]).collect();
            assert_eq!(joined, src, "{src}");
        }
    }

    #[test]
    fn memo_returns_an_equal_job() {
        let src = "let x = 1; // hi";
        let f = FontId::monospace(12.0);
        let a = highlight(src, f.clone());
        let b = highlight(src, f);
        assert_eq!(a.text, b.text);
        assert_eq!(a.sections.len(), b.sections.len());
    }
}
