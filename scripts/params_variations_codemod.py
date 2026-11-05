"""One-shot codemod for the one-parameter-system plan's variations step
(docs/archive/projects/one-parameter-system.md, step 7), run after
`params_codemod.py --struct VariationParamDef`:

- each parameter `variations::SLOW_PARAMS` names becomes slow where it
  is defined: `param!(slow ...)`, or `.slow()` on a `ParamDef::new`'s
  `Spec` (the literal it was, converted);
- `VariationParamDef` and `ParamType` leave every `defs/` file's
  `use crate::variations::{...}` (the macro names its own paths), and a
  file that writes `ParamDef::new` gains `use crate::params::{ParamDef, Spec};`.

    python scripts/params_variations_codemod.py
"""

import glob
import re
import sys

DEFS = "src/variations/defs"


def load(p):
    raw = open(p, "rb").read().decode("utf-8")
    return raw.replace("\r\n", "\n"), "\r\n" in raw


def save(p, s, crlf):
    if crlf:
        s = s.replace("\n", "\r\n")
    open(p, "wb").write(s.encode("utf-8"))


def slow_list():
    s, _ = load("src/variations/mod.rs")
    a = s.index("pub const SLOW_PARAMS")
    return re.findall(r'\("([^"]+)", "([^"]+)"\)', s[a:s.index("];", a)])


def def_span(s, variation):
    """The text of the `VariationDef` named `variation`: from its
    `name: "..."` to the next `pub static`."""
    m = re.search(r'\bname:\s*"' + re.escape(variation) + r'"', s)
    if not m:
        return None
    end = s.find("pub static ", m.end())
    return m.start(), len(s) if end < 0 else end


files = sorted(glob.glob(f"{DEFS}/*.rs"))
texts = {p: load(p) for p in files}
for variation, param in slow_list():
    done = False
    for p, (s, crlf) in texts.items():
        span = def_span(s, variation)
        if not span:
            continue
        a, b = span
        body = s[a:b]
        m = re.search(r'param!\(\s*"' + re.escape(param) + r'"', body)
        if not m and ('param!(slow "' + param + '"') in s:
            # Marked by hand where a macro shares it (`fractwf_params!`).
            done = True
            break
        if m:
            body = body[:m.start()] + 'param!(slow "' + param + '"' + body[m.end():]
        else:
            m = re.search(r'ParamDef::new\(\s*"' + re.escape(param) + r'",.*?(Spec::\w+\([^()]*\)(?:\.\w+\(\))*)', body, re.S)
            if not m:
                sys.exit(f"{variation}.{param}: defined in {p}, but neither a param! nor a ParamDef::new")
            body = body[:m.end(1)] + ".slow()" + body[m.end(1):]
        texts[p] = (s[:a] + body + s[b:], crlf)
        done = True
        break
    if not done:
        sys.exit(f"{variation}.{param}: no definition found")
    print(f"{variation}.{param}: slow")

for p, (s, crlf) in texts.items():
    def trim(m):
        # The two names out, the list's layout kept.
        body = m.group(1)
        for n in ("VariationParamDef", "ParamType"):
            body = re.sub(r"\b" + n + r"\b\s*,[ \t]*", "", body)
            body = re.sub(r",\s*\b" + n + r"\b", "", body)
        return "use crate::variations::{" + body + "};"
    # The list may nest (`definition::{Feature, VariationDef, ...}`): to
    # its matching brace.
    at = s.find("use crate::variations::{")
    while at >= 0:
        open_brace = at + len("use crate::variations::")
        depth, k = 0, open_brace
        while True:
            depth += {"{": 1, "}": -1}.get(s[k], 0)
            if depth == 0:
                break
            k += 1
        m = re.match(r"(?s)use crate::variations::\{(.*)\}", s[at:k + 1])
        replaced = trim(m)
        replaced = replaced[:-1]  # trim() closes with `};`; the `;` follows
        s = s[:at] + replaced + s[k + 1:]
        at = s.find("use crate::variations::{", at + len(replaced))
    if "ParamDef::new(" in s and "use crate::params::" not in s:
        s = s.replace("use crate::param;\n", "use crate::param;\nuse crate::params::{ParamDef, Spec};\n", 1)
        if "use crate::params::" not in s:
            first = s.index("\nuse ") + 1
            s = s[:first] + "use crate::params::{ParamDef, Spec};\n" + s[first:]
    save(p, s, crlf)
print(f"{len(texts)} files")
