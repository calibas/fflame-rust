# iOS Native App

**Status:** Planning
**Priority:** Medium
**Depends on:** [Mobile UI/UX Redesign](mobile-ui-redesign.md)

## Goal

Build a native iOS app from the existing Rust codebase. The rendering core (wgpu compute shaders) runs on Metal directly — no WebGPU translation layer. The UI uses egui with the mobile layout from the UI redesign project.

## Architecture

```
┌─────────────────────────────┐
│  iOS App Bundle (.ipa)      │
│  ┌───────────────────────┐  │
│  │  Rust Binary           │  │
│  │  ├─ wgpu → Metal      │  │
│  │  ├─ winit → UIKit     │  │
│  │  ├─ egui (mobile UI)  │  │
│  │  └─ fractal engine    │  │
│  └───────────────────────┘  │
└─────────────────────────────┘
```

- **GPU:** wgpu 27 with Metal backend — compute shaders run natively
- **Windowing:** winit 0.30 supports iOS (UIKit integration)
- **UI:** egui + egui_dock with compact workspace layout (from UI redesign project)
- **No WebView, no WASM** — fully compiled to aarch64-apple-ios

## Known Blockers

### 1. `rfd` Crate (File Dialogs)

`rfd` doesn't work on iOS. Fix: conditional compilation.

```rust
#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod file_dialogs;
```

All file I/O features that use `rfd` need mobile alternatives:
- **Config import/export:** In-app UI (already have JSON import/export panel)
- **PNG export:** Save to photo library via iOS APIs, or share sheet
- **Palette import:** Copy/paste from clipboard, or load from API

### 2. Text Input / Virtual Keyboard

winit's soft keyboard support on iOS is incomplete (winit issue #1823, open since 2021). The virtual keyboard doesn't reliably appear when egui TextEdit fields gain focus.

**Text input is required for:** login forms, flame naming, search, numeric entry.

**Potential solutions (in order of preference):**
1. **Wait for winit improvements** — active development, may be resolved by the time we reach this project
2. **Platform-native text dialogs** — call into Swift/ObjC via FFI for text input. Present a native `UIAlertController` with a text field, return the result to Rust. Reliable but disruptive UX.
3. **Hybrid WebView for auth** — embed a WKWebView just for login/registration screens (reuse the React app's auth pages). Use egui for everything else.
4. **In-app virtual keyboard** — render a keyboard in egui itself (community crate `egui_virtual_keyboard` exists). Last resort — poor UX.

### 3. Safe Area Insets

iPhone notch/Dynamic Island and home indicator create non-rectangular safe areas. egui 0.33 added `safe_area_insets` to `RawInput` — need to verify winit passes these through correctly on iOS.

### 4. Build Toolchain

- Requires Xcode + iOS SDK
- Target: `aarch64-apple-ios`
- Need Apple Developer account for device testing and App Store
- Code signing configuration

## iOS-Specific Features

- **Share sheet** — native iOS share for exported PNGs (via `UIActivityViewController` FFI)
- **Photo library** — save exports directly to Camera Roll
- **Haptic feedback** — subtle vibration on parameter snaps (via `UIImpactFeedbackGenerator`)
- **App lifecycle** — handle background/foreground transitions, save state

## Performance Expectations

Native Metal should outperform WASM WebGPU:
- Direct Metal API access (no WebGPU abstraction layer)
- Native threading (no web worker constraints)
- No WASM sandbox overhead on CPU-side code
- Direct memory access (no bounds checking)

Mobile GPUs (Apple A-series/M-series) have strong compute shader support.

## Build Command

```bash
cargo build --target aarch64-apple-ios --release
```

Current status: compiles with dependency fixes needed (rfd). Runtime not tested.

## Open Questions

- App Store distribution vs TestFlight only?
- Minimum iOS version target? (iOS 26+ for parity with WebGPU Safari, or older?)
- Should the app require network (API) or work fully offline?
- How to handle app updates (vs web which is always latest)?
