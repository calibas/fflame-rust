"""One-shot codemod for the one-parameter-system plan
(docs/projects/one-parameter-system.md, steps 4 and 5): rewrite every
`<Struct> { name, display_name, default, min, max, tooltip, choices }`
literal -- `sim::SimParamDef`, `escape::EscapeParamDef`, which share the
shape -- as `ParamDef::new(name, label, hint, Spec::...)`.

A literal with choices becomes `Spec::choice(default, choices)` (its min
and max are its indices, checked against the snapshot before the move);
any other `Spec::float(default, min, max)`. A parameter named slow
(`--slow owner.param,...`) gains `.slow()`, one whose range spans five
decades from above zero (`--log-decades`; escape's slider rule) or is
named (`--log owner.param,...`) gains `.log()`. A literal's owner is the
last `name: "..."` before it outside a parameter literal.

    python scripts/params_codemod.py --struct SimParamDef \
        --slow lenia.radius,... src/sim/models.rs src/sim/colorings.rs

`--check output/params-snapshot.json --kinds sim-model,sim-coloring`
compares the (owner, name) pairs found with the snapshot's, and
`--dry-run` writes nothing.
"""

import argparse
import json
import re
import struct
import sys


def scan(src):
    """Yield (index, char, in_code) over Rust source: in_code is False
    inside comments, strings, raw strings and char literals."""
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                yield k, src[k], False
            i = j
            continue
        if src.startswith("/*", i):
            depth, k = 1, i + 2
            while k < n and depth:
                if src.startswith("/*", k):
                    depth, k = depth + 1, k + 2
                elif src.startswith("*/", k):
                    depth, k = depth - 1, k + 2
                else:
                    k += 1
            for m in range(i, k):
                yield m, src[m], False
            i = k
            continue
        m = re.compile(r'b?r(#*)"').match(src, i)
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            end = src.find('"' + m.group(1), m.end())
            end = n if end < 0 else end + 1 + len(m.group(1))
            for k in range(i, end):
                yield k, src[k], False
            i = end
            continue
        if c == '"' or (c == "b" and src.startswith('b"', i) and (i == 0 or not src[i - 1].isalnum())):
            k = i + (2 if c == "b" else 1)
            while k < n and src[k] != '"':
                k += 2 if src[k] == "\\" else 1
            for m2 in range(i, k + 1):
                yield m2, src[m2], False
            i = k + 1
            continue
        if c == "'":
            # A char literal ('x', '\n', '\u{..}'), else a lifetime or label.
            if i + 1 < n and src[i + 1] == "\\":
                k = src.find("'", i + 2)
                for m2 in range(i, k + 1):
                    yield m2, src[m2], False
                i = k + 1
                continue
            if i + 2 < n and src[i + 2] == "'":
                for m2 in range(i, i + 3):
                    yield m2, src[m2], False
                i += 3
                continue
        yield i, c, True
        i += 1


def literals(src, struct):
    """The spans (start, open_brace, end) of every `struct { ... }`
    literal in code."""
    code = bytearray(len(src))
    for i, _, in_code in scan(src):
        code[i] = in_code
    out = []
    for m in re.finditer(r"\b" + struct + r"\s*\{", src):
        if not code[m.start()] or src[:m.start()].rstrip().endswith("->"):
            continue
        open_brace = m.end() - 1
        depth, k = 0, open_brace
        while True:
            if code[k]:
                if src[k] in "{[(":
                    depth += 1
                elif src[k] in "}])":
                    depth -= 1
                    if depth == 0:
                        break
            k += 1
        out.append((m.start(), open_brace, k + 1))
    return out, code


def fields(src, code, open_brace, end):
    """A literal's fields, by name: their source text, as written."""
    body_start, body_end = open_brace + 1, end - 1
    parts, depth, start = [], 0, body_start
    for k in range(body_start, body_end):
        if not code[k]:
            continue
        if src[k] in "{[(":
            depth += 1
        elif src[k] in "}])":
            depth -= 1
        elif src[k] == "," and depth == 0:
            parts.append(src[start:k])
            start = k + 1
    if src[start:body_end].strip():
        parts.append(src[start:body_end])
    out = {}
    for p in parts:
        key, colon, value = p.strip().partition(":")
        # `default,` is `default: default,`.
        out[key.strip()] = value.strip() if colon else key.strip()
    return out


def f32(x):
    return struct.unpack("f", struct.pack("f", x))[0]


def five_decades(lo, hi):
    """`escape_panel::param_slider`'s rule, in its f32s: 0.001..100 is
    not five decades there."""
    return lo > 0 and f32(f32(hi) / f32(lo)) >= f32(1e5)


def number(text):
    try:
        return float(text.rstrip("f32").rstrip("_"))
    except ValueError:
        return None


def as_index(text):
    v = number(text)
    if v is not None and v == int(v):
        return str(int(v))
    return f"({text}) as usize"


def owners(src, code, spans):
    """Each literal's owner: the last `name: "..."` before it in code,
    outside a parameter literal."""
    inside = bytearray(len(src))
    for s, _, e in spans:
        for k in range(s, e):
            inside[k] = 1
    names = [(m.start(), m.group(1)) for m in re.finditer(r'\bname:\s*"([^"]*)"', src) if code[m.start()] and not inside[m.start()]]
    out = []
    for s, _, _ in spans:
        before = [n for at, n in names if at < s]
        out.append(before[-1] if before else None)
    return out


def rewrite(src, struct, slow, log, log_decades):
    spans, code = literals(src, struct)
    found = []
    pieces, last = [], 0
    for (s, ob, e), owner in zip(spans, owners(src, code, spans)):
        f = fields(src, code, ob, e)
        missing = {"name", "display_name", "default", "min", "max", "tooltip", "choices"} - set(f)
        if missing:
            sys.exit(f"{struct} literal at {src.count(chr(10), 0, s) + 1} lacks {sorted(missing)}")
        name = json.loads(f["name"]) if f["name"].startswith('"') else None
        if name is not None:
            found.append((owner, name))
        choices = f["choices"]
        if choices.replace(" ", "") in ("&[]",):
            spec = f"Spec::float({f['default']}, {f['min']}, {f['max']})"
            lo, hi = number(f["min"]), number(f["max"])
            if f"{owner}.{name}" in log or (log_decades and lo is not None and hi is not None and five_decades(lo, hi)):
                spec += ".log()"
        else:
            spec = f"Spec::choice({as_index(f['default'])}, {choices})"
        if f"{owner}.{name}" in slow:
            spec += ".slow()"
        line_start = src.rfind("\n", 0, s) + 1
        indent = re.match(r"[ \t]*", src[line_start:]).group(0)
        inner = indent + "    "
        args = [f["name"], f["display_name"], f["tooltip"], spec]
        pieces.append(src[last:s])
        pieces.append("ParamDef::new(\n" + "".join(f"{inner}{a},\n" for a in args) + indent + ")")
        last = e
    pieces.append(src[last:])
    return "".join(pieces), found


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--struct", required=True)
    ap.add_argument("--slow", default="")
    ap.add_argument("--log", default="")
    ap.add_argument("--log-decades", action="store_true")
    ap.add_argument("--check")
    ap.add_argument("--kinds", default="")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("files", nargs="+")
    a = ap.parse_args()
    slow = set(filter(None, a.slow.split(",")))
    log = set(filter(None, a.log.split(",")))
    found, outputs = [], {}
    for path in a.files:
        raw = open(path, "rb").read().decode("utf-8")
        crlf = "\r\n" in raw
        src = raw.replace("\r\n", "\n")
        out, f = rewrite(src, a.struct, slow, log, a.log_decades)
        found += f
        if crlf:
            out = out.replace("\n", "\r\n")
        outputs[path] = out
        print(f"{path}: {len(f)} literals")
    unused = {s for s in slow if tuple(s.split(".", 1)) not in set(found)}
    if unused:
        sys.exit(f"slow parameters not found: {sorted(unused)}")
    if a.check:
        rows = json.load(open(a.check, encoding="utf-8"))
        kinds = set(a.kinds.split(","))
        want = sorted((r["owner"], r["name"]) for r in rows if r["kind"] in kinds)
        got = sorted(found)
        w, g = set(want), set(got)
        if g - w:
            sys.exit(f"pairs the snapshot does not have: {sorted(g - w)[:10]}")
        # A literal named by an expression (a const fn's) is checked by
        # the snapshot compare after, not here.
        if w - g:
            print(f"{len(w - g)} snapshot pairs from literals named by an expression: {sorted(w - g)}")
        print(f"{len(g)} (owner, name) pairs match the snapshot")
    if not a.dry_run:
        for path, out in outputs.items():
            open(path, "wb").write(out.encode("utf-8"))


if __name__ == "__main__":
    main()
