"""Name the function containing a wasm crash offset, by ALIGNING two builds.

The shipped `dist` module is stripped, so a browser reports a trap as a
bare byte offset. This maps that offset to a function name using a
names build of the same commit -- but without the two assumptions that
made the previous attempt produce confident, meaningless answers:

 1. It read `target/wasm32-unknown-unknown/<profile>/*.wasm`. The
    browser loads `pkg/*_bg.wasm`, which wasm-bindgen REWRITES from
    that file -- different functions, different offsets. The offset was
    being looked up in a module the browser never ran.
 2. It assumed function index i in one build is function index i in the
    other, and shifted offsets by the code-section delta. `strip`
    renumbers, so that is false: measured on one pair, index-aligned
    bodies match only 15% of the time. It is the case naive index
    arithmetic gets wrong while looking perfectly plausible.

So this aligns the two body-SIZE sequences with a diff and maps through
the alignment. Measured on a real dist/dist-symbols pair: 13,214 vs
13,205 bodies, 98.7% agreement on the size multiset, and 81.6% of
bodies landing in a matching run. That 81.6% is the honest ceiling --
richer alignment keys were tried and all did WORSE (body ends 61%,
size+12 bytes each end 45%, exact content hash 19%), which says the two
links differ in more than function numbering, so no cheap key
identifies a body across them.

That is why every answer carries a confidence line. An offset in a long
matching run whose two bodies are byte-identical is a name worth acting
on; an offset in an unaligned region gets no name rather than a guess.

USAGE:

  1. Build BOTH from the SAME commit, no `src/` change between:
       ./build-wasm.sh              -> pkg/fractal_flame_wgpu_bg.wasm
       ./build-wasm.sh --symbols    -> pkg/fractal_flame_wgpu_bg.names.wasm
  2. Reproduce with the SHIPPED bundle and copy the offset.
  3. python scripts/wasm-locate.py <offset> [<offset> ...]

Offsets may be decimal or 0x-hex. If the trace gives `wasm-function[N]`,
pass it as `#N` instead: that skips the offset lookup and is the more
reliable input.
"""

import io
import sys
import difflib

SHIPPED = "pkg/fractal_flame_wgpu_bg.wasm"
NAMED = "pkg/fractal_flame_wgpu_bg.names.wasm"


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
    for sid, off, size in sections(d):
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
    for sid, off, size in sections(d):
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


def func_names(d):
    """function index -> name, from the `name` section's subsection 1."""
    for sid, off, size in sections(d):
        if sid != 0:
            continue
        nl, p = leb(d, off)
        if d[p:p + nl] != b"name":
            continue
        p += nl
        end, names = off + size, {}
        while p < end:
            sub = d[p]
            p += 1
            ssz, p2 = leb(d, p)
            p = p2
            if sub == 1:
                cnt, q = leb(d, p)
                for _ in range(cnt):
                    idx, q = leb(d, q)
                    ln, q = leb(d, q)
                    names[idx] = d[q:q + ln].decode("utf8", "replace")
                    q += ln
            p += ssz
        return names
    return {}


def export_names(d):
    """function index -> exported name. These survive `strip`, so when
    the crash lands in an exported function it is named with NO
    cross-build assumption at all -- read that in preference to the
    aligned name whenever both appear."""
    for sid, off, size in sections(d):
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
                out[idx] = nm
        return out
    return {}


def load(path):
    try:
        return io.open(path, "rb").read()
    except FileNotFoundError:
        sys.exit("missing " + path + " -- see the usage note at the top of this script")


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)

    ship, named = load(SHIPPED), load(NAMED)
    sb, nb = bodies(ship), bodies(named)
    if not sb or not nb:
        sys.exit("no code section -- are these wasm modules?")
    names = func_names(named)
    if not names:
        sys.exit(NAMED + " has no name section; build it with --symbols")
    s_imp, n_imp = n_imported_funcs(ship), n_imported_funcs(named)
    s_exports = export_names(ship)

    ssz = [z for _, z in sb]
    nsz = [z for _, z in nb]

    # --- pairing sanity, printed before any answer below it is readable ---
    print("shipped : {}".format(SHIPPED))
    print("          {:,} bytes, {:,} bodies, {:,} code bytes, {:,} imports".format(
        len(ship), len(sb), sum(ssz), s_imp))
    print("named   : {}".format(NAMED))
    print("          {:,} bytes, {:,} bodies, {:,} code bytes, {:,} imports".format(
        len(named), len(nb), sum(nsz), n_imp))
    drift = abs(sum(nsz) - sum(ssz)) / max(sum(ssz), 1)
    if drift > 0.01:
        print("code-size drift: {:.3f}%  <-- TOO LARGE. Not the same commit; "
              "rebuild both and do not read the names below.".format(drift * 100))
    else:
        print("code-size drift: {:.3f}%  (consistent with one commit)".format(drift * 100))

    # --- align the two body-size sequences ---
    sm = difflib.SequenceMatcher(None, ssz, nsz, autojunk=False)
    blocks = sm.get_matching_blocks()
    aligned = sum(n for _, _, n in blocks)
    print("alignment: {:,} of {:,} bodies fall in matching runs ({:.2f}%), "
          "{:,} runs".format(aligned, len(sb), 100.0 * aligned / len(sb), max(len(blocks) - 1, 0)))
    print()

    def map_index(i):
        for a, b, n in blocks:
            if a <= i < a + n:
                return b + (i - a), n
        return None, 0

    for raw in args:
        if raw.startswith("#"):
            fi = int(raw[1:])
            i = fi - s_imp
            if not 0 <= i < len(sb):
                print("{}: function index outside the shipped module's defined range".format(raw))
                continue
            where, into = "function #{}".format(fi), None
        else:
            target = int(raw, 16) if raw.lower().startswith("0x") else int(raw)
            i = next((k for k, (bo, bz) in enumerate(sb) if bo <= target < bo + bz), None)
            if i is None:
                print("offset {:,} -> not inside any function body of the shipped module".format(target))
                continue
            bo, bz = sb[i]
            where, into = "offset {:,}".format(target), (target - bo, bz)

        j, run = map_index(i)
        bo, bz = sb[i]
        line = "{}  ->  shipped func #{}".format(where, i + s_imp)
        if into:
            line += "  ({:,} bytes into a {:,}-byte body)".format(into[0], into[1])
        print(line)
        exported = s_exports.get(i + s_imp)
        if exported:
            print("    exported as `{}`  <-- from the shipped module itself, "
                  "no alignment involved".format(exported))
        if j is None:
            print("    <no aligned counterpart -- one of the bodies the two builds disagree on>")
            print()
            continue
        nbo, nbz = nb[j]
        same = ship[bo:bo + bz] == named[nbo:nbo + nbz]
        conf = ("EXACT (the two bodies are byte-identical)" if same else
                "high (same size, inside an aligned run)" if bz == nbz else
                "LOW (aligned but the sizes differ)")
        print("    {}".format(names.get(j + n_imp, "<unnamed>")))
        print("    confidence: {}; its aligned run is {:,} bodies long".format(conf, run))
        print()


if __name__ == "__main__":
    main()
