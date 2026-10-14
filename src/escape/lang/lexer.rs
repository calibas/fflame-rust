//! Tokens of an entry's body.
//!
//! Identifiers are case-insensitive in both languages, so they are
//! lowered here and every later stage compares lowered names. Newlines
//! are tokens: a line ends a statement (and in Fractint's files so does
//! a comma at the top level, which the parser decides, since commas
//! also separate arguments).

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

/// Lex `source[start..end]`. Spans are offsets into the whole source,
/// so messages point at the file the user is editing.
pub fn lex(source: &str, start: usize, end: usize, diags: &mut Vec<Diag>) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = start;
    let push = |out: &mut Vec<Token>, tok: Tok, a: usize, b: usize| out.push(Token { tok, span: Span::new(a, b) });
    while i < end {
        let c = bytes[i];
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
                while i < end && bytes[i] != b'"' && bytes[i] != b'\n' {
                    i += 1;
                }
                if i < end && bytes[i] == b'"' {
                    text.push_str(&source[a + 1..i]);
                    i += 1;
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
    out.push(Token { tok: Tok::Eof, span: Span::new(end, end) });
    out
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
        let mut d = Vec::new();
        lex("a $ b", 0, 5, &mut d);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains('$'));
    }
}
