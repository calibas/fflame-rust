"""Apply doc-comments + per-param descriptions to shapes2.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
BUTTERFLY and CELL handled manually (they already had # Authors blocks
that needed description prepending)."""
import re
import textwrap

DOC = {
    "BUTTERFLY3D": (
        "3D version of Butterfly — same XY butterfly curve plus a Z component that scales with the radial distance times `|2y|`.",
        ["Don Town"],
    ),
    "ENNEPERS": (
        "Enneper's-surface parametric mapping — `(x(1 − x²/3 + y²), y(1 − y²/3 + x²))`. Inspired by Alfred Enneper's classical minimal surface.",
        None,
    ),
    "PYRAMID": (
        "3D pyramid using cubic-distance norm — each coordinate is cubed and divided by the sum of absolute cubes. Produces a pyramid-shaped silhouette.",
        ["Zueuk"],
    ),
    "RAYS2": (
        "Cosine-of-tangent rays — uses `1/cos((t+ε)·tan(1/t+ε))` on the squared radius `t`. Creates intricate ray patterns radiating from the origin.",
        ["Raykoid666"],
    ),
    "RAYS3": (
        "Variant of Rays2 with `sqrt(cos(sin(...)·sin(...)))` and tangent on Y. Denser ray pattern with sharper structure.",
        ["Raykoid666"],
    ),
    "SPIRALWING": (
        "Spiral wing — uses cos/sin of `x²` with `sin(y²)` modulation. Produces wing-shaped spiral patterns.",
        ["Raykoid666"],
    ),
    "WHITNEY_UMBRELLA": (
        "Parametric Whitney umbrella surface — output is `(xy, x, y²)`. The classical algebraic surface with the same name.",
        None,
    ),
    "CHRYSANTHEMUM": (
        "Chrysanthemum curve — Sosa's flower-like parametric curve. Plots `r` as a function of a random angle, producing dense overlapping petal patterns.",
        ["Sosa"],
    ),
    "ENNEPERS2": (
        "Parameterized Enneper variant — 3-parameter 3D extension of Ennepers with separate `a`/`b`/`c` controls.",
        ["DarkBeam"],
    ),
    "FLOWER": (
        "Flower — produces petal patterns based on a uniform-random distance scaled by `cos(petals · angle)`. The `holes` parameter controls how hollow the center is.",
        ["cyberxaos"],
    ),
}

PARAM_DOC = {
    ("CELL", "size"): "Width of each cell in the grid.",
    ("ENNEPERS2", "a"): "Coefficient on the X factor.",
    ("ENNEPERS2", "b"): "Coefficient on the Y factor.",
    ("ENNEPERS2", "c"): "Square-root correction strength applied to both X and Y outputs.",
    ("FLOWER", "holes"): "How hollow the center of the flower is. Higher = bigger center hole.",
    ("FLOWER", "petals"): "Number of petals around the flower.",
}

PATH = "src/variations/defs/shapes2.rs"
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
        print(f"  WARN: no static {static_name}")
        continue
    next_static = src.find("\npub static ", start_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)
    block = src[start_idx:end_idx]
    pdef_pat = re.compile(
        r'(VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?)description:\s*None\s*\}',
        re.DOTALL,
    )
    new_block, n = pdef_pat.subn(
        lambda m: m.group(1) + f'description: Some("{desc}") }}',
        block, count=1,
    )
    if n == 0:
        already_pat = re.compile(
            r'VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?description:\s*Some\(',
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
