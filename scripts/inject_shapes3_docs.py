"""Apply doc-comments + per-param descriptions to shapes3.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "SUPER_SHAPE": (
        "Warps points via the Gielis super-formula — a generalized rose/star curve controlled by 5 shape parameters. Produces flower-like patterns.",
        None,
    ),
    "HENON": (
        "Classic Hénon strange attractor — iteration of `(c − a·x² + y, b·x)`. Maps trajectories onto the famous Hénon curve.",
        ["TyrantWave"],
    ),
    "APOLLONY": (
        "Apollonian-gasket IFS — randomly picks one of 3 Möbius-style branches per iteration to spawn points along an Apollonian gasket fractal.",
        ["Jesus Sosa", "Paul Bourke"],
    ),
}

PARAM_DOC = {
    ("SUPER_SHAPE", "rnd"): "Mix between random sampling and the input radius. 0 = pure input radius, 1 = uniform random.",
    ("SUPER_SHAPE", "m"): "Number of symmetry petals — sets how many lobes the super-shape has.",
    ("SUPER_SHAPE", "n1"): "Outer exponent — controls overall puffiness or sharpness.",
    ("SUPER_SHAPE", "n2"): "First inner exponent — controls one half of each lobe shape.",
    ("SUPER_SHAPE", "n3"): "Second inner exponent — controls the other half of each lobe shape.",
    ("SUPER_SHAPE", "holes"): "Radial offset that punches a hole in the center.",

    ("HENON", "a"): "Quadratic coefficient — controls the strength of the parabolic fold.",
    ("HENON", "b"): "Linear coefficient on X — scales the Y output.",
    ("HENON", "c"): "Constant offset added to X output.",
}

PATH = "src/variations/defs/shapes3.rs"
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
