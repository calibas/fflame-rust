# Visual Regression Testing

Renders every config under `configs/` through the release binary's CLI
export and compares each picture against a committed baseline, with a
tolerance. It needs a GPU and takes minutes, so `scripts/release.py
check` does not run it; run it yourself before a release and after any
change that can move pixels. The wider testing picture is in
[docs/TESTING-GUIDE.md](../../docs/TESTING-GUIDE.md).

## Quick start

```bash
# 1. Build the binary. run_tests.py does NOT build it, and silently
#    tests whatever binary is already there.
cargo build --release

# 2. Run the suite
python tests/visual/run_tests.py

# 3. After checking that a change is intended, update the baselines
python tests/visual/run_tests.py --update-baseline
```

Run from the repository root: the script resolves `tests/visual/...`
and `target/...` against the current directory.

## Requirements

- Python 3, with **Pillow and numpy** (`pip install Pillow numpy`).
  Without them the script falls back to an exact file-hash comparison,
  which fails on any difference at all and cannot read render times
  from PNG metadata.
- A GPU the binary can use.

## Options

| flag | effect |
|---|---|
| `--category NAME` | run one category only (`2d`, `3d`, `escape`, `sim`, `solid`, `tonemap`, `variations`) |
| `--update-baseline` | run the suite, then rewrite the baselines that moved (see below) |
| `--binary PATH` | test this executable instead of `target/release/FractalArtEditor(.exe)` |
| `--debug` | test `target/debug/FractalArtEditor(.exe)` instead |
| `--skip-warmup` | skip the warmup render |

Exit code 0 when every test passes, 1 otherwise.

## Layout

```
tests/visual/
├── run_tests.py            desktop suite (this document)
├── run_all_tests.py        desktop suite + WASM suite + performance CSV
├── performance_history.csv written by run_all_tests.py
├── configs/
│   ├── warmup.fflame       rendered first to warm the GPU
│   ├── 2d/  3d/  escape/  sim/  solid/  tonemap/  variations/
├── baseline/               committed 160x120 thumbnails + baseline_manifest.json
│   └── wasm/               the WASM suite's baselines (exact-hash, 7 images)
├── current/                renders from the last run (gitignored)
│   └── wasm/
└── wasm/                   browser suite (Selenium); see wasm/README.md
```

### Categories

| category | configs | what it covers |
|---|---:|---|
| `2d` | 6 | 2D flames, subflames |
| `3d` | 15 | 3D flames and 3D variations |
| `escape` | 119 | escape-time fractals |
| `sim` | 105 | simulations |
| `solid` | 6 | solid rendering: hard, translucent, lit, shadows, depth effects |
| `tonemap` | 2 | tone curves |
| `variations` | 121 | smoke tests of individual variations and variation families |

374 configs in the categories. `configs/warmup.fflame` is rendered once
before the suite to absorb cold-start cost, and is then picked up by
discovery as a test of its own (`general-warmup`), so a full run reports
375 tests.

A test is named `<category>-<file stem>` (for example
`2d-simple-linear`). The category prefix is not cosmetic: two configs
once shared a stem across `3d/` and `variations/` and overwrote each
other's output and baseline.

## How a run works

1. **Discovery** — every `.fflame` under `configs/`, recursively. A
   config without `"deterministic_rng": true` is skipped with a warning.
2. **Render** — for each config:
   `FractalArtEditor export -i <config> -o current/<name>.png --width 800 --height 600 --iterations-per-thread 256`,
   with a 60-second timeout. The iterations-per-thread value is pinned
   because every baseline was recorded at 256.
3. **Compare** — against `baseline/<name>.png` (below).
4. **Throughput** — after all renders, flags any render whose
   iterations per second fall below the run's median divided by 12.
   Render time and iteration count come from the PNG's `RenderTime` and
   `Iterations` metadata, not from wall-clock time. Escape-time and
   simulation renders are excluded: their `Iterations` field is a
   formula iteration cap or a step count, not a sample count.

### The tolerance compare

Baselines are 160x120 thumbnails with no PNG metadata. Each render is
downscaled to the same size (Lanczos) and compared per channel on a
0-255 scale. A test passes when both hold:

- mean absolute difference ≤ **2.0**, and
- at most **1%** of pixels have a channel differing by more than **40**.

The reasons are recorded in the header of `run_tests.py`:

- **Small and metadata-free keeps baselines git-trackable.** Every
  export embeds the git hash, build date and render time; stripped, an
  unchanged baseline re-renders byte-identical and git stores nothing
  new.
- **A tolerance survives GPU and driver rounding** across machines.
- **The `solid-*` tests need it.** Solid rendering is not
  bit-reproducible by design (an in-batch depth race), so an exact hash
  would fail them on every run.
- **Downscaling averages per-pixel noise**, and the outlier fraction
  (rather than a bare maximum) keeps one sample landing a pixel over
  from failing a sparse render.

The constants are `BASELINE_SIZE`, `TOLERANCE_MEAN`, `TOLERANCE_MAX`,
`TOLERANCE_OUTLIER_FRAC` and `THROUGHPUT_FACTOR` at the top of
`run_tests.py`.

### Updating baselines

`--update-baseline` runs the suite, then walks every PNG in `current/`:

- a PNG whose baseline still passes the tolerance compare is left
  alone, so the baseline file stays byte-identical;
- anything else is written as a new thumbnail.

It then rewrites `baseline/baseline_manifest.json` (sizes, tolerances,
per-image hashes, git hash). It updates whatever is in `current/`, not
only what this run rendered: after a `--category` run, renders left
over from earlier runs of other categories are considered too.

Look at the changed images before committing them.

## Adding a test

1. Put a `.fflame` in the right category directory.
2. Set `"deterministic_rng": true`, or the suite skips it.
3. Run the category (`--category NAME`) and look at the render in
   `current/`.
4. Run with `--update-baseline` and commit the new baseline with the
   config.

## Running while the app is open

Windows locks a running executable, so `cargo build --release` fails
while the app is up, and the suite then tests the stale binary. Build
into a side tree and point the runner at it:

```bash
CARGO_TARGET_DIR=target-verify cargo build --release
python tests/visual/run_tests.py --binary target-verify/release/FractalArtEditor.exe
```

`target-verify` is gitignored. A running app contending for the GPU
shows up as a throughput failure rather than a pixel mismatch; repeat
the run before chasing it.

## WASM tests

`wasm/test_wasm.py` renders configs in Chrome through the web build's
`WasmApi` and compares exact pixel hashes against `baseline/wasm/`. It
is separate from this suite: see [wasm/README.md](wasm/README.md) and
the WASM section of [docs/TESTING-GUIDE.md](../../docs/TESTING-GUIDE.md).

## Related documentation

- [docs/TESTING-GUIDE.md](../../docs/TESTING-GUIDE.md) — every test, gate and benchmark
- [docs/RELEASE.md](../../docs/RELEASE.md) — where this suite sits in a release
- [docs/main/EXPORT.md](../../docs/main/EXPORT.md) — the CLI export and PNG metadata
- [docs/archive/projects/visual-regression-testing.md](../../docs/archive/projects/visual-regression-testing.md) — the original plan (archived)
