# Testing Guide

Every test, gate, benchmark and diagnostic tool in the repository, what
each one covers, and how to run it.

**Nothing runs automatically.** There is no CI, on purpose:
`scripts/release.py check` does what CI would do, when you type it. See
[RELEASE.md §7](RELEASE.md) for the reasoning and §2 for what a release
requires.

## Quick command reference

```bash
# Unit tests (~1,900 in the library; GPU tests skip or are #[ignore]d)
cargo test --release --lib

# Every fast gate: versions, tests, wasm build, generated files, doc
# links, the five gallery crates (also the engine feature matrix)
python scripts/release.py check
python scripts/release.py check --fix      # ...and regenerate stale generated files

# Visual regression suite (needs a GPU; does NOT build the binary)
cargo build --release && python tests/visual/run_tests.py

# Benchmarks: Criterion + desktop GPU renders (+ WASM unless --quick)
python scripts/run_benchmarks.py --quick

# Variation math probe (per-platform shader arithmetic report)
cargo run --release --bin variation_probe

# Render configs to PNG through the library, without the release exe
INSPECT="a.fflame=a.png" INSPECT_SIZE=1280x720 \
  cargo test --release --lib render_configs_for_inspection -- --ignored --nocapture
```

---

## Per-engine tests

Most of what follows is engine-agnostic. Three things are worth knowing
before you go looking for a simulation or escape failure:

- **GPU tests need `--test-threads=1`.** The heavier simulation tests
  live in `src/sim/app_repro_test.rs` and replay the app's exact frame
  call order against a fresh renderer; several are `#[ignore]`d and run
  deliberately. The escape engine has the same kind of file,
  `src/escape/app_repro_test.rs`, and nearly all of its tests are
  `#[ignore = "needs a GPU"]`.
- **The preset probes are the simulation's real safety net, and one of
  them does not run by default.** `every_preset_draws_something`
  renders every shipped preset and fails any whose image is flat, but
  it needs a GPU and is `#[ignore]`d — run it deliberately after
  touching a model or a preset:

  ```bash
  cargo test --release every_preset_draws_something -- --ignored --test-threads=1
  ```

  It has caught a model rendering black that no name check could. See
  [SIMULATION.md](main/SIMULATION.md) ("Adding a model").
- **`release.py check` compiles the library at each single-engine
  feature combination** (`engine-flame`, `engine-escape`, `engine-sim`
  alone). Those are what the WASM gallery modules link, and a `#[cfg]`
  attached to the wrong item compiles fine in every other build.

Engine-specific testing notes live with each engine:
[ESCAPE.md](main/ESCAPE.md), [SIMULATION.md](main/SIMULATION.md),
[TERRAIN.md](main/TERRAIN.md), [FOCUSED-RENDERING.md](main/FOCUSED-RENDERING.md).

---

## 1. Unit tests

**Where:** `#[cfg(test)]` modules inside the source files under `src/`,
plus one integration file, `tests/regression.rs` (12 tests: transform
defaults, core variations registered, presets load and validate, config
serialization, multi-config files).

**Scale:** the library holds 1,927 tests, of which 370 are `#[ignore]`d
(`cargo test --release --lib -- --list` and `-- --list --ignored` count
them).

```bash
cargo test --release --lib                  # the library (what release.py runs)
cargo test --release --lib palette          # tests whose path contains "palette"
cargo test --release --test regression      # the integration file
cargo test --release --lib some_test -- --nocapture   # see println! output
```

Release mode is the normal way to run them: many tests render, and a
debug build is far slower.

### GPU tests

Tests that touch the GPU follow a few conventions.

- **Two ways to need a GPU.** A test that runs by default asks a local
  helper for a device and **returns early when there is no adapter**:
  `fn device() -> Option<(Device, Queue)>`, used as
  `let Some((device, queue)) = device() else { return; }` (for example
  `src/renderer/sticky.rs`, `src/renderer/compute_kernel.rs`,
  `src/escape/terrain.rs`'s `gpu_tests::device`). So on a machine
  without a GPU the suite passes without exercising them. The heavier
  ones are instead `#[ignore = "needs a GPU"]` and call
  `device().expect("gpu")`; they run only when asked.
- **One at a time, where it matters.** `cargo test` runs tests in
  parallel. Tests that share global state or need a lot of memory take
  a static `Mutex<()>` first:
  - `gpu()` in `src/escape/footprint.rs` hands out a device together
    with a turn — each terrain test holds an atlas of hundreds of MB,
    and two at once ran a 6 GB card out of memory. The turn orders
    only the terrain tests, so nothing in the suite may hold most of
    the card: `fit_export_sections` does not probe the card's memory
    under `cfg(test)` (its probe took a 6 GB card to 5.8 GB, and the
    tests beside it ran out of memory or crashed the driver with
    `STATUS_ACCESS_VIOLATION`); its real probe is the ignored
    `an_exports_section_cap_fits_the_card`, run alone;
  - `DIAG_LOCK` in `src/escape/app_repro_test.rs` guards the global
    escape diagnostics snapshot between a render and its assertion;
  - `BREAKER_LOCK` in `src/escape/renderer.rs`.

  The locks recover from poisoning (`lock().unwrap_or_else(|e|
  e.into_inner())`), so one failing test does not fail every test
  queued behind it. Running ignored GPU tests with `--test-threads=1`
  is still the safe default.
- **`--ignored` is a menu, not a dustbin.** The ignore reason says what
  a test is for: `"needs a GPU"`, `"prints a measurement"`,
  `"a survey; run with --ignored --nocapture"`, `"diagnostic"`,
  `"reads output/flame-zoom"` (local files that the tests marked
  `"writes output/flame-zoom"`, in `src/scene/cylinder.rs`, produce).
  Measurements and surveys assert little and print a lot; run them
  with `--nocapture`.

  ```bash
  cargo test --release --lib some_measurement -- --ignored --nocapture --test-threads=1
  ```

### Rendering a config for inspection (INSPECT)

`render_configs_for_inspection` (`src/sim/terrain.rs`, ignored) renders
config files through the same `renderer::render` the CLI export uses —
for when the release executable is locked by a running app:

```bash
INSPECT="a.fflame=a.png;b.fflame=b.png" INSPECT_SIZE=1280x720 \
  cargo test --release --lib render_configs_for_inspection -- --ignored --nocapture
```

`INSPECT` is a `;`-separated list of `input=output` pairs;
`INSPECT_SIZE` defaults to 1920x1080. Write the outputs under `output/`
(gitignored).

---

## 2. Release gates — `scripts/release.py check`

The fast gates, run in this order from the repository root. Each prints
pass or FAIL with its time; any failure exits 1.

| gate | command |
|---|---|
| versions agree | `python scripts/release.py versions` (run in-process) |
| unit tests | `cargo test --release --lib` |
| wasm builds | `cargo check --release --target wasm32-unknown-unknown --lib` |
| generated contract | `cargo test --release --lib contract_is_current` |
| shader dumps | `cargo test --release --lib canonical_shader_dumps` |
| doc links | `python scripts/check_doc_links.py` |
| gallery script parity | `cargo test --profile gallery-test`, in `wasm/script` |
| gallery renderer smoke | `cargo test --profile gallery-test`, in `wasm/render` |
| gallery sim smoke | the same, in `wasm/sim` (the library with only `engine-sim`) |
| gallery escape smoke | the same, in `wasm/escape` (only `engine-escape`) |
| gallery flame smoke | the same, in `wasm/flame` (only `engine-flame`) |

The last three are the engine feature matrix: each single-engine crate
links the library with `default-features` off and one engine on, so
building its tests catches a `#[cfg]` on the wrong item that every
all-engines build hides, and the tests assert the other engines are
really gone. The gallery gates run from the crate directory, where
cargo finds `wasm/.cargo/config.toml` ([RELEASE.md §5](RELEASE.md)).

`--fix` regenerates the two generated files below when their gate
fails, then tells you to read the diff. No other gate has a fix.

What `check` does not run, because each needs a GPU and minutes: the
visual suite (§3), the benchmarks (§5) and the variation probe (§6).
The other `release.py` subcommands (`changelog`, `version`, `build`) are
described in [RELEASE.md](RELEASE.md).

### Generated files

Two committed files are compared against what the code produces now.
Regenerate deliberately, and read the diff: the point of the gate is
that somebody looks.

| file | test | regenerate |
|---|---|---|
| `docs/generated/engine-contract.json` (what the API reads) | `contract_is_current` (`src/contract.rs`) | `UPDATE_CONTRACT=1 cargo test --lib contract_is_current` |
| `tests/shader_dumps/*.wgsl` (8 canonical flame shaders: in-app vs export, 2D/3D, solid, multi-emit, path/xaos) | `canonical_shader_dumps` (`src/shader_dumps.rs`) | `UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps` |

The shader dumps make codegen visible in review: a commit that changes
the generated WGSL carries the before and after next to the source
change. See [SHADERS.md](main/SHADERS.md).

### Doc links

`scripts/check_doc_links.py` finds Markdown links to `.md` files that do
not exist, in `docs/**/*.md` and `CLAUDE.md`. Broken links inside
`docs/archive/` are counted but do not fail the check (an archived doc
records what it said at the time); a broken link in a live doc does.

---

## 3. Visual regression suite

**Where:** `tests/visual/run_tests.py`; full detail in
[tests/visual/README.md](../tests/visual/README.md).

```bash
cargo build --release                       # run_tests.py does not build
python tests/visual/run_tests.py            # everything (~375 renders)
python tests/visual/run_tests.py --category escape
python tests/visual/run_tests.py --update-baseline
```

- **Renders** each config under `tests/visual/configs/` through the
  binary's CLI export (`target/release/FractalArtEditor(.exe)` by
  default, `--binary` to override) at 800x600 and 256 iterations per
  thread.
- **Categories** (subdirectories of `configs/`): `2d` (6), `3d` (15),
  `escape` (119), `sim` (105), `solid` (6), `tonemap` (2),
  `variations` (121) — 374 configs, plus `configs/warmup.fflame`, which
  is rendered first as a warmup and is then also run as the test
  `general-warmup`.
- **Compares with a tolerance, not a hash.** Baselines in
  `tests/visual/baseline/` are 160x120 metadata-free thumbnails; each
  render is downscaled and passes when the mean channel difference is
  at most 2.0 and at most 1% of pixels differ by more than 40 (0-255
  scale). Small thumbnails keep the baselines git-trackable, the
  tolerance survives GPU and driver rounding, and the `solid-*` tests
  need it because solid rendering is not bit-reproducible (an in-batch
  depth race). The header of `run_tests.py` has the full reasoning.
- **Throughput:** a render more than 12x below the run's median
  iterations per second fails. Escape and simulation renders are left
  out of that check (their `Iterations` metadata is not a sample count).
- **Updating:** `--update-baseline` runs the suite, then rewrites only
  the baselines whose renders moved beyond the tolerance; the rest stay
  byte-identical. It considers only the renders this run produced (each
  test deletes its old output first), so a `--category` run touches only
  that category, and it merges them into `baseline_manifest.json`.
- **Every config must set `"deterministic_rng": true`**, or it is
  skipped.

**Running the suite while the app is open.** Windows locks a running
executable, so `cargo build --release` fails with `Access is denied`
whenever the app is up — and a suite run then silently tests the
STALE binary. Build into a side tree and point the runner at it:

```bash
CARGO_TARGET_DIR=target-verify cargo build --release
python tests/visual/run_tests.py --binary target-verify/release/FractalArtEditor.exe
```

`target-verify` is gitignored. The first build there is a full one;
after that it is incremental like any other tree.

One symptom worth recognising: contending with a running app for the
GPU shows up as a THROUGHPUT failure (`below N Miter/s floor`) rather
than a pixel mismatch. Treat a lone throughput failure as a measurement
to repeat, not a regression to chase.

`tests/visual/run_all_tests.py` runs this suite and the WASM suite (§4)
one after the other, compares each render's time with the previous
run's, and appends every render's timing to
`tests/visual/performance_history.csv`, which is where the previous run
is read from. (It used to read timings from the baselines; the desktop
ones are metadata-stripped thumbnails, so every desktop delta was 0%.)

---

## 4. WASM browser tests

**Where:** `tests/visual/wasm/`. Selenium driving Chrome (needs
`chromedriver` on PATH, and `pip install selenium Pillow numpy`); see
[tests/visual/wasm/README.md](../tests/visual/wasm/README.md).

```bash
python tests/visual/wasm/test_wasm.py
python tests/visual/wasm/test_wasm.py --category 2d
python tests/visual/wasm/test_wasm.py --update-baseline
```

`test_wasm.py`:

1. builds the web app with `build-wasm.bat` on Windows and
   `build-wasm.sh` elsewhere;
2. serves the repository root on port 8080 (with the running Python)
   and opens `tests/visual/wasm/test.html` in a Chrome window with
   WebGPU flags (ANGLE's D3D11 backend on Windows only);
3. for each config in the `2d`, `3d`, `tonemap`, `variations`, `escape`
   and `sim` categories, calls `WasmApi.load_config_json` and
   `await WasmApi.export_png(800, 600, 256, false)`
   ([WASM.md](WASM.md) lists the API). `solid` is left out: solid
   rendering is not bit-reproducible, and this suite compares exact
   hashes;
4. names each test `<category>-<stem>`, as `run_tests.py` does, writes
   `tests/visual/current/wasm/<name>.png` (deleting the old one first)
   and compares an **exact** SHA-256 of the pixels against
   `tests/visual/baseline/wasm/<name>.png`. Seven baselines are
   committed. A config with no baseline is reported as `[NONE]` and
   counted as "no baseline", not as a pass, and the run exits 1 unless
   `--update-baseline` wrote it; `--update-baseline` writes baselines
   only for the configs that rendered in that run.

`test_plan.py` is a separate gate for Focused Rendering's planner in
the browser: it drives `PlanBench` (`src/wasm_api.rs`) one step per
animation frame from `tests/visual/wasm/plan.html` and fails any step
over 16 ms. Build first (`./build-wasm.sh` or `build-wasm.bat`).

Neither runs in `release.py check`. The check's `wasm builds` gate only
compiles the library for `wasm32-unknown-unknown`.

---

## 5. Benchmarks — `scripts/run_benchmarks.py`

```bash
python scripts/run_benchmarks.py              # CPU + desktop GPU + WASM
python scripts/run_benchmarks.py --quick      # skip WASM, Criterion --quick, 3 runs per config
python scripts/run_benchmarks.py --wasm-only  # only the WASM stage
python scripts/run_benchmarks.py --no-save    # do not write the CSV or last_run.json
```

Three stages:

1. **CPU microbenchmarks** — `cargo bench --bench flame_bench`
   (Criterion; `benches/flame_bench.rs`). These measure CPU-side
   helpers only — affine maps, point helpers (r, θ, φ), colour
   blending, a preset's iteration step, variation-weight lookup,
   registry lookups, transform creation. Variation math runs on the
   GPU and is not here.
2. **Desktop GPU renders** — runs `cargo build --release` itself, then
   renders every config in `tests/visual/configs/` with
   `cargo run --release -- export ... --width 800 --height 600 --iterations-per-thread 1024`,
   5 runs each (3 with `--quick`), the first discarded as warmup. Times
   come from each PNG's `RenderTime` metadata. Output:
   `benchmark_results/renders/<category>-<stem>.png` (gitignored), the
   names `run_tests.py` uses; the benchmark rows carry the same names.
3. **WASM renders** — runs `tests/visual/wasm/test_wasm.py` repeatedly
   and reads the times from the PNGs it writes to
   `tests/visual/current/wasm/`. Its exit status is not used — it exits
   1 for any config without a WASM baseline — only whether it rendered.
   A failure here is a warning, not a failed run.

The report compares each benchmark with the previous run
(`benchmark_results/last_run.json`) and the two previous full runs in
`benchmark_results/unified_benchmarks.csv`, colouring more than 10%
slower red, more than 5% slower yellow and more than 2% faster green.

**The hash table is informational.** The report ends with one table of
exact pixel hashes: the WASM renders against `tests/visual/baseline/wasm/`,
the one pair rendered the same way. It does not affect the exit status.
There is no desktop table: the benchmark renders at ipt 1024 and full
size, while the visual suite's baselines are 160x120 thumbnails of
ipt-256 renders, so an exact match was impossible (the Baseline-vs-Current
and Desktop-vs-WASM tables it used to print could only ever mismatch).
Use `tests/visual/run_tests.py` for regressions.

The benchmark names became `<category>-<stem>` on 2026-10-11 (they were
bare stems, and the two `affine3d-smoke` configs shared one), so the
previous-run columns show "—" until two runs with the new names exist.

### Other measurement binaries

| binary | what |
|---|---|
| `cargo run --release --bin simple_benchmark` | quick CPU timing of the presets, the CPU-side variation helpers and affine maps |
| `cargo run --release --bin shader_bench` | shader-compilation measurements behind sticky compilation ([record](archive/projects/sticky-shader-compilation.md)) |
| `cargo run --release --bin compare_images -- --image1 a.png --image2 b.png [--output diff.png] [--threshold N]` | pixel diff of two PNGs, with their metadata |

### Frame cost in the app, and the governor's calibration

For the quality governor ([quality-governor.md](projects/quality-governor.md)).
All of these write to `output/governor/`; run them with the machine
otherwise idle, since the times are real.

| what | how |
|---|---|
| **The frame log**: a CSV row a frame from the running app (CPU time, the GPU's time for the UI and for the engine, the view, playback, the flame's dispatch, escape supersampling, the simulation's grid) | `FFLAME_FRAME_LOG=out.csv` when starting the release app; `FFLAME_FRAME_LOG_OPEN=x.anim` (played) or `x.fflame`, and `FFLAME_FRAME_LOG_SECONDS=N` to quit, for a scripted run. Settings beside it in `out.csv.json` |
| **Reference playback**: eight looped scenes made from visual-suite configs, each played in the app with the frame log on | `python scripts/governor_scenes.py`, then `python scripts/governor_playback.py [--warm] [--seconds N] [--only a,b]`. Opens a window per scene |
| **The calibration**: each scene at each rung of the governor's ladder (escape supersampling and size, flame samples and size, a simulation step's cost at each field scale), timed | `cargo test --release --lib -- --ignored governor_calibration --nocapture`; `CALIBRATE_SIZE`, `CALIBRATE_ONLY`, `CALIBRATE_REPEATS`, `CALIBRATE_OUT` |
| **Loss, flicker, summaries** | `python scripts/frame_metrics.py loss` (each rung against its reference, upscaled by bilinear, bicubic and Lanczos; writes `table.md`), `flicker DIR` (a folder of frames), `summary LOG.csv` |

---

## 6. Variation math probe and reachability census

Both live in one binary, `src/bin/variation_probe.rs`, and write
committed reports under `docs/generated/`. They exist because a visual
test can say a picture changed but not which variation, at which input,
computed what — the class of bug where Metal's fast-math returns a
plausible wrong value. The rules they back are in
[SHADERS.md](main/SHADERS.md).

```bash
cargo run --release --bin variation_probe                  # probe -> docs/generated/variation-probe.txt (+ -sweep.txt)
cargo run --release --bin variation_probe -- --no-sweep    # skip the parameter sweep
cargo run --release --bin variation_probe -- compare OLD NEW
cargo run --release --bin variation_probe -- census flame.fflame [iterations]
cargo run --release --bin variation_probe -- census --corpus [iterations] [seeds]
cargo run --release --bin variation_probe -- rank OLD NEW [census-report]
```

- **The probe** (`src/probe/`) evaluates every variation's shader
  arithmetic at a fixed adversarial input grid (signed zeros, values
  around the 1e32 bad-value threshold, subnormals), in 2D and 3D, and
  writes `variation-probe.txt`. The sweep companion,
  `variation-probe-sweep.txt`, moves each parameter one at a time.
  The first run on a machine pays cold driver compiles (minutes).
- **`compare OLD NEW`** diffs two reports. It reports value **classes**
  (zero, finite, NaN, infinite, past-threshold), which no rounding
  difference can change, separately from quantised magnitudes. It exits
  1 only for a class change or a variation present in one report alone;
  magnitude-only differences are printed and exit 0.
- **The census** (`src/census/`) instruments the generated
  `apply_variations` dispatcher and counts which input classes real
  renders actually feed each variation. `census <file>` measures one
  config (default 100M iterations); `census --corpus` sweeps a corpus
  (defaults: 50M iterations, 100 seeds) and writes
  `docs/generated/variation-census-<os>.txt` — per platform, because it
  is a measurement of that GPU. `cargo run --release --bin
  census_dumpseed -- N` prints the config behind a corpus entry
  `random:N`.
- **`rank`** joins two probe reports with the census into a worklist.
  `docs/accepted-divergences.txt` records what has been examined and
  deliberately left alone.

Design records: [variation-math-probe.md](archive/projects/variation-math-probe.md),
[variation-reachability-census.md](archive/projects/variation-reachability-census.md).

### What a variation parameter costs

`renderer::param_cost` times every whole-number parameter, and every
one named like a count, at its default and at its maximum, with the
variation alone in a small flame. It reports what the maximum adds to an
app frame. A parameter that adds 15 ms or more belongs in
`variations::SLOW_PARAMS` (UI.md, *Slow settings*). The dispatches are
small, so no maximum comes near a GPU watchdog; the first figures are
noisy until the GPU has clocked up, so run it twice and compare.

```bash
cargo test --release --lib -- --ignored --nocapture measure_variation_param_costs
PARAM_COST_ONLY=glsl_ cargo test --release --lib -- --ignored --nocapture measure_variation_param_costs
```

---

## 7. Profiling

- **In the app:** the Performance panel (`src/ui/performance.rs`) shows
  version, build profile, FPS, frame time and its range, the per-pass
  timings (compute, accumulate, tonemap, UI, submit, present),
  accumulated frames and total iterations. The metrics live in
  `PerformanceMetrics` (`src/util.rs`).
- **GPU timestamps:** `GpuProfiler` in `src/profiler.rs` wraps
  timestamp queries, but nothing in the app uses it; wiring it in is
  manual.
- **In the browser:** the developer tools' Performance tab; the old
  guide is archived as [WASM-PROFILING.md](archive/WASM-PROFILING.md).

---

## Troubleshooting

```bash
RUST_BACKTRACE=1 cargo test --release --lib failing_test -- --nocapture
cargo test --release --lib some::module::path::exact_test -- --exact
```

- **A GPU test fails only when run with others:** rerun it alone and
  with `--test-threads=1`; see the locks in §1.
- **The visual suite reports a whole category off at once:** check that
  the binary is current (`run_tests.py` does not build), and that
  nothing else is using the GPU.
- **Criterion numbers are noisy:** close other applications;
  `cargo bench -- --sample-size 200` or `--warm-up-time 5` for more
  stable runs.

---

## Files reference

| what | where | run |
|---|---|---|
| unit tests | `src/**/*.rs` | `cargo test --release --lib` |
| integration tests | `tests/regression.rs` | `cargo test --release --test regression` |
| release gates | `scripts/release.py` | `python scripts/release.py check` |
| doc links | `scripts/check_doc_links.py` | `python scripts/check_doc_links.py` |
| engine contract | `src/contract.rs` → `docs/generated/engine-contract.json` | `UPDATE_CONTRACT=1 cargo test --lib contract_is_current` |
| shader dumps | `src/shader_dumps.rs` → `tests/shader_dumps/` | `UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps` |
| visual suite | `tests/visual/run_tests.py` | `python tests/visual/run_tests.py` |
| visual + WASM + CSV | `tests/visual/run_all_tests.py` | `python tests/visual/run_all_tests.py` |
| WASM suite | `tests/visual/wasm/test_wasm.py` | `python tests/visual/wasm/test_wasm.py` |
| WASM planner gate | `tests/visual/wasm/test_plan.py` | `python tests/visual/wasm/test_plan.py` |
| benchmarks | `scripts/run_benchmarks.py`, `benches/flame_bench.rs` | `python scripts/run_benchmarks.py` |
| variation probe / census | `src/bin/variation_probe.rs`, `src/probe/`, `src/census/` | `cargo run --release --bin variation_probe` |
| gallery module tests | `wasm/*/tests/` | `cargo test --profile gallery-test` in the crate directory |
| CLI export | `src/main.rs`, `src/app/export.rs` | `cargo run --release -- export -i config.fflame -o out.png` |
| image diff | `src/bin/compare_images.rs` | `cargo run --release --bin compare_images -- --image1 a.png --image2 b.png` |
