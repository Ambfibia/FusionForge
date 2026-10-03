# Unstable Nano: latest Retro powers and retired Van Kleiss alias

The owner requested removal of the duplicate Unstable Nano using Van Kleiss's
model, and restoration of the remaining Unstable Nano's powers from the latest
**Retro** build. Retro is distinct from Retrobution. The owner explicitly selected
`retro-010920`; Retrobution 20260821 is not the authority for this change.

## Source contract

The strict, hashed table evidence is recorded in
`docs/reference/evidence/legacy/ffone/sources/retro-unstable-powers-20260915.json`.
Retro `assets/TableData.resourceFile` is 744830 bytes, SHA-256
`700f47e9d80d861b1a6392e80166bd10c18e9db4d463e0e519f60f6bf888d637`.
The unique `xdtdatas` MonoBehaviour is path ID 7 in serialized asset
`CustomAssetBundle-1dca92eecee4742d985b799d8226666d`; strict export resolves all
pointers. Source Nano 41 selects tunes 210, 211, 212, all pointing to skill 122.
All three names are `UNSTABLE POWER`, type `???`, with a description saying the
power currently appears to do nothing. Native skill 122 already matches every
source field exactly, including zero battery use. The owner's subsequent explicit
icon 67 selection changes only skill 122's icon-row reference, from 1 to 68.

Retrobution's slot 41 instead selects Coop with bonus/scavenge/cone-stun powers.
That was the wrong source for this request. The previous restoration's authored
neutral powers are not evidence for Retro's Unstable Nano.

## Native changes

- Native Nano 41 keeps its hologram model, portrait, name and quest ownership.
  Only its tune references change to new native rows 288–290.
- Those rows reproduce source tunes 210–212, remapping tune/string IDs only.
  Their requirements remain 100 Fusion Matter and five items of ID 37.
- Skill 122 is reused after complete gameplay-field equality with the raw source.
  Its icon now selects native skill-icon row 68, number 67, as requested by the owner.
  `Icons.resourceFile` in primary Retrobution contains the exact Texture2D at
  `CustomAssetBundle-784fa24bcf2da4f5eabe9547958616eb:1562`. The native
  `icons/skills/skillicon_67.png` is byte-identical to its exact PNG export.
  That file was missing at the existing Nano UI icon route, which is now populated
  with the same verified bytes. No other ability's icon reference changes.
- The invalid apostrophe decoded as U+FFFD in the source description is corrected
  to an ASCII apostrophe. EN/RU semantic tuning and table keys are published together.
- Native 52, the rejected Unstable alias over the Van Kleiss model, becomes an
  empty reserved table slot. The array is not shortened and IDs are not shifted.
  The client gallery removes 52 and contains 66 active Nanos. Historical bank
  records for 52 do not recreate a gallery entry.
- Van Kleiss 66 retains all its existing data and shared assets. Coop 67 keeps
  tunes 210–212. Ben 68, Ghostfreak 69 and Upgrade 70 keep tunes 285–287.
- The server XDT receives the same Nano table and skill 122 icon reference.
  Other table rows, native models, geometry and audio remain unchanged.

## Transparent portrait repair

The owner also reported an absent Unstable Nano in the bottom-right Nano wheel
and the mission journal. `gameplay_hud_gpu_preview` reproduced both empty regions
with production Nano 41 and task 5215. The native hologram's FusionEffect material
uses the faithfully retained `ColorMask RGB`; rendering it against a transparent
RGBA target writes color while leaving alpha zero. The UI then discards the image.

`GameplayNanoPortraitPlugin` now gives RGB-only materials inside its own portrait
subtrees an instance-local RGBA-writing copy. This applies to HUD and journal
targets, including material passes that finish loading later. Color blending,
depth, culling, camera transforms, textures, animation and pass order are unchanged.
World instances retain their original material. An applied marker prevents repeat
allocation or unchanged handle writes. The regression test verifies the original
material remains intact and subsequent frames keep the same two assets/handle.

This supersedes the 20260913 decision to preserve alias 52 and the power assignment
for Nano 41 in `nano-gameplay-identities-20260913.md`; its other decisions survive.
The old publisher is historical. Use the commands below after replaying it.

## Replay from FusionForge

```powershell
work/build/debug/fusionforge.exe dump-object-evidence retro-010920 ../builds/retro-010920 assets/TableData.resourceFile 7 --type MonoBehaviour --out work/cases/unstable-nano-20260915/retro-table.evidence.json
python tools/native/retire-unstable-nano-alias.py --target-root ../FFOneClient --server-xdt ../OpenFusion/bin/tdata/xdt.json --stage work/cases/unstable-nano-20260915/retired --apply
python tools/native/restore-retro-unstable-powers.py --target-root ../FFOneClient --server-xdt ../OpenFusion/bin/tdata/xdt.json --source-evidence work/cases/unstable-nano-20260915/retro-table.evidence.json --stage work/cases/unstable-nano-20260915/powers --apply
python tools/native/restore-unstable-power-icon.py --target-root ../FFOneClient --server-xdt ../OpenFusion/bin/tdata/xdt.json --stage work/cases/unstable-nano-20260915/icon --apply
```

Omit `--apply` for staging. The publishers check their expected input contracts,
preserve unrelated content, guard concurrent target changes and replace each file
atomically. Replaying the installed result produces identical hashes.

## Validation and deployment

Production-bundle tests in `nano_identity_roster` cover the reserved slot,
remaining Van Kleiss model/icon, gallery with a historical ID 52 bank record,
three skill/tune references, icon 67's actual UI file, requirements, quest rewards and EN/RU keys.
Validation artifacts remain below `work/cases/unstable-nano-20260915`.
Client/server semantic comparison shows only Nano rows 41/52, six appended
tune/string rows and skill 122's icon reference change. The normal `OpenFusion/bin/database.db` contains no
saved Nano 41 or 52 records, so no player-save migration was necessary there.
The normal running server must reload the updated XDT on restart.

Accepted checks:

- `cargo test -p ffone-client --lib gameplay_nano_portraits::tests`: 6 passed.
- The same library test binary, `user_equip_ui::tests`: 35 passed.
- `cargo test -p ffone-client --test nano_identity_roster`: 2 passed.
- Real client plus HUD, equipment and acquisition examples build successfully.
- Isolated OpenFusion on ports 23260/23261 accepts all three new tune IDs,
  rejects old tune 285 for Nano 41, equips it and restores skill 122 after relogin.
- Reviewed `hud-journal-after.png` and `hud-after.png`: hologram appears in both
  targets and uses icon 67. The baseline `hud-journal-before.png` has empty models.
- The production `ffone-client` offline acquisition fixture finishes with
  `selected=true`; its PowerSelection and ResultSkill captures show the model,
  restored strings and icon. Output is under native ignored
  `target/performance/unstable-nano-20260915`. Offline network errors in the fixture
  are synthetic; actual persistence is covered by the isolated server check.
  Existing missing `Unstable_NanPwrAAA02/03` audio-route warnings are recorded in
  the log; this change does not claim to restore those audio clips.

Final accepted output hashes and commands:

- `recipes/native/tables/unstable-nano-retro-20260915-client.json`
- `recipes/native/tables/unstable-nano-retro-20260915-server.json`
