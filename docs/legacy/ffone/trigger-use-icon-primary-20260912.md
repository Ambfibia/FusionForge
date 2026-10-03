# World trigger interaction cue

Question: why does the native client allow use of a zipline or launcher without
the original interaction icon? Authority is current primary Retrobution 20260821.
The consumer is FFOne's gameplay HUD and the existing avatar target selection.

## Recovered contract

`main.unity3d` (8,221,718 bytes, SHA-256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`)
contains `sharedassets0.assets`. Its `OwnUser` GameObject 1330 owns PrintName
MonoBehaviour 1442, with MonoScript 1044 (`CSharp`, `PrintName`). The fifth
`CombatIcon` slot points locally to Texture2D 143, `target_icon3`, 37x52, DXT5,
one mip, bilinear filtering. The exact output and object hashes are recorded in
`recipes/native/ui/trigger-use-icon-primary-20260912.json`.

The same container's Assembly-CSharp payload is 1,762,816 bytes, SHA-256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The verified extraction and decompilation are reused from
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary`.
Focused ownership, assembly, static interaction and image geometry evidence is
under `work/cases/trigger-use-icon-20260912`.

`cnAvatarAttack.Update` chooses a trigger, arbitrates it against focused NPCs and
players, and calls `myname.SetCombatIcon(4)` only when the trigger wins. This
happens before the input-enabled/system-popup gate. Both launcher and zipline
use this same cue; there are no separate cannon/zipline pictures. Nonordinary
movement hides the local icon (the skill branch returns before recalculation).

`PrintName.PositionUpdate` puts the local user's icon at screen center, using
integer division for both screen and texture dimensions. At 1264x681 the Rect
is (614,314,37,52). `DrawAll` draws the texture with Stretch at GUI depth 900 and
scales around its center. Every RGBA channel is multiplied by
`0.5 + (sin(Time.time * 5) + 1) * 0.25`. The original bitmap includes its CLICK
legend; it is retained as an exact shared image with no new native text entity.

## Native repair

The original bitmap was absent from native assets. FFOne also lacked the local
PrintName interaction presentation owner; its existing target overlays only
covered NPCs and players. The new HUD child consumes the existing selected
trigger, checks local movement ownership and active launcher UI, and resets
visibility when the selection, player or target disappears. It does not consume
input or invent another proximity radius. The cue remains available with rocket
and grenade weapons. Unchanged geometry is not marked dirty each frame.

World volume detection now explicitly precedes avatar action resolution, after
the ordinary world target producer has cleared and rebuilt the feed. This
removes an unspecified same-frame ordering between trigger admission and target
selection. No placements, colliders, camera settings or animation timing change.

## Replay and checks

Run from FusionForge:

```powershell
python tools/legacy-sources/publish-trigger-use-icon.py --source-root ../builds/retrobution-20260821 --work work/cases/trigger-use-icon-20260912/replay --target-root ../FFOneClient/assets/game --apply
```

Omit `--apply` to re-extract into staging and compare the installed bytes.
FFOne's `trigger_use_icon` tests cover the production UI node, native payload,
selection loss, scripted movement, HUD visibility, stale entities, weapon mode,
scale and unchanged layout ticks. The existing GPU HUD preview accepts
`FFONE_MISSION_UI_PREVIEW=trigger-use` and `trigger-hidden`.

Static interaction export is candidate evidence, not a legacy runtime capture.
Full cross-renderer parity is not claimed from the recovered code alone.


## Native acceptance completed

The real FFOne Dev binary and gameplay HUD preview built successfully. Reviewed
1264x681 preview frames show the cue at the recovered Rect and zero cue pixels in
the traversal-hidden frame. The actual 1920x1080 D3D12 client was run through the
offline fixture at native launcher position (-2008.0763,-50.0,2374.0366) and zipline
position (-278.13,3.9,4881.51). Both streamed world triggers produced the cue and
both fixtures exited successfully. Frame hashes are retained in the native
`docs/native-ui.md` and `target/performance/trigger-use-icon/captures.json`.

All three native regression tests passed. The shared Cargo queue repeatedly
rebuilt unrelated concurrent changes, so the final test invocation included the
unchanged production trigger-icon module by path and linked the current native
client and Bevy libraries. No stand-in behavior or native types were used. The
exact compiler command, module hash and pass count are retained under the native
`target/performance/trigger-use-icon`. Native fast/full asset graph validation and
EN/RU production key/placeholder parity also passed.
