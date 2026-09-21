#!/usr/bin/env python3
"""Hard measurement for annulus support (docs/projects/forward-bounds.md).

Two questions about the inversive flames -- the ones whose maps have a
pole at a finite point, so no DISC can be invariant -- answered by
running the exact maps on the CPU, not bounds:

  1. Does an invariant region of the shape "outer disc minus one hole
     per pole" exist at all?  Sampled: M points in the candidate
     region, pushed through every map (every julian branch), checked
     for membership.  A leak is a point that leaves.

  2. Do long words contract?  Along the real chaos-game orbit the
     Jacobian of each applied map is composed over windows of L steps
     and its spectral norm recorded.  If ||J_L|| does not fall with L,
     a word's image disc will not shrink below the view, however good
     the region.

A working flame runs as a control so "contracts" has a number.

Usage: python scripts/inversive_probe.py output/flame-zoom/*.fflame
"""
import json
import math
import sys

import numpy as np

TAU = 2.0 * math.pi


# ----------------------------------------------------------- variations
# Copied from the shipped WGSL (src/variations/defs/{basic,advanced}.rs),
# including the guards.  `branch` is julian's random arm, made explicit
# so a finite-difference Jacobian evaluates both sides on the same arm.

def v_linear(p, prm, branch):
    return p

def v_spherical(p, prm, branch):
    r2 = p[0] * p[0] + p[1] * p[1] + 1e-6
    return p / r2

def v_julian(p, prm, branch):
    power = prm["julian.power"]
    dist = prm["julian.dist"]
    cpower = dist / max(abs(power), 1e-30) / 2.0
    r2 = p[0] * p[0] + p[1] * p[1]
    r = r2 ** cpower if r2 > 0 else (0.0 if cpower > 0 else float("inf"))
    theta = math.atan2(p[1], p[0])
    t = (theta + TAU * branch) / power
    return np.array([r * math.cos(t), r * math.sin(t)])

def v_disc(p, prm, branch):
    theta = math.atan2(p[0], p[1])
    r = math.hypot(p[0], p[1])
    tp = theta / math.pi
    pr = math.pi * r
    return np.array([tp * math.sin(pr), tp * math.cos(pr)])

VARS = {"linear": v_linear, "spherical": v_spherical, "julian": v_julian, "disc": v_disc}
# Post-phase identity in 2D; contributes nothing to the normal sum.
IGNORE = {"flatten"}
# Variations with a pole at affine^-1(0).
INVERSIVE = {"spherical", "julian"}


class Xform:
    def __init__(self, t):
        self.w = float(t["weight"])
        self.m = np.array([[t.get("a", 1.0), t.get("b", 0.0)], [t.get("c", 0.0), t.get("d", 1.0)]], float)
        self.t = np.array([t.get("e", 0.0), t.get("f", 0.0)], float)
        self.prm = dict(t.get("variation_params") or {})
        self.vars = []
        for name, vw in (t.get("variations") or {}).items():
            if name in IGNORE or vw == 0.0:
                continue
            if name not in VARS:
                raise SystemExit(f"no CPU body for `{name}` -- add it to VARS")
            self.vars.append((name, float(vw)))
        # julian's arm count; 1 for everything else.
        self.branches = 1
        for name, _ in self.vars:
            if name == "julian":
                self.branches = max(1, int(abs(self.prm["julian.power"])))
        self.inversive = any(n in INVERSIVE for n, _ in self.vars)
        # Where the affine sends to the origin -- the variation's pole.
        self.pole = np.linalg.solve(self.m, -self.t) if abs(np.linalg.det(self.m)) > 1e-12 else None

    def apply(self, p, branch):
        q = self.m @ p + self.t
        out = np.zeros(2)
        for name, vw in self.vars:
            out += vw * VARS[name](q, self.prm, branch)
        return out

    def jacobian(self, p, branch, h=1e-6):
        J = np.zeros((2, 2))
        for k in range(2):
            d = np.zeros(2); d[k] = h
            J[:, k] = (self.apply(p + d, branch) - self.apply(p - d, branch)) / (2 * h)
        return J


def load(path):
    d = json.load(open(path))
    xs = [Xform(t) for t in d["flame"]["transforms"] if t["weight"] > 0]
    return d, xs


# ----------------------------------------------------------- chaos game
def orbit(xs, n, rng, burn=20):
    w = np.array([x.w for x in xs]); w /= w.sum()
    pts = np.zeros((n, 2)); which = np.zeros(n, int); branch = np.zeros(n, int)
    p = rng.uniform(-1, 1, 2)
    respawns = 0
    i = 0
    k = 0
    while i < n:
        j = rng.choice(len(xs), p=w)
        b = rng.integers(xs[j].branches)
        q = xs[j].apply(p, b)
        if not np.all(np.isfinite(q)) or np.abs(q).max() > 1e6:
            p = rng.uniform(-1, 1, 2); respawns += 1; k = 0
            continue
        p = q; k += 1
        if k > burn:
            pts[i] = p; which[i] = j; branch[i] = b; i += 1
    return pts, which, branch, respawns


# ------------------------------------------------- invariant region test
def in_region(P, c_out, r_out, holes):
    """Membership of rows of P in D(c_out, r_out) minus the holes."""
    d = np.linalg.norm(P - c_out, axis=1)
    inside = d <= r_out
    for (hc, hr) in holes:
        inside &= np.linalg.norm(P - hc, axis=1) >= hr
    return inside


def sample_region(m, c_out, r_out, holes, rng):
    out = []
    while sum(len(o) for o in out) < m:
        u = rng.uniform(-1, 1, (4 * m, 2))
        u = u[np.linalg.norm(u, axis=1) <= 1.0] * r_out + c_out
        u = u[in_region(u, c_out, r_out, holes)]
        out.append(u)
    return np.concatenate(out)[:m]


def region_leaks(xs, c_out, r_out, holes, rng, m=20000):
    """Fraction of region points whose image under SOME map and SOME
    branch leaves the region, and where they go."""
    P = sample_region(m, c_out, r_out, holes, rng)
    # Boundary points too: the leaks live there.
    ang = rng.uniform(0, TAU, m // 4)
    ring = c_out + r_out * 0.999 * np.stack([np.cos(ang), np.sin(ang)], 1)
    P = np.concatenate([P, ring])
    for (hc, hr) in holes:
        ang = rng.uniform(0, TAU, m // 4)
        P = np.concatenate([P, hc + hr * 1.001 * np.stack([np.cos(ang), np.sin(ang)], 1)])
    P = P[in_region(P, c_out, r_out, holes)]
    worst = 0.0
    detail = []
    for j, x in enumerate(xs):
        for b in range(x.branches):
            Q = np.array([x.apply(p, b) for p in P])
            ok = np.isfinite(Q).all(axis=1)
            leak_out = ok & (np.linalg.norm(Q - c_out, axis=1) > r_out)
            leak_hole = np.zeros(len(Q), bool)
            for (hc, hr) in holes:
                leak_hole |= ok & (np.linalg.norm(Q - hc, axis=1) < hr)
            f = (leak_out | leak_hole | ~ok).mean()
            worst = max(worst, f)
            if f > 0:
                detail.append((j, b, f, leak_out.mean(), leak_hole.mean(), (~ok).mean()))
    return worst, detail, len(P)


def search_region(name, xs, pts, rng):
    c_out = pts.mean(axis=0)
    # Smallest enclosing disc is overkill; centroid + max distance,
    # refined a few times, is within a few percent.
    for _ in range(8):
        d = np.linalg.norm(pts - c_out, axis=1)
        far = pts[d.argmax()]
        c_out = c_out + 0.1 * (far - c_out)
    r_att = np.linalg.norm(pts - c_out, axis=1).max()
    poles = [(j, x.pole) for j, x in enumerate(xs) if x.inversive and x.pole is not None]
    print(f"     attractor: centre [{c_out[0]:.4f}, {c_out[1]:.4f}]  radius {r_att:.4f}")
    gaps = []
    for j, pole in poles:
        g = np.linalg.norm(pts - pole, axis=1).min()
        gaps.append(g)
        print(f"     pole of xform {j} at [{pole[0]:.4f}, {pole[1]:.4f}]  attractor comes within {g:.3e}")
    if not poles:
        return
    print("     invariance of  D(centre, slack*r) minus D(pole_i, frac*gap_i):")
    print("       slack  frac   worst leak   (per map/branch: out-of-disc, into-hole, non-finite)")
    best = None
    for slack in (1.0, 1.25, 1.5, 2.0, 3.0):
        for frac in (0.9, 0.5, 0.25, 0.1, 0.03):
            holes = [(pole, frac * g) for (_, pole), g in zip(poles, gaps)]
            worst, detail, n = region_leaks(xs, c_out, slack * r_att, holes, rng, m=6000)
            tag = ""
            if worst == 0.0:
                tag = "   <-- INVARIANT (no leak in %d samples)" % n
            elif detail:
                d = max(detail, key=lambda t: t[2])
                tag = f"   xf{d[0]}/b{d[1]}: {d[3]:.3f} out, {d[4]:.3f} hole, {d[5]:.3f} inf"
            print(f"       {slack:<5} {frac:<5} {worst:>10.4f}{tag}")
            if best is None or worst < best[0]:
                best = (worst, slack, frac)
    print(f"     best: leak {best[0]:.4f} at slack {best[1]}, frac {best[2]}")
    # Per transform at the best configuration, because "worst" hides
    # a second leaker behind the first.
    holes = [(pole, best[2] * g) for (_, pole), g in zip(poles, gaps)]
    _, detail, n = region_leaks(xs, c_out, best[1] * r_att, holes, rng, m=6000)
    by_xf = {}
    for j, b, f, fo, fh, fi in detail:
        cur = by_xf.get(j, (0.0, 0.0, 0.0))
        by_xf[j] = (max(cur[0], fo), max(cur[1], fh), max(cur[2], fi))
    for j in range(len(xs)):
        fo, fh, fi = by_xf.get(j, (0.0, 0.0, 0.0))
        print(f"       xform {j}: {fo:.4f} out of the disc, {fh:.4f} into a hole, {fi:.4f} non-finite"
              + ("" if (fo + fh + fi) else "   ok"))


# ------------------------------------------------------------ contraction
def contraction(xs, pts, which, branch, windows=(1, 2, 5, 10, 20, 40, 80, 96, 128, 200)):
    """||J|| of the composed map over windows of L consecutive orbit
    steps.  Composition order: the step applied LAST multiplies on the
    left."""
    L_max = max(windows)
    n = len(pts) - 1
    norms = {L: [] for L in windows}
    J = np.eye(2)
    k = 0
    for i in range(n):
        Ji = xs[which[i + 1]].jacobian(pts[i], branch[i + 1])
        if not np.isfinite(Ji).all():
            J = np.eye(2); k = 0; continue
        J = Ji @ J
        k += 1
        for L in windows:
            if k % L == 0:
                norms[L].append(np.linalg.norm(J, 2) if L == k else None)
        if k == L_max:
            J = np.eye(2); k = 0
    # Only windows that started at a reset count for each L.
    print("     L    median ||J_L||   90th pct     max     frac<1   frac<1e-6   rate = log||J||/L (median)   [n]")
    for L in windows:
        v = np.array([x for x in norms[L] if x is not None and np.isfinite(x) and x > 0])
        if len(v) == 0:
            continue
        med = np.median(v)
        print(f"     {L:<4} {med:>12.3e}  {np.percentile(v, 90):>10.3e}  {v.max():>9.2e}  {(v < 1).mean():>6.3f}   {(v < 1e-6).mean():>8.3f}      {math.log(med) / L:>+.3f}   [{len(v)}]")


# ------------------------------------------------------------ truncation
def truncation(xs, pts, which, c_out):
    """The trade-off a leaky region offers: for a hole radius h around
    each pole, the attractor measure inside the holes (lost), the
    outer radius the hole boundary's image then demands, and the
    measure beyond THAT radius (also lost).  Every inversive flame
    measured has an unbounded attractor, so this is the only kind of
    region there is -- the question is what it costs."""
    poles = [(j, x.pole) for j, x in enumerate(xs) if x.inversive and x.pole is not None]
    dist_c = np.linalg.norm(pts - c_out, axis=1)
    print("     h (hole)   lost in holes   R needed    lost beyond R    total lost")
    for h in (1e-1, 3e-2, 1e-2, 3e-3, 1e-3, 1e-4, 1e-5, 1e-6, 1e-8):
        in_hole = np.zeros(len(pts), bool)
        R_need = 0.0
        for j, pole in poles:
            in_hole |= np.linalg.norm(pts - pole, axis=1) < h
            # Image of the hole boundary under this map, every branch.
            ang = np.linspace(0, TAU, 720, endpoint=False)
            ring = pole + h * np.stack([np.cos(ang), np.sin(ang)], 1)
            for b in range(xs[j].branches):
                Q = np.array([xs[j].apply(p, b) for p in ring])
                Q = Q[np.isfinite(Q).all(axis=1)]
                if len(Q):
                    R_need = max(R_need, np.linalg.norm(Q - c_out, axis=1).max())
        beyond = dist_c > R_need
        lost = (in_hole | beyond).mean()
        print(f"     {h:<9.0e}  {in_hole.mean():>12.3e}  {R_need:>10.3e}  {beyond.mean():>13.3e}  {lost:>12.3e}")


# ------------------------------------------------------------- localize
def inverse_pairs(xs, pts, rng):
    """Which ordered pairs (i, j) satisfy S_j(S_i(p)) == p on the
    attractor, detected numerically rather than assumed.  A map with a
    random branch is many-valued and is never anyone's inverse here."""
    sample = pts[rng.integers(len(pts), size=200)]
    scale = np.linalg.norm(sample, axis=1).max() + 1.0
    pairs = set()
    for i, xi in enumerate(xs):
        if xi.branches > 1:
            continue
        for j, xj in enumerate(xs):
            if xj.branches > 1:
                continue
            err = 0.0
            for p in sample:
                q = xj.apply(xi.apply(p, 0), 0)
                if not np.all(np.isfinite(q)):
                    err = np.inf
                    break
                err = max(err, np.linalg.norm(q - p))
            if err < 1e-6 * scale:
                pairs.add((i, j))
    return pairs


def reduce_word(word, pairs):
    """Free reduction: cancel adjacent (a, b) with S_b o S_a = id.
    `word` is in application order, so adjacency in the list is
    adjacency in the composition."""
    out = []
    for sym in word:
        if out and (out[-1][0], sym[0]) in pairs and out[-1][1] == 0 and sym[1] == 0:
            out.pop()
        else:
            out.append(sym)
    return tuple(out)


def localize(xs, name, rng, steps=4_000_000, hist=20):
    """**Does the measure reaching a view concentrate on few words?**

    That is the one thing cylinder targeting depends on, and the one
    thing the first measurement did not ask.  For samples that land in
    a view, count the distinct words (the last k symbols, application
    order) that carry them, raw and reduced, and how many words hold
    90% of that measure.  A gasket needs one word per depth.  A flame
    whose pieces overlap needs ever more, and no bound on any region
    changes that."""
    w = np.array([x.w for x in xs]); w /= w.sum()
    # Pre-draw the choices: the loop is sequential but the dice are not.
    choice = rng.choice(len(xs), size=steps, p=w)
    branch = np.array([rng.integers(xs[j].branches) for j in choice])
    p = rng.uniform(-1, 1, 2)
    pts = np.zeros((steps, 2))
    ok = np.zeros(steps, bool)
    for i in range(steps):
        q = xs[choice[i]].apply(p, branch[i])
        if not np.all(np.isfinite(q)) or np.abs(q).max() > 1e6:
            p = rng.uniform(-1, 1, 2)
            continue
        p = q
        pts[i] = p
        ok[i] = i > 2000
    pairs = inverse_pairs(xs, pts[ok], rng)
    inv_desc = ", ".join(f"S{j}oS{i}=id" for i, j in sorted(pairs)) or "none"
    print(f"     inverse pairs: {inv_desc}")

    idx = np.flatnonzero(ok)
    centre_of = pts[ok].mean(axis=0)
    extent = np.linalg.norm(pts[ok] - centre_of, axis=1).max()
    print(f"     extent {extent:.3e}")

    for which_c, ci in enumerate([idx[len(idx) // 2], idx[len(idx) // 3]]):
        x = pts[ci]
        print(f"     view centre {which_c}: [{x[0]:.4f}, {x[1]:.4f}]")
        d = np.linalg.norm(pts - x, axis=1)
        for r in (1e-1, 3e-2, 1e-2):
            inside = np.flatnonzero(ok & (d <= r) & (np.arange(steps) >= hist))
            print(f"       radius {r:.0e}: {len(inside)} samples ({len(inside)/len(idx):.2e} of the measure)")
            if len(inside) < 200:
                print("         too few to say anything")
                continue
            print("         depth   raw words  raw for 90%   reduced words  reduced for 90%")
            for k in (2, 4, 6, 8, 10, 12, 14, 16, 18, 20):
                if k > hist:
                    break
                raw = {}
                red = {}
                for i in inside:
                    word = tuple((int(choice[t]), int(branch[t])) for t in range(i - k + 1, i + 1))
                    raw[word] = raw.get(word, 0) + 1
                    rw = reduce_word(word, pairs)
                    red[rw] = red.get(rw, 0) + 1
                def need90(counts):
                    v = sorted(counts.values(), reverse=True)
                    tot = sum(v); acc = 0
                    for n, c in enumerate(v, 1):
                        acc += c
                        if acc >= 0.9 * tot:
                            return n
                    return len(v)
                print(f"         {k:>5}   {len(raw):>9}   {need90(raw):>11}   {len(red):>13}   {need90(red):>15}")


def main(paths):
    rng = np.random.default_rng(7)
    for path in paths:
        d, xs = load(path)
        name = path.replace("\\", "/").split("/")[-1].replace(".fflame", "")
        print(f"\n== {name}")
        for j, x in enumerate(xs):
            print(f"     xform {j}  w={x.w:<6.3f} {[(n, w) for n, w in x.vars]}  "
                  f"pole={None if x.pole is None else np.round(x.pole, 4).tolist()}"
                  f"{'  (' + str(x.branches) + ' branches)' if x.branches > 1 else ''}")
        # **Extent and pole gap against sample size.** A bounded
        # attractor's extent settles; an unbounded one -- a translation
        # in the IFS, or a cascade that sends near-pole points far and
        # far points back near the pole -- keeps growing, and the gap to
        # the pole keeps shrinking, however many samples are drawn.
        # This is the check the forward-sampled invariance test cannot
        # make: the leak it would need to see is a patch of area ~1e-6.
        poles_ = [(j, x.pole) for j, x in enumerate(xs) if x.inversive and x.pole is not None]
        print("     extent and pole gap against sample size:")
        for n in (20000, 200000, 1000000, 5000000):
            p_, _, _, _ = orbit(xs, n, rng)
            r_ = np.linalg.norm(p_, axis=1)
            gaps_ = "  ".join(f"gap(xf{j})={np.linalg.norm(p_ - pole, axis=1).min():.3e}" for j, pole in poles_)
            print(f"       n={n:<8} |p| max {r_.max():>9.3e}   {gaps_}")
        pts, which, br, respawns = orbit(xs, 60000, rng)
        r = np.linalg.norm(pts, axis=1)
        print(f"     orbit of {len(pts)}: |p| in [{r.min():.3e}, {r.max():.3e}], {respawns} respawns")
        search_region(name, xs, pts, rng)
        if any(x.inversive for x in xs):
            print("     truncation trade-off (1M-point orbit):")
            big, bw, _, _ = orbit(xs, 1000000, rng)
            truncation(xs, big, bw, big.mean(axis=0))
        print("     contraction along the orbit:")
        contraction(xs, pts[:40001], which[:40001], br[:40001])


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--localize":
        rng = np.random.default_rng(7)
        for path in args[1:]:
            d, xs = load(path)
            name = path.replace("\\", "/").split("/")[-1].replace(".fflame", "")
            print(f"\n== {name}")
            localize(xs, name, rng)
    else:
        main(args)
