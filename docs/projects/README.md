# Current Projects

This folder contains documentation for **active development work** and ongoing projects.

## What Goes Here

- **Active feature development** - In-progress work on new features
- **Refactoring projects** - Ongoing code reorganization efforts
- **Investigation reports** - Active research into problems or improvements
- **Project plans** - Detailed plans for multi-step projects
- **Work-in-progress designs** - Architecture proposals being implemented

## File Organization

Use descriptive names that indicate the project:
- `feature-name-plan.md` - Planning document
- `feature-name-progress.md` - Progress tracking
- `investigation-topic.md` - Research and findings

## When to Archive

When a project is **completed** or **abandoned**, move the documentation to:
- `docs/archive/` - For completed work that's now part of the system
- `docs/experimental/` - For abandoned experiments or alternative approaches

## Current Projects

### Active

- **palette-system-redesign.md** - Design document for palette management improvements
  - Status: Planning phase
  - Focus: Better palette library management, custom palettes, built-in vs user palettes

- **undo-redo-issues.md** - Documentation of known undo/redo system issues and future improvements
  - Status: Documentation only
  - Focus: Multi-level undo, batch operations, state compression

- **animation-system.md** - Future feature for keyframe animation
  - Status: Planning/Design
  - Focus: Keyframe timeline, parameter interpolation

- **tiled-high-res-export.md** - High-resolution export via tiling
  - Status: Planning
  - Focus: Export beyond GPU memory limits

- **supersampling-antialiasing.md** - Quality improvement via SSAA
  - Status: Planning
  - Focus: Multi-sample rendering for smoother edges

- **apophysis-remaining-features.md** - Full Apophysis compatibility
  - Status: Planning
  - Focus: Final transform, additional variations, XML import/export

- **RESOLUTION-AND-QUALITY-ROADMAP.md** - Master plan for quality improvements
  - Status: Planning
  - Focus: Tiled export, SSAA, adaptive sampling

- **i18n-ui-migration.md** - Internationalization project
  - Status: In Progress (foundation complete, panel migration ongoing)
  - Focus: Translate all UI panels to 4+ languages

### Recently Completed (2025-11-14)

- ~~**png-export-fix.md**~~ → Archived to `archive/projects/`
  - Unified fractal texture approach
  - Fixed brightness and tone curve issues

- ~~**png-export-brightness-bug.md**~~ → Archived to `archive/projects/`
  - Investigation of resolution-dependent brightness
  - Led to PNG export fix

- ~~**viewport-vs-export-comparison.md**~~ → Archived to `archive/projects/`
  - Side-by-side analysis of rendering code paths
  - Identified accumulation strategy differences

- ~~**ui-improvements-docking.md**~~ → Archived to `archive/projects/`
  - egui_dock integration complete
  - 4 workspace layouts implemented
  - Menu bar with full translations

- ~~**frame-synchronization-issues.md**~~ → Archived to `archive/projects/`
  - Fixed race conditions in render pipeline
  - Proper UI/GPU state synchronization

### Previously Completed (2025-11-01)

- ~~**centralized-update-logic.md**~~ → Archived to `archive/state-centralization/`
  - All UI controls now use ConfigManager
  - UpdateType pattern fully implemented
  - Preview mode issues resolved

---

**Related:**
- [docs/main/](../main/) - Current implementation documentation
- [docs/experimental/](../experimental/) - Future ideas and experimental designs
- [docs/archive/](../archive/) - Historical documentation
