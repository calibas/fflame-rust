"""Remove wasm custom sections by byte edit, without relinking.

The load-freeze crash appears ONLY in the stripped `dist` build. Every
build that keeps names fails to reproduce it -- and the two are not
the same program: measured, `dist` and `dist-symbols` differ by nine
functions, 4,638 differing body sizes, and not one body matches by
content hash. `strip` changes the LINK, so a name section cannot be
carried across that boundary and offsets cannot be mapped across it
either.

This separates the two explanations. Take the build that HAS names and
delete the name section from the bytes: the code section is untouched,
so the program is identical, but the module drops ~2.9 MB to roughly
the shipped size.

  reproduces  -> it was module size / load timing, and every offset
                 maps EXACTLY back to the unstripped file's names
  does not    -> the crash needs the stripped LINK itself, and no
                 amount of symbol-preserving tooling will ever see it

Usage:  python scripts/wasm-strip-names.py in.wasm out.wasm
"""
import io, sys

def leb(b, o):
    r = 0; s = 0
    while True:
        x = b[o]; o += 1
        r |= (x & 0x7f) << s; s += 7
        if not x & 0x80:
            return r, o

def main(src, dst):
    d = io.open(src, "rb").read()
    out = bytearray(d[:8])
    o = 8
    removed = []
    code_off_before = code_off_after = None
    while o < len(d):
        start = o
        sid = d[o]; o += 1
        size, body = leb(d, o)
        end = body + size
        keep = True
        if sid == 0:
            nl, p = leb(d, body)
            nm = d[p:p + nl].decode("utf8", "replace")
            if nm in ("name", ".debug_info", ".debug_abbrev", ".debug_line",
                      ".debug_str", ".debug_ranges", ".debug_loc"):
                keep = False
                removed.append((nm, size))
        if sid == 10:
            code_off_before = body
            code_off_after = len(out) + (body - start)
        if keep:
            out += d[start:end]
        o = end
    io.open(dst, "wb").write(bytes(out))
    print(f"in  {len(d):,} bytes -> out {len(out):,} bytes")
    for nm, sz in removed:
        print(f"  removed custom section {nm!r}: {sz:,} bytes")
    if code_off_before is not None:
        shift = code_off_after - code_off_before
        print(f"  code section offset shift: {shift:+,}")
        print(f"  => an offset X in the stripped file is X{-shift:+,} in the original")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
