> Owner correction — 2026-09-08: the subsequent wrist/palm height alignment is superseded and removed. The accepted Monkey placement is the original Broomstick socket position, holding the character at shoulder level, with the corrected world scale. The zipline handle remains unchanged. Contact-alignment results below are historical, not the current acceptance contract.

# Player traversal attachments (primary, 2026-09-07)

Question: which placement contract keeps Monkey Skyway at its original size and
seat position during flight, and which object supplies the missing zipline handle
to FFOne's selected-player rig?

Authority is current primary `retrobution-20260821`. This is a primary runtime
repair, with no alternate donor. Evidence and staging are under
`work/cases/traversal-attachments-20260907`; publication hashes and replay are in
`recipes/native/models/zipline-trolley-primary-20260907.json`.

## Skyway

Fresh `export-managed-assembly-evidence` of `main.unity3d`, exact entry
`Assembly - CSharp.dll`, matches the earlier avatar-animation evidence payload:
SHA-256 `0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
FFSpy's `cnAvatarAnimation.LoadBroomMonkey` instantiates `wear/item_broom_01.nif`
at `Bip01 Broomstick.position`, with `Bip01 Broomstick.rotation * Rx(90°)`, and
then assigns the parent while retaining the world transform. It does **not**
call `ActorSkinCombiner.AttachGO` or force the socket's local scale to one.

The native male socket's authored local scale is approximately 0.3647636.
The previous FFOne attachment used ordinary equipment's local unit scale,
shrinking both the monkey and its internal offsets toward that socket. The
model's published root is already identity; an extra root translation or
rotation correction would be wrong. No Skyway model bytes were changed.

The native contract is: sample the animated socket's world transform after
Bevy transform propagation, create the model at that position and the converted
90-degree local rotation with unit world scale, and reparent while preserving
that world pose. Retain the resulting local transform for subsequent socket
animation. Continue looping the model's `nif-default` animation. Destroy the
attachment on dismount or when its owning rig ceases to be active.

### Owner-requested height correction

World-scale-preserving parenting alone did not satisfy hand contact. The owner
reported the remaining vertical mismatch after the first native preview.
`skyway-grip-before.log` measures player wrist heights 1.3862/1.3787 versus monkey
wrists 1.2878/1.2786: approximately 0.10 native units too low. The earlier visual
inspection was insufficient to accept wrist alignment.

FFOne therefore adds an explicit **native contact extension**, rather than
claiming this correction was present in the primary script: the midpoint of the
two monkey palm grip points is aligned vertically to the midpoint of the two player palm grip points
after animation evaluation and before transform propagation. It changes only
world Y translation. Scale, rotation, horizontal placement, source clips,
controller position, and network ownership stay with their existing owners.
The grip resolves the two exact monkey hierarchy paths once on scene readiness,
and the player's canonical gender-rooted hand paths through NativePlayerRigBones.
The grip point is `Bip01 L/R Finger2`, the middle-finger base at the palm edge.
A wrist-only intermediate was rejected after inspection: differently sized palms
have different offsets from their wrist origins, so equal wrist heights did not
place the visible palms at equal heights.
It does not search all world materials or allocate a new lookup cache each frame.
The native schedule regression moves the hand targets between frames under a
rotated, nonuniformly scaled parent and checks current-frame contact and unchanged
horizontal placement/rotation/scale.

Final palm-edge captures are `skyway-palms-male.png` (height/body 2/1) and
`skyway-palms-female.png` (height/body 4/2). Their measured per-hand vertical
errors are approximately 0.00031 and 0.00064 native units respectively; mean
height errors are below 0.000001. Both images were inspected and both attachments
were released after dismount. The final zipline regression also passed.
The final development binary builds successfully with the palm bindings.
All 25 rig runtime tests passed; the numeric constraint implementation is
unchanged between wrist and final palm binding verification. See
`docs/reference/evidence/cases/skyway-hand-contact-20260907.json` for hashes and replay commands.

## Zipline

`cnAvatarStatus.LoadZiplineObject` supplies `wear/pistol_trolley.nif` to
attachment slot 5. `ActorSkinCombiner.Generate` resolves slot 5 to the right
hand's `Bip01 Rweapon01` full path. `AttachGO` uses zero local translation,
`Rx(90°)`, unit item scale, and unit socket scale. Movement activates slot 5
during zipline traversal and hides it on release. This differs intentionally
from Skyway's world-scale-preserving parenting.

The exact model is GameObject 147411 in
`CharacterSelection.resourceFile / CustomAssetBundle-ce09c4c9be8a046ca92e0044f22d1b99`.
The strict owner evidence resolves all direct references without unresolved
pointers. Main texture is Texture2D 407 in
`NpcTexture.resourceFile / CustomAssetBundle-d304c52c4bae348e38c743762c1bd818`;
the ramp comes from Texture2D 53 in
`CharacterCreation.resourceFile / CustomAssetBundle-bd5f53480423447d7bcaed95cb2a96c8`.

FFOne creates this native GLB on the active rig while its controller's
presentation is `Zipline`; candidate apparel rigs cannot acquire it. Leaving
the state or replacing the active rig releases the scene and its handles.
The hand's ordinary equipment remains controlled by the existing weapon
visibility state. No UI labels or audio spawns were added.

## Verification

- Two independent native publications produced identical GLB/PNG bytes (11 files).
- The tracked replay command verifies the accepted hashes before any installation.
- Standalone native GPU capture: 1 mesh, 1 material, 9 exact mip levels,
  no material or shader errors; image inspected in
  `gpu/models/traversal/pistol_trolley.gpu.png`.
- Nine focused native tests pass, including world-scale/position preservation,
  traversal model identity roots, and the existing traversal animation contracts.
- `tutorial_player_rig_gpu_preview` has `skyway` and `zipline` cases using the
  production selected-player spawn, animation, material and attachment systems.
  They capture the animated state and require attachment removal after dismount.
  Both cases passed and their images were inspected (`skyway-player.png` and
  `zipline-player.png`). Skyway also passed with height selector 0. The monkey
  sits above the player, but still had the vertical grip mismatch documented
  above. The trolley
  spans the two raised hands in the zipline pose. Each scene was released after
  leaving traversal. During the captured animated pose both attachments had
  world scale approximately 0.99875, retaining subsequent animation as intended.
- All 24 selected-player rig runtime tests pass. The development asset graph
  validates 5 domains and 43,684 references.
- The real `ffone-client` development binary and preview example build with
  `CARGO_INCREMENTAL=0`. An initial link failed on undefined internal LLVM/Bevy
  symbols in a cached library; the nonincremental rebuild succeeded.

GPU model loading and native fixture captures do not by themselves prove every
live server route or a frame-for-frame comparison with a running Unity client.
