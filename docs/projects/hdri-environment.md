# HDRI environments: skies that light and are reflected

Planned 2026-10-20 at the user's request: "I do want quality reflections,
so I'd like to plan on HDRI support. I'd like to include a couple
skyboxes, like one with a cloudy sky and another with stars ... most of
the HDRI's I'm seeing are 50+ MB, they'd double our file size. I'm
wondering if there's versions around 1-5 MB we can use?" **A plan:
nothing here is built.** It reopens, for bundled skies only, the
2026-10-02 decision to skip image files (user-imported images keep the
old storage problem; a sky here is a bundled asset named by id, as a
palette pack is).

## What it is

An environment map: a 360-degree high-dynamic-range photograph of a sky,
in the equirectangular layout (2:1), that **lights the scene and is what
its reflections show**, and that the camera may see as the backdrop.
Today the path tracer's sky is the background colour, a two-colour
gradient or the procedural Studio (TERRAIN.md, *The studio*); a polished
or metal surface reflects only those, which is why metals read flat. The
lit tier lights by the same sky through analytic approximations, so Auto
does not jump.

Nothing needs an HDR display: the map's range is in the light, and the
picture is tone mapped to the display as now. (HDR *output* -- EXR
export -- is a separate item on the future list.)

## Sizes and sources

[Poly Haven](https://polyhaven.com/hdris) publishes every sky at several
resolutions, CC0 (any use, no attribution required; they ask for a credit
link). Its "pure skies" have the ground replaced -- the right kind for a
fractal floating in space. Radiance `.hdr` sizes, from its API:

| Sky | 1K | 2K | 4K |
|---|---|---|---|
| [Kloofendal Overcast](https://polyhaven.com/a/kloofendal_overcast_puresky) | 1.17 MB | 4.3 MB | 17.0 MB |
| [Kloofendal 48d Partly Cloudy](https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky) | 1.44 MB | 5.5 MB | 20.7 MB |
| [Qwantani Night](https://polyhaven.com/a/qwantani_night_puresky) | 1.39 MB | 5.5 MB | 21.2 MB |

(EXR is about four times the size; the 50+ MB files are the 8K-16K ones.)
What each resolution is for:
- **Light and reflections: 1K.** A rough or glossy fractal blurs the
  environment anyway; 1K holds the sun and the clouds' light whole.
- **The visible backdrop: more.** A 60-degree view shows about a sixth of
  the map's width, so at 1K about 170 pixels across the screen: soft. A
  separate **4K 8-bit JPEG** of the same sky, made once from the 4K
  `.hdr` (`scripts/`), is perhaps 1.5-3 MB.
- **Stars: procedural.** At 1K a star is smaller than a pixel and
  vanishes in the filtering; only 8K+ holds them. A star field generated
  in the shader costs no file, is sharp at any zoom, and shows as points
  in a polished reflection. The night sky's 1K `.hdr` underneath gives
  the Milky Way's glow and the moonlight.

So about **3-4 MB for the cloudy sky and 1.4 MB for the night one**. On
the web a sky is fetched when picked (as palette packs are), so the web
app's download does not grow; the desktop install grows by those files.

## Design

- **Assets:** `assets/environments/<id>/` -- `light.hdr` (1K), an optional
  `backdrop.jpg` (4K), and a catalog (`environments.json`: id, name,
  credit, the light's scale, whether to draw procedural stars). Desktop
  reads the folder (the presets' and palette packs' pattern); WASM fetches
  from `pkg/assets/environments/` with a manifest `build.rs` generates.
  The `image` crate's `hdr` feature decodes Radiance (small, pure Rust);
  EXR is not supported (its decoder is large).
- **Config** (both path-tracing blocks, `escape.path` and
  `sim.terrain.path`; skip-if-default): `environment_map` (an id, empty
  for none), `environment_rotation` (degrees about the up axis),
  `environment_horizon` (the elevation its horizon stands at, degrees;
  added 2026-10-20 at the user's request, to align it with the fractal),
  `environment_backdrop` (the camera sees the sky, or the background as
  now). Sky light (`environment`) is its brightness. A file naming a sky
  this build does not have falls back to the background, with a notice.
- **The path tracer** (`pt_sky`, `pt_sky_seen`): the map in place of the
  sky's light, in the studio's frame (a solid's fixed in the fractal or
  following the camera, a terrain's the world's); the backdrop for a
  camera ray that misses. **The sun needs importance sampling** -- a
  bounce rarely finds a small bright disc, so a partly cloudy sky would
  be fireflies: either a 2D distribution over the map's luminance (a
  texture; textures do not count against the storage-buffer limit the
  solid's tracer is near), or the sun found and lifted out as one of the
  rig's directional lights, with its shadow rays and MIS, the rest of the
  map smooth. Measured before choosing.
- **The lit tier** (`ifs_studio_diffuse`, `ifs_studio_reflect`'s
  counterparts): the map's irradiance as spherical harmonics (nine
  coefficients, computed on loading) and its reflection prefiltered by
  roughness into a mip chain, read with the coat's albedo fit (the
  split-sum approximation). Gated as the studio was: the diffuse within a
  few percent of the path tracer's, a coat's reflection within 5% in the
  mean.
- **Stars** (WGSL, in the rig): cells over the sphere, a star in some,
  its brightness by a power law and its colour by temperature, a fixed
  small angular size; drawn in the backdrop and in reflections, too faint
  to light anything.
- **Exposure:** the backdrop JPEG is display-referred, as the background
  colour is (`shown`): it reads as itself at any exposure. The light map
  is scaled by the catalog's factor so Sky light 1 lights a white surface
  about as the backdrop shows the sky.

## The user's choices (2026-10-20)

"I found ones I like better" -- in `assets/environments/`, both Poly
Haven CC0 at 1K (1024x512 Radiance RLE):
- [**Sunflowers (Pure Sky)**](https://polyhaven.com/a/sunflowers_puresky):
  midday, partly cloudy, high contrast, 1.48 MB.
- [**Rogland Clear Night**](https://polyhaven.com/a/rogland_clear_night):
  stars and the Milky Way, 1.69 MB -- not a pure sky: its lower half is
  the photographed desert, a landscape below the horizon.

The goals: "I want to be able to do high-quality reflections, doesn't
have to be perfect mirror kind of quality, but I want shiny materials to
look correct. And the other goal is skyboxes."

**Their light, measured** (`scratchpad` decoder: luminance by solid
angle):

| Sky | peak | median | the brightest 0.01% of texels hold | the upper half holds |
|---|---|---|---|---|
| Sunflowers | 61 501 | 0.197 | **47.5%** of the light | 82% |
| Rogland Clear Night | 37.2 | 0.080 | 0.4% (1% hold 5.6%) | 85% |

So **the sun is half of Sunflowers' light in about 50 texels**: a bounce
that does not aim at it finds it by chance, and half the light becomes
fireflies; the lit tier, lighting by a smooth irradiance, would draw no
sun shadow. Hence the sun is **lifted out of the map as a light** (as
Blender's EEVEE does with a world's "sun threshold"): the texels above a
threshold, gathered around the brightest, become a directional light --
its direction their centroid, its colour and power their light, its
angular size theirs -- **the rig's fifth light**, so both tiers shadow
it with the code they have, and the map keeps the rest, smooth enough
for the bounces' own sampling. Rogland needs nothing of the kind.

**Skyboxes at 1K are soft** (about 170 of the map's pixels across a
60-degree view). Good for light and glossy reflections; as a backdrop,
the 4K versions would be sharper, kept only as a 4K JPEG (a few MB, made
once by a script) -- for the user's decision once the 1K backdrop is
seen.

## Order of work

1. [x] **Choose the skies**: Sunflowers and Rogland Clear Night (above).
2. [~] Assets and loading: **built 2026-10-20 for the desktop** --
   `escape::environment` (the catalog in code, `ENVIRONMENTS`; the `hdr`
   feature; the sun lifted out; `rgba16float` on the GPU). Left: the web
   build's loading (a binary fetch, as palette packs are fetched), and the
   4K backdrop (for the user's decision).
3. [~] The path tracer: **built for solids** -- the map as the sky's light
   (`pt_env`, in the studio's frame, turned) and the backdrop
   (`pt_sky_seen`); the sun in the rig's next free light (not a fifth: the
   lit tier caches four shadow terms), in place of an untouched rig's
   stand-in key. Tests: a one-colour map lights as the uniform sky to the
   map's half-float rounding; the sun lights the lit tier; without a sky
   the pictures are the same bits. Left: measuring metal and glossy
   solids under the skies (`denoise_quality`'s cases).
4. [ ] The lit tier: irradiance and prefiltered reflections, gated against
   the path tracer.
5. [ ] Procedural stars.
6. [ ] Terrains (the world's frame) and the simulation terrain.
7. [~] The UI: **built** -- the Sky section's HDRI sky (None and the
   skies; a solid's only until terrains take one), Rotation, Horizon,
   Show as backdrop and the credit linked; the gradient and the studio rows hide
   while a sky is chosen. Left: presets showing it.

## Decisions for the user

- **Which skies** (step 1).
- **A 4K backdrop JPEG** for the cloudy sky (sharp, about 2 MB more), or
  the 1K map alone (soft backdrop, light and reflections the same).
- **The night sky:** procedural stars over Qwantani Night's glow (1.4 MB),
  or stars alone (no file at all).
- Whether the skies join the Studio as choices in one list, or stand
  beside it.
