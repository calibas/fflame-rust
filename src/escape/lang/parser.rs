//! Formula files to parse trees.
//!
//! Two levels. A file is split into entries by scanning the raw text
//! (an entry's name may hold almost any character but white space, so
//! it is not a token), then each body is lexed and parsed into sections
//! of statements, or of settings for `default:` and `switch:`.

use super::ast::*;
use super::lexer::{lex, Tok, Token};
use super::{Diag, Span};

/// Parse a whole file. Errors in one entry do not stop the others.
pub fn parse_file(source: &str) -> (File, Vec<Diag>) {
    let mut diags = Vec::new();
    let mut file = File::default();
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    loop {
        i = skip_blank(bytes, i, true);
        if i >= n {
            break;
        }
        let name_start = i;
        while i < n && !bytes[i].is_ascii_whitespace() && bytes[i] != b'(' && bytes[i] != b'{' && bytes[i] != b';' {
            i += 1;
        }
        let name = &source[name_start..i];
        if name.is_empty() {
            diags.push(Diag::error(Span::new(i, i + 1), "expected an entry name"));
            i = next_line(bytes, i);
            continue;
        }
        i = skip_blank(bytes, i, false);
        let mut setting = None;
        if i < n && bytes[i] == b'(' {
            let a = i + 1;
            while i < n && bytes[i] != b')' && bytes[i] != b'\n' {
                i += 1;
            }
            if i < n && bytes[i] == b')' {
                setting = Some(source[a..i].trim().to_string());
                i += 1;
            } else {
                diags.push(Diag::error(Span::new(a - 1, i), "unclosed `(` after the entry name"));
            }
        }
        i = skip_blank(bytes, i, true);
        if i >= n || bytes[i] != b'{' {
            diags.push(Diag::error(Span::new(name_start, i.min(n)), format!("expected `{{` after `{name}`")));
            i = next_line(bytes, i);
            continue;
        }
        let body_start = i + 1;
        let Some(close) = matching_brace(bytes, body_start) else {
            diags.push(Diag::error(Span::new(name_start, n), format!("`{name}` has no closing `}}`")));
            break;
        };
        i = close + 1;
        if name.eq_ignore_ascii_case("comment") {
            continue;
        }
        let tokens = lex(source, body_start, close, &mut diags);
        let mut p = Parser { toks: tokens, pos: 0, diags: &mut diags, modulus_depth: 0 };
        let mut entry = Entry {
            name: name.to_string(),
            setting,
            span: Span::new(name_start, close + 1),
            body: Span::new(body_start, close),
            sections: Vec::new(),
            default: Settings::default(),
            switch: Settings::default(),
        };
        p.body(&mut entry);
        file.entries.push(entry);
    }
    (file, diags)
}

/// Skip white space and comments; newlines too when `lines`.
fn skip_blank(bytes: &[u8], mut i: usize, lines: bool) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' if lines => i += 1,
            b';' if lines => i = next_line(bytes, i),
            _ => break,
        }
    }
    i
}

fn next_line(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i + 1
}

/// The `}` closing a body that starts at `i`, skipping comments and
/// strings (a brace in either is text).
fn matching_brace(bytes: &[u8], mut i: usize) -> Option<usize> {
    let mut depth = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b';' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'{' => depth += 1,
            b'}' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Words that end a block; a statement never starts with one.
const BLOCK_ENDS: &[&str] = &["endif", "else", "elseif", "endwhile", "until"];

struct Parser<'d> {
    toks: Vec<Token>,
    pos: usize,
    diags: &'d mut Vec<Diag>,
    /// How many `|...|` are open: inside one, a `|` in operator
    /// position closes it rather than starting anything.
    modulus_depth: usize,
}

/// Where the body currently is.
enum Mode {
    Statements(SectionKind, bool),
    Default,
    Switch,
}

impl Parser<'_> {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn peek_at(&self, k: usize) -> &Tok {
        &self.toks[(self.pos + k).min(self.toks.len() - 1)].tok
    }

    fn span(&self) -> Span {
        self.toks[self.pos].span
    }

    fn prev_span(&self) -> Span {
        self.toks[self.pos.saturating_sub(1)].span
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == tok {
            self.bump();
            true
        } else {
            false
        }
    }

    fn is_ident(&self, word: &str) -> bool {
        matches!(self.peek(), Tok::Ident(w) if w == word)
    }

    fn error(&mut self, span: Span, msg: impl Into<String>) {
        self.diags.push(Diag::error(span, msg));
    }

    /// Skip to the end of the line: recovery after an error.
    fn recover(&mut self) {
        while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
            self.bump();
        }
    }

    fn skip_separators(&mut self) {
        while matches!(self.peek(), Tok::Newline | Tok::Comma) {
            self.bump();
        }
    }

    /// A section label at the current position: `name:` with a known
    /// name, or Fractint's bare `:`. Returns the label and consumes it.
    fn label(&mut self) -> Option<String> {
        match (self.peek().clone(), self.peek_at(1).clone()) {
            (Tok::Ident(w), Tok::Colon)
                if SectionKind::from_label(&w).is_some() || w == "default" || w == "switch" =>
            {
                self.bump();
                self.bump();
                Some(w)
            }
            (Tok::Colon, _) => {
                self.bump();
                Some(String::new())
            }
            _ => None,
        }
    }

    fn body(&mut self, entry: &mut Entry) {
        let mut mode = Mode::Statements(SectionKind::Init, false);
        let mut current: Vec<Stmt> = Vec::new();
        let mut start = self.span();
        let mut labelled = false;
        let finish = |entry: &mut Entry, mode: &Mode, body: Vec<Stmt>, span: Span, labelled: bool, diags: &mut Vec<Diag>| {
            if let Mode::Statements(kind, fractint_loop) = mode {
                // An unlabelled prefix with nothing in it is not a section.
                if !labelled && body.is_empty() {
                    return;
                }
                if entry.sections.iter().any(|s| s.kind == *kind) {
                    diags.push(Diag::error(span, format!("a second `{}:` section", kind.label())));
                    return;
                }
                entry.sections.push(Section {
                    kind: *kind,
                    span,
                    body,
                    last_is_bailout: *fractint_loop,
                    labelled,
                });
            }
        };
        loop {
            self.skip_separators();
            if matches!(self.peek(), Tok::Eof) {
                break;
            }
            let at = self.span();
            if let Some(label) = self.label() {
                let body = std::mem::take(&mut current);
                finish(entry, &mode, body, start.to(self.prev_span()), labelled, self.diags);
                start = at;
                labelled = true;
                mode = match label.as_str() {
                    "" => Mode::Statements(SectionKind::Loop, true),
                    "default" => Mode::Default,
                    "switch" => Mode::Switch,
                    l => Mode::Statements(SectionKind::from_label(l).expect("a label"), false),
                };
                continue;
            }
            match mode {
                Mode::Default => {
                    let mut s = std::mem::take(&mut entry.default);
                    self.settings(&mut s);
                    entry.default = s;
                }
                Mode::Switch => {
                    let mut s = std::mem::take(&mut entry.switch);
                    self.settings(&mut s);
                    entry.switch = s;
                }
                Mode::Statements(..) => {
                    if let Some(s) = self.statement() {
                        current.push(s);
                    }
                }
            }
        }
        let end = self.span();
        finish(entry, &mode, current, start.to(end), labelled, self.diags);
    }

    /// A block of statements up to one of `ends` (not consumed).
    fn block(&mut self, ends: &[&str], opener: Span) -> Vec<Stmt> {
        let mut out = Vec::new();
        loop {
            self.skip_separators();
            match self.peek().clone() {
                Tok::Ident(w) if ends.contains(&w.as_str()) => return out,
                Tok::Eof | Tok::Colon => {
                    self.error(opener, format!("missing `{}`", ends.last().unwrap_or(&"end")));
                    return out;
                }
                Tok::Ident(w) if matches!(self.peek_at(1), Tok::Colon) && SectionKind::from_label(&w).is_some() => {
                    self.error(opener, format!("missing `{}`", ends.last().unwrap_or(&"end")));
                    return out;
                }
                _ => {
                    if let Some(s) = self.statement() {
                        out.push(s);
                    }
                }
            }
        }
    }

    fn statement(&mut self) -> Option<Stmt> {
        let start = self.span();
        let stmt = match self.peek().clone() {
            Tok::Ident(w) if TypeName::from_name(&w).is_some() && matches!(self.peek_at(1), Tok::Ident(_)) => {
                let ty = TypeName::from_name(&w).expect("a type");
                self.bump();
                let Tok::Ident(name) = self.bump().tok else { unreachable!() };
                if matches!(self.peek(), Tok::LBracket) {
                    self.error(self.span(), "arrays are not supported yet");
                    self.recover();
                    return None;
                }
                let init = if self.eat(&Tok::Assign) { Some(self.expr()) } else { None };
                Stmt::Decl { ty, name, init, span: start.to(self.prev_span()) }
            }
            Tok::Ident(w) if w == "if" => {
                self.bump();
                let mut arms = vec![(self.expr(), Vec::new())];
                arms[0].1 = self.block(&["elseif", "else", "endif"], start);
                let mut otherwise = None;
                loop {
                    if self.is_ident("elseif") {
                        self.bump();
                        let cond = self.expr();
                        let body = self.block(&["elseif", "else", "endif"], start);
                        arms.push((cond, body));
                    } else if self.is_ident("else") {
                        self.bump();
                        otherwise = Some(self.block(&["endif"], start));
                    } else {
                        break;
                    }
                }
                self.eat(&Tok::Ident("endif".into()));
                Stmt::If { arms, otherwise, span: start.to(self.prev_span()) }
            }
            Tok::Ident(w) if w == "while" => {
                self.bump();
                let cond = self.expr();
                let body = self.block(&["endwhile"], start);
                self.eat(&Tok::Ident("endwhile".into()));
                Stmt::While { cond, body, span: start.to(self.prev_span()) }
            }
            Tok::Ident(w) if w == "repeat" => {
                self.bump();
                let body = self.block(&["until"], start);
                let cond = if self.eat(&Tok::Ident("until".into())) {
                    self.expr()
                } else {
                    Expr { kind: ExprKind::Bool(true), span: start }
                };
                Stmt::Repeat { body, cond, span: start.to(self.prev_span()) }
            }
            Tok::Ident(w) if BLOCK_ENDS.contains(&w.as_str()) => {
                self.error(start, format!("`{w}` without its opening statement"));
                self.bump();
                return None;
            }
            _ => Stmt::Expr(self.expr()),
        };
        // A statement ends at a line, a comma (Fractint), the loop's
        // `:`, or the end; anything else is a stray token.
        match self.peek() {
            Tok::Newline | Tok::Comma | Tok::Colon | Tok::Eof => {}
            Tok::Ident(w) if BLOCK_ENDS.contains(&w.as_str()) => {}
            _ => {
                let at = self.span();
                self.error(at, "expected the end of the statement");
                self.recover();
            }
        }
        Some(stmt)
    }

    // ---- expressions -------------------------------------------------

    fn expr(&mut self) -> Expr {
        let lhs = self.logic();
        if self.eat(&Tok::Assign) {
            let rhs = self.expr();
            let span = lhs.span.to(rhs.span);
            return Expr { kind: ExprKind::Assign(Box::new(lhs), Box::new(rhs)), span };
        }
        lhs
    }

    fn logic(&mut self) -> Expr {
        let mut lhs = self.comparison();
        loop {
            let op = match self.peek() {
                Tok::AndAnd => BinOp::And,
                // Inside `|...|`, `||` is two closing bars.
                Tok::OrOr if self.modulus_depth == 0 => BinOp::Or,
                _ => return lhs,
            };
            self.bump();
            let rhs = self.comparison();
            lhs = binary(op, lhs, rhs);
        }
    }

    fn comparison(&mut self) -> Expr {
        let mut lhs = self.additive();
        loop {
            let op = match self.peek() {
                Tok::Eq => BinOp::Eq,
                Tok::Ne => BinOp::Ne,
                Tok::Lt => BinOp::Lt,
                Tok::Le => BinOp::Le,
                Tok::Gt => BinOp::Gt,
                Tok::Ge => BinOp::Ge,
                _ => return lhs,
            };
            self.bump();
            let rhs = self.additive();
            lhs = binary(op, lhs, rhs);
        }
    }

    fn additive(&mut self) -> Expr {
        let mut lhs = self.multiplicative();
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return lhs,
            };
            self.bump();
            let rhs = self.multiplicative();
            lhs = binary(op, lhs, rhs);
        }
    }

    fn multiplicative(&mut self) -> Expr {
        let mut lhs = self.power();
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => return lhs,
            };
            self.bump();
            let rhs = self.power();
            lhs = binary(op, lhs, rhs);
        }
    }

    /// `^`, left-associative, BELOW the unary operators: UF's table
    /// puts negation at 7 and power at 6, so `-x^2` is `(-x)^2`.
    fn power(&mut self) -> Expr {
        let mut lhs = self.unary();
        while self.eat(&Tok::Caret) {
            let rhs = self.unary();
            lhs = binary(BinOp::Pow, lhs, rhs);
        }
        lhs
    }

    fn unary(&mut self) -> Expr {
        let start = self.span();
        match self.peek() {
            Tok::Minus => {
                self.bump();
                let e = self.unary();
                let span = start.to(e.span);
                Expr { kind: ExprKind::Unary(UnOp::Neg, Box::new(e)), span }
            }
            Tok::Plus => {
                self.bump();
                self.unary()
            }
            Tok::Bang => {
                self.bump();
                let e = self.unary();
                let span = start.to(e.span);
                Expr { kind: ExprKind::Unary(UnOp::Not, Box::new(e)), span }
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Expr {
        let start = self.span();
        let at = |kind: ExprKind, span: Span| Expr { kind, span };
        match self.peek().clone() {
            Tok::Int(v) => {
                self.bump();
                at(ExprKind::Int(v), start)
            }
            Tok::Float(v) => {
                self.bump();
                at(ExprKind::Float(v), start)
            }
            Tok::Imag(v) => {
                self.bump();
                at(ExprKind::Imag(v), start)
            }
            Tok::Ident(w) if w == "true" || w == "false" => {
                self.bump();
                at(ExprKind::Bool(w == "true"), start)
            }
            Tok::Ident(w) => {
                self.bump();
                if matches!(self.peek(), Tok::LParen) {
                    let args = self.call_args();
                    at(ExprKind::Call { name: w, args, user: false }, start.to(self.prev_span()))
                } else {
                    at(ExprKind::Name(w), start)
                }
            }
            Tok::Hash(w) => {
                self.bump();
                at(ExprKind::Predef(w), start)
            }
            Tok::At(w) => {
                self.bump();
                if matches!(self.peek(), Tok::LParen) {
                    let args = self.call_args();
                    at(ExprKind::Call { name: w, args, user: true }, start.to(self.prev_span()))
                } else {
                    at(ExprKind::Param(w), start)
                }
            }
            Tok::LParen => {
                self.bump();
                let saved = self.modulus_depth;
                self.modulus_depth = 0;
                let first = self.expr();
                let e = if self.eat(&Tok::Comma) {
                    let second = self.expr();
                    ExprKind::Complex(Box::new(first), Box::new(second))
                } else {
                    first.kind
                };
                self.modulus_depth = saved;
                if !self.eat(&Tok::RParen) {
                    let s = self.span();
                    self.error(s, "expected `)`");
                }
                at(e, start.to(self.prev_span()))
            }
            Tok::Bar | Tok::OrOr => {
                // `||` here opens two moduli: split it.
                if matches!(self.peek(), Tok::OrOr) {
                    self.split_double_bar();
                }
                self.bump();
                self.modulus_depth += 1;
                let e = self.expr();
                self.modulus_depth -= 1;
                if matches!(self.peek(), Tok::OrOr) {
                    self.split_double_bar();
                }
                if !self.eat(&Tok::Bar) {
                    let s = self.span();
                    self.error(s, "expected a closing `|`");
                }
                at(ExprKind::Modulus(Box::new(e)), start.to(self.prev_span()))
            }
            Tok::Str(_) => {
                self.bump();
                self.error(start, "a string is not a value here");
                at(ExprKind::Int(0), start)
            }
            _ => {
                self.error(start, "expected a value");
                // Do not consume a statement boundary: the caller
                // recovers from it.
                if !matches!(self.peek(), Tok::Newline | Tok::Eof | Tok::Comma | Tok::Colon) {
                    self.bump();
                }
                at(ExprKind::Int(0), start)
            }
        }
    }

    /// Replace the current `||` with two `|` tokens.
    fn split_double_bar(&mut self) {
        let span = self.span();
        let mid = span.start + 1;
        self.toks[self.pos] = Token { tok: Tok::Bar, span: Span::new(span.start, mid) };
        self.toks.insert(self.pos + 1, Token { tok: Tok::Bar, span: Span::new(mid, span.end) });
    }

    fn call_args(&mut self) -> Vec<Expr> {
        self.bump(); // (
        let saved = self.modulus_depth;
        self.modulus_depth = 0;
        let mut args = Vec::new();
        if !matches!(self.peek(), Tok::RParen) {
            loop {
                args.push(self.expr());
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.modulus_depth = saved;
        if !self.eat(&Tok::RParen) {
            let s = self.span();
            self.error(s, "expected `)` after the arguments");
        }
        args
    }

    // ---- settings ------------------------------------------------------

    /// One line of a `default:` or `switch:` section: a setting or a
    /// block.
    fn settings(&mut self, out: &mut Settings) {
        let start = self.span();
        let mut ty = None;
        if let Tok::Ident(w) = self.peek().clone() {
            if let Some(t) = TypeName::from_name(&w) {
                if matches!(self.peek_at(1), Tok::Ident(k) if k == "param" || k == "func") {
                    ty = Some(t);
                    self.bump();
                }
            }
        }
        match self.peek().clone() {
            Tok::Ident(w) if w == "param" || w == "func" => {
                self.bump();
                let name = match self.bump().tok {
                    Tok::Ident(n) | Tok::At(n) => n,
                    _ => {
                        self.error(start, format!("expected a name after `{w}`"));
                        self.recover();
                        return;
                    }
                };
                let end = if w == "param" { "endparam" } else { "endfunc" };
                let settings = self.block_settings(end, start);
                let span = start.to(self.prev_span());
                if w == "param" {
                    out.params.push(ParamBlock { ty, name, settings, span });
                } else {
                    out.funcs.push(FuncBlock { ty, name, settings, span });
                }
            }
            Tok::Ident(w) if w == "heading" => {
                self.bump();
                // Headings group parameters in UF's dialog; the panel
                // keeps block order, so their settings are not needed.
                let _ = self.block_settings("endheading", start);
            }
            Tok::Ident(_) => {
                if let Some(s) = self.setting() {
                    out.items.push(s);
                }
            }
            _ => {
                self.error(start, "expected a setting");
                self.recover();
            }
        }
    }

    fn block_settings(&mut self, end: &str, opener: Span) -> Vec<Setting> {
        let mut out = Vec::new();
        loop {
            self.skip_separators();
            match self.peek().clone() {
                Tok::Ident(w) if w == end => {
                    self.bump();
                    return out;
                }
                Tok::Eof => {
                    self.error(opener, format!("missing `{end}`"));
                    return out;
                }
                Tok::Ident(_) => {
                    if let Some(s) = self.setting() {
                        out.push(s);
                    }
                }
                _ => {
                    let s = self.span();
                    self.error(s, "expected a setting");
                    self.recover();
                }
            }
        }
    }

    /// `name = value`.
    fn setting(&mut self) -> Option<Setting> {
        let start = self.span();
        let Tok::Ident(name) = self.bump().tok else { return None };
        if !self.eat(&Tok::Assign) {
            self.error(start, format!("expected `=` after `{name}`"));
            self.recover();
            return None;
        }
        let value = match self.peek().clone() {
            Tok::Str(_) => {
                let mut strings = Vec::new();
                while let Tok::Str(s) = self.peek().clone() {
                    strings.push(s);
                    self.bump();
                }
                SettingValue::Strings(strings)
            }
            // `default = sin()`: a function, not a call.
            Tok::Ident(f) if matches!(self.peek_at(1), Tok::LParen) && matches!(self.peek_at(2), Tok::RParen) => {
                self.bump();
                self.bump();
                self.bump();
                SettingValue::Func(f)
            }
            _ => SettingValue::Expr(self.expr()),
        };
        if !matches!(self.peek(), Tok::Newline | Tok::Eof) {
            let s = self.span();
            self.error(s, "expected the end of the setting");
            self.recover();
        }
        Some(Setting { name, value, span: start.to(self.prev_span()) })
    }
}

fn binary(op: BinOp, l: Expr, r: Expr) -> Expr {
    let span = l.span.to(r.span);
    Expr { kind: ExprKind::Binary(op, Box::new(l), Box::new(r)), span }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> File {
        let (f, d) = parse_file(src);
        assert!(d.is_empty(), "{}", d.iter().map(|d| d.render(src)).collect::<Vec<_>>().join("\n"));
        f
    }

    /// The expression as a fully parenthesised string, for precedence.
    fn show(e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(v) => v.to_string(),
            ExprKind::Float(v) => v.to_string(),
            ExprKind::Imag(v) => format!("{v}i"),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Complex(a, b) => format!("({}, {})", show(a), show(b)),
            ExprKind::Name(n) => n.clone(),
            ExprKind::Predef(n) => format!("#{n}"),
            ExprKind::Param(n) => format!("@{n}"),
            ExprKind::Call { name, args, user } => format!(
                "{}{name}({})",
                if *user { "@" } else { "" },
                args.iter().map(show).collect::<Vec<_>>().join(", ")
            ),
            ExprKind::Unary(UnOp::Neg, e) => format!("(-{})", show(e)),
            ExprKind::Unary(UnOp::Not, e) => format!("(!{})", show(e)),
            ExprKind::Binary(op, l, r) => format!("({} {} {})", show(l), op.symbol(), show(r)),
            ExprKind::Modulus(e) => format!("|{}|", show(e)),
            ExprKind::Assign(t, v) => format!("({} = {})", show(t), show(v)),
        }
    }

    fn stmt_expr(s: &Stmt) -> String {
        match s {
            Stmt::Expr(e) => show(e),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn a_uf_formula_parses_into_its_sections() {
        let f = parse_ok(
            "Mandelbrot(XAXIS) {\n\
             ; the classic\n\
             init:\n  z = 0\n  complex c = #pixel\n\
             loop:\n  z = z^2 + c\n\
             bailout:\n  |z| < @bailout\n\
             default:\n  title = \"Mandelbrot\"\n  center = (-0.5, 0)\n\
             float param bailout\n    caption = \"Bail-out value\"\n    default = 4.0\n    min = 1\n    exponential = true\n  endparam\n\
             func fn1\n    default = sqr()\n  endfunc\n\
             }\n\
             comment {\n  this is ignored }\n",
        );
        assert_eq!(f.entries.len(), 1);
        let e = &f.entries[0];
        assert_eq!(e.name, "Mandelbrot");
        assert_eq!(e.setting.as_deref(), Some("XAXIS"));
        let kinds: Vec<_> = e.sections.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, vec![SectionKind::Init, SectionKind::Loop, SectionKind::Bailout]);
        assert_eq!(stmt_expr(&e.sections[1].body[0]), "(z = ((z ^ 2) + c))");
        assert_eq!(stmt_expr(&e.sections[2].body[0]), "(|z| < @bailout)");
        assert!(matches!(&e.sections[0].body[1], Stmt::Decl { ty: TypeName::Complex, name, .. } if name == "c"));
        assert!(matches!(e.default.get("title").unwrap().value, SettingValue::Strings(ref s) if s == &["Mandelbrot"]));
        let p = &e.default.params[0];
        assert_eq!((p.ty, p.name.as_str()), (Some(TypeName::Float), "bailout"));
        assert!(matches!(p.get("default").unwrap().value, SettingValue::Expr(Expr { kind: ExprKind::Float(v), .. }) if v == 4.0));
        assert!(matches!(e.default.funcs[0].get("default").unwrap().value, SettingValue::Func(ref f) if f == "sqr"));
    }

    #[test]
    fn a_fractint_formula_parses_its_init_and_bailout() {
        let f = parse_ok("MyFrm (XAXIS) { z = pixel, c = p1 : z = sqr(z) + c, |z| <= 4 }\n");
        let e = &f.entries[0];
        assert_eq!(e.sections.len(), 2);
        assert_eq!(e.sections[0].kind, SectionKind::Init);
        assert!(!e.sections[0].labelled);
        assert_eq!(e.sections[0].body.len(), 2);
        let lp = &e.sections[1];
        assert!(lp.last_is_bailout);
        assert_eq!(lp.body.len(), 2);
        assert_eq!(stmt_expr(&lp.body[1]), "(|z| <= 4)");
    }

    #[test]
    fn precedence_follows_uf_s_table() {
        let f = parse_ok("P {\nloop:\n a = -x^2\n b = 2^3^2\n c = 1 + 2 * 3 - 4 / 5 % 6\n d = a < b && !c || d == e\n e = ||z| - 1|\n f = (1, 2) + 3i\n}");
        let got: Vec<String> = f.entries[0].sections[0].body.iter().map(stmt_expr).collect();
        assert_eq!(
            got,
            vec![
                // Negation binds tighter than ^ (UF: 7 against 6).
                "(a = ((-x) ^ 2))",
                // ^ is left-associative.
                "(b = ((2 ^ 3) ^ 2))",
                "(c = ((1 + (2 * 3)) - ((4 / 5) % 6)))",
                // && and || share one level, left to right.
                "(d = (((a < b) && (!c)) || (d == e)))",
                "(e = |(|z| - 1)|)",
                "(f = ((1, 2) + 3i))",
            ]
        );
    }

    #[test]
    fn control_flow_parses_in_both_spellings() {
        let f = parse_ok(
            "C {\ninit:\n i = 0\nloop:\n if i > 2\n  z = 1\n elseif(i == 1)\n  z = 2\n else\n  z = 3\n endif\n \
             while i < 10\n  i = i + 1\n endwhile\n repeat\n  i = i - 1\n until i <= 0\n}\n",
        );
        let lp = &f.entries[0].sections[1];
        assert!(matches!(&lp.body[0], Stmt::If { arms, otherwise: Some(_), .. } if arms.len() == 2));
        assert!(matches!(&lp.body[1], Stmt::While { body, .. } if body.len() == 1));
        assert!(matches!(&lp.body[2], Stmt::Repeat { body, .. } if body.len() == 1));
    }

    #[test]
    fn a_colouring_without_labels_is_one_unlabelled_section() {
        let f = parse_ok("Basic {\n #index = #numiter / 32\n}\n");
        let s = &f.entries[0].sections[0];
        assert!(!s.labelled);
        assert_eq!(stmt_expr(&s.body[0]), "(#index = (#numiter / 32))");
    }

    #[test]
    fn errors_point_at_the_problem_and_parsing_continues() {
        let src = "Bad {\nloop:\n z = (1 + \n endif\n}\nGood {\nloop:\n z = 1\n}\nNoBrace\n";
        let (f, d) = parse_file(src);
        let msgs: Vec<String> = d.iter().map(|d| d.render(src)).collect();
        assert!(msgs.iter().any(|m| m.starts_with("3:")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("without its opening")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("expected `{` after `NoBrace`")), "{msgs:?}");
        assert_eq!(f.entries.len(), 2, "the good entry survives the bad one");
        assert_eq!(f.entries[1].name, "Good");
        let (_, d) = parse_file("Dup {\nloop:\n z = 1\nloop:\n z = 2\n}");
        assert!(d.iter().any(|d| d.message.contains("second `loop:`")));
    }
}
