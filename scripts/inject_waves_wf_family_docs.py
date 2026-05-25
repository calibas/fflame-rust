"""Apply doc-comments + per-param descriptions to waves_wf_family.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

JF = ["Joel Faber"]

DOC = {
    "WAVES2_WF": (
        "WF waves2 (single-trig perturbation) — adds a per-axis sin or cos wave to the input: `output = (x + dampX·scalex·trig(y·freqx), y + dampY·scaley·trig(x·freqy))`, with the whole output scaled by the per-axis damping factors. `use_cos_x`/`use_cos_y` pick sin or cos per axis.",
        JF,
    ),
    "WAVES3_WF": (
        "WF waves3 (squared-trig perturbation) — same structure as `waves2_wf` but the trig terms are squared (`trig²`), producing always-positive waves with double the apparent frequency.",
        JF,
    ),
    "WAVES4_WF": (
        "WF waves4 (triple-trig product perturbation) — same structure as `waves2_wf` but the trig terms are triple products: `cos·sin·cos` when `use_cos = 1`, `sin·cos·sin` when `use_cos = 0`. Produces a sharper, more lobed wave shape.",
        JF,
    ),
    "DINIS_SURFACE_WF": (
        "Dini's Surface parametric mapping — emits `(a·cos(u)·sin(v), a·sin(u)·sin(v), −(a·(cos(v) + log tan(v/2)) + b·u))` where `(u, v) = (x, y)`. Dini's surface is a helicoid of constant negative Gaussian curvature, a generalization of the pseudosphere. See [mathworld.wolfram.com/DinisSurface](https://mathworld.wolfram.com/DinisSurface.html).",
        None,
    ),
}

WAVES_PARAMS = {
    "scalex": "X-axis wave amplitude.",
    "scaley": "Y-axis wave amplitude.",
    "freqx": "X-axis wave frequency (applied to the Y input).",
    "freqy": "Y-axis wave frequency (applied to the X input).",
    "use_cos_x": "1 = use cosine on the X axis; 0 = use sine.",
    "use_cos_y": "1 = use cosine on the Y axis; 0 = use sine.",
    "dampx": "X-axis exponential damping factor — the output is multiplied by `exp(dampx)` (or 1 when `|dampx|` is tiny).",
    "dampy": "Y-axis exponential damping factor — same as dampx but for Y.",
}

PARAM_DOC = {}
for static_name in ("WAVES2_WF", "WAVES3_WF", "WAVES4_WF"):
    for k, v in WAVES_PARAMS.items():
        PARAM_DOC[(static_name, k)] = v

PARAM_DOC[("DINIS_SURFACE_WF", "a")] = "Radial scale of the surface (controls XY radius and Z magnitude)."
PARAM_DOC[("DINIS_SURFACE_WF", "b")] = "Helical twist coefficient — multiplies the input U to add a linear helical Z offset."

PATH = "src/variations/defs/waves_wf_family.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

inserted = 0
already = 0
for name, (body, authors) in DOC.items():
    target = f"\npub static {name}: VariationDef = VariationDef {{"
    if target not in src:
        print(f"  WARN: no match for {name}")
        continue
    idx = src.find(target)
    prefix = src[:idx]
    last_nl = prefix.rfind("\n")
    if last_nl != -1:
        prev_line = src[last_nl + 1:idx + 1].strip()
        if prev_line.startswith("///"):
            already += 1
            continue
    lines = []
    for paragraph in body.split("\n"):
        wrapped = textwrap.fill(paragraph, width=72) if paragraph.strip() else ""
        for line in wrapped.split("\n"):
            lines.append(f"/// {line}".rstrip())
    if authors:
        lines.append("///")
        lines.append("/// # Authors")
        for author in authors:
            lines.append(f"/// - {author}")
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  pass1: inserted {inserted}, already {already}")


def find_macro_close(text: str, open_paren_idx: int) -> int:
    assert text[open_paren_idx] == "("
    depth = 1
    i = open_paren_idx + 1
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


pinserted = 0
palready = 0
for (static_name, param_name), desc in PARAM_DOC.items():
    start_pattern = f"pub static {static_name}: VariationDef"
    start_idx = src.find(start_pattern)
    if start_idx == -1:
        continue
    next_static = src.find("\npub static ", start_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)
    block = src[start_idx:end_idx]

    head = f'param!("{param_name}"'
    head_idx = block.find(head)
    if head_idx == -1:
        print(f"  WARN: no param {static_name}.{param_name}")
        continue
    open_idx = block.find("(", head_idx)
    close_idx = find_macro_close(block, open_idx)
    if close_idx == -1:
        print(f"  WARN: unbalanced param {static_name}.{param_name}")
        continue
    inner = block[open_idx + 1:close_idx]
    depth = 0
    in_str = False
    commas = 0
    j = 0
    while j < len(inner):
        ch = inner[j]
        if in_str:
            if ch == "\\":
                j += 2
                continue
            if ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
            elif ch == "," and depth == 0:
                commas += 1
        j += 1
    if commas >= 6:
        palready += 1
        continue

    new_block = (
        block[:close_idx]
        + f', "{desc}"'
        + block[close_idx:]
    )
    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
