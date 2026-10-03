# Fusion Kimchi: Academy geometry and the original Fusion atlas

NPC 3460 uses the complete Kimchi model recovered from the owner's Academy build
and the original `fusion_kimchi` texture recovered from `beta-20101123`. The atlas
contains the oval red eyes with dark angular upper lids shown in the owner's
reference. The source geometry, UVs, skin and animation remain intact.

The earlier white-eye atlas, shiny `spwaneye` projection and Fusion Coco eye-card
transplant were incorrect. The Coco trial was rolled back without an accepted
receipt. The final publication supersedes `fusion-kimchi-fusion-eyes-20260906`:
[publication receipt](../../../recipes/native/characters/fusion-kimchi-academy-original-atlas-20260906.receipt.json).

## Source chain

Academy discovery uses alias `academy`, canonical role **alternate**, with the
owner's build at `../builds/6543a2bb-d154-4087-b9ee-3c8aa778580a`.

- `TableData.resourceFile`, 751481 bytes, SHA-256
  `d724c9417de5ff35e9bdb7bdfcf9bb93b4c8f715f42623fa2e1834d835be60a5`:
  serialized asset `CustomAssetBundle-TableData`, MonoBehaviour `xdtdatas`,
  path ID 2139558964. NPC 3543 selects mesh row 523, `npc_kimchi` with texture
  `fusion_kimchi`; ordinary NPC 3544 selects row 522 with `vehicle_kimchi`.
- `DongResources_00_12.resourceFile`, 8025967 bytes, SHA-256
  `75059741cf78db2492b4bfe33ba01c0d8f9f78cd950f1622ebdce4c337dc9fc9`:
  serialized asset `CustomAssetBundle-DongResources_00_12`, route
  `mob/npc_kimchi.kfm`, GameObject 2271773135. Body Mesh 4010026119 contains
  550 vertices and 660 triangles. The exported model retains all four meshes,
  13 skin joints and three clips: `death`, `nif-default`, `stand1`.
- The managed NPC loader requests `texture/<table name>.dds`; its recovered
  implementation contains no Kimchi-specific substitution or eye-mesh assembly.
  The dedicated atlas was absent from the checked Academy split NPC texture
  bundles. A name scan across 647 decompressed cache files found its table name
  only; that scan is navigation evidence, not the authority for the final texture.
- Original 2010 builds use `NpcTexture.resourceFile`, rather than the later split
  `Character_Texture_npc*` names. The exact texture exists in both checked
  `beta-20101011` and `beta-20101123` archives. The final donor is
  [beta-20101123 NpcTexture](https://cdn.dexlabs.systems/ff/big/beta-20101123/NpcTexture.resourceFile),
  12156773 bytes, SHA-256
  `75572d2cea620e6a320ef7303424b655ca515279492ee0e6a97d4078d1aedb99`.
  The scoped identity is `CustomAssetBundle-NpcTexture`, Texture2D 1238925593,
  route `texture/fusion_kimchi.dds`. Version navigation comes from
  [OpenFusionClient's build list](https://github.com/OpenFusionProject/OpenFusionClient/blob/master/defaults/versions.json).

The atlas is 256 x 256 DXT1 with one source mip, linear filtering, repeat sampling,
anisotropy 1 and mip bias 0. Its exact decoded PNG SHA-256 is
`2617fe0aba23496809ae24f98ac21b17c38ccb5fab09d92c18583e4b1f6f4d22`.
The texture export has zero unresolved pointers. This is an explicit historical
alternate donor extension; the texture was not recovered from the Academy folder.

## Packed-pointer recovery

Academy's format-7 assets with long object IDs pack the external index into the
low 16 bits of the first pointer word and the high object tag into its high
16 bits. The second word contains the low 32 object-ID bits. For example, the
Kimchi material shader pointer bytes `01 00 e6 b1 7c 05 6f d1` mean external
index 1 (`TrainingGrounds`) and full object ID `0xb1e6_d16f_057c`; the target's
object table stores `7c 05 6f d1 e6 b1 00 00`.

`Asset::pointer_file_index` decodes the external index while preserving the raw
pointer fields. The strict resolver validates the named external owner and the
complete object tag. It does not select an unrelated low-ID match. The scoped
Unity parser suite passes all 18 tests, including incorrect tag and absent
external-owner cases. Typed model recovery now resolves the source materials,
rig and animation through that contract.

## Native publication and verification

`tools/legacy-sources/publish-academy-kimchi.py` stages and independently replays
five native files. It changes only the model root name to `fusion_kimchi`; the
Academy binary geometry, UVs, skin, animation and mesh definitions remain exact.
The original ToonRamp9 material binding reuses the existing complete
`textures/shared/toonramp9_variant_03` mip chain after pixel and sampler equality
checks. No per-model texture copies are introduced for that shared chain.

Native NPC 3460 retains mesh row 581 and alias `npc_3460_kimchi`. Its texture cell
now selects `fusion_kimchi`. The identical single-cell change is applied to
`OpenFusion/tdata/xdt.json` and `OpenFusion/bin/tdata/xdt.json`. All 63 carried
outputs from the preceding NPC publication remain byte-identical.

`tools/legacy-sources/audit-academy-kimchi.py` verifies installed bytes against the
independent replay, the scoped table changes, source geometry and animations,
every shared mip, raw donor hashes, and the absence of legacy identities in the
native model. The native production regression passes; both the native client
and editor build successfully. The actual NPC editor reports ready with no
error, and its capture shows oval red eyes with dark upper lids.

Raw downloads, typed exports, plan, backups, replay, logs and captures remain
under ignored `work/legacy-sources/fusion-kimchi-academy-20260906/`. Relevant
visual evidence is `owner-reference.png`, `staged/3460.png`,
`editor-correct.png` and `editor-installed.png`. The final native payload does
not require this workspace, the Academy build or any legacy conversion tool.
