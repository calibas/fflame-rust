"""Apply doc-comments to quaternion.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
All variations in this file are attributed to zephyrtronium."""
import textwrap

DOC = {
    "SINQ": "Quaternion sine — extends the complex Sin to 3D by treating (x, y, z) as a split quaternion. In 2D mode (z = 0) it collapses to the same shape as Sin.",
    "COSQ": "Quaternion cosine — 3D extension of Cos. In 2D it collapses to the same shape as Cos.",
    "SINHQ": "Quaternion hyperbolic sine — 3D extension of Sinh.",
    "COSHQ": "Quaternion hyperbolic cosine — 3D extension of Cosh.",
    "SECQ": "Quaternion secant — 3D extension of Sec (1/cos).",
    "CSCQ": "Quaternion cosecant — 3D extension of Csc (1/sin).",
    "SECHQ": "Quaternion hyperbolic secant — 3D extension of Sech.",
    "CSCHQ": "Quaternion hyperbolic cosecant — 3D extension of Csch.",
    "TANQ": "Quaternion tangent — 3D extension of Tan (sin/cos).",
    "COTQ": "Quaternion cotangent — 3D extension of Cot (cos/sin).",
    "TANHQ": "Quaternion hyperbolic tangent — 3D extension of Tanh.",
    "COTHQ": "Quaternion hyperbolic cotangent — 3D extension of Coth.",
}

PATH = "src/variations/defs/quaternion.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

inserted = 0
already = 0
for name, body in DOC.items():
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
    lines.append("///")
    lines.append("/// # Authors")
    lines.append("/// - zephyrtronium")
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  inserted {inserted}, already {already}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
