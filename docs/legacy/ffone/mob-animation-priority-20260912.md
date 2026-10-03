# Mob animation ownership and interruption

Evidence question: why do ordinary native mobs, particularly Bad Max, appear
to restart damage/attack animations and choose the wrong continuation when
attack, damage, movement and skill packets arrive close together?
Authority: primary Retrobution 20260821. Native consumers: FFOneClient
`entity_lifecycle` and `network_world_runtime`.

## Reproducible managed authority

Raw `main.unity3d`: 8,221,718 bytes, SHA-256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
Exact UnityWeb level-0 entry `Assembly - CSharp.dll`: 1,762,816 bytes, SHA-256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
Re-exported from the raw primary container for this case; no patched or Academy
assembly is used. The assembly matches the earlier HNPC evidence, so this is a
native implementation defect rather than evidence of a new source revision.

From FusionForge:

```powershell
New-Item -ItemType Directory -Force work/cases/mob-animation-priority-20260912/managed, work/cases/mob-animation-priority-20260912/decompiled
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/cases/mob-animation-priority-20260912/assembly.evidence.json --payload-out work/cases/mob-animation-priority-20260912/managed/Assembly-CSharp.dll
foreach ($type in 'NpcAnimation', 'NpcMoveController', 'GameFrame') {
  dotnet vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.dll work/cases/mob-animation-priority-20260912/managed/Assembly-CSharp.dll -t $type -o work/cases/mob-animation-priority-20260912/decompiled --preserve-iterator-state-machines
}
```

FusionForge revision: `5d592c4e0c08d4673b5c4ed378cd87a066437a7a`.
FFSpy revision: `045e563f5c6c503ae21899e69bc6e36ee39ecd9a`.
Generated C# remains in ignored Editor work. Acceptance hashes:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| NpcAnimation.decompiled.cs | 19860 | 627fbb5eb9b87d5973c7927a02b26dd25531e6cb48600dd3698608822cc2e408 |
| NpcMoveController.decompiled.cs | 105565 | 291c3ddfce35e4763826bf1eb5c1faa6450c00bf4e89b08108ad71a916b60af8 |
| GameFrame.decompiled.cs | 203220 | ab958e5eac4679bc726d1273878203adae3bf9251eda55d7ccf750d44564b667 |

## Proven contract

- `NpcMoveController.Awake` resolves `NpcAnimation` on the same component
  owner. Its event dispatch routes movement, damage, attack and skill-ready
  callbacks to that owner. `GameFrame` dispatches NPC skill READY and HIT;
  FIRE has no animation callback in its packet switch.
- `NpcAnimation.MakeUperLayer` gives melee variants additive layer 102 and
  wound additive layer 104, both Once. Ordinary base clips loop. The two
  high layers are independent of each other and of the base animation.
- `AttackMotion` and `DamageMotion` call `Animation.Play` on the high layer,
  updating the current high name. They do not replace the current low name.
  Unity 3.5.3 `Animation.Play` defaults to StopSameLayer and explicitly does
  not rewind an already playing destination:
  https://docs.unity3d.com/353/Documentation/ScriptReference/Animation.Play.html
- Only an `end` from the current high name calls `AttackReady`. An older
  attack or wound may finish its visual tail, but its event does not own the
  new state. Low-layer skill/corruption `end` selects stand. Death ignores
  high-layer continuation and stays terminal; DeadMotion also plays wound.
- `AttackMotion` sets `bStandAttack`. `NpcMoveController.ForceUpdate` holds
  the movement delta while this flag is set. `AttackReady` clears it. A held
  destination is not an arrival and must not be discarded by the native mover.
- `NpcMoveController.SkillHit` selects the NPC table's Mega/ActiveSkill1/
  ActiveSkill2/Support slot in that order and passes its animation number to
  `NpcAnimation.Skill`. Numbers 0, 1, 2, 3 select skill0, skill1, skill2,
  skill3. `MegaAttackMotion` names skill0, not megatak. Unknown skill IDs have
  no matched animation callback. Mega-ready selection additionally excludes
  skill type 23; other skills use readyspell with ready fallback.
- Native production NPC 461 (Bad Max) selects skill 114 with animation 1.
  Primary table row 461 uses mesh row 218, mob_ammonia, scale 4.8. The native
  table preserves this route. Its wound ends at 0.41666669 s (duration
  0.63333344 s); melee1 ends at 0.61666667 s (duration 0.83333361 s).
  A fresh primary World_shared_part5 mob/mob_ammonia.kfm export confirms all
  13 native clip names, durations and event arrays. Both source and native
  model lack skill1 despite that table request: the native missing-clip
  recovery is exercised, not a fabricated skill0 substitution. The initial
  junkdinosaur test assumption was rejected; final tests use mob_ammonia and
  assert the production catalog route.

## Native corrections and scope

Keep a persistent request counter, independent low/melee/wound lanes and the
latest high end owner. A completed transient component must not recycle the
identity of the next request. Retain existing clip time and sound/effect cursor
when Play selects an already active clip. Only competing melee variants replace
one another. Keep separate sound/effect cursors for the playing base and both
high layers, including their tails. Select skills from the in-memory table
catalog. Runtime assets, geometry, textures and native content bytes are unchanged.

Missing high clips or missing end events retain a native recovery path at
playback completion; that robustness fallback is not evidence of a Unity
callback that does not exist. The existing explicit skill-cancel handling is
also retained as a native extension; primary GameFrame has no corresponding
animation branch. No synchronized Unity-versus-Bevy video or pixel-exact
blend-curve claim is made.

## Acceptance

Use the real production Bad Max GLB with Bevy's GLTF loader, scene spawner,
animation graph and a fixed 60 Hz clock. Test repeated damage, both orders of
attack/damage (including one packet batch), stale end callbacks, base-skill
preservation, skill-to-stand continuation, movement hold/release, new request
identity after completion and terminal death. Verify the production table's
Bad Max skill route and the packet lifecycle independently. Build the actual
development client after the final source changes. Results are recorded in
the associated case; generated logs belong below ignored target/performance.

The opt-in full-client fixture uses `FFONE_PERF_MOB_ANIMATION=1` alongside
`FFONE_PERF_OUTPUT=target/performance/mob-animation-20260912/runtime-final`. It
spawns native NPC 461 through the offline lifecycle ingress after world
loading, feeds attack/damage/skill/death packets, saves seven renderer captures
and records actual active graph nodes in `mob-animation-replay.json`. This
scenario is for temporal/render acceptance, not an FPS comparison. Reused
visuals also have a regression ensuring a new spawn clears old high layers
while keeping monotonically distinct request identities.

Primary identity replay from FusionForge:

```powershell
./fusionforge.cmd list-contents ../builds/retrobution-20260821/TableData.resourceFile
./fusionforge.cmd dump-xdt ../builds/retrobution-20260821/TableData.resourceFile 7 work/cases/mob-animation-priority-20260912/primary-xdt.json
./fusionforge.cmd export-logical-model-source ../builds/retrobution-20260821/World_shared_part5.resourceFile mob/mob_ammonia.kfm work/cases/mob-animation-priority-20260912/mob-ammonia-source.json work/projects/retrobution-ui-20260821.ffclient
```

The existing primary navigation cache resolves cross-bundle dependencies; the
model export is selected from the raw primary bundle and its exact container
route. No native model or table rewrite is part of this repair.

## Verified result

Final production-rig suite: 51 network-world tests and 28 lifecycle tests pass.
The actual development client builds successfully (4 domain catalogs, 43,818
resolved direct references). Full-client acceptance uses positive offline NPC
identity 1904461, type 461, and retains its table scale. The diagnostic placement
is moved away from the fixed player camera to fit the large mob. Its seven
captures are partly occluded by existing scenery; they are supporting render
checks, not a pixel-parity comparison or performance benchmark.

The 112 recorded graph samples prove wound seek continuity (0.066667, 0.150000,
0.233333, 0.316667, 0.400000 seconds across repeated hits), melee/wound overlap,
AttackReady continuation, same-name melee no-rewind, missing skill1 recovery
without substituting skill0, and finished terminal death at sample frame 560.
Acceptance artifacts live in FFOneClient's ignored
`target/performance/mob-animation-20260912/runtime-final`. The earlier preview
runs are superseded: one assumed the wrong model and another used a negative
fixture identity rejected by the strict skill-packet validator.

An earlier broad `npc_` sweep had two independently reproduced unrelated
failures: localized capitalization of the oil-ogre name and the production
icon coverage set's additional effect 670. The final affected-system suites
are clean. Details, artifact hashes and commands are in the associated case.
