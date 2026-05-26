"""Apply doc-comments + per-param descriptions to parametric_curves.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "SPIROGRAPH": (
        "Hypotrochoid curve sampler — picks a random parameter `t` and emits a point on the classic spirograph curve. `a`/`b` are the outer/inner wheel radii; `c1`/`c2` give per-axis amplitudes; `d` is the pen-arm length.",
        None,
    ),
    "LISSAJOUS": (
        "Lissajous-figure sampler — picks a random `t` and emits `(sin(a·t + d), sin(b·t))`. Adds linear drift `c·t` and noise `e·y` for thicker curves.",
        ["Jed Kelsey"],
    ),
    "VOGEL": (
        "Vogel-spiral sampler — places points at integer-indexed angles `i · 2π/φ²` from a golden-angle spiral. Produces the iconic sunflower-seed pattern.",
        ["Victor Ganora"],
    ),
    "CROP3D": (
        "Axis-aligned 3D crop with optional jitter — constrains points to a 3D box. Outside the box, points either collapse to zero (`zero` on) or scatter back toward the nearest edge with `scatter_area` randomness.",
        ["Xyrus02"],
    ),
}

PARAM_DOC = {
    ("SPIROGRAPH", "a"): "Outer-wheel radius.",
    ("SPIROGRAPH", "b"): "Inner-wheel radius.",
    ("SPIROGRAPH", "d"): "Pen-arm length added to the output.",
    ("SPIROGRAPH", "c1"): "X-axis amplitude of the inner-wheel cosine term.",
    ("SPIROGRAPH", "c2"): "Y-axis amplitude of the inner-wheel sine term.",
    ("SPIROGRAPH", "tmin"): "Minimum value of the random parameter `t`.",
    ("SPIROGRAPH", "tmax"): "Maximum value of the random parameter `t`.",
    ("SPIROGRAPH", "ymin"): "Minimum random Y offset added to both output coordinates.",
    ("SPIROGRAPH", "ymax"): "Maximum random Y offset added to both output coordinates.",

    ("LISSAJOUS", "tmin"): "Minimum value of the random parameter `t`.",
    ("LISSAJOUS", "tmax"): "Maximum value of the random parameter `t`.",
    ("LISSAJOUS", "a"): "X-axis frequency multiplier.",
    ("LISSAJOUS", "b"): "Y-axis frequency multiplier.",
    ("LISSAJOUS", "c"): "Linear drift added to both axes.",
    ("LISSAJOUS", "d"): "Phase offset on the X axis.",
    ("LISSAJOUS", "e"): "Noise scale added to both axes.",

    ("VOGEL", "n"): "Number of seed positions (1-1000). Larger = more points around the spiral.",
    ("VOGEL", "scale"): "Mixes the radial distance with the input coordinates. 0 = pure spiral; higher = blended with input.",

    ("CROP3D", "left"): "Left bound of the cropping box (X axis).",
    ("CROP3D", "top"): "Top bound (Y axis).",
    ("CROP3D", "floor"): "Floor bound (Z axis).",
    ("CROP3D", "right"): "Right bound (X axis).",
    ("CROP3D", "bottom"): "Bottom bound (Y axis).",
    ("CROP3D", "ceiling"): "Ceiling bound (Z axis).",
    ("CROP3D", "scatter_area"): "Width of the scatter band along each box edge. 0 = snap to edge; 1 = scatter across the full box width.",
    ("CROP3D", "zero"): "When on, out-of-box points collapse to origin. When off, they scatter back toward the nearest edge.",
}

PATH = "src/variations/defs/parametric_curves.rs"
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
