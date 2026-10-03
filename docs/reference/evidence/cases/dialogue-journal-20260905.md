# Dialogue and journal follow-up — 2026-09-05

Authority: canonical `primary`, source `retrobution`, build 20260821. This is a
bounded follow-up to `parity-audit-20260905.md`, not completion of the full backlog.
The owner's latest instruction remains: do not place NPCs.

## Primary evidence

Reused the verified assembly extraction below
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/`.
`assembly-csharp.evidence.json` records the main raw source and assembly extraction:

- Raw main SHA256: `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
- `Assembly-CSharp.dll` SHA256: `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- Decompiled `cnMissionJournal.cs` SHA256:
  `FA474EE046628F211F2135A17FD5EDF2055723DBE238AF0D24D65878586153DB`.
- Decompiled `cnGUINanocom.cs` SHA256:
  `E207DA3DBC5D6AE29E095478AC34C9521FFD4C0EDDF48471C2EEF630760278B1`.
- Decompiled `AvatarUtil.cs` SHA256:
  `4B62D5528780486D113512FE6086C047F5BC6F4A545F9B4CB456A7255704625A`.

`cnMissionJournal.SetCurrentMission` (around lines 637–654) queries the existing
tracked mission through event 12/17 and assigns the viewed `currentMission`.
The row background buttons (2265–2269 and 2305–2312) call that method. Tracking is
a separate event 12/18 from MAKE CURRENT MISSION (1767–1773) or the small icon
toggles (2283–2291, 2325–2333). A row background click therefore must not retarget
the HUD mission.

`cnGUINanocom.Update` and `RenderNanoMessage` freeze message presentation during a
tutorial scene event. Extending this suspension to an ordinary NPC conversation
or journal is an **owner-requested correction**, not a claim that primary Unity
already had that gate. The native queue retains its head, reveal phase, remaining
lifetime, and unplayed sounds during suspension.

## Native changes

- `mission_ui.rs`: row browsing changes only `viewed_journal_task_id`; the explicit
  MAKE CURRENT action still changes `selected_journal_task_id`. Updated the model
  documentation and interaction regression to distinguish those operations.
- `app/social_ingress.rs`: include the NPC panel, all journal modes, mission system
  popup, and central foreign-modal suppression in the NanoCom message gate. Read
  central suppression before running the message context and queue tick.
- `app/tests/social.rs`: exercise the production context against an NPC dialogue,
  offer page, reward page, and ordinary journal. A 120-second hidden interval must
  retain the same message and its original lifetime; closing the dialog resumes it.
- `nanocom_message_ui.rs`: exercise the production audio consumer in EN/RU. A
  suspended head spawns no audio; resuming spawns the slide and localized voice
  once, with no per-frame replay.

No UI geometry, text keys, voice payloads, shaders, NPC definitions or placements
change in this follow-up. Existing locale controls and replacement fonts remain.

## Findings that are not new gameplay fixes

Larry 3000's production NPC type 692 already selects voice owner `larry`. Its two
greetings, quest greeting, three farewells, three good-luck and three completion
lines exist in both voice locales. Added production-table/catalog coverage rather
than replacing valid audio. An interactive failure to hear Larry is not ruled out
by a catalog test alone.

The NanoCom audio test also covers the current `NumTwo_CommOut01` EN-only route:
RU legitimately resolves its catalog fallback to EN. Larry's CommOut has separate
EN/RU files and verifies actual locale switching. Do not invent or require a
nonexistent RU Numbuh Two file to satisfy an audio-consumer test.

The generic Dexter voice test incorrectly required `Dexter_clickwarp0*` even though
Dexter is not a normal warp-service owner. Removed that unsupported expectation;
the separate production warp-owner test still covers actual warp cues and the
primary silent branch. Intermediate talk/rewardless hand-ins already have native
regressions that avoid opening an offer/completion page.

## Validation

- Native test executables and ordinary `ffone-client.exe` built successfully.
  Existing unrelated warning: unused `CameraProjection` import in `app/mod.rs`.
- `target/parity-dialogue-social-final.log`: 31 passed, including the production
  NPC/journal suspension and existing modal/chat integration checks.
- `target/parity-dialogue-audio-final.log`: 20 passed, including Larry's production
  owner/catalog, spatial NPC audio ownership and localized voice lifecycle.
- `target/parity-dialogue-mission-final.log`: 49 passed, one pre-existing failure.
  `transportation_service_icons_match_clean_primary_bytes` still expects 1,969
  bytes for `ui/gameplay/journal/npcicon_10.png`, while the installed file has 2,058.
  The same failure is present in `target/parity-tests-after-publication.log`.
  No icon bytes or expected hashes were changed here.
- Full production app, opt-in `FFONE_PERF_OUTPUT` fixture:
  `target/performance/dialogue-gate-20260905/{frame.png,report.json,stdout.log,stderr.log}`.
  Finished 600 sampled frames, saved a world/player/HUD capture, and exited through
  the fixture's success path. No schedule-cycle or runtime panic. This is a
  startup/schedule smoke check, not a before/after performance claim or interactive
  proof of dialogue rendering. The capture was visually inspected.
- NPC placement files retain SHA256
  `5E62CBD0CE9574653B3C2DC8A57478ED179E28B38B2F1EEEFFC84056C1A588F2`
  in both server trees.

Final NanoCom recheck is recorded in
`target/parity-dialogue-nanocom-final-verified.log`: **23 passed, zero failures**
(test fixtures distinguish native
Windows paths and the existing EN fallback described above).
