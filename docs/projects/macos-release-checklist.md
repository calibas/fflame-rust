# macOS release checklist

Everything between the current state of `macos-support` and calling
macOS supported. Check items off in place; each carries enough context
to be worked without re-deriving it.

**Already done and cross-verified, for orientation:** icons + `.app`
bundle + zip packaging (`make_macos_app.py`), executable-relative asset
loading, the vsync governor fix, startup surface-error fix, and the
whole fast-math divergence workstream — probe/census/rank built, the
REACHABLE worklist cleared 96 → 0, every fix verified bit-neutral on
Windows, two shipped presets (Flower, Bubbles) fixed. Visual suite
148/148 and 663 unit tests green on Metal.

## Tier 1 — blocks the first release

- [ ] **dist-profile build + bundle + smoke test.** Everything so far
      was `--release`; a shipped binary must report `dist` in its PNG
      provenance (docs/RELEASE.md). Steps: `cargo build --profile dist`,
      `python3 scripts/make_macos_app.py --zip`, then launch the bundle,
      export a PNG, and confirm `BuildProfile: dist` in its metadata.
- [ ] **DECISION — Intel Macs.** The bundle is aarch64-only; Intel users
      get "can't open". Recommendation: *Apple Silicon only for v1* (one
      sentence on the download page, zero work). Alternative: universal2
      (`x86_64-apple-darwin` + `lipo`) — launchable under Rosetta here,
      but Intel-Mac GPUs (AMD/Intel Metal) have their own unprobed
      fast-math texture, so launching ≠ rendering correctly on real
      Intel hardware.
- [ ] **Quarantine instructions on the download page.** Unsigned app ⇒
      Gatekeeper refuses; macOS 15 removed right-click→Open. User-facing
      copy needed wherever the zip lives: System Settings → Privacy &
      Security → "Open Anyway", or
      `xattr -d com.apple.quarantine "/Applications/Fractal Art Editor.app"`.
      This repeats on EVERY update until the app is notarized.

## Tier 2 — untested surface area (interactive checklist)

The viewport, editing, and the governor are user-verified. Nothing
below has been exercised on macOS at all. For each: does it work, and
does anything feel platform-wrong (shortcuts, focus, lag)?

- [ ] **Audio-reactive stack** (cpal → CoreAudio). Mic capture: does the
      OS permission prompt appear, does the Signal panel see levels?
      Audio-file playback analysis (mp3/wav via symphonia). BPM
      detection sanity. Screen/tab capture is Chrome-only web — skip.
- [ ] **Animation + video export.** Track animation plays in-app; video
      export produces a playable file; export doesn't wedge the UI
      longer than expected.
- [ ] **Fly mode** (F2 / 🚀). WASD/QE + mouse-look feel; sensitivity
      sane on a trackpad?; FreeLook vs FPS both behave; Esc exits
      cleanly.
- [ ] **Clipboard.** Copy/paste of config JSON in and out of the app;
      palette import via clipboard; ⌘C/⌘V in egui text fields.
- [ ] **File dialogs** (rfd → native). Open/save flame, palette import,
      PNG export path picking; a filename with spaces/unicode.
- [ ] **Fullscreen + display changes.** Toggle fullscreen; drag the
      window to a second display mid-render (the governor re-reads
      refresh every 120 frames — a 60↔120 Hz move should not wedge
      iteration rate); close the lid / reopen.
- [ ] **Retina/DPI.** UI crispness at 2x; export resolution unaffected
      by window scale; screenshots of the window look right.
- [ ] **Keyboard shortcuts.** ⌘Q quits cleanly (settings persisted?),
      ⌘W behavior, undo/redo binding on macOS.
- [ ] **High-res export in-app** (4K/8K path switch) — CLI is verified,
      the in-app dialog path is not.
- [ ] **Longevity.** Leave it rendering 30+ min: memory stable, no
      thermal runaway weirdness, no surface errors in the log.

## Tier 3 — deferred by decision, tracked so they stay decisions

- [ ] **Notarization** ($99/yr Apple Developer). Kills the per-update
      quarantine dance. Revisit after first release.
- [ ] **In-app version check.** `APP_VERSION` + one HTTP GET + "0.5.0 is
      out" link. Small; the biggest update-UX win short of Sparkle.
- [ ] **Sparkle auto-update.** Only worth it post-notarization.
- [ ] **`.fflame` file association** (`CFBundleDocumentTypes` +
      open-file event handling). Double-click a flame → opens in app.
- [ ] **macOS version breadth.** Plist claims 11.0; tested on 14.3/M2
      only. One probe+census run on any other Apple Silicon machine
      (M1/M3, macOS 12/13/15) would confirm the fast-math texture is
      family-stable.
- [ ] **Benchmarks baseline on M2** (`run_benchmarks.py`) so
      performance_history.csv has a macOS column.
- [ ] **Wire accepted-divergences into `probe compare`** so a matched
      cross-platform pair exits 0 — the original probe-design goal, now
      one PR away.
- [ ] **Census coverage for solid flames** (v1 excluded them; solid
      *renders* fine — 148/148 includes solid tests).
- [ ] **rank per-input severity** (entries currently paint all their
      inputs with the worst transition's severity).
