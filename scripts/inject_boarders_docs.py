"""Apply doc-comments + per-param descriptions to boarders.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
BOARDERS handled manually (existing # Authors block needed prepending)."""
import re
import textwrap

DOC = {
    "BOARDERS2": (
        "Variant of Boarders with 3 tunable knobs — scale factor `c` and per-direction `left`/`right` offsets controlling the border-push behavior.",
        ["Xyrus02"],
    ),
    "PRE_BOARDERS2": (
        "Pre-phase version of Boarders 2 — same border-warp math but applied before the rest of the variations run.",
        ["Xyrus02"],
    ),
    "SPLITBRDR": (
        "Combines a Bubble warp (radial sphere projection) with a Boarders-style cell-grid border. The `x`/`y` parameters control the border behavior; `px`/`py` add an extra linear pass-through component.",
        ["FracFx"],
    ),
}

PARAM_DOC = {
    ("BOARDERS2", "c"): "Cell scale factor — how much the in-cell offset shrinks toward the cell center.",
    ("BOARDERS2", "left"): "Border push-out distance — how far points get pushed toward the nearest cell border.",
    ("BOARDERS2", "right"): "Threshold for border behavior vs pass-through. Higher = more points get pushed to borders.",

    ("PRE_BOARDERS2", "c"): "Cell scale factor — how much the in-cell offset shrinks toward the cell center.",
    ("PRE_BOARDERS2", "left"): "Border push-out distance — how far points get pushed toward the nearest cell border.",
    ("PRE_BOARDERS2", "right"): "Threshold for border behavior vs pass-through. Higher = more points get pushed to borders.",

    ("SPLITBRDR", "x"): "Border push offset in one direction.",
    ("SPLITBRDR", "y"): "Border push offset in the other direction.",
    ("SPLITBRDR", "px"): "Linear pass-through scaling for X — adds a fraction of the input X to the output.",
    ("SPLITBRDR", "py"): "Linear pass-through scaling for Y.",
}

PATH = "src/variations/defs/boarders.rs"
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

    macro_pat = re.compile(
        r'(param!\(\s*"' + re.escape(param_name) + r'"\s*,[^)]*?)(\))',
        re.DOTALL,
    )
    new_block, n = macro_pat.subn(
        lambda m: m.group(1) + f', "{desc}"' + m.group(2),
        block, count=1,
    )
    if n == 0:
        already_pat = re.compile(
            r'param!\(\s*"' + re.escape(param_name) + r'"[^)]*"' + re.escape(desc[:10]),
            re.DOTALL,
        )
        if already_pat.search(block):
            palready += 1
        else:
            print(f"  WARN: no param {static_name}.{param_name}")
        continue
    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
