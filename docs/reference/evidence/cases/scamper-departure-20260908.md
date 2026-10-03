# SCAMPER departure: ClickWarp versus ClickMove

Question: can the existing Russian MKNDTransport1/2_ClickWarp02/03 clips
be treated as the original SCAMPER departure voices? Native consumer:
`transportation_move_ok_voice_set` and `PlayNpcMoveOkVoice` in FFOneClient.

## Primary evidence

Current primary is retrobution-20260821. Re-exported the exact
`Assembly - CSharp.dll` entry of main.unity3d with
`fusionforge export-managed-assembly-evidence`, then decompiled AvatarUtil
and cnTrans using the repository's FFSpy console binary. The assembly evidence,
payload and decompiled classes are in `work/cases/scamper-departure-20260908`.
Assembly SHA-256: 0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792.

- cnTrans's accepted travel path calls AvatarUtil.CallVoicePlay with the NPC's
  m_strComment2 owner and eVoice.MoveOK.
- AvatarUtil maps MoveOK to owner + `_clickmove0` + Random.Range(1, 4).
- WarpOK independently maps to owner + `_clickwarp0` + Random.Range(1, 4).
- The native table uses M_KNDTransport1 and M_KNDTransport2 as SCAMPER owners.
  Its six departure routes correctly retain ClickMove01/02/03.
- The native spawn resolves through NativeAudioCatalog with VoiceLanguage and
  carries LocalizedVoice. No runtime routing change is needed for a translation
  placed at its declared relative path.

Raw AudioClip evidence confirms distinct identities, not alternate route spellings:

| Clip | Raw container | Serialized asset | PathID |
| --- | --- | --- | --- |
| M_KNDTransport1_ClickMove02 | Tutorial.resourceFile | CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a | 1497 |
| MKNDTransport2_ClickWarp02 | DongResources_06_10.resourceFile | CustomAssetBundle-fa3d425d6dbc24cee8ec6c3ffda4bef2 | 300 |

Both were exported with strict `dump-object-evidence --type AudioClip
--serialized-asset ...`. The companion JSON records raw container and object
hashes. Encoded audio payload hashes differ; reported durations are 1.55 s and
1.27666664 s. This does not establish a translation or take equivalence.
DongResources' m_kndtransport1 routes reference an external serialized asset;
PathID 1497 there is an Animation, not the AudioClip. The strict selector rejected
that candidate; the accepted AudioClip is scoped to Tutorial above.

## Existing Russian files and accepted extension

Both male owner folders contain ClickWarp02 and ClickWarp03, but no ClickWarp01.
The two owners' 02 files have equal SHA-256, as do their 03 files. These are two
distinct recordings copied under two owner names, not four distinct recordings.
The adjacent JSON records all four native file hashes. Their speech content has
not been transcribed or matched to the primary English lines.

`recipes/native/audio/scamper-departure-20260908.plan.json` is the accepted
localization extension. The owner explicitly approved it for both male voices
on 2026-09-08 after the separate primary contracts and missing take 01 were
explained. It maps exactly
ClickWarp02 -> ClickMove02 and ClickWarp03 -> ClickMove03 for each male owner,
preserving bytes and take numbers. It makes no primary-parity claim. Take 01
continues to use EN fallback, with no take substitution or random selection change.

Replay from FFClientEditor:

```powershell
./tools/native/repair-scamper-pilot-paths.ps1 -TargetRoot ../FFOneClient/assets/game -RecipePath recipes/native/audio/scamper-departure-20260908.plan.json
```

The repair tool verifies declared SHA-256 values before renaming and supports
idempotent verification. The four renames were replayed in Editor staging before
applying to the explicit native target, and all staged/native hashes matched.
No audio bytes or runtime code changed. Raw lineage of the pre-existing Russian
recordings is not established by this path repair; the accepted mapping is the
owner's localization extension, not a recovered primary translation contract.

The production `scamper_pilot_voice` regression was compiled against the current
prebuilt foundation/Bevy dependencies. Before publication it failed on RU
ClickMove02 resolving to EN. After publication it passed for both owners and all
three departure takes in EN/RU, exact fallback of 01 and audible decoding.
The same test also revalidated greetings and farewells (32 locale/cue cases total).
No live in-game playback or new client build is claimed. Restarting the client
is necessary to rebuild its in-memory locale catalog.
