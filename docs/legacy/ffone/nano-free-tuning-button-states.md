# Nano Free Tuning button states

Question: why do the three SELECT buttons not visibly react to the pointer in
FFOne, and what are their actual primary states? Consumer: the production
`NanoFreeTuningUiPlugin`. Acceptance: real Bevy hit-testing over the label,
press/release cancellation, disabled input rejection, EN/RU GPU frames.

## Authority and focused recovery

Primary: `builds/retrobution-20260821/main.unity3d`, 8221718 bytes,
SHA-256 `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
`sharedassets0.assets`, MonoBehaviour 1379, `FusionFallNanoSkin`, raw object
SHA-256 `5D7191BC489C6AE0C202DD4DFBA00BBF47602741C00AC217267C75C885DCCEA6`.
The current primary preserves the earlier button state pointers/geometry;
the installed button textures match its RGBA32 payloads, not the older
20260613 DXT5 PNG byte hashes. No runtime payload is changed by this repair.

From FusionForge, with `target-build/debug/fusionforge.exe fusionforge`:

```text
dump-object-evidence primary ../builds/retrobution-20260821 main.unity3d 1379 --serialized-asset sharedassets0.assets --type MonoBehaviour --allow-unresolved-pointers --out work/legacy-sources/nano-button-states/skin.triage.json
export-exact-texture ../builds/retrobution-20260821/main.unity3d 309 work/legacy-sources/nano-button-states/hover.texture.json
export-exact-texture ../builds/retrobution-20260821/main.unity3d 640 work/legacy-sources/nano-button-states/normal.texture.json
export-ui-interaction-evidence work/legacy-sources/gm-chat-commands-primary/nano-ui-interaction.request.json --out work/legacy-sources/gm-chat-commands-primary/reports/nano-ui-interaction.evidence.json
```

Exporter revision: `86324d05303a076b9814bddff800635cb2f73954`.
Strict whole-skin export fails on the absent built-in Unity resource external.
The triage report is NOT a publishable whole-skin proof: 32 built-in references
remain unresolved. The reached `m_button.m_Normal/m_Hover/m_Active` backgrounds
are local fileID 0 references to 640/309/640; those resolutions are recorded
and the exact texture exports are independently matched to installed PNGs.
Unused `m_On*` states must not be substituted: the owner calls `GUI.Button`,
not a toggle. No claim about those built-in states is made.

Managed authority is the existing exact assembly report in
`work/legacy-sources/gm-chat-commands-primary/reports/assembly-csharp.evidence.json`;
payload SHA-256 `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
The static interaction report locates the three `GUI.Button` calls in
`CnGuiNanoFreeTuning.OnGUI` and the `bGUIEnabled` gate; it is static IL evidence,
not a legacy runtime capture.

## Control ledger

All three buttons use local Rects `(237,145,97,20)`, `(237,232,97,20)`,
`(237,317,97,20)`, nine-slice borders L/R/T/B `6/6/6/4`, padding `6/6/3/3`.
The label retains `ui.nano_free_tuning.select`, the approved JEFFE replacement,
and the existing EN/RU geometry. Each button follows its row's description in
paint order; labels are visual children and must pass pointer/focus hits.

| State | Background | RGBA text | Input |
| --- | --- | --- | --- |
| Normal | local Texture2D 640 | .8,1,1,1 | Idle |
| Hover | local Texture2D 309 | 0,.2784314,.47843137,1 | No request |
| Pressed/active | local Texture2D 640 | .8,1,1,1 | Arm until release |
| Release inside same button | hover | hover color | One semantic selection |
| Release outside / disable / hide | normal | normal color | Cancel armed click |
| Awaiting authoritative response | normal | normal color, alpha .5 | Reject input |

Disabled alpha .5 is explicit native feedback, not a recovered serialized
disabled GUIStyle (the skin has no such slot). Native action still requires
the model's enabled gate; disabled buttons and the modal backdrop block
underlying world interaction.

Installed normal PNG SHA-256:
`506EA52CF108EEB14A035FA52A4C650BA1A5255B6E4FB62A10ADC3AEDFC72C8E`.
Installed hover PNG SHA-256:
`1EFF6610DB23CFFDB5E454708141B56A10A4E60CDFB7D64357EFE0C6C30B1798`.

## Native failure and repair

`Button` requires `Interaction` and `FocusPolicy::Block`, but not `Pickable`.
The visual query required `&mut Pickable`, so none of the spawned buttons
matched. `Pickable::IGNORE` on decoration also does not control the separate
Bevy UI focus system: labels need `FocusPolicy::Pass`. Both are now explicit.
The hover text color is synchronized with the image. A local armed entity
and `RelativeCursorPosition` implement release-inside confirmation; a press
no longer sends a request immediately.

GPU reproduction uses the production plugin and real `Window` cursor position
before `UiSystems::Focus`, not an injected `Interaction`. Set
`FFONE_NANO_BUTTON_STATE=normal|hover|pressed|disabled` and run
`nano_free_tuning_gpu_preview en|ru <absolute-output.png>`.
Captures and verification results are retained below
`work/legacy-sources/nano-button-states`.
