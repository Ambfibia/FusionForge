# Mission dialogue, selection acknowledgements and capped Fusion Matter

Evidence question: does an NPC request completion voice for every successful
task, which text reaches quest balloons/chat, and does a full level-4 Fusion
Matter meter prohibit a task hand-in? Native consumers are the mission reducer,
NPC speech queue and production network-frame handler. No geometry or binary
asset changes are involved.

## Primary authority

Reused the exact assembly export and producer commands in
[NPC dialogue audio lifecycle](npc-dialogue-audio-lifecycle.md). On 2026-09-08,
rechecked SHA-256 from FusionForge using `Get-FileHash -Algorithm SHA256`:

- `../builds/retrobution-20260821/main.unity3d`:
  `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
- `work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/Assembly-CSharp.dll`:
  `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- `decompiled/cnMissionManager.cs` alongside that assembly:
  `BD40E292DD98DE45881C530AAF25E10FE9BFC08C9AE805AED1F744B94A9277C0`.
- `decompiled/NpcContainer.cs`:
  `2C6EE299D528673588170566262F847255202ACB1FD617910CABD75906960EAB`.
- `decompiled/NpcIconMode.cs`:
  `60489BFD4E2F97CBC32C4FBF62B1A914A79251F390B0F7A6C82D0151A394FEE5`.
- `decompiled/cnMissionJournal.cs`:
  `FA474EE046628F211F2135A17FD5EDF2055723DBE238AF0D24D65878586153DB`.

The role is primary. No patched or alternate donor establishes these behaviors.

## Recovered behavior

`cnMissionManager.SetBubbleChat` (1745–1828) uses the Start, Success/Complete,
and Fail dialog-bubble string/NPC fields independently of NanoCom flags.
Strings longer than one character use the explicitly declared nearby speaker.
`GetNearList` passes 5000 directly in client coordinates; NpcContainer adds
the converted actor radius or half-height (whichever is larger). A missing explicit bubble speaker returns
before generic voice. Start voice belongs to the task giver (escort has its
own cue); only Complete requests QuestEnd voice from the terminator. Success
and Fail do not request that generic completion voice.

`ProcessEndSucc` (2209–2276) classifies Complete using IsFinalTask or a zero
success outgoing ID. Other successful steps use Success. The previous native
network handler requested QuestCompleted for every correlated END, bypassing
this distinction, and chose the speaker from whichever NPC UI was pending.
The native table reader omitted all three dialog-bubble fields entirely.

`ProcessEndFail` (2287–2338) treats error 13 as SystemMessage row 12. The local
OpenFusion `Missions.cpp::giveMissionReward` sends this error when a reward
cannot fit in inventory. The native handler unlocked retry but omitted that
message. The repair uses the existing EN/RU row-12 semantic key and source
button type; it does not invent a Fusion Matter error.

## Explicit native robustness corrections

- Retain a selected mission while its outgoing START is pending, even if an
  unrelated server-initiated START arrives in the gap between END and START.
- A correlated SET_CURRENT acknowledgement clears its pending request after
  the selected mission has ended. It never replaces a newer local choice.
  These are asynchronous native protocol corrections, not claims that the
  original Unity client handled those races identically.
- Drain accepted dialogue events through the existing NPC speech owner so
  chat and balloon preferences remain independent. No portrait is required.

## Level-4 investigation limits

No full-meter completion gate was found in native `complete_mission`, mission
request dispatch, `can_complete_task`, or the reward/END reducers. OpenFusion's
level-4 unpaid check is in `updateFusionMatter` and stops Nano-quest generation;
it is not an early return from `endTask`. Inventory exhaustion is a separate
proven refusal and must be visible. The user's original exact mission and
packet sequence were unavailable; a full-meter regression can validate the
client path but cannot establish which server event occurred in that session.

Acceptance uses production content, EN/RU bundles, deterministic ECS speech,
delayed protocol acknowledgements, and production reward/END frame replay.
No new legacy GPU or acoustic capture is claimed. Results are recorded below
after execution.

## Executed checks

`cargo test -p ffone-client --lib world_mission_runtime::tests -- --test-threads=1`
passed all 27 tests, including both new acknowledgement/interleaving regressions.
The freshly built library-test executable also passed all 50 `mission_ui::tests`,
`gameplay_ui::tests::mission_dialogue_publishes_localized_chat_once_without_balloon_or_portrait`,
and `localization::tests::russian_bundle_covers_the_canonical_bundle_with_matching_placeholders`.
The speech test uses the production bubble update and verifies one chat event
with balloons disabled, plus no repeated event on the following update.

Independent read-only table/bundle audit found 159 valid Start and 1059 valid
Success dialogue references; all have EN/RU keys. All 66823 bundle keys and
template placeholder sets match. Task 2454 references out-of-range string 21822
in a 16050-row string table; the loader preserves the source bounds check and
does not invent a replacement or publish an invalid key.

`cargo test -p ffone-client --bin ffone-client app::mission_dialogue::tests -- --test-threads=1`
passed all three production-app regressions: terminal-only voice and table
ownership, inventory-full localized popup without a nearby NPC/player, and
level-4 capped-FM reward/END replay after error-13 retry. The latter installs
the actual task-451 item reward in inventory and checks cash, unchanged full
FM, cleared pending UI, and authoritative completed history.
The newly built app-test executable also passed
`app::tests::social::npc_chat_receives_localized_speech_once_with_independent_filters_and_history_cap`.
Total executed relevant checks: 83 passed, zero failed.

The rebuilt development executable available at 21:51 Moscow was copied to
FFOne's ignored `target/performance/mission-dialogue-20260908/ffone-client-probe.exe`
to avoid locking the shared build output while other tasks compile. It passed
`--validate-assets` (exit 0, status ok) and the full offline `FFONE_PERF_OUTPUT`
fixture with `FFONE_PERF_NPCS=1` (exit 0). The 1920×1080 `frame.png` was visually
reviewed: world, player and HUD rendered. This exercises the production World
schedule with the new dialogue consumer installed; the screenshot is not a
specific quest dialogue or a live server reproduction. Fixture logs retain
existing unresolved NPC-asset warnings; no new schedule panic occurred.

Probe executable SHA-256:
`1879AE549ECCB384A27C0C769F0BC29AFC3FD084D885353DC871297F452AA3D3`.
Reviewed frame SHA-256:
`7E56B84D1821D5187CD0A0FCF3746976E4CCF9E2AC473F851BD454CD40F854C2`.

The final ordinary incremental relink failed with missing internal LLVM
symbols in library objects. A process-local `CARGO_INCREMENTAL=0 cargo build
-p ffone-client --bin ffone-client` regenerated the objects and succeeded in
4m 41s; no project build settings were changed. Final development executable:
2026-09-08 22:01:32 Moscow, 211002880 bytes, SHA-256
`55F6623145DFAC3254C37195987C2602A05535FF67FC0F0231FEA38B57B3EC2E`.
Its `--validate-assets` run also returned exit 0 and status ok. The GPU capture
above remains explicitly associated with the earlier executable hash.
`cargo fmt --all --check` and the touched-file `git diff --check` passed.
