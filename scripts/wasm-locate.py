"""Identify the function containing a wasm crash offset, from the
SHIPPED module alone.

A browser reports a trap in the stripped `dist` bundle as a bare byte
offset. This turns that into an identification without needing the
crash to survive an instrumented build, and -- the part that matters --
without comparing against a second build at all.

WHY NOT COMPARE BUILDS. Three earlier versions of this tool did, and
each produced confident, meaningless names:

 1. It read `target/wasm32-unknown-unknown/<profile>/*.wasm`. The
    browser loads `pkg/*_bg.wasm`, which wasm-bindgen REWRITES from
    that file -- 11,443 function bodies against the raw module's
    13,214. Offsets were being looked up in a module nobody ran.
 2. It assumed function index i in one build is function index i in the
    other. `strip` renumbers; index-aligned bodies match 15% of the
    time.
 3. Aligning the two builds' body-size sequences with a diff looked
    much better -- until it was MEASURED. Exports named in both modules
    are ground truth, so map each shipped index through the alignment
    and compare: of 22 testable, 9 correct, 6 WRONG, 7 unaligned, with
    the wrong ones missing by thousands of indices
    (`wasmapi_get_config_json` mapped to 4,752 against a truth of
    8,971). A tool wrong 40% of the time it answers is worse than one
    that stays quiet.

WHAT WORKS. The module names itself, three ways, none involving a
second build:

 - **String literals.** Rust materialises a `&str` as an
   `i32.const <addr>, i32.const <len>` pair pointing into a data
   segment. Read them back and a function announces what it is. This is
   what actually solved the load crash: one 5,905-byte body holding
   'handler woken up without user event' and the `unreachable!`
   message -- two strings that occur together in exactly one place in
   winit 0.30.13, its web event-loop handler closure.
 - **Export names**, which survive `strip`. Several exports routinely
   share one function index (`__wasm_bindgen_func_elem_10607_11`, `_12`,
   `_13` ... are one trampoline), so they are reported as a list.
 - **Body geometry** -- index, size, offset into it -- which at least
   says whether two crash offsets are the same function.

USAGE:

  python scripts/wasm-locate.py <offset|#index> [...]

Offsets may be decimal or 0x-hex; `#N` is a `wasm-function[N]` from a
Chrome trace. Reads `pkg/fractal_flame_wgpu_bg.wasm`, whatever
`./build-wasm.sh` last put there.

AN OFFSET IS ONLY MEANINGFUL AGAINST THE MODULE THAT PRODUCED IT. Any
rebuild moves every function, and a stale offset will still resolve --
to a different function, with a straight face. Read the offset out of
the browser and resolve it before rebuilding, or keep a copy of the
module you reproduced with.

Capture Firefox's async stack in full when you have one: it chains the
causality (`init -> microtask -> addEventListener ->
requestAnimationFrame -> trap`), and that alone placed the load crash
in a rAF callback.
"""

import io
import sys

SHIPPED = "pkg/fractal_flame_wgpu_bg.wasm"


def leb(b, o, signed=False):
    r = s = 0
    while True:
        x = b[o]
        o += 1
        r |= (x & 0x7F) << s
        s += 7
        if not x & 0x80:
            if signed and s < 64 and x & 0x40:
                r |= -(1 << s)
            return r, o


def sections(d):
    o, out = 8, []
    while o < len(d):
        sid = d[o]
        o += 1
        size, o2 = leb(d, o)
        out.append((sid, o2, size))
        o = o2 + size
    return out


def bodies(d):
    """[(module_offset, size)] per function body, in code-section order."""
    for sid, off, _ in sections(d):
        if sid == 10:
            n, p = leb(d, off)
            out = []
            for _ in range(n):
                bsz, p2 = leb(d, p)
                out.append((p2, bsz))
                p = p2 + bsz
            return out
    return []


def n_imported_funcs(d):
    """Imports occupy the low function indices, so bodies start after them."""
    for sid, off, _ in sections(d):
        if sid != 2:
            continue
        cnt, p = leb(d, off)
        n = 0
        for _ in range(cnt):
            for _ in range(2):
                ln, p = leb(d, p)
                p += ln
            kind = d[p]
            p += 1
            if kind == 0:
                _, p = leb(d, p)
                n += 1
            elif kind == 1:
                p += 1
                fl = d[p]
                p += 1
                _, p = leb(d, p)
                if fl:
                    _, p = leb(d, p)
            elif kind == 2:
                fl = d[p]
                p += 1
                _, p = leb(d, p)
                if fl:
                    _, p = leb(d, p)
            elif kind == 3:
                p += 2
        return n
    return 0


def export_names(d):
    """function index -> [exported names]; these survive `strip`.

    A list, not a name: several exports routinely share one index."""
    for sid, off, _ in sections(d):
        if sid != 7:
            continue
        cnt, p = leb(d, off)
        out = {}
        for _ in range(cnt):
            ln, p = leb(d, p)
            nm = d[p:p + ln].decode("utf8", "replace")
            p += ln
            kind = d[p]
            p += 1
            idx, p = leb(d, p)
            if kind == 0:
                out.setdefault(idx, []).append(nm)
        return out
    return {}


def data_segments(d):
    """[(memory address, bytes)] for active data segments."""
    for sid, off, _ in sections(d):
        if sid != 11:
            continue
        cnt, p = leb(d, off)
        segs = []
        for _ in range(cnt):
            flags, p = leb(d, p)
            addr = 0
            if flags == 2:
                _, p = leb(d, p)
            if flags in (0, 2):
                if d[p] != 0x41:  # i32.const
                    return segs
                p += 1
                addr, p = leb(d, p, signed=True)
                if d[p] != 0x0B:  # end
                    return segs
                p += 1
            ln, p = leb(d, p)
            segs.append((addr, d[p:p + ln]))
            p += ln
        return segs
    return []


def strings_in(body, segs):
    """String literals a function body references.

    Rust materialises a `&str` as its address then its length, so an
    `i32.const <addr>, i32.const <len>` pair whose target lands in a
    data segment and decodes to printable bytes is a literal. Scanning
    for the opcode rather than decoding instructions can in principle
    match an immediate inside another instruction; the strings validate
    themselves, so a false positive reads as noise rather than as a
    plausible wrong answer."""
    if not segs:
        return {}
    lo = min(a for a, _ in segs)
    hi = max(a + len(b) for a, b in segs)

    def read(addr, n):
        for a, b in segs:
            if a <= addr and addr + n <= a + len(b):
                return b[addr - a:addr - a + n]
        return None

    found, i = {}, 0
    while i < len(body) - 1:
        if body[i] != 0x41:
            i += 1
            continue
        try:
            v, j = leb(body, i + 1, signed=True)
        except IndexError:
            break
        if lo <= v < hi and j < len(body) - 1 and body[j] == 0x41:
            try:
                n, _ = leb(body, j + 1, signed=True)
            except IndexError:
                break
            if 2 <= n <= 300:
                s = read(v, n)
                if s and all(32 <= c < 127 or c in (9, 10) for c in s):
                    t = s.decode()
                    found[t] = found.get(t, 0) + 1
        i = j
    return found


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    try:
        d = io.open(SHIPPED, "rb").read()
    except FileNotFoundError:
        sys.exit("missing " + SHIPPED + " -- run ./build-wasm.sh first")

    sb = bodies(d)
    if not sb:
        sys.exit("no code section -- is that a wasm module?")
    imp = n_imported_funcs(d)
    exp = export_names(d)
    segs = data_segments(d)
    print("{}: {:,} bytes, {:,} bodies, {:,} imports, {:,} data segments".format(
        SHIPPED, len(d), len(sb), imp, len(segs)))
    print()

    for raw in args:
        if raw.startswith("#"):
            i = int(raw[1:]) - imp
            if not 0 <= i < len(sb):
                print("{}: outside the module's defined function range".format(raw))
                continue
            where, into = "function {}".format(raw), None
        else:
            t = int(raw, 16) if raw.lower().startswith("0x") else int(raw)
            i = next((k for k, (bo, bz) in enumerate(sb) if bo <= t < bo + bz), None)
            if i is None:
                print("offset {:,} -> not inside any function body".format(t))
                continue
            where, into = "offset {:,}".format(t), t - sb[i][0]

        bo, bz = sb[i]
        line = "{}  ->  func #{} (body #{}), {:,} bytes".format(where, i + imp, i, bz)
        if into is not None:
            line += ", trap at +{:,}".format(into)
        print(line)
        for nm in exp.get(i + imp, []):
            print("    exported as `{}`".format(nm))
        lits = strings_in(d[bo:bo + bz], segs)
        if not lits:
            print("    (references no string literals -- try the callers, or")
            print("     another offset from the same stack)")
        else:
            print("    references {} string literal(s):".format(len(lits)))
            for s, c in sorted(lits.items(), key=lambda kv: -len(kv[0])):
                print("      {}x {!r}".format(c, s))
        print()


if __name__ == "__main__":
    main()
