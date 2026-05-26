"""Apply doc-comments + per-param descriptions to standalone_exotics.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "KALEIDOSCOPE": (
        "Half-plane mirror warp — rotates and reflects the input across the X axis with separate behavior above and below the line. Produces the classic kaleidoscope mirror pattern.",
        ["Will Evans"],
    ),
    "TAURUS": (
        "Torus-section mapping — wraps the input through a torus-shaped surface modulated by `n`, creating 3D donut-like structures.",
        ["gossamer_light"],
    ),
    "HOLE2": (
        "10-shape radial hole — applies one of 10 user-selectable radial shape formulas to create a hole pattern. `shape` picks which formula; `inside` toggles whether the hole inverts (`w/r`) or scales (`w·r`).",
        ["Michael Faber", "Brad Stefanov", "Rick Sidwell"],
    ),
}

PARAM_DOC = {
    ("KALEIDOSCOPE", "pull"): "Y-axis pull strength — pulls the upper and lower halves apart.",
    ("KALEIDOSCOPE", "rotate"): "Rotation scaling on both axes.",
    ("KALEIDOSCOPE", "line_up"): "Linear offset along the mirror line.",
    ("KALEIDOSCOPE", "x"): "Additional X offset.",
    ("KALEIDOSCOPE", "y"): "Additional Y offset for the upper half.",

    ("TAURUS", "r"): "Torus radius.",
    ("TAURUS", "n"): "Number of cosine-modulation cycles around the torus.",
    ("TAURUS", "inv"): "Blend between fixed and modulated radius. 0 = fully modulated; 1 = fixed.",
    ("TAURUS", "sor"): "Spherical-coordinate blend factor for the Z output.",

    ("HOLE2", "a"): "Power for the angle-derived scaling factor.",
    ("HOLE2", "b"): "Angular wave frequency — used by shapes 2, 6, 7, 8, 9 to vary their angular modulation.",
    ("HOLE2", "c"): "Multiplier on the angle-derived scaling factor.",
    ("HOLE2", "d"): "Angle multiplier on the input.",
    ("HOLE2", "inside"): "When on, inverts the radial formula (`w/r`) instead of scaling (`w·r`).",
    ("HOLE2", "shape"): "Picks one of 10 radial formulas (0-9).",
}

PATH = "src/variations/defs/standalone_exotics.rs"
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
