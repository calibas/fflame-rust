"""One-shot fixer for the one-parameter-system plan's migration steps
(docs/archive/projects/one-parameter-system.md): read `cargo check`'s
`no field `X` on type `...ParamDef`` errors and rewrite exactly those
field accesses as `ParamDef`'s -- `.default` -> `.slot_default()`,
`.min` / `.max` -> `.slot_range().0` / `.1`, `.display_name` ->
`.label`, `.tooltip` -> `.hint`. Each site is checked to read the field
before it is rewritten; anything else is left for a hand.

    cargo check --release --lib --tests 2>&1 | python scripts/params_field_fix.py
"""

import re
import sys
from collections import defaultdict

FIELDS = {
    "default": "slot_default()",
    "default_value": "slot_default()",
    "min": "slot_range().0",
    "max": "slot_range().1",
    "display_name": "label",
    "tooltip": "hint",
}

text = sys.stdin.read()
sites = defaultdict(set)
for m in re.finditer(r"error\[E0609\]: no field `(\w+)` on type `&?(?:params::)?ParamDef`\n\s*--> ([^:\n]+):(\d+):(\d+)", text):
    field, path, line, col = m.group(1), m.group(2).replace("\\", "/"), int(m.group(3)), int(m.group(4))
    if field in FIELDS:
        sites[path].add((line, col, field))

# A `Cow` name, label or hint where a `&str` was (`p.name` -> `&*p.name`).
for m in re.finditer(r"error\[E0(?:308|507)\]: (?:mismatched types|cannot move out of `[\w.]+` which is behind a shared reference)\n\s*--> ([^:\n]+):(\d+):(\d+)\n(?:.*\n){0,3}?.*(?:expected `&(?:str|_)`, found `Cow<'_, str>`|does not implement the `Copy` trait)", text):
    path, line, col = m.group(1).replace("\\", "/"), int(m.group(2)), int(m.group(3))
    sites[path].add((line, col, "&*"))

# A `Cow` cloned where a `String` was (`p.name.clone()` -> `p.name.to_string()`).
for m in re.finditer(r"error\[E0308\]: mismatched types\n\s*--> ([^:\n]+):(\d+):(\d+)\n(?:.*\n){0,3}?.*expected `String`, found `Cow<'_, str>`", text):
    path, line, col = m.group(1).replace("\\", "/"), int(m.group(2)), int(m.group(3))
    sites[path].add((line, col, "to_string"))

fixed = 0
for path, at in sites.items():
    raw = open(path, "rb").read().decode("utf-8")
    crlf = "\r\n" in raw
    lines = raw.replace("\r\n", "\n").split("\n")
    for line, col, field in sorted(at, reverse=True):
        s = lines[line - 1]
        # rustc's column is 1-based, in chars.
        i = col - 1
        if field == "to_string":
            m = re.match(r"[A-Za-z_][\w.]*\.(?:name|label|hint)\.clone\(\)", s[i:])
            if not m:
                print(f"{path}:{line}:{col}: expected a cloned name, label or hint, left alone: {s.strip()}", file=sys.stderr)
                continue
            lines[line - 1] = s[:i] + m.group(0)[:-len("clone()")] + "to_string()" + s[i + m.end():]
            fixed += 1
            continue
        if field == "&*":
            if not re.match(r"[A-Za-z_][\w.]*\.(name|label|hint)\b", s[i:]):
                print(f"{path}:{line}:{col}: expected a name, label or hint, left alone: {s.strip()}", file=sys.stderr)
                continue
            lines[line - 1] = s[:i] + "&*" + s[i:]
            fixed += 1
            continue
        if s[i:i + len(field)] != field or (i > 0 and s[i - 1] != "."):
            print(f"{path}:{line}:{col}: expected `.{field}`, left alone", file=sys.stderr)
            continue
        lines[line - 1] = s[:i] + FIELDS[field] + s[i + len(field):]
        fixed += 1
    out = "\n".join(lines)
    if crlf:
        out = out.replace("\n", "\r\n")
    open(path, "wb").write(out.encode("utf-8"))
print(f"{fixed} field accesses rewritten in {len(sites)} files")
