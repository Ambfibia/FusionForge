# Vendor live portrait — 20260907

Authority: Retrobution 20260821 / primary. Question: which actual camera does Panel_Vendor use, which NPCs are eligible, and how does native ownership release the target?

The name-matched NPC_RTCamera candidate was rejected. The decisive chain in main.unity3d / sharedassets0.assets is GameObject1324 -> MonoBehaviour1411 (Panel_Vendor, script1097) -> cNPC local PPtr1413 (Camera) -> GameObject1325 -> MonoBehaviour1414 (cnCharRenderCamera, script1154). Direct raw reports resolve pointers without ambiguity. Candidate1456/1457 has different framing and must not be reused.

Accepted camera: FOV70, near0.2, far2.0, distance0.4, height1.1, yaw-20 in Unity. Runtime panel Init overrides serialized ownposition x230 with x310; final area is (310,0,200,150). SetNpc enables bViewNpc only when m_iHNpc>0. Native HNPC visual admission is the corresponding runtime gate. Root fallback and LookAt-before-height follow cnCharRenderCamera; native basis uses the established character-forward conversion. Transparent independent target replaces immediate depth-only rendering into the parent framebuffer; no NPC/model clone or material edits.

Runtime changes: fifth ServicePortraitSlot Vendor; match live session requested NPC identity and require NetworkHnpcVisual0104; wait for mesh children; retain camera/target while active; release image refs, camera and leased render bits on close/loss. UI owns one ImageNode with transparent unbound tint and passes pointer focus. No static labels or localized voice added.

Evidence commands (Editor cwd): fusionforge.cmd dump-object-evidence primary ../builds/retrobution-20260821 main.unity3d <id> --serialized-asset sharedassets0.assets --type <type> --out work/legacy-sources/whole-client-audit-20260905/<report>.evidence.json. IDs/types:1324/GameObject,1411/MonoBehaviour,1413/Camera,1325/GameObject,1414/MonoBehaviour. Reuse hashed main-decompiled/Panel_Vendor.cs and cnCharRenderCamera.cs from the whole-client static audit.

Verification is pending final visual capture. Offline fixture FFONE_PERF_VENDOR_PORTRAIT=1 uses the real streamed HNPC and service-camera owner and emits no vendor request. It sets an empty catalog session only to expose the portrait independently from server purchases. Initial empty frame rejected; diagnostics retained in ignored work. Do not count it as visual acceptance.

## Accepted correction and verification

The empty first frame was caused by late shared-rig reparenting restoring RenderLayers(0) after the service lease was recorded. The old equality shortcut did not verify the actual bits. The owner now reapplies missing owned bits while preserving foreign bits, with a regression test. No mesh/material duplication or visibility-rule reduction.

Four current production-bin camera tests pass. cargo check for the Vendor example passes; the actual Dev binary builds. Final Russian GPU frame shows the streamed HNPC (fixture NPC163, table793) in the exact Vendor surface. Fixture inventory/catalog are intentionally empty and do not test purchases. EN capture and final receipt follow. No new binary payload publication is involved.

Final EN and RU production captures both exit0 and were visually reviewed. Diagnostics confirm admitted meshes retain layers0+36 and the bound camera uses FOV70. Receipt: artifacts/shared-ui-owners/vendor-portrait/acceptance.json. Replay from native cwd: FFONE_PERF_OUTPUT=target/performance/vendor-portrait-<locale>, FFONE_PERF_NPCS=1, FFONE_PERF_VENDOR_PORTRAIT=1, FFONE_PERF_POSITION="-7029.7 -52.88 893.78", empty FFONE_USERNAME; target/debug/ffone-client.exe --asset-root <native>/assets/game --language <locale>.
