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

- [ ] **Audio-reactive stack** (cpal → CoreAudio). FIRST PASS FAILED
      (2026-08-04): no input from mic/iPhone/loopback, no permission
      prompt. Root cause: the bundle plist had no
      `NSMicrophoneUsageDescription` — macOS suppresses the prompt and
      delivers silence. Key added to make_macos_app.py; RETEST from a
      rebuilt bundle (the prompt should appear on first capture; after
      a denial, flip it in System Settings → Privacy → Microphone).
      Note: "loopback" on macOS structurally needs a virtual device
      (BlackHole) selected as INPUT — CoreAudio has no WASAPI-style
      output capture; the WASM build's tab capture is a Chrome API.
      RC2 (2026-08-05) has the key; retest pending.
- [x] **Animation** works (2026-08-04). **Video export** FAILED: ffmpeg
      installed but not detected — Finder-launched bundles get
      launchd's PATH, which lacks /opt/homebrew/bin. Fixed: resolver
      probes PATH then Homebrew/MacPorts locations. VERIFIED on RC2
      under a simulated launchd PATH: "ffmpeg found:
      /opt/homebrew/bin/ffmpeg" where bare `ffmpeg` does not resolve.
      A real in-app export from the bundle is still worth one run.
- [x] **Fly mode** works (2026-08-04) — but bare F2 does not: macOS
      sends it as brightness. Help now carries the Fn+F2 hint on macOS.
      OPEN QUESTION for the user: add an alternate non-Fn binding?
- [x] **Clipboard** works (2026-08-04).
- [x] **File dialogs** work (2026-08-04).
- [x] **Fullscreen** works (2026-08-04). Second-display drag untested —
      no second display available; keep on the list for whenever one
      is (the governor's refresh re-read is the edge in question).
- [x] **Retina/DPI** crisp (2026-08-04).
- [x] **Keyboard shortcuts** work (2026-08-04).
- [x] **High-res export in-app + longevity** (2026-08-04). 8K×8K @ 2x AA
      completed, PNG fine; no issues over a long session.

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
