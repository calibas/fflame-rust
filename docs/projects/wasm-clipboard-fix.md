# WASM Clipboard Fix

## Problem

Copy/paste between the system clipboard and the WASM app doesn't work in Firefox (and possibly other browsers). The "Export to Clipboard" and "Import from JSON" flows in Config Import/Export and Palette Editor are affected. Clipboard operations only work *within* the egui app, not with the OS clipboard.

## Root Cause

**The `clipboard` feature is disabled on WASM.** In `Cargo.toml`:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
egui-winit = { version = "0.33", default-features = false }
```

egui-winit's default features include `"clipboard"`, which enables cut/copy/paste to the OS clipboard. With `default-features = false`, egui-winit falls back to a **simulated in-memory clipboard** that only works within the app.

The native (desktop) dependency uses `default-features = true`, which is why clipboard works fine on desktop.

### Secondary Issues (Firefox-specific)

Even after enabling the clipboard feature, Firefox has additional restrictions:

1. **`navigator.clipboard.readText()` requires user activation** - Firefox is stricter than Chrome about what counts as a trusted user gesture from a `<canvas>` element. Paste via the Clipboard API may still fail.

2. **HTTPS requirement** - The Clipboard API requires a secure context. `localhost` counts as secure, but serving over plain HTTP from another hostname will not work. The build scripts suggest `python -m http.server` which uses HTTP.

3. **eframe vs egui-winit** - eframe (egui's official web framework) has additional JS-level `paste` event listeners that work around Firefox's readText limitations. This project uses egui-winit directly, so it doesn't get those workarounds automatically.

## Solution

### Step 1: Enable the clipboard feature (may be all that's needed)

In `Cargo.toml`, change the WASM egui-winit dependency:

```toml
egui-winit = { version = "0.33", default-features = false, features = ["clipboard"] }
```

This should fix:
- **Copy/Export** (`ctx.copy_text()`) - calls `navigator.clipboard.writeText()` which works in both Chrome and Firefox when served from a secure context with user activation
- **Paste into egui text fields** - may work via `navigator.clipboard.readText()` in Chrome; Firefox support depends on the egui-winit implementation details

**Test this first.** It may be sufficient for both browsers, especially if running on localhost.

The `web_sys_unstable_apis` cfg flag is already set in both build scripts (`build-wasm.bat` and `build-wasm.sh`), which is required for the web-sys Clipboard API bindings.

### Step 2: If Firefox paste still doesn't work - add JS paste event bridge

If Step 1 fixes copy but not paste in Firefox, add a JavaScript paste event listener in `index.html` that bridges browser paste events into the WASM app:

```javascript
// Listen for paste events on the document and forward to WASM
document.addEventListener('paste', (e) => {
    const text = e.clipboardData?.getData('text/plain');
    if (text && window.__egui_paste_callback) {
        window.__egui_paste_callback(text);
    }
});
```

Then expose a Rust callback via wasm-bindgen that injects the pasted text into egui's input state. This is essentially what eframe does internally.

This would require:
- Adding `"ClipboardEvent"`, `"DataTransfer"` to web-sys features in Cargo.toml
- A small wasm-bindgen bridge function in the WASM entry point
- Hooking into egui's `RawInput` to inject clipboard text

### Step 3: If copy also fails in Firefox - add JS copy bridge

Similar to paste, add a copy event listener or use `document.execCommand('copy')` as a fallback:

```javascript
document.addEventListener('copy', (e) => {
    if (window.__egui_copy_text) {
        e.clipboardData?.setData('text/plain', window.__egui_copy_text);
        e.preventDefault();
        window.__egui_copy_text = null;
    }
});
```

## Affected Code

All clipboard operations go through `egui::Context::copy_text()`:

| File | Line | Operation |
|------|------|-----------|
| `src/app/ui_handlers.rs` | 44 | Config export to clipboard |
| `src/app/ui_handlers.rs` | 259 | Palette export to clipboard |
| `src/app/ui_handlers.rs` | 434 | Palette file loaded to clipboard (WASM) |
| `src/ui/config_dialog.rs` | 28-34 | Config import text area (manual paste) |
| `src/ui/palette_editor.rs` | 315 | Palette export button |
| `src/ui/palette_editor.rs` | 320-336 | Palette import text area (manual paste) |

## Testing

1. Build WASM: `build-wasm.bat` (already sets `web_sys_unstable_apis`)
2. Serve locally: `python -m http.server 8080` (localhost = secure context)
3. Test in Firefox:
   - Open Config Import/Export dialog
   - Click "Export to Clipboard" - verify JSON appears when pasting in Notepad
   - Copy JSON from Notepad, paste into the Import text area with Ctrl+V
4. Test in Chrome for comparison
5. If deploying to a real domain, must use HTTPS

## References

- [egui discussion #2171: WASM & clipboard](https://github.com/emilk/egui/discussions/2171) - HTTPS requirement and `web_sys_unstable_apis` flag
- [egui issue #5388: Clipboard on non-HTTPS](https://github.com/emilk/egui/issues/5388) - Crash fix for non-secure contexts (fixed in egui >= 0.29.1)
- [MDN: Clipboard.readText()](https://developer.mozilla.org/en-US/docs/Web/API/Clipboard/readText) - Browser compatibility table (Firefox restrictions)
- [MDN: Clipboard.writeText()](https://developer.mozilla.org/en-US/docs/Web/API/Clipboard/writeText) - Works in all browsers with secure context + user activation
