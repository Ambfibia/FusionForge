# GM command coverage audit, 2026-09-08

Historical pre-implementation snapshot. See [the completion report](gm-command-completion-20260908.md)
for the subsequent 77/77 dispatch implementation, verification, and remaining topology limits.

Question: which primary client chat commands still lack a native dispatch path,
and can the current OpenFusion server service their original packets?

This is a source audit, not a live-server acceptance run. Runtime files were not
changed. Source authority and replay commands are recorded in
[gm-chat-commands-20260908.md](gm-chat-commands-20260908.md). The primary raw
container and managed assembly hashes were checked again and match that receipt.
The audited method is `CnGuiChat.CheckCheatKey`, its first, privileged switch.
The later non-GM suppression list is deliberately excluded: `/mapwarp`, `/emote`
and `/dance` appear there without executable cases in the privileged switch.

## Result

There are **77 command names**, counting aliases such as `/announce` and `/bcast`
separately. **13 have native dispatch; 64 do not**. This is not a claim that all
13 are fully accepted end to end. Native aliases `/taro`, `/fm`, `/hp`, `/item`
and the native `/fly` extension are outside the primary denominator.

Of the 64 missing dispatch names:

- **44** produce a packet with a registered handler in current OpenFusion.
- **8** produce a packet without a registered handler: `/motd`, `/groupsummon`,
  `/summonshiny`, `/mission`, `/task`, `/chnum`, `/chinfo`, `/chwarp`.
- **12** use local/event-driven behavior: `/rule`, `/nanoArr`, `/viewloc`,
  `/viewnetinfo`, `/hideui`, `/viewcol`, `/qinven`, `/shwarp`, `/viewid`, `/Store`,
  `/tasklog`, `/shardwarp`. Some open native UI that already exists, but there is
  no chat binding. Shard transitions additionally depend on server topology.

Packet declarations and the protocol ABI registry are not server implementations.
Registration was checked against `REGISTER_SHARD_PACKET` in OpenFusion source,
not the diagnostic packet-name list in `core/Packets.cpp`.

## Remaining implementation issues

1. The native chat route only intercepts cashmall, fly, speed, nano and the new
   value/goto/item module. Other slash text is sent as FreeChat. OpenFusion
   `CustomCommands::runCmd` does not implement the missing primary names and
   responds `Unknown command!`.
2. No native application consumers were found for `P_FE2CL_GM_REP_PC_ANNOUNCE`
   (`0x310000c8`), `P_FE2CL_GM_REP_PC_LOCATION` (`0x310000c7`) or
   `P_FE2CL_GM_REP_REWARD_RATE_SUCC` (`0x3100012c`). Closing `/announce`, `/locate_*`
   and `/rateT`/`/rateF` requires response handling/presentation as well as parsing.
3. `/item` and `/itemN` pick an empty slot from the last authoritative snapshot,
   without reserving it until the server replies. Two requests before the first
   response can therefore carry the same slot. OpenFusion's `itemGMGiveHandler`
   directly assigns `plr->Inven[itemreq->iSlotNum]`, so the second overwrites the
   first. This is a code-derived interleaving, not a live reproduction.
4. `/itemQ` has a related correlation gap: the server chooses an existing stack
   or empty slot using `Missions::findQSlot`, but echoes the request's slot in its
   success packet. Two different quest items sent from the same stale snapshot
   can leave the native quest inventory inconsistent with the server.
5. Invalid item IDs can be silently ignored by OpenFusion. Native feedback says
   only “Command sent”; there is no pending completion/failure correlation.

Priority: fix item request serialization/correlation, then common commands
(`/summon`, `/unsummon`, `/nanoskill`, `/nano_equip`, `/nano_unequip`,
`/nano_active`, `/warptopc`, `/unstick`, GM flags), then moderation and announcements
with their replies. The eight unregistered packet cases require a separate server
decision; merely adding their parser would leave them nonfunctional.

## Server-only commands

OpenFusion registers 34 custom chat names in `CustomCommands::init`. Their
transport exists through the native FreeChat slash-command route. Examples:
`/help`, `/access`, `/level`, `/buff`, `/summonGroup`, `/summonW`, `/unsummonW`,
`/instance`, `/refresh`, `/ban`, `/unban`, `/registerall`, `/redeem`.
This establishes dispatch, not complete visual/gameplay acceptance of every
server-only command. `/summonGroup` is case-sensitive and distinct from the
primary `/groupsummon`. `/help` enumerates server-only registrations and does not
provide a complete inventory of client-side GM commands.

`PC2PC_BLOCKED_CHAT_COMMANDS` in native `pc2pc_ui.rs` is a trade-chat suppression
list. Its command strings do not establish world-chat command implementations.

## Detailed matrix

`implemented` means a native chat dispatch exists; `missing` means it does not.
`registered` means a server packet handler exists; `unregistered` means none was
found; `local/event` means the primary branch uses local/event behavior.

| Command | Native dispatch | Primary packet | Server |
| --- | --- | --- | --- |
| `/rule` | missing | `` | local/event |
| `/motd` | missing | `P_CL2FE_GM_REQ_PC_MOTD_REGISTER` | unregistered |
| `/announce` | missing | `P_CL2FE_GM_REQ_PC_ANNOUNCE` | registered |
| `/bcast` | missing | `P_CL2FE_GM_REQ_PC_ANNOUNCE` | registered |
| `/nano_equip` | missing | `P_CL2FE_REQ_NANO_EQUIP` | registered |
| `/nano_unequip` | missing | `P_CL2FE_REQ_NANO_UNEQUIP` | registered |
| `/nano_active` | missing | `P_CL2FE_REQ_NANO_ACTIVE` | registered |
| `/speed` | implemented | `P_CL2FE_GM_REQ_PC_SET_VALUE` | registered |
| `/jump` | implemented | `P_CL2FE_GM_REQ_PC_SET_VALUE` | registered |
| `/warp` | implemented | `P_CL2FE_REQ_PC_GOTO` | registered |
| `/goto` | implemented | `P_CL2FE_REQ_PC_GOTO` | registered |
| `/warptopc` | missing | `P_CL2FE_REQ_PC_WARP_TO_PC` | registered |
| `/itemN` | implemented | `P_CL2FE_REQ_PC_GIVE_ITEM` | registered |
| `/itemQ` | implemented | `P_CL2FE_REQ_PC_GIVE_ITEM` | registered |
| `/nano` | implemented | `P_CL2FE_REQ_PC_GIVE_NANO` | registered |
| `/nanoArr` | missing | `` | local/event |
| `/summon` | missing | `P_CL2FE_REQ_NPC_SUMMON` | registered |
| `/groupsummon` | missing | `P_CL2FE_REQ_NPC_GROUP_SUMMON` | unregistered |
| `/summonshiny` | missing | `P_CL2FE_REQ_SHINY_SUMMON` | unregistered |
| `/unsummon` | missing | `P_CL2FE_REQ_NPC_UNSUMMON` | registered |
| `/nanoskill` | missing | `P_CL2FE_REQ_PC_GIVE_NANO_SKILL` | registered |
| `/mission` | missing | `P_CL2FE_REQ_PC_MISSION_COMPLETE` | unregistered |
| `/task` | missing | `P_CL2FE_REQ_PC_TASK_COMPLETE` | unregistered |
| `/unstick_n` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/unstick_i` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/unstick_ui` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/unstick` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/locate_i` | missing | `P_CL2FE_GM_REQ_PC_LOCATION` | registered |
| `/locate_ui` | missing | `P_CL2FE_GM_REQ_PC_LOCATION` | registered |
| `/locate_n` | missing | `P_CL2FE_GM_REQ_PC_LOCATION` | registered |
| `/teleport2me_n` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleport2me_i` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleport2me_ui` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleportXYZ_i` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleportXYZ_ui` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleportXYZ_n` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleportMapXYZ_i` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleportMapXYZ_n` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleportMapXYZ_ui` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleport_i_i` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleport_ui_ui` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleport_i_n` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/teleport_n_n` | missing | `P_CL2FE_GM_REQ_TARGET_PC_TELEPORT` | registered |
| `/kick_i` | missing | `P_CL2FE_GM_REQ_KICK_PLAYER` | registered |
| `/kick_ui` | missing | `P_CL2FE_GM_REQ_KICK_PLAYER` | registered |
| `/kick_n` | missing | `P_CL2FE_GM_REQ_KICK_PLAYER` | registered |
| `/invisible` | missing | `P_CL2FE_GM_REQ_PC_SPECIAL_STATE_SWITCH` | registered |
| `/invulnerable` | missing | `P_CL2FE_GM_REQ_PC_SPECIAL_STATE_SWITCH` | registered |
| `/health` | implemented | `P_CL2FE_GM_REQ_PC_SET_VALUE` | registered |
| `/batteryW` | implemented | `P_CL2FE_GM_REQ_PC_SET_VALUE` | registered |
| `/batteryN` | implemented | `P_CL2FE_GM_REQ_PC_SET_VALUE` | registered |
| `/fusionmatter` | implemented | `P_CL2FE_GM_REQ_PC_SET_VALUE` | registered |
| `/taros` | implemented | `P_CL2FE_GM_REQ_PC_SET_VALUE` | registered |
| `/gmmarker` | missing | `P_CL2FE_GM_REQ_PC_SPECIAL_STATE_SWITCH` | registered |
| `/equipitem` | missing | `P_CL2FE_REQ_ITEM_MOVE` | registered |
| `/viewloc` | missing | `` | local/event |
| `/viewnetinfo` | missing | `` | local/event |
| `/mute_i_on` | missing | `P_CL2FE_GM_REQ_TARGET_PC_SPECIAL_STATE_ONOFF` | registered |
| `/mute_i_off` | missing | `P_CL2FE_GM_REQ_TARGET_PC_SPECIAL_STATE_ONOFF` | registered |
| `/mute_ui_on` | missing | `P_CL2FE_GM_REQ_TARGET_PC_SPECIAL_STATE_ONOFF` | registered |
| `/mute_ui_off` | missing | `P_CL2FE_GM_REQ_TARGET_PC_SPECIAL_STATE_ONOFF` | registered |
| `/mute_n_on` | missing | `P_CL2FE_GM_REQ_TARGET_PC_SPECIAL_STATE_ONOFF` | registered |
| `/mute_n_off` | missing | `P_CL2FE_GM_REQ_TARGET_PC_SPECIAL_STATE_ONOFF` | registered |
| `/hideui` | missing | `` | local/event |
| `/viewcol` | missing | `` | local/event |
| `/qinven` | missing | `` | local/event |
| `/chnum` | missing | `P_CL2FE_REQ_PC_CHANNEL_NUM` | unregistered |
| `/chinfo` | missing | `P_CL2FE_REQ_CHANNEL_INFO` | unregistered |
| `/chwarp` | missing | `P_CL2FE_REQ_PC_WARP_CHANNEL` | unregistered |
| `/shwarp` | missing | `` | local/event |
| `/viewid` | missing | `` | local/event |
| `/cashmall` | implemented | `` | local/event |
| `/Store` | missing | `` | local/event |
| `/rateT` | missing | `P_CL2FE_GM_REQ_REWARD_RATE` | registered |
| `/rateF` | missing | `P_CL2FE_GM_REQ_REWARD_RATE` | registered |
| `/tasklog` | missing | `` | local/event |
| `/shardwarp` | missing | `` | local/event |

## Input snapshot hashes

The disposable working matrix and input manifest are under
`work/cases/gm-command-audit-20260908`. The following hashes identify this audit snapshot:

| File | SHA-256 |
| --- | --- |
| `work/legacy-sources/gm-chat-commands-primary/decompiled/CnGuiChat.cs` | `23E17B181947608A01769703F017A60436A1C0023C526FE76B8149EB29F47A39` |
| `../FFOneClient/crates/ffone-client/src/app/gameplay_ui_actions/chat.rs` | `39C0C36BBE08F147E591F0BD6364E0C2816D268E8983840BC25BFB2CE5A76840` |
| `../FFOneClient/crates/ffone-client/src/app/gameplay_ui_actions/gm_commands.rs` | `9D360D4AD80E50FB743624859B62E76FEFB810A900A43DBA6A064F47D610E659` |
| `../OpenFusion/src/BuiltinCommands.cpp` | `D51408776A671D5C3D80A94C4078B75F817C14FBED8C1797CA6A60301250734F` |
| `../OpenFusion/src/CustomCommands.cpp` | `7526989E4B82DB46CCEBC8A2521CA64495AC2B008BD8E70503B5CD48959315B8` |
