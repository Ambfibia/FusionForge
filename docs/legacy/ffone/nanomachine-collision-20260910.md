# Nanomachine collision front faces

Evidence question: does the native Nanomachine sidecar preserve the primary
cooked collision mesh's exterior under the native X reflection, and can the
native capsule land on it and collide with it from outside?

The primary source remains `World_shared_part5.resourceFile` (23,289,685 bytes,
SHA-256 `29b0b4c238077a36dd8f328e54ef8d9cd6bd3720a74f0a9faaa3b66c15459fb1`).
In serialized asset `CustomAssetBundle-bf22cf134e84044c18d80e6c70a99425`,
MeshCollider 19431 points locally to Mesh 354, under `mob/nanomachine.kfm`.
Fresh strict FusionForge exports exactly match the hashes in publication
`nanomachine-collision-20260904`; there is no source-version substitution.

That publication negated vertex X but retained `m_CollisionTriangles` order.
These cooked collision indices must not be interpreted as render winding.
The native controller uses `(b-a).cross(c-a)` and front-side-only triangle
contacts. Its published shell consequently had inward faces: signed volume
`-2.860365629600397`. Compensating the negative-determinant coordinate change
by exchanging corners 1 and 2 gives `+2.860365629600397`.

`tools/native/publish-nanomachine-collision.py` pins the strict primary evidence
hashes, runs the tracked collider publisher with explicit `--reverse-winding`,
replays into staging, and verifies that all 332 vertices and 394 triangle
memberships remain exact. No triangles are added, no convex hull is substituted,
and no global two-sided collision rule is introduced. Historical default replay
of the generic publisher remains available without the flag.

The native sidecar and its BLAKE3 binding are the only runtime outputs. The
visual GLB, node hierarchy, scale, animations, materials, and textures are
unchanged. This is a conversion-artifact repair, not a donor or visual extension.

Native acceptance is `world::tests::nanomachine_top_stops_a_falling_capsule`:
the production GLBs and collision-node transform feed the real capsule solver,
with nine downward sweeps and 32 radial approaches. Structural hash binding is
also covered by `production_nano_stations_bind_exact_native_collision`.

The related Slider correction is native runtime/network work. Idle platform
carry previously never emitted a horizontal position packet. OpenFusion's
`stopPlayer` updates player position and chunk interest; periodic reconciled
STOP packets now publish the carried world position without inventing input
velocity. A pending carry survives until the existing packet interval and is
cleared on authoritative teleport. The Slider geometry is unchanged. The
production-collider test `slider_idle_rider_keeps_support_and_publishes_route_positions`
checks 600 frames of translation and vertical motion, ground-support retention,
and exact packet coordinates. Live-server visual confirmation remains separate
from these deterministic native checks.
