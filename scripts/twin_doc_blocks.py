"""Open every variation twin with its built-in's doc block, verbatim
(rhai-plugins P7 step 3): the `///` lines above the def in
`src/variations/defs/*.rs` -- its description and `# Authors` -- so the
twin loses neither, and the API reads them from the Rhai source as text,
as it reads them from the defs. A twin that opens with a block already is
left alone. The Rust side reads the same block: `variations::docs`.

  python scripts/twin_doc_blocks.py
"""
import os, re

DEFS = "src/variations/defs"
TWINS = "assets/variations/twins"

def blocks():
    out = {}
    for f in sorted(os.listdir(DEFS)):
        if not f.endswith(".rs"):
            continue
        lines = open(os.path.join(DEFS, f), encoding="utf-8").read().splitlines()
        for i, line in enumerate(lines):
            if not (line.startswith("pub static ") and ": VariationDef" in line):
                continue
            name = None
            for body in lines[i + 1:]:
                if body.startswith("};"):
                    break
                m = re.match(r'^    name: "([^"]*)",', body)
                if m:
                    name = m.group(1)
                    break
            start = i
            while start > 0 and lines[start - 1].lstrip().startswith("///"):
                start -= 1
            if name and start < i:
                out[name] = "\n".join(l.lstrip() for l in lines[start:i])
    return out

if __name__ == "__main__":
    docs = blocks()
    done, had, missing = 0, 0, []
    for f in sorted(os.listdir(TWINS)):
        if not f.endswith(".rhai"):
            continue
        name = f[:-5]
        path = os.path.join(TWINS, f)
        text = open(path, encoding="utf-8").read()
        if text.lstrip().startswith("///"):
            had += 1
            continue
        if name not in docs:
            missing.append(name)
            continue
        open(path, "w", encoding="utf-8", newline="\n").write(docs[name] + "\n\n" + text)
        done += 1
    print(f"{done} given their block, {had} had one, {len(missing)} with no doc to copy: {' '.join(missing)}")
