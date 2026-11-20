# One list: the Catalog and the pickers

**Status: built 2026-11-19 on branch `curated-content` (steps 1-4), not
merged. What was built is under each step; the reference is
[CATALOG.md](../main/CATALOG.md) (*The model*, *The picker*).**

## The problem

Every picker reads the Catalog's shared list (`inventory::shared`), but
each one filters it down to what is "usable now": built-ins and files
flagged as **plug-ins** (`escape_panel.rs`, `transforms.rs`,
`effects_panel.rs`, `sim_panel.rs`, ...). The Catalog lists every file.

So the Catalog shows things no picker offers:

- the shipped Rhai examples (Phoenix, Halley, Burning Ship, the orbit
  formulas, ...), which cannot be made plug-ins at all (`plugin_of`
  requires `shipped: false`);
- the user's own and adopted files that are not flagged;
- UF and Fractint files.

The plug-in flag is the Rhai plug-ins plan's model (D3): "register this
file at startup so the pickers offer it". The Catalog came later.

## Decisions (the user, 2026-11-19)

| # | Question | Decision |
|---|---|---|
| L1 | The plug-in flag | **Retire it**, as long as the Catalog still says where each thing comes from. Custom and Plug-in may become one origin. |
| L2 | Files a picker cannot use (a UF or Fractint file of many entries, a file that does not parse) | **Hidden from the pickers.** Many resources in one file, as palette packs already are, is later work. |
| L3 | Downloads in pickers | **Pickers offer what can be downloaded**: the variation and effect catalogs, as now, and curated online items. **A user setting turns it off.** |
| L4 | Duplicates | **Allowed when the user makes them.** A Rhai twin counts as its built-in wherever it was chosen for comparable or better performance (the Rhai plug-ins plan's D7), so a built-in is listed once. |

## What exists that already fits

- **L1, origins.** Where a thing comes from is its `provenance::Origin`
  (Builtin, Local, Api) and its file's link (`storage::links`, with
  `from_others` and the cloud id). The plug-in flag is not part of that,
  so retiring it loses nothing about origin.
- **L4, twins.** `inventory::sources::files` already skips a built-in's
  twin file (the effects' `BUILTIN_TWINS`, the simulation twins), and
  each built-in entry takes its twin's prose. The escape built-ins that
  D7 retired resolve to their twins. Variations, solids, fields and the
  remaining simulation built-ins keep their WGSL (D7 open for them), and
  their twins are not files, so they are not listed twice either.
- **Compiling a file on demand** is what the Catalog's previews do
  (`library::prepare_plugin`: register in the kind's registry, give the
  source as a config carries it), without listing the file as a plug-in.

## The plan

1. **Pickers admit every usable entry of their kinds.** That means the
   built-ins, the shipped examples, the user's files and the adopted
   ones. A file is usable when it parses as a GPU kind
   (`library::rhai_kind`, `Kind::can_be_plugin`); UF and Fractint files
   and unreadable ones are hidden (L2).
   - Picking a file compiles it (`prepare_plugin`) and uses it as
     choosing a plug-in did: its source is carried in the config.
   - A compile failure says why, in a notice, and is not retried every
     frame.
   - *Built:* `ui::inventory::compiled` -> `library::compile_file`,
     cached by (shipped, file, version), failures too, so a list asks once
     a version. A file that does not compile is not offered by a picker;
     the Catalog's Use says why (`catalog.does_not_compile`). A file with
     no `INFO.id` compiles under one derived from its name
     (`scripted::derived_id`) and is not rewritten (the flag wrote one
     in).
   - One predicate for every picker (`inventory::pickable`), so the
     lists cannot drift apart again.
   - The flame's own Rhai variations (`picker::own_variations`) stay
     offered, but never twice: not when the same key is a file's.
2. **Retire the plug-in flag (L1).**
   - The startup registration of flagged files (`library::load_plugins`),
     the management action, the origin filter's *Plug-ins* (its files
     are *Custom*), the "(plug-in)" marks and the note in an entry's
     origin line all go.
   - `Link::plugin` is still read from old link files, so nothing
     breaks, and never written.
   - *Built:* the origin filter is All, Built-in, Custom, From others;
     the list of what a session registered (`library::plugins`) is now
     the files compiled this session (`library::is_file_key`: the variation
     picker's own-variation filter); the track editor's Add Script Track
     lists every track file of the user's from the shared list.
   - Docs: CATALOG.md, ESCAPE.md and VARIATIONS.md where they describe
     plug-ins, and the Rhai plug-ins plan's D3 marked as superseded.
3. **Downloads in pickers (L3).**
   - A system setting, *Offer downloads in pickers*, on by default, with
     a toggle in the Catalog menu.
   - Off, the pickers drop the variation and effect catalogs' *Available*
     rows. The Catalog itself still lists them.
   - **Curated online items.** Each picker's script kinds (formula,
     colouring, field, solid, their colourings, variation, effect, the
     simulation kinds) get the curated list
     (`GET /scripts?kind=K&curated=true`), fetched in the background once
     per session.
   - These are offered as *Available online* rows, not installed.
     **Download** adopts the script (`ScriptCloudRequest::Adopt`), and
     once it is a file it can be picked.
   - A curated item already held as a file is not offered again.
   - *Built:* `SystemSettings::picker_downloads` (on unless `false`), the
     Catalog menu's checkbox; `sources::curated_scripts` over the panel's
     one fetch a session (`Scope::Curated`, the kinds that can be
     compiled, 100 a kind), grouped with *Available to download*; the
     server's copies of the app's own content (`origin` "builtin") and a
     kind this build does not know are left out. Download queues an
     Adopt, handed to the script cloud one at a time. The Catalog's own
     list leaves these rows to its Online place. Not offered: curated
     effect scripts in the Effects panel's Add list, which sorts by a
     stage the list item does not carry; they are in the Catalog's Online
     place.
4. **L4.** Check that no shipped example is a byte-for-byte copy of a
   built-in's twin; hide any that is.
   - *Checked:* none is. The Mandelbrot, Burning Ship and Phoenix
     examples share a name with built-ins but are different programs, so
     they stay.

## Later

- Many resources in one file, as palette packs: UF and Fractint files in
  the pickers, an entry each (L2). On the backlog.
