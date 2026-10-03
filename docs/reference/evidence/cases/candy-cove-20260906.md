# Candy Cove Past: orbiting foliage and intersecting jump-pad shells

Question: why do static-looking props orbit Candy Cove Past, and why do its two
shore jump pads have intersecting models in FFOne's native world renderer?

Consumer: FFOne `map_08_06`, world behaviour admission and static-world rendering.
Authority inspected: primary `retrobution-20260821`, `Map_08_06.unity3d` and
`DongResources_08_06.resourceFile`. This repair deliberately corrects defects
present in the primary scene; it is not a claim of byte-for-byte primary parity.
No alternate or patched donor is used.

## Orbiting props

Primary `BuildPlayer-Map_08_06` GameObject 8712 (`Plane01`) owns Transform 5128.
Its local position is `(25, 0, -2)`, its quaternion is all zero, and its parent
is Transform 5132 (`Object01`). The native animation owner at GameObject 13249
has scale 10 and plays the `Object01` rotation clip. The two other affected
renderers, GameObjects 8715 and 8718, are similarly parented. They are a plant
and two parts of one tree, not travelling platforms. Their native placement
IDs are 02106, 02107 and 02108.

The previous zero-quaternion normalization fix prevents NaNs but does not stop
the inherited orbit. The production GPU probe measured 214 m displacement for
the plant and 152 m for each tree part. The fix removes only these three visual
bindings from that animation. It preserves their authored world placement,
all hierarchy target records, the rotating prop itself and the clip.

## Jump pads

The scene renders an octopus visual at each of `(-4375, -57.5, 3031)` and
`(-4306, -58, 3040)`. Each also has a standard metal jump-pad shell with exactly
the same native TRS. Those shells are scene GameObjects 11211 and 11271. Their
object placements contain visual geometry only, with no collider ownership.

The exact `map/ep/ep_exsh_iumppad_octopus_01.nif` closure in primary
`DongResources_08_06.resourceFile`, serialized asset
`CustomAssetBundle-a3962f73ba4214b50b6d88e38442b1c2`, root GameObject 1367,
contains octopus renderer owner 1370 and collision owner 1377. It has no
standard metal shell. The before GPU capture shows that shell covering the
octopus body and intersecting its tentacles. Removing the two extra visual
placements exposes the complete original octopus model. Its GLB and texture
are unchanged. All 706 scene colliders, trigger volumes, jump powers and
animation clips compare equal to the pre-repair documents.

Rejected hypotheses: the differently positioned blue pad was a separate pad,
not a displaced half of the octopus; the index order difference between the
logical exporter and native GLB is the correct face-winding conversion (all
408 source triangles oppose the supplied normals before conversion).

## Replay and verification

All investigation output and pre-repair native input files are under
`work/legacy-sources/candy-cove-20260906`. The publication receipt pins their
hashes and the repair tool. Two independent staging replays produced identical
bytes. The tool validates exact ownership/overlap, removes no colliders, updates
the tile and catalogue hashes, and refuses a target changed since staging.

From FFClientEditor:

```powershell
python tools/native/repair-candy-cove.py --source-root work/legacy-sources/candy-cove-20260906/native-before --stage-root work/legacy-sources/candy-cove-20260906/stage-b --target-root ../FFOneClient/assets/game
```

From FFOneClient, the matching before/after probe was:

```powershell
$env:FFONE_WORLD_TRANSFORM_AUDIT='1'
target/debug/examples/world_performance_probe.exe shared target/performance/candy-pad-after -4306 -58 3040
```

`candy-pad-before` and `candy-pad-after` contain matching camera captures and
placement snapshots. After repair no `map_08_06` visual moved more than 30 m
from its authored placement; the three corrected foliage placements remain
stationary. The second octopus was also captured in `candy-second-pad-after`.
These probes verify native rendering; they do not exercise server interaction.

The native regression test materializes the production scene in bounded
batches, advances animation for 30 seconds, checks finite bounded placements,
and rejects a standard shell intersecting either octopus. The existing
zero-quaternion regression remains intact.

Final verification: focused regression passed; world-behaviour suite 41 passed,
1 ignored and one pre-existing hash-verification test failed (HEAD ignores the
hash argument). Real development binary built and completed the full-client
offline fixture with exit 0 and no runtime errors. All 72 tile files and catalogue
hashes passed. Full asset audit is blocked by the unrelated untracked
`textures/hnpc/m_face_006_a.png` lacking a declared owner group.
