# NPC greeting and barker bubble evidence

This ledger covers the native reconstruction of `PrintName`'s green NPC
`Barker` bubble. The runtime contract is clean-primary parity; the patched
cache was used only to navigate objects already owned by the primary bundle.

## Source identity

| Role | Source | Size | SHA-256 |
|---|---|---:|---|
| primary bundle | `builds/retrobution-20260613/main.unity3d` | 7,000,415 | `59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F` |
| primary managed owner | `Assembly - CSharp.dll` in `main.unity3d` | 1,517,568 | `33D6F70216B1C7BA05BCC0F270FBA97E767B129159755AF4C8835922E60ACADB` |
| primary UI transform owner | `Assembly - CSharp - first pass.dll` in `main.unity3d` | 340,992 | `D5849A0B866DE92740AF11DB620D1687C54C38A5554DAEC23BF4AEB3FC2AC5F3` |
| navigation cache | `retrobution-20260613.ffclient/cache/extracted-bundles/ed793e024e70bdfc/sharedassets0.assets` | offline only | mapped to the primary bundle above |

Managed code was recovered with ILSpy `11.0.0.9375`. The focused object
dumps were produced with `fusionforge.exe fusionforge dump-objects` after
`fusionforge list-contents` identified the exact path IDs. Acceptance-critical
textures were decoded from those objects and checked against their primary
container ownership; neither the cache nor the legacy build is read at
runtime.

The serialized owner was also followed end-to-end in the clean primary data:
`mainData` `GlobalManager` MonoBehaviour path ID 49 points to `NpcManager`
path ID 1331 in `sharedassets0.assets`; its MonoBehaviour path ID 1501 points
to `NPCPrefab` Transform path ID 1211 / GameObject path ID 1321; the attached
`NpcMoveController` MonoBehaviour path ID 1458 serializes
`iBarkerChance=10`, `fBarketPeriod=15.0`, and `fBarketDistance=10.0`.

## Behavior contract

| Owner | Recovered behavior | Native contract |
|---|---|---|
| `NpcContainer.Add` | `m_iNpcName` selects `NpcStringTable`; positive `m_iBarkerNumber` selects `NpcBarkerTable`; `m_iHeight * 0.01` becomes `PrintName` height. | The production TableData loader retains the greeting row identity, greeting text, barker row identity, four fields and exact height. |
| `NpcIconMode.NpcGreetingBubble` | Calls `SetChatBubble(pStringTable.m_strComment, Barker)` before NPC interaction routing. | `TalkNpc` queues the localized greeting before shop, transport, race or mission routing. |
| `NpcMoveController.Start/Update` | Initializes `fBarkerTime` with `Random.Range(0, 15)`; after `Time.time - fBarkerTime > 15`, draws a 0–99 chance, requires `< 10`, then requires player distance `< 10`, then draws one of four fields in order: name, comment, comment1, comment2. | The session-seeded app-global legacy random stream preserves Unity's shared ownership, draw order, ranges, strict boundaries, global-time timer and 10%/10-unit gates. Tutorial actors and network NPCs both enter this controller-equivalent path. |
| `cnMissionManager.Update` | Refreshes the near list every second; every 20 refreshes selects one exact node from `m_CompletedMissionList`, scans NPCs within 5000 units, and sends `P_CL2FE_REQ_BARKER` when `m_iBarkerType` selects a positive `m_iHBarkerTextID[0..3]`. | The world mission runtime retains the exact completed task ID plus both Barker fields, uses the shared random stream, reproduces the 1-second/20-refresh cadence and sends the exact 8-byte registered OpenFusion request. |
| `P_FE2CL_REP_BARKER` handler | Resolves the packet's NPC runtime ID and `MissionTable.m_pMissionStringData[iMissionStringID].m_pstrNameString`, then calls `SetChatBubble(..., Barker)`. | The protocol/lifecycle path decodes the exact 8-byte reply and forwards the localized Mission NameString to the same per-NPC Barker FIFO instead of dropping it into passthrough. |
| `NpcMoveController.SkillReady` / `ReceiveCorruptionSkillReady` | Resolves `FindNpcSkillString(iSkillID)` or `m_iCorruptionString`, then submits `SkillStringTable.m_strComment1` as a Barker line. | READY and CORRUPTION_READY signals retain the clean NPC/string lookup and share the Barker FIFO. Empty primary strings are still rejected by `SetChatBubble`. |
| `PrintName.SetChatBubble` | Rejects empty and exactly one-space strings; NPC Barker entries append to `ChatList`; width is clamped to 100–300; lifetime is `max(7, height / 5)`. | Per-NPC FIFO entries use the same string gate, bounds and lifetime formula. |
| `PrintName.PositionUpdate/DrawBarker` | Projects root plus `fHeight * 0.8`; offsets upward by 15 pixels only when `bRDrawName` is true; requires `bBaloonChat`; draws `bbBox`, then a 5×20 `bbLabel` tail at `x + width/2 - 4`. | The Bevy overlay follows the same anchor and conditional offset, reads the native projection of both display options, and uses the same box/tail bounds and order. Clean defaults are `bNpcName=false`, `bBaloonChat=true`. |
| `FFGUIUtility.ScaleAroundRect` | Resolves the rectangle centre with `GetRectCenter`, then applies `ScaleAroundPivot`; `GetUiScale` is `max(1, Screen.height / 768 * 1.05)` while UI scaling is enabled. | The complete bubble root, including its tail, receives the gameplay UI scale around its own centre. At the 1280×720 reference size this remains exactly 1.0. |
| IMGUI ownership and depth | `PrintName.DrawBarker` is called by the resident world-name/barker pass; it is not a child of the normal HUD/Nanocom root and does not assign a foreground `GUI.depth`. Ordinary windows with lower IMGUI depth therefore cover it. | `NpcBarkerBubbleLayer` is an independent, non-pickable full-viewport root at `GlobalZIndex(-1000)`: hiding `LegacyGameplayHud` does not destroy it, while every default or foreground UI root covers it. |

The production regression guard uses the exact NPC visible in the reported
frame: Computress NPC type 2555 owns `m_iBarkerNumber=51`. Its four autonomous
lines are loaded from `NpcBarkerTable` row 51 and are intentionally distinct
from the `NpcStringTable` greeting shown when interaction begins. The ECS test
places that network NPC nine native units from the player, advances the clean
15-second timer, forces only the deterministic random draws, and verifies the
resulting `content.tabledata.npc.npc_barker.51.str_comment2` FIFO entry.

## Visual ownership

| Native asset | Primary owner | Source style/call | Conversion and acceptance |
|---|---|---|---|
| `assets/game/ui/gameplay/speech/barker_box.png` | `sharedassets0.assets` Texture2D path ID `236`, `bbubble`, 21×19 | `BubbleChatSkin` path ID `1386`, custom style `bbBox`; serialized border 6 on every edge; middle-centre text; 10-pixel padding; bilinear/repeat/no mipmaps | Raw Texture2D decode to PNG; 528 bytes; SHA-256 `FCF63DA682B189208EBF1B865A28EFBDD7B98488E1879C776621D3C9848C7D8B`; rendered as a 6-pixel nine-slice. |
| `assets/game/ui/gameplay/speech/barker_tail.png` | Texture2D path ID `303`, `bb_009`, 5×21 | `bbLabel`; clean draw rectangle is exactly 5×20 | Raw Texture2D decode to PNG; 274 bytes; SHA-256 `6C1E8EA7AD1C2717673A0696BC6FB2C6DC714F7572ED4FE8ACF2E48F930BA513`; rendered with direct stretch as the source call requires. |
| `assets/game/fonts/jeffe.otf` | legacy Font path ID `903`, `JEFFE___14`; serialized line spacing `13.710000038146973` | `bbBox`/`bbLabel` font owner | Approved native replacement font required for Cyrillic; 32,776 bytes; SHA-256 `F8D41844AD2092D9998E51B8CBEF5B65B3CE6DB276C93949ECECAE227674C3E1`; native line height pinned to 13.71. |

The two PNGs are validated native assets. Raw recovery and focused reports
remain under `work/legacy-sources`; no legacy container or extraction cache is a
runtime dependency.

## Localization

Greeting entities use
`content.tabledata.npc.npc_string.{m_iNpcName}.str_comment`. Random barker
entities use
`content.tabledata.npc.npc_barker.{m_iBarkerNumber}.{str_name|str_comment|str_comment1|str_comment2}`.
Server mission barkers use
`content.tabledata.mission.mission_string.{iMissionStringID}.str_name_string`;
skill-ready lines use
`content.tabledata.skill.skill_string.{stringID}.str_comment1`. Every spawned
text entity owns `LocalizedText` with the exact English TableData value as
fallback. The populated key families already exist with matching placeholders
in authored EN/RU bundles and are published byte-for-byte to the runtime
bundles.

## Deterministic GPU acceptance

`gameplay_hud_gpu_preview` owns an `FFONE_NPC_BARKER_PREVIEW` route that
spawns network NPC 643, a real world camera/player pair, and requests its
TableData greeting through `NpcBarkerBubbleRuntime`. It does not draw a
standalone mock bubble.

| Locale | Capture | Size | SHA-256 |
|---|---|---:|---|
| EN | `target/ffone-gameplay-hud-npc-barker.png` | 175,737 | `C952186660418789D50DE1F452CD34DFD8DCB09F5B0A15B07D0D1DC1F403AE60` |
| RU | `target/ffone-gameplay-hud-npc-barker-ru.png` | 178,493 | `4A1CF5FD24E92D74352099A79208B636EB45F5780562DE24B031C1CD86F56268` |
| EN + NPC vendor mode | `target/ffone-gameplay-hud-npc-barker-vendor.png` | 74,633 | `17DC3B2CA4F8B46EBB15A8266BA4800D78CCF7E48728CA78B1DEC593A4F58349` |
| EN + Spawn Spree reward mode | `target/ffone-gameplay-hud-npc-barker-reward-world.png` | 371,835 | `9385EBB3378713DBADBD736500D6A300C0F2D9D94556BC26AEEE41E97B6EB66A` |

All four 1264×681 captures were visually inspected at original resolution. The
standalone EN/RU frames verify intact nine-slice corners, centered
direct-stretch tail, 300-pixel wrap, the default no-name anchor (15 pixels
below the earlier unconditional-offset implementation), and Cyrillic glyph
coverage with the approved native font. The Spawn Spree reward frame reproduces
the reported Mission Completion window and verifies that it fully covers the
resident Barker pass.
