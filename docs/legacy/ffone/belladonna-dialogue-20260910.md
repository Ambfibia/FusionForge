# Belladonna NPC dialogue recovery

Question: why does native NPC 720 remain silent for ordinary conversation while its communicator clips are present?

Source: primary Retrobution 20260821, `DongResources_04_03.resourceFile`, serialized asset `CustomAssetBundle-71c4e29715f9c43c7890d672636790f4`. No alternate donor or behavior change is involved.

The native NPC string row 720 selects voice owner `Belladonna`. Existing gameplay audio generates the owner-based greeting, quest greeting, farewell, good-luck, and nice-job identities. The native audio table contained only Belladonna's three communicator recordings.

The primary AssetBundle routes `vo/belladonna_*.wav` point locally to twelve AudioClips whose internal names are `Buttercup_*`. Importing only the clip names loses the Belladonna route identities. This is an asset routing omission, not a missing source performance and not a reason to replace Belladonna's voice owner globally (her communicator remains separately owned).

Native contract: publish twelve route-derived `Belladonna_*` semantic identities under owner `belladonna`, preserving the exact embedded Ogg bytes. Keep existing dialogue scheduling, random take selection, spatial ownership, LocalizedVoice, and locale fallback unchanged. The publisher pins the original internal names separately from the native semantic names. English files resolve in EN and as the existing EN fallback for RU; no Russian Belladonna recordings were available in the native voice tree.

The tracked plan and receipt are `recipes/native/audio/belladonna-dialogue-20260910.plan.json` and `recipes/native/audio/belladonna-dialogue-20260910.receipt.json`. Both independent extraction passes and installation agree on every audio hash. The broad bundle evidence is explicitly triage due to unrelated external model references; the twelve selected audio pointers are all fileId 0 and each resolves through strict, scoped AudioClip evidence with zero unresolved pointers. No unresolved external is used by this repair.

Target checks cover every take's non-silent decoding in EN/RU and all five dialogue cue families queued using the production NPC 720 owner. See the receipt for exact commands and build results.
