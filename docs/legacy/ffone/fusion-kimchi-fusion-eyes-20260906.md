# Fusion Kimchi: correction to Fusion Eyes

**Appearance rejected by the owner after publication.** The later Academy
reference requires oval red eyes with dark upper lids; the `spwaneye` projection
described below is not the correct appearance. Keep this publication history,
but follow [the final Academy/original-atlas repair](fusion-kimchi-academy-20260906.md)
for the corrected appearance. The stored receipt certifies published bytes, not approval
of their visual match to the later reference.

The owner corrected the September 5 white-eye repair: NPC 3460 must have red
Fusion Eyes. The ordinary `vehicle_Kimchi` atlas used by the previous repair was
the wrong appearance. This note supersedes that conclusion, not the other NPC fixes.

The accepted `spwaneye` atlas is the primary Retrobution 20260821 128×512 RGBA32
texture, with one mip, sRGB color, linear filtering, repeat sampling, and the
`normal_blendOneOneTest_cullOff` additive material. The typed Fusion Ice King
export proves Material 1468 → Texture2D 1470 and Shader 1469 within serialized
asset `CustomAssetBundle-e5a53b272630f4113ac7dfa756eae499` in
`DongResources_07_06.resourceFile`. The source container and exact typed PNG are
rechecked by `tools/legacy-sources/audit-fusion-kimchi-fusion-eyes.py`.

The native Fusion Dexter material supplies the same certified shared atlas and
material contract, plus its single-eye atlas cell. Kimchi placement is an authored
repair: each eye ring and pupil cap receives one continuous projection, mirrored
between eyes. A black backing on the existing eye geometry prevents additive red
from mixing with the green body. Mapping only the eye rings was rejected: it left
holes where the separate pupils should be. A first capture without the backing
was also rejected because the green body made the eyes yellow.

Original geometry, source binary bytes, bones, weights, hierarchy, animation,
body indices and body UVs are unchanged. The eye overlay adds one small additive
draw over 58 existing triangles. Both backing and eye overlay reject NPC table
texture overrides. The accepted shared PNG is reused without copying or editing
texture pixels. The ordinary Kimchi model is unchanged.

The publisher changes only `characters/npcs/fusion_kimchi/fusion_kimchi.glb` and
the corresponding digest in `_runtime/characters.json`. No XDT, server table or
icon change is needed. The new canonical receipt supersedes
`npc-material-followup-20260905` and carries its other 65 byte-verified outputs.
Replay requires the previous receipt's pinned normal-eye input snapshot.

Evidence lives under `work/legacy-sources/fusion-kimchi-fusion-eyes-20260906`:
`editor.png/json` show the actual editor loading NPC 3460 successfully;
`installed/3460.png/json` show the installed animated front view;
`verification.json` checks exact replay and scope; `test.log` and `build.log`
record the focused native contract test and development binaries build.
