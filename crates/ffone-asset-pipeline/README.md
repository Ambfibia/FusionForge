# ffone-asset-pipeline

> The global runtime manifest workflow is retired. Older commands documented below may consume
> historical manifest fixtures for reproducibility, but must not target the current `assets/game`
> tree. New Nano, mob, NPC, cosmetic, prop and world publication follows
> [`docs/native-asset-workflow.md`](../../docs/native-asset-workflow.md) and finishes with
> `cargo xtask assets` / `cargo xtask assets --full`.

`ffone-asset-pipeline` is the native project-asset boundary for FFOneClient. Its bulk importer reads
a validated `ffone.content-pack.v1` directory, while its strict logical-model publisher reads an
offline `ffone.logical-model-source.v1` JSON export. It never opens source client builds, editor
projects, Unity containers, Gamebryo files, or intermediate OBJ files at runtime.

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- import `
  --pack D:\path\to\native-pack `
  --output assets\game
```

A complete logical source can be staged under a true-name family path with:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- publish-logical-model `
  work\projects\<project>.ffclient\staging\logical-model-sources-retrobution\npc\npc_dexter.source.json `
  npc `
  work\projects\<project>.ffclient\staging\logical-models-candidate-retrobution
```

Publish a complete ownership-routed source tree atomically with:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- publish-logical-model-batch `
  work\projects\<project>.ffclient\staging\logical-model-sources-retrobution `
  work\projects\<project>.ffclient\staging\logical-models-candidate-retrobution
```

The batch root is strict: every file is
`<family>/<exact-route-stem>[/...]/<minimally-safe-true-m_Name>.source.json`, and there are no
manifests or unrelated files inside it. The first directory is the family and all remaining parent
directories are mirrored under `models/<family>/`; neither value is guessed from a basename. Before
creating a staging directory the publisher reads every source, verifies that `logicalName` is the
exact sole root `m_Name`, and reserves every GLB, adjacent report, base PNG and source-mip PNG path.
Case-insensitive and Unicode-normalized aliases, file/directory aliases, generated hashes, PathIDs,
and minimally-sanitized true-name collisions fail with no destination tree. A successful run writes
`logical-model-batch-report.json`, passes the structural tree audit, and publishes the fresh
destination with one sibling-directory rename. It never overwrites an existing destination; the
separate GPU and visual-parity gates remain pending.

The publisher requires one exact root/name, preserves the Transform hierarchy in one GLB, binds
each mesh to its source Transform, keeps `MeshFilter` geometry rigid, and requires complete
`SkinnedMeshRenderer` palettes, weights and inverse-bind matrices. Joint and animation paths must
resolve uniquely by exact path or suffix under the true root. Curve counts, CUBICSPLINE tangents,
LINEAR compressed rotations, clip settings and events are checked before writing. Euler, float,
PPtr, unresolved event-object bindings, warnings, missing material names and any source/published
count mismatch fail without creating output.

The exact source also carries `ffone.native-coordinate-contract.v1`. Positions, normals,
translations, rotations, UVs and inverse-bind matrices are already converted from Unity into the
declared right-handed native space. The H=diag(-1,1,1) handedness reflection establishes the native basis, but exact Unity mesh dumps
can contain mixed serialized winding, including within one primitive. Publication therefore checks
every normal-bearing triangle against its authored vertex normals and swaps only an opposed
triangle; degenerate and normal-less cards preserve their reflected source order. The native
validator independently repeats that per-triangle audit and fails closed before installation if a
future exporter reintroduces opposed geometry.
A missing or contradictory coordinate
contract fails closed.

The publisher writes real glTF materials, images, textures, samplers and primitive material
indices. Ordered legacy properties, explicit null slots, effective queues and typed pass state are
retained in `extras.ffone`. Every source Texture2D mip is preserved exactly: the base level remains
a directly viewable true-name PNG beside its owning logical model, while smaller source levels live
under `<texture>.mips/mip-NN.png` with exact raw offsets, byte lengths and SHA-256 hashes in the
native contract. The publisher never resizes or generates replacement mips. Unsupported Crunch
payloads fail closed until a lossless Crunch decoder is available. Every currently audited
ShaderLab program is resolved strictly from its complete source script. The Simon additive,
two-sided program is additionally bound to its exact source SHA-256. Unknown or contradictory
programs fail.

The staged `npc_dexter`, `fusion_dexter` and `npc_max` logical models pass the separate native Bevy
GPU evidence gate with exact true names, GLB and screenshot hashes, materials, source mips,
skinning, resolved joints, inverse-bind matrices, one exact sampled animation and
ShaderLab-compatible outline rendering. The per-model publish reports remain deliberately
`status: staged-incomplete`, `publishable: false`: this three-model candidate is not the complete
ownership-proven catalog and must not replace `assets/game` yet. Automated GPU acceptance is also
not a claim of manual visual 1:1 parity.

Audit a complete candidate tree after all intended models have been added:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- audit-logical-model-tree `
  work\projects\<project>.ffclient\staging\logical-models-candidate-retrobution `
  work\projects\<project>.ffclient\reports\logical-models-candidate-retrobution.audit.json
```

The publisher independently re-decodes its emitted GLB JSON/BIN and requires exact canonical
SHA-256 matches against `NativeModel` values, not only equal feature counts. Five ordered sections
cover hierarchy names/TRS/bindings; geometry positions/normals/UV/indices/JOINTS/WEIGHTS; joint
palettes/inverse binds; standard animation times/values/tangents; and all non-TRS metadata,
metadata-only clips, float/object curves and events. The audit reopens the GLBs from disk,
recomputes those value digests, and also checks true root/file names, reachability, materials, every
exact source mip PNG, raw-chain continuity and publish-report hashes. Missing, corrupt, generated
or orphan files fail the audit. It cannot clear the separate GPU gate.

Audit immutable native GPU evidence independently against the current GLBs:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- audit-logical-model-gpu-evidence `
  work\projects\<project>.ffclient\staging\logical-models-candidate-retrobution `
  work\projects\<project>.ffclient\reports\logical-model-gpu-evidence-retrobution `
  work\projects\<project>.ffclient\reports\logical-model-gpu-evidence-retrobution.audit.json
```

The GPU audit requires exactly one JSON+PNG pair per GLB and rechecks true names, hashes, the exact
sampled clip, mesh/skinning/joint/IBM/material/pass/mip counts, PNG dimensions and non-empty
foreground. It can set `automatedGpuPassed: true`; `visualParityPending` remains true until the
controlled images are compared with the named reference client.

Install a structurally accepted, table-classified character tree into the
permanent Bevy asset root without changing its GLB/PNG bytes:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- install-semantic-characters `
  work\projects\<project>.ffclient\staging\logical-models-candidate-retrobution `
  work\projects\<project>.ffclient\reports\logical-model-gpu-evidence-retrobution.audit.json `
  <TABLE_SET_JSON> `
  assets\game `
  retrobution-20260613 `
  target\semantic-character-install-retrobution.report.json
```

The installer rechecks structural and GPU evidence, classifies exact models
from table ownership, and publishes physical
`characters/{nanos,npcs,mobs,fusions,shared}` payloads. It regenerates the
`ffone.semantic-character-registry.v2` authority at
`_runtime/characters.json`. Old singular folders,
`characters/catalog.json`, and adjacent audit reports are removed from the
runtime contract; the install report remains in ignored Editor work. Exact
authoring-role overrides keep `npc_deedee` and `npc_dexter` under `npcs` even
though legacy encounter tables also reuse those meshes in hostile/HNPC roles;
all observed legacy roles remain recorded in the install report.

The transaction commits every declared runtime category, including `fusions`.
On a repeat install it distinguishes installer-owned files by the exact
`native-semantic-characters/` manifest provenance and preserves complete
externally published packages, such as tutorial promotions or future editor
content, together with their registry rows. Mixed ownership inside one package,
a missing registry row, or any disk/manifest/hash drift blocks the transaction.

After the semantic character registry is installed, remove only the twelve
audited tutorial package copies that are byte-for-byte identical to their
canonical `characters/{npcs,mobs}` packages:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  dedupe-tutorial-character-models assets\game
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  dedupe-tutorial-character-models assets\game --apply
cargo xtask assets --full
```

The first command is a read-only gate. It verifies both package trees, every
disk/manifest size and BLAKE3 identity, matching asset kinds, and the absence
of active data references to the redundant paths. The apply transaction moves
only the proven duplicate directories aside, commits the reduced project
manifest, then deletes the staged copies. Unique tutorial models remain under
`tutorial/models`; a missing model is never synthesized. The command is
idempotent. For the current native tree, finish any supported publication with
the read-only release graph validator.

Promote the two proven static tutorial props, `etc_domeglass_04` and
`npc_building`, from their legacy tutorial package roots to canonical
`assets/game/props/tutorial/structures/<id>` packages. Preview the complete
proof and transaction first, then apply the same inputs:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- promote-tutorial-props `
  . `
  work\projects\<project>.ffclient\staging\logical-models-candidate-retrobution-tutorial-complete-v1 `
  work\projects\<project>.ffclient\staging\logical-model-sources-retrobution-tutorial-complete-v1 `
  work\projects\<project>.ffclient\reports\logical-model-gpu-evidence-retrobution-tutorial-complete-v1

cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- promote-tutorial-props `
  . `
  work\projects\<project>.ffclient\staging\logical-models-candidate-retrobution-tutorial-complete-v1 `
  work\projects\<project>.ffclient\staging\logical-model-sources-retrobution-tutorial-complete-v1 `
  work\projects\<project>.ffclient\reports\logical-model-gpu-evidence-retrobution-tutorial-complete-v1 `
  --apply
cargo xtask assets --full
```

The allowlist contains exactly those two rigid, non-animated models. The
transaction verifies their logical source, publish report, GLB, GPU evidence,
complete package identity, manifest entries and runtime references before it
moves the packages and commits updated references and the project manifest.
These models are props, not characters, so the command neither publishes below
`characters/` nor changes `_runtime/characters.json`. Byte-identical texture
files outside the two package roots are external duplicates and are not
removed. For current native publication, run `cargo xtask assets --full` after a successful applied
change.

Cook the exact Retrobution male Test Ser source checkpoint without publishing a false assembled
model:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- cook-player-avatar `
  --bundle ..\builds\retrobution-20260613\CharacterSelection.resourceFile `
  --objects target\player-cooker-character-selection.objects.json `
  --logical-plan ..\FusionForge\target\retrobution.logical-model-plan.duplicate-aware.json `
  --output assets\game `
  --part-source wear/m_face_001_type01.nif=target\player-cooker-probe-face.source.json `
  --part-source wear/m_head_023_type01.nif=target\player-cooker-probe-head.source.json `
  --part-source wear/m_shirt_baseballset.nif=target\player-cooker-probe-shirt.source.json `
  --part-source wear/m_pants_beltarmorset.nif=target\player-cooker-probe-pants.source.json `
  --part-source wear/m_shoes_blooarmorset.nif=target\player-cooker-probe-shoes.source.json `
  --part-source wear/theown_discobomb.nif=target\player-cooker-probe-discobomb.source.json
```

It produces the semantic, directly readable
`characters/player/male/base/male_skeleton.json` inventory. The conversion
report proves exact bundle ownership and shared-skeleton bone remaps, but it is
offline evidence: raw historical recovery belongs in the Editor project recovery tree and
new review/audit reports belong in its reports tree, never
`assets/game/data`. The checkpoint remains incomplete until animation
projection, height/shape sampling, combined skin construction and assembled-GLB
round-trip validation are implemented.

Preview the conversion-only JSON archive without changing the project:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- clean-runtime-metadata `
  . `
  retrobution-20260613
```

The command is dry-run by default and prints a deterministic JSON report. It recognizes logical
model publish evidence and superseded character reports/catalogs, icon import reports, proven
`data/catalog` conversion reports, the tutorial-model import catalog, both former UI installer
catalogs, the legacy localization catalog/dump and the source font index. Active runtime JSON is
explicitly retained, including character-creation `appearance`, `avatar_items` and
`runtime_textures`, content indexes, runtime registries, the domain-owned
`characters/player/shared/player_rig_contract.json`, authored UI layouts, and the tutorial
effect/projectile catalogs. A `catalog.json` name alone is therefore not evidence that a file is
disposable. The sole exception is the exact native-only, incomplete legacy
`data/catalog/content-index--818266880751860c.json`, whose path, source path,
schema, profile, size and BLAKE3 are pinned before it can move to offline
evidence; sibling/current content indexes stay active. Audio
publication is authoritative only through the domain-owned
`ffone.semantic-audio-catalog.v5` at `_runtime/audio.json`; it contains path-only
logical audio identities with no runtime byte lengths or hashes, collision-checked
legacy-key aliases, and automatic mirrored voice-locale discovery. Nano voice is physically grouped
below `voice/{locale}/nano_<id>`, Computress below
`voice/{locale}/computress`, and Nano ability SFX below `sfx/nano_skills`.
The current Computress package contains 180 English and 42 Russian clips; the
remaining 138 lines use the English fallback. Two English dialogue clips
recovered from the shared-SFX bucket are accepted only by pinned hashes, not as
inferred variants.
Ambiguous/ignored Russian inputs and their report remain under
`work/projects/<project>.ffclient/reports/voice-localization-v3`. World conversion metadata is handled
by the separate runtime-world migration below.

After reviewing a clean dry-run, add `--apply` to copy raw selected recovery
files by value under
`work/projects/<project>.ffclient/recovery/<SOURCE_BUILD>/conversion-metadata/files/` and remove the
runtime copies in one rollback-capable transaction. Metadata stored with that
archive records source provenance; new operational audits, review reports and
quarantine queues go to the Editor project reports tree.

An existing, valid primary archive does not block later cleanup. It is fully
revalidated first; newly discovered candidates are stored once in an immutable
`conversion-metadata/revisions/<plan-blake3>/` directory with a revision index
and report. Previously archived payloads are verified and removed from runtime
without being recopied, and a run with no new candidates creates no revision.
A stale transaction directory or collision with an immutable revision still
fails closed. Apply holds an exclusive sentinel and durable journal, copies and
fsyncs a hidden archive stage, revalidates manifest/source identities, and
publishes the immutable target only after the manifest swap. Ordinary errors
roll back; an incomplete rollback preserves stage/backup/journal. Hard-crash
artifacts require operator recovery rather than an automatic roll-forward.

The same command also audits scene-less world payloads against the manifested
`_runtime/world.json` closure. This orphan-world residual rule is deliberately limited to the three
reviewed Retrobution residuals `world/maps/map_00_08`, `map_01_09`, and
`map_11_15`: together 210 files, including 6 JSON files, and 4,989,201 bytes.
Their exact file/JSON/byte totals and terrain/environment BLAKE3 anchors are
pinned in source. Any additional unregistered root, changed anchor, manifest
mismatch, runtime-registry route, or external data-JSON reference blocks
`--apply`. The payload bytes are archived as an immutable conversion-metadata
revision before the rollback-capable manifest transaction removes them.
This Retrobution-only rule activates from the immutable source-pack schema and
manifest BLAKE3, not from the mutable project asset-manifest hash, so adding new
editor content cannot silently disable the archive check. Empty residual
directory trees count as absent and still require all 210 archived identities;
an unregistered non-empty residual blocks cleanup.

Independent of those three orphan roots, one exact Retrobution revision archives
20,056 superseded native-world conversion payloads (343,585,139 bytes): 18,356
non-detail `.source.bin` files, 1,530 raw-component files comprising 170 copies
of each of nine pinned basenames, and 170 `gameplay/attributes.raw.json`
documents. All 170 terrain roots must have complete closure, active detail
textures are excluded, and any disk/manifest drift or surviving JSON reference
blocks apply. That revision also archives the one exact legacy content index
described above (6,293,459 bytes), for a combined 20,057 files and 349,878,598
bytes. Its deterministic plan BLAKE3 is
`57b9c590ea86c9d30c1f181d588fd49d4baa3f69bd6469d35c1609d0a61d9144`.

`localization/catalog.json` is the active path-only locale registry. Runtime code opens every text
bundle declared there through `AssetLocator`; `_runtime/localization.json` remains retired. The
authored/runtime byte-identity test is the publication gate for the maintained catalog and EN/RU
bundles.

Preview the world conversion-metadata migration without writing:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  migrate-runtime-world . retrobution-20260613
```

After reviewing the deterministic report, repeat with `--apply`. The transaction
publishes the 170-entry `ffone.runtime-world.v1` registry at `_runtime/world.json`,
sanitizes runtime scene/terrain/environment JSON, updates their manifest identities,
and archives legacy `world/catalog.json`, provenance/import manifests, parsed documents
and conversion-only raw JSON below
`work/projects/<project>.ffclient/recovery/<SOURCE_BUILD>/world-conversion-metadata`. Registry and referenced
documents are each bound to one case-exact manifest `Data` entry and matching disk
bytes/BLAKE3.

The `ffone-semantic-assets` commands `plan-world`, `publish-world`, and
`publish-native-terrain-batch` are legacy pre-migration publishers. When a target has
the world-v1 registry they fail before mutation and direct the operator to the
registry-aware world/editor workflow. Migration completion is an integrity statement;
full rendering, collision and gameplay parity remain incomplete.

`--output` defaults to ignored Editor staging at `work/ffone/imported-native-assets`; publishing
to FFOne requires an explicit target path. The destination must not already exist. Import is staged
in a sibling directory and published by one rename, so errors leave no partially updated output. Every
payload is read through `ffone-content` hash/size verification and written as a new regular file;
hardlinks and source-dependent file references are never used.

## Project layout

```text
assets/game/
  _runtime/
    characters.json         semantic character registry
    audio.json              semantic audio catalog v4
    world.json              native world registry v1
  characters/
    nanos/
    npcs/
    mobs/
    fusions/
    shared/
    player/
  audio/                    OGG; localized voice plus nano_skills SFX taxonomy
  localization/             en.json and ru.json keyed text
  icons/                    semantic entity/item/skill taxonomy
  world/                    sanitized native scene/terrain/environment payloads
  models/world/             generated scene-bound GLB/PNG dependencies
  props/                    hand-owned nature and tutorial structure packages
  textures/                 PNG
  fonts/                    TTF and OTF
  shaders/                  WGSL
  data/                     native gameplay tables/catalogs
```

The complete hand-authored payload layout is documented in
[Asset layout](../../docs/asset-layout.md). The former flat
`assets/game/models` diagnostic dump has been removed; raw per-Mesh recovery
belongs below the Editor project recovery tree and is never a
publishable logical-model registry. The narrow `models/world/<scope>/<tile>`
tree is not that dump: it is generated from and owned by world scenes, grouped
as `world-generated`, and is not a manual authoring destination.

Character and world catalogs record dynamic paths and BLAKE3 values. The editable v5 audio catalog
and localization catalog record paths only; their loose payloads are excluded from integrity
hashing. Static UI and table routes remain stable code contracts. `cargo xtask assets --full`
builds the complete immutable dependency graph in memory and publishes no inventory JSON.

## Logical-model v2 gate

Final models are grouped by normalized AssetBundle container route and its target/preload graph. A
single GLB owns the reachable hierarchy, mesh parts, materials, skin, bones, clips, colliders and
LOD levels. Orphan meshes are excluded. Its sole scene root and metadata keep the exact legacy
`m_Name`; only Windows-invalid/control characters, a trailing dot/space and DOS device names are
minimally protected in the filename. Transliteration, slugs, hashes and PathID suffixes are banned.

`ffone.logical-model.v1` is the publication gate. Source and published feature counts must match,
the GLB BLAKE3, exact root name and value-level semantic round trip are verified, and unresolved
source features make the contract fail. Represented curve tangents/modes, object-reference curves
and animation events are included in the semantic proof; a source binding that the schema cannot
represent remains a fail-closed unsupported feature.

Stage sources, candidates and audit reports in the Editor project staging tree; publish
only logical models with passing contracts into the semantic runtime tree. Do
not create an `assets/game-v2` or restore a flat `assets/game/models`
namespace. Raw diagnostic recovery remains under the Editor project recovery tree.

The legacy recovery audit command remains an offline diagnostic:

```powershell
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- audit-models `
  --assets work/projects/retrobution-20260613.ffclient/recovery `
  --cook-report ../builds/FFOneContent-retrobution-20260613-ru.cook-report.json `
  --layout-report ../builds/retrobution-20260613.ffclient/bundle-layout-report.json `
  --table-set ../builds/FFOneContent-retrobution-20260613-ru/tables/table-set--4c58cfc64992b756.json `
  --output target/model-audit.json
```

The report deliberately leaves the exact global logical-model count unknown: v1 cook/layout reports
retain aggregate route counts but not enough normalized route-extension and graph identity data to
reconstruct every world/effect root. The v2 source-graph extraction must inventory those roots.
