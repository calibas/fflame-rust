# Testing Rendering Determinism

This guide shows how to test if the fractal flame renderer produces identical output for the same inputs.

## Quick Start

1. **Run the app** and load a preset (e.g., "Simple")
2. **Export to PNG** twice with identical settings:
   - Settings → Export PNG (with background)
   - Save as `test1.png`
   - Close and reopen the app
   - Load the same preset
   - Export again as `test2.png`

3. **Compare the images:**
   ```bash
   cargo run --release --bin compare_images -- -1 test1.png -2 test2.png -o diff.png
   ```

## Expected Results

### Deterministic Rendering
If rendering is deterministic, you should see:
```
✓ Images are identical!
```

### Non-Deterministic Rendering
If there are differences, you'll see statistics:
```
Results:
  Total pixel difference: 15234
  Average difference per pixel: 0.15
  Maximum pixel difference: 8
  Different pixels (threshold 0): 10234 (0.52%)

⚠ Images have minor differences (< 1% different)
```

The difference image (`diff.png`) will show where pixels differ (amplified 10x for visibility).

## Testing Scenarios

### 1. Test Basic Determinism
```bash
# Export the same preset twice and compare
cargo run --release --bin compare_images -- -1 export1.png -2 export2.png
```

### 2. Test Tone Curve Bug
Export with and without tone curves to see the saturation issue:

**Without tone curve:**
1. Disable tone curve
2. Export as `no_curve.png`

**With linear tone curve:**
1. Enable tone curve
2. Select "Linear" preset (should be identity function)
3. Export as `linear_curve.png`

**Compare:**
```bash
cargo run --release --bin compare_images -- -1 no_curve.png -2 linear_curve.png -o curve_diff.png --amplify 20
```

If there's a bug, you'll see differences even though the linear curve should be identical to no curve.

### 3. Test with Threshold
Ignore tiny differences (e.g., rounding errors):
```bash
cargo run --release --bin compare_images -- -1 test1.png -2 test2.png --threshold 1
```

## Comparison Tool Options

```
compare_images --help

Options:
  -1, --image1 <IMAGE1>        First image path
  -2, --image2 <IMAGE2>        Second image path
  -o, --output <OUTPUT>        Output difference image (optional)
  -t, --threshold <THRESHOLD>  Threshold for considering pixels different (0-255) [default: 0]
  -a, --amplify <AMPLIFY>      Amplify differences in output image [default: 10]
```

## Exit Codes

- **0**: Images are identical (or within threshold)
- **1**: Images differ

This makes it easy to use in automated testing:
```bash
if cargo run --release --bin compare_images -- -1 a.png -2 b.png; then
    echo "Test passed!"
else
    echo "Test failed - images differ"
    exit 1
fi
```

## Known Limitations

1. **RNG Determinism**: The renderer uses `rand::random()` which may not be deterministic across runs
   - The Julia variation specifically uses non-seeded RNG
   - This may cause minor pixel differences

2. **GPU Precision**: Different GPUs may have slight precision differences in floating-point math

3. **Accumulation Order**: Frame-to-frame accumulation may introduce minor variations

## Next Steps

Once determinism is confirmed (or RNG is fixed), this tool enables:
- **Regression testing**: Detect unintended visual changes from code modifications
- **Bug investigation**: Quantify exact pixel differences for debugging
- **Performance testing**: Compare output quality at different iteration counts
