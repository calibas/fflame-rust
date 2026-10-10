# One material per picture (camera-unification P7, C7)

**Status:** decided and built, 2026-10-09, on `camera-unification`
(§4: the highlight kept in both tiers, one material per picture beside
the lights). What was built is §6.

## 1. Where things stand

Two material models describe the same surface, depending on the tier:

| | Lit tier | Path tracer |
|---|---|---|
| Who | the 3D flame's shade pass (`shaders/shade.wgsl`); mode D's walk and relight and both terrains' relight (one rig, `IFS_RIG` in `escape/assembler.rs`) | mode D and both terrains (`escape/path_core.rs`) |
| Albedo | the colouring | the colouring |
| Diffuse | `solid_shading.diffuse` × Lambert | the same field × Lambert, × (1 − Fresnel) under a coat |
| Highlight | Blinn-Phong: `specular` × (n·h)^`shininess`, unnormalised, direct light only | none |
| Coat | none | `gloss` (reflectance at normal incidence) and `roughness`: GGX, Smith masking, Schlick Fresnel; lights the direct light and the bounces, and reflects the sky |
| Glow | none | `emission` × albedo |
| Sky light | `ambient` × albedo × occlusion -- white | the background × `environment`, through the bounces |

Where the settings live: the highlight with the lights in
`solid_shading` (one set for every view); the coat and glow in each
engine's path settings -- `escape.path` (mode D and the escape terrain)
and `sim.terrain.path`. A flame has neither coat nor glow.

**What that costs.** Switching tier changes the material, not just the
light's accuracy -- and the Auto tier switches by itself, lit while
anything moves and path traced when it settles, so a glossy picture
loses its gloss on every drag and a highlighted one gains and loses its
highlight. `output/materials/before/sheet.png`: a solid and a terrain,
each with a coat (gloss 0.5, roughness 0.3) and with a highlight
(specular 0.6, shininess 40), lit beside path traced. The lit coat is
absent; the path-traced highlight is absent.

Shipped pictures: no shipped escape or simulation picture sets a coat,
a glow, or a highlight. Five flame pictures use the highlight
(`solid/solid-lit`, `solid-shadows`, `solid-lit-transparent`,
`3d/stereogram-smoke`, `variations/spray_blur-normal-xform-smoke`).

## 2. The proposal

**One material per picture, read by every tier**: the albedo (the
colouring), Diffuse, the coat (Gloss, Roughness), Glow -- and the
highlight, unless §4.1 retires it. Every tier evaluates all of it on
the direct light exactly as the path tracer does; what the tiers still
differ in is the indirect light, which is what a tier is.

1. **The lit tier reads the coat and the glow.** Per
   light, the path tracer's own direct-light term: GGX × Smith × Schlick,
   the diffuse taking (1 − Fresnel). For the sky the path tracer's
   bounces would see, the coat reflects the lit tier's ambient at this
   view (Fresnel at n·v) -- white at the ambient's strength, as the lit
   tier's sky light is. Glow adds albedo × glow. Gloss 0 and glow 0
   reduce to exactly the expressions the rig had, so no existing picture
   moves. `output/materials/compare.png`: lit today, lit with the coat,
   path traced. The lit coat now reads as the path tracer's does; it is
   somewhat whiter on the solid, because the lit sky is white and the
   path tracer's is the background's blue-grey (§5).
2. **The flame's shade pass reads the same coat and glow** -- the
   material settings the 3D flame never had (the original ask). Same
   term, same fields.
3. **The highlight**, per §4.1.
4. **The View panel's Lighting & Material** shows one Material block --
   Diffuse, Gloss, Roughness, Glow (and the highlight if kept) -- for
   every 3D view, labelled for every tier.

## 3. Converting the highlight is not faithful

Blinn-Phong's highlight and a GGX coat can be matched at the peak and
in width -- reflectance `8·s / (n + 2)`, roughness `(2 / (n + 2))^¼`
(Blinn-Phong ≈ Beckmann/GGX at α² = 2 / (n + 2), peak `F0 / 4α²` set to
`s`) -- but a coat is more than a highlight: it reflects the sky over
the whole surface, brightens toward grazing angles, and takes its share
from the diffuse. `output/materials/converted.png`: specular 0.6 /
shininess 40 against its converted coat (reflectance 0.114, roughness
0.467), lit and path traced. On the terrain they are close; on the
solid, whose many faces are seen at every angle, the coat washes the
colour out. The two tiers agree with each other on the converted
material -- the conversion keeps the goal and changes the picture.

## 4. Decisions

1. **The highlight (Specular, Shininess).**
   - **(a) Keep it, in both tiers** -- recommended. The path tracer gains
     the lit tier's highlight on its direct light (the same unnormalised
     term; it is not a lobe a bounce samples). No picture moves except a
     path-traced one with a highlight set, which gains the highlight its
     lit frames already showed (none shipped). Material: Diffuse,
     Highlight and its size, Gloss, Roughness, Glow.
   - (b) Retire it into the coat, converted on load (§3). One specular
     model, physically consistent; the five shipped flame pictures and
     any saved picture with a highlight change, solids most.
2. **Where the material lives.**
   - **(a) One per picture, beside the lights** (`solid_shading`) --
     recommended. It is what "one material per picture" says, and the
     lights already work this way: one set, shown for whatever 3D view
     the picture is. The coat and glow move out of `escape.path` and
     `sim.terrain.path` (which keep the render settings: samples,
     bounces, sky, environment, lens, denoise); a saved picture's are
     read into it on load (the active engine's, if both were set), and
     the old `Escape.Path.Gloss`-style paths and tracks keep working as
     aliases. Mode D and the escape terrain already share one; this adds
     a simulation terrain and a flame to that.
   - (b) Per engine, as today, plus one for flames -- no migration, but
     three stores of one idea, and a flame's would sit in
     `solid_shading` where the escape views would ignore it.
3. **The flame gets the coat and the glow** (step 2) -- assumed yes:
   it was the original ask.

## 5. Not in this step

- **The lit tier's sky is white at the ambient's strength; the path
  tracer's is the background × Sky light.** So a coat reflects white
  lit and blue-grey path traced, and the ambient level itself differs
  (0.2 × albedo against roughly the background's brightness × albedo).
  Deriving the ambient from the sky -- one sky per picture -- would move
  every lit picture, and is its own step with its own renders.
- The flame's shade pass has no sky to reflect but its ambient; its
  coat reflects that, as the escape lit tier's does.
- A terrain lake keeps its own coat (reflectance 0.02 and the lake's
  roughness) in the path tracer; the lit tier draws a lake as its tint,
  as today.

## 6. What was built

- **The material** is `SolidShadingSettings`' -- beside the lights, one
  per picture: Diffuse, the highlight (Specular, Shininess), and new
  `gloss`, `roughness`, `glow`. The path settings keep the render
  settings only. Paths `SolidGloss`, `SolidRoughness`, `SolidGlow`;
  `Escape.Path.Gloss`, `Sim.Path.Gloss` and their roughness and emission
  are aliases of them, so old tracks and scripts write the one material.
- **On load** (`lift_path_material`, a shape fix, not a version bump): a
  picture saved with its coat and glow under `escape.path` or
  `sim.terrain.path` has them lifted beside the lights -- the shown
  engine's first, the lighting's own values winning -- and saves them
  there.
- **Every tier reads it all.** The escape and simulation lit tiers
  (`IFS_RIG`) and the flame's shade pass evaluate the coat and the glow
  as the path tracer's direct light does, the coat reflecting the
  ambient sky; the path tracer evaluates the highlight as the lit tiers
  do, on its direct light. Measured: under a sun alone a coated, glowing,
  highlighted plane is the same picture in both tiers to 7e-5, where the
  material moves it by 0.17 (`a_coated_sunlit_plane_is_the_lit_tiers`).
- **The untouched rig.** A solid's or a terrain's untouched rig is a
  default sun; `rig_untouched` leaves the coat and glow out of that test,
  so giving a surface a coat never switches the sun off. The panel shows
  the coat and glow wherever a surface is lit, and the diffuse and
  highlight once the rig is in use (editing them over the default sun
  would replace it).
- **The panel**: Lighting & Material's Material block -- Diffuse,
  Highlight, Highlight size, Gloss, Roughness (with a coat), Glow -- for
  every 3D view, a 3D flame included; the path tracer's own Material
  fold is gone.
- No shipped picture moves: none sets a coat or a glow, and the only
  path-traced pictures with a rig set have no highlight.

**Corrected after use (2026-10-09).** Two things the above missed:

- **Every touched rig has a highlight.** `SolidShadingSettings`'
  default specular is the flame's 0.35, so any solid or terrain whose
  lighting was ever edited carried it -- and from P7 the path tracer drew
  it, a sun glint over a plain, on pictures whose lit frames never showed
  it (their shading strength was 0, which draws the lit tier unlit). The
  path tracer now weights the highlight by the shading strength, as the
  lit tier's `mix` does (decided with the user): at strength 0 no tier
  draws it, at 1 both draw it whole
  (`the_highlight_follows_the_shading_strength`). The panel also showed
  a solid's or terrain's lights and highlight only above strength 0,
  though the path tracer lights a touched rig at any strength; it now
  shows them whenever the rig is touched.
- **The denoiser dropped the highlight's white.** It divides the light by
  the albedo and multiplies a channel the albedo does not reflect back by
  zero; a palette's pure red or pure yellow lost the glint's other
  channels, as hard bands. Its guide now takes the white share of the
  first surface's direct light -- the highlight's and the coat's -- as it
  already took the coat's reflection of the sky
  (`a_highlight_keeps_its_colour_denoised`).

## 7. Gates

Per step: the visual suite unchanged except where §4 says a picture
moves, with those renders before/after; GPU tests that the lit tier's
direct term equals the path tracer's (one bounce, no sky, no noise) for
a coated surface; the shader dumps regenerated deliberately.
