# Experimental: ideas and research

Ideas not being worked on, research notes and proposals. Nothing here
is a commitment, and some of it is a pasted conversation rather than a
plan. When an idea becomes active work, its plan goes to
[docs/projects/](../projects/); when one is implemented or overtaken,
it goes to [docs/archive/](../archive/) with a banner saying so.

## Ideas with a document

| Doc | What it is |
|---|---|
| [terrain-speed.md](terrain-speed.md) | A deep escape terrain's section renders out of the governor's reach. A rough outline; measure first. |
| [preview-slicing.md](preview-slicing.md) | The Fractal Browser's previews as a job advanced a few ms a frame. Planned 2026-11-04; four decisions. |
| [camera-lens-everywhere.md](camera-lens-everywhere.md) | The camera lens in every mode. Planned 2026-10-10; three decisions. |
| [slow-frame-slider-hold.md](slow-frame-slider-hold.md) | Hold any slider's drag once its frames turn slow. Planned 2026-10-12. |
| [online-assets.md](online-assets.md) | Skies and textures from providers (Poly Haven first). Planned and decided 2026-10-21. |
| [hyperbolic-camera-extras.md](hyperbolic-camera-extras.md) | The hyperbolic camera's roadmap: pathing and navigation through the animation system next. |
| [wasm-modules.md](wasm-modules.md) | The web app against the desktop, and ways to load parts on demand. Researched 2026-10-23. |
| [planner-experiments.md](planner-experiments.md) | Five experiments from the 2026-11-09 review of `rhai-planners`: a distance field from Focused Rendering's samples, inverses by Newton's method, plans from the Flame Attractor's beam walk, Focused Rendering in 3D (z as a passenger; true 3D). Not started. |
| [3d-lut.md](3d-lut.md) | A 3D colour lookup table after the tonemap. Not started. |
| [filmic-tunable.md](filmic-tunable.md) | A tunable (Hable-style) filmic highlight curve beside the shipped Clip/MaxNorm/Reinhard/Filmic. Not started. |
| [per-transform-color-adjustments.md](per-transform-color-adjustments.md) | Hue/saturation/brightness adjustments per transform. Not started; it predates `Transform.opacity` and the current colour modes. |
| [PROPOSAL-true-3d-affine.md](PROPOSAL-true-3d-affine.md) | A full 3x4 affine per transform. Deferred; JWildfire's three-plane affines shipped meanwhile. |
| [3d-volume-surface-planning.md](3d-volume-surface-planning.md) | Research on volume and surface rendering for 3D flames. Its depth-buffer route became solid rendering; the volume part was built and removed. |
| [cpu-rendering-research.md](cpu-rendering-research.md) | A software (CPU / SwiftShader) fallback renderer. Stalled: the partial work is on the `experimental/swiftshader-cpu-fallback` branch, far behind main. |
| [new-shaders.md](new-shaders.md) | A survey of 18 WGSL post-processing techniques, the research behind the effect chain. |
| [osc-input.md](osc-input.md) | OSC input for live control. Not started. |
| [audio-analysis-dsp-ml.md](audio-analysis-dsp-ml.md) | Audio analysis beyond FFT, including ML features (the audio system's phase 9). Research. |
| [android-native-app.md](android-native-app.md), [ios-native-app.md](ios-native-app.md) | Native mobile apps. Never started; the web app's compact layout serves phones meanwhile. |
| [other-spaces.md](other-spaces.md) | A conversation on a taxonomy of geometries; the hyperbolic part became `hyperbolic_camera`. |
| [NEURAL_NETWORK_IDEAS.md](NEURAL_NETWORK_IDEAS.md) | Speculative notes. |
| random-notes.txt | A scratch list. |

## Ideas without a document

- **Skill levels in the UI** (progressive disclosure): phase 2 of the
  archived [UX plan](../archive/projects/ux-improvements.md), never built.
- **Temporal antialiasing and spectral rendering** for the escape
  engine: listed in [escape-sim-followups.md](../archive/projects/escape-sim-followups.md),
  costed in the archived new-families research.
