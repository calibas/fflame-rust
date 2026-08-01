# API v1 → v2 (archived)

The client's first API integration and the v2 wire-format redesign. Both
are **done and shipped**; the live successor is
[docs/projects/api-shared-resources.md](../../projects/api-shared-resources.md),
which is still active and deliberately not archived.

Read these for *why* the API looks the way it does. Do not read them for
what the API currently is — for that, `docs/main/openapi.json` is the
spec, and `docs/generated/engine-contract.json` is the generated
client-side contract.

## What is here

**`api-integration.md`** — the original WASM-first integration: auth,
saving and loading flames and palettes, desktop `ureq` vs browser Fetch.

> **Outdated in a specific way.** It describes a `--features api` gate.
> That gate no longer exists; the API client is unconditional, and the
> only feature in `Cargo.toml` is `web-app`. If you are looking for a
> feature flag because this doc mentions one, stop — there isn't one.

**`api-integration-pr.md`** — the PR description for that work, kept as
a summary of what landed in one place.

**`api-v2.md`** and **`api-v2-server.md`** — the paired client and
server plans for the v2 wire format: the root-transform split, the
inline palette, and the version-keyed config migration. They cross-link
each other and were moved together.

> **`api-v2-server.md` still says "Status: design, not yet
> implemented".** That was true when written and is not now — the server
> shipped, and it lives in a separate repository. A server-side schema
> plan in the *client* repo is exactly the kind of document that goes
> stale without anyone noticing, which is part of why it is here.

## Where the live information went

| topic | now lives in |
|---|---|
| the wire format, authoritatively | `docs/main/openapi.json` |
| variation/effect payloads | `docs/projects/VARIATIONS_WIRE_FORMAT.md` |
| vocabularies and engine limits | `docs/generated/engine-contract.json` (generated) |
| shared resources, curation, plugins | `docs/projects/api-shared-resources.md` |
| version-keyed config migration | `docs/archive/config-versioning.md` for the mechanism; `src/api/sync.rs` for its API-specific use |

Nothing unique was lost in archiving these — that was checked before
moving them, not assumed.

## A note on one dangling reference

These docs mention `api-v2-server-side.md`, which `api-v2.md` itself
instructed be deleted. It was. The reference is left as written rather
than repaired, because an archived document should record what it said,
not what we would say now.
