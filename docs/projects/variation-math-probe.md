# Variation math probe

Evaluates every shipped variation's shader arithmetic directly and
writes one compact, diffable report. Run it on two platforms, diff the
reports, and a divergence names the variation and the input that moved.

```bash
cargo run --release --bin variation_probe
cargo run --release --bin variation_probe -- compare OLD NEW
```

Output, both committed:

| file | one line per | `classes` column |
|---|---|---|
| `variation-probe.txt` | variation × dimension, at **default** parameters | one glyph per output component, in input order |
| `variation-probe-sweep.txt` | variation × **parameter** × dimension | a fixed-width presence mask per swept value |

Plus `variation-probe.timings.txt`, which is *not* compared across
machines and is gitignored. `--no-sweep` runs the base pass alone.

## Why it exists

`npolar` rendered differently on macOS because Metal runs shaders with
fast-math, where `atan2(0, 0)` returns π/4 instead of a signed zero. It
cost that flame 73% of its lit pixels, and it was found by bisecting a
visual difference. `CLAUDE.md` records that **616 `atan2` call sites
across 86 files remain unaudited**.

A visual test can tell you a picture changed. It cannot tell you which
of the variations in that picture computed what, at which input. This
can.

It does **not** replace the visual regression suite. That suite covers
composition and the full pipeline; this covers the math of a variation
in isolation.

## The two signals

Comparing raw `f32` across vendors is a non-signal — different GPUs
legitimately differ in `sin`/`cos`/`exp`, in FMA contraction, and in
reassociation. Hashing the bits would flag all 646 variations on any
other GPU, which tells you exactly as much as a golden image that fails
everywhere.

So each output carries two independent signals:

| | what it is | what a difference means |
|---|---|---|
| **class** (hard) | is it zero, finite, NaN, infinite, past 1e32 — and the sign | a real behavioural difference; no rounding can move a value between these |
| **digest** (soft) | magnitudes on a relative 1e-4 log grid | numbers moved past tolerance — worth reading, not proof |

Every known Metal divergence is a class change:

| | IEEE | Metal fast-math |
|---|---|---|
| `x != x` for NaN | true | false |
| `Inf / Inf` | NaN | 1.0 |
| `atan2(0, 0)` | ±0 / ±π | π/4 |

Keeping them apart is the point. Merged into one hash, the soft signal's
false positives would bury the hard signal's real ones, and the report
would get ignored the way a noisy test does. `compare` exits non-zero
only for the hard signal.

## How it works

The probe does **not** implement variation dispatch. It builds a flame
whose transform *i* holds exactly variation *i*, hands it to the
ordinary `ShaderBuilder`, and calls the generated
`apply_variations(xform_i, i, …)`. Phase ordering, weight folding, the
`NeedsAccum` and `WritesColor` plumbing, helper-library splicing — all
of it is the renderer's own code, because it is the renderer's own code.

The alternative — a harness calling each variation function directly —
would mean reimplementing the ~300 lines of signature and call-site
generation in `build_apply_variations_2d`. Two copies of that drift, and
a probe that drifts tests something the renderer does not do.

The entry point therefore lives in `main_template.wgsl`, beside the real
call sites, gated by a `PROBE` flag, so its call inherits the same
`HAS_DC` / `HAS_RGB` / `HAS_ANALYTIC_BLUR` conditionals. With the flag
off the emitted WGSL is byte-identical — the same contract
`solid_enabled` holds to, enforced by `probe_off_is_byte_identical`.

I/O rides on the histogram binding, so the bind group layout is
unchanged and the probe can reuse a live `FlameRenderer`'s bind group.

646 variations pack into **7 flames** (99 each, under both the
variations-per-flame and the 1600-slot caps), so the whole run is 14
shader compiles.

## The parameter sweep

The base pass evaluates every variation at its defaults, which exercises
exactly one path through code that often branches on those parameters.
A variation whose `if (fixed_dist_calc)` arm is only reachable with the
flag on is, at default, half untested.

The sweep moves **one parameter at a time**, every other at default.
Booleans get both arms; enums get every choice; numeric parameters get
their extremes plus zero — zero only when it lies strictly inside the
declared range, because probing a must-be-positive scale at zero would
report a NaN about a state the app cannot reach.

It works. **386 sweep entries reach NaN, infinity, or past-threshold
values that the default parameters never do** — `julian.power` at one
value returns NaN at every single input. That is the coverage the base
pass structurally cannot have.

Cost: 3231 parameters, 8986 values, **1082 dispatches per dimension** and
no extra compiles. Parameters are read through `get_param` from a storage
buffer, so a step is a `queue.write_buffer`, not a shader rebuild.

The dispatch count is `max steps of any one variation`, not the product
of parameters and values, because every variation sits in its own
transform and round *r* can set step *r* for all of them at once. That
matters: `lsystem_path_3D` carries 157 parameters, and a
parameter × value grid would have cost 2651 dispatches per dimension
instead of 1082.

### Why the sweep column is a presence mask

Recording a class per input would multiply that report by 27. The first
attempt instead picked the single "most notable" class via a ranking —
and any ranking is wrong somewhere. Ordering zero above ordinary finites
made every variation returning zero at *one* input read as returning
zero everywhere, which flagged **1103** sweep entries as producing no
output when they produce plenty. The mask needs no ranking and loses
nothing about which kinds occurred; the count fell to 531, and those
look genuine.

What the mask does lose: which input produced which class, and the split
between components. Both are stated in the report's own header, and both
are recoverable by rerunning the probe on the variation alone. A change
confined to one input — or a sign flip — shows up in the digest, which
covers every raw sample.

## Things that were nearly wrong

Recorded because each would have produced a *green* report that tested
nothing, which is worse than a red one.

- **Pre and post variations need a carrier.** The dispatcher runs
  pre → normal → post. A transform holding only a pre variation returns
  the empty normal-phase sum — zero — whatever the pre variation
  computed. 45 variations would have shown a column of identical glyphs
  and read as passing. They get a `linear` alongside them; normal
  variations deliberately do not, because there the `+p` offset would
  mask a target wrongly returning zero, which is the atan2 signature.

- **The init dispatch does not run in `load_config`.** It rides inside
  `render()`, which the probe never calls. Without
  `FlameRenderer::run_init_pass`, the 134 variations with init-derived
  slots read zeros — stable, reproducible, identical on every platform,
  and meaningless. `ripple` returning NaN at every input is what exposed
  it. Fixing it took all-NaN entries from 2 to 0 and all-zero from 56 to
  35.

- **`+0.0 == -0.0` in Rust.** The obvious classifier folds them
  together, losing exactly the distinction `atan2` is specified on. The
  classifier reads the sign bit, for the same reason the `npolar` guard
  uses `bitcast<u32>`.

- **Trimming hid a real difference.** The identity check originally
  compared `trim_end()`ed strings, which let a stray blank line and a
  CRLF/LF mismatch reach every canonical shader dump. It now compares
  exactly.

- **Init-derived slots go stale mid-sweep.** They are computed *from*
  the user parameters, so moving one invalidates them. Without re-running
  the init pass each step, the sweep would probe new parameters against
  the previous step's derived values — a state no real flame is ever in.

- **Sweep timings are per round, not per batch.** Batches do wildly
  different amounts of sweep work, so comparing totals just flags the
  batch that legitimately does the most.

## What the report does not cover

Stated in the report's own header, not just here:

- The accumulate and tonemap passes, and how variations compose.
- The colour registers (`vc`, `vrc`) that `WritesColor` / `WritesRgb`
  variations drive, and the `should_hide` flag of the `CanHide` family.
  Position only, today.
- Entries with **no observable output** are named in the header. Some
  are correct — a Z-only variation contributes `(0, 0)` in 2D by design
  — but a line of zeros carries no signal either way, and a reader
  comparing two reports would otherwise count them as agreement.
- Combinations of parameters. The sweep moves one at a time, so a bug
  needing two parameters to conspire is out of scope — combinatorial
  coverage would be millions of dispatches for a class of bug that may
  not exist.
- Enum choices past the eighth, if any variation ever ships that many.
  The cap is reported per entry rather than applied silently.

## Timings

Kept out of the diffable report on purpose: absolute times vary by
machine, by thermal state, and by run, and a report that is always noisy
is one nobody reads. What travels is the *spread within one run*.

Compile and dispatch are timed separately and outliers are computed
**per phase**. Pooling them was actively misleading — driver compiles
run two orders of magnitude longer than the dispatches they set up, so a
single median sat between the two populations, flagged every compile,
and hid the one that was genuinely pathological.

**The first run on a machine is slow.** Cold, the NVIDIA driver took
~3m45s across the 14 compiles, with one batch alone at 84 seconds.
Warm — the driver's on-disk shader cache — the same run takes about 3
seconds. A first run on a new machine, or after a driver update, will
look alarming and is not.

## Hangs

A GPU hang is a device loss that takes the process with it and cannot be
caught in-process. The binary writes `variation-probe.progress` naming
the batch in flight before dispatching it, and removes it on success. If
a run dies, that file is the diagnosis.
