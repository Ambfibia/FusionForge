# Missing NPC, Guide, loading and Nano audio

The native investigation covers five reported gaps: added NPC dialogue, the
message after changing Guide, instance music, the main theme during loading,
and passive Nano ability speech. Current Retrobution 20260821 is `primary`.
No Academy or patched voice donor is used.

## Confirmed corrections

`cnOwnAvatarStatus.ReceiveNanoActive` passes the server's nonzero
`eCSTB___Add` to `NanoContainer.SetPassiveSkill`. The tune-card slot selects
Skill1/2/3, consumed after Call finishes. Native activation previously discarded
this flag. It now retains a presentation request for the acknowledged loadout,
queues equip/summon before that request, and uses the model's existing animation
events and localized voice pipeline. Recall, replacement by an active ability,
session reset and regeneration clear stale requests. No damage or cooldown is
invented by this presentation.

`cnGuideMode.ReceivePacket` invokes mission `ChangeGuide` after an established
Guide changes. `ChangeGuide` invokes `ReceiveStartGames(null)`, which chooses
the new Guide's mail/no-mail TableData text and type-9 NanoCom message. The
native successful, correlated reply now calls the existing localized producer;
malformed, uncorrelated and failed replies do not request this message. The
first-choice warp branch retains its distinct lifecycle.

`WorldDataContainer` selects music at (1,1), instance=false, while the world is
not ready. The native loading screen now requests that published main-theme
zone even without a player entity. Login/selection sources pause while loading
owns the music request, including sources that have not finished decoding.

`cnSystemMessageManager.ReceiveMOTD` types 11/12 and `CnGuiChat.ReceiveChat`
emote 666 select server-directed music. Native ingress previously treated these
as ordinary messages or discarded them. The native music channel now resolves
only declared catalog tracks and existing zone aliases, retains ambient zone
selection, and honors repeating versus one-shot selection. A non-repeating
override is consumed when the requested track is acknowledged, matching
`MusicController.LoadSoundByCoordinate`; the following scan resumes authored
zone/delay selection. The special server cue `vsweeper.ogg` resolves to the
semantic `music/vs_weeper` track. Its exact primary AssetBundle route points to
AudioClip 19, title `Vs Weeper`, in `CustomAssetBundle-RetroMusic`.

## Exact audio publication

Two independent raw exports produced identical output hashes for all 41 Ogg
payloads, without transcoding:

| Family | Files | Primary container |
| --- | ---: | --- |
| Fátima | 5 | DongResources_03_04.resourceFile |
| Noonja | 5 | DongResources_04_13.resourceFile |
| M_Barber, used by multiple Sweeper NPCs | 7 | Tutorial.resourceFile |
| Dustin, used by Magic Krab | 12 | World_shared_part4.resourceFile |
| Warp Frog | 9 | World_shared_part4.resourceFile |
| highlands, darklands beginnings | 2 | PastMusic.resourceFile |
| Vs Weeper | 1 | RetroMusic.resourceFile |

The maintained producer is `tools/legacy-sources/publish-audio-gaps.py` and its
reviewed plan is `recipes/native/audio/audio-gaps-20260908.plan.json`. Six
`audio-gaps-*-20260908.receipt.json` records retain raw container hashes, scoped
AudioClip identities, editable route rows and accepted output hashes.
Only Ogg payloads and native TableData audio routes are installed into FFOne.
Voice paths are locale-relative; existing EN/RU lookup and one-way EN fallback
remain authoritative. Raw evidence and both staging replays remain under
`work/cases/audio-gaps-20260908`.

Replay from FusionForge:

```powershell
python tools/legacy-sources/publish-audio-gaps.py --plan recipes/native/audio/audio-gaps-20260908.plan.json --source-root ../builds/retrobution-20260821 --native-target ../FFOneClient/assets/game --work work/cases/audio-gaps-20260908/replay
```

Compare `replay/outputs.json` to the receipts before adding `--apply`.
`--reuse-evidence --apply` additionally requires every output's pinned hash and
rechecks each raw container hash and scoped object identity.

## Verification and remaining questions

The focused native tests exercise the production EN/RU catalog, main-theme
selection without a player, instance/non-instance polygon choice, exact
server music aliases, one-shot acknowledgment, correlated passive activation,
Guide message selection, malformed music packets and instance packet layout.
The existing Nano/audio suite also passed all 54 tests. These checks use
deterministic ECS state and asset resolution, not an acoustic capture.

The real client executable is built separately under FFOne's ignored
`target/performance/audio-gaps` using existing Cargo development dependencies,
because other tasks hold the shared Cargo target lock. Build/test logs and the
verification script remain beside that executable. `--validate-assets` checks
the installed TableData routes. Final result hashes and verification outcomes
are recorded in `docs/reference/evidence/cases/audio-gaps-20260908.json`.

Unresolved scope is explicit:

- The primary navigation index has no matching greeting family for TOM, Kiva,
  YoungKevin, Clyde, Kimchi, VV Argost or bbasher. This is an unresolved source
  ownership question, not proof that any unrelated recording should be used.
- The existing music-zone contract still has unresolved Monster Island,
  Stormalong, Monster Island Cove and Absolution routes. No replacement track
  was guessed for them.
- The existing zone document predates current primary: current MusicDataStorage
  has an identical duplicate final index 135 and seven changed outdoor music
  references. This task adds the two corresponding audio assets for semantic
  resolution but does not republish that older, provenance-bearing zone document.
- No specific silent instance or live Unity/native acoustic recording was
  supplied. Complete audible parity across every instance is not claimed.
