# Normal NPC Warp parity evidence

This slice reproduces the clean `NpcIconMode` Warp path. Legacy files remain
offline evidence only; the runtime consumes the published native effect/audio
catalogs and protocol-0104 contracts.

## Accepted primary evidence

- Source alias: `primary` (`builds/retrobution-20260613`).
- `main.unity3d`: 7,000,415 bytes, SHA-256
  `59788201962b6a1737b114486c361fe74eef69f507d1d125ca3171377eec602f`.
- Navigation-only extracted `Assembly - CSharp.dll`: 1,517,568 bytes, SHA-256
  `33d6f70216b1c7ba05bcc0f270fba97e767b129159755af4c8835922e60acadb`.
  It was read from patched cache path
  `cache/extracted-bundles/ed793e024e70bdfc/Assembly - CSharp.dll` and verified
  against the primary container rather than treated as patched behavior.
- `Effects.resourceFile`: 7,593,292 bytes, SHA-256
  `7f7d4c2b49fd6a99e564acd83c98ad9aab4b7b0831cbdc6dcad132fcc96c7209`.
- Effect container identity:
  `prefabs/particle/effectscripts/es[394].prefab`, root asset
  `CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918`, PathID 9545.

## Recovered contract

`NpcIconMode.WarpOK` ends the camera subtarget, sets a 1.5-second wait, plays
spatial SFX true name `Dexbot_Warp`, starts `LoadWarpEffect`, and requests the
NPC string-table `WarpOK` voice. `LoadWarpEffect` preloads and instantiates
ES394 at the local player's world position and rotation.

The primary acceptance row for normal-Warp NPC 681 uses voice owner
`m_plmber3`, but clean Retrobution publishes no
`m_plmber3_clickwarp01..03` containers. A
`find-objects.cmd -Query m_plmber3_clickwarp` lookup returns no route, while
the broader `clickwarp` lookup returns only the independently owned
`botwarp1/2/3`, `mkndtransport2`, `door`, `kumari`, and `taxi` families. The
native path therefore preserves the original silent `AssetLoader` miss for
this optional voice call; it does not alias another character's localized
voice. The independently catalogued `Dexbot_Warp` SFX still plays.

When the timer expires, `NpcIconMode.Update` disables movement-packet
emission, calls `GameFrame.DongReady`, emits `eFadeType.WIn`, and sends
`P_CL2FE_REQ_PC_WARP_USE_NPC`. `GameFrame` renders `WIn` with `WhiteTexture` at
alpha 1.0 and reduces alpha by 2.0 per unscaled second. Position and Candy are
not predicted: only `P_FE2CL_REP_PC_WARP_USE_NPC_SUCC`/`GOTO_SUCC` may apply
the server position. `P_FE2CL_REP_PC_WARP_USE_NPC_FAIL` restores packet
emission and opens SystemMessage 173.

## Reproduction

- Decompiler: `ilspycmd 11.0.0.9375`, types `NpcIconMode` and `GameFrame`.
- Effect publication: `ffone-asset-pipeline install-tutorial-effects` using
  `work/legacy-sources/effects-source-retrobution-v2/{fa9,b4f,bd5}.dump-object-all.json`
  and the three verified serialized assets recorded by the generated catalog.
- Audit: `tools/legacy-sources/audit-effects.ps1`.

The native movement controller is disabled at the send edge in addition to
the explicit movement-intent packet gate. This is an adapter detail: while
`NpcIconMode` owns input, it prevents local drift and produces the same
observable network behavior as clean `bEnableSendPacket = false`. There is no
intentional content or timing divergence from `primary`.

## Tutorial routing repair (2026-09-04)

Evidence question: why can the first Infected Zone warp (tutorial NPC 2694,
WarpTable 253) disappear when its button is accepted, and why is there no
departure presentation? This repair reuses the primary `NpcIconMode.WarpOK`
contract above; it introduces no alternate donor or binary payload.

Native diagnosis: `consume_world_gameplay_ui_outbox` runs in both Tutorial and
World. It and `consume_tutorial_mission_outbox` both drained the entire
`GameplayUiOutbox`, so the first scheduled consumer could discard the other
owner's actions. The world NPC validator cannot accept a tutorial actor as its
network NPC. The tutorial implementation also applied the destination directly
without the common NpcIconMode departure effect or wait.

The native queue now partitions actions by authority while preserving order
within each owner. Tutorial task/warp/close/exit actions remain with the local
virtual server; shared chat and menu actions remain with their shared handlers.
Tutorial warp acceptance validates the immutable destination and task gate,
starts ES394 and semantic `Dexbot_Warp`/WarpOK audio, waits 1.5 seconds, and
revalidates before applying the local virtual-server destination. It then
confirms the UI, emits NpcWarp, and starts the existing WIn fade. A lost NPC,
lost UI request, or tutorial completion cancels the pending warp.

Regression coverage is in FFOneClient's `app/tests/tutorial_warp.rs`: production
effect compilation, departure position, delayed destination, one-shot commit,
shared queue retention, and owner-loss cancellation. Reproduce with
`cargo test --manifest-path ../FFOneClient/Cargo.toml -p ffone-client --bin ffone-client tutorial_warp`.
GPU timing/pixel acceptance for this repair has not yet been captured; the
structural tests must not be presented as a new visual parity certification.

Verification observed in the shared checkout: the real Dev binary built
successfully at 2026-09-04 02:54 MSK and was launched with Russian text. The
startup log reported unrelated unresolved NPC assets; no panic was observed
before the verification process was stopped. The installed effect/projectile
closure compilation test passed. The new binary regression tests were blocked
by concurrent changes to unrelated terrain test calls; a queued retry was
cancelled after repeated shared-build invalidation. They remain unverified,
and an actual in-game teleport capture is still required for visual acceptance.

## Shared NPC button hit testing repair (2026-09-04)

The owner clarified that normal-world warps also fail. A second defect is
upstream of either warp authority: native NPC action captions and icons carry
`Pickable::IGNORE`, but no `FocusPolicy::Pass`. Bevy 0.17.3 `ui_focus_system`
does not query `Pickable`; it walks the UI stack front to back and treats a
missing FocusPolicy as Block, even on a text/image node without Interaction.
Thus clicking the caption or icon never presses the parent WARP button.

FFOne's shared skin-label builder and NPC action icons now explicitly pass
Interaction focus through to the button. This changes no primary geometry,
label, asset, localization, warp eligibility, or server authority. The existing
primary NpcIconMode GUI.Button contract above remains the behavioral source.

`mission_ui_warp_input_tests.rs` exercises Bevy's real mouse-to-Interaction
system against the production button and children in both world and tutorial
input modes. It first removes the fix to reproduce the swallowed click, then
restores it and checks one NpcWarp action for clicks on caption and icon.
`app/gameplay_ui_actions/warp_tests.rs` exercises the real world outbox consumer,
production warp/effect assets, and the delayed request without local movement.
Verification completed at 2026-09-04 03:26 MSK:

- `warp_button_caption_and_icon`: passed (one test covers caption/icon and both
  world/tutorial modes, including negative reproduction without FocusPolicy).
- `ordinary_world_warp_click`: passed against the real world action consumer
  and installed native effect library.
- `app::tests::tutorial_warp`: both tests passed, superseding the earlier blocked
  binary-test result recorded above.
- `russian_bundle_covers_the_canonical_bundle_with_matching_placeholders` and
  `warp_and_npc_close_emit_original_tutorial_event_semantics`: passed.
- `cargo build -p ffone-client --bin ffone-client`: succeeded; updated Dev binary.

No GPU acceptance or live-server destination transition is claimed by these
headless regression tests.
