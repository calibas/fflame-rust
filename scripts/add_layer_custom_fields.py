"""One-shot codemod (rhai-plugins P4 step 4): give every `SimLayer` and
`SimColorLayer` struct literal its new carried-source field
(`custom_model: None`, `custom_coloring: None`).

Literals that end in a struct update (`..x`) already take the field from
it and are left alone, as are the struct definitions and `impl` headers.
Run from the repository root: python scripts/add_layer_custom_fields.py
"""

import re
import sys

FILES = [
    "src/config/sim.rs",
    "src/sim/app_repro_test.rs",
    "src/sim/mod.rs",
    "src/sim/terrain.rs",
    "src/ui/sim_panel.rs",
]
FIELDS = {"SimLayer": "custom_model", "SimColorLayer": "custom_coloring"}
LITERAL = re.compile(r"\b(SimLayer|SimColorLayer) \{")


def close_of(s, open_at):
    depth = 0
    for i in range(open_at, len(s)):
        if s[i] == "{":
            depth += 1
        elif s[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    raise ValueError("unbalanced")


def edit(s):
    out = []
    at = 0
    count = 0
    for m in LITERAL.finditer(s):
        line_start = s.rfind("\n", 0, m.start()) + 1
        head = s[line_start:m.start()]
        if re.search(r"\b(struct|impl|for)\s*$", head) or "impl Default for" in head:
            continue
        open_at = m.end() - 1
        if open_at < at:
            continue
        close = close_of(s, open_at)
        body = s[open_at + 1:close]
        if re.search(r"\.\.\s*[A-Za-z_(]", body) or FIELDS[m.group(1)] + ":" in body:
            continue
        field = FIELDS[m.group(1)]
        if "\n" in body:
            # Multi-line: a line of its own, indented as the last field.
            last = body.rstrip().rsplit("\n", 1)[-1]
            indent = re.match(r"\s*", last).group(0)
            text = body.rstrip()
            if not text.endswith(","):
                text += ","
            nl = "\r\n" if "\r\n" in body else "\n"
            closing_indent = body[len(body.rstrip()):]
            new_body = text + nl + indent + field + ": None," + closing_indent
        else:
            text = body.rstrip()
            text = text[:-1] if text.endswith(",") else text
            new_body = text + ", " + field + ": None "
        out.append(s[at:open_at + 1])
        out.append(new_body)
        at = close
        count += 1
    out.append(s[at:])
    return "".join(out), count


def main():
    total = 0
    for path in FILES:
        s = open(path, encoding="utf-8", newline="").read()
        new, n = edit(s)
        if n:
            open(path, "w", encoding="utf-8", newline="").write(new)
        print(f"{path}: {n}")
        total += n
    print(f"total {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
