# Tutorial dialogue visibility and mission bubble regression

Evidence question: why are Dexter/Numbuh Two tutorial voices audible without
localized dialogue, and why is Buttercup's mission bubble absent?

Authority: current primary `retrobution-20260821/main.unity3d`, SHA-256
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
No alternate donor or patched build is used.

Replay from FusionForge (create the output directory first):

```powershell
.\fusionforge.cmd export-managed-assembly-evidence primary ..\builds\retrobution-20260821 main.unity3d 'Assembly - CSharp.dll' --level 0 --out work/cases/tutorial-missing-messages-20260911/managed-evidence.json --payload-out work/cases/tutorial-missing-messages-20260911/Assembly-CSharp.dll
& vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.exe -t cntutorialscript work/cases/tutorial-missing-messages-20260911/Assembly-CSharp.dll | Set-Content -Encoding utf8 work/cases/tutorial-missing-messages-20260911/cntutorialscript.cs
& vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.exe -t NpcIconMode work/cases/tutorial-missing-messages-20260911/Assembly-CSharp.dll | Set-Content -Encoding utf8 work/cases/tutorial-missing-messages-20260911/NpcIconMode.cs
& vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.exe -t cnMissionManager work/cases/tutorial-missing-messages-20260911/Assembly-CSharp.dll | Set-Content -Encoding utf8 work/cases/tutorial-missing-messages-20260911/cnMissionManager.cs
```

The exact assembly payload is 1,762,816 bytes, SHA-256
`0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
The resulting focused decompilations are:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| cntutorialscript.cs | 129612 | FAD147807D47413312FD0C4433287C6DC2BB70DB734FD5EEE0AFA7B8A80319F9 |
| NpcIconMode.cs | 64736 | 6F2BE9A069EBBF492ED7EE8BB8274EA8757FC42DD1665E07BFB7B94D51FF3AD7 |
| cnMissionManager.cs | 71785 | 8E366D93918FD646F00697279B5989AB669B1BD849AB303A5FBC99E217CC42AF |

`cntutorialscript.OnGUI`, line 1070 in the focused output, explicitly renders
nonempty `VOSubTitle` when `!bEventScene && localized.lang > 0`. Its cinematic
branch remains gated by the event scene, scene number, cinematic flag and
alpha above 0.9. Both branches use the same area, `centerbox` speaker and
`smallfont2` dialogue. FFOne's visibility helper had deliberately removed the
interactive localized branch. Restoring it requires no new geometry, fonts,
text, voice asset, or timing contract.

Rejected hypothesis: `NpcIconMode.NpcGreetingBubble` calls
`PrintName.SetChatBubble` with `m_strComment` (line 396), but the production
tutorial Buttercup row has a one-space greeting and no autonomous barker.
Adding that call cannot produce the requested bubble; the exploratory change
was removed.

`cnMissionManager.SetBubbleChat` selects the success dialogue and its explicit
NPC owner for both Success and Complete (lines 1562-1565), and calls
`PrintName.SetChatBubble(..., QuestChat)` at line 1589. Start/Complete/Success
invoke it at lines 1911/2034/2044. Its near-list uses 5000 plus NPC geometry;
the native world mission presenter already implements that contract.

Production task 2250 has success dialogue string 11684 and NPC owner 2672:
"Dexter has disappeared. You gotta see if he's in the infected zone. I'll
check out here. There's no time to lose!" The tutorial's local task transitions
did not publish mission dialogue edges. They now enqueue Start and Complete
only on actual state changes, preserving Complete-before-outgoing-Start order.
The tutorial presenter resolves the declared nearby live TutorialActor, then
uses the existing keyed quest speech queue, balloon option, projection and
chat-history owner. The tutorial voice timeline remains the audio owner.

Native regression coverage includes cinematic/localized visibility gates and
an ECS test for Buttercup's production mission dialogue reaching the tutorial
actor's bubble and chat, with key resolution in both EN and RU bundles, plus
exactly-once ordered task transition coverage.

These are managed-code and native structural findings. No new visual assets
are published. Interactive GPU acceptance remains a separate check; this
record does not claim a completed gameplay capture.

Validation during this repair:

- 12 native subtitle tests passed, including restored interactive visibility,
  text entities, source timing, font/area geometry and Z order.
- Production EN/RU canonical key and placeholder parity test passed.
- `tutorial_buttercup_mission_dialogue_reaches_bubble_and_chat_in_both_locales`
  passed against production task 2250 and both localization bundles.
- Full Dev build and binary task-transition test were attempted but are
  blocked by concurrent, unrelated edits: initially `NativeWaterOcclusion`
  visibility (subsequently corrected), then incomplete `MissionUiView`
  handling of `NpcTopNotice` / `NpcTopNoticeText` in `mission_ui.rs`.
  The binary task-transition test has not run, and a rebuilt executable or
  gameplay capture is not claimed.
- The old `mapping_matches_every_decompiled_scene_text_voiceout` test cannot
  run at its obsolete FFOne-owned `work/ilspy-retrobution-csharp` path. No
  legacy cache was restored in FFOne. Instead, decompile the exact assembly
  above with `--preserve-iterator-state-machines -t cntutorialscript` into
  this Editor case's `cntutorialscript-full.cs`: all 87 unique
  `VoiceOut(cue, TextManager.GetSceneText(2, line), duration)` tuples match
  the native `spec`/`override_spec` array, case-insensitively by cue and
  exactly by scene line and duration.
