# Warp screen presentation, 2026-09-08

Follow-up to `warp-departure-readiness-20260908.md`: the owner reports that NPC
warp/transport departure restores the HUD and loses the top/bottom black bars.

## Primary evidence

Reused the checked assembly export and decompilation at
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/`.
Raw primary `retrobution-20260821/main.unity3d` SHA-256:
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
The exported `Assembly-CSharp.dll` SHA-256:
`0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.

- `NpcIconMode.cs`, `DoNpcWindow`, lines 1616–1620: full-width top/bottom
  black rectangles, each `0.15673982 * Screen.height`.
- `NpcIconMode.OnGUI` disables controls while `fWarpWaitTime > 0` or
  `bSendFlag`; departure does not dispose the NPC screen frame.
- `NpcIconMode.WarpOK` calls `EndSubTarget` before the departure effect.
- `cnTrans.Trans` sets `bWarp` and ends the camera subtarget.
- `cnTrans.OnGUI` shows its window only when `!bWarp && !bSend`.

Decompiled SHA-256: `NpcIconMode.cs`
`60489BFD4E2F97CBC32C4FBF62B1A914A79251F390B0F7A6C82D0151A394FEE5`;
`cnTrans.cs` `84DE8B8D634DC3D7F6DC52AC63BCE5447F79E1A2471DB47A6D322E05F3D21C9F`.
These observations do not prove an independent cnTrans letterbox. Applying the
same frame to transportation is the owner's requested shared presentation.

## Native repair

Separate screen ownership from the NPC interaction panel and camera subtarget.
`MissionUiModel::npc_letterbox_visible` gates the existing native rectangles and
HUD chrome. The main HUD uses the same predicate. A production presentation
system follows NPC pending warp and transportation PendingWarp/AwaitingServer,
latching across authoritative destination loading and clearing on completion,
failure or leaving gameplay. Gameplay/chat input remains blocked during this
ownership. No new UI text or render asset is introduced.

Pending departure releases the NPC camera subtarget so its local-character
visibility rule cannot hide the departing avatar. Authored cinematic visibility
continues to have priority. Bind ordering updates the screen ownership before
HUD and mission presentation consumers.

## Verification

Regression coverage exercises real mission UI binding, the production HUD/player
visibility system, destination loading, rejection and leaving gameplay. The
transportation departure fixture also observes screen ownership. A deterministic
GPU preview mode `FFONE_MISSION_UI_PREVIEW=warp-departure` uses production mission
UI and HUD entities. Captures belong under FFOneClient `target/performance`.
Execution results:

- `cargo test -p ffone-client --bin ffone-client warp`: 17 passed, including the
  final extended transport reply/loading boundary fixture.
- `cargo test -p ffone-client --lib warp_presentation`: both tests passed,
  including the production UI binding and renderer-readiness regression.
- `cargo build -p ffone-client --bin ffone-client --example gameplay_hud_gpu_preview`:
  passed. The pre-existing unused `CameraProjection` import warning remains.
- The real `target/debug/ffone-client.exe` completed the opt-in offline
  `FFONE_PERF_OUTPUT=target/performance/warp-screen-20260908/client-smoke` run.
  It loaded and rendered the normal world on an RTX 3060/DX12 and wrote
  `report.json` and `frame.png`; no schedule graph or runtime panic occurred.
- The UI binding test initially had a missing test-only `AssetLocator` import;
  it was added before rerunning.
- Rebuilt and ran the GPU preview with an independent scene camera. The accepted
  `target/performance/warp-screen-20260908/departure.png` is 1264×681: full-width
  black bars occupy rows 0–106 and 574–680 (107 pixels each, matching the native
  percentage after raster rounding). The central backdrop is unobstructed and
  no HUD or NPC controls are visible. The earlier preview had no scene camera
  and was entirely black; that capture was rejected and replaced.

The full-client smoke is a normal world load, not a live server teleport capture.
No live-server visual parity certification is claimed.
