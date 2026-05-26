"""Apply doc-comments + per-param descriptions to classic_2d.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "FAN": (
        "Affine-driven fan sweep — wraps points into pie wedges sized by the affine's X-translation field (`e`). The wedge angle and rotation are read directly from the transform's affine matrix.",
        ["Scott Draves"],
    ),
    "FISHEYE": (
        "Classic fisheye distortion — `2r/(r+1)` radial scaling that pulls distant points inward. Note: preserves a long-standing Apophysis X/Y swap bug; use Eyefish for the corrected form.",
        ["Scott Draves"],
    ),
    "GRIDOUT": (
        "Snaps points onto a discrete grid by following the nearest cell-edge direction. Produces a chunky, tile-like output.",
        ["Michael Faber", "DarkBeam"],
    ),
    "CIRCULAR": (
        "Randomized rotation by a deterministic-plus-RNG term — each iteration adds a small random angle to the point's polar angle. The `seed` parameter changes the noise pattern.",
        ["Tatyana Zabanova"],
    ),
    "PANORAMA1": (
        "Spherical-style panoramic projection — maps the plane onto a hemispherical fisheye then unwraps it into longitude/latitude coordinates.",
        ["Tatyana Zabanova", "DarkBeam"],
    ),
    "PANORAMA2": (
        "Variant of Panorama 1 using `1/(r+1)` instead of `1/sqrt(r²+1)` as the radial denominator — same overall shape with subtly different distortion.",
        ["Tatyana Zabanova", "DarkBeam"],
    ),
}

PARAM_DOC = {
    ("CIRCULAR", "angle"): "Maximum rotation per iteration (degrees).",
    ("CIRCULAR", "seed"): "Random seed for the noise term — change to vary the pattern.",
}

PATH = "src/variations/defs/classic_2d.rs"
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

# CIRCULAR uses the macro form (param!(...))
pinserted = 0
palready = 0
for (static_name, param_name), desc in PARAM_DOC.items():
    start_pattern = f"pub static {static_name}: VariationDef"
    start_idx = src.find(start_pattern)
    if start_idx == -1:
        print(f"  WARN: no static {static_name}")
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
