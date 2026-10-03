# Escort mission network contract (2026-09-10)

Evidence question: which client requests cause an accepted escort NPC to join
the player, and who owns its translation? Native consumers are FFOne's mission
request codec, automatic task-chain sender and ordinary-world mission systems.

## Primary authority

Reused the strict managed export in
`work/legacy-sources/gm-chat-commands-primary/reports/assembly-csharp.evidence.json`.
Rechecked the raw container and materialized assembly with `Get-FileHash`:

- Source role: `primary`, `../builds/retrobution-20260821/main.unity3d`,
  8,221,718 bytes; SHA-256
  `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
- Exact UnityWeb level 0 entry `Assembly - CSharp.dll`, 1,762,816 bytes;
  SHA-256 `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- `decompiled/cnMissionManager.cs`:
  `BD40E292DD98DE45881C530AAF25E10FE9BFC08C9AE805AED1F744B94A9277C0`.
- `decompiled/NpcMoveController.cs`:
  `6B3A249BF2CFD52BA17D099361033FF408A6A04AFD39EE2547DCA36E00A7F4A8`.
- `decompiled/NpcContainer.cs`:
  `2C6EE299D528673588170566262F847255202ACB1FD617910CABD75906960EAB`.
- `decompiled/cnGroupManager.cs`:
  `0E87A2983BBD5A9E36B06234E95BE0082FFD78267F7AE6955450AB693F5F0E12`.

Replay the exact assembly export and pinned FFSpy commands in
[NPC dialogue lifecycle](npc-dialogue-audio-lifecycle.md), using an Editor
`work/cases/escort-missions` output directory. No patched or alternate donor,
binary publication, or UI geometry change is involved.

## Proven behavior

- `cnMissionManager.RequestTaskStart` resolves `m_iCSUDEFNPCID` for type 6
  and writes the live `Status.iID` into `iEscortNPC_ID` (offset 8). The
  table type is not the network entity ID. This also applies to outgoing tasks.
- `ProcessStartSucc` invites the nearby escort with
  `P_CL2FE_REQ_NPC_GROUP_INVITE` (`0x13000084`, four-byte NPC ID).
- `Update` refreshes once per second. If the authoritative group reports zero
  NPC members, it repeats the invite for each active type-6 task's nearby NPC.
  A late NPC appearance can therefore recover without reaccepting the task.
- `ProcessEndSucc` and terminal `ProcessEndFail` send
  `P_CL2FE_REQ_NPC_GROUP_KICK` (`0x13000085`, four-byte NPC ID).
- `RequestTaskComplete` carries the escort's live ID at offset 12. Reward
  choice bytes and pack(4) padding retain their existing positions.
- `NpcMoveController.ReceiveMove` consumes server `NPC_MOVE`, converts speed
  and coordinates, and chooses walking for move style 0. This is not a
  client-authored follow path or a teleport-to-player mechanic.

## Native correction and limits

FFOne previously encoded zero for explicit task start/end escort IDs, also
encoded zero for automatic outgoing starts, and never sent escort group
invite/kick requests. The correction resolves IDs at the send boundary and
adds mission-owned invites, one-second retries, roster acknowledgement gating,
and release when a server reply retires the task, before sending its outgoing
start request. Session epochs discard stale owners without
sending an old-session kick. A temporarily invisible owner remains tracked;
it is not invited again until visible. Task cancellation cleanup is an explicit
native lifecycle extension. Reward-page requests use the declared escort owner;
the primary journal instead mistakenly looks up the terminator in that field.
This is an explicit native correction, not a claim of byte-for-byte journal
behavior. Existing server-owned movement is retained.

The typed network methods are separate from the stock OpenFusion registered
handler inventory: the paired local `Groups.cpp` does not register NPC group
invite/kick. Do not falsely add those IDs to that inventory or claim this
client correction implements server escort AI.

The owner-supplied tabledata revision
[`fed031b972c5876bf2e04c03bae634e8c4aa104e`](https://github.com/OpenFusionProject/tabledata/tree/fed031b972c5876bf2e04c03bae634e8c4aa104e)
contains 101 entries under `paths.json.npc`, but no positive `iTaskID`.
The paired local `tdata/paths.json` and `bin/tdata/paths.json` contain seven
NPC routes and no positive `iTaskID` either. Thus these files alone do not
provide an Eduardo escort path. No server data was replaced.

## Verification

Passed on 2026-09-10:

```powershell
# From FFOneClient
rustc --edition 2024 --test crates/ffone-client/src/app/mission_escort/state.rs -o target/tests/escort-state.exe
target/tests/escort-state.exe
cargo check -p ffone-client --bin ffone-client
cargo test -p ffone-net escort -- --nocapture
cargo test -p ffone-client --lib escort -- --nocapture
cargo build -p ffone-client --bin ffone-client --locked
target/debug/deps/ffone_client-735e7cd1cecfebb2.exe entity_lifecycle::tests::npc_enter_new_and_move_require_appearance_and_publish_typed_motion --exact --nocapture
target/debug/deps/ffone_client-735e7cd1cecfebb2.exe network_world_runtime::tests::npc_motion_reaches_horizontal_destination_without_using_packet_y --exact --nocapture
```

Results: two request-state tests, one encrypted sender test, three escort
catalog/codec tests and two existing NPC movement tests passed. The full
development executable was rebuilt. Cargo commands queued behind concurrent
workspace builds. The sender test was corrected to decode packet sequence 1
for the second packet, rather than incorrectly reusing sequence 0.

- Direct Rust tests of the production escort request state cover immediate
  invite, one-second retry, acknowledgement, temporary disappearance, task
  retirement, late appearance and session replacement.
- Added production-catalog coverage for every type-6 task with a declared
  escort, including Eduardo task 576 / NPC type 1007.
- Added start/end wire decoding assertions and an encrypted TCP sender test
  for both NPC group request IDs and four-byte bodies.
- A live server mission playthrough is still required for end-to-end parity;
  the local shard's missing NPC-group handlers remain a separate limitation.
