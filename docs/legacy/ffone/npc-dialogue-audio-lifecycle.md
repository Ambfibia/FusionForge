# NPC dialogue audio lifecycle correction (2026-09-04)

## Evidence question

Determine who owns NPC dialogue voice playback, whether a later line replaces
the current line, whether ordinary close requests farewell, how warp/journal
returns differ, and whether the source follows the NPC. Native consumer:
FFOne `GameplayAudioRuntime` and NPC dialogue action handoffs. No binary assets
are republished. No alternate or patched donor establishes this behavior.

## Primary evidence

Reused strict export:
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/assembly-csharp.evidence.json`.

- Primary `builds/retrobution-20260821/main.unity3d`: 8,221,718 bytes,
  SHA-256 `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
- Exact level-0 `Assembly - CSharp.dll`: 1,762,816 bytes,
  SHA-256 `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- Both raw container and exported assembly hashes were rechecked with
  `Get-FileHash -Algorithm SHA256` on 2026-09-04.
- Navigation `decompiled/NpcMoveController.cs` beside the export has SHA-256
  `6B3A249BF2CFD52BA17D099361033FF408A6A04AFD39EE2547DCA36E00A7F4A8`.

`AvatarUtil.CallVoicePlay` dispatches an NPC-owned request to
`NpcMoveController.PlayVO`. Its coroutine stops the existing component AudioSource,
assigns the new clip, sets rolloffFactor 0.1 and voice gain, then plays on the
same NPC GameObject. It does not spawn an independent unowned UI one-shot.
The coroutine replaces the current source only after the requested clip loads;
a null result leaves the current source untouched.

`NpcIconMode.EndMode` (790–819) first rejects `ModeChange` lock, then restores
the camera/cursor and sends event 2/1. If the target owns NpcMoveController and
`bWarpFlag` is false, it requests `CloseNpcUI`. `AvatarUtil.CallVoicePlay`
maps that cue to the base owner plus `_farewell01` through `_farewell03`
(exclusive integer Random.Range upper bound 4). `ReceivePacket` sets
`bWarpFlag` after successful warp and requests `WarpOK` instead (1095–1104).
`cnMissionJournal.EndModeToTarget` returns to the existing NPC with init flag
false; it does not call NpcIconMode.EndMode or request farewell.

Additional navigation artifact SHA-256 values, rechecked 2026-09-04:

- `NpcIconMode.cs`: `60489BFD4E2F97CBC32C4FBF62B1A914A79251F390B0F7A6C82D0151A394FEE5`.
- `AvatarUtil.cs`: `4B62D5528780486D113512FE6086C047F5BC6F4A545F9B4CB456A7255704625A`.
- `cnMissionJournal.cs`: `FA474EE046628F211F2135A17FD5EDF2055723DBE238AF0D24D65878586153DB`.

Replay from FusionForge using the pinned FusionForge/FFSpy toolchain:

```powershell
cargo run -- fusionforge export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/assembly-csharp.evidence.json --payload-out work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/Assembly-CSharp.dll
dotnet run --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/Assembly-CSharp.dll -o work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/decompiled -p --preserve-iterator-state-machines -r work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary
```

## Native contract (farewell correction supersedes silent-close interpretation)

- Dialogue voice is one replaceable source per NPC. Ordinary close requests
  farewell; it cancels obsolete queued requests and replaces the active source
  only once the new clip is ready. Missing/failed clips leave the previous line
  untouched, matching PlayVO. A same-frame stale greeting cannot overwrite
  farewell. Explicit cancellation remains available for teardown.
- A confirmed warp uses its separate WarpOK/confirm path without NpcIconClose.
  Repeated close and close during pending warp cannot emit farewell. Tutorial
  ModeChange lock prevents close even when the Mouse gate permits the button.
- World character audio is a child of its owner with identity local transform.
  Bevy transform propagation precedes audio playback, so first-frame and moving
  emitter positions follow the actual NPC/nano/mob rather than a frozen point.
- Semantic catalog lookup, independent VoiceLanguage, and LocalizedVoice remain
  mandatory. No locale path is constructed by dialogue code.
- The previous task interpreted closing as silent cancellation. The owner's
  follow-up explicitly rejected that interpretation; ordinary farewell is now
  restored to the primary contract. Returning from a mission page to the NPC
  neither stops its current line nor emits farewell.
- Tutorial's separate NPC-close action stops its active tutorial voice/subtitle
  and auxiliary dialogue timeline, then requests farewell on that exact actor.
  Radio and cinematic ownership is otherwise
  unchanged; those speakers are not guessed from the nearest world NPC.
- UI hit-tested pointer input is excluded from world avatar action input;
  stale TalkNpc requests cannot reopen an already active mission/NPC modal.

## Acceptance

New FFOne tests exercise production Dexter voice lookup in EN/RU, pending-load
retention, greeting-to-farewell replacement, cancellation before asset load,
missing-clip behavior, source movement across the listener, ownership/despawn,
UI-pointer exclusion, journal return, warp and ModeChange gates. These are deterministic
ECS checks, not an acoustic or GPU capture. No change to UI geometry/text/assets
is involved. Test/build results are recorded in the task handoff; manual in-game
listening remains a separate acceptance check.

Before the farewell follow-up, on 2026-09-04, the production-catalog voice lifecycle test passed for both
locales, the pointer-input regression passed, and the existing animation-sound
routing and ordinary-world Nano event tests passed. `git diff --check` passed
for the touched runtime files. The full development build and tutorial binary
test were not accepted: concurrent workspace builds/edits occupied the shared
Cargo target after earlier unrelated compilation failures in Nano Station and
character creation. No claim of a rebuilt executable or in-game acoustic
acceptance is made by this record.

Follow-up acceptance on 2026-09-04:

- `cargo test -p ffone-client --lib npc_dialogue_voice_replaces_stops_and_follows_owner_in_both_locales -- --nocapture`
  passed. The first test run exposed a fixture assumption that a replaced clip
  remains cached forever; the test now completes a fresh load after reopening
  when Bevy has released that clip. Loader preregistration makes the injected
  asset-ready boundary deterministic without an audio device.
- Six additional filters passed on the newly built library test executable:
  `npc_close_respects_end_mode_lock_before_requesting_farewell`,
  `linked_npc_mode_blocks_gameplay_and_keeps_one_subtarget_through_journal`,
  `warp_and_npc_close_emit_original_tutorial_event_semantics`,
  `npc_dialogue_pointer_press_cannot_also_become_world_talk_input`,
  `animation_sound_payloads_expand_all_inclusive_random_tokens_and_route_nano_voice`,
  `ordinary_world_nano_replays_its_own_glb_voice_event_once`.
- `git diff --check` passed for the changed runtime files.
- The shared workspace's development executable was rebuilt during the task
  (2026-09-04 03:13:02 local, 205,031,936 bytes,
  SHA-256 `8121C22C0E486DBCFF5E17F276EBE661DB75A8926E50A3E7F6E2A675C69FAB9D`).
  Running `target/debug/ffone-client.exe --validate-assets` returned status `ok`
  for the audio, character and map catalogs. This task did not own the concurrent
  command that produced that executable.
- The standalone tutorial binary test and in-game acoustic capture were not
  run for this follow-up. Catalog validation does not exercise playback, and
  ECS acceptance must not be described as complete audible 1:1 parity.
