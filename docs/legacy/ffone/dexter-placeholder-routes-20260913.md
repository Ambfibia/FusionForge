# Dexter placeholder routes 753/754 (2026-09-13)

Question: why do NPC IDs 753 and 754 show the wrong Dexter appearance after
the primary model update, and does ordinary Dee Dee still use the owner's
Retrobution 20260613 appearance without reverting Fusion Dee Dee?

Primary is Retrobution 20260821; previous is the clean Retrobution 20260613.
The consumer is FFOne's `NetworkNpcVisualCatalog0104`.

The primary `TableData.resourceFile`, `CustomAssetBundle-1dca92eecee4742d985b799d8226666d`,
MonoBehaviour 7 `xdtdatas`, selects mesh row 1 for both IDs. That source row
contains `npc_dexter2`, `npc_dexter`, and `npc_dexter2_face_0`. The native table
retained its old mesh row 1 (`npc_dexter` and `npc_dexter_glass`), while the
updated full source row was already installed at native index 659.

The earlier name-based NPC repair updated rows named Dexter. It missed these
two rows because their names are Nuclear Plant Console and Other Kid 1.
Their identities and names remain unchanged; only their `m_iMesh` changes
from 1 to 659. The existing new model, rig, material slots, textures and
animations are reused. No binary assets are published.

The replayable repair is `tools/legacy-sources/repair-dexter-placeholder-routes.py`.
Exact source identity, hashes and commands are recorded in
`recipes/native/characters/dexter-placeholder-routes-20260913.receipt.json`.
Staging and strict source evidence are below `work/cases/dexter-753-754-20260913`.
The script verifies the raw source hash, reuses an exactly equal native mesh
row, preserves file formatting, and proves that no other table fields change.
It is idempotent.

Ordinary Dee Dee 701 already selects retained mesh row 16 `npc_deedee` and
the previous model-owned atlas. The full source-backed previous-build
retention check passes both before and after this repair with zero rows to
restore. See `previous-build-retention-20260912.md` for the original geometry
and texture evidence. Fusion Dee Dee remains at SHA-256
`18e65c8be61065667a8b7a52dcf165f2b399e663e43c1c60651a62ec77ba530d`, matching
both HEAD's LFS identity and the retained primary publication.

The production catalog regression covers both placeholder IDs' model and
texture resolution, as well as ordinary Dee Dee's retained route and atlas.
An in-game screenshot comparison was not performed for this table-only repair.
