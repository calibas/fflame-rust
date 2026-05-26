"""Apply doc-comments + per-param descriptions to supershape3d_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "SUPERSHAPE_3D": (
        "Two-axis Gielis super-shape projected onto a 3D surface — each iteration picks two random angles `ρ` and `φ` (uniform in `[0, rho]` and `[-phi, phi]`) and evaluates the Gielis super-shape formula `r(θ) = (|cos(m·θ/4)/a|^n2 + |sin(m·θ/4)/b|^n3)^(-1/n1)` independently for each angle (subscript 1 for the ρ axis, 2 for the φ axis). The two radii `r1` and `r2` then combine into either a spherical product (`r1·r2·cos·cos`, etc.) or a toroidal sum (`cos·(r1 + r2·cos)`, etc.) based on `toroidmap`. Optional `spiral` term advances the ρ-axis radius linearly with the ρ angle to produce a spiraling output.",
        ["David Young"],
    ),
}

PARAM_DOC = {
    ("SUPERSHAPE_3D", "rho"): "Angular range (radians) for uniform random sampling of the ρ axis. Larger values sweep through more of the shape per iteration; default 9.9 (~3π).",
    ("SUPERSHAPE_3D", "phi"): "Angular range (radians) for the φ axis (sampled in `[-phi, phi]`). Default 2.5 (~0.8π).",
    ("SUPERSHAPE_3D", "m1"): "Petal count for the ρ-axis Gielis shape — controls rotational symmetry. Higher values produce more spokes.",
    ("SUPERSHAPE_3D", "m2"): "Petal count for the φ-axis Gielis shape.",
    ("SUPERSHAPE_3D", "a1"): "Cosine-term width scaling for the ρ-axis shape. Smaller values pinch the cos-side petals narrower.",
    ("SUPERSHAPE_3D", "a2"): "Cosine-term width scaling for the φ-axis shape.",
    ("SUPERSHAPE_3D", "b1"): "Sine-term width scaling for the ρ-axis shape.",
    ("SUPERSHAPE_3D", "b2"): "Sine-term width scaling for the φ-axis shape.",
    ("SUPERSHAPE_3D", "n1_1"): "Outer exponent for the ρ-axis shape: final radius is `pr^(-1/n1_1)`. Controls overall radial blow-up — small values produce sharp spikes, larger values rounder shapes.",
    ("SUPERSHAPE_3D", "n1_2"): "Outer exponent for the φ-axis shape.",
    ("SUPERSHAPE_3D", "n2_1"): "Cosine-term exponent for the ρ-axis shape — controls petal sharpness on the cos side.",
    ("SUPERSHAPE_3D", "n2_2"): "Cosine-term exponent for the φ-axis shape.",
    ("SUPERSHAPE_3D", "n3_1"): "Sine-term exponent for the ρ-axis shape — controls petal sharpness on the sin side.",
    ("SUPERSHAPE_3D", "n3_2"): "Sine-term exponent for the φ-axis shape.",
    ("SUPERSHAPE_3D", "spiral"): "Spiral phase advance: `r1` is incremented by `spiral · ρ`, producing a linear radial growth with angle. 0 disables.",
    ("SUPERSHAPE_3D", "toroidmap"): "0 = spherical projection (output = `r1·r2·cos·cos`, etc.). 1 = toroidal projection (output = `cos·(r1 + r2·cos)`, etc.). Both modes use `r2·sin(φ)` for Z.",
}


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


PATH = "src/variations/defs/supershape3d_misc.rs"
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
