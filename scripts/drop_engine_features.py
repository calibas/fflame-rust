"""Remove the engine-flame / engine-escape / engine-sim / lang cfgs.

One-shot, run 2026-11-04 when the features were removed (the
single-engine gallery modules had retired). Every build turned all four on, so each predicate is evaluated with them
TRUE and every other cfg (terrain, web-app, test, target_arch, ...) left
symbolic:
  - true      -> the attribute goes, the item stays
  - false     -> the attribute and its item go (and comments directly above)
  - otherwise -> the attribute is rewritten simplified
Prints every deletion and every bare block left behind, for review.

usage: python drop_engine_features.py [--write] FILE...
"""
import re
import sys

FORCED = {"engine-flame", "engine-escape", "engine-sim", "lang"}
MENTION = re.compile(r'feature\s*=\s*"(engine-flame|engine-escape|engine-sim|lang)"')


# ---- cfg predicates -------------------------------------------------------

def parse(s):
    s = s.strip()
    pos = 0

    def ws():
        nonlocal pos
        while pos < len(s) and s[pos].isspace():
            pos += 1

    def pred():
        nonlocal pos
        ws()
        m = re.match(r'[A-Za-z_][A-Za-z0-9_]*', s[pos:])
        name = m.group(0)
        pos += len(name)
        ws()
        if name in ("all", "any", "not") and pos < len(s) and s[pos] == "(":
            pos += 1
            args = []
            ws()
            while s[pos] != ")":
                args.append(pred())
                ws()
                if s[pos] == ",":
                    pos += 1
                    ws()
            pos += 1
            return (name, args)
        if pos < len(s) and s[pos] == "=":
            pos += 1
            ws()
            m = re.match(r'"[^"]*"', s[pos:])
            pos += len(m.group(0))
            return ("kv", name, m.group(0)[1:-1])
        return ("id", name)

    p = pred()
    ws()
    assert pos == len(s), (s, pos)
    return p


def simplify(p):
    """True, False, or a predicate tree."""
    kind = p[0]
    if kind == "kv":
        if p[1] == "feature" and p[2] in FORCED:
            return True
        return p
    if kind == "id":
        return p
    args = [simplify(a) for a in p[1]]
    if kind == "not":
        a = args[0]
        if a is True:
            return False
        if a is False:
            return True
        return ("not", [a])
    if kind == "all":
        if any(a is False for a in args):
            return False
        rest = [a for a in args if a is not True]
        if not rest:
            return True
        return rest[0] if len(rest) == 1 else ("all", rest)
    if kind == "any":
        if any(a is True for a in args):
            return True
        rest = [a for a in args if a is not False]
        if not rest:
            return False
        return rest[0] if len(rest) == 1 else ("any", rest)
    raise ValueError(p)


def render(p):
    if p[0] == "kv":
        return f'{p[1]} = "{p[2]}"'
    if p[0] == "id":
        return p[1]
    return f'{p[0]}({", ".join(render(a) for a in p[1])})'


def split_top(s):
    """Split at the first top-level comma."""
    depth = 0
    in_str = False
    for i, c in enumerate(s):
        if in_str:
            if c == '"':
                in_str = False
            continue
        if c == '"':
            in_str = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
        elif c == "," and depth == 0:
            return s[:i], s[i + 1:]
    return s, None


def match_attr(line):
    """(indent, 'cfg'|'cfg_attr', inner, trailing) for a line opening with
    #[cfg(...)] or #[cfg_attr(...)], else None."""
    m = re.match(r'^(\s*)#\[(cfg|cfg_attr)\(', line)
    if not m:
        return None
    start = m.end()
    depth = 1
    in_str = False
    i = start
    while i < len(line):
        c = line[i]
        if in_str:
            if c == '"':
                in_str = False
        elif c == '"':
            in_str = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                break
        i += 1
    assert line[i + 1] == "]", line
    return m.group(1), m.group(2), line[start:i], line[i + 2:]


# ---- item extent ----------------------------------------------------------

class Scanner:
    """Bracket depth across lines, ignoring strings, chars and comments."""

    def __init__(self):
        self.depth = 0
        self.raw = None  # closing delimiter of a raw string in progress
        self.in_str = False
        self.block = 0

    def feed(self, line):
        i = 0
        n = len(line)
        while i < n:
            c = line[i]
            if self.block:
                if line.startswith("*/", i):
                    self.block -= 1
                    i += 2
                    continue
                i += 1
                continue
            if self.raw is not None:
                if line.startswith(self.raw, i):
                    i += len(self.raw)
                    self.raw = None
                    continue
                i += 1
                continue
            if self.in_str:
                if c == "\\":
                    i += 2
                    continue
                if c == '"':
                    self.in_str = False
                i += 1
                continue
            if line.startswith("//", i):
                return
            if line.startswith("/*", i):
                self.block += 1
                i += 2
                continue
            m = re.match(r'b?r(#*)"', line[i:])
            if m and (i == 0 or not (line[i - 1].isalnum() or line[i - 1] == "_")):
                self.raw = '"' + m.group(1)
                i += m.end()
                continue
            if c == '"':
                self.in_str = True
                i += 1
                continue
            if c == "'":
                m = re.match(r"'(\\.[^']*|[^'\\])'", line[i:])
                if m:
                    i += m.end()
                    continue
                i += 1  # a lifetime
                continue
            if c in "([{":
                self.depth += 1
            elif c in ")]}":
                self.depth -= 1
            i += 1


def code_part(line):
    """The line with a trailing // comment dropped (approximate)."""
    s = Scanner()
    out = []
    i = 0
    # cheap: reuse the scanner by finding the comment start
    for j in range(len(line)):
        if line.startswith("//", j):
            t = Scanner()
            t.feed(line[:j])
            if not t.in_str and t.raw is None and not t.block:
                return line[:j]
    return line


def item_end(lines, start, first):
    """Index of the last line of the item whose text begins with `first`
    (the part of lines[start] after the attribute, maybe empty)."""
    sc = Scanner()
    i = start
    text = first
    seen_code = False
    while True:
        stripped = text.strip()
        is_attr = stripped.startswith("#[") and not seen_code
        is_comment = stripped.startswith("//") and not seen_code
        sc.feed(text)
        if stripped and not is_attr and not is_comment:
            seen_code = True
        if seen_code and sc.depth == 0 and sc.raw is None and not sc.in_str:
            body = code_part(text).rstrip()
            if body.endswith((";", ",", "}")):
                return i
        i += 1
        text = lines[i]


# ---- the pass -------------------------------------------------------------

def process(path, write):
    raw = open(path, "rb").read().decode("utf-8")
    eol = "\r\n" if "\r\n" in raw else "\n"
    lines = raw.split(eol)
    out = []
    report = []
    i = 0
    changed = False
    while i < len(lines):
        line = lines[i]
        m = match_attr(line)
        if not m or not MENTION.search(m[2]):
            out.append(line)
            i += 1
            continue
        indent, kind, inner, trailing = m
        if kind == "cfg_attr":
            pred, attrs = split_top(inner)
            v = simplify(parse(pred))
            changed = True
            if v is False:
                report.append(f"  {i + 1}: cfg_attr dropped: {line.strip()}")
                if trailing.strip():
                    out.append(indent + trailing.lstrip())
            elif v is True:
                out.append(f"{indent}#[{attrs.strip()}]{trailing}")
            else:
                out.append(f"{indent}#[cfg_attr({render(v)},{attrs})]{trailing}")
            i += 1
            continue
        v = simplify(parse(inner))
        changed = True
        if v is True:
            if trailing.strip():
                out.append(indent + trailing.lstrip())
            nxt = lines[i + 1].strip() if not trailing.strip() else trailing.strip()
            if nxt == "{" or (nxt.startswith("{") and not trailing.strip()):
                report.append(f"  {i + 1}: BARE BLOCK left: {line.strip()} / {nxt}")
            i += 1
            continue
        if v is False:
            end = item_end(lines, i, trailing)
            # comments directly above go with it
            k = len(out)
            while k > 0 and out[k - 1].strip().startswith("//"):
                k -= 1
            above = out[k:]
            del out[k:]
            gone = above + [line] + lines[i + 1:end + 1]
            report.append(f"  {i + 1 - len(above)}-{end + 1}: DELETED\n" +
                          "\n".join("      | " + g for g in gone))
            i = end + 1
            continue
        out.append(f"{indent}#[cfg({render(v)})]{trailing}")
        i += 1
    if changed:
        print(path)
        for r in report:
            print(r)
        if write:
            open(path, "wb").write(eol.join(out).encode("utf-8"))


def main():
    args = sys.argv[1:]
    write = "--write" in args
    for p in [a for a in args if a != "--write"]:
        process(p, write)


if __name__ == "__main__":
    main()
