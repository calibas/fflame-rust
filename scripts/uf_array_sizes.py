"""Measurement: how UF's public library sizes its arrays (plan, item 3,
step 3f). For each entry declaring an array whose size is not a
constant, the section it is declared in and what sets the size: a
parameter, `#maxiter`, the screen (`#width` / `#height`), or a mix.

    python scripts/uf_array_sizes.py <folder of .ufm / .ucl files>

The files are third-party, read for measurement only.
"""
import re
import sys
from collections import Counter
from pathlib import Path

DECL = re.compile(r"^\s*(?:float|int|complex|bool|color)\s+([A-Za-z_]\w*)\s*\[([^\]]*)\]", re.I)
SECTION = re.compile(r"^\s*([A-Za-z]+):\s*(?:;.*)?$")
PARAM = re.compile(r"^\s*(?:float|int|complex|bool|color)?\s*param\s+([A-Za-z_]\w*)", re.I)


def entries(text):
    """(name, body lines) of each top-level `name { ... }` entry."""
    out, depth, name, body = [], 0, None, []
    for line in text.splitlines():
        code = line.split(";", 1)[0]
        if depth == 0:
            m = re.match(r"^\s*([^{\s][^{]*?)\s*\{", code)
            if m:
                name, body = m.group(1), []
                depth = code.count("{") - code.count("}")
                if depth <= 0:
                    out.append((name, body))
                    depth = 0
            continue
        depth += code.count("{") - code.count("}")
        if depth <= 0:
            out.append((name, body))
            depth = 0
        else:
            body.append(code)
    return out


def size_kind(size):
    s = size.lower()
    if not s.strip():
        return "dynamic"
    has_p = "@" in s
    has_m = "#maxiter" in s or "#maxit" in s
    has_s = "#width" in s or "#height" in s
    other_hash = re.search(r"#(?!maxiter|maxit|width|height)\w+", s)
    parts = [k for k, v in (("param", has_p), ("maxiter", has_m), ("screen", has_s)) if v]
    if other_hash:
        parts.append("other#")
    if not parts:
        return "constant-or-name" if re.search(r"[A-Za-z_]", s) else "constant"
    return "+".join(parts)


def param_maxes(body):
    """Each parameter's declared max, when it has one."""
    maxes, current = {}, None
    for line in body:
        m = PARAM.match(line)
        if m:
            current = m.group(1).lower()
            continue
        if re.match(r"^\s*endparam", line, re.I):
            current = None
            continue
        if current:
            mm = re.match(r"^\s*max\s*=\s*(.+)$", line, re.I)
            if mm:
                maxes[current] = mm.group(1).strip()
    return maxes


def main():
    folder = Path(sys.argv[1])
    by_entry = Counter()
    by_decl = Counter()
    param_with_max = Counter()
    entry_names = {}
    for path in sorted(folder.iterdir()):
        ext = path.suffix.lower()
        if ext not in (".ufm", ".ucl"):
            continue
        kind = "formula" if ext == ".ufm" else "colouring"
        text = path.read_bytes().decode("latin-1")
        for name, body in entries(text):
            section, kinds = "init", set()
            maxes = param_maxes(body)
            imports = any(re.match(r"^\s*import\b", l, re.I) for l in body)
            for line in body:
                sm = SECTION.match(line)
                if sm:
                    section = sm.group(1).lower()
                    continue
                d = DECL.match(line)
                if not d or section == "default":
                    continue
                k = size_kind(d.group(2))
                if k == "constant":
                    continue
                where = "global" if section == "global" else "per-pixel"
                kinds.add((where, k))
                by_decl[(kind, where, k)] += 1
                if k == "param":
                    ps = re.findall(r"@(\w+)", d.group(2))
                    param_with_max[all(p.lower() in maxes for p in ps)] += 1
            if kinds:
                key = (kind, "imports" if imports else "no import", tuple(sorted(kinds)))
                by_entry[key] += 1
                entry_names.setdefault(key, []).append(f"{path.name}:{name}")
    print("entries by kind, import, (section, size) set:")
    for key, n in by_entry.most_common():
        print(f"  {n:>4}  {key[0]:<9} {key[1]:<9} {', '.join(f'{w} {k}' for w, k in key[2])}")
        if "--names" in sys.argv:
            for e in entry_names[key][:6]:
                print(f"          {e}")
    print("declarations by (kind, section, size):")
    for key, n in by_decl.most_common():
        print(f"  {n:>4}  {key}")
    print(f"parameter-sized declarations whose parameters all declare a max: {param_with_max[True]} of {sum(param_with_max.values())}")


if __name__ == "__main__":
    main()
