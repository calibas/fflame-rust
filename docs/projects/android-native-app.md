# Android Native App

**Status:** Planning
**Priority:** Medium
**Depends on:** [Mobile UI/UX Redesign](mobile-ui-redesign.md)

## Goal

Build a native Android app from the existing Rust codebase. The rendering core (wgpu compute shaders) runs on Vulkan directly — no WebGPU translation layer. The UI uses egui with the mobile layout from the UI redesign project.

## Architecture

```
┌─────────────────────────────┐
│  Android App (.apk)         │
│  ┌───────────────────────┐  │
│  │  Rust Binary (JNI)     │  │
│  │  ├─ wgpu → Vulkan     │  │
│  │  ├─ winit → NativeAct  │  │
│  │  ├─ egui (mobile UI)  │  │
│  │  └─ fractal engine    │  │
│  └───────────────────────┘  │
│  ┌───────────────────────┐  │
│  │  android-activity      │  │
│  │  (NativeActivity glue) │  │
│  └───────────────────────┘  │
└─────────────────────────────┘
```

- **GPU:** wgpu 27 with Vulkan backend (primary), GLES 3.0 fallback for older devices
- **Windowing:** winit 0.30 supports Android via `android-activity` crate
- **UI:** egui + egui_dock with compact workspace layout (from UI redesign project)
- **No WebView, no WASM** — fully compiled to `aarch64-linux-android`

## Known Blockers

### 1. `rfd` Crate (File Dialogs)

`rfd` doesn't work on Android. Fix: conditional compilation (same as iOS).

```rust
#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod file_dialogs;
```

All file I/O features that use `rfd` need mobile alternatives:
- **Config import/export:** In-app UI (already have JSON import/export panel)
- **PNG export:** Save to gallery via Android MediaStore API, or share intent
- **Palette import:** Copy/paste from clipboard, or load from API

### 2. `android-activity` Crate Configuration

winit on Android requires `android-activity` with the correct feature enabled. Two options:

- **`NativeActivity`** — simpler, uses Android's built-in NativeActivity class
- **`GameActivity`** — Google's GameActivity library, better input handling

Likely need to add to `Cargo.toml`:
```toml
[target.'cfg(target_os = "android")'.dependencies]
android-activity = { version = "0.6", features = ["native-activity"] }
```

winit also needs the `android-native-activity` or `android-game-activity` feature enabled.

### 3. Text Input / Virtual Keyboard

Same issue as iOS — winit's soft keyboard support on Android is incomplete (winit issue #1823). The virtual keyboard doesn't reliably appear when egui TextEdit fields gain focus.

**Text input is required for:** login forms, flame naming, search, numeric entry.

**Potential solutions (in order of preference):**
1. **Wait for winit improvements** — active development, may be resolved by the time we reach this project
2. **Platform-native text dialogs** — call into Java/Kotlin via JNI for text input. Present a native `AlertDialog` with an `EditText`, return the result to Rust. Reliable but disruptive UX.
3. **Hybrid WebView for auth** — embed a WebView just for login/registration screens (reuse the React app's auth pages). Use egui for everything else.
4. **In-app virtual keyboard** — render a keyboard in egui itself (community crate `egui_virtual_keyboard` exists). Last resort — poor UX.

### 4. Status Bar / Navigation Bar Insets

Android has status bar (top), navigation bar (bottom), and display cutouts (notch/punch-hole). The app needs to avoid drawing interactive UI behind these areas.

- winit should provide safe area information, but support may be incomplete
- May need JNI calls to query `WindowInsets` API (Android 10+)
- egui 0.33 `safe_area_insets` in `RawInput` — need to verify winit populates this on Android

### 5. Build Toolchain

- Requires Android SDK + NDK (C/C++ toolchain for cross-compilation)
- Target: `aarch64-linux-android` (ARM64, 99%+ of modern Android devices)
- Optional: `armv7-linux-androideabi` (32-bit ARM for very old devices)
- Build tool: `cargo-apk` or `xbuild` to package as `.apk`
- Need to configure `ANDROID_HOME`, `ANDROID_NDK_HOME` environment variables
- Minimum SDK version decision needed (API 24 / Android 7.0 for Vulkan 1.0)

### 6. Vulkan Availability

Not all Android devices support Vulkan:
- **Vulkan 1.0**: Required from Android 7.0 (API 24), but some devices still lack it
- **Vulkan 1.1**: Required from Android 10 (API 29)
- **GLES 3.0 fallback**: wgpu can fall back to OpenGL ES, but compute shaders require GLES 3.1
- **Practical**: Target Vulkan 1.0. ~95%+ of active Android devices support it (2025 data)

## Android-Specific Features

- **Share intent** — native Android share for exported PNGs (via `Intent.ACTION_SEND` through JNI)
- **MediaStore** — save exports directly to gallery/Photos
- **Haptic feedback** — subtle vibration on parameter snaps (via `Vibrator` API through JNI)
- **App lifecycle** — handle `onPause`/`onResume`, save state on background
- **Back button** — handle Android back gesture (undo? close panel? exit?)

## GPU Considerations

Android GPU landscape is more fragmented than iOS:
- **Qualcomm Adreno** — most common, good Vulkan support
- **ARM Mali** — common in Samsung Exynos, MediaTek devices
- **PowerVR** — rare, older devices
- **Samsung Xclipse (AMD RDNA2)** — newer Galaxy flagships

Compute shader support varies. Test on multiple GPU families. wgpu abstracts most differences, but edge cases exist (driver bugs, feature gaps).

## Performance Expectations

Native Vulkan should outperform WASM WebGPU:
- Direct Vulkan API access (no WebGPU abstraction layer)
- Native threading (no web worker constraints)
- No WASM sandbox overhead on CPU-side code
- Direct memory access (no bounds checking)

However, Android GPUs are generally less powerful than Apple's A-series/M-series. May need lower default iteration counts for mid-range devices.

## APK Size

- Debug builds: ~150MB (includes debug symbols, unoptimized)
- Release builds: ~5-10MB (stripped, optimized, LTO)
- Shader code is generated at runtime (not pre-compiled SPIR-V), so no shader bloat

## Build Command

```bash
# Install target
rustup target add aarch64-linux-android

# Build with cargo-apk (after SDK/NDK setup)
cargo apk build --target aarch64-linux-android --release
```

Current status: not yet attempted. Requires SDK/NDK setup and dependency fixes first.

## Open Questions

- Google Play distribution vs sideload APK only?
- Minimum Android version target? (API 24 for Vulkan, or higher?)
- Should the app require network (API) or work fully offline?
- How to handle the fragmented GPU landscape (Adreno vs Mali vs others)?
- Support tablets specifically, or phone-only initially?
- 32-bit ARM support needed, or 64-bit only?
