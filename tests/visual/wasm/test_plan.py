# pylint: skip-file
#!/usr/bin/env python3
"""
The browser's gate for phase 3 of docs/projects/gpu-cylinder-planning.md.

A cylinder plan is made in the browser the way the app makes one on the
web -- a future polled a slice at a time, the GPU's answers awaited across
frames -- and the page drives it one step per animation frame
(tests/visual/wasm/plan.html). Every step (the planner's share of a frame)
and every frame interval is timed.

Gate: no step over 16 ms. Frame intervals are reported: they include the
browser's own work, and the editor's -- the module's start function boots
the whole app on the page's canvas, so it runs beside the bench -- so they
are context, not the gate. Frames over 50 ms are attributed by the browser
(Long Animation Frames): a script in plan.html is the bench's, one in
fractal_flame_wgpu.js is the editor's.

Expect one long frame (~40-50 ms) in a flame's first cold case: the GPU
process compiles the planner's kernels -- synchronously, since wgpu has no
async pipeline creation -- on the thread that also serves the editor's
rendering, and the editor's next frame waits for it. Traced and explained
in docs/projects/gpu-cylinder-planning.md §17.

The first case waits --settle seconds after the page loads, so the
editor's boot is over before any plan starts.

Build first: build-wasm.bat (or ./build-wasm.sh), which writes ./pkg.
Usage: python tests/visual/wasm/test_plan.py [--slice 6]
"""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.support.ui import WebDriverWait

ROOT = Path(__file__).parent.parent.parent.parent
PORT = 8081

CASES = [
    # (label, config, zoom multiple, cold)
    ('saved view, cold', ROOT / 'output' / 'grand-julian-missing-pieces.fflame', 1.0, True),
    ('saved view, warm', ROOT / 'output' / 'grand-julian-missing-pieces.fflame', 1.0, False),
    ('10x deeper, warm', ROOT / 'output' / 'grand-julian-missing-pieces.fflame', 10.0, False),
    # The flame whose analysis was the slowest (random1: 26 ms natively).
    ('random1, cold', ROOT / 'output' / 'flame-zoom' / 'random1.fflame', 1.0, True),
    ('random1 100x, warm', ROOT / 'output' / 'flame-zoom' / 'random1.fflame', 100.0, False),
]


def stats(xs):
    s = sorted(xs)
    return {
        'n': len(s),
        'p50': s[len(s) // 2],
        'p95': s[min(len(s) - 1, int(len(s) * 0.95))],
        'max': s[-1],
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--slice', type=float, default=6.0, help='ms a step may run')
    ap.add_argument('--settle', type=float, default=3.0, help='seconds to let the editor boot first')
    args = ap.parse_args()

    server = subprocess.Popen(
        [sys.executable, '-m', 'http.server', str(PORT)],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(1)
    options = Options()
    options.add_argument('--no-sandbox')
    options.add_argument('--disable-dev-shm-usage')
    options.add_argument('--window-size=1280,800')
    options.add_argument('--enable-unsafe-webgpu')
    options.add_argument('--enable-features=Vulkan,UseSkiaRenderer')
    options.add_argument('--use-angle=d3d11')
    options.add_argument('--ignore-gpu-blocklist')
    options.set_capability('goog:loggingPrefs', {'browser': 'ALL'})
    driver = webdriver.Chrome(options=options)
    failed = False
    try:
        driver.set_script_timeout(600)
        driver.get(f'http://localhost:{PORT}/tests/visual/wasm/plan.html')
        WebDriverWait(driver, 120).until(
            lambda d: d.execute_script('return window.benchReady === true || window.benchError !== null')
        )
        err = driver.execute_script('return window.benchError')
        if err:
            raise RuntimeError(f'page failed: {err}')
        time.sleep(args.settle)
        for label, path, mult, cold in CASES:
            if not path.exists():
                print(f'== {label}: {path} missing, skipped')
                continue
            config = path.read_text()
            r = driver.execute_async_script(
                '''
                const done = arguments[arguments.length - 1];
                window.runPlan(arguments[0], arguments[1], arguments[2], arguments[3])
                    .then(done)
                    .catch(e => done({error: String(e), steps: [0], frames: [0], total: 0, words: -1}));
                ''',
                config,
                mult,
                args.slice,
                cold,
            )
            st = stats(r['steps'])
            fr = stats(r['frames'][1:] or [0])
            over = sum(1 for x in r['steps'] if x > 16.0)
            print(
                f"== {label}: {r['words']} words in {r['total'] / 1000:.2f} s, {st['n']} frames | "
                f"step p50 {st['p50']:.1f} p95 {st['p95']:.1f} max {st['max']:.1f} ms, {over} over 16 ms | "
                f"frame interval p50 {fr['p50']:.1f} p95 {fr['p95']:.1f} max {fr['max']:.1f} ms"
                + (f" | error: {r['error']}" if r.get('error') else '')
            )
            for line in (r.get('trace') or '').splitlines():
                print('   long:', line)
            # Frame intervals past two vsyncs, and the browser's own account
            # of any frame over 50 ms.
            t = 0.0
            for i, f in enumerate(r['frames']):
                t += f
                if i > 0 and f > 33.4:
                    print(f'   long frame interval: {f:.1f} ms at {t:.0f} ms (frame {i}, step {r["steps"][i]:.1f} ms)')
            for e in r.get('loaf') or []:
                scripts = '; '.join(
                    f"{s['invoker']} {s['duration']:.1f} ms ({s.get('source', '').strip() or '?'})" for s in e.get('scripts') or []
                )
                print(
                    f"   long animation frame at {e['start']:.0f} ms: {e['duration']:.1f} ms, "
                    f"blocking {e['blocking']:.1f} ms, render {e.get('render', 0):.1f} ms | scripts: {scripts or 'none'}"
                )
            if r['words'] < 0 or over > 0:
                failed = True
        for entry in driver.get_log('browser'):
            if entry['level'] in ('SEVERE',):
                print('   console:', entry['message'][:300])
    finally:
        driver.quit()
        server.terminate()
    if failed:
        print('FAILED: a step ran past 16 ms, or a plan failed')
        sys.exit(1)
    print('ok')


if __name__ == '__main__':
    main()
