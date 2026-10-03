# Mission journal checkbox input repair (2026-09-10)

Question: why does clicking the active-list checkbox browse a mission without changing tracking?
Native consumer: `mission_ui.rs`, production journal hierarchy and `handle_mission_ui_buttons`.

Reuses primary assembly provenance and replay commands from
[mission dialogue evidence](mission-dialogue-and-selection-20260908.md).
The cached assembly and decompiled journal hashes were rechecked and match that record:

- Assembly-CSharp.dll: `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- cnMissionJournal.cs: `FA474EE046628F211F2135A17FD5EDF2055723DBE238AF0D24D65878586153DB`.

The active row code at lines 2263-2291 and 2303-2333 separates `IconToggle`
from the overlapping `MissionBack` button. The toggle updates `SelectMission`
and sends local channel 12/event 18; it does not call `SetCurrentMission`
to replace the detail panel. Clicking the already tracked toggle never clears tracking.
These are static managed-code findings, not a fresh legacy runtime capture.
No source or binary payload is republished.

The native checkbox was a decorative `ImageNode` with `FocusPolicy::Pass`
and `Pickable::IGNORE`. It passed clicks to the browsing row. The repair gives
it a blocking Button and row-specific tracking control, retains the native
pending/active-tab/tutorial mouse gates, and derives hover from the checkbox's
own Interaction. Existing geometry, localized copy, fonts and textures are reused.

Acceptance uses the production `spawn_journal` tree with Bevy `ui_focus_system`
and mouse events. Each decorative overlay must pass through to browsing;
the checkbox must consume the click, change tracking, preserve the viewed
mission, and emit no gameplay packet. Layout is supplied by the fixture;
this is an input/ECS check, not a GPU screenshot or a live server session.

Executed validation: final `cargo test -p ffone-client --lib mission_ui -- --nocapture`
passed all 53 tests, including the mouse regression and EN/RU content coverage.
`cargo build -p ffone-client --bin ffone-client` succeeded; the built executable's
`--validate-assets` returned exit 0, `status: ok`. Touched Rust files were formatted
and `git diff --check` passed. No manual live-session checkbox check is claimed.
