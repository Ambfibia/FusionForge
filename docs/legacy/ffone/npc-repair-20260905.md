# NPC repair — 2026-09-05

Owner-requested repair of native NPC rendering/editor coverage and current Retrobution additions. Native output lives in FFOneClient; every source identity, extraction plan, replay and receipt remains here.

## Installed result

- Updated Dee Dee, Dexter, Courage and Numbuh Two, including Dexter pistol/sword and future Numbuh Two variants. Nine GPU-accepted NPC models published in total, including the two new source routes for catacomb entrances and big dog.
- Kevin 2226 uses the previous primary Retrobution 20260613 mesh, texture and visual timing. Existing gameplay fields and ID remain intact.
- Runty 3036 now uses the current primary visual contract.
- Restored XDT texture resolution for Fusion Rex, Fusion Johnny Test, Paradox, Azmuth, Fusion Kimchi, Fusion Finn and Otto. Fusion Johnny Test additionally required preserving its declared but unassigned `_MainTex` binding for runtime assignment.
- Fusion Ice King, Mandroid M-199, Mandroid Infiltration Unit and Fusion Spidermonkey load and render in the native GPU preview. Their original exact models were already present; no substitute meshes were introduced.
- Editor now uses the runtime HNPC assembly path. The native HNPC catalog contains 205 appearances (185 preserved, 20 appended), 331 texture contracts and nine newly published equipment parts. Named native routes isolate new clothing from distinct existing item variants.
- Native HNPC catalog v2 removes source provenance from the runtime definition and validates native size/hash/sampling metadata. Existing 305 HNPC PNG paths are preserved; ten new PNGs live under textures/hnpc.
- Recovered 500 scoped icon Texture2D objects. 357 icon PNGs differ from or extend the installed set; 135 existing NPC rows now reference the current primary icon numbers.
- Original Ben Tennyson, Gwen Tennyson, Kevin Levin, Albedo and Grandpa Max icons/models are protected. Separate owner NPCs 3464–3468 are preserved.
- Client NPC table and both OpenFusion/tdata/xdt.json and OpenFusion/bin/tdata/xdt.json are synchronized. No world placements were invented for new table definitions.

## New IDs

| Primary ID | Native/server ID | Name |
|---:|---:|---|
| 3430 | 3469 | Ranger Lindsey Lenses |
| 3431 | 3470 | Sweeper Johnny |
| 3432 | 3471 | Sweeper Johnny2 |
| 3433 | 3472 | Sweeper Johnny3 |
| 3434 | 3473 | Upper Catacombs |
| 3435 | 3474 | Lower Catacombs |
| 3436 | 3475 | Magic Krab |
| 3437 | 3476 | Numbuh Mach 2 |
| 3438 | 3477 | Sweeper Leon |
| 3439 | 3478 | Sweeper Leon2 |
| 3440 | 3479 | Sweeper Leon3 |
| 3441 | 3480 | Sweeper Charlie |
| 3442 | 3481 | Sweeper Charlie2 |
| 3443 | 3482 | Sweeper Charlie3 |
| 3444 | 3483 | Sweeper Randal |
| 3445 | 3484 | Sweeper Randal2 |
| 3446 | 3485 | Sweeper Randal3 |
| 3447 | 3486 | Sweeper Jay |
| 3448 | 3487 | Sweeper Jay2 |
| 3449 | 3488 | Sweeper Jay3 |
| 3450 | 3489 | big dog |

## Remaining source gaps

| Native ID | Name | Unresolved resource |
|---:|---|---|
| 3257 | Chupacabra | No verified model route npc_bustle_chupacabra in the inspected source indexes. |
| 3367 | Hippie Hop | No verified model route mob_ruinrhino. |
| 3428 | Exterminator Tent | Primary row selects HNPC appearance 0 (empty); no verified npc_exterminatortent model route. |
| 3148 / 3395 | Chowder | 3148 requests absent texture npc_chowder2; 3395 requests absent model npc_chowder2. The ordinary npc_chowder payload alone does not prove the missing variant. |

The current/previous primary and configured alternate indexes were inspected. These gaps remain unresolved pending a build containing the actual resources; no similar-looking substitute was published.

Fusion Four’s Door (662) and Fusion Blossom’s door (669) are primary class-111 ObjectNPC1 location markers. The clean SetupNPC rule hides classes >= 100. Guide Changer (3035) is class 19 but likewise references ObjectNPC1; it has no separate character model in that row. Do not describe Guide Changer as class 111.

## Verification

- Built ffone-editor and ffone-client successfully.
- Native HNPC production catalog test passes, including all 20 new appearances.
- Editor production catalog test passes.
- Declared-null NPC material regression test passes.
- Nine candidate NPC GLBs and nine new equipment parts passed GPU acceptance. Installed editor captures verify old and new HNPCs, the rollback and repaired materials; reports and PNGs are under work/legacy-sources/npc-repair-20260905.
- Independent replay matched all 9 GLBs and 19 recovered exact Texture2D documents byte for byte. The missing Chowder texture is explicitly recorded as blocked.
- Exact whole mip-chain bytes, color space and sampler contracts are compared before redirecting models to shared PNGs. GLB BIN chunks are unchanged by relocation. New player items pass scoped hash and external-image closure checks.
- Full player-item repository verification still encounters pre-existing publication debt: the existing hnpc-runtime-textures directory is classified as retired, and a later check finds an unresolved ToonRamp9 URI in existing melee_eduardoclub. These pre-existing item issues are outside the nine accepted additions; no global-clean claim is made.
- EN/RU key sets and template placeholders match; new NPC content names are keyed and displayed through localization.
- Both server documents match every native table section; 49,230 checked index references, zero broken.
- Final publication receipt verifies 437 changed/added outputs. Existing owner assets and unrelated UI/localization changes were preserved.
- Read-only native duplicate audit ran in FFOneClient; output is target/performance/render-duplicates.json. No FPS improvement is inferred from duplicate counts.

## Reproduction and evidence

Tracked plans and receipt: recipes/native/characters/npc-repair-20260905*.json. The receipt contains source role/container/serialized-object identities, raw hashes, protected identities, source-to-native ID mapping and native output hashes.

Run from FusionForge. `recover-npc-assets.py` replays the scoped model and texture plans; `export-exact-texture-batch` replays each icon selection. Equipment sources use `export-equipment-model-sources`, then `publish-equipment-logical-model-batch` and GPU acceptance. All candidate output belongs under an ignored Editor work tree.

Stage a copy of the original native assets, install the accepted models/items, then run the models/textures/tables/hnpc/icons/icon-rows lanes of `stage-npc-repair.py`. Run `close-npc-native-textures.py` after item installation: the existing installer preserves old relative PNG URIs, so this verified closure step is required before accepting the installed tree. Preserve original JSON preimages and use atomic replacement when the staging tree contains hardlinks.

`publish-npc-repair.py` produces a pinned publication plan and applies it with verified preimages/backups. The ten new HNPC textures use the final textures/hnpc path; the receipt records the intermediate-path correction made during this run. Sync both server paths with `sync-openfusion-xdt.py` after native validation.

Original trial failures, source defects, deterministic replay reports and installed captures remain in work/legacy-sources/npc-repair-20260905; generated data is not a runtime dependency.
