# NanoCom type-10 Nano mission notice evidence

This append-only supplement closes and supersedes the type-10 intentional
limit recorded in `nanocom-login-mission-evidence.md`. It covers the passive
notice for the first task of a server-assigned Nano mission. It does not
authorize runtime access to any legacy build.

## Authority and extraction

- Canonical source alias: `primary`.
- Raw owner: `builds/retrobution-20260821/main.unity3d`, 8,221,718 bytes,
  SHA-256 `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
- Managed owner: `Assembly-CSharp.dll`, 1,762,816 bytes, SHA-256
  `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- Strict object evidence is retained below ignored project
  `work/projects/retrobution-ui-20260821.ffclient/reports` as
  `nanocom-type10-component-1562.evidence.json` and
  `nanocom-type10-texture-137.evidence.json`. Component
  `sharedassets0.assets:1562` resolves field `/NanoComNanoBg` to
  `sharedassets0.assets:137`, Texture2D `nanocom_message_nano`.
- The accepted publication record is
  `recipes/native/ui/nanocom-type10-nano-message-primary-20260821.json`.

## Recovered producer contract

- `cnMissionManager.ProcessStartSucc` special-cases mission type 2 only when
  `parents.GetChildByIndex(0) == task`. It calls `SetMissionMessage` with
  button type 10 and `m_iSTNanoID`; later tasks retain the ordinary type-9
  route.
- `cnMissionManager.SetMissionMessage` still requires positive NPC/string
  identities, `messageType & 2 != 0`, and resolved copy longer than one
  character.
- The native producer keys this branch from authoritative `TASK_START_SUCC`
  and serialized mission child order. It never infers readiness from a local
  Fusion Matter threshold, so the notice is attached to the server assignment
  edge rather than a per-frame condition.

## Recovered presentation contract

- `cnGUINanocom.SetMessageBox` type 10 resolves the portrait from the Nano
  table row `m_iSTNanoID` and its `m_iIcon1`.
- The source NPC's `NpcStringTable.m_strComment2` remains the owner of the
  localized `<owner>_CommOut01..03` voice.
- The keyed title is `New Nano Mission!`; the body retains the source
  `MissionStringData` semantic key and EN/RU fallback.
- The passive lifetime is `fNanoMessageTime * 0.5`, exactly 10 seconds. The
  shared reveal remains the existing 0.5-second squared-sine slide.
- Type 10 draws full `NanoComNanoBg` in `NanoComNanoBox (0,0,372,122)`,
  places the portrait at `NanoIconRect (55,10,64,64)`, and retains the shared
  title/body Rects `(130,2,200,30)` and `(120,25,180,70)`.

## Native publication

- Runtime path:
  `assets/game/ui/gameplay/nanocom/nanocom_message_nano.png`.
- Dimensions: 372x122.
- Bytes: 27,271.
- SHA-256:
  `AE1EAE32C160A1FC34079C02EB8BF92AEB63BCB2AF834F208C8B157AF74FE449`.
- Source identity: `sharedassets0.assets:137`,
  `nanocom_message_nano`. This is an exact Texture2D decode, not a crop or
  visual substitute.

## Verification

- The publisher replayed into Editor-owned staging and the staged/target
  SHA-256 values were identical.
- `cargo test -p ffone-asset-pipeline --bin publish_exact_texture` passed.
- FFOne `cargo xtask assets --full` passed for 66,878 files and
  3,294,604,939 bytes.
- FFOne focused Rust tests are currently blocked before the Nanocom module by
  unrelated in-progress tutorial actor changes: missing
  `TutorialActorVisual`, `tutorial_npc_definition`, and associated imports.
  The affected Nanocom Rust files were formatted successfully, and asset,
  localization, exact byte, dimension, and publisher checks pass.

## Intentional adjacent limit

Clean starts `ownstatus.NanoTimeEffect()` after type-10 CommOut playback
begins. That minimap-ring effect is a separate visual owner and remains outside
this message-only change; no substitute animation is inferred here.
