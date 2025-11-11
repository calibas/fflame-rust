# UI Improvements - Docking and Better UX

**Status:** Planning
**Priority:** Medium
**Estimated Effort:** 12-20 hours

---

## Overview

Modernize the UI with a professional docking layout system, better organization, and progressive disclosure of advanced features. Make the app more accessible to beginners while maintaining power-user functionality.

---

## Current Problems

### Layout Issues
- Fixed side panel layout - no flexibility
- All panels always visible - cluttered
- No way to focus on specific tasks
- Window too wide for smaller displays
- Settings scattered across multiple collapsible sections

### UX Issues
- Overwhelming number of controls for beginners
- Advanced features mixed with basic controls
- No progressive disclosure (everything visible at once)
- No context-sensitive help or tooltips
- Parameter sliders don't explain what they do

### Accessibility
- No dark/light theme toggle
- No internationalization support
- No keyboard shortcuts documented
- Font size not adjustable
- No high-contrast mode

---

## Proposed Solution

### Phase 1: egui_dock Integration (6-8 hours)

**Status:** Complete ✅

**Goal:** Replace fixed side panel with flexible docking system

**Implementation (Complete):**
- Converted all 7 main windows → dockable panels (1:1 mapping)
- All existing functionality preserved unchanged
- No reorganization or refactoring - pure migration
- **All panels migrated:** Transforms, Triangle Editor, Colors, Palette Editor, View, Rendering, History


**Features:**
- Multiple tabs/panels that can be rearranged
- Panels can be floating windows
- Save/restore workspace layouts
- Default layouts for different workflows

**Migrated Panels:**
1. **Transforms** ✅ (from Transforms window)
   - Transform list with controls
   - Add/delete transform buttons
   - Affine matrix, variation weights, parameters
2. **Triangle Editor** ✅ (from Triangle Editor window)
   - Visual triangle editing
   - All existing functionality
3. **Colors** ✅ (from Tone Mapping & Colors window)
   - Color mode controls
   - Palette selector
   - Tone mapping settings
   - Background color
4. **View** ✅ (camera/navigation)
   - Zoom, pan, rotation
   - 3D camera controls
   - Reset view button
5. **Rendering** ✅ (performance/quality)
   - File & Project (presets, config I/O, undo/redo)
   - Iteration controls
   - Accumulation settings
   - Speed multiplier
   - PNG export
6. **History** ✅ (undo/redo)
   - Visual undo history browser
   - Jump to any config state

**Implementation:**
```toml
[dependencies]
egui_dock = "0.13"  # Latest version
```

**Workspace Layouts:**
- **Beginner:** Fractal + Appearance tabs only
- **Standard:** Fractal + Transform Editor + Appearance + View
- **Advanced:** All tabs visible, split layout
- **Export:** Rendering + Appearance focused layout

**Files to Modify:**
- `Cargo.toml` - Add egui_dock dependency
- `src/ui/mod.rs` - Replace side panel with DockArea
- `src/ui/workspace.rs` (new) - Workspace layout management
- `src/ui/fractal_panel.rs` (new) - Split from mod.rs
- `src/ui/transform_editor.rs` (rename from transforms.rs)
- `src/ui/appearance_panel.rs` (new) - Split from mod.rs
- `src/ui/view_panel.rs` (new) - Split from mod.rs
- `src/ui/rendering_panel.rs` (new) - Split from mod.rs
- `src/ui/advanced_panel.rs` (new) - Expert features

---

### Phase 2: Progressive Disclosure (3-4 hours)

**Goal:** Hide complexity, reveal gradually as user needs it

**Beginner Mode:**
- Only essential controls visible
- Preset selector prominent
- Quality slider (maps to iteration count + accumulation)
- "Show Advanced" toggle button

**Standard Mode:**
- Transform editing exposed
- Variation controls visible
- Rendering parameters available
- Advanced features collapsed by default

**Expert Mode:**
- All controls visible
- Direct parameter access
- Debug features enabled
- Performance metrics displayed

**UI Patterns:**
- Collapsible "Advanced" sections within panels
- Info icons (ⓘ) with hover tooltips
- "Learn more" links to documentation
- Visual presets before manual editing

**Mode Switcher:**
```
[Beginner ▼] [Standard] [Expert]
```

**Persistence:**
- Save mode in UI state (not FractalConfig)
- Remember which advanced sections are expanded
- Per-user preference, not per-flame

---

### Phase 3: Better Menu System (2-3 hours)

**Goal:** Professional menu bar with organized commands

**Current State:**
- No menu bar at all
- Actions scattered in panels
- Keyboard shortcuts not discoverable

**Proposed Menu Bar:**

```
[File] [Edit] [View] [Fractal] [Rendering] [Window] [Help]
```

**File Menu:**
- New Flame (Ctrl+N)
- Open Config... (Ctrl+O)
- Save Config As... (Ctrl+Shift+S)
- ---
- Import Apophysis XML... (Ctrl+I)
- Export PNG... (Ctrl+E)
- Export Apophysis XML... (when implemented)
- ---
- Recent Files ▶ (when implemented)
- ---
- Quit (Ctrl+Q)

**Edit Menu:**
- Undo (Ctrl+Z)
- Redo (Ctrl+Y / Ctrl+Shift+Z)
- ---
- Preferences...

**View Menu:**
- Reset View (Home)
- Fit to Window (F) (when implemented)
- ---
- Zoom In (Ctrl++)
- Zoom Out (Ctrl+-)
- ---
- 2D Mode
- 3D Mode
- ---
- Show Grid (G) (when implemented)

**Transforms Menu:**
- Show Transform Editor (E)
- Show Triangle Editor (T)
- ---
- Add Transform (Ctrl+T)
- Copy Transform (Ctrl+C)
- Paste Transform (Ctrl+V)
- Duplicate Transform (Ctrl+D)
- Delete Transform (Delete)
- Randomize Transform (R)
- ---
- Load Preset ▶
- Save as Preset...
- ---
- Import Palette...
- Export Palette...

**Rendering Menu:**
- Pause/Resume (Space)
- Reset Accumulation (Ctrl+R)
- ---
- Quality ▶
  - Preview (Low)
  - Draft (Medium)
  - Final (High)
  - Ultra (Very High)
- ---
- Benchmark...

**Window Menu:**
- Beginner Layout
- Standard Layout
- Advanced Layout
- ---
- Reset Layout
- Save Layout...
- ---
- Fractal Panel
- Transform Editor
- Appearance Panel
- View Panel
- Rendering Panel
- Advanced Panel
- History Panel

**Help Menu:**
- Documentation (F1)
- Keyboard Shortcuts (Ctrl+/)
- ---
- Report Bug...
- About...

**Implementation:**
```rust
egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| { /* ... */ });
        ui.menu_button("Edit", |ui| { /* ... */ });
        // etc.
    });
});
```

---

### Phase 4: Translation Support (4-5 hours)

**Goal:** Internationalization (i18n) for wider accessibility

**Approach:**
- Use `fluent-rs` or `rust-i18n` crate
- External language files (not compiled in)
- Runtime language switching
- Fallback to English

**Languages (Priority Order):**
1. English (en-US) - Default
2. Spanish (es-ES)
3. French (fr-FR)
4. German (de-DE)
5. Japanese (ja-JP)
6. Chinese Simplified (zh-CN)

**Translation Files:**
```
assets/
  translations/
    en-US.ftl  (Fluent format)
    es-ES.ftl
    fr-FR.ftl
    ...
```

**Example Fluent File:**
```fluent
# en-US.ftl
app-title = Fractal Flame Renderer
file-menu = File
edit-menu = Edit
transform-weight = Weight
transform-color = Palette Position
```

**Implementation:**
```rust
use fluent_bundle::FluentBundle;

struct Translator {
    bundle: FluentBundle,
    current_locale: String,
}

impl Translator {
    fn t(&self, key: &str) -> String {
        // Lookup translation, fallback to key
    }
}
```

**UI Usage:**
```rust
ui.label(t("transform-weight"));
ui.add(egui::Slider::new(&mut weight, 0.0..=1.0).text(t("weight-slider")));
```

**Locale Selector:**
- In Preferences dialog
- Dropdown: [English ▼] [Español] [Français] ...
- Restart required (for initial implementation)

---

### Phase 5: Context Help & Tooltips (2-3 hours)

**Goal:** Help users understand parameters without leaving the app

**Features:**
- Hover tooltips on all controls
- Info icons (ⓘ) for complex parameters
- Context-sensitive help panel
- Link to documentation for details

**Tooltip Content:**
```rust
ui.label("Zoom")
    .on_hover_text("Controls the magnification level.\nHigher = more zoomed in.");

ui.add(egui::Slider::new(&mut zoom, 0.1..=10.0))
    .on_hover_text("Keyboard: Ctrl+Scroll or +/-");
```

**Info Icons:**
```rust
ui.horizontal(|ui| {
    ui.label("Histogram Color Scale");
    if ui.button("ⓘ").on_hover_text("Click for details").clicked() {
        show_help_panel = true;
    }
});
```

**Help Panel:**
- Dockable panel showing detailed parameter explanation
- Includes formula, visual examples, typical values
- "Learn more" link to docs/main/*.md files

**Parameter Database:**
```rust
struct ParamHelp {
    name: &'static str,
    tooltip: &'static str,
    description: &'static str,
    typical_range: &'static str,
    docs_link: Option<&'static str>,
}
```

---

### Phase 6: Theme Support (1-2 hours)

**Goal:** Dark/light/custom themes for accessibility

**Implementation:**
```rust
enum Theme {
    Dark,      // Default
    Light,
    HighContrast,
    Custom(egui::Style),
}

fn apply_theme(ctx: &egui::Context, theme: Theme) {
    let style = match theme {
        Theme::Dark => egui::Style::default(),  // egui default is dark
        Theme::Light => {
            let mut style = egui::Style::default();
            style.visuals = egui::Visuals::light();
            style
        }
        Theme::HighContrast => {
            let mut style = egui::Style::default();
            // Increase contrast, larger fonts, etc.
            style
        }
        Theme::Custom(s) => s,
    };
    ctx.set_style(style);
}
```

**Theme Selector:**
- In Preferences dialog or View menu
- Radio buttons: ( ) Dark  (•) Light  ( ) High Contrast
- Live preview (updates immediately)

---

## Implementation Plan

### Milestone 1: Docking Foundation (Week 1)
- [ ] Add egui_dock dependency
- [ ] Create basic DockArea structure
- [ ] Split UI into panel modules
- [ ] Define default workspace layouts
- [ ] Test workspace save/restore

### Milestone 2: Panel Organization (Week 2)
- [ ] Implement Fractal panel
- [ ] Implement Transform Editor panel
- [ ] Implement Appearance panel
- [ ] Implement View panel
- [ ] Implement Rendering panel
- [ ] Implement Advanced panel
- [ ] Test panel switching and rearrangement

### Milestone 3: Progressive Disclosure (Week 3)
- [ ] Add Beginner/Standard/Expert mode switcher
- [ ] Hide advanced features by default
- [ ] Add "Show Advanced" toggles to panels
- [ ] Test mode persistence
- [ ] Create beginner-friendly preset gallery

### Milestone 4: Menu System (Week 3)
- [ ] Implement menu bar
- [ ] Wire up all menu commands
- [ ] Add keyboard shortcuts
- [ ] Test shortcuts across platforms
- [ ] Document shortcuts in Help menu

### Milestone 5: Polish (Week 4)
- [ ] Add tooltips to all controls
- [ ] Add info icons for complex parameters
- [ ] Implement theme support
- [ ] Add translation infrastructure (English only initially)
- [ ] Test on different screen sizes
- [ ] Performance testing with many panels

---

## Design Mockup

```
┌─────────────────────────────────────────────────────────────────────────┐
│ File  Edit  View  Fractal  Rendering  Window  Help     [Beginner ▼]     │
├─────────────────────────────────────────────────────────────────────────┤
│                                │                                          │
│  ┌──────────────────────────┐  │                                          │
│  │ Fractal     Transform Ed │  │                                          │
│  ├──────────────────────────┤  │                                          │
│  │                          │  │         [Fractal Viewport]               │
│  │ • Transform 1  [Edit]    │  │                                          │
│  │   ├─ Weight: 1.0         │  │                                          │
│  │   └─ Color: 0.5          │  │                                          │
│  │                          │  │                                          │
│  │ • Transform 2  [Edit]    │  │                                          │
│  │   ├─ Weight: 0.8         │  │                                          │
│  │   └─ Color: 0.3          │  │                                          │
│  │                          │  │                                          │
│  │ [+ Add] [Delete]         │  │                                          │
│  └──────────────────────────┘  │                                          │
│  ┌──────────────────────────┐  │                                          │
│  │ Appearance               │  │                                          │
│  ├──────────────────────────┤  │                                          │
│  │ Palette: [Fire      ▼]   │  │                                          │
│  │ █████████████████        │  │                                          │
│  │                          │  │                                          │
│  │ Background: [████]       │  │                                          │
│  │ Exposure: [═══╪═════]    │  │                                          │
│  │ Gamma:    [═══╪═════]    │  │                                          │
│  └──────────────────────────┘  │                                          │
│                                │                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Benefits

### User Experience
- **Flexible:** Arrange UI to match workflow
- **Focused:** Show only relevant panels
- **Accessible:** Progressive disclosure reduces overwhelm
- **Professional:** Modern docking UI like Blender/Photoshop
- **Learnable:** Menu bar makes features discoverable

### Developer Experience
- **Maintainable:** Panels are separate modules
- **Extensible:** Easy to add new panels
- **Testable:** Each panel can be tested independently
- **Documented:** Keyboard shortcuts and tooltips self-document

### Internationalization
- **Wider audience:** Support non-English speakers
- **Professional:** Shows project maturity
- **Community:** Easier to accept translations from contributors

---

## Technical Considerations

### Dependencies
```toml
[dependencies]
egui_dock = "0.13"         # Docking layout system
fluent-rs = "0.16"         # Translation (optional Phase 4)
serde = { version = "1.0", features = ["derive"] }  # Workspace persistence
```

### Performance
- Docking adds minimal overhead (~1-2ms per frame)
- Panel rendering only when visible
- Workspace save/load uses serde (fast)

### Platform Compatibility
- egui_dock works on all platforms (Windows/macOS/Linux/WASM)
- Keyboard shortcuts need platform-specific mapping (Ctrl vs Cmd)
- Translation files loaded from assets/ (embedded in WASM)

### Backward Compatibility
- Old FractalConfig files still load
- UI state separate from config (no breaking changes)
- Workspace layouts stored in separate file

---

## Files to Create/Modify

### New Files
- `src/ui/workspace.rs` - Workspace layout management
- `src/ui/fractal_panel.rs` - Main fractal controls
- `src/ui/transform_editor.rs` - Detailed transform editing
- `src/ui/appearance_panel.rs` - Visual controls
- `src/ui/view_panel.rs` - Camera/navigation
- `src/ui/rendering_panel.rs` - Performance/quality
- `src/ui/advanced_panel.rs` - Expert features
- `src/ui/menu_bar.rs` - Menu system
- `src/ui/themes.rs` - Theme support
- `src/ui/translator.rs` - Translation system (Phase 4)
- `assets/workspaces/default.json` - Default layout
- `assets/translations/en-US.ftl` - English strings (Phase 4)

### Modified Files
- `Cargo.toml` - Add dependencies
- `src/ui/mod.rs` - Replace side panel with DockArea
- `src/app/mod.rs` - Wire up menu commands
- `src/config/fractal_config.rs` - Add UI state (optional)

---

## Success Criteria

### Usability
- [ ] Beginners can create basic flames without documentation
- [ ] Expert users can access all features efficiently
- [ ] Keyboard shortcuts for common tasks
- [ ] Help available without leaving app

### Flexibility
- [ ] Panels can be rearranged and docked
- [ ] Layouts can be saved and restored
- [ ] Panels can be floating windows
- [ ] Multiple monitors supported

### Accessibility
- [ ] Dark and light themes
- [ ] Tooltips on all controls
- [ ] Keyboard navigation works
- [ ] High contrast mode available

### Professional Polish
- [ ] Menu bar with organized commands
- [ ] Keyboard shortcuts documented
- [ ] Context-sensitive help
- [ ] Smooth panel transitions

---

## Risks and Mitigations

### Risk: egui_dock Learning Curve
**Impact:** Medium
**Mitigation:** Start with simple tab system, gradually add docking features. egui_dock has good examples and documentation.

### Risk: Translation Maintenance
**Impact:** Low (Phase 4 is optional)
**Mitigation:** Start with English only. Add translations as community contributions. Use Fluent format (industry standard).

### Risk: UI Complexity Creep
**Impact:** Medium
**Mitigation:** Stick to planned phases. Test with beginners at each milestone. Maintain beginner mode as priority.

### Risk: Performance with Many Panels
**Impact:** Low
**Mitigation:** Render only visible panels. Profile early and often. egui is fast by design.

---

## Future Enhancements (Post-MVP)

### Advanced Features
- Custom keyboard shortcuts
- Command palette (Ctrl+P)
- Scriptable layouts
- Panel presets per fractal type
- Plugin panels for custom features

### Accessibility
- Screen reader support (egui has limited support)
- Font size adjustment
- Color blindness modes
- High DPI scaling fixes

### Integration
- Drag-and-drop for config/palette files
- Right-click context menus
- Parameter linking (e.g., link two transforms)
- Preset gallery with thumbnails

---

## Related Documentation

- `docs/main/UI.md` - Current UI documentation
- `docs/main/CONFIG.md` - Configuration system
- egui_dock: https://github.com/Adanos020/egui_dock
- Fluent: https://projectfluent.org/

---

**Created:** 2025-11-08
**Status:** Planning
**Next Steps:** Review with stakeholders, prioritize phases, start with Phase 1 (egui_dock)
