# NPC material and icon follow-up — 2026-09-05

The previous repair's GPU smoke reports proved loading, not correct appearance. This follow-up inspected the actual front/reverse renders and the real native editor. It installs 67 changed native files, including 50 corrected icon files, and restores Kevin 3380's previous-version mesh in both server XDT locations.

| NPC | Result |
| --- | --- |
| Fusion Ice King 252 | Recovered the current primary model/material closure, including FusionMatterLightDir and the additive eye material; restored the matching primary table texture. |
| Mandroid M-199 981 / Mandroid Infiltration Unit 2854 | Recovered the current primary renderer/material bindings and shader declarations; retained their exact table texture and UV transforms. |
| Fusion Spidermonkey 3256 | Recovered the primary model and current toon/Fusion overlay bindings. |
| HIPPIE HOP 3414 | Recovered the actual requested ID. Its exact tutorial FusionEffect shader uses black defaults, not the white defaults of the previously published material. The shader intentionally retains lighting-dependent transparency. |
| Dee Dee 701 | Selected the current model's CharacterCreation texture, replacing the incompatible same-named NpcTexture version. |
| Dexter 728 and pistol/sword model variants | Selected the CharacterCreation body texture. Renderer slot zero receives XDT replacement; authored face, sword and glass slots retain their textures. |
| Kevin 3380 | Restored the 20260613 mesh, matching the owner's previous-version policy for 2226. Icon and gameplay fields are unchanged. |
| Fusion Kimchi 3460 | Restored eyes using the exact primary vehicle_Kimchi atlas on the existing eye surfaces; kept the green body texture. Ordinary Kimchi is unchanged. |
| Fusion Four Door 662 / Fusion Blossom door 669 | These source rows are class 111 ObjectNPC1 location markers. SetupNPC hides their model. They are not standalone door models, and no substitute door geometry was assigned to these NPCs. |
| Guide Changer 3035 | Source class 19 uses ObjectNPC1 with no table effect. Its small marker now has the current primary material closure. A separate guidechanger.nif effect exists in world bundles, but it is not the NPC mesh selected by this row. The marker was not silently replaced with that world object. |
| Exterminator Tent 3428 | Still unresolved. The row selects empty HNPC appearance 0; the named texture exists, but a usable npc_exterminatortent model route was not found in the checked Retrobution, Academy and FusionFall-ru-0-0-4 indexes. |

## Renderer slot ownership

The recovered primary SetupNPC loop operates on `Renderer.material`, which is the first material slot. Applying its replacement to all glTF primitives overwrote Dexter's face, sword and glass materials. The repaired models carry a native primitive ownership field, `npcTableTextureWritable`; runtime and GPU preview both honor it. Older publications without the field retain compatibility behavior. No shared material is modified in place for a per-NPC replacement.

The two named Dee Dee/Dexter texture routes occur in both CharacterCreation and NpcTexture. Their names do not establish equality or compatibility. The accepted CharacterCreation bytes fit the current models' UVs and were verified visually in the editor.

## Kimchi eyes

The missing visible eyes were not missing geometry. They are painted in `vehicle_Kimchi`; the imported solid green XDT texture replaced their atlas regions along with the body. The small separate untextured blue meshes in the source are not a replacement for those visible eye surfaces.

`stage-fusion-kimchi-eyes.py` preserves four disconnected surfaces associated with the two source eye bones: 60 existing vertices and 58 existing triangles. It partitions the original 660-triangle body mesh into 602 green-body triangles and 58 atlas-eye triangles. Both primitives reference the original position, normal, UV, joint and weight accessors. Original binary data, triangle winding, animations and transforms remain unchanged. One disjoint material draw is added; there is no generated image or recolored source texture.

The eye atlas is byte-identical to Texture2D 144 in primary NpcTexture.resourceFile, serialized asset `CustomAssetBundle-d304c52c4bae348e38c743762c1bd818`. PNG SHA-256: `26eecc787eb44c6e2ae190a8eb48dc192958098b2bfbfe6f93ee8750f3a95442`. A complete immutable native texture chain is reused through a shared reference.

The native appearance fork is `npc/fusion_kimchi`, reached through the existing `npc_3460_kimchi` alias. Ordinary `npc_kimchi` retains its original runtime file. This is an explicit repair of the owner's green variant, not a claim that primary shipped this combined appearance.

## Icons and XDT

The runtime reads `icons/entities/mobs/mobicon_XX.png`, not `mob/`, and numbers below ten are zero-padded in all three families. The old staging/publishing helpers wrote the wrong paths. Both helpers now use the actual runtime naming contract.

All 500 recovered primary icon payloads were checked against the installed paths. Fifty files needed replacement. All 3490 XDT rows resolve their referenced NPC icon files: 537 unique paths, zero missing files. This validates reference and source-byte identity; it is not a claim that every portrait was individually judged by eye.

Ben Tennyson, Gwen Tennyson, Kevin, Albedo and Grandpa Max's protected icon paths and separate NPC identities were preserved. All XDT rows except 3380 are byte-for-value unchanged from the beginning of this follow-up. Row 3380 changes only `m_iMesh`. Both OpenFusion `tdata` and `bin/tdata` were synchronized; each check validated 49,230 references with no broken references. The Kevin 2226/3380 exception is recorded in Editor AGENTS.md.

## Verification and remaining limits

- Deterministic replay reproduced all 67 declared native outputs exactly.
- 55 focused production-resource tests passed; one intentionally ignored test remained ignored.
- 23 shader-state pipeline tests passed; one ignored.
- Actual client, editor and GPU preview development binaries built successfully.
- Installed GPU captures passed for IDs 252, 981, 2854, 3256, 3414, 701, 728, 3380 and 3460. Front/reverse views were inspected; Kimchi also has a direct front capture.
- Real editor captures for 701, 728, 3414 and 3460 reported ready with no error and were visually inspected.
- The read-only duplicate audit completed at native `target/performance/render-duplicates.json`; no FPS claim is made.

The initial broader production test run also exposed two independent failures. The equipment UI source scanner does not recognize that its localization ordering is registered in the parent editor module. Five Sweeper NPCs added by the previous publication reference overhead effect ES670, which has no installed native effect entry. Those two checks were explicitly excluded from the final focused run; neither is described as passing. NPC portrait-file coverage and overhead effect coverage are different checks.

The native runtime retains its separately authored comic lighting. These results establish source texture/material bindings and the requested appearance repairs, not a pixel-identical Unity renderer.

Evidence remains under `work/legacy-sources/npc-material-followup-20260905`, including actual editor captures, source recovery, donor triage and server reports. Academy's TableData dump required unresolved-pointer triage and was never accepted as publication authority. The FusionFall-ru-0-0-4 Kimchi comparison had the same green-atlas loss and was not used as the accepted donor.

The previous `npc-repair-20260905.receipt.json` uses a historical schema and remains unchanged. The new canonical receipt and scoped source record describe this follow-up. Global historical catalog ownership is still migration debt: older Nano/Cyberus receipts also mention the generated character registry and contain paths changed by subsequent texture sharing. The scoped hash/replay audit passed; this report does not claim a clean global historical-receipt audit.
