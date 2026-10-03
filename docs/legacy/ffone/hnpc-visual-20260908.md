# HNPC banker clothing and skin palette — 2026-09-08

The native HNPC catalog retained earlier outfit substitutions for Ruby Banker
(NPC 3124, appearance 176) and Emerald Banker (NPC 2507, appearance 177).
Current primary `all_hnpc.asset` selects `f_shirt_secretagent`,
`f_pants_secretagent`, `f_shoes_secretagent` and `shoes_agentsix`. The installed
catalog instead selected the separate secretagentcoat/secretagentpants/
secretagentshoes set. Those meshes have different geometry and UV islands.
The repair publishes the three exact primary models, keeps the existing banker
shirt/pants atlases, and installs the declared shoe texture. Shared toon-ramp
references are reused only after full sampling/color/mip and byte equality.
The other appearances and every NPC placement remain unchanged.

`HNPCLook` directly indexes `bsdManager.cSkinColor.skinColor[iSkinColor]` and
the corresponding hair array. The native loader previously interpreted these
as one-based creator codes and used a creator palette with only 12 skin and
18 hair entries. Missing extended colors became white. The new native
`data/hnpc/palette.json` carries all 20 zero-based entries for each palette,
preserving serialized f32 components. Creator UI choices are independent.
The existing white fallback for unset/out-of-range selectors is preserved;
appearance 147 still contains invalid source skin selector 20.

Primary palette authority is `main.unity3d`, `sharedassets0.assets`,
MonoBehaviour 1378 (`male.bsd`). HNPC authority is `TableData.resourceFile`,
`CustomAssetBundle-1dca92eecee4742d985b799d8226666d`, MonoBehaviour 6.
Scoped source identities, replay commands and output hashes are in
`recipes/native/characters/hnpc-palette-20260908.json` and
`recipes/native/characters/hnpc-banker-20260908.json`.
The HNPCLook managed extraction is recorded in
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/assembly-csharp.evidence.json`.

## Verification

- `hnpc_palette_preserves_zero_based_extended_colors` and
  `production_catalog_closes_all_published_hnpc_appearances` pass.
- `xtask assets --source assets/game` passes (43,804 references).
- Production GPU captures for appearances 176, 177 and 31 reach
  `ReadyAnimated`. Capture 31 exercises extended skin selector 15.
- The development executable built at 21:51:44 +03:00 contains the new palette
  consumer and passes `--validate-assets --asset-root assets/game`. A redundant
  queued rebuild was stopped after verifying that executable.
- Both publishers replay identically into Editor staging; all six output
  SHA-256 hashes match the installed native files.
- Captures are below `work/cases/npc-visual-20260908/`: `banker-ruby.png`,
  `banker-emerald.png`, `skin-15.png`. `banker-ruby-before.png` uses the previous
  native catalog with the same corrected palette and camera. The controlled
  `skin-15-before.png` capture reproduces the former palette lookup, with all
  other resources shared with the current native tree.

## Fred face report remains open

The owner clarified that Fred's reported problem is the face texture, not the
mesh. Native NPC rows 3310/3407 select `npc_fredfredburger2` with the same named
texture as primary. Installed UVs match the current typed export within f32
rounding (maximum component difference below 3e-8); the main PNG is identical
to the source-derived candidate. Static and animated GPU captures load without
material errors. The static face is visible in `fred-static.png`.

Re-exporting the model changes its renderer node and winding metadata but did
not resolve an identifiable face-texture discrepancy. That candidate was not
published. An in-game screenshot is needed to identify the owner's precise
failure before choosing a repair. No Fred payload or table row was changed.
