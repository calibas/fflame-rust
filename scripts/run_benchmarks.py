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
    quick_mode: bool  # True if run with --quick flag

    # CPU benchmarks
    cpu_benchmarks: List[CriterionBenchmark]

    # GPU benchmarks
    render_benchmarks: List[RenderBenchmark]

    # Summary stats
    total_tests: int
    passed_tests: int
    failed_tests: int

    # Aggregate metric (ops/sec across all benchmarks)
    aggregate_ops_sec: float = 0.0


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
        self.previous_runs: List[Tuple[str, float]] = []  # [(timestamp, aggregate_ops_sec), ...]
        self.prev_run_1: Optional[List] = None  # Previous run #1 rows
        self.prev_run_2: Optional[List] = None  # Previous run #2 rows

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

        # Load previous 2 full runs from CSV
        self.load_previous_runs()

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
            # Criterion outputs benchmark name on one line, then "time:" on the next
            # Example:
            # affine_transform/affine_2x3_matrix
            #                         time:   [1.2973 ns 1.2988 ns 1.2992 ns]

            lines = (result.stdout + result.stderr).split('\n')
            current_benchmark = None

            for line in lines:
                # Check for benchmark name in various formats:
                # 1. "benchmark_name/subname" (on its own line)
                # 2. "Benchmarking benchmark_name/subname: Analyzing"
                # 3. "benchmark_name/subname time:   [...]" (name on same line as time)

                # Try to extract benchmark name from line
                bench_name_match = re.search(r'([a-z_]+/[a-z_0-9/.]+)', line, re.IGNORECASE)
                if bench_name_match and 'Benchmarking' not in line:
                    # Found a potential benchmark name
                    potential_name = bench_name_match.group(1)
                    # Only update if this line doesn't have 'time:' (which means name is on previous line)
                    if 'time:' not in line:
                        current_benchmark = potential_name

                # Check if this is a time line (and extract benchmark name from same line if present)
                time_pattern = r'time:\s+\[([0-9.]+)\s+([a-zµ]+)\s+([0-9.]+)\s+([a-zµ]+)\s+([0-9.]+)\s+([a-zµ]+)\]'
                match = re.search(time_pattern, line)
                if match:
                    # If benchmark name is on the same line as time, use it
                    if bench_name_match and current_benchmark is None:
                        current_benchmark = bench_name_match.group(1)

                    if current_benchmark:
                        lower = float(match.group(1))
                        mean = float(match.group(3))
                        upper = float(match.group(5))
                        unit = match.group(4)

                        # Convert to nanoseconds
                        multiplier = {'ps': 0.001, 'ns': 1.0, 'µs': 1000.0, 'us': 1000.0, 'ms': 1_000_000.0}
                        mean_ns = mean * multiplier.get(unit, 1.0)
                        stddev_ns = (upper - lower) / 2.0 * multiplier.get(unit, 1.0)

                        # Calculate ops/sec
                        throughput_ops_sec = 1_000_000_000.0 / mean_ns if mean_ns > 0 else 0.0

                        self.cpu_benchmarks.append(CriterionBenchmark(
                            name=current_benchmark,
                            mean_ns=mean_ns,
                            stddev_ns=stddev_ns,
                            throughput_ops_sec=throughput_ops_sec
                        ))

                        current_benchmark = None  # Reset after capturing

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
            print(f"Running {5 if not self.quick_mode else 3} iterations per config (warmup + measurement)")
            print()

            # Render each config multiple times
            repeats = 3 if self.quick_mode else 5

            for config_path in config_files:
                name = config_path.stem
                print(f"  {name}:")

                render_times = []
                metadata_cache = None

                # Run multiple times
                for i in range(repeats):
                    output_path = self.current_dir / f"{name}_run{i}.png"

                    cmd = [
                        "cargo", "run", "--release", "--",
                        "export",
                        "-i", str(config_path),
                        "-o", str(output_path)
                    ]

                    is_warmup = (i == 0)
                    print(f"    Run {i+1}/{repeats} {'(warmup)' if is_warmup else ''}...", end=" ", flush=True)

                    result = subprocess.run(
                        cmd,
                        capture_output=True,
                        text=True,
                        cwd=self.root
                    )

                    if result.returncode != 0:
                        print(f"{Colors.FAIL}FAILED{Colors.ENDC}")
                        continue

                    # Extract metadata
                    metadata = self.extract_png_metadata(output_path)
                    if metadata and metadata['render_time_ms'] > 0:
                        render_times.append(metadata['render_time_ms'])
                        if metadata_cache is None:
                            metadata_cache = metadata
                        print(f"{Colors.OKGREEN}{metadata['render_time_ms']:.1f}ms{Colors.ENDC}")
                    else:
                        print(f"{Colors.WARNING}no metadata{Colors.ENDC}")

                    # Clean up intermediate files (keep last one for visual inspection)
                    if i < repeats - 1:
                        try:
                            output_path.unlink()
                        except:
                            pass

                # Rename last file to canonical name
                if repeats > 0:
                    last_file = self.current_dir / f"{name}_run{repeats-1}.png"
                    final_file = self.current_dir / f"{name}.png"
                    if last_file.exists():
                        if final_file.exists():
                            final_file.unlink()
                        last_file.rename(final_file)

                # Calculate statistics (discard first run as warmup)
                if len(render_times) > 1:
                    times_for_stats = render_times[1:]  # Skip warmup
                    mean_time = sum(times_for_stats) / len(times_for_stats)
                    min_time = min(times_for_stats)
                    max_time = max(times_for_stats)

                    # Calculate standard deviation
                    if len(times_for_stats) > 1:
                        variance = sum((t - mean_time) ** 2 for t in times_for_stats) / len(times_for_stats)
                        stddev = variance ** 0.5
                        cv_percent = (stddev / mean_time * 100) if mean_time > 0 else 0
                    else:
                        stddev = 0
                        cv_percent = 0

                    if metadata_cache:
                        throughput = metadata_cache['total_iterations'] / mean_time / 1000.0
                        self.render_benchmarks.append(RenderBenchmark(
                            name=name,
                            test_type="desktop",
                            width=metadata_cache['width'],
                            height=metadata_cache['height'],
                            total_iterations=metadata_cache['total_iterations'],
                            render_time_ms=mean_time,
                            throughput_miter_sec=throughput
                        ))

                        print(f"    {Colors.BOLD}Stats: mean={mean_time:.1f}ms, stddev={stddev:.2f}ms ({cv_percent:.1f}%), range=[{min_time:.1f}, {max_time:.1f}]ms{Colors.ENDC}")
                    print()
                elif len(render_times) == 1:
                    # Only one successful run
                    if metadata_cache:
                        throughput = metadata_cache['total_iterations'] / render_times[0] / 1000.0
                        self.render_benchmarks.append(RenderBenchmark(
                            name=name,
                            test_type="desktop",
                            width=metadata_cache['width'],
                            height=metadata_cache['height'],
                            total_iterations=metadata_cache['total_iterations'],
                            render_time_ms=render_times[0],
                            throughput_miter_sec=throughput
                        ))
                    print()
                else:
                    print(f"    {Colors.FAIL}All runs failed{Colors.ENDC}")
                    print()

            return True

        except Exception as e:
            print(f"{Colors.FAIL}Error in desktop rendering: {e}{Colors.ENDC}")
            return False

    def run_wasm_rendering(self) -> bool:
        """Run WASM browser rendering tests with multiple runs."""
        try:
            wasm_dir = self.current_dir / "wasm"
            wasm_dir.mkdir(parents=True, exist_ok=True)

            repeats = 3 if self.quick_mode else 5
            print(f"Running {repeats} iterations (warmup + measurement) via browser automation")
            print()

            # Track render times per config
            config_times = {}

            # Run multiple times
            for run_idx in range(repeats):
                is_warmup = (run_idx == 0)
                print(f"  WASM Run {run_idx + 1}/{repeats} {'(warmup)' if is_warmup else ''}:")

                # Run the WASM test script
                result = subprocess.run(
                    [sys.executable, "tests/visual/wasm/test_wasm.py"],
                    capture_output=True,
                    text=True,
                    cwd=self.root
                )

                if result.returncode != 0:
                    print(f"{Colors.FAIL}    WASM test run failed{Colors.ENDC}")
                    if run_idx == 0:  # If first run fails, abort
                        print(f"{Colors.WARNING}WASM tests failed or not available{Colors.ENDC}")
                        return False
                    continue

                # Extract metadata from WASM PNGs for this run
                if wasm_dir.exists():
                    for png_path in wasm_dir.glob("*.png"):
                        name = png_path.stem
                        metadata = self.extract_png_metadata(png_path)
                        if metadata and metadata['render_time_ms'] > 0:
                            if name not in config_times:
                                config_times[name] = {
                                    'times': [],
                                    'metadata': metadata
                                }
                            config_times[name]['times'].append(metadata['render_time_ms'])
                            print(f"    {name}: {metadata['render_time_ms']:.1f}ms")

                print()

            # Calculate statistics for each config
            for name, data in config_times.items():
                times = data['times']
                metadata = data['metadata']

                if len(times) > 1:
                    # Discard first run (warmup)
                    times_for_stats = times[1:]
                    mean_time = sum(times_for_stats) / len(times_for_stats)
                    min_time = min(times_for_stats)
                    max_time = max(times_for_stats)

                    # Calculate standard deviation
                    if len(times_for_stats) > 1:
                        variance = sum((t - mean_time) ** 2 for t in times_for_stats) / len(times_for_stats)
                        stddev = variance ** 0.5
                        cv_percent = (stddev / mean_time * 100) if mean_time > 0 else 0
                    else:
                        stddev = 0
                        cv_percent = 0

                    throughput = metadata['total_iterations'] / mean_time / 1000.0
                    self.render_benchmarks.append(RenderBenchmark(
                        name=name,
                        test_type="wasm",
                        width=metadata['width'],
                        height=metadata['height'],
                        total_iterations=metadata['total_iterations'],
                        render_time_ms=mean_time,
                        throughput_miter_sec=throughput
                    ))

                    print(f"  {name} Stats: mean={mean_time:.1f}ms, stddev={stddev:.2f}ms ({cv_percent:.1f}%), range=[{min_time:.1f}, {max_time:.1f}]ms")

                elif len(times) == 1:
                    # Single run only
                    throughput = metadata['total_iterations'] / times[0] / 1000.0
                    self.render_benchmarks.append(RenderBenchmark(
                        name=name,
                        test_type="wasm",
                        width=metadata['width'],
                        height=metadata['height'],
                        total_iterations=metadata['total_iterations'],
                        render_time_ms=times[0],
                        throughput_miter_sec=throughput
                    ))

            print()
            print(f"{Colors.OKGREEN}✅ Completed WASM tests ({len(config_times)} configs){Colors.ENDC}")
            return True

        except Exception as e:
            print(f"{Colors.WARNING}WASM tests skipped: {e}{Colors.ENDC}")
            import traceback
            traceback.print_exc()
            return False

    def extract_png_metadata(self, png_path: Path) -> Optional[Dict]:
        """Extract metadata from PNG tEXt chunks."""
        if not HAS_PILLOW or not png_path.exists():
            return None

        try:
            img = Image.open(png_path)
            info = img.info

            # Extract key fields (PNG metadata keys are capitalized)
            total_iterations = int(info.get('Iterations', 0))
            render_time_str = info.get('RenderTime', '0ms')

            # Parse render time (format: "123.45ms")
            render_time_ms = float(render_time_str.replace('ms', ''))

            return {
                'width': img.width,
                'height': img.height,
                'total_iterations': total_iterations,
                'render_time_ms': render_time_ms,
            }
        except Exception as e:
            print(f"{Colors.WARNING}Failed to extract metadata from {png_path.name}: {e}{Colors.ENDC}")
            return None

    def generate_report(self):
        """Generate console report with regression detection."""
        print(f"{Colors.HEADER}{'='*70}")
        print("Benchmark Results")
        print(f"{'='*70}{Colors.ENDC}")
        print()

        # CPU Benchmarks Table
        if self.cpu_benchmarks:
            print(f"{Colors.BOLD}CPU Microbenchmarks (Criterion):{Colors.ENDC}")
            print(f"{'Benchmark':<50} {'Mean':<15} {'Ops/sec':<15} {'% Change':<15} {'Previous #1':<15} {'Previous #2':<15}")
            print("-" * 135)

            for bench in self.cpu_benchmarks:
                mean_str = self.format_time(bench.mean_ns)
                ops_str = f"{bench.throughput_ops_sec:,.0f}"

                # Get previous runs
                prev1_ops = self.get_prev_cpu_benchmark(bench.name, 0)
                prev2_ops = self.get_prev_cpu_benchmark(bench.name, 1)

                prev1_str = f"{prev1_ops:,.0f}" if prev1_ops else "—"
                prev2_str = f"{prev2_ops:,.0f}" if prev2_ops else "—"

                # Calculate % change vs previous #1
                if prev1_ops:
                    delta = ((bench.throughput_ops_sec - prev1_ops) / prev1_ops) * 100.0
                    row_color = Colors.ENDC
                    # For CPU: higher ops/sec is better, lower is worse
                    if delta < -10.0:  # >10% slower
                        row_color = Colors.FAIL
                    elif delta < -5.0:  # >5% slower
                        row_color = Colors.WARNING
                    elif delta > 4.0:  # >4% faster
                        row_color = Colors.OKGREEN
                    delta_str = f"{delta:+.1f}%"
                else:
                    delta_str = "—"
                    row_color = Colors.ENDC

                print(f"{row_color}{bench.name:<50} {mean_str:<15} {ops_str:<15} {delta_str:<15} {prev1_str:<15} {prev2_str:<15}{Colors.ENDC}")

            print()

        # GPU Benchmarks Table
        if self.render_benchmarks:
            print(f"{Colors.BOLD}GPU Rendering Benchmarks:{Colors.ENDC}")
            print(f"{'Test':<30} {'Type':<10} {'Time':<15} {'Throughput':<20} {'% Change':<15} {'Previous #1':<20} {'Previous #2':<20}")
            print("-" * 135)

            for bench in self.render_benchmarks:
                time_str = f"{bench.render_time_ms:.1f}ms"
                throughput_str = f"{bench.throughput_miter_sec:.1f} Miter/s"

                # Get previous runs
                prev1_throughput = self.get_prev_render_benchmark(bench.name, bench.test_type, 0)
                prev2_throughput = self.get_prev_render_benchmark(bench.name, bench.test_type, 1)

                prev1_str = f"{prev1_throughput:.1f} Miter/s" if prev1_throughput else "—"
                prev2_str = f"{prev2_throughput:.1f} Miter/s" if prev2_throughput else "—"

                # Calculate % change vs previous #1
                if prev1_throughput:
                    delta = ((bench.throughput_miter_sec - prev1_throughput) / prev1_throughput) * 100.0
                    row_color = Colors.ENDC
                    if delta < -10.0:  # Note: lower throughput is worse
                        row_color = Colors.FAIL
                    elif delta < -5.0:
                        row_color = Colors.WARNING
                    elif delta > 2.0:
                        row_color = Colors.OKGREEN
                    delta_str = f"{delta:+.1f}%"
                else:
                    delta_str = "—"
                    row_color = Colors.ENDC

                print(f"{row_color}{bench.name:<30} {bench.test_type:<10} {time_str:<15} {throughput_str:<20} {delta_str:<15} {prev1_str:<20} {prev2_str:<20}{Colors.ENDC}")

            print()

        # Summary
        total = len(self.cpu_benchmarks) + len(self.render_benchmarks)
        print(f"{Colors.BOLD}Summary:{Colors.ENDC}")
        print(f"  Total benchmarks: {total}")
        print(f"  CPU benchmarks: {len(self.cpu_benchmarks)}")
        print(f"  GPU benchmarks: {len(self.render_benchmarks)}")
        print()

        # Aggregate Performance Comparison
        current_ops_sec = self.calculate_aggregate_ops_sec()
        print(f"{Colors.BOLD}Aggregate Performance Comparison:{Colors.ENDC}")
        print()

        # Prepare data for 3 columns: Current, Prev #1, Prev #2
        prev1_ops_sec = self.previous_runs[0][1] if len(self.previous_runs) >= 1 else None
        prev2_ops_sec = self.previous_runs[1][1] if len(self.previous_runs) >= 2 else None

        # Calculate deltas
        delta1_str = "—"
        delta2_str = "—"

        if prev1_ops_sec:
            delta1 = ((current_ops_sec - prev1_ops_sec) / prev1_ops_sec) * 100.0
            color = Colors.ENDC
            if delta1 > 10.0:
                color = Colors.FAIL
            elif delta1 > 5.0:
                color = Colors.WARNING
            elif delta1 < -2.0:
                color = Colors.OKGREEN
            delta1_str = f"{color}{delta1:+.1f}%{Colors.ENDC}"

        if prev2_ops_sec:
            delta2 = ((current_ops_sec - prev2_ops_sec) / prev2_ops_sec) * 100.0
            color = Colors.ENDC
            if delta2 > 10.0:
                color = Colors.FAIL
            elif delta2 > 5.0:
                color = Colors.WARNING
            elif delta2 < -2.0:
                color = Colors.OKGREEN
            delta2_str = f"{color}{delta2:+.1f}%{Colors.ENDC}"

        # Table header
        print(f"  {'Metric':<20} {'Current':<20} {'Previous #1':<20} {'Previous #2':<20}")
        print(f"  {'-'*20} {'-'*20} {'-'*20} {'-'*20}")

        # Throughput row
        current_throughput = self.format_throughput(current_ops_sec)
        prev1_throughput = self.format_throughput(prev1_ops_sec) if prev1_ops_sec else "—"
        prev2_throughput = self.format_throughput(prev2_ops_sec) if prev2_ops_sec else "—"
        print(f"  {'Throughput':<20} {current_throughput:<20} {prev1_throughput:<20} {prev2_throughput:<20}")

        # ops/sec row
        current_ops_str = f"{current_ops_sec:,.0f}"
        prev1_ops_str = f"{prev1_ops_sec:,.0f}" if prev1_ops_sec else "—"
        prev2_ops_str = f"{prev2_ops_sec:,.0f}" if prev2_ops_sec else "—"
        print(f"  {'ops/sec':<20} {current_ops_str:<20} {prev1_ops_str:<20} {prev2_ops_str:<20}")

        # Delta row
        print(f"  {'Delta vs Current':<20} {'—':<20} {delta1_str:<20} {delta2_str:<20}")

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

    def calculate_aggregate_ops_sec(self) -> float:
        """Calculate total ops/sec across all benchmarks."""
        total = 0.0

        # CPU benchmarks: use throughput_ops_sec directly
        for bench in self.cpu_benchmarks:
            total += bench.throughput_ops_sec

        # GPU benchmarks: throughput_miter_sec is already in millions of iterations per second
        # Convert to ops/sec by multiplying by 1,000,000
        for bench in self.render_benchmarks:
            total += bench.throughput_miter_sec * 1_000_000.0

        return total

    def load_previous_runs(self):
        """Load last 2 full benchmark runs from CSV for comparison."""
        if not self.csv_path.exists():
            return

        try:
            # Read CSV from bottom up to get most recent runs
            with open(self.csv_path, 'r', encoding='utf-8') as f:
                lines = list(csv.reader(f))

            if len(lines) <= 1:  # Only header or empty
                return

            # Group rows by timestamp (each run has multiple rows)
            runs = {}
            for row in reversed(lines[1:]):  # Skip header, process newest first
                timestamp = row[0]
                if timestamp not in runs:
                    runs[timestamp] = []
                runs[timestamp].append(row)

            # Get the two most recent runs
            sorted_timestamps = sorted(runs.keys(), reverse=True)
            if len(sorted_timestamps) >= 1:
                self.prev_run_1 = runs[sorted_timestamps[0]]
            if len(sorted_timestamps) >= 2:
                self.prev_run_2 = runs[sorted_timestamps[1]]

            # Also calculate aggregate for each run
            run_aggregates = []
            for timestamp in sorted_timestamps[:2]:
                rows = runs[timestamp]
                total_ops_sec = 0.0

                for row in rows:
                    benchmark_type = row[5]  # 'cpu' or 'render'

                    if benchmark_type == 'cpu':
                        # Column 10: Throughput_ops_sec
                        if row[10]:
                            total_ops_sec += float(row[10])
                    elif benchmark_type == 'render':
                        # Column 15: Throughput_Miter_sec (convert to ops/sec)
                        if row[15]:
                            total_ops_sec += float(row[15]) * 1_000_000.0

                run_aggregates.append((timestamp, total_ops_sec))

            self.previous_runs = run_aggregates

        except Exception as e:
            print(f"{Colors.WARNING}Failed to load previous runs: {e}{Colors.ENDC}")

    def get_prev_cpu_benchmark(self, bench_name: str, run_index: int) -> Optional[float]:
        """Get ops/sec from previous run for CPU benchmark (run_index: 0=prev1, 1=prev2)."""
        prev_run = self.prev_run_1 if run_index == 0 else self.prev_run_2
        if not prev_run:
            return None

        for row in prev_run:
            if row[5] == 'cpu' and row[6] == bench_name:
                # Column 10: Throughput_ops_sec
                if row[10]:
                    return float(row[10])
        return None

    def get_prev_render_benchmark(self, bench_name: str, test_type: str, run_index: int) -> Optional[float]:
        """Get throughput from previous run for GPU benchmark (run_index: 0=prev1, 1=prev2)."""
        prev_run = self.prev_run_1 if run_index == 0 else self.prev_run_2
        if not prev_run:
            return None

        for row in prev_run:
            if row[5] == 'render' and row[6] == bench_name and row[7] == test_type:
                # Column 15: Throughput_Miter_sec
                if row[15]:
                    return float(row[15])
        return None

    def save_to_csv(self):
        """Save results to unified CSV (only if not quick mode)."""
        if self.quick_mode:
            print(f"{Colors.OKCYAN}Quick mode: Skipping CSV save{Colors.ENDC}")
            print()
            return

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
