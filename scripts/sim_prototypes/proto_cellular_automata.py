"""Hodgepodge, cyclic CA, spatial rock-paper-scissors and Ising.

Phase-0 measurement for simulation mode: the Tier-1 integer/stochastic
cellular automata. Each is checked for the thing the catalogue claims
about it, and timed for the step budget.

All four "never still" in the Gray-Scott sense -- they are animated
subjects -- so the useful number is not settling but **steps to
developed**: the point where the churn rate (fraction of cells changing
per step) stops falling. That is what a still export's `steps` should
default to, and it is measured here rather than guessed.

Three of the four also carry `[verify]` marks in the catalogue on
parameters taken from secondary sources. Those are the runs.

Writes PNGs and a JSON row set into output/sim_proto/.
"""
import json
import os
import time

import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)
N = 256


def save_states(name, s, n_states):
    """Cyclic palette, so state 0 and state N-1 are adjacent hues."""
    t = (np.asarray(s, dtype=np.float64) % n_states) / max(n_states, 1)
    h = t * 6.0
    i = np.floor(h).astype(int) % 6
    f = h - np.floor(h)
    v, p, q = 1.0, 0.15, 1.0 - f * 0.85
    w = 0.15 + f * 0.85
    r = np.select([i == 0, i == 1, i == 2, i == 3, i == 4, i == 5],
                  [v, q, p, p, w, v])
    g = np.select([i == 0, i == 1, i == 2, i == 3, i == 4, i == 5],
                  [w, v, v, q, p, p])
    b = np.select([i == 0, i == 1, i == 2, i == 3, i == 4, i == 5],
                  [p, p, w, v, v, q])
    img = (np.dstack([r, g, b]) * 255).astype(np.uint8)
    Image.fromarray(img).save(os.path.join(OUT, name))


def save_binary(name, s):
    Image.fromarray((np.asarray(s) > 0).astype(np.uint8) * 255).save(
        os.path.join(OUT, name))


def moore(a):
    """The eight neighbours, as a list, periodic."""
    return [np.roll(np.roll(a, dy, 0), dx, 1)
            for dy in (-1, 0, 1) for dx in (-1, 0, 1) if (dy, dx) != (0, 0)]


def von_neumann(a):
    return [np.roll(a, -1, 0), np.roll(a, 1, 0), np.roll(a, -1, 1), np.roll(a, 1, 1)]


class Churn:
    """Steps to developed: where the change rate stops falling.

    'Never stills' models have a churn rate that decays to a plateau
    rather than to zero, so the settle metric never fires. This reports
    the step at which the rate first comes within 10% of its final
    value -- the point after which more steps do not change the
    character of the picture."""

    def __init__(self):
        self.hist = []

    def feed(self, rate):
        self.hist.append(float(rate))

    def developed(self):
        if len(self.hist) < 10:
            return None
        tail = np.mean(self.hist[-max(3, len(self.hist) // 10):])
        for i, r in enumerate(self.hist):
            if abs(r - tail) <= 0.1 * max(tail, 1e-9):
                return i
        return None

    def plateau(self):
        return float(np.mean(self.hist[-max(3, len(self.hist) // 10):])) if self.hist else 0.0


def run_hodgepodge(q=200, k1=2, k2=3, g=70, steps=600, seed=1, snaps=()):
    """healthy: A/k1 + B/k2 ; infected: S/(A+B+1) + g ; ill: 0 ; capped at q."""
    rg = np.random.default_rng(seed)
    s = rg.integers(0, q + 1, (N, N)).astype(np.int64)
    ch, t0 = Churn(), time.time()
    for step in range(1, steps + 1):
        nb = moore(s)
        A = sum(((n > 0) & (n < q)).astype(np.int64) for n in nb)
        B = sum((n == q).astype(np.int64) for n in nb)
        S = s + sum(nb)
        healthy = A // k1 + B // k2
        infected = S // (A + B + 1) + g
        nxt = np.where(s == 0, healthy, np.where(s == q, 0, infected))
        nxt = np.minimum(nxt, q)
        ch.feed((nxt != s).mean())
        s = nxt
        if step in snaps:
            save_states(f"hodgepodge_{step}.png", s, q + 1)
    save_states("hodgepodge.png", s, q + 1)
    return {"model": "hodgepodge", "params": f"q={q} k1={k1} k2={k2} g={g}",
            "steps_run": steps, "developed_at": ch.developed(),
            "churn_plateau": round(ch.plateau(), 4),
            "distinct_states": int(len(np.unique(s))),
            "seconds": round(time.time() - t0, 1)}


def run_cyclic(n_states=14, R=1, T=1, nbh="von_neumann", steps=600, seed=2, snaps=()):
    """Advance to (s+1) mod N when >= T neighbours are already there."""
    rg = np.random.default_rng(seed)
    s = rg.integers(0, n_states, (N, N)).astype(np.int64)
    ch, t0 = Churn(), time.time()
    offs = [(dy, dx)
            for dy in range(-R, R + 1) for dx in range(-R, R + 1)
            if (dy, dx) != (0, 0)
            and (nbh == "moore" or abs(dy) + abs(dx) <= R)]
    for step in range(1, steps + 1):
        nxt_state = (s + 1) % n_states
        count = sum((np.roll(np.roll(s, dy, 0), dx, 1) == nxt_state).astype(np.int64)
                    for dy, dx in offs)
        nxt = np.where(count >= T, nxt_state, s)
        ch.feed((nxt != s).mean())
        s = nxt
        if step in snaps:
            save_states(f"cyclic_{R}_{T}_{n_states}_{step}.png", s, n_states)
    save_states(f"cyclic_{R}_{T}_{n_states}.png", s, n_states)
    return {"model": "cyclic_ca", "params": f"{R}/{T}/{n_states} {nbh}",
            "steps_run": steps, "developed_at": ch.developed(),
            "churn_plateau": round(ch.plateau(), 4),
            "distinct_states": int(len(np.unique(s))),
            "seconds": round(time.time() - t0, 1)}


def run_rps(p_sel=1.0, p_rep=1.0, species=3, steps=600, seed=3, snaps=()):
    """Each cell picks a random neighbour; loses to it with probability p_sel.

    0 is empty; 1..species are the species, each beating the next
    cyclically."""
    rg = np.random.default_rng(seed)
    s = rg.integers(0, species + 1, (N, N)).astype(np.int64)
    ch, t0 = Churn(), time.time()
    for step in range(1, steps + 1):
        nb = np.stack(moore(s))
        pick = rg.integers(0, 8, (N, N))
        other = np.take_along_axis(nb, pick[None], 0)[0]
        # a beats b when a == b + 1 (mod species), on 1..species
        beats = (s > 0) & (other > 0) & (((other - 1) % species) ==
                                         ((s - 1 + 1) % species))
        take = beats & (rg.random((N, N)) < p_sel)
        fill = (s == 0) & (other > 0) & (rg.random((N, N)) < p_rep)
        nxt = np.where(take | fill, other, s)
        ch.feed((nxt != s).mean())
        s = nxt
        if step in snaps:
            save_states(f"rps_{step}.png", s, species + 1)
    save_states("rps.png", s, species + 1)
    return {"model": "spatial_rps", "params": f"p_sel={p_sel} p_rep={p_rep} k={species}",
            "steps_run": steps, "developed_at": ch.developed(),
            "churn_plateau": round(ch.plateau(), 4),
            "survivors": int(len(np.unique(s[s > 0]))),
            "seconds": round(time.time() - t0, 1)}


def run_ising(T, sweeps=600, J=1.0, H=0.0, seed=4, snaps=()):
    """Metropolis on a checkerboard: two half-passes per sweep."""
    rg = np.random.default_rng(seed)
    s = rg.choice(np.array([-1, 1]), (N, N)).astype(np.int64)
    y, x = np.indices((N, N))
    masks = [((y + x) % 2 == 0), ((y + x) % 2 == 1)]
    ch, t0 = Churn(), time.time()
    mags = []
    for step in range(1, sweeps + 1):
        changed = 0
        for m in masks:
            nb = sum(von_neumann(s))
            dE = 2.0 * s * (J * nb + H)
            acc = (dE <= 0) | (rg.random((N, N)) < np.exp(-np.clip(dE, 0, 60) / T))
            flip = m & acc
            changed += int(flip.sum())
            s = np.where(flip, -s, s)
        ch.feed(changed / s.size)
        mags.append(abs(float(s.mean())))
        if step in snaps:
            save_binary(f"ising_T{T}_{step}.png", s)
    save_binary(f"ising_T{T}.png", s)
    return {"model": "ising", "params": f"T={T}",
            "steps_run": sweeps, "developed_at": ch.developed(),
            "churn_plateau": round(ch.plateau(), 4),
            "abs_magnetisation": round(float(np.mean(mags[-50:])), 4),
            "seconds": round(time.time() - t0, 1)}


def main():
    rows = []
    print("hodgepodge (catalogue: spirals at q=200 k1=2 k2=3 g=70, secondary source)")
    rows.append(run_hodgepodge(snaps=(50, 200, 600)))
    print("cyclic CA")
    rows.append(run_cyclic(14, 1, 1, "von_neumann", snaps=(50, 200, 600)))
    rows.append(run_cyclic(3, 1, 3, "moore", snaps=(200,)))
    print("spatial RPS")
    rows.append(run_rps(snaps=(50, 200, 600)))
    print("Ising (T_c = 2.269)")
    for T in (1.5, 2.269, 3.5):
        rows.append(run_ising(T, snaps=(50, 600)))

    with open(os.path.join(OUT, "ca_step_costs.json"), "w") as f:
        json.dump(rows, f, indent=1)
    print(f"\n{'model':<14}{'params':<26}{'developed':>10}{'churn':>9}{'sec':>7}  extra")
    for r in rows:
        extra = ""
        for k in ("distinct_states", "survivors", "abs_magnetisation"):
            if k in r:
                extra = f"{k}={r[k]}"
        print(f"{r['model']:<14}{r['params']:<26}{str(r['developed_at']):>10}"
              f"{r['churn_plateau']:>9}{r['seconds']:>7}  {extra}")


if __name__ == "__main__":
    main()
