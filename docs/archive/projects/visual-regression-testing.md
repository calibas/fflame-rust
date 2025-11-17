# Visual Regression Testing System

**Status:** Phase 1 Complete, Phase 3 Blocked (2025-11-15)
**Priority:** High
**Created:** 2025-11-14

## Current Status (2025-11-15)

**✅ Phase 1 Complete - Core infrastructure working!**

- Python test orchestrator with 8 passing tests
- GPU warmup prevents first-render slowdown
- Pixel-perfect comparison via deterministic RNG
- Baseline management with --update-baseline
- Performance metrics (iterations/second)
- Ready to catch PNG export bugs automatically

**⏸️ Phase 3 Blocked - WASM Testing**

- **Blocker**: WASM build doesn't expose test APIs (runs full UI app, not headless renderer)
- Created scaffolding (`test_wasm.py`, `test.html`, `IMPLEMENTATION_NOTES.md`)
- Requires wasm_bindgen exports for headless rendering
- See `tests/visual/wasm/IMPLEMENTATION_NOTES.md` for required changes
- **Recommendation**: Defer to later phase after expanding desktop test coverage

**Next Steps:**
- Phase 2: Add more test configs (target: 40+) - prioritize this
- Phase 4: GPU benchmarks (Criterion.rs)
- Phase 3 (deferred): WASM testing (requires code changes)
- Phase 5: CI/CD integration (GitHub Actions)

See [tests/visual/README.md](../../tests/visual/README.md) for usage instructions.

## Problem Statement

We need automated testing to catch regressions in:
- **Image quality**: PNG exports must match reference images
- **Performance**: Rendering speed shouldn't degrade
- **Cross-platform**: Desktop app, CLI export, WASM must produce identical results
- **Rendering modes**: 2D, 3D, different tone mapping settings

**Recent near-miss:** PNG export brightness bug went undetected until manual testing.

**Critical Constraint:** Fractal flames are fundamentally random - they won't render identically unless `deterministic_rng: true` is set in the config. All visual regression test configs MUST have deterministic RNG enabled.

## Goals

1. **Detect visual regressions** - Any change in rendered output triggers test failure
2. **Measure performance impact** - Quantify speed changes from code modifications
3. **Cross-platform parity** - Ensure all 5 build targets produce identical results
4. **Automated CI/CD** - Run tests on every commit

## Test Targets

### 1. Desktop App (Windows/macOS/Linux)
- **Method**: Render to internal texture, export PNG
- **Test**: `cargo run --release -- export -i config.fflame -o output.png`
- **Platforms**: Windows 10/11, macOS (Intel/ARM), Ubuntu 22.04

### 2. Headless CLI Export
- **Method**: Same code path as desktop export
- **Test**: `cargo run --release -- export -i tests/visual/configs -o tests/visual/current`
- **Already implemented**: Yes ✓

### 3. WASM Build
- **Method**: Browser-based rendering with headless browser
- **Test**: Selenium to capture canvas → PNG
- **Implementation**: Python-based (no Node.js) to keep dependencies consistent

### 4. Benchmarks
- **Method**: Criterion.rs for CPU code, custom GPU timing
- **Test**: `cargo bench`
- **Already implemented**: Partial (CPU only)

### 5. macOS (if available)
- **Method**: Same as desktop
- **Test**: CI runner on macOS (GitHub Actions)

## Architecture

### Python Test Orchestrator

**File:** `tests/visual/run_tests.py`

```python
#!/usr/bin/env python3
"""
Visual regression and performance testing for fractal flame renderer.

Tests all build configurations and compares:
- Image quality (pixel-perfect comparison)
- Rendering performance (iterations/second)
- Cross-platform consistency
"""

import subprocess
import json
import hashlib
from pathlib import Path
from dataclasses import dataclass
from typing import List, Dict, Optional
import argparse

@dataclass
class TestConfig:
    name: str
    config_file: Path
    category: str  # "2d", "3d", "tonemap", "variations"
    expected_iterations: int
    max_render_time_ms: float
    reference_sha256: Optional[str] = None

@dataclass
class TestResult:
    name: str
    passed: bool
    actual_sha256: str
    expected_sha256: Optional[str]
    render_time_ms: float
    iterations_per_second: float
    error: Optional[str] = None

class VisualTestRunner:
    def __init__(self, cargo_bin: str = "cargo"):
        self.cargo_bin = cargo_bin
        self.configs_dir = Path("tests/visual/configs")
        self.current_dir = Path("tests/visual/current")
        self.baseline_dir = Path("tests/visual/baseline")
        self.results: List[TestResult] = []

    def run_all_tests(self) -> bool:
        """Run all test configurations and compare results."""
        configs = self.load_test_configs()

        print(f"Running {len(configs)} visual regression tests...")

        for config in configs:
            result = self.run_single_test(config)
            self.results.append(result)
            self.print_result(result)

        return all(r.passed for r in self.results)

    def run_single_test(self, config: TestConfig) -> TestResult:
        """Run a single test configuration."""
        output_path = self.current_dir / f"{config.name}.png"

        # Run CLI export
        start = time.time()
        try:
            result = subprocess.run([
                self.cargo_bin, "run", "--release", "--",
                "export",
                "-i", str(config.config_file),
                "-o", str(output_path),
                "--category", config.category
            ], capture_output=True, text=True, timeout=30)

            if result.returncode != 0:
                return TestResult(
                    name=config.name,
                    passed=False,
                    actual_sha256="",
                    expected_sha256=config.reference_sha256,
                    render_time_ms=0,
                    iterations_per_second=0,
                    error=f"Export failed: {result.stderr}"
                )
        except subprocess.TimeoutExpired:
            return TestResult(
                name=config.name,
                passed=False,
                actual_sha256="",
                expected_sha256=config.reference_sha256,
                render_time_ms=0,
                iterations_per_second=0,
                error="Export timed out (>30s)"
            )

        render_time_ms = (time.time() - start) * 1000

        # Calculate SHA256 of output
        actual_sha256 = self.hash_file(output_path)

        # Read metadata from PNG to get actual render time and iterations
        metadata = self.read_png_metadata(output_path)
        if metadata:
            render_time_ms = metadata.get("render_time_ms", render_time_ms)
            total_iterations = metadata.get("total_iterations", config.expected_iterations)
            iterations_per_second = total_iterations / (render_time_ms / 1000)
        else:
            iterations_per_second = 0

        # Compare against baseline
        passed = True
        error = None

        if config.reference_sha256:
            if actual_sha256 != config.reference_sha256:
                passed = False
                error = f"Image mismatch: expected {config.reference_sha256[:8]}..., got {actual_sha256[:8]}..."

        if render_time_ms > config.max_render_time_ms:
            passed = False
            error = f"Performance regression: {render_time_ms:.1f}ms > {config.max_render_time_ms:.1f}ms limit"

        return TestResult(
            name=config.name,
            passed=passed,
            actual_sha256=actual_sha256,
            expected_sha256=config.reference_sha256,
            render_time_ms=render_time_ms,
            iterations_per_second=iterations_per_second,
            error=error
        )

    def hash_file(self, path: Path) -> str:
        """
        Calculate SHA256 hash of raw pixel data (not PNG file).

        This ignores PNG compression differences and only compares
        actual rendered pixels. Requires deterministic_rng: true in config.
        """
        from PIL import Image
        import numpy as np

        img = np.array(Image.open(path))
        return hashlib.sha256(img.tobytes()).hexdigest()

    def read_png_metadata(self, path: Path) -> Optional[Dict]:
        """Extract metadata from PNG tEXt chunks."""
        # Use Python PNG library to read metadata
        import png
        try:
            reader = png.Reader(filename=str(path))
            # Extract tEXt chunks
            # Return parsed metadata dict
            pass
        except:
            return None

    def load_test_configs(self) -> List[TestConfig]:
        """Load test configurations from manifest."""
        manifest = self.configs_dir / "test_manifest.json"
        if manifest.exists():
            with open(manifest) as f:
                data = json.load(f)
                return [TestConfig(**cfg) for cfg in data["tests"]]
        else:
            # Auto-discover .fflame files
            configs = []
            for fflame in self.configs_dir.rglob("*.fflame"):
                category = fflame.parent.name if fflame.parent != self.configs_dir else "general"
                configs.append(TestConfig(
                    name=fflame.stem,
                    config_file=fflame,
                    category=category,
                    expected_iterations=10_000_000,
                    max_render_time_ms=5000
                ))
            return configs

    def update_baselines(self):
        """Copy current outputs to baseline directory."""
        print("Updating baseline images...")
        for png in self.current_dir.glob("*.png"):
            baseline = self.baseline_dir / png.name
            shutil.copy(png, baseline)
            print(f"  {png.name} -> baseline")

    def print_result(self, result: TestResult):
        """Print test result."""
        status = "✓ PASS" if result.passed else "✗ FAIL"
        print(f"{status} {result.name}: {result.render_time_ms:.1f}ms, {result.iterations_per_second:.1f} iter/s")
        if result.error:
            print(f"      {result.error}")

    def print_summary(self):
        """Print test summary."""
        passed = sum(1 for r in self.results if r.passed)
        failed = len(self.results) - passed

        print("\n" + "="*60)
        print(f"Tests: {passed} passed, {failed} failed, {len(self.results)} total")

        if failed > 0:
            print("\nFailed tests:")
            for r in self.results:
                if not r.passed:
                    print(f"  - {r.name}: {r.error}")

def main():
    parser = argparse.ArgumentParser(description="Visual regression testing")
    parser.add_argument("--update-baseline", action="store_true", help="Update baseline images")
    parser.add_argument("--category", help="Run only tests in this category")
    args = parser.parse_args()

    runner = VisualTestRunner()

    if args.update_baseline:
        runner.run_all_tests()
        runner.update_baselines()
    else:
        success = runner.run_all_tests()
        runner.print_summary()
        exit(0 if success else 1)

if __name__ == "__main__":
    main()
```

### Test Manifest

**File:** `tests/visual/configs/test_manifest.json`

```json
{
  "version": 1,
  "tests": [
    {
      "name": "simple-2d",
      "config_file": "tests/visual/configs/2d/simple.fflame",
      "category": "2d",
      "expected_iterations": 10000000,
      "max_render_time_ms": 1000,
      "reference_sha256": "abc123..."
    },
    {
      "name": "discus-3d",
      "config_file": "tests/visual/configs/3d/discus3.fflame",
      "category": "3d",
      "expected_iterations": 10000000,
      "max_render_time_ms": 1500,
      "reference_sha256": "def456..."
    },
    {
      "name": "tonemap-white",
      "config_file": "tests/visual/configs/tonemap/tcwhite.fflame",
      "category": "tonemap",
      "expected_iterations": 10000000,
      "max_render_time_ms": 1000,
      "reference_sha256": "789abc..."
    }
  ]
}
```

## Test Categories

### 1. 2D Rendering Tests
**Directory:** `tests/visual/configs/2d/`

- `simple.fflame` - Basic 2D flame (linear + sinusoidal)
- `all-variations.fflame` - One of each 2D variation
- `multi-transform.fflame` - 5+ transforms
- `high-variation-count.fflame` - 15+ active variations
- `palette-test.fflame` - Palette color accuracy

**IMPORTANT:** All test configs must have `"deterministic_rng": true` to ensure reproducible results.

### 2. 3D Rendering Tests
**Directory:** `tests/visual/configs/3d/`

- `simple-3d.fflame` - Basic 3D with zcone
- `hemisphere.fflame` - Full 3D structure
- `rotation.fflame` - Pre/post rotation variations
- `perspective.fflame` - Perspective projection
- `orthographic.fflame` - Orthographic projection

### 3. Tone Mapping Tests
**Directory:** `tests/visual/configs/tonemap/`

- `tcwhite.fflame` - Should produce all white (curve test)
- `tcblack.fflame` - Should produce all black (curve test)
- `exposure.fflame` - Exposure parameter accuracy
- `gamma.fflame` - Gamma correction accuracy
- `brightness.fflame` - Brightness scaling

### 4. Variation Tests
**Directory:** `tests/visual/configs/variations/`

- One config per variation to test correctness
- `linear.fflame`, `sinusoidal.fflame`, ..., `blob.fflame`

### 5. Performance Tests
**Directory:** `tests/visual/configs/performance/`

- `low-iterations.fflame` - 1M iterations (fast)
- `medium-iterations.fflame` - 10M iterations (baseline)
- `high-iterations.fflame` - 100M iterations (quality)
- `many-workgroups.fflame` - Stress test GPU parallelism

## Performance Benchmarking

### GPU Rendering Benchmarks

**File:** `benches/gpu_bench.rs`

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fractal_flame_wgpu::*;

fn bench_full_render_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_render");

    // Test different iteration counts
    for iterations in [1_000_000, 10_000_000, 100_000_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(iterations),
            &iterations,
            |b, &iterations| {
                // Setup GPU, load config
                let (device, queue) = setup_gpu();
                let config = load_test_config("simple.fflame");
                let renderer = FlameRenderer::new(&device, &queue, ...);

                b.iter(|| {
                    // Render to completion
                    render_to_png(&device, &queue, &renderer, &config, iterations)
                });
            }
        );
    }

    group.finish();
}

fn bench_compute_pass(c: &mut Criterion) {
    // Benchmark just the compute shader (iteration generation)
    // Measure iterations/second
}

fn bench_accumulate_pass(c: &mut Criterion) {
    // Benchmark accumulation pass in isolation
}

fn bench_tonemap_pass(c: &mut Criterion) {
    // Benchmark tone mapping pass
}

criterion_group!(benches,
    bench_full_render_pipeline,
    bench_compute_pass,
    bench_accumulate_pass,
    bench_tonemap_pass
);
criterion_main!(benches);
```

### WASM Testing

**File:** `tests/wasm/test_wasm_render.js`

```javascript
const puppeteer = require('puppeteer');
const fs = require('fs');
const crypto = require('crypto');

async function testWasmRender(configName) {
    const browser = await puppeteer.launch();
    const page = await browser.newPage();

    // Load WASM build
    await page.goto('http://localhost:8080/index.html');

    // Wait for WASM to load
    await page.waitForFunction(() => window.wasmReady === true);

    // Load test config
    const config = fs.readFileSync(`tests/visual/configs/${configName}.fflame`, 'utf8');
    await page.evaluate((cfg) => {
        window.loadConfig(cfg);
    }, config);

    // Wait for rendering to complete
    await page.waitForFunction(() => window.renderComplete === true, {timeout: 30000});

    // Capture canvas to PNG
    const canvasElement = await page.$('canvas');
    const screenshot = await canvasElement.screenshot({type: 'png'});

    // Calculate hash
    const hash = crypto.createHash('sha256').update(screenshot).digest('hex');

    await browser.close();

    return {
        hash: hash,
        size: screenshot.length
    };
}

// Run all WASM tests
async function main() {
    const configs = ['simple', 'discus3', 'tcwhite'];

    for (const config of configs) {
        console.log(`Testing ${config}...`);
        const result = await testWasmRender(config);
        console.log(`  Hash: ${result.hash}`);
    }
}

main();
```

## CI/CD Integration

### GitHub Actions Workflow

**File:** `.github/workflows/visual-regression.yml`

```yaml
name: Visual Regression Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  visual-tests-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-python@v4
        with:
          python-version: '3.10'

      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libvulkan1 mesa-vulkan-drivers
          pip install pypng pillow

      - name: Build release
        run: cargo build --release

      - name: Run visual regression tests
        run: python tests/visual/run_tests.py

      - name: Upload artifacts
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: visual-test-failures-linux
          path: tests/visual/current/*.png

  visual-tests-windows:
    runs-on: windows-latest
    # Same structure as Linux

  visual-tests-macos:
    runs-on: macos-latest
    # Same structure as Linux

  performance-benchmarks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Run benchmarks
        run: cargo bench

      - name: Compare with baseline
        run: |
          # Compare criterion output with previous run
          # Fail if performance degrades >10%
```

## Implementation Plan

### Phase 1: Core Infrastructure ✅ COMPLETE (2025-11-14)
- [x] CLI export already exists ✓
- [x] Create `tests/visual/` directory structure ✓
- [x] Write Python test orchestrator (`run_tests.py`) ✓
- [x] Create initial test configs (8 configs) ✓
- [x] Generate baseline images ✓
- [x] **BONUS:** GPU warmup system implemented ✓
- [x] **BONUS:** Deterministic RNG validation ✓
- [x] **BONUS:** Pixel-perfect comparison via PIL/Pillow ✓

### Phase 2: Test Coverage (IN PROGRESS)
- [x] Add 2D rendering tests (1 config - need 4 more)
- [x] Add 3D rendering tests (2 configs - need 3 more)
- [x] Add tone mapping tests (2 configs - need 3 more)
- [x] Add variation tests (2 configs - need 24 more)
- [x] Add warmup test (1 config)
- [ ] Add performance comparison tests (need 3 configs)
- [ ] Reach 40+ total test configs

### Phase 3: WASM Testing (FUTURE)
- [ ] Set up Puppeteer/Playwright
- [ ] Create Node.js test runner
- [ ] Add WASM render capture
- [ ] Compare WASM vs desktop hashes

### Phase 4: Performance Benchmarking (FUTURE)
- [x] Basic performance measurement in test script (iterations/second) ✓
- [ ] Add GPU render benchmarks to `benches/` (Criterion.rs)
- [ ] Create performance baseline tracking
- [ ] Add regression detection (fail if >10% slower)
- [ ] Track performance trends over time

### Phase 5: CI/CD Integration (FUTURE)
- [ ] Create GitHub Actions workflow
- [ ] Run on Linux, Windows, macOS
- [ ] Upload failure artifacts
- [ ] Add status badge to README
- [ ] Auto-update baselines on approved PRs

## Success Criteria

**Phase 1 (Current Status):**
- [x] Python test orchestrator working ✓
- [x] Pixel-perfect comparison (SHA256 of raw pixels) ✓
- [x] Baseline management (--update-baseline) ✓
- [x] GPU warmup for consistent timing ✓
- [x] Deterministic RNG validation ✓
- [x] 8 working test configs ✓
- [x] All tests pass ✓
- [x] Tests run in <15 seconds ✓

**Future Goals:**
- [ ] 40+ test configurations covering all rendering modes
- [ ] Performance regression >10% triggers failure
- [ ] WASM build tested automatically
- [ ] CI runs on every commit
- [ ] All platforms produce identical SHA256 hashes

## Pixel-Perfect Comparison

**Prerequisites:**
1. All test configs MUST have `"deterministic_rng": true` in the JSON
2. This ensures the RNG seed is fixed and renders are reproducible
3. Without this, random variation in iteration paths makes pixel-perfect comparison impossible

**Challenge:** PNG exports may have minor compression differences across platforms.

**Solution 1: Compare raw pixel data (preferred)**
```python
from PIL import Image
import numpy as np

def compare_images_exact(img1_path, img2_path):
    """
    Pixel-perfect comparison of two PNG images.

    Requires deterministic_rng: true in config!
    """
    img1 = np.array(Image.open(img1_path))
    img2 = np.array(Image.open(img2_path))
    return np.array_equal(img1, img2)
```

**Solution 2: SHA256 hash of pixel data**
```python
def hash_image_pixels(img_path):
    """
    Hash the raw pixel data, not the PNG file.
    This ignores compression differences.
    """
    img = np.array(Image.open(img_path))
    return hashlib.sha256(img.tobytes()).hexdigest()
```

**Solution 3: Perceptual hash (fallback if exact match too strict)**
```python
import imagehash

def compare_images_perceptual(img1_path, img2_path, threshold=5):
    """
    Use perceptual hash if deterministic RNG still has minor differences.
    Threshold of 5 allows very small variations.
    """
    hash1 = imagehash.phash(Image.open(img1_path))
    hash2 = imagehash.phash(Image.open(img2_path))
    return hash1 - hash2 < threshold
```

**Decision:** Start with pixel-perfect comparison (Solution 1). If we discover platform-specific floating-point differences, fall back to perceptual hashing.

## Detecting Rendering Changes

### What Should Trigger Test Updates?

**Intentional changes** (update baseline):
- Adding new variations
- Improving rendering algorithm
- Fixing visual bugs

**Unintentional changes** (test failure):
- Refactoring breaks rendering
- Performance regression
- Platform-specific bugs

### Update Baseline Workflow
```bash
# Run tests and update baselines
python tests/visual/run_tests.py --update-baseline

# Commit new baselines
git add tests/visual/baseline/*.png
git commit -m "TEST: Update visual baselines after rendering improvements"
```

## Future Enhancements

- **Diff images**: Generate visual diffs showing changed pixels
- **HTML report**: Web page showing all test results with images
- **Parallel execution**: Run tests in parallel for speed
- **GPU profiling**: Measure shader execution time
- **Memory usage**: Track GPU memory consumption
- **Cross-platform hashing**: Ensure deterministic PNG encoding

## Related Documentation

- `docs/TESTING-GUIDE.md` - Current testing documentation
- `tests/visual/` - Test configurations and baselines (to be created)
- `.github/workflows/` - CI/CD workflows (to be created)

---

**Next Steps:**
1. Review this plan
2. Create directory structure
3. Write Python orchestrator
4. Generate initial baselines
5. Add to CI/CD
