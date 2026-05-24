"""Apply doc-comments to trig.rs.

One-off script for the variations-bulk-metadata project. Idempotent.
All variations in this file are attributed to cothe."""
import textwrap

DOC = {
    "SIN": "Treats the input as a complex number and applies the sine function. Output is `sin(x)*cosh(y), cos(x)*sinh(y)` — horizontally periodic, growing vertically away from the real axis.",
    "COS": "Complex cosine. Same shape as Sin but shifted — output is `cos(x)*cosh(y), -sin(x)*sinh(y)`.",
    "TAN": "Complex tangent (sin/cos). Singularities at ±π/2 produce dramatic poles in the output.",
    "SEC": "Complex secant (1/cos). Singularities at ±π/2 create high-density rings around the poles.",
    "CSC": "Complex cosecant (1/sin). Singularities at 0 and ±π.",
    "COT": "Complex cotangent (cos/sin). Singularities at 0 and ±π.",
    "SINH": "Complex hyperbolic sine, applied to `z·π/4`. Stretches the plane vertically.",
    "COSH": "Complex hyperbolic cosine. Sister function to Sinh — even symmetry instead of odd.",
    "TANH": "Complex hyperbolic tangent, applied to `z·π/4`. Compresses extreme values toward ±1.",
    "COTH": "Complex hyperbolic cotangent. Singularities at 0.",
    "SECH": "Complex hyperbolic secant (note: ported with a JWildfire formula quirk that makes it equivalent to a sign-flipped csch — preserved so existing flames render the same).",
    "CSCH": "Complex hyperbolic cosecant. Singularities at 0.",
}

PATH = "src/variations/defs/trig.rs"
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
    lines.append("/// - cothe")
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  inserted {inserted}, already {already}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
