# Character creation audio lifecycle

Question: which native creation controls emit which UI sounds, and when must
creation-owned audio stop? This repair changes native event gating, gain and
source lifetime; it publishes no new assets and changes no layout or text.

## Authority

Canonical `primary`: `builds/retrobution-20260821/main.unity3d`, 8,221,718 bytes,
SHA-256 `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
The raw hash was rechecked against both managed extraction receipts.

- `Assembly - CSharp.dll`: SHA-256
  `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
  Receipt and verified payload are under
  `work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/`.
- `Assembly - CSharp - first pass.dll`: SHA-256
  `AFFF470BEC30BF06703EB0664DE364CE37EA7B8092C848420A92FBA63E91DA81`.
  Receipt and verified payload are under the same project's
  `reports/nano-summon-primary/` directory.

Focused IL inventory was regenerated from the verified payload:

```powershell
.\target-build\debug\ff-client-editor.exe fusionforge export-ui-interaction-evidence `
  work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/creation-audio-interaction.request.json `
  --out work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/creation-audio-interaction.evidence.json
```

Roots are `CnGuiCharCreation.OnGUI` and `CnGuiNameCreation.OnGUI`, depth 3.
This inventory is static evidence, not a claim of legacy runtime capture.

## Recovered contract

| Control | Cue |
| --- | --- |
| Height down / up | `Height_Down` / `Height_Up` |
| Build narrow / wide | `Girth_Narrow` / `Girth_Wide` |
| Skin, hair or eye color | `Select_Color` |
| Random appearance or generated name | `Randomize` |
| Continue appearance or name | `Continue_01` |
| Switch name tab | `Tab_Click01` |
| Change gender, hair, face, clothing; appearance exit/fullscreen | `ButtonSound`, random mouse_click01–05 |
| Camera controls, name scrolling, selected gender, name exit/fullscreen | No cue |

Both GUI owners gate input with `bGUIEnabled` and `IsSysPopUp`.
Inactive screens/tabs cannot emit their controls. The selected clothing center
is a label, not a button. Name Continue plays before local name validation;
an invalid custom name must not suppress that source-defined click.

`SoundUtil.ButtonSound` and `Playsound` use SFX volume times 0.7.
`PlayLoopSound`, despite the musical content, also uses SFXSound at that gain.
`CnCharNameCreationMode` and `CnCharCreationMode` start the creation loop and
call `StopUIModeSound` on exit. Native music-toggle support remains an extension.
Selection has a distinct MusicController owner (`CnCharSelectionMode` and
`CnGuiCharSelection.ResetMusicOff` in the same verified CSharp payload).
Its persistent native SelectionMusic source must also observe leaving selection,
otherwise it overlaps the creation loop after its presentation schedule is gated.

## Native implementation and verification

FFOne's `character_creation_ui.rs` gates sound by current screen, capability,
selected state and system-popup ownership; muted one-shots are not queued.
The audio system remains scheduled after the creation phase ends, updates
pending PlaybackSettings as well as live AudioSinks, and rewinds a playing loop
when leaving. It uses the shared SFX gain and Bevy's separate master gain.

Regression tests exercise actual AudioPlayer entities and selected handles,
single playback during a held button, hidden/blocked/selected/muted/popup cases,
and the production startup-phase transition even without a live audio device.
Device playback and legacy GPU captures are not established by these tests.

`cargo test -p ffone-client --lib character_creation_ui -- --nocapture`:
26 passed, including all three new audio regressions and semantic text ownership;
one existing image-dimension assertion failed because `CCCheckboxChecked.png`
is 24x23 while its static contract expects 24x22. No image or dimension contract
was modified in this audio repair. Rustfmt and the focused diff whitespace check passed.
The selection production-phase test was extended to assert that its pending
audio source pauses on the transition to CharacterCreation.
`cargo build -p ffone-client --bin ffone-client` completed successfully with
one unrelated unused `WORLD_MAP_DEFAULT_KEY` warning.
The freshly built library test executable (successful fingerprint invocation
03:06:09 and completion 03:09:13, both after the selection source edit at
02:53:57 on 2026-09-04 local time) passed the exact
`character_selection_ui::tests::production_defers_selection_assets_until_selection_phase`
test, including its new audio assertion. It was executed directly to avoid
another redundant Cargo-lock wait. The development executable also returned
success for `--help`; no live-device listening result is claimed.
