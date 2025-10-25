# Version Tracking and Build Numbers

## Overview

The fractal flame renderer now includes comprehensive version tracking with auto-incrementing build numbers. Version information is displayed in the UI and included in all performance statistics exports.

## Features

### 1. Auto-Incrementing Build Numbers ✅

Each successful build automatically increments the build number stored in [build_number.txt](build_number.txt).

**Build Script:** [build.rs](build.rs)
- Reads current build number from file
- Increments by 1
- Writes back to file
- Exposes as `BUILD_NUMBER` environment variable

**Example Build Output:**
```
warning: fractal_flame_wgpu@0.1.0: Building version 0.1.0 (build #4)
```

### 2. Version Information Captured ✅

The build script captures comprehensive version information at compile time:

| Variable | Example | Source |
|----------|---------|--------|
| `APP_VERSION` | `0.1.0` | Cargo.toml |
| `BUILD_NUMBER` | `4` | build_number.txt (auto-increment) |
| `GIT_HASH` | `dba27e8` | `git rev-parse --short HEAD` |
| `GIT_BRANCH` | `main` | `git rev-parse --abbrev-ref HEAD` |
| `BUILD_TARGET` | `x86_64-pc-windows-msvc` | Cargo TARGET env |
| `BUILD_PROFILE` | `release` | Cargo PROFILE env |
| `BUILD_TIME` | `2025-10-21T15:30:45Z` | chrono::Utc::now() |
| `RUSTC_VERSION` | `1.75.0` | rustc --version |

### 3. Version Module ✅

**File:** [src/version.rs](src/version.rs)

Provides structured access to version information:

```rust
use fractal_flame_wgpu::version::get_version_info;

let info = get_version_info();
println!("{}", info.full_version());  // "0.1.0 (build #4)"
println!("{}", info.build_id());      // "4-dba27e8"
println!("{}", info.platform());      // "Windows"
println!("{}", info.architecture());  // "x86_64"
```

**Key Methods:**
- `full_version()` → `"0.1.0 (build #4)"`
- `short_version()` → `"0.1.0"`
- `build_id()` → `"4-dba27e8"`
- `compact_info()` → `"v0.1.0 build #4 (dba27e8) release"`
- `detailed_info()` → Multi-line detailed information
- `is_release()` / `is_debug()` → Build profile checks
- `platform()` → Human-readable platform name
- `architecture()` → Human-readable architecture

### 4. UI Integration ✅

**Display Location:** Performance Window (top section)

The UI now shows:
```
Fractal Flame Renderer
Version: 0.1.0 (build #4)
Build: dba27e8 (main) release
```

**Implementation:** [src/ui/mod.rs:166-176](src/ui/mod.rs#L166-L176)

### 5. Performance Statistics Integration ✅

All performance metrics now include version information:

#### PerformanceMetrics

**File:** [src/util.rs](src/util.rs)

```rust
pub struct PerformanceMetrics {
    // ... timing fields ...
    pub version_info: Option<VersionInfo>,
}
```

**New Methods:**
- `export_json()` → Export metrics with version as JSON
- `snapshot()` → Get version-tagged performance snapshot

**PerformanceSnapshot:**
```rust
pub struct PerformanceSnapshot {
    pub version: String,           // "0.1.0 (build #4)"
    pub build_number: u32,          // 4
    pub git_hash: String,           // "dba27e8"
    pub fps: f64,
    pub frame_time_ms: f64,
    pub frame_count: u64,
    pub compute_time_ms: f64,
    pub accumulate_time_ms: f64,
    pub tonemap_time_ms: f64,
    pub ui_time_ms: f64,
    pub timestamp: String,          // RFC3339 timestamp
}
```

#### FrameProfile

**File:** [src/profiler.rs](src/profiler.rs)

```rust
pub struct FrameProfile {
    // ... timing fields ...
    pub version: Option<String>,        // "0.1.0 (build #4)"
    pub build_number: Option<u32>,      // 4
}
```

All frame profiles automatically capture version info at creation.

---

## Usage Examples

### Export Performance Stats with Version

```rust
let metrics = PerformanceMetrics::new();
// ... run app ...
let json = metrics.export_json()?;
println!("{}", json);
```

**Output:**
```json
{
  "fps": 250.5,
  "frame_time_ms": 3.99,
  "frame_count": 12500,
  "compute_time_ms": 2.5,
  "accumulate_time_ms": 0.8,
  "tonemap_time_ms": 0.3,
  "ui_time_ms": 0.4,
  "version_info": null  // skipped in serialization
}
```

### Get Version-Tagged Snapshot

```rust
let snapshot = metrics.snapshot();
let json = serde_json::to_string_pretty(&snapshot)?;
println!("{}", json);
```

**Output:**
```json
{
  "version": "0.1.0 (build #4)",
  "build_number": 4,
  "git_hash": "dba27e8",
  "fps": 250.5,
  "frame_time_ms": 3.99,
  "frame_count": 12500,
  "compute_time_ms": 2.5,
  "accumulate_time_ms": 0.8,
  "tonemap_time_ms": 0.3,
  "ui_time_ms": 0.4,
  "timestamp": "2025-10-21T15:30:45.123Z"
}
```

### Frame Profile with Version

```rust
let profile = FrameProfile::new(1000);
// ... measure timings ...
let json = serde_json::to_string_pretty(&profile)?;
```

**Output:**
```json
{
  "frame_number": 1000,
  "total_cpu_ms": 4.5,
  "gpu_compute_ms": 2.5,
  "gpu_accumulate_ms": 0.8,
  "gpu_tonemap_ms": 0.3,
  "cpu_ui_ms": 0.4,
  "cpu_submit_ms": 0.5,
  "version": "0.1.0 (build #4)",
  "build_number": 4
}
```

---

## Build System Integration

### Dependencies Added

**[Cargo.toml](Cargo.toml):**
```toml
[dependencies]
once_cell = "1.19"
chrono = "0.4"

[build-dependencies]
chrono = "0.4"
rustc_version = "0.4"
```

### Build Process

1. **Pre-Build:** [build.rs](build.rs) runs
   - Reads [build_number.txt](build_number.txt)
   - Increments number
   - Writes back to file
   - Captures git info (hash, branch)
   - Sets environment variables

2. **Compilation:** Version info embedded
   - `env!("APP_VERSION")` → Cargo.toml version
   - `env!("BUILD_NUMBER")` → Auto-incremented number
   - `env!("GIT_HASH")` → Git commit hash
   - `env!("GIT_BRANCH")` → Git branch name
   - `env!("BUILD_TIME")` → Timestamp

3. **Runtime:** Version info accessible
   - `get_version_info()` → Global singleton
   - `VersionInfo::current()` → New instance

---

## Version Control

### What to Commit

✅ **Commit:**
- [build.rs](build.rs) - Build script
- [build_number.txt](build_number.txt) - Build number state
- [src/version.rs](src/version.rs) - Version module
- [VERSION-TRACKING.md](VERSION-TRACKING.md) - This file

❌ **Don't Commit:**
- Build artifacts (already in .gitignore)

### Build Number State

The [build_number.txt](build_number.txt) file is intentionally committed so that:
- Build numbers remain consistent across machines
- Team members share the same build sequence
- CI/CD has a canonical build number

If you want independent build numbers per machine, add `build_number.txt` to `.gitignore`.

---

## Platform Support

### Desktop (Windows/macOS/Linux)

✅ Full support
- All version info captured
- Git info available (if in git repo)
- Build time captured

### WebAssembly

✅ Full support
- Version info embedded at compile time
- Git info captured during build
- Runtime access works identically

### Mobile (iOS/Android)

⚠️ Experimental (requires dependency fixes, see [CLAUDE.md](CLAUDE.md))
- Version tracking will work once builds are functional
- Platform-specific: Returns correct `platform()` and `architecture()`

---

## Testing

### Unit Tests

**File:** [src/version.rs:230-280](src/version.rs#L230-L280)

```bash
cargo test --lib version
```

**Tests:**
- Version info is non-empty
- Build number is positive
- Full version format is correct
- Platform detection works
- Profile detection works (debug/release)
- JSON serialization round-trip

### Integration Test

```bash
# Build and check version output
cargo build --release 2>&1 | grep "Building version"
```

**Expected:**
```
warning: fractal_flame_wgpu@0.1.0: Building version 0.1.0 (build #N)
```

### Check Current Version

```bash
cargo run --release --example show_version
```

**Example Output:**
```
Version: 0.1.0 (build #4)
Git: dba27e8 (main)
Target: x86_64-pc-windows-msvc
Profile: release
Built: 2025-10-21T15:30:45Z
Rustc: 1.75.0
```

---

## Troubleshooting

### Build Number Doesn't Increment

**Cause:** Build script didn't run

**Solution:**
```bash
# Force rebuild
cargo clean
cargo build --release
```

### Git Info Shows "unknown"

**Cause:** Not in a git repository or git not in PATH

**Solution:**
- Ensure you're in a git repository
- Ensure `git` command is available
- Build script falls back to "unknown" if git fails

### Version Info All Zeros/Empty

**Cause:** Environment variables not set

**Solution:**
- Check build.rs ran successfully
- Look for build warnings in cargo output
- Try `cargo clean && cargo build`

---

## Future Enhancements

### Possible Additions

1. **Semantic Version Bump Command**
   - Script to bump major/minor/patch version in Cargo.toml
   - Reset build number on version change

2. **Build Metadata in PNG Exports**
   - Embed version in PNG metadata
   - EXIF/XMP tags for build info

3. **Performance Regression Detection**
   - Compare performance across build numbers
   - Alert when FPS drops more than X%

4. **Build Number Per-Profile**
   - Separate build numbers for debug/release
   - `build_number_debug.txt` and `build_number_release.txt`

5. **Version API Endpoint** (for WASM)
   - Expose version info via JavaScript
   - Allow web page to display version

---

## Summary

✅ **Auto-incrementing build numbers**
✅ **Comprehensive version info capture**
✅ **UI display of version/build**
✅ **Version in all performance exports**
✅ **Full serialization support**
✅ **Cross-platform compatible**

**Current Build:** #4 (as of this documentation)

---

**Last Updated:** 2025-10-21
**Project:** fflame-rust
**Module:** Version Tracking System
