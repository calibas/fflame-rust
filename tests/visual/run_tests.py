#!/usr/bin/env python3
"""
Visual regression and performance testing for fractal flame renderer.

Tests all build configurations and compares:
- Image quality (pixel-perfect comparison via deterministic RNG)
- Rendering performance (iterations/second)
- Cross-platform consistency

IMPORTANT: All test configs MUST have "deterministic_rng": true for reproducible results.
"""

import subprocess
import json
import hashlib
import time
import shutil
from pathlib import Path
from dataclasses import dataclass
from typing import List, Dict, Optional
import argparse
import sys

try:
    from PIL import Image
    import numpy as np
    HAS_PILLOW = True
except ImportError:
    HAS_PILLOW = False
    print("Warning: PIL/Pillow not installed. Install with: pip install Pillow numpy")
    print("Falling back to PNG file hash comparison (less reliable)")


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
    def __init__(self, binary_path: Optional[Path] = None, use_release: bool = True):
        self.use_release = use_release
        self.configs_dir = Path("tests/visual/configs")
        self.current_dir = Path("tests/visual/current")
        self.baseline_dir = Path("tests/visual/baseline")
        self.results: List[TestResult] = []

        # Determine binary path
        if binary_path:
            self.binary = binary_path
        else:
            profile = "release" if use_release else "debug"
            exe_name = "fractal_flame_wgpu.exe" if sys.platform == "win32" else "fractal_flame_wgpu"
            self.binary = Path(f"target/{profile}/{exe_name}")

        # Ensure directories exist
        self.current_dir.mkdir(parents=True, exist_ok=True)
        self.baseline_dir.mkdir(parents=True, exist_ok=True)

    def gpu_warmup(self):
        """
        Run a warmup render to avoid 20-40% slowdown on first GPU render.

        The first GPU render is often significantly slower due to cold start effects:
        - Shader compilation caching
        - GPU driver initialization
        - Memory allocation patterns

        This warmup ensures consistent performance measurements across all tests.
        """
        warmup_config = self.configs_dir / "warmup.fflame"

        if not warmup_config.exists():
            print("Note: No warmup.fflame found - skipping GPU warmup")
            return

        print("GPU Warmup: Running warmup render...")

        warmup_output = self.current_dir / "warmup.png"

        cmd = [
            str(self.binary),
            "export",
            "-i", str(warmup_config),
            "-o", str(warmup_output),
        ]

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=30,
                cwd=Path.cwd()
            )

            if result.returncode == 0:
                print("GPU Warmup: Complete [OK]\n")
                # Delete warmup output - we don't need it
                if warmup_output.exists():
                    warmup_output.unlink()
            else:
                print(f"GPU Warmup: Failed (will continue anyway): {result.stderr[:100]}\n")
        except Exception as e:
            print(f"GPU Warmup: Error (will continue anyway): {str(e)}\n")

    def run_all_tests(self, category_filter: Optional[str] = None, skip_warmup: bool = False) -> bool:
        """Run all test configurations and compare results."""
        # GPU warmup to avoid 20-40% slowdown on first render
        if not skip_warmup:
            self.gpu_warmup()

        configs = self.discover_test_configs(category_filter)

        if not configs:
            print(f"No test configs found in {self.configs_dir}")
            return False

        print(f"Running {len(configs)} visual regression tests...")
        print("=" * 60)

        for config in configs:
            result = self.run_single_test(config)
            self.results.append(result)
            self.print_result(result)

        return all(r.passed for r in self.results)

    def run_single_test(self, config: TestConfig) -> TestResult:
        """Run a single test configuration."""
        output_path = self.current_dir / f"{config.name}.png"
        baseline_path = self.baseline_dir / f"{config.name}.png"

        # Build command using binary directly (avoids cargo warnings)
        cmd = [
            str(self.binary),
            "export",
            "-i", str(config.config_file),
            "-o", str(output_path),
        ]

        # Run CLI export
        start = time.time()
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=60,
                cwd=Path.cwd()
            )

            if result.returncode != 0:
                return TestResult(
                    name=config.name,
                    passed=False,
                    actual_sha256="",
                    expected_sha256=config.reference_sha256,
                    render_time_ms=0,
                    iterations_per_second=0,
                    error=f"Export failed (exit {result.returncode}): {result.stderr[:200]}"
                )
        except subprocess.TimeoutExpired:
            return TestResult(
                name=config.name,
                passed=False,
                actual_sha256="",
                expected_sha256=config.reference_sha256,
                render_time_ms=0,
                iterations_per_second=0,
                error="Export timed out (>60s)"
            )
        except Exception as e:
            return TestResult(
                name=config.name,
                passed=False,
                actual_sha256="",
                expected_sha256=config.reference_sha256,
                render_time_ms=0,
                iterations_per_second=0,
                error=f"Unexpected error: {str(e)}"
            )

        total_time_ms = (time.time() - start) * 1000

        # Check if output exists
        if not output_path.exists():
            return TestResult(
                name=config.name,
                passed=False,
                actual_sha256="",
                expected_sha256=config.reference_sha256,
                render_time_ms=total_time_ms,
                iterations_per_second=0,
                error="Output PNG not created"
            )

        # Calculate SHA256 of output (pixel data if PIL available, else file)
        actual_sha256 = self.hash_image(output_path)

        # Read metadata from PNG to get actual render time and iterations
        # (Future: implement PNG metadata reading)
        render_time_ms = total_time_ms
        total_iterations = config.expected_iterations
        iterations_per_second = total_iterations / (render_time_ms / 1000) if render_time_ms > 0 else 0

        # Compare against baseline
        passed = True
        error = None

        # If baseline exists, compare
        if baseline_path.exists():
            baseline_sha256 = self.hash_image(baseline_path)
            if actual_sha256 != baseline_sha256:
                passed = False
                error = f"Image mismatch: baseline {baseline_sha256[:8]}..., got {actual_sha256[:8]}..."
        elif config.reference_sha256:
            # Compare against expected hash
            if actual_sha256 != config.reference_sha256:
                passed = False
                error = f"Image mismatch: expected {config.reference_sha256[:8]}..., got {actual_sha256[:8]}..."

        # Check performance regression
        if render_time_ms > config.max_render_time_ms:
            if passed:
                error = f"Performance regression: {render_time_ms:.0f}ms > {config.max_render_time_ms:.0f}ms limit"
            else:
                error += f" | Perf: {render_time_ms:.0f}ms > {config.max_render_time_ms:.0f}ms"
            passed = False

        return TestResult(
            name=config.name,
            passed=passed,
            actual_sha256=actual_sha256,
            expected_sha256=config.reference_sha256 or (baseline_sha256 if baseline_path.exists() else None),
            render_time_ms=render_time_ms,
            iterations_per_second=iterations_per_second,
            error=error
        )

    def hash_image(self, path: Path) -> str:
        """
        Calculate SHA256 hash of image pixel data (if PIL available) or file.

        This ignores PNG compression differences and only compares
        actual rendered pixels when PIL is available.
        """
        if HAS_PILLOW:
            try:
                img = np.array(Image.open(path))
                return hashlib.sha256(img.tobytes()).hexdigest()
            except Exception as e:
                print(f"Warning: Failed to read image pixels for {path.name}: {e}")
                # Fall back to file hash

        # Fallback: hash the file itself
        sha256 = hashlib.sha256()
        with open(path, 'rb') as f:
            while chunk := f.read(8192):
                sha256.update(chunk)
        return sha256.hexdigest()

    def discover_test_configs(self, category_filter: Optional[str] = None) -> List[TestConfig]:
        """Auto-discover .fflame files in configs directory."""
        configs = []

        # Search for all .fflame files
        for fflame_path in self.configs_dir.rglob("*.fflame"):
            # Get category from parent directory name
            category = fflame_path.parent.name if fflame_path.parent != self.configs_dir else "general"

            # Apply category filter
            if category_filter and category != category_filter:
                continue

            # Read config to get max_iterations
            try:
                with open(fflame_path) as f:
                    config_data = json.load(f)
                    max_iterations = config_data.get("max_iterations", 10_000_000)

                    # Verify deterministic_rng is enabled
                    if not config_data.get("deterministic_rng", False):
                        print(f"Warning: {fflame_path.name} missing deterministic_rng: true - skipping")
                        continue
            except Exception as e:
                print(f"Warning: Failed to read {fflame_path}: {e}")
                max_iterations = 10_000_000

            # Create test config with generous time limits
            configs.append(TestConfig(
                name=fflame_path.stem,
                config_file=fflame_path,
                category=category,
                expected_iterations=max_iterations,
                max_render_time_ms=10000  # 10 seconds max per test
            ))

        return sorted(configs, key=lambda c: (c.category, c.name))

    def update_baselines(self):
        """Copy current outputs to baseline directory."""
        print("\nUpdating baseline images...")
        count = 0
        for png in self.current_dir.glob("*.png"):
            baseline = self.baseline_dir / png.name
            shutil.copy(png, baseline)
            print(f"  {png.name} -> baseline")
            count += 1
        print(f"Updated {count} baseline images")

    def print_result(self, result: TestResult):
        """Print test result."""
        status = "[PASS]" if result.passed else "[FAIL]"
        perf = f"{result.render_time_ms:.0f}ms, {result.iterations_per_second/1e6:.1f}M iter/s"
        print(f"{status:8} {result.name:30} {perf}")
        if result.error:
            print(f"         {result.error}")

    def print_summary(self):
        """Print test summary."""
        passed = sum(1 for r in self.results if r.passed)
        failed = len(self.results) - passed

        print("\n" + "=" * 60)
        print(f"Tests: {passed} passed, {failed} failed, {len(self.results)} total")

        if failed > 0:
            print("\nFailed tests:")
            for r in self.results:
                if not r.passed:
                    print(f"  - {r.name}: {r.error}")

        return failed == 0


def main():
    parser = argparse.ArgumentParser(
        description="Visual regression testing for fractal flame renderer",
        epilog="Example: python run_tests.py --update-baseline"
    )
    parser.add_argument(
        "--update-baseline",
        action="store_true",
        help="Update baseline images from current run"
    )
    parser.add_argument(
        "--category",
        choices=["2d", "3d", "tonemap", "variations"],
        help="Run only tests in this category"
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Use debug build instead of release"
    )
    parser.add_argument(
        "--skip-warmup",
        action="store_true",
        help="Skip GPU warmup (may result in slower first test)"
    )
    args = parser.parse_args()

    if not HAS_PILLOW:
        print("\nNote: Running without PIL/Pillow - using file hash comparison")
        print("For pixel-perfect comparison, install: pip install Pillow numpy\n")

    runner = VisualTestRunner(use_release=not args.debug)

    if args.update_baseline:
        print("Running tests and updating baselines...\n")
        success = runner.run_all_tests(args.category, skip_warmup=args.skip_warmup)
        runner.print_summary()
        # Always update baselines when explicitly requested
        runner.update_baselines()
    else:
        success = runner.run_all_tests(args.category, skip_warmup=args.skip_warmup)
        runner.print_summary()

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
