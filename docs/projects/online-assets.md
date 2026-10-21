# Online skies and textures: Poly Haven first, other providers later

Drafted 2026-10-21 at the user's word: "I'm considering an online API
integration with Poly Haven and our Fractals for All site. You'd select
from a list, and download HDRIs and textures. Discuss and plan this
first with me before building anything." Decided the same day (below).
**Nothing here is built yet.**

It reopens, for assets named by provider and id, the 2026-10-02 decision
to skip image files. That decision's reasons -- a path that breaks on
other machines, a bloated `.fflame`, a read-only `assets/`, a shared
file reading a private image (docs/archive/projects/sim-textures.md) --
are about user files; an asset a config names, which any machine can
fetch from its provider, has none of them.

## Decided (2026-10-21)

The user's answers, in order:
1. **Providers' lists, downloaded and kept locally.** "Get HDRI/texture
   lists from Poly Haven, and other APIs later on, the user can select
   from whatever is offered, then it's downloaded from that site and
   stored locally." The app talks to each provider directly; our site is
   not in the path. Poly Haven is the first provider, behind an interface
   the next ones implement.
2. **Everything a provider offers**, skies with ground included. (The
   first draft had leaned to "pure skies" for a floating fractal: an
   aesthetic assumption, not a limit -- a photographed ground lights from
   below and reflects correctly; Horizon moves it.)
3. **Every resolution the provider offers**, the user's choice.
4. **The web build keeps downloads for the session only**, for now. The
   desktop keeps them in its data folder.
5. **Textures on solids and terrains**, with separate targets for
   valleys, slopes and plateaus, each replacing the colouring's colour or
   blended with it.
6. **Contacting Poly Haven** (their API page asks it): the user's.

## Poly Haven, as read 2026-10-21

- **The API**: `https://api.polyhaven.com` -- `/assets?type=hdris` (or
  `textures`; `&categories=` to filter), `/info/{id}`, `/files/{id}`,
  `/categories/{type}`, `/search`. 997 HDRIs and 869 textures. A
  `/files` entry lists each resolution (1K up to 16K, some 24K) and
  format with its URL, size and md5; our two bundled skies are
  byte-identical to their 1K files.
- **Sizes** (Sunflowers): `.hdr` 1K 1.5 MB, 2K 5.7 MB, 4K 21 MB, 8K 76 MB,
  16K 266 MB; `.exr` about four times that. A texture is a set of maps
  -- diffuse, normal (OpenGL and DirectX), roughness, AO, displacement,
  an ARM pack -- each 1K-8K in jpg, png or exr: a 1K diffuse jpg 0.2 MB,
  4K 2.9 MB.
- **Thumbnails** from their CDN, resized on request:
  `cdn.polyhaven.com/asset_img/thumbs/{id}.png?width=256&height=256&format=jpg`,
  about 11 KB (we decode png, jpeg and hdr; not webp).
- **Terms** ([API ToS](https://github.com/Poly-Haven/Public-API/blob/master/ToS.md),
  changed 2026-07 to allow any use): "free to access and use by anyone
  ... including commercial use, at no charge" (2.1); calls "must be made
  with a unique "Referer" header or user-agent that matches your software
  name" (2.4); surfacing content from the live API, "you must make it
  clear to your users where that content comes from" -- "Powered by Poly
  Haven" near the content is enough (2.5), which "does not apply to the
  CC0 assets themselves once downloaded or self-hosted"; nothing that
  degrades the API (2.6); no rate limit published. The assets are CC0.
- **The browser can reach it**: the API, the file host (`dl.polyhaven.org`)
  and the CDN answer `Access-Control-Allow-Origin: *`. A browser cannot
  set a User-Agent but sends the page's origin as the Referer (2.4
  accepts either). Our signed-in fetch (`credentials: include`) fails
  against a `*` origin; a plain fetch works.

## Design

**Providers** (`src/assets/`, new): a provider lists its HDRIs and its
textures (id, name, categories, tags, authors, a thumbnail), lists one
asset's files (each resolution and format: URL, size, md5), and names
the credit its terms ask for. Poly Haven implements it over its API;
the next provider implements the same. The lists are fetched when the
picker opens and kept for the session.

**A config names an asset, never holds it**: the sky as
`polyhaven:<id>` (the bundled ones keep their short ids) and the
resolution chosen (`environment_resolution`, e.g. `4k`); a texture
likewise. The `.fflame` stays small and opens anywhere that can fetch
it. Opening one whose asset is not stored: with online mode on, it is
downloaded (a notice, a progress bar) and the picture fills in; without,
the background stands in, as for a missing bundled sky now.

**Fetching and keeping** (any provider):
- Desktop: ureq on a background thread, identified as
  `FractalArtEditor/<version>` (ToS 2.4), bodies past ureq's 10 MiB text
  cap read as bytes; stored under the data folder,
  `downloads/<provider>/<kind>/<id>/<file>`, checked against the md5. A
  Downloads list in the picker shows what is stored and what it takes,
  with Delete.
- Web: `fetch` to an `ArrayBuffer`, no credentials (the Referer
  identifies us), kept in memory for the session (decision 4); the two
  bundled skies are fetched from `pkg/assets/environments/` when picked,
  so the web app's download does not grow.
- Downloads run off the frame, with progress and cancel, as exports do.
  Only in online mode (`SystemSettings::online_mode`), as every other
  network call.

**Every resolution, made usable** (decision 3). What a GPU and memory
hold sets two limits a large file meets: WebGPU's default texture side is
8192 (most desktop adapters allow 16384), and a 16K sky decoded whole in
f32 is 1.6 GB. So:
- **A streaming Radiance decoder** that reads the file's run-length
  scanlines and reduces as it reads: memory is a few rows, not the image
  (the `image` crate decodes whole). EXR is not decoded (its decoder is
  large); `.hdr` is offered for every sky.
- **Two maps from one file**: the light at 1K as now -- the sun lifted
  out, the irradiance's harmonics, the reflection levels -- and, when the
  file is larger, a **backdrop** at its own resolution, reduced to the
  device's texture limit, read by camera rays only. Lighting costs the
  same at any resolution; a sharper sky costs only its texture (4K: 64 MB
  in half floats; 8K: 256 MB).
- The picker lists every resolution with its download size, and marks
  those past the device's texture limit as reduced on loading.

**The sky's level** (how bright its median shows): measured on loading,
set by rule -- a provider's night category, or a median far below a
day's, at 0.2; others 0.6, as the bundled ones -- and Sky light adjusts
it.

**The picker**: a window -- provider, then Skies or Textures; categories
and search; a grid of thumbnails (jpg from the provider's CDN), each with
its name, authors and sizes; the resolution, Download, then Use. The
provider's credit on the window ("Powered by Poly Haven") and under a
chosen sky, as now.

**Loading off the frame**: decoding and making the reflection levels
takes about 0.2 s for a 1K sky on the desktop, more for a large file and
on the web; it runs as a background job, the old sky drawn until the new
one is ready.

## Textures on solids and terrains (decision 5)

Downloading a texture is the machinery above. Using one is a rendering
project of its own: its plan is to be written once the sky half is in,
with these starting points.
- **Its maps**: the diffuse as the albedo, the normal map as bump, the
  roughness into the coat, AO into the occlusion. Displacement moves a
  surface, which a distance field cannot take from an image: read as
  bump, or left out.
- **Placement**: our surfaces have no UV coordinates. A terrain's ground
  takes a texture in the plane, its steep faces from the side
  (triplanar); a solid takes it from three sides in the fractal's frame,
  so it stays put as the camera moves.
- **Targets**: valleys, slopes and plateaus each their own texture. On a
  terrain: a slope by its steepness (the normal's tilt); valley and
  plateau by the ground's curvature or its height against its
  neighbours (concave or low; convex or high and flat); blended across
  the boundaries. On a solid, the same three by the fractal's up
  direction and its concavity (crevices as valleys) -- to settle with the
  user.
- **Replace or blend**: per target, the texture's colour in place of the
  colouring's, or mixed with it (multiplied, overlaid, or by a weight).
- **Both tiers**: the lit tier and the path tracer read the same maps,
  as they read the same material now.

## Order of work

1. **Fetching and keeping**: binary download on both platforms (desktop
   background thread, user agent, md5; web `fetch`), the store (desktop
   folder, web session), progress and cancel.
2. **The provider interface and Poly Haven's**: lists, files,
   thumbnails, credit.
3. **The sky picker**, and configs naming `provider:id` with a
   resolution; a missing asset fetched on opening.
4. **The streaming decoder, the backdrop map, the level rule**: every
   resolution usable.
5. **The web build's skies**: the bundled ones from `pkg/`, downloads for
   the session.
6. **Texture downloads** in the picker (the maps a texture needs, at the
   chosen resolution).
7. **Texture materials**: their own plan first (above), then terrains
   with the three targets, then solids.

## Still open

- **A solid's targets**: the same three by orientation and concavity, as
  above, or one texture all over.
- **Our site's part**, for later: none in the path now (decision 1); a
  provider of its own one day (curated or user skies), through the same
  interface.
