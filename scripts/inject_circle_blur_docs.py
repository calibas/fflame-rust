"""Apply doc-comments + per-param descriptions to circle_blur.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "CIRCLEBLUR": (
        "Pure random sample inside the unit disc — replaces the input with a uniformly random point in the unit circle (area-uniform via `sqrt(rand)` radius).",
        ["Zy0rg"],
    ),
    "CIRCLESPLIT": (
        "Pass-through inside a `radius − split` disc; outside, points get pushed outward by `split`. Creates a visible gap ring between the inner and outer regions.",
        ["Tatyana Zabanova", "Brad Stefanov"],
    ),
    "FLIPCIRCLE": (
        "Flips the Y coordinate inside a circular region sized by the variation's own weight. Outside the circle, points pass through unchanged.",
        ["Michael Faber"],
    ),
    "BLUR_LINEAR": (
        "Directional linear-segment blur — each iteration scatters the point along a fixed-angle line of random length up to `length`. Produces directional streaks.",
        ["Joel Faber", "DarkBeam"],
    ),
}

PARAM_DOC = {
    ("CIRCLESPLIT", "radius"): "Distance from origin where the splitting starts.",
    ("CIRCLESPLIT", "split"): "Gap size — how far outside the radius points get pushed.",

    ("BLUR_LINEAR", "length"): "Maximum scatter distance along the blur direction.",
    ("BLUR_LINEAR", "angle"): "Direction of the blur, in radians.",
}

PATH = "src/variations/defs/circle_blur.rs"
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

# Macro form
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
