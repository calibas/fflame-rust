# WASM Visual Regression Tests

Renders the visual-suite configs in Chrome through the web app's
`WasmApi.export_png` and compares exact pixel hashes against WASM
baselines of its own (WASM hashes differ from desktop's).

## Prerequisites

### 1. The web app's build tools
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.121   # must match Cargo.lock
```
See [docs/WASM.md](../../../docs/WASM.md) for why that version.

### 2. Python dependencies
```bash
pip install selenium pillow numpy
```

### 3. Chrome and chromedriver
- **Chrome**: https://www.google.com/chrome/
- **chromedriver**: must match your Chrome version
  - Download from: https://chromedriver.chromium.org/
  - On Windows: add to PATH or place in the same directory as the script
  - On macOS: `brew install chromedriver`
  - On Linux: `sudo apt install chromium-chromedriver`

## Running Tests

```bash
python tests/visual/wasm/test_wasm.py                    # every category
python tests/visual/wasm/test_wasm.py --category 2d      # one category
python tests/visual/wasm/test_wasm.py --update-baseline  # write baselines from this run
```

The script:
1. Builds the web app with `build-wasm.bat` on Windows, `build-wasm.sh`
   elsewhere (`--profile dist`, a few minutes)
2. Serves the repository root on port 8080 (`python -m http.server`,
   with the interpreter running the script)
3. Launches Chrome with WebGPU enabled (not headless; ANGLE's D3D11
   backend on Windows)
4. Loads `test.html` for each config, which calls
   `WasmApi.export_png(800, 600, 256, false)`
5. Hashes the PNG's pixels and compares them with the baseline

## Test Configs

The desktop suite's configs in `tests/visual/configs/`, from `2d/`,
`3d/`, `tonemap/`, `variations/`, `escape/` and `sim/`. `solid/` is
left out on purpose: solid rendering is not bit-reproducible (an
in-batch depth race), and this suite compares exact hashes.

All configs must have `deterministic_rng: true`; one without is skipped
with a warning.

## Output and baselines

- `tests/visual/current/wasm/<category>-<stem>.png` — this run's renders
  (gitignored). Each test deletes its old output before rendering.
- `tests/visual/baseline/wasm/<category>-<stem>.png` — the baselines.
  Names carry the category, as in `run_tests.py`: `3d/` and
  `variations/` both have an `affine3d-smoke`.

Each test reports `[PASS]`, `[FAIL]` or `[NONE]` — no baseline, so
nothing was compared. The summary counts the three separately, and a
run with any `[NONE]` exits 1 unless `--update-baseline` wrote them.
`--update-baseline` writes baselines only for configs that rendered in
this run. Only a handful of configs have WASM baselines so far.

## Troubleshooting

**"build-wasm.bat failed" / "build-wasm.sh failed"**
- The last lines of the build output follow the message; run the
  script by hand to see all of it.

**"Failed to launch Chrome"**
- Install Chrome and chromedriver
- Ensure chromedriver is in PATH and matches the Chrome version

**"Render timeout"**
- The timeout is 10 minutes per test (`CONFIG['timeout']`); the
  browser console logs for the render are printed after each test

**"Hash mismatch"**
- Baselines are exact pixel hashes from one machine's GPU and browser;
  a different GPU, driver or Chrome version can change them. Compare
  the two PNGs before treating it as a regression.
