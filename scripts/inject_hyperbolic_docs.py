"""Apply doc-comments to hyperbolic.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "ACOTH": "Treats the input as a complex number and applies the inverse hyperbolic cotangent, then swaps the real and imaginary parts. Creates two singularity points at (±1, 0) with the pattern flowing between them.",
    "ACOSH": "Inverse hyperbolic cosine on the complex input. Each iteration randomly picks one of the two branches, producing a symmetric upper/lower pattern.",
    "ACOSECH": "Inverse hyperbolic cosecant on the complex input (arcsinh of 1/z), then swaps the real and imaginary parts. Random branch selection per iteration produces symmetric two-branch patterns.",
    "ARCSECH": "Inverse hyperbolic secant on the complex input (arccosh of 1/z). Singular at the origin and outflows along the real axis.",
    "ARCSECH2": "Variant of ArcSecH with translation by ±i depending on the imaginary sign — produces two parallel arcs instead of one.",
    "ARCSINH": "Inverse hyperbolic sine on the complex input. Maps the entire plane onto a horizontal strip — acts as a `spreading` transform.",
    "ARCTANH": "Inverse hyperbolic tangent on the complex input. Maps the unit disc onto the entire plane; everything inside |z|=1 expands outward, everything outside compresses inward.",
}

PATH = "src/variations/defs/hyperbolic.rs"
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
    doc = "\n".join(lines)
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  inserted {inserted}, already {already}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
