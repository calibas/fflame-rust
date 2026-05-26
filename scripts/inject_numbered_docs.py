"""Apply doc-comments + per-param descriptions to numbered.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
POPCORN2 handled manually (existing # Authors block needed description
prepending)."""
import re
import textwrap

DOC = {
    "SPHERICAL3D": (
        "3D version of Spherical — inverts each point through the unit sphere (`1/r²`). Pulls distant points toward the origin and pushes nearby points outward in all three axes.",
        None,
    ),
    "SINUSOIDAL3D": (
        "3D sinusoidal — applies `sin` to X and Y like Sinusoidal, then adds `atan2(x², y²) · cos(z)` on the Z axis.",
        ["gossamer_light"],
    ),
    "SQUARE": (
        "Random 2D unit-square sampler — replaces the input with a uniformly random point in `[-0.5, 0.5]²`.",
        None,
    ),
    "SQUARE3D": (
        "3D unit-cube version of Square — random point in `[-0.5, 0.5]³`.",
        None,
    ),
    "DISC3D": (
        "3D version of Disc with a tweakable `pi` constant. Wraps the (x, y) plane onto a disc and adds a `r·cos(z)` Z component.",
        None,
    ),
    "BUBBLE2": (
        "Parameterized 3D bubble with separate X / Y / Z scaling. Maps the input onto a sphere and stretches each axis independently.",
        ["FracFx"],
    ),
    "SPLITS3D": (
        "3D version of Splits — pushes each coordinate away from zero by a fixed per-axis offset, creating a gap along each axis.",
        ["TyrantWave"],
    ),
    "WAVES2_3D": (
        "3D version of Waves2 — adds `scale · sin(freq · avg(x, y))` to the Z coordinate alongside the standard 2D Waves2 X/Y displacement.",
        None,
    ),
    "JULIAQ": (
        "Rational-power Julia — like JuliaN but with separate `power` and `divisor`, allowing fractional/rational branch counts (e.g. 3/2 gives 1.5 branches).",
        ["Zueuk"],
    ),
    "JULIA3DQ": (
        "3D version of JuliaQ — extends the rational-power Julia into the Z axis.",
        None,
    ),
    "JULIAC": (
        "Complex-power Julia — `power = re + i·im`. Like CPow but with a `dist` parameter that scales the log-of-radius term separately.",
        ["David Young"],
    ),
}

PARAM_DOC = {
    ("DISC3D", "pi"): "Phase constant — defaults to π. Tweaking it warps the disc shape.",

    ("BUBBLE2", "x"): "X-axis scaling of the sphere projection.",
    ("BUBBLE2", "y"): "Y-axis scaling.",
    ("BUBBLE2", "z"): "Z-axis displacement plus scaling.",

    ("POPCORN2", "x"): "X-axis displacement strength.",
    ("POPCORN2", "y"): "Y-axis displacement strength.",
    ("POPCORN2", "c"): "Frequency of the tan-sine wave that drives the displacement on both axes.",

    ("SPLITS3D", "x"): "How far positive-X and negative-X points get pushed apart along X.",
    ("SPLITS3D", "y"): "How far positive-Y and negative-Y points get pushed apart along Y.",
    ("SPLITS3D", "z"): "How far positive-Z and negative-Z points get pushed apart along Z.",

    ("WAVES2_3D", "freq"): "Wave frequency on all three axes.",
    ("WAVES2_3D", "scale"): "Wave amplitude — how strongly points get displaced.",

    ("JULIAQ", "power"): "Number of Julia branches in the rational power.",
    ("JULIAQ", "divisor"): "Rational-power divisor. Combined with `power` lets you pick non-integer branch counts (e.g. power=3, divisor=2 → 1.5 branches).",

    ("JULIA3DQ", "power"): "Number of Julia branches.",
    ("JULIA3DQ", "divisor"): "Rational-power divisor.",

    ("JULIAC", "re"): "Real part of the complex power.",
    ("JULIAC", "im"): "Imaginary part of the complex power.",
    ("JULIAC", "dist"): "Distance scaling on the log-of-radius term — affects how rapidly the spiral grows outward.",
}

PATH = "src/variations/defs/numbered.rs"
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
