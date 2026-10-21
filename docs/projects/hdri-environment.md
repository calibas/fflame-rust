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

## Order of work

1. [ ] **Choose the skies** (the user): one cloudy -- overcast (soft,
   shadowless, no sun to sample) or partly cloudy (a sun, sharper
   shadows and highlights, needs step 3's sampling) -- and the night one
   (Qwantani Night, or another of the five night pure skies). Credit
   wording.
2. [ ] Assets and loading: the catalog, the `hdr` feature, the 4K backdrop
   script, desktop and web loading, the GPU texture (`rgba16float`).
3. [ ] The path tracer: the map as the sky's light and the backdrop,
   rotation; importance sampling or sun extraction, measured on a metal
   and a glossy solid (`denoise_quality`'s cases under the new sky).
4. [ ] The lit tier: irradiance and prefiltered reflections, gated against
   the path tracer.
5. [ ] Procedural stars.
6. [ ] Terrains (the world's frame) and the simulation terrain.
7. [ ] The UI (the Sky section: Background / Gradient / Studio / the
   skies; rotation; show as backdrop), presets showing it, the docs.

## Decisions for the user

- **Which skies** (step 1).
- **A 4K backdrop JPEG** for the cloudy sky (sharp, about 2 MB more), or
  the 1K map alone (soft backdrop, light and reflections the same).
- **The night sky:** procedural stars over Qwantani Night's glow (1.4 MB),
  or stars alone (no file at all).
- Whether the skies join the Studio as choices in one list, or stand
  beside it.
