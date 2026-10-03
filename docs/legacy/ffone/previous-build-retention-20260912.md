# Previous-build retention (2026-09-12)

Question: which FFOne assets must keep the clean Retrobution 20260613 appearance after the
20260821 primary revision replaced them, and how do later migrations keep them?

Source roles: `previous` is the clean `retrobution-20260613` build (raw containers pinned by size
and SHA-256 in the declaration). The declined revision is `primary` `retrobution-20260821`.
This is an owner decision, not a parity claim: primary remains the parity authority everywhere
else. The same pattern already applies to Kevin (NPCs 2226 and 3380) and the original Ben,
Gwen, Kevin, Albedo and Grandpa Max variants.

- Declaration: [previous-build-retention.json](../../../recipes/native/retention/previous-build-retention.json)
- Publisher and check: `tools/legacy-sources/retain-previous-build-assets.py`
- Shared migration guard: `tools/legacy-sources/previous_build_retention.py`
- Receipt: `recipes/native/retention/previous-build-retention-20260912.receipt.json`

## Retained assets

| Native output | Previous identity | What 20260821 changed |
| --- | --- | --- |
| `ui/en/gameplay/intro/tutorialpan_06.png` | `Tutorial.resourceFile` Texture2D 170 `TutorialPan_06` | repainted pan |
| `ui/en/gameplay/intro/tutorialpan_07.png` | `Tutorial.resourceFile` Texture2D 148 `TutorialPan_07` | repainted pan |
| `ui/en/gameplay/loading/load.png` | `main.unity3d` `sharedassets0.assets` Texture2D 369 `load` | new loading art |
| `ui/en/launcher/login/login_screen_bg_16x10.png` | `CharacterCreation.resourceFile` Texture2D 35 | new login art |
| `ui/en/gameplay/guide/ben.png` | `main.unity3d` Texture2D 135 `Ben` | Omniverse-style Ben |
| `ui/en/gameplay/guide/ben_confirm.png` | `main.unity3d` Texture2D 211 `ben confirm` | 116x182 became 126x185 |
| `ui/en/gameplay/guide/ben_icon.png` | `main.unity3d` Texture2D 517 `ben icon` | new icon |
| `ui/en/gameplay/mission/guide-banners/ben.png` | `main.unity3d` Texture2D 575 `ben_banner` | new banner |
| `data/tables/npc_texture_overrides.json#npc_deedee` | `CharacterCreation.resourceFile` Texture2D 93 `npc_deedee` | new Dee Dee atlas |
| `data/tables/table-set.json` NPC rows | `TableData.resourceFile` MonoBehaviour 7 `xdtdatas`, mesh row 16 | mesh row 16 became `npc_deedee2` |

The guide images are the Ben entries of the guide selection menu (`guide_ui.rs`). The banner route
has no FFOne consumer yet; it is retained so a future mission banner shows the previous art.

### Dee Dee

Previous mesh row 16 is `npc_deedee`/`npc_deedee`. Thirty-six previous NPC rows select it:
Dee Dee (701), thirty-three placeholder rows that reuse her model, and Birthday Basher
(159, 3412). Primary changed row 16 to the new `npc_deedee2` model. FFOne had appended native
mesh row 600 (`npc_deedee2`) and pointed thirty-four of those rows at it: the thirty-three
placeholders before 2026-09-01 through an unrecorded table-set rebuild, and Dee Dee herself in the
2026-09-05 NPC repair. Retention points all thirty-six rows at native mesh row 16 again. Row 600 is
left in place, unreferenced, so no later mesh index shifts.

The `npc_deedee` override returns to the model's own previous texture
(`characters/npcs/npc_deedee/npc_deedee.textures/npc_deedee.png`), which FFOne used until
2026-09-06. The previous `NpcTexture.resourceFile` cannot supply it: its route
`texture/npc_deedee.dds` resolves to PathID 93, which is `rifle_shortsnipe_d` (the route
`texture/rifle_shortsnipe_d.dds` points at the same object). Check with
`fusionforge list-assetbundle ../builds/retrobution-20260613/NpcTexture.resourceFile`.

Verified already previous, not rewritten: `characters/npcs/npc_deedee/npc_deedee.glb` (both builds
export byte-identical candidates), `characters/nanos/nano_deedee/nano_deedee.glb` (all 32 native
animations equal the previous export), the `nano_deedee` atlas, and
`ui/en/server-selection/login_screen_bg_16x10.png` (previous `main.unity3d` PathID 80).

### Fusion Dee Dee follows primary

Fusion Dee Dee (`characters/fusions/fusion_deedee/fusion_deedee.glb`) is explicitly not retained.
It keeps the 20260821 revision: the `SkinnedFusionMatterLightDir` sub-link_b shader and the
128x512 primary `spwaneye` eye. The first application republished it from the previous build. The
owner corrected that the same day, and the installed GLB was restored byte-identically to the
20260821 publication (SHA-256 `18e65c8be61065667a8b7a52dcf165f2b399e663e43c1c60651a62ec77ba530d`,
FFOne HEAD `c88e460e9`). Its NPC rows (mesh row 308) were never touched. The retention tool still
supports declared models for future use, but the declaration lists none.

## Extraction pitfalls

- Previous `main.unity3d` holds two different textures named `ben_icon` (PathID 121, 18x18) and
  `ben icon` (PathID 517, 34x34). `unityextract` writes both to `ben_icon.png`, and the survivor
  depends on the filter. Only PathID 517 is the guide icon. `ben confirm` also carries a space. The
  tool pins each filter and fails unless the expected file is written exactly once and decodes to
  the declared PathID's `export-exact-texture` pixels.
- `find-objects.cmd -Source previous` requires `work/sources/previous`; build it once with
  `fusionforge index-client ../builds/retrobution-20260613 work/sources/previous`.

## Migrations that must respect the retention

- `publish-retrobution-ui-20260821.py` drops retained routes from its plan and reports them as
  `retainedPreviousBuildRoutes`.
- `stage-npc-repair.py` (tables and textures lanes), `stage-npc-material-followup.py` and
  `publish-npc-material-followup.py` skip retained NPC rows and texture overrides.
- A rebuilt table set (`install-table-set`) or any new primary migration can reintroduce
  `npc_deedee2` or the primary art. The history above shows that already happened once through
  an unrecorded rebuild. After such a change, run the check and apply if it reports `pending`.

```powershell
python tools\legacy-sources\retain-previous-build-assets.py `
  --previous-build ..\builds\retrobution-20260613 `
  --work work\cases\previous-build-retention-20260912 `
  --target-root ..\FFOneClient\assets\game `
  --receipt recipes\native\retention\previous-build-retention-20260912.receipt.json --check
```

Replace `--check` with `--apply` to reinstall; the receipt is rewritten from the replay.

## Verification (2026-09-12)

- NPC table: exactly 34 rows changed, only `m_iMesh` 600 -> 16. All 36 retained rows now select
  `npc_deedee`, and no NPC row selects `npc_deedee2`. `table-set.json` keeps its CRLF bytes; only
  those 34 values differ.
- `npc_texture_overrides.json`: only the `npc_deedee` entry changed, and its SHA-256 matches the
  file. Six other entries (`fusion_edd`, `fusion_juniper`, `fusion_numbuhthree`,
  `fusion_professorutonium`, `fusion_samuraijack`, `mob_bshell`) already mismatched their files at
  FFOne HEAD `c88e460e9`; they are unrelated migration debt.
- All eight UI routes decode to the declared previous pixels.

## Not retained

- `characters/fusions/fusion_deedee/fusion_deedee.glb`: follows primary (see above).
- `characters/npcs/npc_deedee2/npc_deedee2.glb` and `characters/mobs/npc_deedee2/npc_deedee2.glb`:
  20260821-only model, installed but no longer selected by any NPC row.
- `characters/npcs/npc_deedee/textures/npc_deedee.png`: the declined 20260821 atlas, no longer
  referenced by the override catalog.
- `ui/en/user-equip/guide-ben-tennyson.png` (`BenIcon`): the equipment-window guide icon is outside
  the requested guide selection and banner scope and stays at 20260821.
