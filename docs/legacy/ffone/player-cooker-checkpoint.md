# Historical Test Ser player-cooker checkpoint

This is a partial recovery record, not a command to generate new intermediate products.
`cook-player-avatar` proved CharacterSelection.resourceFile route ownership, the
133-node actor/m.kfm skeleton and 230 AnimationClip objects. Clothing requires agreement
between ActorWearIndexTable.transformIndicesM, renderer bone palettes and inverse-bind
counts before mapping onto the shared actor skeleton.

The recorded customization used m_face_001_type01, m_head_023_type01,
m_shirt_baseballset, m_pants_beltarmorset, m_shoes_blooarmorset and theown_discobomb.
The previous20260613 source was explicit, not the current primary by implication.
Native position/translation=[−x,y,z], rotation=[x,−y,−z,w]; scale unchanged, with no
centering, normalization or extra facing rotation.

The old `characters/player/male/base/male_skeleton.json` was evidence, not a substitute
model; the checkpoint emitted no complete GLB. `complete:false` remained until all
curves were projected/round-trip checked, clothing palettes/IBMs merged, exact shape/
height clips sampled and the assembled GLB passed native structural/visual checks.
Recorded source GameObject/AssetBundle IDs and index-table mappings are provenance,
not fields to restore to final runtime data. Existing checkpoint products refused
replacement; that historical behavior does not justify a mandatory work/cache tree.

Implement any remaining operation inside the direct converter rather than manually
assembling the former objects/logical-plan/part-source JSON arguments. Current entry
points and coverage: [conversion](../../conversion-workflow.md).
