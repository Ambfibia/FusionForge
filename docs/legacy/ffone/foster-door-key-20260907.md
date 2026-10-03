# Foster door and key repair

The evidence question is why a warp through Foster's Front Door leaves the
playable interior, and why the key NPC is static in the native client.
The source role for animation recovery is current `primary` (Retrobution
20260821); no alternate donor or coordinate adjustment is introduced.

## Door: native collision recovery

Production WarpTable row 295, NPC 1464, enters map 152 at protocol coordinates
`[481181, 435788, -18000]`, native `[-4811.81, -180, 4357.88]`.
The native client and OpenFusion's current XDT agree. The interior geometry
already exists below the outdoor terrain in native tile `map_09_08`.

`resolve_authored_world_ground` searched the entire vertical range for terrain
whenever the ordinary downward sweep found no heightfield contact. That
recovery ran before the mesh-floor support query and promoted the higher
outdoor terrain above an intentionally underground destination. The repair
bounds terrain penetration recovery to the existing 0.3-unit controller step
offset above the current feet. Normal swept contacts and mesh-floor ownership
are unchanged; no warp coordinates or server authority are overridden.

The production-data regression `world::tests::foster_warp_keeps_interior_floor_below_outdoor_terrain`
loads the actual table row, outdoor heightfield and interior mesh colliders,
advances the movement/collision systems for 120 frames, checks the interior
floor and grounded state, then checks shallow outdoor penetration recovery.

## Interior: rejected native material

The first full-client capture after the collision repair kept the player on the
interior floor but showed an empty room: its native `RetroLit` material failed
shader admission. Primary `DongResources_10_07.resourceFile`, serialized asset
`CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12`, Material 169 owns Shader 168.
The strict shader dump and explicit external texture resolution are recorded in
`docs/reference/evidence/legacy/foster-20260907/retrolit-shader-evidence.json` and
`interior-material-contract.json`. Material inspection is labelled triage because
the dump tool did not automatically load the external texture bundle; the exact
external asset identity was resolved separately and the texture dumped strictly.

The source color pass is opaque, back-face culled, depth writing, with lighting,
ambient equal to `_Color`, explicit `_Emission`, separate specular and a doubled
texture/primary RGB combiner. Foster's saved specular RGB is zero and material
alpha is one. Those values permit reuse of the existing family-6 native color
program with opaque pass state. Admission rejects other specular/alpha variants
instead of silently approximating them. Original geometry and textures remain
unchanged. Two production/synthetic regression tests verify this contract,
including the shader's black emission default and rejection of unsupported values.

## Key: lost animation payload

The installed `characters/npcs/npc_key/npc_icekey.glb` had no animations, and its
native registry entry listed none. Current primary `Retro_shared.resourceFile`
owns route `mob/npc_key.kfm`, GameObject `npc_icekey` 27118 in serialized asset
`CustomAssetBundle-Retro_shared`. Its Animation component 27141 refers to
AnimationClip 27143, `stand1`. The clip rotates the child
`icekey_ring/icekey_ring NonAccum`; it is not a rotation of the network NPC root.
The recovered looping clip lasts 1.333333373 seconds and has nine TRS channels.

The exact object evidence and preserved native render-contract fixture live in
`evidence/legacy/foster-20260907`. The canonical publication receipt is
`recipes/native/characters/foster-key-animation-20260907.receipt.json`.
`tools/native/restore-foster-key-animation.py` copies only the validated clip's
accessors and buffer views into the existing native model. It verifies exact
node hierarchy/TRS equality against the current primary export and retains
all original geometry, materials, image/sampler contracts and shared texture
references byte-for-byte. The fixture preserves the accepted native render
payload so the Editor can replay publication independently of the client tree.

`world::tests::foster_key_has_a_bound_looping_rotation_clip` checks production
registry/hash agreement, target-node ownership, nonconstant quaternion poses,
duration and a closed loop. Binary replay has already produced identical bytes.

## Verification

The four Foster regression tests pass, as do all 65 tests in the native material
module and three existing grounded-controller regressions. Key publication
replay is byte-identical. GPU validation loaded the original mesh/texture
contract, ran 199 animation evaluation frames with one AnimationPlayer and no
material/shader errors. A separate silver-texture capture ran 71 animation
evaluation frames with both runtime texture bindings and no errors; both
captures were visually inspected.

The first full-client offline capture measured 600 frames at the production warp
destination and settled at native Y `-179.97001648`, the actual interior floor.
Its empty-room screenshot exposed the material issue above. The final rebuilt
client capture after material admission passed: 600 frames, player position
`[-4811.81005859375, -179.9700164794922, 4357.8798828125]`, no material errors,
and a visually inspected frame showing the room, entrance doors, floor and
player inside. Report and frame are under the client's ignored
`target/performance/foster-20260907/interior-fixed/`. The final binary build
passed with the existing unused `CameraProjection` import warning. This was
the offline production-app fixture, not a live-server end-to-end door session.

Concurrent UI edits initially blocked compilation/startup: two exhaustive
`PaletteLabel` match arms were restored after that variant's early return,
quit-menu lookup now finds its actual button kind rather than indexing a
shortened array by enum ordinal, and a diagnostic JSON macro was qualified.
