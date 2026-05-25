"""Apply doc-comments + per-param descriptions to apo_misc7.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "ASTERIA": (
        "RNG-blended linear/asteria warp — splits behavior based on whether the input falls in a star-shaped region (inside the unit circle, with corners cut out by unit-discs around the four corners `(±1, ±1)`). Inside, linear pass-through dominates (with 35% chance of falling through to the asteria branch). Outside, applies a square-root-bend warp rotated by `alpha`.",
        ["DarkBeam"],
    ),
    "ESTIQ": (
        "Quaternion exponential extension — emits `(e^x · cos|v|, e^x · sin|v|/|v| · y, e^x · sin|v|/|v| · z)` where `|v| = sqrt(y² + z²)`. Generalizes the complex exponential `(x + iy → e^x · (cos y, sin y))` to a 3D quaternion-like form.",
        ["zephyrtronium"],
    ),
    "FDISC": (
        "Fractal disc — combines an inverse-radius cosine wave `cos(2π / (r + ashift) + xshift)`, an angular term `(atan2(y,x)/π + rshift) / 2`, and four blending weights. Each `term_i` controls a different contribution to the output (pure-prx, x-scaled-prx, x-scaled-pr, pass-through), producing rich layered disc patterns.",
        ["CozyG"],
    ),
    "BTRANSFORM": (
        "Bipolar transform — extends the bipolar-coords mapping `(τ, σ)` with adjustable `power`, `move`, `rotate`, and `split` (asymmetric τ offset based on the sign of x). Picks a random angular branch from `floor(rand · power)`, producing a generalized bipolar warp with N-fold rotational structure.",
        ["Michael Faber"],
    ),
    "NPOLAR": (
        "N-power polar warp with parity-based branch selection — even-parity (`|parity|` even) routes through a log-polar mid-step before the angular power; odd-parity stays in cartesian. A random `floor(rand · |n|)` selects one of `|n|` angular branches.",
        None,
    ),
}

PARAM_DOC = {
    ("ASTERIA", "alpha"): "Rotation angle (× π) applied before and inverted after the asteria branch's square-root bend.",

    ("FDISC", "ashift"): "Radial denominator offset added to `sqrt(x² + y²)`.",
    ("FDISC", "rshift"): "Angular offset added to `atan2(y, x) / π`.",
    ("FDISC", "xshift"): "Phase offset on the X cosine wave.",
    ("FDISC", "yshift"): "Phase offset on the Y sine wave.",
    ("FDISC", "term1"): "Weight on the pure `prx`/`pry` (radius × axis-wave) contribution.",
    ("FDISC", "term2"): "Weight on the input-scaled `x·prx`/`y·pry` contribution.",
    ("FDISC", "term3"): "Weight on the `x·pr`/`y·pr` (input-scaled radius) contribution.",
    ("FDISC", "term4"): "Weight on the pass-through `x`/`y` contribution.",

    ("BTRANSFORM", "rotate"): "Additive rotation on the bipolar σ coordinate.",
    ("BTRANSFORM", "power"): "Number of angular branches (≥ 1).",
    ("BTRANSFORM", "move"): "Additive offset on τ.",
    ("BTRANSFORM", "split"): "Asymmetric τ offset: added when x ≥ 0, subtracted when x < 0.",

    ("NPOLAR", "parity"): "Parity selector. `|parity| mod 2` chooses the branch (even = log-polar mid-step, odd = cartesian); the value itself also flips the sign of the even-branch's radial magnitude.",
    ("NPOLAR", "n"): "Angular branch count. 0 is forced to 1 internally; negative values use `|n|` branches.",
}

PATH = "src/variations/defs/apo_misc7.rs"
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


def find_macro_close(text: str, open_paren_idx: int) -> int:
    assert text[open_paren_idx] == "("
    depth = 1
    i = open_paren_idx + 1
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


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

    head = f'param!("{param_name}"'
    head_idx = block.find(head)
    if head_idx == -1:
        print(f"  WARN: no param {static_name}.{param_name}")
        continue
    open_idx = block.find("(", head_idx)
    close_idx = find_macro_close(block, open_idx)
    if close_idx == -1:
        print(f"  WARN: unbalanced param {static_name}.{param_name}")
        continue
    inner = block[open_idx + 1:close_idx]
    depth = 0
    in_str = False
    commas = 0
    j = 0
    while j < len(inner):
        ch = inner[j]
        if in_str:
            if ch == "\\":
                j += 2
                continue
            if ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
            elif ch == "," and depth == 0:
                commas += 1
        j += 1
    if commas >= 6:
        palready += 1
        continue

    new_block = (
        block[:close_idx]
        + f', "{desc}"'
        + block[close_idx:]
    )
    src = src[:start_idx] + new_block + src[end_idx:]
    pinserted += 1
print(f"  pass2: injected {pinserted}, already {palready}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
