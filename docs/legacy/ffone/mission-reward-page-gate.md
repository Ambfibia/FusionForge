# NPC mission reward-page gate (2026-09-04)

## Evidence question

Which condition opens the Reward journal when a ready mission row is clicked
in NpcIconMode? FFOne previously treated only type-1 tasks with an outgoing
task as immediate progress. The reported window still appeared for other
rewardless NPC interactions. Native consumer: MissionUiModel, shared by the
local tutorial and server-authoritative world. No assets or layout change.

## Primary evidence

Reused strict assembly export and decompilation from
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary`.
The raw container, exported assembly and navigation C# hashes were rechecked
on 2026-09-04:

- `primary`, `main.unity3d`: 8,221,718 bytes, SHA-256
  `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
- Level 0, exact entry `Assembly - CSharp.dll`: 1,762,816 bytes, SHA-256
  `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- `decompiled/NpcIconMode.cs`: SHA-256
  `60489BFD4E2F97CBC32C4FBF62B1A914A79251F390B0F7A6C82D0151A394FEE5`.

The CompleteTaskList button branch (lines 1684-1710) reads the clicked
mission's `m_iSUReward` (the separately selected Korean expansion branch uses
`m_iKorSuccRewardID`). A positive reward opens `eJM_Reward`; otherwise it sends
the task-end event directly. This gate does not inspect task type, outgoing
task ID, cash, Fusion Matter, or the mission's eventual final reward.

Replay the strict export/decompilation using the commands recorded in
[NPC dialogue audio lifecycle](npc-dialogue-audio-lifecycle.md). No patched or
alternate donor establishes this gate.

## Native contract and regression coverage

- Project `has_task_reward` from the current task's reward ID. Keep the
  existing final-task reward amounts for journal display only.
- Ready NPC rows without a task reward submit one zero-choice QuestEnd and
  never enter Reward mode. This includes type-2 proximity, type-3 object,
  type-4 delivery, type-6 escort and rewardless terminal rows.
- Preserve the existing explicitly authored first-Nano offer handoff; this
  correction does not redefine that separate extension.
- A failed request unlocks the NPC list; a rewarded hand-in still waits for
  its explicit Complete button and server reply.
- Production input-system regression exercises tasks 2253 and 2251 (tutorial)
  plus 25, 44 and 576 (world). A content test proves that task 2248 displays
  the final reward but has no current reward, while 2249 has that reward.

This is managed-branch evidence plus native state/input acceptance, not a
claim of a newly captured legacy GPU frame. Test/build results are reported
in the task handoff.

## Verification results

On 2026-09-04 the new production NPC button regression, rewardless terminal
regression, existing intermediate-talk and rewarded-terminal tests passed.
The current-versus-final reward content test and production world Computress
reward hand-in regression also passed. `mission_ui::tests` returned 43 passed
and 5 pre-existing failures: four missing `GameplayUiAudioOutbox` resources in
test apps and the `npcicon_10.png` byte-count expectation (2058 vs 1969).
Formatting and `git diff --check` passed for the touched FFOne files.

`cargo build -p ffone-client --bin ffone-client` succeeded. The rebuilt
development executable is dated 2026-09-04 09:07:03 local, 205,036,544 bytes,
SHA-256 `70F798569CBD2690ACE02DAF6C2BA3E0BB78DCC931081BD2818AD2C0D6885940`.
Running it with `--validate-assets` returned `status: ok`. This validates the
rebuilt runtime's catalogs; manual gameplay capture of the reported NPC was
not performed.

## Follow-up: pending-frame flicker

The user confirmed the Reward page no longer opens, but reported a brief
flash. Rechecked the same raw container, assembly and C# hashes above. In
`NpcIconMode` lines 1690-1707, the close-mode event exists only in the
positive-reward branch. The rewardless branch sets `bSendPacket` and sends
TaskEnd without closing NpcIconMode. Reply handlers at lines 861-877 clear
the pending flag and refresh `CheckQuest` on the same target.

FFOne still hid `npc_icon_mode_visible` before both branches, then restored
it in `confirm_quest_end`. This also toggled the letterbox, HUD chrome,
gameplay input ownership and camera sub-target during the pending frame.
The native correction hides the NPC list only when opening an actual
Allow/Reward page. Rewardless hand-in retains the NPC presentation and uses
the existing pending request guard to reject duplicate input.

The regression uses the production button handler and presentation binder,
checking actual Bevy Node display states before submission, on every delayed
reply frame (including a repeated press), and after success or rejection.
Success replaces the last quest row with the ordinary close-only NPC menu;
rejection retains the row for retry. Neither path hides the letterbox or
opens the journal. This is deterministic ECS visibility/timing acceptance;
no new in-game GPU capture is claimed.

Follow-up verification: the pending-frame regression passed, and all 44
mission UI tests outside the five previously recorded unrelated failures
passed. World mission tests returned 21 passed and one content error:
`instance_and_escort_contracts_are_not_offered_or_promised_on_the_minimap`
encounters a task-5176 mission-chain cycle. The current-versus-final reward
content test still passed. No task data was modified for this follow-up.

The follow-up development binary rebuilt successfully in 3m 30s and is
dated 2026-09-04 09:30:40 local (205,036,544 bytes). Its `--validate-assets`
run returned `status: ok`. Rust formatting and FFOne `git diff --check`
also passed.
