# GM command implementation, 2026-09-08

Follow-up to [the read-only coverage audit](gm-command-coverage-20260908.md).
The earlier 13/77 count describes the pre-implementation snapshot. The paired
native/client-server changes now provide dispatch for **77/77 primary names**,
including the 64 previously missing entries. This count measures executable
entry paths, not pixel-identical reproduction of every legacy debug screen.

## Authority and boundaries

Primary chat authority remains `retrobution` / role `primary`, source
`retrobution-20260821/main.unity3d`, SHA-256
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
Use the exact original receipt in
[gm-chat-commands-20260908.md](gm-chat-commands-20260908.md) for provenance and
replay; cached source/receipt remain below Editor `work/legacy-sources/gm-chat-commands-primary`.
The audited executable branches are `CnGuiChat.CheckCheatKey`; Nano batching
also consults `GameFrame`'s level-up coroutine and acquisition/tuning handling.
No extraction tool or legacy intermediate was added to FFOneClient.

The current paired OpenFusion base was `959516cc013d25aeb2a00700f22575eabb5e10e5`.
Server extensions live in OpenFusion, not in runtime asset data. Native command
syntax and operational limits are documented in FFOneClient `docs/admin-commands.md`.

## Implementation

- Typed parser for target IDs, 64-bit UIDs and names; teleport coordinate
  conversion and field ownership are explicit. The erroneous primary goal-name
  assignment in `/teleport_i_n` is corrected as an intentional native extension.
- Local entry into Rule and UserStore; Nano batching, quest/task diagnostics,
  HUD visibility, position/ID/network overlays and bounded packet history.
- GM location, timed announcement, reward-rate, mission and channel replies
  have native consumers. Mission completion updates the local mission flags.
- Item commands serialize until committed reply. Server free-slot fallback
  prevents overwrites by old/stale clients; quest replies carry the actual
  server-selected slot and granted delta. Invalid requests produce failure.
- `/nanoskill` now uses its four-byte GM body and checks GM access; the old
  handler incorrectly cast that body to a longer ordinary tuning request.
- Fifteen registered server additions: eight missing GM packets plus the seven
  street-stall transaction requests. Client registry and server registration
  packet-ID sets both contain 131 entries and match exactly.

## Explicit extensions / practical limits

- Single shard/channel: number 1 is already current; other numbers fail.
  `/shwarp` and `/shardwarp` share that native validation, rather than claiming
  a multi-shard reconnect/handoff that this server does not provide.
- `/nanoArr` explicitly sets the greater target level and grants IDs old-level+1
  through target, with their first declared skills. Individual Nano grants on
  this server correctly do not infer character level from Nano identity.
- GM store entry needs no consumable. `/Store <PC-ID>` browses another nearby
  GM stall. Five listing slots and five-percent seller tax match the existing
  UI. Non-GM inventory/player-menu store entry remains independently gated.
  Offers retain inventory until purchase; purchase revalidates full item value,
  proximity, money, destination slot and active-trade exclusion. Offers are
  removed on disconnect, so there is no crash-prone item escrow.
- Debug overlays are native presentation. Collider view draws nearby world
  bounds; NPC IDs are a diagnostic list. Announcement types share one native
  overlay style. No claim of primary debug/popup visual parity is made.
- MOTD text/type persist in the running server only, until restart.

## Verification

- Protocol crate: 107 tests pass, including all registered ABI sizes/counts.
- Command standalone suite: 9 tests pass; every extended name has a valid
  example, access/no-player checks, and representative exact field assertions.
  Item queue tests cover delayed/mismatched/malformed replies and timeout lock.
- Store standalone suite: 29 tests pass (reducers, UI ownership, EN/RU keys and
  placeholders). Native store UI GPU captures in EN and RU pass 99 key-first
  text checks; these are existing synthetic preview fixtures with deliberately
  unresolved sample icons, not live player inventories.
- OpenFusion `tests/gm_commands.cpp`, run via `tests/run-gm-commands.ps1`, uses
  real encrypted loopback replies and synthetic players. It tests stale slots,
  quest stack/overflow, invalid/access failures, channel responses, Nano access,
  and a two-player shop transaction, including stale item, active trade and
  repeated-purchase rejection. It opens no production DB/account.
- Standard optimized native client build succeeds after quarantining one bad
  incremental library cache; `--validate-assets` returns `status: ok`.
- Full-client offline world capture completes without system-parameter or
  schedule errors. This is a smoke check, not a before/after FPS claim.

Ignored build logs and captures: FFOneClient `target/performance/gm-commands`
and OpenFusion `work/`. The running game server was not restarted or replaced.
