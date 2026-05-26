"""Apply doc-comments + per-param descriptions to internal_weight.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "LOONIE3": (
        "Variant 3 of Loonie — same coin-shape inversion as Loonie but uses `(r²)²/x²` for the radial threshold check, producing a stretched single-arm form.",
        ["dark-beam"],
    ),
    "LOONIE_3D": (
        "3D version of Loonie — inverts points inside a sphere sized by the variation's weight; Z gets folded through an atan2 substitution for non-zero depth handling.",
        ["Larry Berlin"],
    ),
    "SIGMOID": (
        "Saturating sigmoid in both axes — pushes coordinates through `1/(1 + exp(...))` to compress them toward the [-1, 1] range. `shiftx` / `shifty` control how steep the saturation curve is on each axis.",
        ["Xyrus02", "Brad Stefanov"],
    ),
    "BLOCKY": (
        "2D-block warp — maps points through an ellipse-bounded arctan to produce angular blocky patterns. `mp` controls block size; `x` and `y` set per-axis aspect.",
        ["Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("SIGMOID", "shiftx"): "X-axis saturation curve. Higher absolute value = steeper transition.",
    ("SIGMOID", "shifty"): "Y-axis saturation curve. Higher absolute value = steeper transition.",

    ("BLOCKY", "x"): "X-axis scaling on the arctan output.",
    ("BLOCKY", "y"): "Y-axis scaling on the arctan output.",
    ("BLOCKY", "mp"): "Block size — smaller values produce more, finer blocks.",
}

PATH = "src/variations/defs/internal_weight.rs"
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
