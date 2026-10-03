# FusionFall parity audit — 2026-09-05

Authority: canonical `primary`, Retrobution 20260821. This is an **incomplete audit** of
the owner's full bug list. Existing custom character shaders and language controls are
intentional. Do not replace the original Ben, Gwen, Kevin, Albedo or Max. The owner's
latest instruction supersedes the earlier placement preference: **do not place NPCs**.

## Changes verified in this pass

| Area | Finding and change | Verification / limit |
| --- | --- | --- |
| Resurrection | The shell discarded the nearest XCom context on every nonzero map. It now supplies the default index accepted by the shard; the shard calculates the authoritative destination. Countdown presentation clamps at zero. Strict elapsed > 60 and request deduplication remain. | Native state-machine test covers late context and one request until reply; production ECS context test covers instance map. Interactive death/revival still requires a server session. |
| FFClientEditor model census | Case-insensitive discovery incorrectly counted model names that the native exact-case resolver could not reach. The census now mirrors category preference, explicit aliases and invisible NPC exclusions. | Five Python tests. Audit reports missing and ambiguous routes separately. |
| Character alias repair | Case-insensitive canonical-package detection skipped required exact-case aliases. Fixed without dropping existing root ownership. | Ten Rust tests; repair replay makes zero further changes. Nine Tech Queen rows and one Fusion Spidermonkey row resolve in the production catalog. |
| FFClientEditor GLB winding | Ben has a nearly collinear triangle whose orientation changes when f64 coordinates are encoded as GLB FLOAT. Checking before that conversion produced one rejected triangle. Audit now uses persisted f32 positions/normals. | Regression passes through encode, semantic roundtrip and strict GPU facts. Old/new vertex, normal, UV, joint and weight buffers are identical; only two triangle indices change. Fixed Ben loads and animates on GPU. |
| Native UI startup | Focused plugins initialized States without installing StatesPlugin. | Initialization is idempotent and works with MinimalPlugins. |
| UI test fixtures | Mission interaction fixtures lacked the production audio outbox. Two localization audits inspected an obsolete source file/test-module boundary. | Corrected fixtures/source audits. These are verification fixes, not claims that all dialogue bugs are solved. |
| Omniverse NPC types | Added 3464 Ben, 3465 Gwen, 3466 Kevin, 3467 Albedo, 3468 Max, with distinct semantic model routes, texture identities and EN/RU names. Existing types retain missions and services. | Five GLBs and exact body/face textures pass GPU load/stand1 and visual inspection; independent publication replay matches all 13 model payload files. Production resolver test covers all five new types. |
| Omniverse portraits | Four primary portraits use separate native icon numbers; old portraits are retained. | Exact exports from Icons.resourceFile. The primary catalog has **no** npcicon_116 route, despite Albedo's table reference; its variant inherits the existing native portrait. |
| World placement | None added. | Both OpenFusion NPCs.json files retain SHA-256 `5e62cbd0ce9574653b3c2dc8a57478ed179e28b38b2f1eeeffc84056c1a588f2`. Only additive xdt definitions are synchronized. |
| Packaging | Map validation rejected accepted shared PNG chains under textures/shared. | Allow only PNG references in that native shared root; retain traversal and other-domain rejection. Packaging tests and full asset validation recorded separately. |

## Evidence and replay

- `work/legacy-sources/parity-audit-20260905`: exact-case census before/after, alias
  repair reports, native registry preimage, model-publisher regression output.
- `work/legacy-sources/omniverse-npcs-20260905`: scoped source exports, object proofs,
  exact texture/icon exports, strict GPU evidence, dressed diagnostic captures,
  deterministic replay, accepted types-only staging and installation preimages.
- `recipes/native/characters/omniverse-*-20260905.json`: five binary publication receipts.
- `tools/legacy-sources/publish-omniverse-npcs.py`: additive type publication. It does not
  read or write placement files; IDs are fixed and occupied append points are rejected.
- `docs/reference/evidence/cases/artifacts/omniverse-types-20260905.json`: native definition hashes and
  the two synchronized server table hashes. Work captures are not clean-client goldens.

The model source roots and authoritative texture objects are scoped by serialized
asset, not by PathID alone. NpcTexture's Ben route is an external pointer to Tutorial
Texture2D 339; guessing a same-named local object is incorrect. ToonRamp9 is owned by
CharacterCreation. The full AssetBundle dumps intentionally contain unresolved
unrelated pointers and are triage only; accepted texture objects have separate scoped
raw proofs. No unverified external pointer is rewritten as a local pointer.

## Full-list status

The user's other reports are **not closed by this pass**. In particular, server/gameplay
acceptance is still needed for quest dialogue windows, tutorial communications and
dialogue interruption, Monkey Skyway, SCAMPER travel, Eduardo escort, guide mail,
instance isolation, nano-mission persistence and level progression, future level-four
mission completion, combat damage, loot/crate notifications, weapon switching and
right-click unequipping. Local fixes must be traced to authoritative client/server
contracts before altering these systems.

The remaining presentation/input items also remain open: Larry and Mendroid voice,
voice localization completeness, spatial NPC audio, NPC chat barkers, combat-to-walk
animation, weapon textures, SCAMPER materials/pilot portrait, station collisions,
teleport UI/letterbox/avatar visibility, name toggles, communicator/modal ownership,
location banners, post-effects, music crossfade/restart behavior, inaccessible nanos,
shader option, Russian/login language defaults, anisotropic filtering, gamepad input,
mouse sensitivity, dropdown order, glass/terraformer/spawn highlights, alt-tab focus,
chat scrolling, local-map NPC filtering, nano ability layout, guide mission icons,
missing Skyway destinations and rotating cannons.

The read-only duplicate audit found 3,035 repeated encoded PNG entries (43,314,121
bytes). This is **not** proof of equal full sampling/material contracts and **not** an
FPS measurement. No visual quality, object count, camera distance or animation timing
was reduced. A matched full-client performance run is still required.

## Test baseline caveats

The initial Cargo cache referred to `target/performance/source`, including its embedded
manifest directory. The client package was cleaned and rebuilt from the real source
tree before accepting results. The subsequent full library run had 1,618 passes,
39 failures and 3 ignored tests. Most failures concern old asset/hash/schema fixtures;
others concern content/source audits and need individual investigation. Do not update
expected hashes merely to make that suite green. The focused new NPC production test
was run after publication. The publisher's obsolete curve-recovery test now verifies
the semantic clip/bone reference and the absence of legacy PathID/asset fields, matching
the existing native serialization contract. Publisher tests: 46 passed, 1 ignored.
Native localization tests: 18 passed. Packaging tests: 12 passed. Full native asset
validation: 5 domains, 103,454 references, 65,564 files, 3,385,997,347 bytes; passed.

The final independent type-publication replay matches all 27 native files and both
server xdt files. Both placement files remain unchanged. The sampler display identities
are the new semantic body identities; filter, wrap, anisotropy, mip bias and texture
pixels retain the exact source values.

Follow-up dialogue/journal work is documented in
[dialogue-journal-20260905.md](dialogue-journal-20260905.md): browsing a mission no
longer tracks it automatically, and ordinary NPC/journal modals suspend queued
NanoCom messages. That case separates primary journal behavior from the requested
modal-suppression extension and records targeted tests and the production-app smoke.
