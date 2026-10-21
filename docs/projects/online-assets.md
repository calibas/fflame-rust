# Online skies and textures: Poly Haven and Fractals for All

Drafted 2026-10-21 for discussion, at the user's word: "For the web
build, and the 4K question, I'm considering an online API integration
with Poly Haven and our Fractals for All site. You'd select from a list,
and download HDRIs and textures. Discuss and plan this first with me
before building anything." **Nothing here is built.** The decisions at
the end are the user's; the order of work follows from them.

It reopens, for assets named by id (as the bundled skies did), the
2026-10-02 decision to skip image files. The reasons that decision gave
-- a path that breaks on other machines, a bloated `.fflame`, a
read-only `assets/`, a shared file reading a private image
(docs/archive/projects/sim-textures.md) -- are about user files, and none
apply to an asset a config names and any machine can fetch.

## What is there to build on

**Poly Haven** (read 2026-10-21; scratch responses kept out of the repo):
- **The API**: `https://api.polyhaven.com` -- `/assets?type=hdris` (or
  `textures`; `&category=pure-skies`), `/info/{id}`, `/files/{id}`,
  `/categories`, `/search`. 997 HDRIs (59 pure skies, 299 skies, 61
  night) and 869 textures. A `/files` entry lists each resolution and
  format with its URL, size and md5; our two bundled skies are
  byte-identical to their 1K files.
- **Sizes** (Sunflowers): `.hdr` 1K 1.5 MB, 2K 5.7 MB, 4K 21 MB, 8K 76 MB;
  `.exr` about four times that. A texture is a set of maps (diffuse, two
  normal conventions, roughness, AO, displacement, an ARM pack), each
  1K-8K in jpg, png or exr: a 1K diffuse jpg 0.2 MB, a 4K one 2.9 MB.
- **Thumbnails** from their CDN, resized on request:
  `cdn.polyhaven.com/asset_img/thumbs/{id}.png?width=256&height=256&format=jpg`
  about 11 KB (we decode png, jpeg and hdr; not webp).
- **Terms** ([API ToS](https://github.com/Poly-Haven/Public-API/blob/master/ToS.md),
  changed 2026-07 to allow any use): "free to access and use by anyone
  ... including commercial use, at no charge" (2.1); calls "must be made
  with a unique "Referer" header or user-agent that matches your software
  name" (2.4); surfacing content from the live API, "you must make it
  clear to your users where that content comes from" -- a label naming
  Poly Haven, or "Powered by Poly Haven" near the content (2.5), which
  "does not apply to the CC0 assets themselves once downloaded or
  self-hosted"; nothing that degrades the API (2.6); no rate limit
  published. [Their API page](https://polyhaven.com/our-api) asks a tool
  integrating it on the user's side to contact them, since endpoints may
  change. The assets are CC0: redistributing and mirroring them is
  allowed.
- **The browser can reach it**: the API, the file host (`dl.polyhaven.org`)
  and the thumbnail CDN all answer `Access-Control-Allow-Origin: *`. A
  browser cannot set a User-Agent, but sends the page's origin as the
  Referer (2.4 accepts either). Our signed-in fetch (`credentials:
  include`) cannot be used against a `*` origin; the plain resource fetch
  can.

**Our side** (the code as of 2026-10-21):
- The Fractals for All API (`src/api/`, docs/main/ONLINE.md) has flames,
  animations, palettes, scripts, variations and effects; no skies or
  textures, and its only binary payloads are thumbnails. Its desktop
  client sets no User-Agent (ureq's own) and reads text only, capped at
  10 MiB.
- No binary HTTP download exists yet, on either platform. The desktop
  data folder holds caches as files; the web build has localStorage only
  (5-10 MB for the whole origin, text): a sky would not fit. IndexedDB
  and the Cache API are not wired.
- A sky today is a `'static` catalog entry (`escape::environment::EnvDef`)
  whose display level is hand-set; the renderer loads it synchronously
  from `assets/`. An online sky needs a catalog built at runtime, bytes
  in place of a path, a level chosen by rule, and loading that does not
  block a frame.

## The options for where the assets come from

| | A. Poly Haven, live | B. Our site hosts a curated mirror | C. Our catalog, Poly Haven's files |
|---|---|---|---|
| What the app asks | their API for the list, their CDN for files | our API for both | our catalog for the list, their CDN for files |
| Choice | all 997 skies and 869 textures, their categories and search | what we put up | what we list |
| Our cost | none | storage and bandwidth (a 4K sky 21 MB a download) | a JSON file |
| Their terms | identify the app, credit in the picker, tell them | none past CC0 (we credit anyway) | identify the app; the credit (we would show it) |
| What only it can do | -- | our own derived files: a 4K JPEG backdrop (2-3 MB), precomputed levels; our own and users' skies | curation and our metadata (level, stars) without hosting |
| If they change their API | the picker breaks until we follow | nothing | nothing for the list; files by URL |

A config would name an asset the same way under any of them -- its source
and id, e.g. `polyhaven:sunflowers_puresky` -- never the image, so a
`.fflame` stays small and opens anywhere that can fetch it. A sky it
names but cannot get falls back to the background, with a notice, as a
missing bundled sky already does.

**Recommendation: A, with the two bundled skies kept, and C's catalog
left for later.** It needs nothing of the API repository; the whole
library browses in the app; the terms ask only what we would do anyway.
Our site comes in when we want what only it can do -- curation, our own
skies, the 4K JPEG backdrops -- and a config's `polyhaven:` names stay
valid when it does. We should write to Poly Haven before release, as
their page asks.

## What the app would need, any option

1. **Fetching bytes**: desktop through ureq, identified as
   `FractalArtEditor/<version>`, bodies past 10 MiB; web through `fetch`
   to an `ArrayBuffer`, no credentials. Off the frame: a download runs in
   the background with progress, as exports do; the md5 from `/files`
   checks it.
2. **A cache that survives restarts**: desktop, files in the data folder
   (`environments/`, `textures/`) with a size cap; web, the Cache API
   (keyed by URL, a quota in the hundreds of MB where localStorage has a
   few). The two bundled skies stay in the install; on the web they are
   fetched from `pkg/assets/environments/` when picked (the web app's
   download does not grow).
3. **A runtime catalog**: `EnvDef` owned rather than `'static`, from the
   bundled entries and the downloaded ones; an online sky's display level
   by rule (night skies -- Poly Haven's `night` category -- at 0.2, others
   at 0.6, as the bundled ones are) and its median measured on loading.
4. **The picker**: a window of thumbnails by category (Pure skies,
   Skies, Night, Studio, ...) and search; each with its name, author and
   download size; Download, then it is in the Sky list. "Powered by Poly
   Haven" on it, and the sky's credit link under the choice as now.
5. **Loading off the frame**: decoding a sky and making its reflection
   levels takes about 0.2 s on the desktop (a 1K sky), longer on the web;
   a worker or a frame-spread job, the old sky kept until the new one is
   ready.

## The 4K question, online

At 1K the light and the reflections are right, and the backdrop is soft
(about 170 of the map's pixels across a 60-degree view). Online, a sky
could carry a separate **backdrop** at a higher resolution, fetched only
when Show as backdrop is on:
- **2K or 4K `.hdr` from Poly Haven** (5.7 or 21 MB): no hosting; on the
  GPU 4K is 64 MB in half floats, too much for some browsers and phones;
  the light map stays 1K either way (made from it), so lighting costs
  nothing more.
- **A 4K 8-bit JPEG we make** (2-3 MB, 32 MB on the GPU, sRGB like the
  background colour): smaller and sharper per byte, but only option B can
  serve it.

## Textures

Downloading them is the same machinery; using them is a rendering project
of its own, to plan once we know what they are for. Our surfaces have no
UV coordinates: a solid would take a texture by triplanar projection (in
the fractal's frame or the world's), a terrain on its ground plane. Each
map has its place: the diffuse as the albedo (in place of the palette, or
blended with it), the normal map as bump (as the surface texture already
bumps terrains), roughness into the coat, AO into the occlusion,
displacement perhaps never (it would move a distance field). Which
surfaces, and how a texture and the escape colouring share the colour,
are questions for the user first.

## Decisions for the user

1. **Where assets come from**: A (Poly Haven live, recommended), B (our
   site hosts a curated mirror), or C (our catalog, their files). And
   whether our site's role later is curation, hosting our own and users'
   skies, or the 4K backdrops.
2. **How much of Poly Haven's library**: everything (including studio
   and landscape HDRIs, which reflect well but have a ground below), or
   skies only (pure skies, skies, night).
3. **The backdrop's resolution**: 1K only; a choice of 2K/4K `.hdr`
   downloads; or 4K JPEGs (needs B).
4. **The web cache**: the Cache API (persistent), or downloads kept for
   the session only.
5. **Textures**: on solids, terrains, or both; replacing the palette's
   colour or blended with it; which maps.
6. **Contacting Poly Haven** before release, as their API page asks.
