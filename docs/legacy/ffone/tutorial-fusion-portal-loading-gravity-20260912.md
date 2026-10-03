# Tutorial Fusion Portal: gravity during warp loading

## Question and source

Does the primary client apply gravity to tutorial NPC 2695 (Fusion Portal,
runtime actor 1012) while a destination world is still loading? Native consumer:
FFOne `tutorial_actors::ground_tutorial_actors`. Acceptance: a delayed warp must
preserve the portal's spawn height and vertical velocity, then settle normally
after collision is ready. No alternate donor or binary payload change.

Reused strict managed export:
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/assembly-csharp.evidence.json`.
Hashes rechecked on 2026-09-12:

| Artifact | SHA-256 |
| --- | --- |
| Primary `retrobution-20260821/main.unity3d` (8,221,718 bytes) | `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF` |
| Exact level-0 `Assembly - CSharp.dll` (1,762,816 bytes) | `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792` |
| Export-adjacent `decompiled/NpcMoveController.cs` | `6B3A249BF2CFD52BA17D099361033FF408A6A04AFD39EE2547DCA36E00A7F4A8` |

The strict evidence file records the exact-entry exporter, source role `primary`,
selector, hashes and replay inputs. These are the same source and decompilation
identified in `npc-dialogue-audio-lifecycle.md`.

## Proven contract

`NpcMoveController.UpdateMove` lines 2381–2429 wraps the vertical ray and gravity
in `GameFrame.IsReadyForPlay()`. The false branch sets `val.y = 0`, leaving the
stored vertical velocity untouched. The ready branch uses a downward ray from
root plus NPC height, distance height plus 10; gravity is 10 and terminal downward
speed is 10. This applies in tutorial mode too.

FFOne already uses the local player's `LegacyWorldColliderPending` marker for
this gameplay-ready boundary. Network warps set it before the destination is ready.
The actor grounding system previously ignored it, so an actor spawned during a
slow load could fall below the eventual floor; its downward ray could no longer
reach that floor. The native correction pauses vertical integration while a
local player controller carries that marker, preserving Y and velocity. The
existing ready-world gravity and ray contract remain unchanged.

This establishes a missing loading guard, not yet the reported disappearance's
root cause. The tutorial-local warp in `app/tutorial_warp.rs` only changes the
player transform inside the current tile and does not set that marker. Its
separate collision/render reproduction must pass before closing the case.

The raw collider at the portal position intersects Y=-91.71234 and has a
downward raw-GLB normal. This is **not** evidence of incorrect runtime winding:
`AuthoredColliderCooking::ExactTriangleMesh` reverses the triangle order before
ground tests, producing upward support. Do not change winding or make collision
two-sided from the raw-GLB observation.

## Native checks

The regression `fusion_portal_waits_for_warp_collision_before_applying_gravity`
uses the production tutorial NPC row and portal spawn coordinates, delays world
collision for five simulated seconds, then installs a floor and releases the
loading barrier. Expected Y is -90 during loading and -91 after settling.

The native `fusion_portal_gpu_preview` exercises catalog routing, character-root
normalization, material conversion, texture binding and deferred scene reveal.
Its captures belong below FFOne's ignored `target/performance/fusiongate`.
The existing logical-model GPU preview independently loaded the production GLB,
played `stand1`, applied all three exact texture chains (26 mip levels), and
reported zero material/shader errors. This verifies native asset rendering; it
does not by itself reproduce a complete user tutorial session.

The production unit regression passed in `ffone_client-735e7cd1cecfebb2` on
2026-09-12. The complete tutorial-actor group yielded 40 passes and two unrelated
catalog expectation failures for NPC 2673: the current content routes to
`npc_dexter2` / `npc_dexter2_face_0`, while those existing tests expect
`npc_dexter` / `npc_dexter_glass`.

The offline world-mode probe loaded the actual tutorial location and spawned
actor 1012 through `TutorialActorCommandQueue`. After 600 revealed frames its
position was `(-564.31, -91.71234, 724.16)`, all six resident world roots were
Ready, and the portal material was applied and texture-bound. The inspected
capture shows green tentacles on the portal's floor. Earlier blank harness
captures were invalid: without a `LegacyOrbitCamera` target, world streaming
unloaded the scene. The corrected harness installs that camera and native world
behaviour/effect systems, and refuses to count an empty NPC query as success.

These checks exclude a generally missing native model or missing support at
the portal coordinates. They do not exercise every ordinary tutorial UI/event
transition or the full client's NPC appearance-effect schedule. The reported
ordinary-play disappearance remains unconfirmed by this reproduction.

The same world probe also passed after deleting actor 1012 at 90 revealed
frames and recreating it through the command queue with the same ID/model.
After another 600 revealed frames its Y remained -91.71234; the inspected
`recreated.png` capture again shows the tentacles. This checks asset lifetime
and re-creation, not execution of the entire InfectionA choreography.
