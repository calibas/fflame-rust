"""Apply doc-comments + per-param descriptions to stub_recoveries.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "BSPLIT": (
        "Tangent-split warp — outputs `(cos(y)/tan(x), -y/sin(x))` with user-adjustable X/Y shifts. Degenerate at `x = 0` and `x = π` (sin = 0); those points contribute zero.",
        ["Raykoid666", "ChronologicalDot"],
    ),
    "CYLINDER2": (
        "Lengthwise unit-cylinder warp — `(x / sqrt(x² + 1), y)`. Compresses X toward [-1, 1] without affecting Y.",
        None,
    ),
    "ECLIPSE": (
        "Eclipse-shaped X-axis clamp — inside an elliptical region sized by the variation's weight, X gets shifted by `shift · w`; outside the region, points pass through unchanged. Produces an eclipse silhouette.",
        ["Michael Faber"],
    ),
    "LOZI": (
        "Lozi strange attractor — `(c − a·|x| + y, b·x)` iteration. Like Hénon but with `|x|` instead of `x²`.",
        ["TyrantWave"],
    ),
    "PULSE": (
        "Sine-wave additive distortion — adds `scale · sin(coord · freq)` to each axis. Per-axis frequency and amplitude controls.",
        None,
    ),
    "HYPERSHIFT": (
        "Möbius-style shift + stretch — inverts the input through the unit circle, adds a horizontal shift, then re-inverts. The Y output gets stretched separately.",
        ["Zy0rg", "Brad Stefanov"],
    ),
}

PARAM_DOC = {
    ("BSPLIT", "x"): "X-axis shift added before the trig terms.",
    ("BSPLIT", "y"): "Y-axis shift added before the trig terms.",

    ("ECLIPSE", "shift"): "Horizontal shift applied to points inside the eclipse region, scaled by the variation's weight.",

    ("LOZI", "a"): "Coefficient on `|x|` — controls the fold strength.",
    ("LOZI", "b"): "Coefficient scaling X to the Y output.",
    ("LOZI", "c"): "Constant offset added to X output.",

    ("PULSE", "freqx"): "X-axis sine frequency.",
    ("PULSE", "freqy"): "Y-axis sine frequency.",
    ("PULSE", "scalex"): "X-axis sine amplitude.",
    ("PULSE", "scaley"): "Y-axis sine amplitude.",

    ("HYPERSHIFT", "shift"): "Horizontal shift applied after the first inversion. Also scales the overall transformation strength via `1 − shift²`.",
    ("HYPERSHIFT", "stretch"): "Y-axis stretching factor applied to the output.",
}

PATH = "src/variations/defs/stub_recoveries.rs"
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
