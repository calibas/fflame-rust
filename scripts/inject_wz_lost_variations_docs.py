"""Apply doc-comments + per-param descriptions to wz_lost_variations.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "Z_VARIATION": (
        "Pure radial boost using Michael Faber's *Lost Variations* four-shape blender. Computes a per-angle `total` by summing contributions from up to four named shapes — `hypergon` (regular polygon with adjustable inner radius), `star` (star polygon with slope-defined arms), `lituus` (polar spiral `(a/π + 1)^(-lituus_a)`), and `super` (Gielis super-shape) — each gated by its weight parameter (zero = disabled) and shaped by its own family of sub-parameters. Output is then `(r_in + total) · (cos a, sin a)`: the input point shoved outward along its own angle.",
        ["Michael Faber"],
    ),
    "W_VARIATION": (
        "Angle-rotated radial clip using the same four-shape blender as `z` (hypergon, star, lituus, super-shape). Computes `total` at the input angle `a`; if `|p| ≤ total` (the input is inside the shape), rotates the angle by `angle` to get `a2`, computes `total2` at `a2`, and emits a new point at radius `total2 · |p| / total` in the rotated direction `a2`. Points outside the shape pass through unchanged. Effectively shears the inside of the four-shape silhouette by a constant rotation.",
        ["Michael Faber"],
    ),
}

# Shared sub-parameter descriptions for both z and w.
SHARED = {
    "hypergon": "Weight of the hypergon (regular-polygon) shape's contribution to the per-angle radius blender. 0 disables this shape.",
    "hypergon_n": "Hypergon symmetry count (number of sides). Used as the modular reduction divisor for the per-sector angle lookup.",
    "hypergon_r": "Hypergon inner-radius parameter. Combined with the per-sector angle to decide whether the shape edge is convex or hollow at that angle.",
    "star": "Weight of the star-polygon shape's contribution. 0 disables this shape.",
    "star_n": "Star symmetry count (number of points).",
    "star_slope": "Star arm slope angle (radians). Internally pre-computed as `tan(star_slope)` and used as the slope of each star arm.",
    "lituus": "Weight of the lituus (polar spiral) shape's contribution. 0 disables this shape.",
    "lituus_a": "Lituus exponent. Internally negated; the contribution is `(a/π + 1)^(-lituus_a)` — a classical polar spiral.",
    "super": "Weight of the Gielis super-shape's contribution. 0 disables this shape.",
    "super_m": "Super-shape symmetry count (petal count). Internally divided by 4 to feed the Gielis sin/cos formula.",
    "super_n1": "Super-shape outer exponent. Internally `-1/(super_n1 + ε)` is used as the radial blow-up power — small values produce sharper points.",
    "super_n2": "Super-shape cosine-term exponent (controls one half of the petal asymmetry).",
    "super_n3": "Super-shape sine-term exponent.",
}

PARAM_DOC = {}
for k, v in SHARED.items():
    PARAM_DOC[("Z_VARIATION", k)] = v
    PARAM_DOC[("W_VARIATION", k)] = v
PARAM_DOC[("W_VARIATION", "angle")] = "Rotation angle (radians) applied to the input angle before the radial rebake. The output direction is `a + angle` (wrapped into `[-π, π]`)."


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


PATH = "src/variations/defs/wz_lost_variations.rs"
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
