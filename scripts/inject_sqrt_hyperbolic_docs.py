"""Apply doc-comments to sqrt_hyperbolic.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import textwrap

DOC = {
    "SQRT_ACOTH": "Like ACoth but applied to sqrt(z) instead of z. The square-root pre-step roughly halves the angle, producing a denser, more intricate version of ACoth's two-singularity pattern. Random ±sign each iteration.",
    "SQRT_ACOSH": "Like ACosh but on sqrt(z). Random ±sign each iteration.",
    "SQRT_ACOSECH": "Like ACosecH but on sqrt(z) — denser version of the symmetric two-branch pattern. Random ±sign each iteration.",
    "SQRT_ASECH": "Variant of ArcSecH applied to sqrt(z). Note: upstream ports a copy-paste bug that makes this equivalent to sqrt_acosh — preserved so JWildfire flames render the same.",
    "SQRT_ASINH": "Like ArcSinh but on sqrt(z). Random ±sign each iteration.",
    "SQRT_ATANH": "Like ArcTanh but on sqrt(z) — a denser, more compressed variant of the unit-disc-to-plane mapping. Random ±sign each iteration.",
}

PATH = "src/variations/defs/sqrt_hyperbolic.rs"
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
