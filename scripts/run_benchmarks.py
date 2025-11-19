#!/usr/bin/env python3
"""
Unified Performance Benchmark Suite

Runs all performance tests and generates a comprehensive report:
1. CPU microbenchmarks (Criterion - cargo bench)
2. GPU rendering tests (Desktop CLI export)
3. GPU rendering tests (WASM browser)

Outputs:
- Console report with color-coded results
- Unified CSV history (benchmark_results/unified_benchmarks.csv)
- Baseline cache for regression detection (benchmark_results/last_run.json)

Usage:
    python scripts/run_benchmarks.py [--quick]

    --quick: Skip WASM tests and use shorter Criterion runs
"""

import argparse
import json
import subprocess
import sys
import csv
import re
import platform
import io
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass, asdict

# Fix Windows console encoding for Unicode
if platform.system() == 'Windows':
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

try:
    from PIL import Image
    HAS_PILLOW = True
except ImportError:
    HAS_PILLOW = False
    print("⚠️  Warning: PIL/Pillow not installed. PNG metadata extraction will be limited.")
    print("   Install with: pip install Pillow")


# ANSI color codes for terminal output
class Colors:
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKCYAN = '\033[96m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'

    @staticmethod
    def disable():
        """Disable colors on Windows if not supported."""
        if platform.system() == 'Windows':
            Colors.HEADER = ''
            Colors.OKBLUE = ''
            Colors.OKCYAN = ''
            Colors.OKGREEN = ''
            Colors.WARNING = ''
            Colors.FAIL = ''
            Colors.ENDC = ''
            Colors.BOLD = ''


@dataclass
class CriterionBenchmark:
    """Single Criterion benchmark result."""
    name: str
    mean_ns: float
    stddev_ns: float
    throughput_ops_sec: float


@dataclass
class RenderBenchmark:
    """Single rendering benchmark result."""
    name: str
    test_type: str  # "desktop" or "wasm"
    width: int
    height: int
    total_iterations: int
    render_time_ms: float
    throughput_miter_sec: float


@dataclass
class BenchmarkRun:
    """Complete benchmark run data."""
    timestamp: str
    platform: str
    git_commit: str
    git_branch: str
    rustc_version: str
    build_profile: str

    # CPU benchmarks
    cpu_benchmarks: List[CriterionBenchmark]

    # GPU benchmarks
    render_benchmarks: List[RenderBenchmark]

    # Summary stats
    total_tests: int
    passed_tests: int
    failed_tests: int


class UnifiedBenchmarkRunner:
    def __init__(self, quick_mode: bool = False):
        self.quick_mode = quick_mode
        self.root = Path.cwd()
        self.results_dir = self.root / "benchmark_results"
        self.results_dir.mkdir(exist_ok=True)

        self.visual_dir = self.root / "tests" / "visual"
        self.configs_dir = self.visual_dir / "configs"
        self.current_dir = self.visual_dir / "current"
        self.baseline_dir = self.visual_dir / "baseline"

        self.csv_path = self.results_dir / "unified_benchmarks.csv"
        self.baseline_path = self.results_dir / "last_run.json"

        # Results
        self.cpu_benchmarks: List[CriterionBenchmark] = []
        self.render_benchmarks: List[RenderBenchmark] = []
        self.baseline_data: Optional[Dict] = None

    def run_all(self) -> bool:
        """Run all benchmarks and generate report."""
        print(f"{Colors.HEADER}{'='*70}")
        print("Unified Performance Benchmark Suite")
        print(f"{'='*70}{Colors.ENDC}")
        print()
        print(f"Platform: {platform.system()}")
        print(f"Quick Mode: {'Yes (skip WASM, fast Criterion)' if self.quick_mode else 'No'}")
        print()

        # Load baseline for comparison
        self.load_baseline()

        # Run benchmarks
        success = True

        print(f"{Colors.BOLD}[1/3] Running CPU Microbenchmarks (Criterion)...{Colors.ENDC}")
        print("-" * 70)
        if not self.run_criterion_benchmarks():
            print(f"{Colors.FAIL}❌ CPU benchmarks failed{Colors.ENDC}")
            success = False
        print()

        print(f"{Colors.BOLD}[2/3] Running GPU Rendering Tests (Desktop CLI)...{Colors.ENDC}")
        print("-" * 70)
        if not self.run_desktop_rendering():
            print(f"{Colors.FAIL}❌ Desktop rendering tests failed{Colors.ENDC}")
            success = False
        print()

        if not self.quick_mode:
            print(f"{Colors.BOLD}[3/3] Running GPU Rendering Tests (WASM Browser)...{Colors.ENDC}")
            print("-" * 70)
            if not self.run_wasm_rendering():
                print(f"{Colors.WARNING}⚠️  WASM rendering tests failed or skipped{Colors.ENDC}")
            print()
        else:
            print(f"{Colors.OKCYAN}[3/3] Skipping WASM tests (quick mode){Colors.ENDC}")
            print()

        # Generate report
        self.generate_report()

        # Save results
        self.save_to_csv()
        self.save_baseline()

        return success

    def load_baseline(self):
        """Load previous run as baseline for comparison."""
        if self.baseline_path.exists():
            try:
                with open(self.baseline_path, 'r') as f:
                    self.baseline_data = json.load(f)
                print(f"{Colors.OKGREEN}✅ Loaded baseline from: {self.baseline_path}{Colors.ENDC}")
                baseline_time = self.baseline_data.get('timestamp', 'unknown')
                print(f"   Baseline timestamp: {baseline_time}")
                print()
            except Exception as e:
                print(f"{Colors.WARNING}⚠️  Failed to load baseline: {e}{Colors.ENDC}")
                print()

    def run_criterion_benchmarks(self) -> bool:
        """Run Criterion benchmarks and parse results."""
        try:
            # Run cargo bench (Criterion doesn't support bencher format anymore)
            args = ["--quick"] if self.quick_mode else []
            cmd = ["cargo", "bench", "--bench", "flame_bench", "--"] + args

            print(f"Running: {' '.join(cmd)}")
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=self.root
            )

            if result.returncode != 0:
                print(f"{Colors.FAIL}Criterion failed with code {result.returncode}{Colors.ENDC}")
                print(result.stderr)
                return False

            # Parse Criterion output
            # Format: benchmark_name     time:   [XX.XXX ns XX.XXX ns XX.XXX ns]
            pattern = r'(\S+)\s+time:\s+\[([0-9.]+)\s+([a-zµ]+)\s+([0-9.]+)\s+([a-zµ]+)\s+([0-9.]+)\s+([a-zµ]+)\]'

            for line in result.stdout.split('\n') + result.stderr.split('\n'):
                match = re.search(pattern, line)
                if match:
                    name = match.group(1).strip()
                    lower = float(match.group(2))
                    mean = float(match.group(4))
                    upper = float(match.group(6))
                    unit = match.group(5)

                    # Convert to nanoseconds
                    multiplier = {'ps': 0.001, 'ns': 1.0, 'µs': 1000.0, 'us': 1000.0, 'ms': 1_000_000.0}
                    mean_ns = mean * multiplier.get(unit, 1.0)
                    stddev_ns = (upper - lower) / 2.0 * multiplier.get(unit, 1.0)

                    # Calculate ops/sec
                    throughput_ops_sec = 1_000_000_000.0 / mean_ns if mean_ns > 0 else 0.0

                    self.cpu_benchmarks.append(CriterionBenchmark(
                        name=name,
                        mean_ns=mean_ns,
                        stddev_ns=stddev_ns,
                        throughput_ops_sec=throughput_ops_sec
                    ))

            print(f"{Colors.OKGREEN}✅ Parsed {len(self.cpu_benchmarks)} CPU benchmarks{Colors.ENDC}")
            return len(self.cpu_benchmarks) > 0

        except Exception as e:
            print(f"{Colors.FAIL}Error running Criterion: {e}{Colors.ENDC}")
            import traceback
            traceback.print_exc()
            return False

    def run_desktop_rendering(self) -> bool:
        """Run desktop CLI rendering tests."""
        try:
            # Build release first
            print("Building release binary...")
            result = subprocess.run(
                ["cargo", "build", "--release"],
                capture_output=True,
                text=True,
                cwd=self.root
            )

            if result.returncode != 0:
                print(f"{Colors.FAIL}Build failed{Colors.ENDC}")
                return False

            # Ensure output directory exists
            self.current_dir.mkdir(parents=True, exist_ok=True)

            # Find all config files
            config_files = list(self.configs_dir.rglob("*.fflame"))

            if not config_files:
                print(f"{Colors.WARNING}No .fflame configs found in {self.configs_dir}{Colors.ENDC}")
                return False

            print(f"Found {len(config_files)} config files")

            # Render each config
            for config_path in config_files:
                name = config_path.stem
                output_path = self.current_dir / f"{name}.png"

                # Run export
                cmd = [
                    "cargo", "run", "--release", "--",
                    "export",
                    "-i", str(config_path),
                    "-o", str(output_path)
                ]

                print(f"  Rendering {name}...", end=" ", flush=True)

                result = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    cwd=self.root
                )

                if result.returncode != 0:
                    print(f"{Colors.FAIL}FAILED{Colors.ENDC}")
                    continue

                # Extract metadata from PNG
                metadata = self.extract_png_metadata(output_path)
                if metadata and metadata['render_time_ms'] > 0:
                    throughput = metadata['total_iterations'] / metadata['render_time_ms'] / 1000.0
                    self.render_benchmarks.append(RenderBenchmark(
                        name=name,
                        test_type="desktop",
                        width=metadata['width'],
                        height=metadata['height'],
                        total_iterations=metadata['total_iterations'],
                        render_time_ms=metadata['render_time_ms'],
                        throughput_miter_sec=throughput
                    ))
                    print(f"{Colors.OKGREEN}OK ({metadata['render_time_ms']:.1f}ms, {throughput:.1f} Miter/s){Colors.ENDC}")
                else:
                    print(f"{Colors.WARNING}OK (no metadata){Colors.ENDC}")

            return True

        except Exception as e:
            print(f"{Colors.FAIL}Error in desktop rendering: {e}{Colors.ENDC}")
            return False

    def run_wasm_rendering(self) -> bool:
        """Run WASM browser rendering tests."""
        try:
            # Run the WASM test script
            result = subprocess.run(
                [sys.executable, "tests/visual/wasm/test_wasm.py"],
                capture_output=True,
                text=True,
                cwd=self.root
            )

            if result.returncode != 0:
                print(f"{Colors.WARNING}WASM tests failed or not available{Colors.ENDC}")
                return False

            # Extract metadata from WASM PNGs
            wasm_dir = self.current_dir / "wasm"
            if wasm_dir.exists():
                for png_path in wasm_dir.glob("*.png"):
                    name = png_path.stem
                    metadata = self.extract_png_metadata(png_path)
                    if metadata and metadata['render_time_ms'] > 0:
                        throughput = metadata['total_iterations'] / metadata['render_time_ms'] / 1000.0
                        self.render_benchmarks.append(RenderBenchmark(
                            name=name,
                            test_type="wasm",
                            width=metadata['width'],
                            height=metadata['height'],
                            total_iterations=metadata['total_iterations'],
                            render_time_ms=metadata['render_time_ms'],
                            throughput_miter_sec=throughput
                        ))

            print(f"{Colors.OKGREEN}✅ Completed WASM tests{Colors.ENDC}")
            return True

        except Exception as e:
            print(f"{Colors.WARNING}WASM tests skipped: {e}{Colors.ENDC}")
            return False

    def extract_png_metadata(self, png_path: Path) -> Optional[Dict]:
        """Extract metadata from PNG tEXt chunks."""
        if not HAS_PILLOW or not png_path.exists():
            return None

        try:
            img = Image.open(png_path)
            info = img.info

            # Extract key fields
            total_iterations = int(info.get('total_iterations', 0))
            render_time_ms = float(info.get('render_time_ms', 0))

            return {
                'width': img.width,
                'height': img.height,
                'total_iterations': total_iterations,
                'render_time_ms': render_time_ms,
            }
        except Exception:
            return None

    def generate_report(self):
        """Generate console report with regression detection."""
        print(f"{Colors.HEADER}{'='*70}")
        print("Benchmark Results")
        print(f"{'='*70}{Colors.ENDC}")
        print()

        # CPU Benchmarks
        if self.cpu_benchmarks:
            print(f"{Colors.BOLD}CPU Microbenchmarks (Criterion):{Colors.ENDC}")
            print(f"{'Benchmark':<50} {'Mean':<15} {'Ops/sec':<15}")
            print("-" * 70)

            for bench in self.cpu_benchmarks:
                mean_str = self.format_time(bench.mean_ns)
                ops_str = self.format_throughput(bench.throughput_ops_sec)

                # Check for regression
                regression = self.check_cpu_regression(bench)
                color = Colors.ENDC
                marker = ""

                if regression:
                    if abs(regression) > 10.0:
                        color = Colors.FAIL
                        marker = f" ⚠️  {regression:+.1f}%"
                    elif abs(regression) > 5.0:
                        color = Colors.WARNING
                        marker = f" ⚠️  {regression:+.1f}%"
                    elif regression < -2.0:
                        color = Colors.OKGREEN
                        marker = f" ✨ {regression:+.1f}%"

                print(f"{color}{bench.name:<50} {mean_str:<15} {ops_str:<15}{marker}{Colors.ENDC}")

            print()

        # GPU Benchmarks
        if self.render_benchmarks:
            print(f"{Colors.BOLD}GPU Rendering Benchmarks:{Colors.ENDC}")
            print(f"{'Test':<30} {'Type':<10} {'Time':<15} {'Throughput':<20}")
            print("-" * 70)

            for bench in self.render_benchmarks:
                time_str = f"{bench.render_time_ms:.1f}ms"
                throughput_str = f"{bench.throughput_miter_sec:.1f} Miter/s"

                # Check for regression
                regression = self.check_render_regression(bench)
                color = Colors.ENDC
                marker = ""

                if regression:
                    if regression > 10.0:
                        color = Colors.FAIL
                        marker = f" ⚠️  {regression:+.1f}%"
                    elif regression > 5.0:
                        color = Colors.WARNING
                        marker = f" ⚠️  {regression:+.1f}%"
                    elif regression < -2.0:
                        color = Colors.OKGREEN
                        marker = f" ✨ {regression:+.1f}%"

                print(f"{color}{bench.name:<30} {bench.test_type:<10} {time_str:<15} {throughput_str:<20}{marker}{Colors.ENDC}")

            print()

        # Summary
        total = len(self.cpu_benchmarks) + len(self.render_benchmarks)
        print(f"{Colors.BOLD}Summary:{Colors.ENDC}")
        print(f"  Total benchmarks: {total}")
        print(f"  CPU benchmarks: {len(self.cpu_benchmarks)}")
        print(f"  GPU benchmarks: {len(self.render_benchmarks)}")
        print()

    def check_cpu_regression(self, bench: CriterionBenchmark) -> Optional[float]:
        """Check for CPU benchmark regression. Returns percent change."""
        if not self.baseline_data or 'cpu_benchmarks' not in self.baseline_data:
            return None

        for baseline_bench in self.baseline_data['cpu_benchmarks']:
            if baseline_bench['name'] == bench.name:
                baseline_mean = baseline_bench['mean_ns']
                current_mean = bench.mean_ns
                percent_change = ((current_mean - baseline_mean) / baseline_mean) * 100.0
                return percent_change

        return None

    def check_render_regression(self, bench: RenderBenchmark) -> Optional[float]:
        """Check for rendering regression. Returns percent change."""
        if not self.baseline_data or 'render_benchmarks' not in self.baseline_data:
            return None

        for baseline_bench in self.baseline_data['render_benchmarks']:
            if baseline_bench['name'] == bench.name and baseline_bench['test_type'] == bench.test_type:
                baseline_time = baseline_bench['render_time_ms']
                current_time = bench.render_time_ms
                percent_change = ((current_time - baseline_time) / baseline_time) * 100.0
                return percent_change

        return None

    def format_time(self, ns: float) -> str:
        """Format nanoseconds as human-readable time."""
        if ns < 1000:
            return f"{ns:.0f} ns"
        elif ns < 1_000_000:
            return f"{ns/1000:.1f} µs"
        else:
            return f"{ns/1_000_000:.1f} ms"

    def format_throughput(self, ops_sec: float) -> str:
        """Format ops/sec as human-readable throughput."""
        if ops_sec < 1_000_000:
            return f"{ops_sec/1000:.1f} K/s"
        elif ops_sec < 1_000_000_000:
            return f"{ops_sec/1_000_000:.1f} M/s"
        else:
            return f"{ops_sec/1_000_000_000:.1f} G/s"

    def save_to_csv(self):
        """Save results to unified CSV."""
        # Get git info
        git_commit = self.run_cmd("git rev-parse --short HEAD")
        git_branch = self.run_cmd("git rev-parse --abbrev-ref HEAD")
        rustc_version = self.run_cmd("rustc --version")

        timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
        platform_name = platform.system()

        # Check if CSV exists
        csv_exists = self.csv_path.exists()

        with open(self.csv_path, 'a', newline='', encoding='utf-8') as f:
            writer = csv.writer(f)

            # Write header if new file
            if not csv_exists:
                writer.writerow([
                    'Timestamp', 'Platform', 'GitCommit', 'GitBranch', 'RustcVersion',
                    'BenchmarkType', 'BenchmarkName', 'TestType',
                    'Mean_ns', 'StdDev_ns', 'Throughput_ops_sec',
                    'Width', 'Height', 'TotalIterations', 'RenderTime_ms', 'Throughput_Miter_sec'
                ])

            # Write CPU benchmarks
            for bench in self.cpu_benchmarks:
                writer.writerow([
                    timestamp, platform_name, git_commit, git_branch, rustc_version,
                    'cpu', bench.name, '',
                    f"{bench.mean_ns:.2f}", f"{bench.stddev_ns:.2f}", f"{bench.throughput_ops_sec:.0f}",
                    '', '', '', '', ''
                ])

            # Write render benchmarks
            for bench in self.render_benchmarks:
                writer.writerow([
                    timestamp, platform_name, git_commit, git_branch, rustc_version,
                    'render', bench.name, bench.test_type,
                    '', '', '',
                    bench.width, bench.height, bench.total_iterations,
                    f"{bench.render_time_ms:.2f}", f"{bench.throughput_miter_sec:.2f}"
                ])

        print(f"{Colors.OKGREEN}✅ Results saved to: {self.csv_path}{Colors.ENDC}")
        print()

    def save_baseline(self):
        """Save current run as baseline for next comparison."""
        baseline = {
            'timestamp': datetime.now().strftime('%Y-%m-%d %H:%M:%S'),
            'cpu_benchmarks': [asdict(b) for b in self.cpu_benchmarks],
            'render_benchmarks': [asdict(b) for b in self.render_benchmarks],
        }

        with open(self.baseline_path, 'w') as f:
            json.dump(baseline, f, indent=2)

        print(f"{Colors.OKGREEN}✅ Baseline saved to: {self.baseline_path}{Colors.ENDC}")
        print()

    def run_cmd(self, cmd: str) -> str:
        """Run command and return output."""
        try:
            result = subprocess.run(
                cmd.split(),
                capture_output=True,
                text=True,
                cwd=self.root
            )
            return result.stdout.strip()
        except:
            return ""


def main():
    parser = argparse.ArgumentParser(
        description='Unified performance benchmark suite',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python scripts/run_benchmarks.py              # Full benchmark suite
  python scripts/run_benchmarks.py --quick      # Skip WASM, fast Criterion
        """
    )
    parser.add_argument(
        '--quick',
        action='store_true',
        help='Quick mode: Skip WASM tests, use faster Criterion settings'
    )

    args = parser.parse_args()

    # Disable colors on Windows if needed
    if platform.system() == 'Windows':
        try:
            import colorama
            colorama.init()
        except ImportError:
            Colors.disable()

    runner = UnifiedBenchmarkRunner(quick_mode=args.quick)
    success = runner.run_all()

    sys.exit(0 if success else 1)


if __name__ == '__main__':
    main()
