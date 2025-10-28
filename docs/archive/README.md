# Documentation Archive

This folder contains **historical documentation** - investigation notes, evolution timelines, and completed project documentation that provides context for current implementation.

## What Goes Here

- **Historical investigations** - Research and exploration that led to current design
- **Completed project documentation** - Documentation from finished projects
- **Evolution timelines** - How systems evolved over time
- **Failed optimization attempts** - What was tried and why it didn't work
- **Alternative approaches** - Designs that were considered but not chosen
- **Decision documentation** - Why certain choices were made

## Purpose

Archive documentation is preserved for:
1. **Historical Context** - Understanding why things work the way they do
2. **Future Reference** - Avoiding re-attempting failed approaches
3. **Design Rationale** - Documenting the reasoning behind decisions
4. **Learning** - Showing the evolution of the system

## Organization

Archive is organized by topic area in subdirectories:

### Current Archive Directories

**histogram/** - Histogram color accumulation evolution (15 documents)
- Complete evolution from textureStore → u16 packed → u32 unpacked
- Failed optimization attempts (per-pixel adaptive scaling, convergence masking, etc.)
- Investigation reports and design decisions
- See [histogram/README.md](histogram/README.md) for details

**Add more topic directories as needed:**
- `archive/ui-evolution/` - UI redesign history
- `archive/shader-system/` - Shader system evolution
- `archive/performance/` - Performance investigation history
- etc.

## What NOT to Archive

Do not archive:
- **Current implementation docs** - Belongs in `docs/main/`
- **Active projects** - Belongs in `docs/projects/`
- **Future plans** - Belongs in `docs/experimental/`
- **Test files or code** - Belongs in source tree or `tests/`

## When to Archive

Archive documentation when:
- A project is **completed** and the work is now part of the system
- An investigation is **finished** and decisions have been made
- A system has **evolved** and you want to preserve the history
- An approach was **attempted and rejected** (preserve for future reference)

---

**Related:**
- [docs/main/](../main/) - Current implementation documentation
- [docs/projects/](../projects/) - Active development work
- [docs/experimental/](../experimental/) - Future ideas and experiments
