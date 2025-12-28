# WASM File Operations Cleanup

## Status: Implementation Complete - Testing Pending

## Problem

WASM file operations have several UX issues due to workarounds for async file dialogs:

1. **Config load** - Copies JSON to clipboard instead of loading (broken)
2. **PNG export** - Shows intermediate dialog instead of direct download
3. **Inconsistent patterns** - Some operations use egui temp storage, others use clipboard hacks

## Changes Made

### Fix 1: Config Load ✅

Replaced clipboard hack with egui temp storage pattern:
- Parse JSON in async block, store in `egui::Id::new("pending_config_load")`
- Render loop checks for pending config and calls `load_config_with_undo()`
- Also fixed file filter: was `.flame` (wrong), now `.fflame` (correct)

### Fix 2: PNG Export Direct Download ✅

Added `trigger_browser_download()` helper function that:
- Creates Blob from PNG bytes
- Creates object URL
- Creates anchor element and triggers click for download
- Cleans up object URL

Both custom size export and viewport export now use direct browser download instead of save dialog.

### Dependencies Added

In `Cargo.toml` for WASM target:
- `js-sys = "0.3"`
- web-sys features: `Blob`, `BlobPropertyBag`, `Url`, `HtmlAnchorElement`

## Files Modified

- `src/app/mod.rs`:
  - Added `trigger_browser_download()` helper function
  - Fixed config load to use egui temp storage
  - Fixed file filter from `.flame` to `.fflame`
  - Updated both PNG export paths to use direct download
  - Added pending config check in render loop

- `Cargo.toml`:
  - Added `js-sys` dependency
  - Added web-sys features for Blob API

## Testing

- [ ] WASM: Load .fflame config file
- [ ] WASM: Export PNG triggers browser download (viewport size)
- [ ] WASM: Export PNG triggers browser download (custom size)
- [ ] WASM: Apophysis import still works
- [ ] WASM: Palette load/save still works
- [ ] Desktop: All file operations unchanged (verified - builds pass)
