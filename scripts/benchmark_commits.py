#!/usr/bin/env python3
"""
Benchmark script for comparing performance across commits.
Runs multiple repeats to account for variance.
Platform-agnostic (Windows/Linux/macOS).

IMPORTANT: This script temporarily checks out different commits to benchmark them.
It will return to the original branch when finished, but will NOT touch untracked
files in your working directory. Benchmark results are stored in benchmark_results/.
"""

import argparse
import subprocess
import sys
import re
import csv
import os
import shutil
from datetime import datetime
from pathlib import Path
import statistics
import platform

# Fix Windows console encoding for Unicode
if platform.system() == 'Windows':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

def run_command(cmd, cwd=None, capture_output=True):
    """Run a shell command and return output."""
    try:
        result = subprocess.run(
            cmd,
            shell=True,
            cwd=cwd,
            capture_output=capture_output,
            text=True,
            check=False
        )
        return result.returncode, result.stdout, result.stderr
    except Exception as e:
        return -1, "", str(e)

def extract_metadata(compare_output):
    """Extract metadata from compare_images output."""
    metadata = {}

    patterns = {
        'total_iterations': r'Total Iterations: ([\d]+)',
        'iterations_per_thread': r'Iterations/Thread: ([\d]+)',
        'speed_factor': r'Speed Factor: ([\d.]+)',
        'render_time': r'Render Time: ([\d.]+)ms',
        'rustc_version': r'Rustc Version: ([\d.]+)',
        'build_profile': r'Build Profile: (\w+)',
    }

    for key, pattern in patterns.items():
        match = re.search(pattern, compare_output)
        if match:
            metadata[key] = match.group(1)

    return metadata

def run_benchmark(commit_hash, commit_name, config_file, repeats, width, height, results_dir, fractal_name):
    """Run benchmark for a single commit."""
    print(f"\n{'='*60}")
    print(f"Testing: {commit_name}")
    print(f"Commit: {commit_hash}")
    print(f"{'='*60}")

    # Checkout commit
    print("Checking out commit...")
    returncode, _, stderr = run_command(f"git checkout {commit_hash}")
    if returncode != 0:
        print(f"❌ Failed to checkout {commit_hash}")
        return None

    # Build in release mode
    print("Building...")
    returncode, _, stderr = run_command("cargo build --release")
    if returncode != 0:
        print(f"❌ Build failed for {commit_hash}")
        return None

    # Run benchmark repeats
    times = []
    metadata_cache = None

    # Use 7-character commit hash for filenames
    short_hash = commit_hash[:7]

    for i in range(1, repeats + 1):
        print(f"  Repeat {i}/{repeats}...", end=" ", flush=True)

        # Format: [name]_[commit]_[number].png
        output_file = results_dir / f"{fractal_name}_{short_hash}_{i}.png"

        # Run export
        cmd = f"cargo run --release -- export -i {config_file} -o {output_file} --width {width} --height {height}"
        returncode, stdout, stderr = run_command(cmd)

        if returncode != 0:
            print("❌ (export failed)")
            continue

        # Try to extract metadata using compare_images (if available)
        cmd = f"cargo run --release --bin compare_images -- -1 {output_file} -2 {output_file} --skip-color-check"
        returncode, stdout, stderr = run_command(cmd)

        # Extract render time - try both new and old format
        match = re.search(r'Render Time: ([\d.]+)ms', stdout + stderr)  # New format
        if not match:
            match = re.search(r'Render time: ([\d.]+)ms', stdout + stderr)  # Old format

        if match:
            time = float(match.group(1))
            times.append(time)
            print(f"✅ {time:.2f}ms")

            # Cache metadata from first successful run
            if metadata_cache is None:
                metadata_cache = extract_metadata(stdout + stderr)
        else:
            print("❌ (failed to extract time)")

    # Calculate statistics
    if not times:
        print("❌ No successful runs")
        return None

    mean = statistics.mean(times)
    stddev = statistics.stdev(times) if len(times) > 1 else 0.0
    min_time = min(times)
    max_time = max(times)
    cv = (stddev / mean * 100) if mean > 0 else 0.0

    # Calculate throughput (Giter/sec)
    total_iter = float(metadata_cache.get('total_iterations', 0)) if metadata_cache else 0
    throughput = (total_iter / (mean / 1000.0)) / 1_000_000_000.0 if mean > 0 and total_iter > 0 else 0.0

    print("\nStatistics:")
    print(f"  Mean:       {mean:.2f} ms")
    print(f"  StdDev:     {stddev:.2f} ms ({cv:.2f}%)")
    print(f"  Min:        {min_time:.2f} ms")
    print(f"  Max:        {max_time:.2f} ms")
    if throughput > 0:
        print(f"  Throughput: {throughput:.2f} Giter/sec")

    return {
        'mean': mean,
        'stddev': stddev,
        'cv': cv,
        'min': min_time,
        'max': max_time,
        'throughput': throughput,
        'samples': len(times),
        'metadata': metadata_cache if metadata_cache else {}
    }

def main():
    parser = argparse.ArgumentParser(
        description='Benchmark fractal flame renderer across commits'
    )
    parser.add_argument(
        '--config',
        default='assets/presets/complex.fflame',
        help='Path to .fflame config file'
    )
    parser.add_argument(
        '--repeats',
        type=int,
        default=5,
        help='Number of benchmark repeats per commit (to measure variance)'
    )
    parser.add_argument(
        '--width',
        type=int,
        default=1920,
        help='Render width'
    )
    parser.add_argument(
        '--height',
        type=int,
        default=1080,
        help='Render height'
    )
    parser.add_argument(
        '--output-dir',
        default='benchmark_results',
        help='Output directory for results'
    )

    args = parser.parse_args()

    # Commits to benchmark
    commits = [
        {'hash': 'dd80003', 'name': 'Before Histogram (textureStore)'},
        {'hash': 'ef0cdd8', 'name': 'Histogram Fixed (naive atomic)'},
        {'hash': '06bfcab', 'name': 'Histogram + Local Cache (current)'},
    ]

    # Setup
    results_dir = Path(args.output_dir)
    results_dir.mkdir(exist_ok=True)

    fractal_name = Path(args.config).stem
    timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
    platform_name = platform.system()
    resolution = f"{args.width}x{args.height}"

    print("="*60)
    print("Fractal Flame Performance Benchmark")
    print("="*60)
    print(f"Fractal:   {fractal_name}")
    print(f"Config:    {args.config}")
    print(f"Timestamp: {timestamp}")
    print(f"Platform:  {platform_name}")
    print(f"Repeats:   {args.repeats}")
    print(f"Resolution:{resolution}")
    print()
    print()
    print("⚠️  Safety check: Checking for uncommitted changes...")

    # Check for uncommitted changes to tracked files (would block checkout)
    returncode, stdout, _ = run_command("git diff-index --quiet HEAD --")
    has_changes = (returncode != 0)

    if has_changes:
        print("❌ ERROR: You have uncommitted changes to tracked files!")
        print("   Git will not allow checkout and you would lose work.")
        print()
        print("   Please commit or stash your changes first:")
        print("   - git add -A && git commit -m 'WIP'")
        print("   - git stash push -m 'Before benchmark'")
        print()
        sys.exit(1)

    print("✅ Working directory is clean - safe to proceed")
    print()
    print("⚠️  This script will temporarily checkout different commits.")
    print("   It will return to your original branch when finished.")
    print("   Untracked files in your working directory will NOT be touched.")

    # Store current branch/commit
    returncode, stdout, _ = run_command("git rev-parse --abbrev-ref HEAD")
    original_ref = stdout.strip()

    # If detached HEAD, get the commit hash instead
    if original_ref == 'HEAD':
        returncode, stdout, _ = run_command("git rev-parse HEAD")
        original_ref = stdout.strip()

    # Initialize CSV
    csv_path = results_dir / 'benchmark_history.csv'
    csv_exists = csv_path.exists()

    csv_headers = [
        'Timestamp', 'Fractal', 'ConfigFile', 'Resolution', 'Platform',
        'Commit', 'CommitName', 'TotalIterations', 'IterationsPerThread',
        'SpeedFactor', 'Workgroups', 'Mean_ms', 'StdDev_ms', 'CV_percent',
        'Min_ms', 'Max_ms', 'Throughput_Giter_sec', 'Samples',
        'RustcVersion', 'BuildProfile'
    ]

    # Run benchmarks
    results = []
    try:
        for commit in commits:
            result = run_benchmark(
                commit['hash'],
                commit['name'],
                args.config,
                args.repeats,
                args.width,
                args.height,
                results_dir,
                fractal_name
            )

            if result:
                results.append({
                    'commit': commit,
                    'stats': result
                })
    finally:
        # ALWAYS return to original branch, even if benchmarks fail
        print(f"\n{'='*60}")
        print("Returning to original branch/commit...")
        returncode, _, _ = run_command(f"git checkout {original_ref}")
        if returncode != 0:
            print(f"⚠️  WARNING: Failed to return to original ref: {original_ref}")
            print("   You may need to manually checkout your branch.")
        else:
            print(f"✅ Returned to: {original_ref}")

    # Write results to CSV
    if results:
        with open(csv_path, 'a', newline='', encoding='utf-8') as f:
            writer = csv.writer(f)

            # Write header if new file
            if not csv_exists:
                writer.writerow(csv_headers)

            # Write data rows
            for r in results:
                commit = r['commit']
                stats = r['stats']
                metadata = stats['metadata']

                row = [
                    timestamp,
                    fractal_name,
                    args.config,
                    resolution,
                    platform_name,
                    commit['hash'],
                    commit['name'],
                    metadata.get('total_iterations', ''),
                    metadata.get('iterations_per_thread', ''),
                    metadata.get('speed_factor', ''),
                    128,  # workgroups (default)
                    f"{stats['mean']:.2f}",
                    f"{stats['stddev']:.2f}",
                    f"{stats['cv']:.2f}",
                    f"{stats['min']:.2f}",
                    f"{stats['max']:.2f}",
                    f"{stats['throughput']:.2f}",
                    stats['samples'],
                    metadata.get('rustc_version', ''),
                    metadata.get('build_profile', ''),
                ]
                writer.writerow(row)

    print()
    print("✅ Benchmark complete!")
    print(f"Results saved to: {results_dir}")
    print(f"Benchmark History: {csv_path}")
    print()
    print("All benchmark data is appended to benchmark_history.csv for long-term tracking.")

if __name__ == '__main__':
    main()
