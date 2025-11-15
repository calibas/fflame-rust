#!/usr/bin/env python3
"""
WASM Visual Regression Testing

Tests the WASM build by:
1. Building wasm-pack
2. Serving the web page locally
3. Using Selenium to load and render fractals
4. Capturing canvas to PNG
5. Comparing pixel hashes with desktop baselines

IMPORTANT: All test configs must have deterministic_rng: true
"""

import subprocess
import time
import hashlib
import json
import http.server
import socketserver
import threading
from pathlib import Path
from typing import Dict, List, Optional
from PIL import Image
import io
import base64

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
    'baseline_dir': Path(__file__).parent.parent / 'baseline',
    'port': 8080,
    'timeout': 60,  # 60 seconds per test
}


class WasmTestRunner:
    def __init__(self):
        self.results = []
        self.server = None
        self.server_thread = None
        self.driver = None

    def build_wasm(self):
        """Build WASM package with wasm-pack"""
        print('Building WASM package...')

        try:
            result = subprocess.run(
                ['wasm-pack', 'build', '--target', 'web', '--release'],
                cwd=CONFIG['build_dir'],
                capture_output=True,
                text=True,
                timeout=300,  # 5 minutes max
            )

            if result.returncode == 0:
                print('WASM build complete [OK]\n')
            else:
                raise Exception(f'WASM build failed with exit code {result.returncode}:\n{result.stderr}')

        except FileNotFoundError:
            raise Exception('wasm-pack not found. Install with: cargo install wasm-pack')
        except subprocess.TimeoutExpired:
            raise Exception('WASM build timed out after 5 minutes')

    def start_server(self):
        """Start local HTTP server in background thread"""
        print(f'Starting HTTP server on port {CONFIG["port"]}...')

        build_dir = str(CONFIG['build_dir'])

        class Handler(http.server.SimpleHTTPRequestHandler):
            def __init__(self, *args, **kwargs):
                super().__init__(*args, directory=build_dir, **kwargs)

            def log_message(self, format, *args):
                pass  # Suppress logging

        self.server = socketserver.TCPServer(("", CONFIG['port']), Handler, bind_and_activate=False)
        self.server.allow_reuse_address = True
        self.server.server_bind()
        self.server.server_activate()

        # Run server in background thread
        self.server_thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.server_thread.start()

        print(f'Server running at http://localhost:{CONFIG["port"]}/\n')

        # Wait a moment for server to fully start
        time.sleep(0.5)

    def launch_browser(self):
        """Launch headless Chrome browser with Selenium"""
        print('Launching headless browser...')

        options = Options()
        options.add_argument('--headless')
        options.add_argument('--no-sandbox')
        options.add_argument('--disable-dev-shm-usage')
        options.add_argument('--disable-gpu')
        options.add_argument('--window-size=1920,1080')

        try:
            self.driver = webdriver.Chrome(options=options)
            print('Browser launched [OK]\n')
        except Exception as e:
            raise Exception(f'Failed to launch Chrome: {e}\n'
                          'Make sure Chrome and chromedriver are installed.')

    def discover_configs(self) -> List[Dict]:
        """Discover test configs"""
        configs = []
        categories = ['2d', '3d', 'tonemap', 'variations']

        for category in categories:
            category_dir = CONFIG['configs_dir'] / category

            if not category_dir.exists():
                continue

            for config_file in category_dir.glob('*.fflame'):
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
                    'name': config_file.stem,
                    'path': config_file,
                    'category': category,
                    'config': config_data,
                })

        return configs

    def run_test(self, test_config: Dict) -> Dict:
        """Run a single WASM test"""
        try:
            # Navigate to test page
            url = f'http://localhost:{CONFIG["port"]}/tests/visual/wasm/test.html'
            self.driver.get(url)

            # Wait for WASM to initialize (wait for window.wasmReady === true)
            wait = WebDriverWait(self.driver, CONFIG['timeout'])
            wait.until(
                lambda driver: driver.execute_script('return window.wasmReady === true')
            )

            # Load fractal config
            config_json = json.dumps(test_config['config'])
            self.driver.execute_script(f'window.loadFractalConfig({config_json})')

            # Start render
            self.driver.execute_script('window.startRender()')

            # Wait for render to complete
            wait.until(
                lambda driver: driver.execute_script('return window.renderComplete === true'),
                message=f'Render timeout for {test_config["name"]}'
            )

            # Get PNG data from WASM (returned as Uint8Array)
            png_data_js = self.driver.execute_script('return Array.from(window.getPngData());')

            # Convert from JS array to Python bytes
            screenshot = bytes(png_data_js)

            # Save PNG
            CONFIG['current_dir'].mkdir(parents=True, exist_ok=True)
            output_path = CONFIG['current_dir'] / f'{test_config["name"]}.png'
            with open(output_path, 'wb') as f:
                f.write(screenshot)

            # Calculate hash
            img_hash = self.hash_image_data(screenshot)

            # Compare with desktop baseline
            baseline_path = CONFIG['baseline_dir'] / f'{test_config["name"]}.png'
            baseline_hash = None
            passed = True
            error = None

            if baseline_path.exists():
                with open(baseline_path, 'rb') as f:
                    baseline_hash = self.hash_image_data(f.read())

                if img_hash != baseline_hash:
                    passed = False
                    error = f'Hash mismatch: baseline {baseline_hash[:8]}..., got {img_hash[:8]}...'
            else:
                print(f'Note: No baseline for {test_config["name"]} - creating first baseline')

            return {
                'name': test_config['name'],
                'passed': passed,
                'hash': img_hash,
                'baseline_hash': baseline_hash,
                'error': error,
            }

        except Exception as e:
            return {
                'name': test_config['name'],
                'passed': False,
                'hash': '',
                'baseline_hash': None,
                'error': f'Test failed: {str(e)}',
            }

    def hash_image_data(self, png_data: bytes) -> str:
        """Hash raw pixel data (not PNG file) using PIL"""
        img = Image.open(io.BytesIO(png_data))
        import numpy as np
        pixels = np.array(img)
        return hashlib.sha256(pixels.tobytes()).hexdigest()

    def run_all_tests(self) -> bool:
        """Run all tests"""
        configs = self.discover_configs()

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
        status = '[PASS]' if result['passed'] else '[FAIL]'
        print(f'{status:<8} {result["name"]:<30}')

        if result['error']:
            print(f'         {result["error"]}')

    def print_summary(self) -> bool:
        """Print summary"""
        passed = sum(1 for r in self.results if r['passed'])
        failed = len(self.results) - passed

        print('\n' + '=' * 60)
        print(f'WASM Tests: {passed} passed, {failed} failed, {len(self.results)} total')

        if failed > 0:
            print('\nFailed tests:')
            for r in self.results:
                if not r['passed']:
                    print(f'  - {r["name"]}: {r["error"]}')

        return failed == 0

    def cleanup(self):
        """Cleanup resources"""
        if self.driver:
            self.driver.quit()
        if self.server:
            self.server.shutdown()


def main():
    runner = WasmTestRunner()

    try:
        # Build WASM
        runner.build_wasm()

        # Start server
        runner.start_server()

        # Launch browser
        runner.launch_browser()

        # Run tests
        success = runner.run_all_tests()

        # Cleanup
        runner.cleanup()

        # Exit with appropriate code
        exit(0 if success else 1)

    except Exception as e:
        print(f'Fatal error: {e}')
        runner.cleanup()
        exit(1)


if __name__ == '__main__':
    main()
