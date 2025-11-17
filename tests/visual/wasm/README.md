# WASM Visual Regression Tests

Tests the WASM build to ensure it produces identical output to the desktop build.

## Prerequisites

### 1. Install wasm-pack
```bash
cargo install wasm-pack
```

### 2. Install Python dependencies
```bash
pip install selenium pillow numpy
```

### 3. Install Chrome and chromedriver
- **Chrome**: https://www.google.com/chrome/
- **chromedriver**: Must match your Chrome version
  - Download from: https://chromedriver.chromium.org/
  - On Windows: Add to PATH or place in same directory as script
  - On macOS: `brew install chromedriver`
  - On Linux: `sudo apt install chromium-chromedriver`

## Running Tests

```bash
# Run all WASM tests
python tests/visual/wasm/test_wasm.py

# The script will:
# 1. Build WASM with wasm-pack (--release)
# 2. Start HTTP server on port 8080
# 3. Launch headless Chrome
# 4. Load test.html and run each test config
# 5. Compare with desktop baselines
```

## How It Works

1. **Build**: Runs `wasm-pack build --target web --release`
2. **Serve**: Starts Python HTTP server on port 8080
3. **Test Page**: Loads `test.html` which:
   - Initializes WASM module
   - Exposes `window.loadFractalConfig()` and `window.startRender()`
   - Renders fractal to canvas
   - Sets `window.renderComplete` when done
4. **Capture**: Selenium captures canvas as PNG via `toDataURL()`
5. **Compare**: Hashes pixel data and compares with desktop baselines

## Test Configs

Uses the same configs as desktop tests from `tests/visual/configs/`:
- 2d/
- 3d/
- tonemap/
- variations/

All configs must have `deterministic_rng: true` for reproducible results.

## Output

- `tests/visual/current/wasm/*.png` - Rendered output from WASM
- Compared against `tests/visual/baseline/*.png` (desktop baselines)

## Troubleshooting

**"wasm-pack not found"**
- Run: `cargo install wasm-pack`

**"Failed to launch Chrome"**
- Install Chrome and chromedriver
- Ensure chromedriver is in PATH and matches Chrome version

**"Render timeout"**
- Increase timeout in script (default 60s)
- Check browser console for errors (remove --headless to debug)

**"Hash mismatch"**
- WASM and desktop should produce identical output
- If mismatch persists, check for platform-specific rendering differences
- Verify WASM build is using same shader code as desktop
