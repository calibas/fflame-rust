## Summary
- Integrate Levels controls (low/high/gamma thresholds) into ConfigManager state system
- Add density histogram visualization with real-time percentile tracking
- Change Auto levels from continuous checkbox to one-shot button (avoids undo/redo issues)
- Add planning doc for future post-processing effects system

## Test plan
- [ ] Verify levels sliders update tonemap in real-time
- [ ] Verify Auto button applies histogram percentiles once when clicked
- [ ] Verify levels save/load with .fflame configs
- [ ] Verify undo/redo works for levels changes

🤖 Generated with [Claude Code](https://claude.com/claude-code)
