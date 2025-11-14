# Visual Regression Testing

Automated visual regression and performance testing for the fractal flame renderer.

## Quick Start

```bash
# 1. Build the release binary first
cargo build --release

# 2. Run all visual tests
python tests/visual/run_tests.py

# 3. Update baselines after verifying changes are correct
python tests/visual/run_tests.py --update-baseline
```

## Requirements

- Python 3.7+
- **Recommended:** PIL/Pillow for pixel-perfect comparison
  ```bash
  pip install Pillow numpy
  ```
- Without PIL: Falls back to PNG file hash (less reliable due to compression differences)

## Directory Structure

```
tests/visual/
├── run_tests.py          # Test orchestrator script
├── configs/              # Test configurations (.fflame files)
│   ├── 2d/              # 2D rendering tests
│   ├── 3d/              # 3D rendering tests
│   ├── tonemap/         # Tone mapping tests
│   └── variations/      # Individual variation tests
├── current/             # Latest test outputs (auto-generated)
└── baseline/            # Reference images for comparison
```

## Test Configuration Requirements

**CRITICAL:** All test configs MUST have `"deterministic_rng": true` for reproducible results.

Fractal flames are fundamentally random without this flag - they will render differently every time, making pixel-perfect comparison impossible.

### Example Test Config

```json
{
  "flame": {
    "name": "Simple Linear",
    "transforms": [{
      "a": 0.5, "d": 0.5,
      "variations": {"linear": 1.0},
      "color": 0.5,
      "weight": 1.0
    }],
    "render_mode": "TwoD"
  },
  "max_iterations": 10000000,
  "deterministic_rng": true  // ← REQUIRED!
}
```

## Usage

### Run All Tests

```bash
python tests/visual/run_tests.py
```

### Run Specific Category

```bash
python tests/visual/run_tests.py --category 2d
python tests/visual/run_tests.py --category 3d
python tests/visual/run_tests.py --category tonemap
```

### Update Baselines

After verifying that visual changes are intentional:

```bash
python tests/visual/run_tests.py --update-baseline
```

This copies all current outputs to the baseline directory.

### Debug Mode

Use debug build instead of release (slower but shows more info):

```bash
python tests/visual/run_tests.py --debug
```

## How It Works

1. **Discovery**: Auto-discovers all `.fflame` files in `configs/` subdirectories
2. **Validation**: Checks that `deterministic_rng: true` is set (skips if missing)
3. **Execution**: Runs headless CLI export for each config
4. **Comparison**: Compares pixel data hash against baseline (or previous hash)
5. **Performance**: Checks render time doesn't exceed threshold
6. **Reporting**: Prints pass/fail for each test with performance metrics

## Test Output

```
Running 3 visual regression tests...
============================================================
[PASS]   simple-linear                  2500ms, 4.0M iter/s
[FAIL]   simple-zcone                   12000ms, 0.8M iter/s
         Performance regression: 12000ms > 10000ms limit
[PASS]   white-curve                    2800ms, 3.6M iter/s

============================================================
Tests: 2 passed, 1 failed, 3 total
```

## Adding New Tests

1. Create a `.fflame` config in appropriate category directory
2. **IMPORTANT:** Set `"deterministic_rng": true`
3. Run tests to generate output
4. Verify output visually
5. Update baseline: `python run_tests.py --update-baseline`

## Comparison Methods

### With PIL/Pillow (Recommended)

- Hashes raw pixel data (ignores PNG compression differences)
- Pixel-perfect comparison
- Cross-platform reliable

### Without PIL (Fallback)

- Hashes entire PNG file
- May have false failures due to compression differences
- Still useful for catching major rendering changes

## Performance Regression Detection

Default timeout: 10 seconds per test

Tests fail if:
- Render time > 10000ms (configurable in code)
- Output image differs from baseline
- Export process crashes or times out

## CI/CD Integration

The test suite is designed for automated CI/CD:

```yaml
# Example GitHub Actions
- name: Run visual regression tests
  run: |
    cargo build --release
    pip install Pillow numpy
    python tests/visual/run_tests.py
```

Exit codes:
- `0` - All tests passed
- `1` - One or more tests failed

## Troubleshooting

### "deterministic_rng: true - skipping"

The config is missing the `deterministic_rng` flag. Without this, renders are non-deterministic and can't be compared.

**Fix:** Add `"deterministic_rng": true` to the config JSON.

### Image mismatch on first run

This is normal - no baseline exists yet.

**Fix:** Run `--update-baseline` after verifying the output looks correct.

### Performance regression false positives

If hardware varies (CI vs local), adjust `max_render_time_ms` threshold in the script.

## Future Enhancements

- [ ] HTML report generation with visual diffs
- [ ] PNG metadata extraction for render time verification
- [ ] Parallel test execution
- [ ] WASM testing via Puppeteer
- [ ] Cross-platform baseline comparison
- [ ] Perceptual hash fallback for minor floating-point differences

## Related Documentation

- [docs/projects/visual-regression-testing.md](../../docs/projects/visual-regression-testing.md) - Full testing plan
- [docs/TESTING-GUIDE.md](../../docs/TESTING-GUIDE.md) - General testing guide
- [docs/main/EXPORT.md](../../docs/main/EXPORT.md) - PNG export documentation
