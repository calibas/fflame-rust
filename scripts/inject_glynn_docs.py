"""Apply doc-comments + per-param descriptions to glynn.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "GLYNNIA": (
        "Random-branched Glynn-set warp — splits into 4 branches per iteration based on radius and a random coin flip. Produces the characteristic organic Glynn fractal shapes.",
        ["Michael Faber"],
    ),
    "GLYNNIA3": (
        "Glynnia with 4 user-tunable knobs — exposes the radius/distance scaling and the branch threshold as parameters.",
        ["Michael Faber", "Maulana Randa", "CozyG"],
    ),
    "GLYNN_SIM1": (
        "Glynn-set inversion with an offset-circle generator — points inside the radius spawn new points on a circle at `(x1, y1)`; outside, points get inverted with probabilistic threshold.",
        ["eralex61"],
    ),
    "GLYNN_SIM2": (
        "Glynn-set with arc-segment generator — like GlynnSim1 but the generator spawns points along an arc instead of a full circle. `phi1` / `phi2` define the arc bounds.",
        ["eralex61"],
    ),
    "GLYNN_SIM3": (
        "Glynn-set with two-radius branching — generator picks between two circles (`r1` and `r2 = radius²/r1`) with probability γ.",
        ["eralex61"],
    ),
}

PARAM_DOC = {
    ("GLYNNIA3", "rscale"): "Radial scaling on the input distance.",
    ("GLYNNIA3", "dscale"): "Scaling on the `d = r + x` term used by both branches.",
    ("GLYNNIA3", "rthresh"): "Radius threshold for the inside-vs-outside branch split.",
    ("GLYNNIA3", "ythresh"): "Y threshold added to the branch condition — only points with `y > ythresh` take the outer branch.",

    ("GLYNN_SIM1", "radius"): "Main Glynn-set inversion radius.",
    ("GLYNN_SIM1", "radius1"): "Offset-circle radius for the generator.",
    ("GLYNN_SIM1", "phi1"): "Angular position of the offset circle (degrees).",
    ("GLYNN_SIM1", "thickness"): "Circle thickness — fraction of the radius (0 = points on the boundary, 1 = filled disc).",
    ("GLYNN_SIM1", "pow"): "Power for the contrast probability — higher concentrates points near the boundary.",
    ("GLYNN_SIM1", "contrast"): "Probability scaling for the inversion-vs-pass-through branch.",

    ("GLYNN_SIM2", "radius"): "Main Glynn-set inversion radius.",
    ("GLYNN_SIM2", "thickness"): "Arc thickness — fraction of the radius.",
    ("GLYNN_SIM2", "contrast"): "Probability scaling for the inversion-vs-pass-through branch.",
    ("GLYNN_SIM2", "pow"): "Power for the contrast probability.",
    ("GLYNN_SIM2", "phi1"): "Start angle of the arc segment (degrees).",
    ("GLYNN_SIM2", "phi2"): "End angle of the arc segment (degrees).",

    ("GLYNN_SIM3", "radius"): "Main Glynn-set inversion radius.",
    ("GLYNN_SIM3", "thickness"): "Outer-circle thickness — combined with radius to form `r1` and `r2 = radius² / r1`.",
    ("GLYNN_SIM3", "contrast"): "Probability scaling for the inversion-vs-pass-through branch.",
    ("GLYNN_SIM3", "pow"): "Power for the contrast probability.",
}

PATH = "src/variations/defs/glynn.rs"
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
