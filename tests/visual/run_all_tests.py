#!/usr/bin/env python3
"""
Unified visual regression and performance testing.

Runs both desktop CLI and WASM browser tests, then compares performance
metrics from PNG metadata between baseline and current runs.

Generates:
- Console report with pass/fail and performance comparison
- CSV file with historical performance data
"""

import subprocess
import sys
import json
import csv
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional
from dataclasses import dataclass
import argparse

try:
    from PIL import Image
    HAS_PILLOW = True
except ImportError:
    HAS_PILLOW = False
    print("Warning: PIL/Pillow not installed. Install with: pip install Pillow")


@dataclass
class PngMetadata:
    """Performance metadata extracted from PNG tEXt chunks."""
    name: str
    width: int
    height: int
    total_iterations: int
    render_time_ms: float
    iterations_per_second: float
    test_type: str  # "desktop" or "wasm"


@dataclass
class PerformanceComparison:
    """Comparison between baseline and current render."""
    name: str
    test_type: str
    baseline: Optional[PngMetadata]
    current: Optional[PngMetadata]
    time_delta_ms: float
    time_delta_percent: float
    throughput_delta_percent: float


class UnifiedTestRunner:
    def __init__(self):
        self.root = Path("tests/visual")
        self.baseline_dir = self.root / "baseline"
        self.current_dir = self.root / "current"
        self.wasm_baseline = self.root / "wasm/baseline"
        self.wasm_current = self.root / "wasm/current"

        self.results: List[PerformanceComparison] = []

    def run_all_tests(self) -> bool:
        """Run desktop and WASM tests."""
        print("=" * 70)
        print("Unified Visual Regression & Performance Testing")
        print("=" * 70)
        print()

        # Run desktop CLI tests
        print("Running Desktop CLI Tests...")
        print("-" * 70)
        desktop_success = self.run_desktop_tests()
        print()

        # Run WASM browser tests
        print("Running WASM Browser Tests...")
        print("-" * 70)
        wasm_success = self.run_wasm_tests()
        print()

        # Compare performance
        print("Comparing Performance (Baseline vs Current)...")
        print("-" * 70)
        self.compare_performance()
        print()

        return desktop_success and wasm_success

    def run_desktop_tests(self) -> bool:
        """Run tests/visual/run_tests.py"""
        try:
            result = subprocess.run(
                [sys.executable, "tests/visual/run_tests.py"],
                capture_output=True,
                text=True,
                cwd=Path.cwd()
            )

            # Print output
            print(result.stdout)
            if result.stderr:
                print(result.stderr, file=sys.stderr)

            return result.returncode == 0

        except Exception as e:
            print(f"❌ Failed to run desktop tests: {e}")
            return False

    def run_wasm_tests(self) -> bool:
        """Run tests/visual/wasm/test_wasm.py"""
        try:
            result = subprocess.run(
                [sys.executable, "tests/visual/wasm/test_wasm.py"],
                capture_output=True,
                text=True,
                cwd=Path.cwd()
            )

            # Print output
            print(result.stdout)
            if result.stderr:
                print(result.stderr, file=sys.stderr)

            return result.returncode == 0

        except Exception as e:
            print(f"❌ Failed to run WASM tests: {e}")
            return False

    def extract_png_metadata(self, png_path: Path) -> Optional[PngMetadata]:
        """Extract performance metadata from PNG tEXt chunks."""
        if not HAS_PILLOW:
            return None

        try:
            img = Image.open(png_path)
            metadata = img.info

            # Parse PNG tEXt chunks
            render_time_str = metadata.get('RenderTime', '0ms')
            render_time_ms = float(render_time_str.replace('ms', '').strip())

            total_iterations = int(metadata.get('Iterations', '0'))

            resolution = metadata.get('Resolution', '0x0')
            width, height = map(int, resolution.split('x'))

            iterations_per_second = 0
            if render_time_ms > 0:
                iterations_per_second = total_iterations / (render_time_ms / 1000.0)

            return PngMetadata(
                name=png_path.stem,
                width=width,
                height=height,
                total_iterations=total_iterations,
                render_time_ms=render_time_ms,
                iterations_per_second=iterations_per_second,
                test_type="unknown"  # Will be set by caller
            )

        except Exception as e:
            print(f"  Warning: Failed to read metadata from {png_path.name}: {e}")
            return None

    def compare_performance(self):
        """Compare baseline vs current performance for all tests."""
        # Desktop tests
        desktop_comparisons = self.compare_directory(
            self.baseline_dir,
            self.current_dir,
            "desktop"
        )

        # WASM tests
        wasm_comparisons = self.compare_directory(
            self.wasm_baseline,
            self.wasm_current,
            "wasm"
        )

        self.results = desktop_comparisons + wasm_comparisons

        # Print report
        self.print_performance_report()

        # Save to CSV
        self.save_to_csv()

    def compare_directory(
        self,
        baseline_dir: Path,
        current_dir: Path,
        test_type: str
    ) -> List[PerformanceComparison]:
        """Compare all PNGs in baseline vs current directory."""
        comparisons = []

        if not baseline_dir.exists():
            print(f"  No baseline directory: {baseline_dir}")
            return comparisons

        if not current_dir.exists():
            print(f"  No current directory: {current_dir}")
            return comparisons

        # Get all baseline PNGs
        baseline_pngs = {p.stem: p for p in baseline_dir.glob("*.png")}
        current_pngs = {p.stem: p for p in current_dir.glob("*.png")}

        # Compare each test
        all_tests = set(baseline_pngs.keys()) | set(current_pngs.keys())

        for name in sorted(all_tests):
            baseline_meta = None
            current_meta = None

            if name in baseline_pngs:
                baseline_meta = self.extract_png_metadata(baseline_pngs[name])
                if baseline_meta:
                    baseline_meta.test_type = test_type

            if name in current_pngs:
                current_meta = self.extract_png_metadata(current_pngs[name])
                if current_meta:
                    current_meta.test_type = test_type

            # Calculate deltas
            time_delta_ms = 0
            time_delta_percent = 0
            throughput_delta_percent = 0

            if baseline_meta and current_meta:
                time_delta_ms = current_meta.render_time_ms - baseline_meta.render_time_ms

                if baseline_meta.render_time_ms > 0:
                    time_delta_percent = (time_delta_ms / baseline_meta.render_time_ms) * 100

                if baseline_meta.iterations_per_second > 0:
                    throughput_delta = current_meta.iterations_per_second - baseline_meta.iterations_per_second
                    throughput_delta_percent = (throughput_delta / baseline_meta.iterations_per_second) * 100

            comparisons.append(PerformanceComparison(
                name=name,
                test_type=test_type,
                baseline=baseline_meta,
                current=current_meta,
                time_delta_ms=time_delta_ms,
                time_delta_percent=time_delta_percent,
                throughput_delta_percent=throughput_delta_percent
            ))

        return comparisons

    def print_performance_report(self):
        """Print performance comparison report."""
        if not self.results:
            print("No performance data to compare")
            return

        print()
        print("Performance Report (Baseline → Current)")
        print("=" * 70)

        # Group by test type
        desktop_results = [r for r in self.results if r.test_type == "desktop"]
        wasm_results = [r for r in self.results if r.test_type == "wasm"]

        for test_type, results in [("Desktop CLI", desktop_results), ("WASM Browser", wasm_results)]:
            if not results:
                continue

            print()
            print(f"{test_type} Tests:")
            print("-" * 70)
            print(f"{'Test':<30} {'Baseline':>12} {'Current':>12} {'Delta':>10} {'Change':>8}")
            print("-" * 70)

            for r in results:
                if r.baseline and r.current:
                    baseline_str = f"{r.baseline.render_time_ms:.1f}ms"
                    current_str = f"{r.current.render_time_ms:.1f}ms"
                    delta_str = f"{r.time_delta_ms:+.1f}ms"

                    # Color code the change
                    if abs(r.time_delta_percent) < 5:
                        change_str = f"{r.time_delta_percent:+.1f}%"  # Within 5% = no change
                    elif r.time_delta_percent < 0:
                        change_str = f"{r.time_delta_percent:+.1f}% ✅"  # Faster
                    else:
                        change_str = f"{r.time_delta_percent:+.1f}% ⚠️"  # Slower

                    print(f"{r.name:<30} {baseline_str:>12} {current_str:>12} {delta_str:>10} {change_str:>8}")

                elif r.baseline and not r.current:
                    print(f"{r.name:<30} {'[baseline]':>12} {'[missing]':>12} {'':>10} {'':>8}")
                elif not r.baseline and r.current:
                    print(f"{r.name:<30} {'[new]':>12} {f'{r.current.render_time_ms:.1f}ms':>12} {'':>10} {'':>8}")

        # Summary statistics
        valid_comparisons = [r for r in self.results if r.baseline and r.current]

        if valid_comparisons:
            avg_time_change = sum(r.time_delta_percent for r in valid_comparisons) / len(valid_comparisons)
            avg_throughput_change = sum(r.throughput_delta_percent for r in valid_comparisons) / len(valid_comparisons)

            print()
            print("Summary:")
            print(f"  Average time change:       {avg_time_change:+.1f}%")
            print(f"  Average throughput change: {avg_throughput_change:+.1f}%")
            print(f"  Tests compared:            {len(valid_comparisons)}")

    def save_to_csv(self):
        """Save performance data to CSV file."""
        csv_path = self.root / "performance_history.csv"
        csv_exists = csv_path.exists()

        timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')

        headers = [
            'Timestamp', 'TestName', 'TestType',
            'Baseline_Time_ms', 'Current_Time_ms', 'Delta_Time_ms', 'Delta_Percent',
            'Baseline_Throughput_Miter_s', 'Current_Throughput_Miter_s', 'Throughput_Delta_Percent',
            'Resolution', 'Total_Iterations'
        ]

        with open(csv_path, 'a', newline='', encoding='utf-8') as f:
            writer = csv.writer(f)

            if not csv_exists:
                writer.writerow(headers)

            for r in self.results:
                if r.baseline and r.current:
                    row = [
                        timestamp,
                        r.name,
                        r.test_type,
                        f"{r.baseline.render_time_ms:.2f}",
                        f"{r.current.render_time_ms:.2f}",
                        f"{r.time_delta_ms:.2f}",
                        f"{r.time_delta_percent:.2f}",
                        f"{r.baseline.iterations_per_second / 1e6:.2f}",
                        f"{r.current.iterations_per_second / 1e6:.2f}",
                        f"{r.throughput_delta_percent:.2f}",
                        f"{r.current.width}x{r.current.height}",
                        r.current.total_iterations,
                    ]
                    writer.writerow(row)

        print()
        print(f"Performance data saved to: {csv_path}")


def main():
    parser = argparse.ArgumentParser(
        description="Unified visual regression and performance testing"
    )
    parser.add_argument(
        "--skip-tests",
        action="store_true",
        help="Skip running tests, only analyze existing PNG metadata"
    )
    args = parser.parse_args()

    if not HAS_PILLOW:
        print("Error: PIL/Pillow required for metadata extraction")
        print("Install with: pip install Pillow")
        sys.exit(1)

    runner = UnifiedTestRunner()

    if args.skip_tests:
        print("Skipping test runs, analyzing existing results...")
        runner.compare_performance()
        success = True
    else:
        success = runner.run_all_tests()

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
