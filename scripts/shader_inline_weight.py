"""How big a WGSL shader is once every call is inlined, as GPU compilers do.

A driver compiles the entry point with every function inlined at every
call site, so a function called from ten places is compiled ten times.
This estimates that size from the source alone: each function's own text
plus, for every call it makes, the callee's inlined size. Writes, per
file, the entry point's inlined size against the source's, and the
functions that contribute the most (their inlined size times how many
times the entry reaches them).

    python scripts/shader_inline_weight.py output/compile-cost/*.wgsl
"""
import re
import sys
from functools import lru_cache


def analyse(path):
    src = open(path, encoding="utf-8").read()
    # Strip comments: a call in a comment is no call.
    code = re.sub(r"//[^\n]*", "", src)
    starts = [(m.group(1), m.start()) for m in re.finditer(r"\bfn\s+(\w+)\s*\(", code)]
    bodies = {}
    for i, (name, at) in enumerate(starts):
        end = starts[i + 1][1] if i + 1 < len(starts) else len(code)
        bodies[name] = code[at:end]
    names = set(bodies)
    call_re = {n: re.compile(r"\b" + re.escape(n) + r"\s*\(") for n in names}

    def calls(f):
        body = bodies[f]
        out = {}
        for n in names:
            if n == f:
                continue
            k = len(call_re[n].findall(body))
            if k:
                out[n] = k
        return out

    graph = {f: calls(f) for f in names}

    @lru_cache(maxsize=None)
    def inlined(f, depth=0):
        if depth > 40:
            return len(bodies[f])
        return len(bodies[f]) + sum(k * inlined(g, depth + 1) for g, k in graph[f].items())

    # Entry points: functions marked @compute (the attribute precedes fn).
    entries = [m.group(1) for m in re.finditer(r"@compute[^\n]*\n(?:\s*@[^\n]*\n)*\s*fn\s+(\w+)", code)]
    if not entries:
        entries = [n for n in names if n in ("main", "escape_main")]
    # How many times the entry reaches each function, through every path.
    reach = {}

    def walk(f, times, depth=0):
        if depth > 40:
            return
        reach[f] = reach.get(f, 0) + times
        for g, k in graph[f].items():
            walk(g, times * k, depth + 1)

    for e in entries:
        walk(e, 1)
    total = sum(inlined(e) for e in entries)
    contrib = sorted(((reach[f] * len(bodies[f]), reach[f], f) for f in reach), reverse=True)
    return len(src), total, contrib


def main():
    for path in sys.argv[1:]:
        size, total, contrib = analyse(path)
        print(f"{path}: {size} B source, {total} B inlined ({total / max(size, 1):.1f}x)")
        for weight, times, f in contrib[:8]:
            print(f"    {weight:>9} B  {f} x{times}")


if __name__ == "__main__":
    main()
