# i18n UI Migration Project

## Overview

Migrate all hardcoded English text in the UI to use the rust-i18n translation system. This will make the application accessible to users worldwide.

## Status

**Started:** 2025-11-13
**Completed:** 2026-01-06
**Status:** ✅ Complete (English UI fully migrated)

## Goals

1. ✅ Add i18n infrastructure (rust-i18n)
2. ✅ Create translation files (en.yml complete, others as needed)
3. ✅ Add font loading system for CJK languages
4. ✅ Add language selector to menu bar (🌐 globe icon)
5. ✅ Migrate all UI panels to use t!() macro
6. ⏳ Add more languages as needed (community contributions welcome)

## Completed Work

### Phase 1: Foundation ✅
- Added rust-i18n 3.1 dependency
- Created locales/ directory structure
- Created `src/i18n.rs` module
- Created `src/ui/font_loader.rs` with runtime font loading
- Added language selector to menu bar (🌐 globe icon, top-right)

### Phase 2: UI Panel Migration ✅

All panels have been migrated to use the t!() macro:

| Panel | File | Status |
|-------|------|--------|
| Menu Bar | `menu_bar.rs` | ✅ Complete |
| Help | `help.rs` | ✅ Complete |
| Animation | `animation.rs` | ✅ Complete |
| Path Editor | `path_editor.rs` | ✅ Complete |
| Preset Library | `preset_library.rs` | ✅ Complete |
| File Browser | `file_browser.rs` | ✅ Complete |
| Config Dialog | `config_dialog.rs` | ✅ Complete |
| Palette Editor | `palette_editor.rs` | ✅ Complete |
| Palette Library | `palette_library.rs` | ✅ Complete |
| Performance | `performance.rs` | ✅ Complete |
| Track Editor | `track_editor.rs` | ✅ Complete |
| Panel Viewer | `panel_viewer.rs` | ✅ Complete |
| Workspace | `workspace.rs` | ✅ Complete |
| Settings | `settings.rs` | ✅ Complete |
| Transforms | `transforms.rs` | ✅ Complete |
| View | `view.rs` | ✅ Complete |
| Colors | `colors.rs` | ✅ Complete |
| Tone Mapping | `tone_mapping.rs` | ✅ Complete |
| Triangle Editor | `triangle_editor.rs` | ✅ Complete |
| Variation Controls | `variation_controls.rs` | ✅ Complete |

### Translation Keys

`locales/en.yml` contains 700+ translation keys organized by section:
- `menu.*` - Menu bar items
- `panels.*` - Panel titles
- `transform.*` - Transform controls
- `variations.*` - Variation names and parameters
- `color.*` - Color settings
- `view.*` - View controls
- `rendering.*` - Rendering settings
- `tonemap.*` - Tone mapping controls
- `settings.*` - Settings panel
- `performance.*` - Performance metrics
- `animation_panel.*` - Animation controls
- `track_editor.*` - Track editor controls
- `path_editor.*` - Path editor
- `common.*` - Common UI elements
- `tooltips.*` - Tooltips
- `messages.*` - Status messages
- `errors.*` - Error messages

## Language Support

### Current
- ✅ **English (en)** - Complete (700+ keys)

### Future (as needed)
Additional languages can be added by:
1. Copying `locales/en.yml` to `locales/<lang>.yml`
2. Translating all values
3. Adding CJK fonts if needed (NotoSans family)

**Font Requirements:**
- Latin/Cyrillic/Greek: Default egui font (no additional files needed)
- Japanese: `NotoSansJP-Regular.otf`
- Chinese (Simplified): `NotoSansSC-Regular.otf`
- Chinese (Traditional): `NotoSansTC-Regular.otf`
- Korean: `NotoSansKR-Regular.otf`

## Related Files

- `src/i18n.rs` - i18n module with locale management
- `src/ui/font_loader.rs` - Runtime font loading for CJK
- `locales/en.yml` - English translations (primary)
- `assets/fonts/` - CJK font files

## Notes

- All translation keys use snake_case
- Translation files use YAML format
- Font loading is on-demand (only loads when language selected)
- WASM builds do not embed CJK fonts (too large, ~15MB each)
- Globe icon (��) in menu bar for language selection
