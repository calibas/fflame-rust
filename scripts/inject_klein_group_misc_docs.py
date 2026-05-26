"""Apply doc-comments + per-param descriptions to klein_group_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "KLEIN_GROUP": (
        "Kleinian limit-set chaos game from Mumford, Series & Wright's *Indra's Pearls: The Vision of Felix Klein* (2002). Each iteration picks one of the four group generators `{a, b, A=a⁻¹, B=b⁻¹}` at random, applies the corresponding Möbius transformation `(α·z + β)/(γ·z + δ)`, and plots the result. `avoid_reversal` skips immediately-cancelling pairs (`aA, bB, Aa, Bb`) so the trajectory drifts through the limit set instead of bouncing back. Seven recipes are available for constructing the generator pair from the complex parameters `(a_re + i·a_im)` and `(b_re + i·b_im)`: Grandma's standard parabolic-commutator construction (0), Maskit's μ slice (1), Jørgensen's trace-based parameterization (2), Riley's c parameter (3), Riley-modified with extra `b` (4), Maskit-modified with extra `b` (5), and Jos Leys' n-fold Maskit variant (6). Inverses are computed on-the-fly via the `SL(2,ℂ)` shortcut `[d, -b, -c, a]` since all generator matrices have determinant 1 by construction.",
        ["CozyG"],
    ),
}

PARAM_DOC = {
    ("KLEIN_GROUP", "a_re"): "Real part of the first complex parameter. Interpretation depends on `recipe`: trace of generator A (recipes 0 GRANDMA, 2 JORGENSEN), Maskit's μ parameter (1, 5, 6), or Riley's c parameter (3, 4).",
    ("KLEIN_GROUP", "a_im"): "Imaginary part of the first complex parameter. See `a_re` for recipe-dependent role.",
    ("KLEIN_GROUP", "b_re"): "Real part of the second complex parameter. Interpretation depends on `recipe`: trace of generator B (0, 2), `b1` translation coefficient (4, 5), the `n` in Leys' `2·cos(π/n)` formula (6). Unused in recipes 1 (MASKIT_MU) and 3 (RILEY) — those have a hardcoded `b = 2`.",
    ("KLEIN_GROUP", "b_im"): "Imaginary part of the second complex parameter. See `b_re` for recipe-dependent role.",
    ("KLEIN_GROUP", "recipe"): "Generator-pair construction recipe (0-6): 0 = GRANDMA_STANDARD (parabolic commutator from *Indra's Pearls* Ch. 6), 1 = MASKIT_MU (single complex μ — the Maskit slice), 2 = JORGENSEN (Troels Jørgensen's trace-based parameterization), 3 = RILEY (Robert Riley's single complex c), 4 = RILEY_MODIFIED (Riley with extra b1), 5 = MASKIT_MU_MODIFIED (Maskit with extra b1), 6 = MASKIT_LEYS_MODIFIED (Jos Leys' n-fold variant).",
    ("KLEIN_GROUP", "avoid_reversal"): "When 1, the next-matrix pick excludes the inverse of the previous matrix (no `aA`, `Aa`, `bB`, or `Bb` cancellations), so the trajectory drifts through the limit set instead of bouncing back-and-forth on a small subset. When 0, all four matrices are equally likely each iteration.",
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


PATH = "src/variations/defs/klein_group_misc.rs"
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
