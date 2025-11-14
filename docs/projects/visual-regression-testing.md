# Visual Regression Testing System

**Status:** Planning
**Priority:** High
**Created:** 2025-11-14

## Problem Statement

We need automated testing to catch regressions in:
- **Image quality**: PNG exports must match reference images
- **Performance**: Rendering speed shouldn't degrade
- **Cross-platform**: Desktop app, CLI export, WASM must produce identical results
- **Rendering modes**: 2D, 3D, different tone mapping settings

**Recent near-miss:** PNG export brightness bug went undetected until manual testing.

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
- **Test**: Playwright/Puppeteer to capture canvas → PNG
- **Challenge**: Requires node.js environment

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
        """Calculate SHA256 hash of file."""
        sha256 = hashlib.sha256()
        with open(path, 'rb') as f:
            while chunk := f.read(8192):
                sha256.update(chunk)
        return sha256.hexdigest()

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

### Phase 1: Core Infrastructure (Week 1)
- [x] CLI export already exists ✓
- [ ] Create `tests/visual/` directory structure
- [ ] Write Python test orchestrator (`run_tests.py`)
- [ ] Create initial test manifest with 5 configs
- [ ] Generate baseline images

### Phase 2: Test Coverage (Week 2)
- [ ] Add 2D rendering tests (5 configs)
- [ ] Add 3D rendering tests (5 configs)
- [ ] Add tone mapping tests (5 configs)
- [ ] Add variation tests (26 configs)
- [ ] Add performance tests (3 configs)

### Phase 3: WASM Testing (Week 3)
- [ ] Set up Puppeteer/Playwright
- [ ] Create Node.js test runner
- [ ] Add WASM render capture
- [ ] Compare WASM vs desktop hashes

### Phase 4: Performance Benchmarking (Week 4)
- [ ] Add GPU render benchmarks to `benches/`
- [ ] Measure iterations/second for all test configs
- [ ] Create performance baseline
- [ ] Add regression detection (fail if >10% slower)

### Phase 5: CI/CD Integration (Week 5)
- [ ] Create GitHub Actions workflow
- [ ] Run on Linux, Windows, macOS
- [ ] Upload failure artifacts
- [ ] Add status badge to README

## Success Criteria

- [ ] 40+ test configurations covering all rendering modes
- [ ] Python script runs all tests in <5 minutes
- [ ] Any visual regression detected immediately (pixel-perfect comparison)
- [ ] Performance regression >10% triggers failure
- [ ] WASM build tested automatically
- [ ] CI runs on every commit
- [ ] All platforms produce identical SHA256 hashes

## Pixel-Perfect Comparison

**Challenge:** PNG exports may have minor compression differences across platforms.

**Solution 1: Compare raw pixel data**
```python
from PIL import Image
import numpy as np

def compare_images_exact(img1_path, img2_path):
    img1 = np.array(Image.open(img1_path))
    img2 = np.array(Image.open(img2_path))
    return np.array_equal(img1, img2)
```

**Solution 2: Perceptual hash (if exact match too strict)**
```python
import imagehash

def compare_images_perceptual(img1_path, img2_path, threshold=5):
    hash1 = imagehash.phash(Image.open(img1_path))
    hash2 = imagehash.phash(Image.open(img2_path))
    return hash1 - hash2 < threshold
```

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
