# Transport NPC portrait — 2026-09-05

Source: `retrobution`, canonical role `primary`, build 20260821. Native consumer:
FFOne's transportation presentation and production NPC runtime. No NPC definitions,
placements, shaders, fonts, voice or texture payloads are changed in this pass.

## Confirmed missing implementation

The transport shell created a black `90x90` camera slot, but no production adapter
ever assigned a camera image. A source-backed live portrait was therefore missing
regardless of whether the NPC's static icon existed.

The exact `main.unity3d` authority is 8,221,718 bytes, SHA256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
All objects below belong to `sharedassets0.assets` in that container:

| Object | Identity | Proven contract |
| --- | --- | --- |
| Transport controller | MonoBehaviour 1583, script 1051 `cnTrans`, owner 1345 | `pCameraPrefab` local PPtr to GameObject 1323 |
| Camera prefab | GameObject 1323 `NPC_RTCamera` | Transform 1229, Camera 1456, controller 1457 |
| Portrait camera | Camera 1456 | FOV 45 degrees, near .001, depth-only clear, mask 1024, no target texture; `cnTrans.Awake` overrides far to 5 |
| Camera controller | MonoBehaviour 1457, script 1154 `cnCharRenderCamera` | Distance .5, height 1.2, Euler angles zero |
| NPC container | MonoBehaviour 1501 | `pNpcPrefab` local Transform 1211, GameObject 1321 |
| NPC prefab | GameObject 1321 | `NpcMoveController`, `Status`, animation/effect components; no `cnAvatarStatus`, hence no player neck target |
| HNPC prefab | GameObject 1355, Transform 1185 | Same no-`cnAvatarStatus` camera fallback; type 964 (Numbuh 255) uses this composite-avatar branch |

Strict object reports are under
`work/projects/retrobution-ui-20260821.ffclient/reports/`:
`transport-mode-primary.evidence.json`, `transport-portrait-controller.evidence.json`,
`transport-portrait-camera.evidence.json`, `transport-npc-prefab.evidence.json`,
`transport-hnpc-prefab.evidence.json`.
Each was produced from the raw container using:

```powershell
.\target-build\debug\ff-client-editor.exe fusionforge dump-object-evidence primary ../builds/retrobution-20260821 main.unity3d <id> --serialized-asset sharedassets0.assets --type <type> --out <report>
```

Managed evidence reuses the independently hashed assembly extraction in
`reports/avatar-animation-primary/assembly-csharp.evidence.json`; DLL SHA256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
`cnTrans.InitMode` supplies the actual NPC transform. Its type-1 paint branch draws
the camera at `(20,34,90,90)`; other transport types draw the monkey texture.
`cnCharRenderCamera.LateUpdate` first positions the camera .5 units along avatar
forward, looks at the avatar root, and only then adds 1.2 world Y. This sequence
must not be replaced with a camera looking down at the root after adding height.
`AvatarUtil.DrawCamera` sets the UI pixel rect and calls Render during repaint.

## Native implementation

`app/transportation_portrait.rs` binds a transparent, owned 90x90 render target to
the existing shell slot. It uses the selected live NPC's meshes and current world
animation/materials. A native render-layer lease isolates that NPC and admits the
existing directional lighting, retaining every other layer bit and restoring the
implicit layer contract when appropriate. No model, material, mesh or texture is
duplicated. This is the native adapter for the source's separate NPC render mask;
nearby unrelated NPCs are intentionally excluded from the portrait.

Camera transforms update after world transform propagation and before visibility
checks. The camera uses the gameplay root's native -Z forward direction. Missing
NPCs/meshes wait without creating a camera. Closing the menu, changing service or
losing the target releases the camera and render target, and removes only the
leased layer. The shell remains localized and retains its original rectangle;
its black placeholder background is replaced with the live transparent image.

## Transport icon test correction

The old test expected older PNG encoding bytes. Both installed icons actually
match fresh exact exports from the primary raw container, with unique Texture2D
identities verified across its serialized files:

| Native file | Texture2D | Bytes | SHA256 |
| --- | --- | --- | --- |
| `ui/gameplay/journal/npcicon_10.png` | 333 `npcicon_monkey` | 2058 | `460cf57f93ae39fd3bdd03aa866fa4c0171e90f932d34684ec86cd80aa15b576` |
| `ui/gameplay/journal/npcicon_11.png` | 71 `npcicon_scamp` | 1270 | `1af2fb6cdfa82f9f8799eb3d339b4e13d60a745e98ba9266e093fec4314eec9f` |

Replay `fusionforge export-exact-texture ../builds/retrobution-20260821/main.unity3d
<id> <report>` from FFClientEditor. Reports:
`transport-monkey-icon.exact.json`, `transport-scamper-icon.exact.json`, comparison
`transport-icon-verification.json` below the same reports directory. Base64-decoded
base-mip PNG payloads equal the installed native files byte for byte. Corrected
only the test expectations; no publication or new binary receipt is necessary.

## Acceptance

- Two production-adapter tests cover pose order, delayed model availability,
  target-image binding/reuse, following the NPC, and layer/resource cleanup.
- 57 existing economy/transport integration tests passed.
- 50 mission UI tests passed, including both transport service icon proofs.
- Transportation shell tests: 13 passed; two pre-existing failures still assert
  historical complete-table bytes and the historical texture-set digest. The same
  failures occur in `target/parity-tests-after-publication.log`. Their expectations
  were not bulk-rewritten in this pass.
- The initial full-app fixture waited for `NetworkNpcVisual0104` only and timed
  out with a fully loaded world: type 964 is an HNPC. Corrected fixture selection
  to include `NetworkHnpcVisual0104`; the production portrait already uses the
  shared NPC appearance/root and its mesh subtree for both branches.
- The corrected HNPC fixture first captured a hidden menu. Diagnostics showed
  ready assets but a reset model; the offline fixture now reopens after a reset
  and requires a continuously visible, loaded menu with a bound image. Normal
  network session behavior was not changed. Earlier timeout/hidden-menu captures
  are not accepted visual evidence.
- Final ordinary client build passed. Final production portrait tests passed
  again (2/2) after the fixture changes. The focused suites total 122 passed and
  the two historical shell-fixture failures above.
- Full-client DX12 captures on RTX 3060 each recorded 600 frames and were visually
  inspected at 1920x1080. Both show Numbuh 255's live portrait in the header with
  transparent surroundings, in Russian and English. Neither log contains an
  ERROR or panic; unrelated pre-existing missing-NPC catalog warnings remain.
  These are presentation checks, not a baseline/performance improvement claim.

Accepted native capture files (under FFOneClient):

| Locale | File | SHA256 |
| --- | --- | --- |
| RU | `target/performance/transport-portrait-accepted-ru-20260905/frame.png` | `de0b7c18602d31333ad3cce426cfbba878bf13f000a94ab65154d3dbdb951c63` |
| EN | `target/performance/transport-portrait-accepted-en-20260905/frame.png` | `1e766488ac5ff590af6e53ad9c5e2c47765dc1663caa0eb8cc6c8ee048d50942` |

Replay the native opt-in full-client fixture with
`FFONE_PERF_TRANSPORT_NPC_TYPE=964`,
`FFONE_PERF_POSITION="-3750.5798 -56.9 4484.4097"`, a fresh
`FFONE_PERF_OUTPUT=target/performance/<capture-name>`, and CLI
`--asset-root assets/game --language ru` (or `en`). This uses existing waypoint
rows through transient typed ingress; it does not write placements.

The runtime bug was the missing camera adapter. Exact icon re-export confirmed
the editor's current payloads; no editor extraction defect was found in this
slice. This case does not close the wider transport/teleport backlog or claim
that existing destination/region localization is complete.
