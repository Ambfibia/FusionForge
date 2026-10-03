# World-map static scene extraction

The 161 `worldMap` tiles previously carried only their native terrain: `scene.json` held an empty
`models`, `visuals` and `colliders` set and `coverage` was `native-heightmap`. Only the nine
`tutorial` tiles had their exact static scene published. This document records how the world-map
tiles were brought to the same standard, what the result contains, and what is still missing.

## Source and scope

| Item | Value |
| --- | --- |
| Source build | `retrobution-20260613` (`primary`) |
| Source containers | `Map_XX_YY.unity3d`, one per tile, raw bytes hashed per tile |
| Tiles | 161 `worldMap` entries of the 170-entry `ffone.runtime-world.v1` registry |
| Exporter | `fusionforge fusionforge export-native-static-world` |
| Installer | `ffone-asset-pipeline install-world-map-static-world` |
| Reviewed contract | `recipes/ffone/imports/world-static-retrobution-20260613.json` |

Nothing was taken from `patched` or `alternate` for this work. Every published byte comes from the
clean parity build.

## Exporter change

`export-native-static-world` was tutorial-shaped: it hardcoded
`world/tutorial/terrain/tiles/<tile>`, `models/world/tutorial/<tile>` and
`world/tutorial/static/tiles/<tile>`, and required the native scene to declare scope `tutorial`.

It now resolves the tile layout from the manifested `ffone.runtime-world.v1` registry in the native
asset root instead of inferring it from the bundle name:

| Scope | Native scene | Runtime models | Static metadata |
| --- | --- | --- | --- |
| `tutorial` | `world/tutorial/terrain/tiles/<id>` | `models/world/tutorial/<id>` | `world/tutorial/static/tiles/<id>` |
| `worldMap` | `world/maps/<id>` | `models/world/maps/<id>` | `world/maps/static/tiles/<id>` |

A tile that the registry does not contain, or whose registry scope is unknown, is rejected before
any extraction work is committed, so a tutorial tile can never be published into the world-map tree
or the reverse. Tutorial output is byte-identical to the previous exporter.

The replacement pass also fixes two render-contract errors found after the first publication:

- After the Unity-to-native reflection, the exporter compares the final geometric face direction
  with the authored vertex normals. If most non-degenerate faces oppose those normals, it reverses
  the complete visual primitive exactly once. A normal-less visual follows the separately audited
  legacy source convention. Collider index order is retained because rendering culling does not
  apply to collision payloads. Two exporter fixtures cover both already-aligned and opposed input.
- alpha mode, cutoff and double-sided state now come from exact `Blend`, `AlphaTest` and `Cull`
  shader directives plus serialized material overrides. Arbitrary occurrences of the word
  `blend` no longer turn opaque materials transparent.

An existing populated scene may be used as the terrain base only when its tile/source catalog,
static array counts/model routes, hierarchy hash and material hash prove it is the previous exact
publication. This permits a reproducible replacement while preserving later terrain revisions and
still rejects a hand-edited or mixed scene.

## Installer change

`install-tutorial-static-world` pinned its nine tiles as a literal `TileContract` array. The same
audit, merge and atomic-transaction code now serves both scopes:

- the publication scope is derived from the tile identity (`tile_` / `map_`), and a contract that
  mixes both is rejected;
- ownership is published per scope: `world/tutorial/static/install-manifest.json` versus
  `world/maps/static/install-manifest.json`;
- the one-time runtime-migration and conversion-metadata-cleanup recovery paths stay tutorial-only.
  A world-map scene that does not match its exact export identity is a hard failure, not a
  candidate for archived-scene reconstruction;
- 161 tiles are too many to pin as a source literal, so the reviewed contract lives in
  `recipes/ffone/imports/world-static-retrobution-20260613.json` and is passed to
  `install-world-map-static-world`. It pins, per tile, the exact source-archive BLAKE3 and the
  scene-node, renderer, collider, model, vertex and index counts the installer must observe again.

Everything else is unchanged: full export-manifest hash closure, GLB texture-dependency closure,
`_runtime/world.json` scene-hash rewrite, staged publication with backup and rollback.

The current runtime asset tree deliberately has no global `asset-manifest.json`. In that layout the
world-map installer uses `world/maps/static/install-manifest.json` as its domain ownership proof,
replaces only the 161 owned model trees plus `world/maps/static`, merges the scenes without changing
terrain fields, and atomically refreshes only the scene hashes in `_runtime/world.json`. A dedicated
integration test exercises this manifestless replacement and rollback boundary.

## Reproducing

```powershell
# 1. Export every world-map tile (about 2 minutes per tile; 4 workers used here).
#    Output root must contain only tile directories.
fusionforge fusionforge export-native-static-world `
  <build>\Map_XX_YY.unity3d <build> ..\FFOneClient\assets\game <export-root>\map_XX_YY

# 2. Rebuild the reviewed contract from a complete, successful batch.
python work\native-static-world-v1-batch\build-contract.py `
  <export-root> ..\FFOneClient\assets\game\_runtime\world.json `
  recipes\ffone\imports\world-static-retrobution-20260613.json

# 3. Install.
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  install-world-map-static-world <export-root> ..\FFOneClient\assets\game `
  recipes\ffone\imports\world-static-retrobution-20260613.json

# 4. One-time audited repair for assets produced by the older exporter.
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  repair-static-world-winding ..\FFOneClient `
  work\legacy-sources\world-static-winding-repair\plan.json
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  repair-static-world-winding ..\FFOneClient `
  work\legacy-sources\world-static-winding-repair\apply.json --apply
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  archive-repaired-tutorial-winding ..\FFOneClient
cargo run -p ffone-asset-pipeline --bin ffone-asset-pipeline -- `
  verify-static-world-winding ..\FFOneClient `
  work\legacy-sources\world-static-winding-repair\apply.json
```

## Result

```text
installed exact world-map static world: 161 tiles, 204846 GLBs, 92814 textures,
483 metadata files and 161 merged scenes as 298306 manifest publications (4720274820 bytes);
nodes=403662, runtimeVisuals=145840, runtimeColliders=59002,
vertices=27861145, indices=66897435,
sourceSetBlake3=b08c9d8b5997c7d3f75591514fa78b2e0ba39a3b6cdc4f86164131d15efb41a3
```

The one-time repair independently scanned and republished all 146,933 visual GLBs across both
scopes (145,844 `worldMap`, 1,089 `tutorial`): 145,967 files required reversal and 966 were already
aligned. It inspected 17,631,613 triangles, rewrote every dependent model hash in all 170 scenes,
updated their static-field hashes and refreshed `_runtime/world.json`. Verification reopens the
actual GLBs and recomputes their geometry evidence, file hashes, scene closure, ownership proofs
and registry hash. The accepted result-set BLAKE3 is
`28b06bc70d5f738efd5f0defd9417e7f22ed6b1aedfab4d6169232cb36d7984e`; the runtime-world BLAKE3 is
`0eb25fad4b7c96e8f16403c7dc58805534d137c1f1de81603cba2f105159c915`. The repaired tutorial
ownership proof is offline conversion evidence in immutable revision
`0feb31893326b5c8e9992cbfc87ac893094fad42881b73dd1e0014214d7e7d5e`, not a runtime asset.

At the time of the historical extraction, the retired manifest tool reported 14 groups, 363,084
files and 7,438,661,916 bytes. The current replacement is `cargo run --manifest-path ..\FFOneClient\Cargo.toml -p xtask -- assets --source ..\FFOneClient\assets\game --full`, which builds
the graph without publishing inventory JSON. Re-running the installer on the same export root reproduces the identical
`sourceSetBlake3` and reports `replacedPreviousInstall=true`, so the replace path is exercised too.

The current acceptance gate is `cargo run --manifest-path ..\FFOneClient\Cargo.toml -p xtask -- assets --source ..\FFOneClient\assets\game --full`; it validates the complete world
dependency closure independently of Bevy startup or unrelated client compilation.

## Source scene census

Component totals across all 161 exported tile hierarchies:

| Component | Count | Published |
| --- | --- | --- |
| Transform | 427,652 | hierarchy only, baked into each payload's world matrix |
| MeshFilter | 222,882 | via its renderer or collider |
| MeshRenderer | 140,009 | yes, as `visuals` |
| MeshCollider | 59,191 | yes, as `colliders` |
| MonoBehaviour | 36,303 | audited; supported world classes published in the behaviour domain |
| Animation | 18,684 | 17,751 world-map playback records plus exact legacy clip curves and target pivots |
| SkinnedMeshRenderer | 6,024 | yes, as `visuals` |
| SphereCollider | 4,438 | yes, as exact local-space trigger volumes |
| Rigidbody | 1,542 | source-verified kinematic state published and driven by native platform behaviour |
| TerrainCollider | 161 | terrain, owned by the terrain exporter |

No `ParticleEmitter`, `ParticleAnimator` or `ParticleRenderer` component exists in any world-map
tile bundle. Tile-local visual effects are not authored as per-tile particle systems; they are
instantiated by the scripted objects counted above from `Effects.resourceFile`. Effect parity
therefore depends on the MonoBehaviour work below, not on more geometry extraction.

## Behaviour status and known gaps

The current atomic behaviour publication contains 170 tile documents and 171 owned files
(389,481,282 bytes), has zero conversion blockers, and is bound by source-set BLAKE3
`e7e35b89f7420e170f5c10f3c1b622d50e9f41e2022358f5328140dcd1a3971c`. The fresh raw pass
contains 60,167 behaviour records and the same number of finite source-derived world matrices;
world-map matrices must exactly equal the separately published hierarchy, while the terrain-only
tutorial scope retains the source matrix directly. A full published-asset audit compiles 2,593
named scripted particle effects and 1,206 EP effects across all 170 documents.

1. **Script execution.** The behaviour domain now publishes exact BillboardNode/VisibleSwitch
   associations, effect payloads, animation clip references, Infected-Zone trigger records,
   waypoints, local-space sphere/box/capsule volumes and rigid-body state with no unresolved
   blockers. Billboards, visibility switches, exact AnimationClip transform/material curves,
   trigger-volume occupancy, all seven world EP effects and serialized EffectEmitterController
   particle prefabs run natively. Jumppad, launcher, zipline, slope, rope, belt, switch and moving
   platform actions now drive local movement/state. The registered special-movement actions emit
   exact protocol-0104 pack(4) payloads rather than generic approximations. Per-tile ring reset
   assigns clean one-based server IDs and hides every course until the race subsystem activates
   it, matching `GameFrameEpContainer.ResetRingsThread` instead of displaying all 2,960 rings in
   ordinary traversal.
2. **Kinematic world motion.** All 1,542 published `Rigidbody` records are kinematic in the clean
   source. The native platform driver evaluates the clean line, curve, rotate and waypoint modes,
   reparents the exact owned visuals and mesh colliders around the authored pivot, and carries the
   avatar by preserving its platform-local support point. This source set therefore does not
   require a free rigid-body solver for world parity.
3. **World animation.** The 17,751 world-map `Animation` playback records resolve their exact
   `AnimationClip` objects, rebuild the required local pivot hierarchy around world-baked models,
   and evaluate legacy cubic transform and supported material curves natively. Animation and
   BillboardNode compose on separate pivots instead of overwriting one another.
4. **World effects.** The 8,811 world-map emitter records preserve exact EP particle elements or
   `EffectEmitterController` script keys. The seven world-only EP IDs are in the validated native
   effect catalog, and every named particle prefab carries a recursive serialized dependency
   closure that is compiled by the same audited Unity particle renderer as tutorial effects. The
   renderer covers every particle shader name present in the publication and distinguishes Unity
   ARGB4444 from RGBA4444 instead of swapping the alpha channel.

The null-prefab rule was verified against `primary` `main.unity3d` (7,000,415 bytes, SHA-256
`59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F`). Its navigation-cache
extraction `Assembly - CSharp - first pass.dll` is 340,992 bytes with SHA-256
`D5849A0B866DE92740AF11DB620D1687C54C38A5554DAEC23BF4AEB3FC2AC5F3`; inspection with
Mono.Cecil 0.11.4 shows that `EffectEmitterController.Start` removes an `EffectKey` whose
`particlePrefab` is null. FFOne therefore treats an all-null serialized particle list as exact
no-particle behaviour; the independently instantiated `nifObject` is already materialized by the
static-world exporter. The DLL and extraction cache remain offline evidence only.
5. **Payload duplication.** Each renderer becomes its own single-root GLB with the world matrix
   baked in, and each tile owns a private copy of its textures. For `map_03_04` that is 2,922 GLBs
   from 389 unique source meshes. This is lossless but far from minimal; cross-tile mesh and
   texture deduplication is not implemented.
6. **Resolved control-plane scale.** The former quarter-million-entry global manifest was removed.
   Development validates domain roots only; release groups these GLBs into tile-scoped packs.
7. **Visual comparison.** Reference-client screenshots now cover Future, Past/Suburbs, Downtown,
   Wilds and Darklands locations. The post-repair GPU batch in
   `work/legacy-sources/world-capture/bevy-asset-winding-fix` renders 14 fixed poses, and the focused
   `bevy-asset-winding-probe` repeats the Sector V and Tech Square regression views after removing
   runtime Front/Back compensation. This validates the winding/culling repair across opaque,
   cutout, transparent, additive, glow and water examples. It does not by itself promote every
   unrelated world-system detail to `reference matched`; broader parity remains tracked in
   [the roadmap](../../../../FFOneClient/docs/parity.md).
