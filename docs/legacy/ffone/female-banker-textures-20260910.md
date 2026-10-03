# Female banker and accountant clothing — 2026-09-10

Question: why do the black-suit female bankers and the accountant have broken
clothing in the native HNPC renderer, compared with current primary Retrobution?

There were two independent conversion defects. Appearances 144 (Penny Morbucks,
NPC 2554), 145 (SeoYoon Morbucks, NPC 2573), and the jacket of appearance 178
(Morbucks Accountant, NPC 2587) still selected the item `secretagentcoat` meshes.
Primary `all_hnpc` instead selects `f_*_secretagent`. The exact native models
already published for appearances 176/177 are reused, including their immutable
shared material texture references. No geometry or animation payload changes.

The HNPC black jacket and trousers PNGs also differed from current primary's
`Retro_shared.resourceFile` atlases. Merely correcting the mesh routes leaves
the jacket visibly scrambled. The two exact primary atlases are installed as
`textures/hnpc/female_banker_jacket.png` and `female_banker_trousers.png`, with
semantic catalog keys limited to these three appearances. Existing texture
consumers outside this scope retain their definitions. Both source textures are
256x256, one level, linear/repeat, anisotropy 1 and mip bias 0.

The jacket object has classId 28 but the legacy type name `Texture` (typeId 27).
The typed texture exporter rejects that name. Strict object evidence resolves
it unambiguously as primary `Retro_shared.resourceFile`, serialized asset
`CustomAssetBundle-Retro_shared`, object 1966. Its DXT1 payload is decoded with
Pillow BC1 and vertically flipped to the existing native PNG convention.
Trousers object 1967 is accepted by the exact texture exporter. No painted or
substitute pixels are used. The publisher checks both raw container hashes,
scoped evidence hashes and the hashes of reused native banker models.

Appearance 178 retains its source-selected skirt, shoes and `shoes_accountant`
texture; changing all three slots to a trouser suit would be incorrect.

The tracked publisher is `tools/native/repair-female-banker-routes.py`; source
identities, replay commands and output hashes are recorded in
`recipes/native/characters/hnpc-female-banker-20260910.json`.
The superseded September 8 receipt retains the original model recovery history.

GPU captures under `work/cases/banker-20260910` include `144-before.png` and
`178-before.png`, intermediate model-only `*-after.png`, and final captures for
144, 145 and 178. All final captures reach ReadyAnimated and were visually
inspected. The final jacket has a continuous white shirt, tie and cuffs; the
trouser belt aligns with the waist. These are production native preview checks,
not a claim of a live server visit or a new Unity gameplay capture.

The production catalog regression covers all five female secret-agent jacket
appearances, exact corrected texture routes, and the accountant's distinct
skirt/shoe contract. Native asset graph validation and development-client asset
validation pass. Replaying the publisher yields all three changed files
byte-identically and does not alter other appearances.
