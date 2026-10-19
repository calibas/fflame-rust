# 3D look, lighting panel and tools

Planned 2026-10-19 from the user's list (in their words where quoted),
worked on while they were away. Each project has its steps, its gates and
its status; the order of work is at the end. Lighting and material work is
tested on a curved solid (the Mandelbulb) as well as the Menger sponge,
whose flat faces can mislead.

The state each plan starts from was surveyed first (2026-10-19): where every
3D lighting, material and shadow setting is edited; what a solid inherits
from a flame; fly mode and what it can know of the surface; the Scripts
panel's highlighter.

## P1. Syntax highlighting for formulas

"Syntax highlighting for Formulas, see the Scripts panel."

The Scripts panel highlights Rhai with a hand-written tokenizer through
egui's `TextEdit::layouter` (no syntect). The Formula Editor had none.

**Done 2026-10-19** (`7c831401`): `ui/code_highlight.rs` (was
`rhai_highlight.rs`) gains a tokenizer for the Ultra Fractal / Fractint
formula language -- `;` comments, strings, numbers with an imaginary `i`,
`#predefined` and `@parameter` symbols, `$directives`, sections, block
words and types without case, calls and entry names -- and the Formula
Editor highlights its buffer in its file's language. A memo slot a
language. Left: underlining a check's errors in the text (the editor keeps
only the rendered messages, not their spans).

## P2. Defaults for better-looking 3D, and Lit matching Path traced

"Defaults for better looking 3D, also adjust Lit so it better matches Path
Tracing. I'm thinking Shading Strength 1.0, Ambient 0.0, Shadows 0.98.
Color Gamma 1.0, Exposure 2.0, Saturation 1.0. It may be copying over
settings from flames, which don't always fit well with 3D."

What the survey found:
- Entering Escape from a flame resets the tone map only when it was Log
  (to Linear, exposure 2, gamma 1); everything else carries over --
  saturation (the startup flames use 1.8 and 3.0), hue shift, highlight
  mode, the tone curve, the background, and the flame's lighting rig.
- The solid presets omit exposure and gamma, so they load with the flame
  defaults, **1.0 and 4.0**. The escape image is linear light and the tone
  map encodes sRGB itself, so gamma 1.0 is the one correct encode; 4.0
  flattens it. Entering by the menu (2 / 1) and loading a preset (1 / 4)
  look different.
- Every solid's `shadow` parameter defaults to 0.7, the terrain's too; the
  solid presets set none.
- The lit tier's occlusion darkens the **direct** light too
  (`ndotl * ao`), which the path tracer has no counterpart for; and the
  lit tier has no indirect light at all. Rendered side by side with the
  proposed settings (`output/look/`), the two tiers agree closely but for
  the lit tier's darker recesses.

Steps:
1. **The 3D look on entry.** Entering a 3D escape view -- a solid formula
   picked, a terrain switched on, or the Escape mode entered with one --
   from a view that is not one sets the tone to Linear, exposure 2.0,
   gamma 1.0, saturation 1.0, hue shift 0, and, when the lighting rig is
   untouched (a flame's default), the rig to shading strength 1.0 and
   ambient 0.0. One undo step.
2. **Shadows 0.98** as the solids' and the terrains' default. A config
   version (5) keeps 0.7 for files saved before, whose `shadow` was
   omitted as the default.
3. **The presets** get the look explicitly (the 18 solid presets: tone,
   rig, shadows), re-inspected on a contact sheet; a colouring that reads
   too dark or too bright gets its exposure.
4. **Lit toward Path traced:** occlusion on the ambient term only, as the
   path tracer's has no counterpart for occluded direct light; measured
   against the path tracer on the Menger and the Mandelbulb (mean and
   recess brightness), and an indirect-light approximation added only if
   the measurement says it closes a real gap.

Gates: the entry paths set the look (one undo step, the rig left alone
when touched); a v4 file without `shadow` loads with 0.7; Lit against Path
traced on both solids, before and after; the visual suite's lit solids and
terrains move only as step 4 says.

**Done 2026-10-19.** `ui::render_mode::look_3d` and `apply_with_look`
(the mode switch, the formula picker, both terrain switches, the Formula
Editor's Use; `entering_a_3d_view_gives_it_the_3d_look`). Shadows 0.98
for the five solid defs (the Rhai template's comes from the Mandelbox's)
and both terrains, config v5 keeping 0.7 for older files
(`shadows_saved_before_v5_keep_their_default`). The 19 solid presets carry
the look (Linear, 2.0, 1.0; strength 1, ambient 0; the def's shadow); on
the contact sheet the side away from the light is dark, as ambient 0
means -- a fill light is the user's to add. Step 4: occlusion on the
ambient only. Lit minus Path traced at the defaults, mean over the
surface (0-255): the Mandelbulb -23.4 to -17.9 (mean |d| 25.0 to 20.4),
the Menger -10.0 to -8.8 (11.5 to 10.5). An indirect approximation (the
albedo times the occluded share times the diffuse) widened the mean
|d| and was left out; what remains is the path tracer's bounce light, which the
lit tier does not model. Gate: `occlusion_leaves_the_direct_light_alone`
(the sponge, no ambient: lit over path 1.0004 now, 0.965 before).

## P3. A Lighting & Material panel

"Consolidate all Lighting, Material & Shadow settings. Shadow settings
shouldn't be in the Escape Fractal panel, see things like KIFS. Texture
settings in the new panel. I want to review what we have and where. All the
3D lighting & shadow settings should be there."

Where they are now (the survey, 2026-10-19):
- **View panel, "Lighting & Material"** (collapsed): the render tier and
  the path tracer (sky gradient, samples, bounces, denoise, Sky light), the
  terrains' shadows, the rig (strength, ambient, occlusion strength), the
  four lights, the material (diffuse, highlight, gloss, roughness, glow).
  Path-tracer aperture and focus are in the View panel's Depth of Field.
- **Escape panel, the formula's parameters:** a solid's **Shadows, Shadow
  Sharpness and Occlusion Reach** (formula parameters of every solid def
  and the Rhai template), drawn among the fold, scale and the rest.
- **Escape panel, Relief Shading:** the terrain's **surface texture**
  (also partly in the Textures panel as "Relief bump").
- **Escape panel, Terrain:** the lake's **tint and roughness**.
- **Colors panel:** the **background colour**, which is also the sky, the
  environment light and the fog colour of every 3D view.
- The old Solid Lighting panel type still exists, out of the Window menu.

Steps:
1. A dockable **Lighting & Material** panel (the Solid Lighting panel type
   renamed and restored to the Window menu, available where a view lights
   a surface): **Rendering** (tier, path tracer), **Sky** (the background
   colour as the sky, the gradient, Sky light), **Lights**, **Shading**
   (strength, ambient, occlusion strength and reach), **Shadows** (a
   solid's three, a terrain's three, a 3D flame's shadow maps -- one
   section, whichever the view has), **Material** (diffuse, highlight,
   gloss, roughness, glow, P4's additions), **Surface** (the terrain's
   texture, the lake's tint and roughness).
2. The Escape panel stops drawing `shadow`, `shadow_sharpness` and
   `occlusion` among a solid's parameters (they stay formula parameters in
   the file: no migration), and its Relief section keeps only the 2D
   relief; the View panel keeps the camera, Atmosphere and Depth of Field
   and points to the new panel.
3. The rig's rows show from the start, not only once touched (the untouched
   default stated beside them).
4. Names: the five things called Shadows and the four called Ambient,
   Elevation, Highlights, Softness each say whose they are.

Gates: the panel lays out in every view with every section; each setting
is drawn in exactly one place (a test over the panels' paths); the Escape
panel no longer draws the shadow parameters.

**Done 2026-10-19.** `ui/lighting_panel.rs` (was `solid_panel.rs`), in
the Window menu after View (desktop and compact); its sections by view
are tabled in UI.md, *The Lighting & Material panel*. The View panel's
section became a button that opens it. The Escape panel leaves a
solid's `escape::ifs::LIGHTING_PARAMS` out of its list, and a terrain's
texture and lake, and says where they went; a plane's relief keeps its
texture. Step 3: an untouched rig's rows show what the first change
makes it, and that change takes the rig over with the 3D look's values
(`render_mode::rig_taken_over`) -- which also fixed P2's entry look
swinging a terrain's sun from the default (135°, 30°) to light 0's
stored (35°, 40°). Step 4, reviewed: with the 3D settings out of the
Escape panel, every remaining same-named control (the relief's Shadows,
Highlights, Ambient, Elevation, Softness; a terrain's Softness) sits
under a section that says whose it is, so nothing was renamed; the
tier's row is now "Render tier" (it read "Rendering" inside Rendering).
The gate is the text the panels draw (egui's shapes, in a test): a
solid's shadow parameters in this panel and not the Escape panel's, a
terrain's texture and lake likewise, the rig not in the View panel
(`a_solids_light_is_drawn_in_the_lighting_panel_alone`,
`a_terrains_surface_is_drawn_in_the_lighting_panel`,
`the_first_change_takes_an_untouched_rig_over`). Left: the background
colour is drawn in Colors and in Sky by design (it is a 2D picture's
backdrop too), and the Textures panel keeps its Relief bump shortcut.

## P4. Materials: metallic and polished

"Some renders, I think mostly from Blender, are shiny in a way I'm unable to
reproduce. They have great contrast. Our Gloss/Roughness mostly just turns
everything black and white, while Highlight has a small effect ... How do I
get things like metallic or polished material effects?"

Why it looks black and white now ([path-tracing-research.md](path-tracing-research.md),
*What is wrong now*, 5-6): the coat's reflectance is one number, so its
reflections are white; at gloss toward 1 the colour goes with the diffuse;
the environment is the background, near black on the usual backdrop, so
reflections are near black; lights never appear in reflections (no MIS);
Linear with Clip clips the highlights. Blender's look is a metallic
workflow, energy-conserving GGX, a lit environment (an HDRI or a studio)
and AgX.

Steps, each gated on the Mandelbulb (curved: it sweeps the environment and
shows the Fresnel rise) and the Menger, gloss 0 / metallic 0 bit-identical
(the coat's code is skipped exactly as before when neither is set):
1. **Metallic** (a skip-if-default `metallic`): the coat's reflectance a
   colour, `mix(vec3(gloss), albedo, metallic)`, the diffuse times
   `1 - metallic`; both tiers. The lit tier reflects the environment
   prefiltered by roughness, or a metal is black there.
2. **Energy compensation** (Kulla and Conty 2017; Turquin 2019): an
   analytic estimate of the lobe's lost energy (Karis 2014's
   EnvBRDFApprox), no table, so rough metals keep their brightness.

   **Steps 1-2 done 2026-10-19.** Karis's fit was up to 47% off for our
   separable Smith masking (it is fitted to the height-correlated form),
   so the albedo is a 16-term fit to our coat's own integral
   (`path_core::COAT_ESS_FIT`), generated into every shader. Fitted from
   cos 0.25 and clamped there it gave a rough metal 24% too much at a
   glance; fitted over the whole range, the compensation is within 1.6%
   from cos 0.2 up and -11% to +5% at a glance. A white metal under a
   uniform sky: 69% of it at roughness 0.7 before, 100.2% after. The
   lit tier's metal is the path tracer's within 0.3% on the sponge and
   the Mandelbulb. Metallic is in every tier, the 3D flame's shade pass
   included; the lit tier reflects only the ambient (step 4's
   environment is the path tracer's), so a metal there reads dark under
   ambient 0, as it does in the path tracer under a black sky.
3. **MIS** (research V1): bounces that escape into a light add it, by the
   power heuristic; polished surfaces reflect the lights.
4. **The environment bounces see, apart from the backdrop the camera
   sees**: a procedural studio (a ground, one to three soft panels or a
   sun disc) as well as the uniform and gradient skies. An HDRI waits on
   the user (it reopens 2026-10-02's decision to skip image files).

   **Done 2026-10-19** (TERRAIN.md, *The studio*): Studio light, a
   softbox overhead and behind, two strips, a front fill, walls and
   floor, Sky light its brightness, the camera's frame for a solid and
   the world's for a terrain. Both tiers: the path tracer's escaped
   bounces see it; the lit tier has closed forms -- the horizon-clipped
   irradiance (within 1.3%) and a cone-overlap reflection blended to the
   irradiance as the coat roughens (within 5% in the mean; a flat face
   at a light's edge a third to a half off). Spherical Gaussians were
   tried and dropped (8% off in the mean, twice the softbox at its
   centre). The first layout tilted the softbox toward the camera, which
   no top face seen from above mirrors; it sits behind the subject now.
5. **Light size** apart from the lit tier's shadow sharpness: broad
   highlights on polished surfaces.
6. **AgX** as a highlight mode beside Filmic (the Narkowicz ACES fit, which
   skews hue in highlights).
7. Presets showing it: a polished Mandelbulb, a metal KIFS.

## P5. Flame variations in solids, and orbit modifications for colouring

"I'd like the flame variation shapes and also orbit modifications for
coloring."

1. **C3 for solids:** a Rhai solid calls a built-in flame variation in its
   step, `vary("spherical3D", v, #{ param: value })`, its 3D body applied
   to the vector. Each call is a slot of the escape view's maps flame (the
   lens's and the transformations' group), built in 3D when a solid calls
   one (`flame_map3`); its parameters constants or the solid's own,
   evaluated each frame. GPU only: `prepare` and the interpreter refuse it.
   In a solid that calls one, the lens uses the variations' 3D bodies at
   z = 0.
2. **Orbit transformations for solids' colourings:** C4's Orbit stage in
   mode D -- a variation applied to each orbit value a solid's colouring
   measures (the trap's point, the address's fold), not to the walk.
3. Gates: a solid calling `linear3D` at 1 is the solid; a call of a
   variation equals the same map written in Rhai (a twin) by coverage; an
   Orbit `linear3D` changes nothing; an Orbit translation moves the trap
   colouring as the trap moved.

## P6. Fly mode: collisions, and speed by proximity

"Collisions in Fly Mode (path/ray tracing)? Another option to adjust
movement speed relative to proximity for 'infinite' zoom?"

What the survey found: fly mode steps a fixed multiple of the camera's
scale (its distance to the target), never changes the zoom, and knows
nothing of the surface; no solid's distance reaches the CPU outside tests
(the built-in solids' CPU twins are test code); the terrain has an
asynchronous GPU probe of the nearest ground in view (`probe_nearest`).

Steps:
1. **A distance probe for mode D:** a one-thread pass evaluating the
   solid's own distance at the eye (`ifs_distance_at(eye_rel)`, every
   solid kind, Rhai included, at the render's precision), read back
   asynchronously as the terrain's probe is.
2. **Collisions:** a fly step never moves further than a fraction of the
   last measured distance less what was flown since -- a sphere-traced
   step, safe in every direction.
3. **Speed by proximity** (an option): the step a fraction of the distance;
   and the zoom follows it -- the camera's scale kept near the distance,
   the eye held where it is (the target moved along the view) -- so a
   flight into the surface is a deep zoom, as far as each solid's precision
   goes (the IFS flame solids' seeded walk; f32 for the others).
4. Terrains: the same from a probe straight down and the frustum's.

## P7. Path tracing: research and plan

"Anything to speed up or improve the quality of Path Tracing is something we
should plan on doing ... I'd like to research this more before we make any
major decisions that rewrite Path Tracing."

Research only: [path-tracing-research.md](path-tracing-research.md) --
written 2026-10-19: what is wrong now (a noisy denoiser guide, an
underestimated variance, a normal stop that blotches flat faces and
spares curved ones, a clamp on the whole path, no MIS), the options
ranked, what not to do, the gates, and the order proposed for the user's
decision. P4 takes the parts the materials need (MIS, metallic, energy
compensation, the environment, AgX).

## Order of work

1. [x] P1 syntax highlighting.
2. [x] P2 the 3D look and Lit toward Path traced, 2026-10-19.
3. [x] P3 the Lighting & Material panel, 2026-10-19.
4. [ ] P4 materials (after P7's research).
5. [ ] P5 flame variations in solids, Orbit colouring.
6. [ ] P6 fly mode.
7. [x] P7 research written up (alongside), 2026-10-19.
