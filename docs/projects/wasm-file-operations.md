# WASM File Operations Cleanup

## Status: In Progress

## Problem

WASM file operations have several UX issues due to workarounds for async file dialogs:

1. **Config load** - Copies JSON to clipboard instead of loading (broken)
2. **PNG export** - Shows intermediate dialog instead of direct download
3. **Inconsistent patterns** - Some operations use egui temp storage, others use clipboard hacks

## Current State

### Working Correctly ✅
- **Config save** (line 486) - Async write works
- **Apophysis import** (line 716) - Uses `egui::Id::new("pending_apophysis_import")` temp storage
- **Animation load** (line 764) - Uses egui memory
- **Palette save** (line 817) - Async write works
- **Palette load** (line 928) - Uses egui memory

### Broken/Poor UX ❌
- **Config load** (line 638) - Copies to clipboard, logs "paste to import" - doesn't actually load
- **PNG export** (line 1073) - Has intermediate dialog, should trigger direct browser download

## Solution

### Pattern: egui Temp Storage

The working pattern (already used for Apophysis import, animation load, palette load):

```rust
// In async block - store result
ctx.data_mut(|data| {
    data.insert_temp(egui::Id::new("pending_config_load"), config);
});

// In render loop - check for pending result
if let Some(config) = self.egui_layer.ctx.data_mut(|data| {
    data.remove_temp::<FractalConfig>(egui::Id::new("pending_config_load"))
}) {
    self.load_config_with_undo(config, "Load Config".to_string());
}
```

### Fix 1: Config Load

Replace clipboard hack with egui temp storage:

```rust
#[cfg(target_arch = "wasm32")]
{
    let ctx = self.egui_layer.ctx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(file_handle) = rfd::AsyncFileDialog::new()
            .add_filter("Fractal Flame", &["fflame"])
            .pick_file()
            .await
        {
            let contents = file_handle.read().await;
            let json = String::from_utf8_lossy(&contents).to_string();

            // Parse and store for pickup in render loop
            match serde_json::from_str::<FractalConfig>(&json) {
                Ok(config) => {
                    ctx.data_mut(|data| {
                        data.insert_temp(egui::Id::new("pending_config_load"), config);
                    });
                    log::info!("Config loaded from file");
                }
                Err(e) => {
                    log::error!("Failed to parse config: {}", e);
                }
            }
        }
    });
}
```

Then in render loop (near line 967 where Apophysis import is checked):

```rust
#[cfg(target_arch = "wasm32")]
{
    // Check for pending config load
    if let Some(config) = self.egui_layer.ctx.data_mut(|data| {
        data.remove_temp::<FractalConfig>(egui::Id::new("pending_config_load"))
    }) {
        if let Err(e) = self.load_config_with_undo(config, "Load Config".to_string()) {
            log::error!("Failed to load config: {}", e);
        }
    }
}
```

### Fix 2: PNG Export Direct Download

Instead of intermediate dialog, trigger browser download directly:

```rust
// After getting PNG bytes, trigger download via JavaScript
#[cfg(target_arch = "wasm32")]
{
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, Url, HtmlAnchorElement};

    let array = js_sys::Uint8Array::from(&png_bytes[..]);
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array);

    let mut options = BlobPropertyBag::new();
    options.type_("image/png");

    let blob = Blob::new_with_u8_array_sequence_and_options(&blob_parts, &options).unwrap();
    let url = Url::create_object_url_with_blob(&blob).unwrap();

    let document = web_sys::window().unwrap().document().unwrap();
    let a = document.create_element("a").unwrap()
        .dyn_into::<HtmlAnchorElement>().unwrap();
    a.set_href(&url);
    a.set_download(&filename);
    a.click();

    Url::revoke_object_url(&url).unwrap();
}
```

## Implementation Plan

### Phase 1: Fix Config Load
- [ ] Replace clipboard hack with egui temp storage pattern
- [ ] Add pending config check in render loop
- [ ] Test load config works in WASM

### Phase 2: Fix PNG Export
- [ ] Implement direct browser download via Blob/URL API
- [ ] Remove intermediate dialog
- [ ] Test PNG export triggers download in browser

### Phase 3: Cleanup
- [ ] Review all WASM file operations for consistency
- [ ] Add error feedback to user (toast/notification)
- [ ] Update any related documentation

## Files to Modify

- `src/app/mod.rs` - Config load fix, PNG export fix
- `Cargo.toml` - May need `js-sys` and `web-sys` features for Blob API

## Testing

- [ ] WASM: Load .fflame config file
- [ ] WASM: Export PNG triggers browser download
- [ ] WASM: Apophysis import still works
- [ ] WASM: Palette load/save still works
- [ ] Desktop: All file operations unchanged
