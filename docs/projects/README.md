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

### Recently Completed (2025-11-01)

- ~~**centralized-update-logic.md**~~ → Archived to `archive/state-centralization/`
  - All UI controls now use ConfigManager
  - UpdateType pattern fully implemented
  - Preview mode issues resolved

---

**Related:**
- [docs/main/](../main/) - Current implementation documentation
- [docs/experimental/](../experimental/) - Future ideas and experimental designs
- [docs/archive/](../archive/) - Historical documentation
