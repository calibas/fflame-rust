# i18n UI Migration Project

## Overview

Migrate all hardcoded English text in the UI to use the rust-i18n translation system. This will make the application accessible to users worldwide.

## Status

**Started:** 2025-11-13
**Status:** In Progress - Phase 1 Complete (Foundation)

## Goals

1. ✅ Add i18n infrastructure (rust-i18n)
2. ✅ Create translation files for 4 languages (en, es, ja, zh-CN)
3. ✅ Add font loading system for CJK languages
4. ✅ Add language selector to menu bar (🌐 globe icon)
5. ⏳ Migrate all UI panels to use t!() macro
6. ⏳ Add more languages (fr, de, ru, ko, zh-TW, ar)

## Completed Work

### Phase 1: Foundation (Complete ✅)
- ✅ Added rust-i18n 3.1 dependency
- ✅ Created locales/ directory structure
- ✅ Created translation files:
  - `locales/en.yml` - English (200+ keys)
  - `locales/es.yml` - Spanish (complete)
  - `locales/ja.yml` - Japanese (complete)
  - `locales/zh-CN.yml` - Chinese Simplified (complete)
- ✅ Created `src/i18n.rs` module
- ✅ Created `src/ui/font_loader.rs` with runtime font loading
- ✅ Added language selector to menu bar (🌐 globe icon, top-right)
- ✅ Migrated menu bar to use t!() macro (File, Edit, View, etc.)

## Current Status: UI Panel Migration

### Panels - Migration Status

| Panel | File | Status | Notes |
|-------|------|--------|-------|
| Menu Bar | `src/ui/menu_bar.rs` | ✅ Complete | All menu items translated |
| Settings | `src/ui/settings.rs` | ❌ Not Started | ~50 UI strings |
| Transforms | `src/ui/transform_list.rs` | ❌ Not Started | Labels, tooltips |
| View | `src/ui/view.rs` | ❌ Not Started | Zoom, pan, rotation labels |
| Colors | `src/ui/colors.rs` | ❌ Not Started | Color mode, palette labels |
| Tone Mapping | `src/ui/tone_mapping.rs` | ❌ Not Started | Mode, curve, exposure labels |
| Performance | `src/ui/performance.rs` | ❌ Not Started | FPS, timing labels |
| Help | `src/ui/help.rs` | ❌ Not Started | Help text, tooltips |
| Triangle Editor | `src/ui/triangle_editor.rs` | ❌ Not Started | Tool tips, labels |
| Palette Editor | `src/ui/palette_editor.rs` | ❌ Not Started | Color stop labels |
| Config Dialog | `src/ui/config_dialog.rs` | ❌ Not Started | Import/export labels |
| Variation Controls | `src/ui/variation_controls.rs` | ❌ Not Started | Variation names, tooltips |
| Variation Params | `src/ui/variation_params.rs` | ❌ Not Started | Parameter labels |

### Total Strings to Migrate
- Estimated: **500-800 UI strings** across all panels
- Completed: ~50 (menu bar)
- Remaining: ~450-750

## Translation Keys Structure

Current structure in `locales/*.yml`:

```yaml
# Menu Bar
menu:
  file: "File"
  edit: "Edit"
  # ... etc

# Panels
panels:
  fractal_viewport: "Fractal Viewport"
  rendering_settings: "Rendering Settings"
  # ... etc

# Transforms
transform:
  transform: "Transform"
  add: "Add"
  delete: "Delete"
  # ... etc

# Variations (26 core variations)
variations:
  linear: "Linear"
  sinusoidal: "Sinusoidal"
  # ... etc

# Color
color:
  mode: "Color Mode"
  palette: "Palette"
  # ... etc

# View, Rendering, ToneMap, History, Settings, Performance, Common, Tooltips, Messages, Errors
```

## Language Support

### Implemented Languages (4)
- ✅ **English (en)** - Default, complete
- ✅ **Spanish (es)** - Complete
- ✅ **Japanese (ja)** - Complete, requires NotoSansJP-Regular.otf
- ✅ **Chinese Simplified (zh-CN)** - Complete, requires NotoSansSC-Regular.otf

### Planned Languages (6+)
- ⏳ **French (fr)** - Uses default font
- ⏳ **German (de)** - Uses default font
- ⏳ **Russian (ru)** - Uses default font (Cyrillic supported)
- ⏳ **Korean (ko)** - Requires NotoSansKR font
- ⏳ **Chinese Traditional (zh-TW)** - Requires NotoSansTC font
- ⏳ **Arabic (ar)** - Requires NotoSansArabic font (RTL layout challenges)

### Font Requirements

**Default egui font supports:**
- Latin (English, Spanish, French, German, Italian, Portuguese)
- Cyrillic (Russian, Ukrainian, Bulgarian)
- Greek

**Requires additional fonts:**
- **Japanese** - NotoSansJP-Regular.otf (already present)
- **Chinese (Simplified)** - NotoSansSC-Regular.otf (already present)
- **Chinese (Traditional)** - NotoSansTC-Regular.otf
- **Korean** - NotoSansKR-Regular.otf
- **Arabic** - NotoSansArabic-Regular.ttf
- **Hebrew** - NotoSansHebrew-Regular.ttf

## Migration Strategy

### Phase 2: Core Panels (Priority)
1. **Settings Panel** (`src/ui/settings.rs`)
   - Many UI strings, frequently accessed
   - Rendering settings, preferences, advanced settings

2. **Transform List** (`src/ui/transform_list.rs`)
   - Core functionality
   - Labels, buttons, tooltips

3. **View Panel** (`src/ui/view.rs`)
   - Zoom, pan, rotation controls
   - Projection settings

### Phase 3: Color & Rendering
4. **Colors Panel** (`src/ui/colors.rs`)
5. **Tone Mapping Panel** (`src/ui/tone_mapping.rs`)
6. **Palette Editor** (`src/ui/palette_editor.rs`)

### Phase 4: Advanced Features
7. **Triangle Editor** (`src/ui/triangle_editor.rs`)
8. **Variation Controls** (`src/ui/variation_controls.rs`)
9. **Variation Params** (`src/ui/variation_params.rs`)

### Phase 5: Supporting Panels
10. **Performance** (`src/ui/performance.rs`)
11. **Help** (`src/ui/help.rs`)
12. **Config Dialog** (`src/ui/config_dialog.rs`)

## Translation Process

For each panel:

1. **Identify strings** - Find all hardcoded English text
2. **Add to .yml files** - Add translation keys to all 4 language files
3. **Use t!() macro** - Replace hardcoded strings with `t!("key.path")`
4. **Test** - Verify all languages display correctly
5. **Update this doc** - Mark panel as complete

### Example Migration

**Before:**
```rust
ui.label("Zoom");
ui.add(egui::Slider::new(&mut zoom, 0.1..=10.0).text("Zoom"));
```

**After:**
```rust
use rust_i18n::t;

ui.label(t!("view.zoom"));
ui.add(egui::Slider::new(&mut zoom, 0.1..=10.0).text(t!("view.zoom")));
```

**In locales/en.yml:**
```yaml
view:
  zoom: "Zoom"
```

**In locales/es.yml:**
```yaml
view:
  zoom: "Zoom"
```

**In locales/ja.yml:**
```yaml
view:
  zoom: "ズーム"
```

**In locales/zh-CN.yml:**
```yaml
view:
  zoom: "缩放"
```

## Future Enhancements

### Potential Improvements
- [ ] Add language selection to welcome screen (first launch)
- [ ] Remember language preference in config file
- [ ] Add fallback chain (e.g., zh-TW → zh-CN → en)
- [ ] Add right-to-left (RTL) layout support for Arabic/Hebrew
- [ ] Add translation contribution guide for community
- [ ] Add language completion checker (ensure all keys present)
- [ ] Add pluralization support if needed
- [ ] Add date/number formatting per locale

### Community Translation
Once core migration is complete:
- Create `TRANSLATION.md` guide
- Accept community PRs for new languages
- Provide template .yml file with all keys
- Document font requirements per language

## Testing

### Manual Testing Checklist
- [ ] Switch between all 4 languages in running app
- [ ] Verify menu bar translates correctly
- [ ] Verify CJK fonts load for ja/zh-CN
- [ ] Verify all panels display translated text
- [ ] Verify tooltips are translated
- [ ] Verify error messages are translated
- [ ] Test on Windows, macOS, Linux (font differences)
- [ ] Test WASM build (no CJK fonts, falls back to English)

### Regression Testing
- [ ] Ensure no hardcoded English remains
- [ ] Verify UI layout doesn't break with longer translations
- [ ] Check for truncated text in narrow panels
- [ ] Verify special characters display correctly (é, ñ, ä, ö, 日本語, 中文)

## Documentation Updates

Files to update after migration:
- [ ] `CLAUDE.md` - Update i18n section with completion status
- [ ] `docs/main/UI.md` - Document translation system
- [ ] `docs/main/I18N.md` - Add translation guide
- [ ] `README.md` - Add supported languages section

## Related Files

- `src/i18n.rs` - i18n module
- `src/ui/font_loader.rs` - Font loading system
- `src/ui/menu_bar.rs` - Menu bar (✅ complete)
- `locales/en.yml` - English translations
- `locales/es.yml` - Spanish translations
- `locales/ja.yml` - Japanese translations
- `locales/zh-CN.yml` - Chinese translations
- `assets/fonts/NotoSans*.otf` - CJK font files

## Notes

- All translation keys use lowercase with underscores (snake_case)
- Translation files use YAML format with `_version: 1` header
- Font loading is on-demand (only loads when language selected)
- WASM builds do not embed CJK fonts (too large)
- Globe icon (🌐) in menu bar is universally recognizable
- Language display format: "🌐 EN English" works without fonts loaded

## Success Metrics

- ✅ 4+ languages supported
- ⏳ 100% UI coverage (all panels translated)
- ⏳ Font loading works for all CJK languages
- ⏳ No hardcoded English strings in UI code
- ⏳ All 4 languages fully tested
- ⏳ Documentation complete
