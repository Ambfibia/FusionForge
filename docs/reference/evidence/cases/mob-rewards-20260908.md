# Mob quest-drop and crate reward notices

Question: which primary server event produces the mob-had/mob-did-not-have
message and the post-kill crate image, and which native consumer is missing?
Authority: primary Retrobution 20260821. No alternate or patched donor.
Consumer: FFOne `apply_world_mission_network_frame` and `gameplay_ui::rewards`.
Acceptance: real packet fields select the correct queue; positive/negative
quest outcomes and crate/battery visuals survive localization, hide during
chat input, expire at the source times, and reset across sessions.

## Evidence and contract

Raw `main.unity3d`: 8,221,718 bytes, SHA-256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
Exact `level0/Assembly - CSharp.dll`: 1,762,816 bytes, SHA-256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
Both were rehashed against the current raw source before reusing the existing
managed evidence. The focused FFSpy replay is:

```text
dotnet run --no-build --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/cases/mob-reward/managed/Assembly-CSharp.dll -t cnDisplayMapName -o work/cases/mob-reward/managed
fusionforge export-ui-interaction-evidence work/cases/mob-reward/interaction.request.json --out work/cases/mob-reward/interaction.json
```

Scoped owner: `main.unity3d/sharedassets0.assets`, MonoBehaviour 1620,
GameObject 1361, MonoScript 1030. Its strict evidence is retained at
`docs/reference/evidence/cases/artifacts/retrobution-ui-completion-20260907/combat-notice.json`.
The owner selects GUISkin 1372, `fRewardIconTime=2`, crate 34, potion 548,
boost 29, all local PPtrs. Exact texture exports are selected by serialized
asset **and** ID; a bare container-level 34 resolves to a different GameObject
and was rejected. Publication is reproduced by the tracked
`tools/legacy-sources/publish-mob-rewards.py` and the receipt
`recipes/native/ui/mob-reward-icons-20260908.json`.

| Role | Trigger and gates | Geometry/style | Time/order |
| --- | --- | --- | --- |
| Found item | `GameFrame` reward packet has type-8 item and positive NPC type; `cnMainGame.ReceiveQuestItem` calls `QuestItem` | Uppercase localized mob/item template, BigFont16, blue `(166,176,255)`, black shadow +1,+1; width UTF-16 count*16+20, x=W/2-count*8, y=H/4 | FIFO, three seconds; final second moves +100 x and toward y=280 while fading |
| Missing item | Reward has no type-8 item; positive NPC type; exact active task matches packet task; first CSU enemy slot matches mob and has zero kill count; item is **STItemID**, not CSTItemID or CSUItemID | Same BigFont16 geometry, red `(234,52,36)` | FIFO, three seconds; final second moves down 150 and fades |
| Crate | Inventory location 1, item type 9, ChestCheck=0; tutorial suppresses icons | Exact 178x142 image, x=W/2-225; caption rect 0,110,178,30 plus black +1,+1 shadow; centerbox3 JEFFE14 | Independent FIFO; two-second cubic-sine bounce, two seconds of icon fade; caption restores opaque white as in source |
| Potion/boost | Positive pre/post packet battery delta | Same 178x142 geometry, x=W/2-75 and W/2+75; `+count` caption | Independent queues; same bounce/fade |

`OnOwnGUI` is called by `cnMainGame`, not a Unity `OnGUI` callback on the
display component. It sets GUI depth 12 and suppresses `DoRewardGUI` while
chat editing is active and the menu slide has settled. `IsInputEnabled`
reads `bChatEnable`, not the native permission field `ChatUi.input_enabled`.
The first real Dev capture exposed that distinction; the native regression
now enables chat permission while leaving editing inactive. Timers still
advance while hidden. Icons use the
bottom-center screen scale pivot and y=H-(170+quickSlotOffset), with offset
40 only for Quick mode. Quest labels undo that screen pivot and use their
own rectangle pivot. Every placement/pass remains distinct.

`BigFont16`: centered, padding 10/6/4/6, font 1012, no background.
`centerbox3`: upper-center, zero padding/border, no background, font 903
`JEFFE___14`, line spacing 13.71. Native captions reuse the existing approved
JEFFE14 vector/vertical-scale adapter, not a new bitmap font. Native images
use explicit Stretch. All three source images are single-mip, linear,
repeat, aniso 1, sRGB UI images, exported with only PNG row inversion.
The skin's built-in Unity default-resource script pointer remains explicitly
unresolved in the generic triage report; it is not used to identify a font
or texture. Local accepted font/image pointers are resolved independently.

The static interaction artifact is an IL candidate inventory, not proof of
runtime reachability by itself. Managed field/caller tracing above establishes
the native event contract. A screenshot alone is not used as source evidence.

## Native verification

FFOne now queues notices only after authoritative inventory validation and
before replacing battery totals or advancing mission completion. Neither a
local death nor an inventory repaint fabricates a reward. Session reset clears
both queues, including a new WorldReady epoch. Language changes resolve both
template and content arguments.

Focused tests cover real production task 2248 / NPC 2676 / item 537, absent
and positive quest drops, wrong/inactive tasks, slot post-state versus drop
count, battery deltas, queue expiry, scale pivots, production ECS visibility,
exact image hashes, and complete EN/RU key/placeholder equality.

GPU harness: `FFONE_REWARD_PREVIEW=found|missing|found-fade|missing-fade|chat`
with `gameplay_hud_gpu_preview`. Real Dev fixture:
`FFONE_PERF_OUTPUT=target/performance/mob-rewards`,
`FFONE_PERF_REWARDS=found|missing`. These hold a deterministic reward pose
for visual inspection and are not performance measurements.

The five focused reward regressions passed through Cargo and through a direct
unoptimized build of the final chat-gate implementation. The actual Dev client
and HUD preview were rebuilt with Cargo; the final reward textures explicitly
retain source bilinear/repeat sampling. Reviewed real-client DX12 captures:
`../FFOneClient/target/performance/mob-rewards/accepted-ru/frame.png` and
`../FFOneClient/target/performance/mob-rewards/missing-en-final/frame.png`,
1920x1080. The final Vulkan HUD preview is
`../FFOneClient/target/performance/mob-rewards/found-ru-final-preview.png`,
1264x681. Both outcomes, both final-second text motion/fade paths, and active
chat were also reviewed in the focused harness. Exact binary/frame hashes and
test outcomes are in `artifacts/mob-rewards-20260908/visual-acceptance.json`.
The saved `final-client.exe` is a byte-identical copy of the successful Dev
build, used to avoid concurrent relinking during its capture.

Verification limits are explicit: these are deterministic reward fixtures,
not a live shard combat replay, and no FPS claim is inferred. The general
localization suite returned 18/19; its failure is the existing source-scanner
ordering marker in `bank_ui/item_popup.rs`. The broader content suite returned
24/27: vendor fixture lacks `m_strComment`, the duplicate-task assertion expects
older error wording, and the Oil Ogre fixture expects mesh 113 while the current
table contains 608. None is changed by the reward fields. Fast asset validation
passed; the full owner audit stopped on `textures/hnpc/m_face_006_a.png`, outside
the reward payloads. Formatting and `git diff --check` passed for this change.
