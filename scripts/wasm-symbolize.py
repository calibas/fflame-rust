"""Name the function containing a wasm crash offset.

A browser reports a trap as `fractal_flame_wgpu_bg.wasm:3534022` -- a
byte offset into the module, with no name, because the shipped `dist`
profile strips the name section.

This maps that offset to a function name WITHOUT needing the crash to
survive instrumentation, which matters: this bug vanishes under every
instrumented build (dist-debug changes codegen by 50.6%; dist-symbols
with DWARF is 12x the size and closes the race window). The offset
from the shipped build is all that is needed.

USAGE, and the pairing rule is not optional:

  1. Build BOTH profiles from the SAME commit, with no `src/` change
     between them:
       cargo build --lib --target wasm32-unknown-unknown --profile dist
       cargo build --lib --target wasm32-unknown-unknown --profile dist-symbols
  2. Reproduce with the `dist` bundle and copy the offset.
  3. python scripts/wasm-symbolize.py <offset> [<offset> ...]

The two profiles differ only by `strip`, which leaves the code section
within ~740 bytes (0.007%) and shifts its start by ~19 -- close enough
that an offset lands in the right function body. Mismatch the commits
and it lands in the WRONG one while looking perfectly plausible: this
script produced three confident, meaningless names that way before the
pairing was checked. It prints the code-section geometry of both
builds first so a bad pairing is visible -- a size delta of more than
a few KB means rebuild, do not read the answer.
"""

import io, sys

def leb(b, o, signed=False):
    r = 0; s = 0
    while True:
        x = b[o]; o += 1
        r |= (x & 0x7f) << s; s += 7
        if not x & 0x80:
            if signed and s < 64 and x & 0x40: r |= -(1 << s)
            return r, o

def sections(d):
    o = 8; out = []
    while o < len(d):
        sid = d[o]; o += 1
        size, o2 = leb(d, o)
        out.append((sid, o2, size))
        o = o2 + size
    return out

def code_ranges(d):
    """(module_offset, size) per function body, in the code section."""
    for sid, off, size in sections(d):
        if sid == 10:
            n, p = leb(d, off)
            out = []
            for _ in range(n):
                bsz, p2 = leb(d, p)
                out.append((p2, bsz))
                p = p2 + bsz
            return out, off, size
    return [], 0, 0

def func_names(d):
    """func index -> name, from the `name` custom section subsection 1."""
    for sid, off, size in sections(d):
        if sid != 0: continue
        nl, p = leb(d, off)
        if d[p:p+nl] != b"name": continue
        p += nl
        end = off + size
        names = {}
        while p < end:
            sub = d[p]; p += 1
            ssz, p2 = leb(d, p); p = p2
            if sub == 1:
                cnt, q = leb(d, p)
                for _ in range(cnt):
                    idx, q = leb(d, q)
                    l, q = leb(d, q)
                    names[idx] = d[q:q+l].decode("utf8", "replace"); q += l
            p += ssz
        return names
    return {}

sym = io.open("target/wasm32-unknown-unknown/dist-symbols/fractal_flame_wgpu.wasm", "rb").read()
dist = io.open("target/wasm32-unknown-unknown/dist/fractal_flame_wgpu.wasm", "rb").read()

_, so, ssz = code_ranges(sym)
_, do, dsz = code_ranges(dist)
print(f"code section start: dist {do:,}   dist-symbols {so:,}   (delta {so-do:+,})")
print(f"code section size : dist {dsz:,}   dist-symbols {ssz:,}   (delta {ssz-dsz:+,})")
print()

bodies, _, _ = code_ranges(sym)
names = func_names(sym)
# wasm function index space: imports first, then defined functions.
n_imports = 0
for sid, off, size in sections(sym):
    if sid == 2:
        cnt, p = leb(sym, off)
        for _ in range(cnt):
            for _ in range(2):
                l, p = leb(sym, p); p += l
            kind = sym[p]; p += 1
            if kind == 0:
                _, p = leb(sym, p); n_imports += 1
            elif kind == 1:
                p += 1; _, p = leb(sym, p)
                fl = sym[p-1]
                _, p = leb(sym, p)
                if fl: _, p = leb(sym, p)
            elif kind == 2:
                fl = sym[p]; p += 1
                _, p = leb(sym, p)
                if fl: _, p = leb(sym, p)
            elif kind == 3:
                p += 2
print(f"imported functions: {n_imports}")
print()

for target in [int(a) for a in sys.argv[1:]]:
    adj = target + (so - do)   # shift into the symbols module's coordinates
    hit = None
    for i, (bo, bsz) in enumerate(bodies):
        if bo <= adj < bo + bsz:
            hit = (i, bo, bsz); break
    if hit is None:
        print(f"offset {target:,} -> not inside any function body (adjusted {adj:,})")
        continue
    i, bo, bsz = hit
    nm = names.get(i + n_imports, "<unnamed>")
    print(f"offset {target:,}  ->  func #{i+n_imports}  ({adj-bo:,} bytes into a {bsz:,}-byte body)")
    print(f"    {nm}")
