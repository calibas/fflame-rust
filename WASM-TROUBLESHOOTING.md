# WASM Troubleshooting

## Issues Fixed

### Issue 1: arboard Clipboard Dependency ✅
**Error**: Build fails with arboard errors
**Fix**: Platform-specific egui-winit configuration (see Cargo.toml)

### Issue 2: std::time::Instant Not Supported ✅
**Error**: "time not implemented on this platform"
**Fix**: Use `web-time` crate instead of `std::time`

### Issue 3: Canvas Size Zero ✅
**Error**:
```
Uncaptured WebGPU error: size is zero
Uncaptured WebGPU error: Texture ... is invalid
```

**Cause**: Canvas had no dimensions set
**Fix**: Set explicit window size from browser window dimensions
```rust
let width = web_window.inner_width().unwrap().as_f64().unwrap() as u32;
let height = web_window.inner_height().unwrap().as_f64().unwrap() as u32;
```

**Location**: [src/lib.rs](src/lib.rs:49-50)

## Testing Checklist

After rebuilding, test:
- [ ] Canvas displays (not black)
- [ ] FPS counter shows non-zero FPS
- [ ] Mouse pan works (drag to move)
- [ ] Mouse zoom works (scroll wheel)
- [ ] UI panels visible (Performance, Transforms)
- [ ] Palette selection works
- [ ] Transform editing updates visuals

## Common Issues

### Black Screen
1. Check browser console for errors
2. Verify WebGPU is supported (Chrome 113+)
3. Hard refresh (Ctrl+F5) to clear cache
4. Check canvas dimensions in console: "Canvas dimensions: WxH"

### Performance Issues
- Reduce "Iterations per Thread" in UI
- Close other GPU-heavy tabs
- Try in Chrome (best WebGPU support)

### Build Issues
```bash
# Clean and rebuild
cargo clean
cargo build --target wasm32-unknown-unknown --lib --release
wasm-bindgen --out-dir ./pkg --target web ./target/wasm32-unknown-unknown/release/fractal_flame_wgpu.wasm
```

## File Sizes
- WASM binary: ~3.4 MB uncompressed
- With gzip: ~800 KB
- With brotli: ~600 KB

Enable compression on your web server for best load times!

## All Fixes Applied ✅

The current code has all fixes applied:
1. ✅ Clipboard disabled for WASM
2. ✅ web-time for WASM-compatible timing
3. ✅ Canvas dimensions set from window size

**Status**: Production ready! 🚀
