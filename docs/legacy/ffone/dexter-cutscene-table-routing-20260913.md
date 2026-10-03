# Dexter in character-creation cutscenes

Both FFOne character-creation cutscenes previously bypassed the NPC table and
loaded `npc_dexter` directly. Consequently, the earlier fixes to NPCs 753/754
and the main Dexter row did not update their appearance. They also requested
`inven`, which is absent from the current `npc_dexter2` model.

The primary `main.unity3d` assembly was exported again with
`export-managed-assembly-evidence`, then `cnDexterShipEventScene` was decompiled
with preserved coroutine state machines. Its `InitMode` loads
`Mob/npc_dexter2.kfm` and assigns `npc_dexter` / `npc_dexter2_face_0` textures.
`NameEvent` requests `observe`, then `cutscene_stand_1`; `TutorialEvent`
requests `excellent`, `deedeeno`, then `busyrun`. Native scene timing is
preserved while these event clips replace the stale selections.

The owner's additional request makes appearance table-driven in FFOne:

| Cinematic role | Native NPC row |
| --- | --- |
| Dexter | 728 |
| Dee Dee | 701 |
| Computress | 730 |

Both preloading and scene spawning resolve these roles through the shared
`NetworkNpcVisualCatalog0104`. No separate model paths remain in the cinematic.
GLB selection, main/sub textures, sampler settings and scale come from the same
definition as the ordinary NPC. The cinematic still owns its transforms,
animation clock and lifetime, so it does not gain gameplay facing rotation,
colliders, network authority or appearance effects.

`NpcSceneTextureOverrides0104` allows cinematic roots to use the existing NPC
texture binder, including primitive write permissions, immutable-material
cloning and companion render passes. Characters are revealed only after the
table-selected textures and first poses are ready. Missing model routes or
animation clips block loading with a diagnostic.

Do not preload NPC overrides using plain `AssetServer::load<Image>`: when the
override path is also a glTF image, that default-sampler request can win the
asset cache race and invalidate the exact sampler descriptor. The shared binder
owns those loads with their native sampler/color settings. The presentation gate
also waits for the resulting image assets.

Dee Dee 701 continues selecting the owner's historical `npc_deedee` model and
atlas. Primary's `npc_deedee2` selection is intentionally not adopted. Fusion
Dee Dee is outside this change. No model or texture payload is rewritten.

Evidence, raw assembly hashes and verification are recorded in
`docs/reference/evidence/cases/dexter-cutscene-table-routing-20260913.json`. Raw extraction,
decompiler output, render captures and logs remain below
`work/cases/dexter-cutscenes-20260913`.

The opt-in native `FFONE_CUTSCENE_PROBE_OUTPUT` fixture enters both production
cutscenes offline, captures three frames before creation and four after, and
records the resolved table identities, model paths, texture paths and active
animation. It exits before a network transition or character creation request.
Each sample includes the independent 512x512 camera target (`*-actors.png`), so
minimizing the main window cannot invalidate visual acceptance. Full-window
captures may be 1x1 when Windows minimizes that window and are not accepted then.
Launch the built native executable from an Editor-owned command with FFOneClient
as its process working directory, an explicit native asset root and an output
directory below the case's `captures` directory. The runtime's startup asset
validation still expects its own workspace working directory.

Final verification passed: eight cinematic regression tests, the companion-pass
texture test, the native client build and asset validation. The production
offline fixture captured all seven timeline samples, each with a valid full
window image and a 512x512 actor target. Diagnostics select NPC 728 and
npc_dexter2 in every sample; representative frames and final event poses were
visually inspected. Accepted artifacts are in
work/cases/dexter-cutscenes-20260913/captures-final.
The four character model payload hashes remain unchanged.
