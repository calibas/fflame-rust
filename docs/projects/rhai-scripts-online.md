# Rhai scripts online: one scripts resource for every kind

**Status 2026-11-12:** **built** on ffapi branch `rhai-scripts` (stacked
on `flame-catalog-fields`; migration 108) and client branch
`rhai-scripts-online`, not merged. All six decisions in §5 taken as recommended by the user the same day (the first publisher owns an id; the Formula Editor and one online library; configs keep carrying sources; the app's spellings; the catalog link; UF/Fractint left out). Where the build settled
details the plan left open, §6 says. This is the API plan's
step 4 ([api-updates.md](api-updates.md)), taken up by the user on
2026-11-12 after first deferring it. The original intent is P6 of
[rhai-plugins.md](../archive/projects/rhai-plugins.md): "a variation,
effect, formula, colouring, solid, simulation rule, track or generator is
published, found and adopted the same way."

---

## 1. Where things stand

**Online today: three kinds.** The server's `scripts` resource (create,
update with an optimistic `version`, unpublish, search, fork, the museum's
publish) stores `generator`, `modifier` and `library`, the CPU scripts.
The app's Scripts panel publishes and adopts them (`app::script_cloud`:
`ListMine`, `Search`, `Publish`, `Update`, `Adopt`, `Refetch`,
`Unpublish`).

**Not online: the GPU kinds.** These are formulas, colourings, solids,
fields, field colourings, simulation models and colourings, effects,
variations and animation tracks. They live in the app's formula library
(`formulas/`; `storage::library::Kind` lists every kind), and a config
carries the source of each one it uses.

**Already in place:**
- **The formula library's link record** (`storage::links::Link`) already
  has `cloud_id`, `version`, `owner`, `from_others` and `plugin`, unused
  for the GPU kinds.
- **Plug-ins exist** (P3: a library entry registered at every start).
- **Every user-written GPU script has a UUID of its own**, its `INFO.id`
  (design rule 7, minted when it is saved). Its version is the SHA-256 of
  its source, and the registry names it `u<uuid>_<8 hex>`.
- **The catalog already keys a script type by that UUID** (migration
  106), and `fractal_types.script_id` is waiting for the stored script it
  names.

---

## 2. What is built

### The server (`ffapi`)

1. **New kinds** (migration, `ADD VALUE` outside a transaction):
   - `formula`, `coloring`, `solid`, `field`, `field_coloring`;
   - `sim_model`, `sim_coloring`;
   - `effect`, `variation`, `track`.

   These are the app's own spellings, the same as the catalog's
   (decision 1).
2. **The client's id on create** (decision 2). A create request may carry
   `id`: the script's `INFO.id`. The server stores the row under it, so a
   catalog `script:<uuid>` type and the stored script are the same id.
   - An id already taken is a 409.
   - Without an id the server mints one, as now; the CPU kinds have no
     `INFO.id`.
3. **Every new kind is shown only to reader 7 or more** (`Reader::script_filter`
   gains them). Requests without `reader` (0.5, older integrations) see
   generators and modifiers as today. The museum is unchanged: Halls are
   generators, Branches are modifiers.
4. **The catalog link** (decision 4):
   - creating or updating a script sets `fractal_types.script_id` where a
     `script` type's key is its id;
   - summarising a flame sets it when the script exists;
   - flame components and `/api/types` gain `script_id`, so "this
     config's formula" links to its page;
   - a script's response gains `used_by`, how many public configs use it.
5. **Search by kind:** `GET /api/search/scripts?kind=formula` (and the
   caller's own list, `GET /api/scripts?kind=`), for any kind.
6. **Validation:** the kind; the source size (the contract's 256 KB); for
   a GPU kind sent with an `id`, that the id appears in the source's
   `INFO` (a plain text check, since the server parses no Rhai). The
   server compiles nothing, as now.

### The app

1. **Publish, update, unpublish for a GPU entry.**
   - The Formula Editor gains them for the entry it has open, if that
     entry is saved in the formula library (decision 3).
   - The request carries the kind (from `lang::rhai::kind_of`), the
     `INFO.id`, the title and description from `INFO` and the header,
     and the source.
   - The link record keeps the cloud id and version, as the Scripts panel
     does for CPU scripts.
2. **What is refused before sending:**
   - a source that does not compile;
   - an unedited twin: it is the built-in, and the catalog already
     groups it as such;
   - a UF or Fractint file: those hold many entries and no `INFO.id`
     (decision 5).
3. **Browse and adopt** (decision 3). The online library lists every kind
   with a kind filter. Adopting a GPU entry saves it into the formula
   library under its own file name, marked `from_others`, and optionally
   as a plug-in. A CPU script still goes to the script store, as now.
   - Adopting does not mint a new id: the copy is that script. Editing it
     and saving gives it an id of its own, as editing a twin does.
4. **Configs keep carrying their sources** (decision 6). A config stays
   self-contained (offline, files, PNG metadata), and stays readable by
   apps without this feature.
5. **`ScriptListItem.kind`** is already a string, so older 0.6 builds
   degrade on unknown kinds. Its consumers learn the new ones.

### The browser app (`ffbrowser`)

ffapi's `docs/BROWSER_APP_0.6.md` §3 is rewritten for what this makes
possible:
- browse each kind;
- a script's page shows its source and the configs that use it;
- a config's components link to their scripts.

---

## 3. Order

1. Server: the kinds, the client's id, the reader filter, search by kind,
   validation, tests.
2. App: publish, update and unpublish from the Formula Editor; the online
   library's kind filter and adoption into the formula library. Tests,
   and an integration case: publish a Rhai formula, adopt it as another
   account.
3. Server: the catalog link (`script_id`, `used_by`), with tests.
4. Docs: ONLINE.md, the browser app's list, the API plan's status.

---

## 4. Not in this step

- **Script version history.** The server keeps the current source and an
  optimistic version counter, and snapshots for the museum's published
  scripts. A browsable history of every version is not built.
- **A config naming a published script instead of carrying its source.**
  See decision 6.
- **Running GPU kinds in the browser app.** That is the WASM modules' job
  when a config uses them, as now.

---

## 5. Decisions for the user

1. **Kind spellings on the wire:** the app's and the catalog's
   (`coloring`, `sim_model`, ...), rather than the earlier plan's British
   `colouring`. Recommended: the app's, so one vocabulary runs through the
   app, the catalog and the scripts resource.
2. **Who owns an id.** A script's id travels inside every config that
   uses it, so someone could publish another author's script under its id
   before the author does.
   - Recommended: the first to publish an id owns it (a 409 for anyone
     after), with the report flow for disputes.
   - The alternative: ids are not globally unique, and the catalog links
     a type to its earliest published row.
3. **Where publishing and browsing live in the app.** Recommended:
   publish / update / unpublish in the Formula Editor, beside Save, and
   one online library with a kind filter: the Scripts panel's online
   section, extended. The alternative is an online tab in the Formula
   Editor for the GPU kinds, beside the Scripts panel's for the CPU ones.
4. **The catalog link** (`script_id` on types and components, `used_by`
   on scripts). Recommended: yes. It is what lets a config's page link to
   the formula it uses, and a script's page say how many configs use it.
5. **UF and Fractint files online.** Recommended: not in this step. They
   hold many entries in one file and have no id. A single Rhai entry is
   the unit online.
6. **Configs keep carrying sources.** Recommended: yes. The alternative,
   a config naming `{id, version}` and fetching the source, makes configs
   depend on the network and unreadable by every app before it.

---

## 6. As built

- **Server:** `ScriptKind::is_gpu()` marks the entry kinds. A GPU kind
  must send `id`, the id must appear in the source, and a CPU kind sends
  none (`script_id_required`, `script_id_not_in_source`,
  `script_id_not_for_kind`); a taken id is 409 `script_id_taken`. An
  update cannot move a script between the CPU and GPU classes. Fork and
  the museum's publish refuse GPU kinds. Tests: ffapi
  `tests/rhai_scripts.rs`.
- **The app's one request layer:** `app::script_cloud::Local` names a
  request's file, `Script(stem)` or `Entry(file)`; the CPU and entry paths
  share every request and result. The payload of an entry is
  `api::entries::create_request`.
- **Unedited built-ins** are recognised by text, not by id: a twin may
  carry a UUID once Use or Edit as Rhai gave it one. `entries::shipped_of`
  compares against every twin and example with the id and line ends
  aside.
- **An id of its own** is minted at Publish when the entry has none (or a
  non-UUID one), and written into the file, as making a plug-in does.
- **An adopted entry, edited and saved,** gets a new id
  (`entries::with_new_id`) and loses its cloud link and its mark as
  somebody else's. Without that it could never be published (the editor
  refuses to republish somebody else's entry), and its id would claim to
  be the original.
- **The kind filter** is the Scripts panel's online section, for listing
  your own and for search. The Formula Editor's Online section shows the
  shared status line.
- **The contract's** `scripts.kinds` lists every kind, the CPU ones first
  (`library::ALL_PUBLISHED`). Its shape does not move.
- **Integration case:** `api::integration_tests::a_rhai_formula_is_published_under_its_id_and_adopted_by_another_account`.
- **Found by it:** the server's script update required `visibility`
  (since migration 83, 2026-08-01), and the app's Update online has
  always sent none, so every update of a published script was a 422. The
  server now takes it as optional, absent keeping the stored one. That
  repairs the apps already shipped, so the client is unchanged.
