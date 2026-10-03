# Pirate combat audio and Academy Nano voices

The reported behavior was NPC dialogue during combat with Candy Buccaneer in
Fusion Wilt's instance, and missing Academy Nano summon/ability speech. The
owner clarified that the Nano report concerns voice, not ability sound effects.

## Pirate

The native Russian name «Конфетный пират» also names hostile Candy Buccaneer
rows 94, 288, 1265 and 1316. These select `mob_pirate`; they are distinct from
friendly HNPC 775, whose dialogue owner is `M_Pirate1`.

`mob_pirate` requests `Pirate_Melee1.wav` from `melee1` at 0.1 seconds. Its
`wound` clip has no sound event. The native catalog erroneously declared the
same true name twice: `sfx/combat/pirate_melee1` and `voice/pirate/melee1`.
The EN streams were identical, but the RU voice stream was different. General
character playback prefers a localized voice when both categories match.
Consequently this combat cue could select that unrelated localized recording.

The repair removes only the erroneous voice row. The remaining SFX file is
byte-identical to primary `DongResources_08_06.resourceFile`, serialized asset
`CustomAssetBundle-a3962f73ba4214b50b6d88e38442b1c2`, AudioClip 179.
The original Ogg SHA-256 is
`b00df248daac22bfd35ba81e276e8daf81dc3b09b7705675b32fc0a3c2eecdb9`.
NPC dialogue routes and ordinary character voice priority remain unchanged.
The removed route's old unreferenced files are not loaded or discovered as takes.

This proves the catalog collision and repaired playback route, not that a
specific sentence was acoustically identified in a live Fusion Wilt session.

## Nano recovery

Three installed models lacked voice events that exist in the raw Academy
donor. The exact Animation owner pointer arrays and four AnimationClips per
model are exported with `dump-object-evidence`; packed Academy pointers are
resolved by that exporter, not interpreted as ordinary file indices.

| Native owner | Alternate container | Animation owner | Summon and skill event time |
| --- | --- | ---: | ---: |
| `nano_flapjack` | `Nano_044.resourceFile` | 2742116923 | 0.25 s |
| `nano_chowder` | `Nano_046.resourceFile` | 2187236750 | 0.1 s |
| `nano_zaksaturday` | `Nano_049.resourceFile` | 3868737284 | 0.1 s |

Serialized assets are respectively `CustomAssetBundle-Nano_044`, `_046` and
`_049`. Each imported sound keeps the donor's exact payload, time and order.
This is an explicit alternate extension; primary models with absent events do
not define the Academy donor contract.

Chowder additionally lacked `skill1`, `skill2` and `skill3`. The maintained
publisher exports the exact donor model, converts it only in Editor staging,
and appends only those three animations and their referenced buffer data.
The native and donor skeletons have different node ordering. Every node is
matched by its unique complete hierarchy path, and exact rest translation,
rotation and scale are required before channel indices are remapped.
Existing nodes, meshes, skins, materials, texture references, animations and
the complete original binary buffer prefix are preserved.

Jack O'Lantern had a separate spelling mismatch: events requested
`Jack O'Lantern_*`, whereas the declared voice true names are
`Jack_O_Lantern_*`. The repair corrects only that exact prefix in sound events.
It does not add fuzzy name or display-name based audio resolution.

All repaired voices retain the native audio catalog and `LocalizedVoice` path:
select a declared take, resolve requested locale, then EN, then silence.
No per-language rows, hardcoded locale paths in gameplay, or substituted voice
owners are introduced. The already installed Ogg payloads are unchanged.

## Reproduction and verification

The maintained tool is `tools/native/repair-nano-voice-events.py`.
`recipes/native/audio/pirate-nano-voice-repair-20260912.receipt.json` records
scoped source identities, raw hashes, replay commands and output hashes.
Work is under `work/cases/pirate-nano-audio-20260912`; only four GLBs and the
single audio-route deletion are installed in FFOne.

Two independent raw-source replays produced identical output hashes. All 96
repaired call/skill take resolutions (four owners, four actions, three takes,
EN/RU) resolve and decode to non-silent waveforms. The existing production
audio/Nano test executable passed all 56 tests on the installed content.
New native regressions exercise the real model event parser, semantic catalog,
pending Nano voice loading and spawned localized players in both languages.
The Pirate regression verifies that its animation request creates the combat
SFX player without `LocalizedVoice` in either language.

Final verification passed all 59 tests in the freshly compiled production-module
test executable, including both new regressions. The complete client and Nano
preview also built. `ffone-client.exe --validate-assets` returned routing status
`ok`. The preview's optional third argument requests skill 1, 2 or 3 after call
completes; all three Chowder captures were inspected and show intact animated
geometry. A fresh-work replay against already repaired assets produced the same
five output hashes, without appending duplicate clips.

Verification binaries, exact compiler commands, logs and captures live below
FFOne's `target/performance/pirate-nano-audio`. Simultaneous unrelated UI changes
made live library/main builds inconsistent, so the final complete client uses
a recorded consistent source snapshot and the current development dependencies.
The regressions compile the live production audio/Nano modules. No live server
combat recording or acoustic loopback comparison is claimed.

## Remaining source gap

Titan's `skill3` requests `Titan_NanPwrBRockets`. The installed catalog lacks
that family. The raw Academy `Nano_048.resourceFile` AudioClip listing contains
only `SSword` and `SArray` power families, and focused primary/alternate route
queries for `titan_nanpwrbrockets` returned no matches. This remains an
unresolved recording gap; a different ability's line was not substituted.
