"""Attribute every variation matching an Apophysis Plugin Pack source file
to "Apophysis Plugin Pack" — replacing any existing # Authors block.

Idempotent."""
import re

# Map: variation name (lowercase, matches `name: "X",`) -> file path
TARGETS = {
    "bent2": "src/variations/defs/misc_extras2.rs",
    "bipolar": "src/variations/defs/advanced.rs",
    "boarders": "src/variations/defs/boarders.rs",
    "butterfly": "src/variations/defs/shapes2.rs",
    "cell": "src/variations/defs/shapes2.rs",
    "circlize": "src/variations/defs/singleton_misc.rs",
    "cpow": "src/variations/defs/advanced.rs",
    "curve": "src/variations/defs/erf_misc.rs",
    "edisc": "src/variations/defs/erf_misc.rs",
    "elliptic": "src/variations/defs/advanced.rs",
    "escher": "src/variations/defs/advanced.rs",
    "foci": "src/variations/defs/advanced.rs",
    "lazysusan": "src/variations/defs/advanced.rs",
    "loonie": "src/variations/defs/advanced.rs",
    "modulus": "src/variations/defs/singleton_misc.rs",
    "ngon": "src/variations/defs/advanced.rs",
    "oscilloscope": "src/variations/defs/misc_extras2.rs",
    "pie": "src/variations/defs/shapes.rs",
    "polar2": "src/variations/defs/advanced.rs",
    "popcorn2": "src/variations/defs/numbered.rs",
    "scry": "src/variations/defs/advanced.rs",
    "separation": "src/variations/defs/extended.rs",
    "split": "src/variations/defs/misc_2d.rs",
    "splits": "src/variations/defs/advanced.rs",
    "stripes": "src/variations/defs/misc_2d.rs",
    "wedge": "src/variations/defs/extended.rs",
    "wedge_julia": "src/variations/defs/wedge_extended.rs",
    "wedge_sph": "src/variations/defs/wedge_extended.rs",
    "whorl": "src/variations/defs/misc_extras4.rs",
}

AUTHOR_LINE = "/// - Apophysis Plugin Pack"

# Group variations by file
by_file = {}
for var, path in TARGETS.items():
    by_file.setdefault(path, []).append(var)


def find_pub_static(src, var_name):
    """Return (idx, end_of_line_idx) of `pub static <UPPER>: VariationDef`
    matching the given variation name. The static identifier is usually the
    uppercase form of the `name:` field, but we locate it indirectly by
    finding `name: "<var>"` and then walking back to the `pub static` line."""
    pattern = re.compile(r'name:\s*"' + re.escape(var_name) + r'"\s*,')
    m = pattern.search(src)
    if not m:
        return None
    # Walk back to find `pub static`
    ps_idx = src.rfind("pub static ", 0, m.start())
    if ps_idx == -1:
        return None
    return ps_idx


def lines_before(src, idx):
    """Return (start_idx_of_run, list_of_lines) for the contiguous run of
    /// or // lines immediately preceding src[idx]. The lines list does not
    include the trailing newline of each line."""
    # idx is the start of the `pub static` line. Walk backward.
    start = idx
    while start > 0:
        # Find start of previous line
        prev_nl = src.rfind("\n", 0, start - 1)
        line_start = prev_nl + 1 if prev_nl != -1 else 0
        line = src[line_start:start - 1]  # strip the \n we're standing on
        # Actually let's redo this differently — slice src[:idx], split on \n
        break
    prefix = src[:idx]
    # Drop the final empty piece (everything after the last \n is what we want
    # before the static, which is empty since static starts at line start)
    parts = prefix.split("\n")
    # Last entry should be empty (since idx is start of a line)
    contiguous = []
    for line in reversed(parts[:-1]):
        stripped = line.strip()
        if stripped.startswith("///") or stripped.startswith("//"):
            contiguous.append(line)
        else:
            break
    contiguous.reverse()
    # Compute start index: end of (idx - len(joined) - 1 for the joining \n)
    if not contiguous:
        return idx, []
    block_len = sum(len(l) for l in contiguous) + len(contiguous)  # newlines
    return idx - block_len, contiguous


def apply_attribution(src, var_name):
    """Find the static for var_name, rewrite its // / /// preamble so the
    only # Authors entry is "Apophysis Plugin Pack".

    Three cases:
    - Existing /// block with # Authors: replace the # Authors bullet
      list with our single line, preserve everything else.
    - Existing /// block without # Authors: append a fresh
      ///\n/// # Authors\n/// - Apophysis Plugin Pack at the end.
    - No /// block (only // or nothing): insert a fresh /// # Authors
      block right before the pub static.
    """
    ps_idx = find_pub_static(src, var_name)
    if ps_idx is None:
        return src, "not-found"

    block_start, block_lines = lines_before(src, ps_idx)
    triple = [l for l in block_lines if l.strip().startswith("///")]

    if triple:
        # The /// block is a contiguous tail of `block_lines` (since
        # contiguous already requires comment lines). Find where /// starts
        # within block_lines.
        first_triple_idx = next(i for i, l in enumerate(block_lines)
                                 if l.strip().startswith("///"))
        triple_start_in_src = block_start + sum(
            len(l) for l in block_lines[:first_triple_idx]
        ) + first_triple_idx  # account for newlines

        # Locate # Authors heading within triple lines
        auth_idx = None
        for i, l in enumerate(triple):
            if l.strip() == "/// # Authors":
                auth_idx = i
                break

        if auth_idx is not None:
            # Replace from auth_idx + 1 through end of bullet list (consecutive
            # `/// - ...` lines).
            end_idx = auth_idx + 1
            while end_idx < len(triple) and triple[end_idx].strip().startswith("/// -"):
                end_idx += 1
            new_triple = triple[:auth_idx + 1] + [AUTHOR_LINE] + triple[end_idx:]
        else:
            # Append `///\n/// # Authors\n/// - Apophysis Plugin Pack`
            new_triple = triple + ["///", "/// # Authors", AUTHOR_LINE]

        # Stitch back together. Replace the existing triple-block in src.
        triple_end_in_src = ps_idx
        new_text = "\n".join(new_triple) + "\n"
        src = src[:triple_start_in_src] + new_text + src[triple_end_in_src:]
        return src, "rewrote"
    else:
        # No /// block. Insert a fresh one right before pub static.
        new_text = "/// # Authors\n" + AUTHOR_LINE + "\n"
        src = src[:ps_idx] + new_text + src[ps_idx:]
        return src, "inserted"


total = {"rewrote": 0, "inserted": 0, "not-found": 0}
for path, vars_in_file in by_file.items():
    with open(path, "rb") as f:
        src = f.read().decode("utf-8")
    for var in vars_in_file:
        src, status = apply_attribution(src, var)
        total[status] += 1
        if status == "not-found":
            print(f"  WARN: {var} not found in {path}")
    with open(path, "wb") as f:
        f.write(src.encode("utf-8"))
    print(f"  {path}: processed {len(vars_in_file)} variations")

print(f"\nTotals: rewrote={total['rewrote']}, inserted={total['inserted']}, not-found={total['not-found']}")
