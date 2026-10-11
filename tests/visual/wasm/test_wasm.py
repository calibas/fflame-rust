# pylint: skip-file
#!/usr/bin/env python3
"""
WASM Visual Regression Testing

Tests the WASM build by:
1. Building the web app (build-wasm.bat on Windows, build-wasm.sh elsewhere)
2. Serving the repository root locally
3. Using Selenium to load test.html and render each config through
   WasmApi.export_png
4. Saving the PNG to tests/visual/current/wasm/<category>-<stem>.png
5. Comparing exact pixel hashes with tests/visual/baseline/wasm/

Tests are named `<category>-<stem>`, as run_tests.py names them: two
configs share the stem `affine3d-smoke` (3d/ and variations/), and a bare
stem made them overwrite each other's output and baseline.

A config with no baseline is reported as "no baseline", not as a pass:
nothing was compared. --update-baseline writes baselines for the configs
that rendered in this run.

IMPORTANT: All test configs must have deterministic_rng: true
"""

import subprocess
import sys
import time
import hashlib
import json
import shutil
from pathlib import Path
from typing import Dict, List, Optional
from PIL import Image
import io

# Selenium imports
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC

# Test configuration
CONFIG = {
    'build_dir': Path(__file__).parent.parent.parent.parent,  # Root of project
    'wasm_pkg': Path(__file__).parent.parent.parent.parent / 'pkg',
    'configs_dir': Path(__file__).parent.parent / 'configs',
    'current_dir': Path(__file__).parent.parent / 'current' / 'wasm',
    'baseline_dir': Path(__file__).parent.parent / 'baseline' / 'wasm',  # WASM has different hashes than desktop
    'port': 8080,
    'timeout': 600,  # 10 minutes per test (500M iterations in WASM is slow)
}

# Every category the web app can render and this suite can judge. The web
# build has all three engines (build-wasm links the default features), so
# `sim` is here; it was missing only because it postdates the list.
# `solid` is NOT here, deliberately: this suite compares EXACT pixel hashes,
# and solid rendering is not bit-reproducible (an in-batch depth race --
# run_tests.py compares those with a tolerance for the same reason), so
# every solid test would fail on noise.
CATEGORIES = ['2d', '3d', 'tonemap', 'variations', 'escape', 'sim']


class WasmTestRunner:
    def __init__(self):
        self.results = []
        self.server_process = None
        self.driver = None

    def build_wasm(self):
        """Build the web app with the platform's build script."""
        print('Building WASM...')

        # The two scripts are one procedure in two languages (RELEASE.md
        # §7). cmd runs the .bat; anything else has bash for the .sh.
        if sys.platform == 'win32':
            script = 'build-wasm.bat'
            cmd = ['cmd', '/c', script]
        else:
            script = 'build-wasm.sh'
            cmd = ['bash', script]
        result = subprocess.run(
            cmd,
            cwd=CONFIG['build_dir'],
            capture_output=True,
            text=True,
        )

        if result.returncode == 0:
            print('WASM build complete [OK]\n')
        else:
            tail = '\n'.join((result.stdout + result.stderr).strip().splitlines()[-20:])
            raise Exception(f'{script} failed:\n{tail}')

    def start_server(self):
        """Start HTTP server using python -m http.server"""
        print(f'Starting HTTP server on port {CONFIG["port"]}...')

        # Start server in background, with this interpreter: a bare
        # `python` is not on PATH everywhere (macOS and many Linuxes
        # have only python3).
        self.server_process = subprocess.Popen(
            [sys.executable, '-m', 'http.server', str(CONFIG['port'])],
            cwd=CONFIG['build_dir'],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        print(f'Server running at http://localhost:{CONFIG["port"]}/\n')
        time.sleep(1)  # Wait for server to start

    def launch_browser(self):
        """Launch Chrome browser with WebGPU support"""
        print('Launching browser...')

        options = Options()
        options.add_argument('--no-sandbox')
        options.add_argument('--disable-dev-shm-usage')
        options.add_argument('--window-size=1920,1080')

        # Enable WebGPU - use experimental flag that allows WebGPU
        options.add_argument('--enable-unsafe-webgpu')
        options.add_argument('--enable-features=Vulkan,UseSkiaRenderer')

        # Use D3D11 backend on Windows (more reliable than Vulkan for WebGPU).
        # ANGLE's D3D11 backend exists only there.
        if sys.platform == 'win32':
            options.add_argument('--use-angle=d3d11')
        options.add_argument('--use-cmd-decoder=passthrough')

        # Disable GPU blocklist to allow WebGPU even if GPU is "unsupported"
        options.add_argument('--ignore-gpu-blocklist')

        # Enable logging to capture console output
        options.set_capability('goog:loggingPrefs', {'browser': 'ALL'})

        try:
            self.driver = webdriver.Chrome(options=options)
            self.driver.set_page_load_timeout(30)  # 30 second page load timeout
            print('Browser launched [OK]\n')
        except Exception as e:
            raise Exception(f'Failed to launch Chrome: {e}\n'
                          'Make sure Chrome and chromedriver are installed.')

    def discover_configs(self, category_filter: Optional[str] = None) -> List[Dict]:
        """Discover test configs"""
        configs = []
        categories = CATEGORIES if category_filter is None else [category_filter]

        for category in categories:
            category_dir = CONFIG['configs_dir'] / category

            if not category_dir.exists():
                continue

            for config_file in sorted(category_dir.glob('*.fflame')):
                # Skip warmup config
                if config_file.name == 'warmup.fflame':
                    continue

                # Load and validate config
                with open(config_file, 'r') as f:
                    config_data = json.load(f)

                # Check for deterministic_rng
                if not config_data.get('deterministic_rng', False):
                    print(f'Warning: {config_file.name} missing deterministic_rng: true - skipping')
                    continue

                configs.append({
                    # `<category>-<stem>`, as run_tests.py names them.
                    'name': f'{category}-{config_file.stem}',
                    'path': config_file,
                    'category': category,
                    'config': config_data,
                })

        return configs

    def run_test(self, test_config: Dict) -> Dict:
        """Run a single WASM test.

        The result's `status` is 'pass', 'fail' or 'no-baseline'."""
        # A failed render must not leave an earlier run's PNG behind for
        # --update-baseline to promote as this run's output.
        output_path = CONFIG['current_dir'] / f'{test_config["name"]}.png'
        output_path.unlink(missing_ok=True)
        try:
            # Navigate to test page
            print(f'  Testing: {test_config["name"]}')
            url = f'http://localhost:{CONFIG["port"]}/tests/visual/wasm/test.html'
            print(f'    Loading {url}...')
            self.driver.get(url)
            print(f'    Page loaded')

            # Wait for WASM to initialize (wait for window.wasmReady === true)
            print(f'    Waiting for WASM to initialize...')
            wait = WebDriverWait(self.driver, CONFIG['timeout'])
            wait.until(
                lambda driver: driver.execute_script('return window.wasmReady === true')
            )
            print(f'    WASM initialized')

            # Load fractal config
            print(f'    Loading config...')
            config_json = json.dumps(test_config['config'])
            self.driver.execute_script(f'window.loadFractalConfig({config_json})')

            # Start render and wait for completion (async)
            print(f'    Starting render...')
            self.driver.set_script_timeout(CONFIG['timeout'])
            result = self.driver.execute_async_script('''
                const callback = arguments[arguments.length - 1];
                window.startRender().then(() => {
                    callback({success: true});
                }).catch(err => {
                    console.error('Render error:', err);
                    callback({success: false, error: String(err), message: err.message, stack: err.stack});
                });
            ''')

            # Check if render succeeded
            if not result.get('success'):
                raise Exception(f'Render failed: {result.get("error")}')

            print(f'    Render complete')

            # Print browser console logs for debugging
            self.print_browser_logs()

            # Get PNG data from WASM (returned as Uint8Array)
            png_data_js = self.driver.execute_script('''
                const pngData = window.getPngData();
                if (!pngData) {
                    throw new Error('PNG data is null or undefined - render may have failed silently');
                }
                return Array.from(pngData);
            ''')

            # Convert from JS array to Python bytes
            screenshot = bytes(png_data_js)

            # Save PNG
            CONFIG['current_dir'].mkdir(parents=True, exist_ok=True)
            with open(output_path, 'wb') as f:
                f.write(screenshot)

            # Calculate hash
            img_hash = self.hash_image_data(screenshot)

            # Extract performance metadata from PNG
            perf_stats = self.extract_performance_metadata(screenshot)

            # Compare with the WASM baseline (WASM hashes differ from
            # desktop's, so it has its own)
            baseline_path = CONFIG['baseline_dir'] / f'{test_config["name"]}.png'
            baseline_hash = None
            status = 'pass'
            error = None

            if baseline_path.exists():
                with open(baseline_path, 'rb') as f:
                    baseline_hash = self.hash_image_data(f.read())

                if img_hash != baseline_hash:
                    status = 'fail'
                    error = f'Hash mismatch: baseline {baseline_hash[:8]}..., got {img_hash[:8]}...'
            else:
                # Nothing was compared, so this is not a pass. Nothing is
                # written either, unless --update-baseline asks for it.
                status = 'no-baseline'
                error = f'No baseline at {baseline_path} (--update-baseline writes one)'

            return {
                'name': test_config['name'],
                'status': status,
                'passed': status == 'pass',
                'hash': img_hash,
                'baseline_hash': baseline_hash,
                'error': error,
                'perf_stats': perf_stats,
            }

        except Exception as e:
            return {
                'name': test_config['name'],
                'status': 'fail',
                'passed': False,
                'hash': '',
                'baseline_hash': None,
                'error': f'Test failed: {str(e)}',
                'perf_stats': None,
            }

    def hash_image_data(self, png_data: bytes) -> str:
        """Hash raw pixel data (not PNG file) using PIL"""
        img = Image.open(io.BytesIO(png_data))
        import numpy as np
        pixels = np.array(img)
        return hashlib.sha256(pixels.tobytes()).hexdigest()

    def extract_performance_metadata(self, png_data: bytes) -> Dict:
        """Extract performance statistics from PNG tEXt chunks"""
        try:
            img = Image.open(io.BytesIO(png_data))

            # Extract PNG tEXt chunks (individual text metadata)
            metadata = img.info

            # Parse individual text chunks
            # RenderTime format: "123.45ms"
            render_time_str = metadata.get('RenderTime', '0ms')
            render_time_ms = float(render_time_str.replace('ms', '').strip())

            # Iterations is a plain number string
            total_iterations = int(metadata.get('Iterations', '0'))

            # Resolution format: "800x600"
            resolution = metadata.get('Resolution', '0x0')
            width, height = map(int, resolution.split('x'))

            # Calculate iterations per second
            iterations_per_second = 0
            if render_time_ms > 0:
                iterations_per_second = total_iterations / (render_time_ms / 1000.0)

            return {
                'render_time_ms': render_time_ms,
                'total_iterations': total_iterations,
                'iterations_per_second': iterations_per_second,
                'width': width,
                'height': height,
            }
        except Exception as e:
            print(f'    Warning: Failed to extract performance metadata: {e}')
            return {
                'render_time_ms': 0,
                'total_iterations': 0,
                'iterations_per_second': 0,
                'width': 0,
                'height': 0,
            }

    def print_browser_logs(self):
        """Print browser console logs for debugging"""
        try:
            logs = self.driver.get_log('browser')
            if logs:
                print('    --- Browser Console Logs ---')
                for entry in logs:
                    level = entry.get('level', 'INFO')
                    message = entry.get('message', '')
                    # Filter to show only WASM/Render related logs
                    if 'Render' in message or 'tonemap' in message or 'read_fractal' in message or 'FlameRenderer' in message or 'WASM' in message:
                        print(f'    [{level}] {message}')
                print('    --- End Browser Logs ---')
        except Exception as e:
            print(f'    Warning: Could not get browser logs: {e}')

    def run_all_tests(self, category_filter: Optional[str] = None) -> bool:
        """Run all tests"""
        configs = self.discover_configs(category_filter)

        if not configs:
            print('No test configs found!')
            return False

        print(f'Running {len(configs)} WASM visual regression tests...')
        print('=' * 60)

        for config in configs:
            result = self.run_test(config)
            self.results.append(result)
            self.print_result(result)

        return self.print_summary()

    def print_result(self, result: Dict):
        """Print single test result"""
        status = {'pass': '[PASS]', 'fail': '[FAIL]',
                  'no-baseline': '[NONE]'}[result['status']]

        # Format performance stats if available
        perf_str = ''
        if 'perf_stats' in result and result['perf_stats']:
            stats = result['perf_stats']
            if stats['render_time_ms'] > 0:
                time_sec = stats['render_time_ms'] / 1000.0
                iters_mil = stats['total_iterations'] / 1_000_000.0
                iters_per_sec_mil = stats['iterations_per_second'] / 1_000_000.0
                perf_str = f' ({time_sec:.2f}s, {iters_mil:.1f}M iters, {iters_per_sec_mil:.1f}M/s)'

        print(f'{status:<8} {result["name"]:<30}{perf_str}')

        if result['error']:
            print(f'         {result["error"]}')

    def print_summary(self) -> bool:
        """Print summary"""
        passed = sum(1 for r in self.results if r['status'] == 'pass')
        failed = sum(1 for r in self.results if r['status'] == 'fail')
        missing = sum(1 for r in self.results if r['status'] == 'no-baseline')

        print('\n' + '=' * 60)
        print(f'WASM Tests: {passed} passed, {failed} failed, {missing} no baseline, '
              f'{len(self.results)} total')

        # Performance summary
        total_render_time = 0
        total_iterations = 0
        test_count = 0

        for r in self.results:
            if 'perf_stats' in r and r['perf_stats']:
                stats = r['perf_stats']
                if stats['render_time_ms'] > 0:
                    total_render_time += stats['render_time_ms']
                    total_iterations += stats['total_iterations']
                    test_count += 1

        if test_count > 0:
            avg_time = total_render_time / test_count / 1000.0
            avg_iters_per_sec = (total_iterations / test_count) / (total_render_time / test_count / 1000.0)
            print(f'\nPerformance Summary:')
            print(f'  Total render time: {total_render_time / 1000.0:.2f}s')
            print(f'  Total iterations:  {total_iterations / 1_000_000.0:.1f}M')
            print(f'  Avg time/test:     {avg_time:.2f}s')
            print(f'  Avg throughput:    {avg_iters_per_sec / 1_000_000.0:.1f}M iterations/sec')

        if failed > 0:
            print('\nFailed tests:')
            for r in self.results:
                if r['status'] == 'fail':
                    print(f'  - {r["name"]}: {r["error"]}')
        if missing > 0:
            print('\nNo baseline (rendered, not compared):')
            for r in self.results:
                if r['status'] == 'no-baseline':
                    print(f'  - {r["name"]}')

        return failed == 0

    def cleanup(self):
        """Cleanup resources"""
        if self.driver:
            self.driver.quit()
        if self.server_process:
            self.server_process.terminate()
            self.server_process.wait()


def main():
    import argparse

    parser = argparse.ArgumentParser(description='WASM Visual Regression Tests')
    parser.add_argument('--update-baseline', action='store_true',
                        help='Write baselines for the configs that rendered in this run')
    parser.add_argument('--category', choices=CATEGORIES,
                        help='Run only the tests in this category')
    args = parser.parse_args()

    runner = WasmTestRunner()

    try:
        # Build WASM
        runner.build_wasm()

        # Start server
        runner.start_server()

        # Launch browser
        runner.launch_browser()

        # Run tests
        success = runner.run_all_tests(args.category)

        # Update baselines if requested: only this run's renders (each
        # test deletes its old output first, so a PNG here is this run's).
        if args.update_baseline:
            print('\nUpdating WASM baselines...')
            CONFIG['baseline_dir'].mkdir(parents=True, exist_ok=True)
            for result in runner.results:
                current = CONFIG['current_dir'] / f'{result["name"]}.png'
                baseline = CONFIG['baseline_dir'] / f'{result["name"]}.png'
                if result['hash'] and current.exists():
                    shutil.copy(current, baseline)
                    print(f'  Updated baseline: {result["name"]}.png')
            print('Baselines updated!')
        elif any(r['status'] == 'no-baseline' for r in runner.results):
            # A run that compared nothing for some configs is not a pass.
            success = False

        # Cleanup
        runner.cleanup()

        # Exit with appropriate code
        sys.exit(0 if success else 1)

    except Exception as e:
        print(f'Fatal error: {e}')
        runner.cleanup()
        sys.exit(1)


if __name__ == '__main__':
    main()
