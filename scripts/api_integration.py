#!/usr/bin/env python3
"""The API integration test: the client's own calls against a local ffapi.

    python scripts/api_integration.py [--ffapi ../ffapi] [--skip-build]

Builds the API (debug), starts it on a free loopback port, runs the client's
ignored `api::integration_tests` against it (`FFLAME_API_URL`), and stops
the server whatever happens. Exit status is the tests'.

The server uses the database in ffapi's own `.env` (the one its test suite
uses), and this script sets, for that run only:

  * EMAIL_API_TOKEN empty and EMAIL_API_URL a dead local port: no email is
    sent anywhere, and new accounts register verified (`.env` loading never
    overrides a variable already set);
  * APP_LATEST_VERSION 99.0.0, which the update check's test expects;
  * LOG_DIR and THUMBNAIL_DIR under output/api_integration/, so nothing is
    written into the API's working tree.

The server's log is output/api_integration/server.log.
See docs/projects/api-updates.md, decision 7.
"""

import argparse
import os
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "output", "api_integration")


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def wait_for(url, proc, seconds):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if proc.poll() is not None:
            return False
        try:
            with urllib.request.urlopen(url, timeout=2) as r:
                if r.status == 200:
                    return True
        except OSError:
            time.sleep(0.5)
    return False


def tail(path, n=30):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return "".join(f.readlines()[-n:])
    except OSError:
        return "(no log)"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ffapi", default=os.path.join(ROOT, "..", "ffapi"), help="the API repository")
    ap.add_argument("--skip-build", action="store_true", help="use the API binary as built")
    args = ap.parse_args()
    ffapi = os.path.abspath(args.ffapi)
    exe = os.path.join(ffapi, "target", "debug", "ffapi" + (".exe" if os.name == "nt" else ""))

    if not args.skip_build:
        print("building the API...", flush=True)
        if subprocess.run(["cargo", "build", "-q"], cwd=ffapi).returncode != 0:
            print("the API did not build")
            return 1
    if not os.path.exists(exe):
        print(f"no API binary at {exe}")
        return 1

    os.makedirs(os.path.join(OUT, "logs"), exist_ok=True)
    os.makedirs(os.path.join(OUT, "thumbnails"), exist_ok=True)
    port = free_port()
    base = f"http://127.0.0.1:{port}"
    env = dict(os.environ)
    env.update({
        "HOST": "127.0.0.1",
        "PORT": str(port),
        "EMAIL_API_TOKEN": "",
        "EMAIL_API_URL": "http://127.0.0.1:9/emails",
        "APP_LATEST_VERSION": "99.0.0",
        "APP_LATEST_URL": "",
        "LOG_DIR": os.path.join(OUT, "logs"),
        "THUMBNAIL_DIR": os.path.join(OUT, "thumbnails"),
        "ENABLE_ACTIVITY_LOG": "false",
        "COOKIE_SECURE": "false",
    })
    log_path = os.path.join(OUT, "server.log")
    with open(log_path, "w", encoding="utf-8") as log:
        print(f"starting the API on {base}...", flush=True)
        server = subprocess.Popen([exe], cwd=ffapi, env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            if not wait_for(f"{base}/health", server, 90):
                print("the API did not come up; its log ends:\n" + tail(log_path))
                return 1
            print("running the integration tests...", flush=True)
            test_env = dict(os.environ, FFLAME_API_URL=base)
            rc = subprocess.run(
                ["cargo", "test", "--release", "--lib", "--", "--ignored", "--test-threads=1", "api::integration_tests"],
                cwd=ROOT,
                env=test_env,
            ).returncode
            if rc != 0:
                print("\nthe API's log ends:\n" + tail(log_path))
            return rc
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()


if __name__ == "__main__":
    sys.exit(main())
