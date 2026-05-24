"""Apply doc-comments + per-param descriptions to blur_extras.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "SINEBLUR": (
        "Radial sine-density blur — samples points uniformly on the unit disc with `acos`-based radial distribution. `power` controls how the density falls off.",
        ["Zy0rg"],
    ),
    "STARBLUR": (
        "N-pointed star-shape blur — samples points uniformly within a star polygon. Each arm is parameterized so the random distribution is area-uniform across the whole star.",
        ["Zy0rg"],
    ),
    "R_CIRCLEBLUR": (
        "Radial-truncated circle blur — truncates the input radius into bands of width `n` and places a randomly-positioned small disc within each band cell. Hash-based per-cell positioning.",
        ["Tatyana Zabanova"],
    ),
}

PARAM_DOC = {
    ("SINEBLUR", "power"): "Density power. 1.0 gives uniform-on-disc; higher values concentrate density near the edge.",

    ("STARBLUR", "power"): "Number of star points.",
    ("STARBLUR", "range"): "Inner-vertex radius — 0 gives a circle (no points), 1 gives sharp points.",

    ("R_CIRCLEBLUR", "n"): "Band width — radius is truncated modulo this value.",
    ("R_CIRCLEBLUR", "seed"): "Random seed for the per-cell hash — change to vary the pattern.",
    ("R_CIRCLEBLUR", "dist"): "Per-cell jitter strength.",
    ("R_CIRCLEBLUR", "min"): "Minimum cell-disc radius (fraction of cell).",
    ("R_CIRCLEBLUR", "max"): "Maximum cell-disc radius (fraction of cell).",
}

PATH = "src/variations/defs/blur_extras.rs"
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
