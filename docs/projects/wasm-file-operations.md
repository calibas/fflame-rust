# WASM File Operations Cleanup

## Status: Implementation Complete - Testing Pending

## Problem

WASM file operations have several UX issues due to workarounds for async file dialogs:

1. **Config load** - Copies JSON to clipboard instead of loading (broken)
2. **PNG export** - Shows intermediate dialog instead of direct download
3. **Extra confirmation dialog** - rfd crate shows unnecessary dialog after file selection
4. **Config Save As** - Uses rfd which requires extra click
5. **Animation Save/Load** - Not working in WASM at all
6. **File Browser** - Uses rfd (extra dialog) and pickup code was missing
7. **Inconsistent patterns** - Some operations use egui temp storage, others use clipboard hacks

## Changes Made

### Fix 1: Config Load ✅

Replaced clipboard hack with native browser file picker:
- Uses `<input type="file">` element directly (no rfd)
- FileReader API reads file contents as text
- Stores raw JSON in `egui::Id::new("pending_config_load_raw")`
- Render loop parses JSON and calls `load_config_with_undo()`
- Also fixed file filter: was `.flame` (wrong), now `.fflame` (correct)

### Fix 2: PNG Export Direct Download ✅

Added `trigger_browser_download()` helper function that:
- Creates Blob from PNG bytes
- Creates object URL
- Creates anchor element and triggers click for download
- Cleans up object URL

Both custom size export and viewport export now use direct browser download instead of save dialog.

### Fix 3: Native File Picker (No Extra Dialogs) ✅

Added `trigger_browser_file_picker()` helper function that:
- Creates hidden `<input type="file">` element
- Sets accept filter (e.g., `.fflame`, `.flame`)
- Attaches FileReader to read contents
- Stores raw text in egui temp storage for render loop pickup
- No extra confirmation dialogs (unlike rfd)

Updated both config load and Apophysis import to use native file picker.

### Fix 4: Config Save As ✅

Changed WASM config save to use `trigger_browser_download()`:
- Direct browser download with auto-generated filename
- No extra confirmation dialogs

### Fix 5: Animation Save/Load ✅

Added WASM support for animation panel Save/Load buttons:
- **Save**: Uses `trigger_browser_download()` for direct download
- **Load**: Uses `trigger_browser_file_picker()` with `.anim,.json` filter
- Render loop parses animation JSON and loads with embedded config support

### Fix 6: File Browser Panel ✅

Fixed WASM file browser to use native file picker:
- Uses `trigger_browser_file_picker()` with `.fflame` filter
- Added missing pickup code for `pending_file_browser_json_raw`
- Render loop parses multi-config JSON and loads into file browser panel
- Supports both single and multi-config .fflame files

### Dependencies Added

In `Cargo.toml` for WASM target:
- `js-sys = "0.3"`
- web-sys features: `Blob`, `BlobPropertyBag`, `Url`, `HtmlAnchorElement`, `HtmlInputElement`, `FileList`, `File`, `FileReader`, `Event`, `InputEvent`

## Files Modified

- `src/app/mod.rs`:
  - Added `trigger_browser_download()` helper function (now public)
  - Added `trigger_browser_file_picker()` helper function (now public)
  - Config load uses native file picker with `pending_config_load_raw`
  - Config save uses `trigger_browser_download()` for direct download
  - Apophysis import uses native file picker with `pending_apophysis_import_raw`
  - Animation load uses native file picker with `pending_animation_load_raw`
  - File browser uses native file picker with `pending_file_browser_json_raw`
  - Updated both PNG export paths to use direct download
  - Render loop parses raw text from native file picker results

- `src/ui/animation_panel.rs`:
  - Added `trigger_animation_load` field to `AnimationPanelResponse` (WASM only)
  - Load button triggers native file picker in WASM

- `src/ui/panel_viewer.rs`:
  - Animation save uses `trigger_browser_download()` in WASM
  - Animation load trigger handled via native file picker

- `Cargo.toml`:
  - Added `js-sys` dependency
  - Added web-sys features for Blob API and file input

## Testing

- [ ] WASM: Load .fflame config file (no extra dialog)
- [ ] WASM: Save As triggers browser download (no extra dialog)
- [ ] WASM: Export PNG triggers browser download (viewport size)
- [ ] WASM: Export PNG triggers browser download (custom size)
- [ ] WASM: Apophysis import works (no extra dialog)
- [ ] WASM: Animation Load works (no extra dialog)
- [ ] WASM: Animation Save triggers browser download
- [ ] WASM: File Browser - Open File loads multi-config .fflame
- [ ] WASM: Palette load/save still works
- [ ] Desktop: All file operations unchanged (verified - builds pass)

## Note: Preset Library

The "From Preset Library..." menu item opens the Preset Library panel which shows built-in presets. This works correctly - it doesn't try to load from a folder.

To load external .fflame files with multiple configs, use the File Browser panel (Window → File Browser).
