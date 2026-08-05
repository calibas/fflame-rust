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

- [x] **dist-profile build + bundle + smoke test.** (Done 2026-08-04,
      63d66c69.) dist built 4m40s / 19.7 MB; bundle 35 MB, zip 21 MB;
      smoke on the BUNDLED binary: export works, PNG provenance reads
      `BuildProfile: dist`, GUI launch clean (assets, 0 errors).
- [x] **DECISION — Intel Macs: Apple Silicon only for v1.** (Decided
      2026-08-04.) The requirements line in the download copy below
      carries it. Universal2 stays possible later, with the caveat that
      Intel-Mac GPUs have their own unprobed fast-math texture.
- [x] **Quarantine instructions — download-page copy.** Drafted below,
      paste-ready; lives here until there is a download page. The steps
      branch on macOS version because Sequoia removed right-click→Open,
      and they repeat on EVERY update until the app is notarized.

      ---
      **Installing on macOS**

      Fractal Art Editor is not yet notarized with Apple, so macOS will
      warn you the first time you open it — including after every
      update.

      *Requires an Apple Silicon Mac (M1 or later), macOS 11+.*

      1. Unzip and drag **Fractal Art Editor.app** into Applications.
      2. Open it once. macOS will refuse ("Apple could not verify…") —
         click **Done** (don't move it to Trash), then:
         - **macOS 15 (Sequoia) and later:** System Settings → Privacy &
           Security → scroll down → **Open Anyway** next to Fractal Art
           Editor, and authenticate.
         - **macOS 14 and earlier:** Control-click the app in
           Applications → **Open** → **Open**.
      3. That's once per downloaded version — macOS re-quarantines each
         update until the app is notarized.

      Terminal alternative, any macOS version:
      `xattr -d com.apple.quarantine "/Applications/Fractal Art Editor.app"`
      ---

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
