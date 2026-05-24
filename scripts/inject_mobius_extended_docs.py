"""Apply doc-comments + per-param descriptions to mobius_extended.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "MOBIUSN": (
        "N-power Möbius — transforms into `z^power` space, applies a 2×2 complex Möbius transformation `(Az + B)/(Cz + D)`, then transforms back via a random branch of the N-th root.",
        ["eralex61"],
    ),
    "MOBIQ": (
        "Quaternion Möbius — same `(Az + B)/(Cz + D)` form as Möbius but with quaternion-valued A, B, C, D and input. Treats `(x, y, z)` as a quaternion with no k-component, producing true 3D output.",
        ["zephyrtronium"],
    ),
}

# Build PARAM_DOC.
PARAM_DOC = {}

# MOBIUSN — 10 params
for letter, role in [("a", "A (numerator multiplier)"), ("b", "B (numerator offset)"),
                      ("c", "C (denominator multiplier)"), ("d", "D (denominator offset)")]:
    PARAM_DOC[("MOBIUSN", f"re_{letter}")] = f"Real part of complex coefficient {role}."
    PARAM_DOC[("MOBIUSN", f"im_{letter}")] = f"Imaginary part of complex coefficient {role}."
PARAM_DOC[("MOBIUSN", "power")] = "Exponent for the `z^power` transform that wraps the Möbius operation. Higher values create more arms in the output."
PARAM_DOC[("MOBIUSN", "dist")] = "Scales the radial component of the wrapping transform."

# MOBIQ — 16 params (4 quaternions × 4 components)
for letter, role in [("a", "A (numerator multiplier)"), ("b", "B (numerator offset)"),
                      ("c", "C (denominator multiplier)"), ("d", "D (denominator offset)")]:
    for comp, comp_name in [("t", "T (scalar)"), ("x", "X (i)"), ("y", "Y (j)"), ("z", "Z (k)")]:
        PARAM_DOC[("MOBIQ", f"q{letter}{comp}")] = f"{comp_name} component of quaternion {role}."

PATH = "src/variations/defs/mobius_extended.rs"
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

# Both use macro form
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
