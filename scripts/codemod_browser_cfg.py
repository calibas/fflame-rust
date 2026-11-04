"""One-shot codemod: code that calls into the web page says `browser`.

`browser` (set by build.rs) is wasm32 minus WASI. Until WASI
(wasm/python) the two meant the same, so `target_arch = "wasm32"`
stood for both "single-threaded and async" and "has a page, wasm-bindgen
and web-sys". This rewrites only the sites of the second kind -- the
ones a WASI build cannot compile -- named by file and the line's text,
and asserts each is found. Nothing changes for the desktop or the
browser.

Run from the repository root: python scripts/codemod_browser_cfg.py
"""

import pathlib

WASM = '#[cfg(target_arch = "wasm32")]'
BROWSER = "#[cfg(browser)]"

# (file, the line that follows the cfg attribute, how many such sites)
SITES = [
    ("src/lib.rs", "mod web_clipboard;", 1),
    ("src/lib.rs", "mod web_text_agent;", 1),
    ("src/lib.rs", "use winit::dpi::PhysicalSize;", 1),
    ("src/lib.rs", "use wasm_bindgen::prelude::*;", 1),
    ("src/lib.rs", "fn parse_url_load_params() -> (Option<String>, Option<String>) {", 1),
    ("src/audio/mod.rs", "mod playback_wasm;", 1),
    ("src/audio/mod.rs", "mod capture_wasm;", 1),
    ("src/audio/mod.rs", "pub use playback_wasm::{AudioPlayer, PlaybackError, PlaybackState};", 1),
    ("src/audio/mod.rs", "pub use capture_wasm::AudioCapture;", 1),
    ("src/storage/backend.rs", "pub fn read_file(key: &Path) -> StorageResult<String> {", 1),
    ("src/storage/backend.rs", "pub fn write_file(key: &Path, content: &str) -> StorageResult<()> {", 1),
    ("src/storage/backend.rs", "pub fn delete_file(key: &Path) -> StorageResult<()> {", 1),
    ("src/storage/backend.rs", "pub fn file_exists(key: &Path) -> bool {", 1),
    ("src/storage/backend.rs", "pub fn list_entries(prefix: &Path) -> StorageResult<Vec<String>> {", 1),
    ("src/resources/fetch.rs", "mod wasm {", 1),
    ("src/resources/fetch.rs", "pub use wasm::*;", 1),
    ("src/api/client.rs", "mod wasm {", 1),
    ("src/api/client.rs", "pub use wasm::{api_put_binary_raw, api_request, api_request_raw};", 1),
]

files = {}
for rel, following, count in SITES:
    if rel not in files:
        raw = pathlib.Path(rel).read_bytes().decode("utf-8")
        files[rel] = (raw, "\r\n" if "\r\n" in raw else "\n")
    raw, nl = files[rel]
    old = WASM + nl + following
    assert raw.count(old) == count, (rel, following, raw.count(old))
    files[rel] = (raw.replace(old, BROWSER + nl + following), nl)

for rel, (raw, _) in files.items():
    pathlib.Path(rel).write_bytes(raw.encode("utf-8"))
    print(rel)
