"""One-shot codemod (2026-10-18, the KIFS plan's C1e): give every
`CustomSource { .. }` literal the new `variations` field, empty.

`CustomSource` gained `variations: Vec<CustomSource>`, the sources of the
variations an entry calls with `vary`. Every construction in the crate
is a literal; each gets `variations: Vec::new()` before its closing
brace. Run from the repository root; idempotent.
"""
import io
import pathlib
import re

FIELD = "variations: Vec::new()"


def closing(s, open_at):
    """The index of the brace closing the one at `open_at`."""
    depth = 0
    i = open_at
    while i < len(s):
        c = s[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
        elif c == '"':
            # Skip a string literal (with escapes).
            i += 1
            while s[i] != '"':
                i += 2 if s[i] == "\\" else 1
        i += 1
    raise ValueError("unbalanced")


def fix(text):
    out = []
    pos = 0
    for m in re.finditer(r"CustomSource \{", text):
        before = text[max(0, m.start() - 7):m.start()]
        # The definition, and a function returning one (its body's brace).
        if before == "struct " or before.endswith("-> "):
            continue
        if m.start() < pos:
            continue
        open_at = m.end() - 1
        close = closing(text, open_at)
        body = text[open_at + 1:close]
        if "variations" in body:
            continue
        out.append(text[pos:close])
        stripped = body.rstrip()
        if "\n" in body and stripped.endswith(","):
            # A multi-line literal: a line of its own, at the fields' indent.
            last_line = stripped.split("\n")[-1]
            indent = last_line[: len(last_line) - len(last_line.lstrip())]
            close_indent = body[len(stripped):]
            out[-1] = text[pos:open_at + 1] + stripped + "\n" + indent + FIELD + "," + close_indent
        else:
            out[-1] = text[pos:open_at + 1] + stripped + (", " if not stripped.endswith(",") else " ") + FIELD + " "
        pos = close
    out.append(text[pos:])
    return "".join(out)


for path in pathlib.Path("src").rglob("*.rs"):
    raw = io.open(path, encoding="utf-8", newline="").read()
    if "CustomSource {" not in raw:
        continue
    new = fix(raw)
    if new != raw:
        io.open(path, "w", encoding="utf-8", newline="").write(new)
        print("updated", path)
