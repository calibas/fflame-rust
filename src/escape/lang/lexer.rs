//! Tokens of an entry's body.
//!
//! Identifiers are case-insensitive in both languages, so they are
//! lowered here and every later stage compares lowered names. Newlines
//! are tokens: a line ends a statement (and in Fractint's files so does
//! a comma at the top level, which the parser decides, since commas
//! also separate arguments).
//!
//! UF's compiler directives are handled here (its reference, "Compiler
//! directives"): `$define`, `$undef`, `$ifdef`, `$else`, `$endif`, case
//! insensitive, each on its own line, scoped to the entry. Ultra
//! Fractal 6 always defines `ULTRAFRACTAL` and `VER20` to `VER60`, and
//! so does this.

use super::{Diag, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// A name, lowered.
    Ident(String),
    /// `#name`, lowered, without the `#`.
    Hash(String),
    /// `@name`, lowered, without the `@`.
    At(String),
    Int(i64),
    Float(f64),
    /// An imaginary literal, `4i` or `2.5i`.
    Imag(f64),
    Str(String),
    Newline,
    Colon,
    Comma,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Assign,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Bar,
    Dot,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

/// The symbols Ultra Fractal 6 defines for every entry.
const PREDEFINED: &[&str] = &["ultrafractal", "ver20", "ver30", "ver40", "ver50", "ver60"];

/// Lex `source[start..end]`. Spans are offsets into the whole source,
/// so messages point at the file the user is editing.
pub fn lex(source: &str, start: usize, end: usize, diags: &mut Vec<Diag>) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = start;
    let push = |out: &mut Vec<Token>, tok: Tok, a: usize, b: usize| out.push(Token { tok, span: Span::new(a, b) });
    let mut defined: Vec<String> = PREDEFINED.iter().map(|s| s.to_string()).collect();
    // Each open `$ifdef`: whether its current branch is taken.
    let mut conds: Vec<bool> = Vec::new();
    while i < end {
        let c = bytes[i];
        if c == b'$' {
            i = directive(source, i, end, &mut defined, &mut conds, diags);
            continue;
        }
        // A branch not taken: its lines are skipped whole, up to the
        // next directive.
        if conds.iter().any(|taken| !taken) {
            if c == b'\n' {
                push(&mut out, Tok::Newline, i, i + 1);
            }
            i += 1;
            continue;
        }
        match c {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                push(&mut out, Tok::Newline, i, i + 1);
                i += 1;
            }
            // A comment runs to the end of the line.
            b';' => {
                while i < end && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            // A backslash at the end of a line continues it.
            b'\\' => {
                let mut j = i + 1;
                while j < end && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\r') {
                    j += 1;
                }
                if j < end && bytes[j] == b'\n' {
                    i = j + 1;
                } else {
                    diags.push(Diag::error(Span::new(i, i + 1), "unexpected `\\`"));
                    i += 1;
                }
            }
            b'"' => {
                let a = i;
                i += 1;
                let mut text = String::new();
                let mut piece = i;
                let mut closed = false;
                while i < end {
                    match bytes[i] {
                        b'"' => {
                            text.push_str(&source[piece..i]);
                            i += 1;
                            closed = true;
                            break;
                        }
                        b'\n' => break,
                        // A backslash ending the line continues the
                        // string on the next, without that line's indent.
                        b'\\' => {
                            let mut j = i + 1;
                            while j < end && matches!(bytes[j], b' ' | b'\t' | b'\r') {
                                j += 1;
                            }
                            if j < end && bytes[j] == b'\n' {
                                text.push_str(&source[piece..i]);
                                j += 1;
                                while j < end && matches!(bytes[j], b' ' | b'\t') {
                                    j += 1;
                                }
                                piece = j;
                                i = j;
                            } else {
                                i += 1;
                            }
                        }
                        _ => i += 1,
                    }
                }
                if closed {
                    push(&mut out, Tok::Str(text), a, i);
                } else {
                    diags.push(Diag::error(Span::new(a, i), "unterminated string"));
                }
            }
            b'0'..=b'9' | b'.' if c != b'.' || (i + 1 < end && bytes[i + 1].is_ascii_digit()) => {
                let a = i;
                let mut is_float = false;
                while i < end && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i < end && bytes[i] == b'.' {
                    is_float = true;
                    i += 1;
                    while i < end && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                // An exponent, only when digits follow: `2e` is a number
                // and a name in Fractint's world as much as anywhere.
                if i < end && (bytes[i] == b'e' || bytes[i] == b'E') {
                    let mut j = i + 1;
                    if j < end && (bytes[j] == b'+' || bytes[j] == b'-') {
                        j += 1;
                    }
                    if j < end && bytes[j].is_ascii_digit() {
                        while j < end && bytes[j].is_ascii_digit() {
                            j += 1;
                        }
                        is_float = true;
                        i = j;
                    }
                }
                let text = &source[a..i];
                // `4i` is imaginary; `4if` is not (a name follows).
                let imag = i < end
                    && (bytes[i] == b'i' || bytes[i] == b'I')
                    && !(i + 1 < end && is_ident_byte(bytes[i + 1]));
                let value: f64 = text.parse().unwrap_or(0.0);
                if imag {
                    i += 1;
                    push(&mut out, Tok::Imag(value), a, i);
                } else if is_float {
                    push(&mut out, Tok::Float(value), a, i);
                } else {
                    match text.parse::<i64>() {
                        Ok(v) => push(&mut out, Tok::Int(v), a, i),
                        Err(_) => push(&mut out, Tok::Float(value), a, i),
                    }
                }
            }
            b'#' | b'@' => {
                let a = i;
                i += 1;
                let b = i;
                while i < end && is_ident_byte(bytes[i]) {
                    i += 1;
                }
                if b == i {
                    diags.push(Diag::error(Span::new(a, i), format!("`{}` must be followed by a name", c as char)));
                    continue;
                }
                let name = source[b..i].to_ascii_lowercase();
                push(&mut out, if c == b'#' { Tok::Hash(name) } else { Tok::At(name) }, a, i);
            }
            _ if is_ident_start(c) => {
                let a = i;
                while i < end && is_ident_byte(bytes[i]) {
                    i += 1;
                }
                push(&mut out, Tok::Ident(source[a..i].to_ascii_lowercase()), a, i);
            }
            _ => {
                let a = i;
                let two = if i + 1 < end { &bytes[i..i + 2] } else { &bytes[i..i + 1] };
                let (tok, len) = match two {
                    b"==" => (Tok::Eq, 2),
                    b"!=" => (Tok::Ne, 2),
                    b"<=" => (Tok::Le, 2),
                    b">=" => (Tok::Ge, 2),
                    b"&&" => (Tok::AndAnd, 2),
                    b"||" => (Tok::OrOr, 2),
                    _ => match c {
                        b':' => (Tok::Colon, 1),
                        b',' => (Tok::Comma, 1),
                        b'(' => (Tok::LParen, 1),
                        b')' => (Tok::RParen, 1),
                        b'[' => (Tok::LBracket, 1),
                        b']' => (Tok::RBracket, 1),
                        b'+' => (Tok::Plus, 1),
                        b'-' => (Tok::Minus, 1),
                        b'*' => (Tok::Star, 1),
                        b'/' => (Tok::Slash, 1),
                        b'%' => (Tok::Percent, 1),
                        b'^' => (Tok::Caret, 1),
                        b'=' => (Tok::Assign, 1),
                        b'<' => (Tok::Lt, 1),
                        b'>' => (Tok::Gt, 1),
                        b'!' => (Tok::Bang, 1),
                        b'|' => (Tok::Bar, 1),
                        b'.' => (Tok::Dot, 1),
                        // Fractint writes `&` for `&&`.
                        b'&' => (Tok::AndAnd, 1),
                        _ => {
                            let ch = source[i..].chars().next().unwrap_or('?');
                            diags.push(Diag::error(Span::new(i, i + ch.len_utf8()), format!("unexpected character `{ch}`")));
                            i += ch.len_utf8();
                            continue;
                        }
                    },
                };
                i += len;
                push(&mut out, tok, a, i);
            }
        }
    }
    if !conds.is_empty() {
        diags.push(Diag::error(Span::new(end, end), "an `$ifdef` has no `$endif`"));
    }
    out.push(Token { tok: Tok::Eof, span: Span::new(end, end) });
    out
}

/// One directive line from the `$` at `at`: applied, and the position
/// after its line returned (the newline is left for the caller).
fn directive(source: &str, at: usize, end: usize, defined: &mut Vec<String>, conds: &mut Vec<bool>, diags: &mut Vec<Diag>) -> usize {
    let line_end = source[at..end].find('\n').map_or(end, |k| at + k);
    // A comment may follow.
    let line = source[at + 1..line_end].split(';').next().unwrap_or("");
    let mut words = line.split_whitespace();
    let word = words.next().unwrap_or("").to_ascii_lowercase();
    let symbol = words.next().map(str::to_ascii_lowercase);
    let span = Span::new(at, line_end);
    let skipping = conds.iter().any(|taken| !taken);
    match (word.as_str(), symbol) {
        ("define", Some(s)) if !skipping => {
            if !defined.contains(&s) {
                defined.push(s);
            }
        }
        ("undef", Some(s)) if !skipping => defined.retain(|d| *d != s),
        ("define" | "undef", _) if skipping => {}
        ("ifdef", Some(s)) => conds.push(defined.contains(&s)),
        ("else", _) => match conds.last_mut() {
            Some(taken) => *taken = !*taken,
            None => diags.push(Diag::error(span, "`$else` without `$ifdef`")),
        },
        ("endif", _) => {
            if conds.pop().is_none() {
                diags.push(Diag::error(span, "`$endif` without `$ifdef`"));
            }
        }
        ("define" | "undef" | "ifdef", None) => diags.push(Diag::error(span, format!("`${word}` needs a symbol"))),
        _ => diags.push(Diag::error(span, format!("unknown directive `${word}`"))),
    }
    line_end
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        let mut d = Vec::new();
        let t = lex(s, 0, s.len(), &mut d);
        assert!(d.is_empty(), "{d:?}");
        t.into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn numbers_names_and_operators() {
        assert_eq!(
            toks("Z = z^2 + #Pixel ; comment\n|z| <= 4"),
            vec![
                Tok::Ident("z".into()),
                Tok::Assign,
                Tok::Ident("z".into()),
                Tok::Caret,
                Tok::Int(2),
                Tok::Plus,
                Tok::Hash("pixel".into()),
                Tok::Newline,
                Tok::Bar,
                Tok::Ident("z".into()),
                Tok::Bar,
                Tok::Le,
                Tok::Int(4),
                Tok::Eof,
            ]
        );
        assert_eq!(toks("1.5e3 .5 2.8 + 4i 3e"), vec![
            Tok::Float(1500.0),
            Tok::Float(0.5),
            Tok::Float(2.8),
            Tok::Plus,
            Tok::Imag(4.0),
            Tok::Int(3),
            Tok::Ident("e".into()),
            Tok::Eof,
        ]);
        assert_eq!(toks("@Bailout && a || b & c"), vec![
            Tok::At("bailout".into()),
            Tok::AndAnd,
            Tok::Ident("a".into()),
            Tok::OrOr,
            Tok::Ident("b".into()),
            Tok::AndAnd,
            Tok::Ident("c".into()),
            Tok::Eof,
        ]);
    }

    #[test]
    fn continuation_strings_and_errors() {
        assert_eq!(toks("a = \\\n 1"), vec![Tok::Ident("a".into()), Tok::Assign, Tok::Int(1), Tok::Eof]);
        assert_eq!(toks("\"Bail-out value\""), vec![Tok::Str("Bail-out value".into()), Tok::Eof]);
        // A string continued over lines, each next line's indent dropped.
        assert_eq!(
            toks("\"solved by \\\n      Newton's method \\  \r\n\tto zero.\""),
            vec![Tok::Str("solved by Newton's method to zero.".into()), Tok::Eof]
        );
        let mut d = Vec::new();
        lex("a $ b", 0, 5, &mut d);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains('$'));
        let mut d = Vec::new();
        lex("\"open\nb", 0, 7, &mut d);
        assert!(d[0].message.contains("unterminated"));
    }

    /// UF's directives: its version symbols defined, a branch not taken
    /// skipped whole, `$define` and `$undef` scoped to the entry.
    #[test]
    fn directives_choose_lines() {
        let src = "$IFDEF VER40\na = 1\n$Else\nb = \"never \\\n read\n$endif\n$ifdef VER70\nc = 1\n$endif\n\
                   $define mine ; a comment\n$ifdef MINE\nd = 1\n$undef mine\n$endif\n$ifdef mine\ne = 1\n$endif";
        let names: Vec<_> = toks(src)
            .into_iter()
            .filter_map(|t| if let Tok::Ident(n) = t { Some(n) } else { None })
            .collect();
        assert_eq!(names, ["a", "d"]);
        for bad in ["$endif", "$ifdef x", "$else", "$pragma x", "$define"] {
            let mut d = Vec::new();
            lex(bad, 0, bad.len(), &mut d);
            assert_eq!(d.len(), 1, "{bad}: {d:?}");
        }
    }
}
