# Character creator height and build

Evidence question: do creator height/build selections change the actor root, or
sample skeleton animation, and does the native preview consume those selections?
The acceptance consumer is FFOne's existing shared player skeleton in creation,
selection, inventory/try-on and selection portraits.

## Primary authority

Source is `primary`, raw `retrobution-20260821/main.unity3d`, 8,221,718 bytes,
SHA-256 `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
No alternate donor or historical build was substituted.

Exact UnityWeb level-0 entries:

| Entry | Bytes | SHA-256 |
| --- | ---: | --- |
| Assembly - CSharp.dll | 1,762,816 | `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792` |
| Assembly - CSharp - first pass.dll | 396,288 | `AFFF470BEC30BF06703EB0664DE364CE37EA7B8092C848420A92FBA63E91DA81` |

Reproduction from FusionForge (create output directories first):

```powershell
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/cases/body-shape-20260912/csharp.evidence.json --payload-out work/cases/body-shape-20260912/Assembly-CSharp.dll
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp - first pass.dll" --level 0 --out work/cases/body-shape-20260912/firstpass.evidence.json --payload-out work/cases/body-shape-20260912/Assembly-CSharp-firstpass.dll
dotnet run --no-build --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/cases/body-shape-20260912/Assembly-CSharp.dll -t CnCharCreationMode -o work/cases/body-shape-20260912/decompiled
dotnet run --no-build --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/cases/body-shape-20260912/Assembly-CSharp-firstpass.dll -t ActorSkinCombiner -o work/cases/body-shape-20260912/decompiled
```

Producer revision: `5d592c4e0c08d4673b5c4ed378cd87a066437a7a`.
The exact-entry reports include assembly and CLR metadata hashes. Decompiled
outputs remain under ignored work, not in FFOne.

## Recovered contract and defect

`CnCharCreationMode` assigns the active male/female actor's normalized shape as
`iBuild / 2` and normalized height as `1 - iHeight / 4` (decompiled lines 946–972).
`ActorSkinCombiner.SetAnimationWrapMode` sets `height_Add` and `shape_Add` to
additive, clamp, layer 100, weight one. `Update` enables and samples those TR
deltas every frame, and samples ordinary `height`/`shape` when selections change.
`SetNewShape` also explicitly resamples the ordinary clips.

The native skeleton GLBs already contain all four clips; their `_Add` channels
are published bind-relative deltas. FFOne's world player animation adapter
already composes those clips. The central preview instead used only its idle
clip, and its geometry key intentionally excludes height/build. Values reached
`NativePlayerLook` but were never consumed by the preview animation player.

The fix adds opt-in body selection to shared preview rigs, composes idle and
TR deltas under one additive node with an ordinary scale sublayer, and pauses
each body clip at its selected time. Body changes update the same player and
bone entities; scene scale, camera geometry and UI text do not change. Preview
idle recovery restores all body layers after replacing a foreign graph.
Selection portraits opt into the same body sampling. The world animation
adapter retains ownership of its existing gameplay layers.

No binary payload was republished. This is a native consumer fix, not a new
extraction or a root-scale approximation. Source runtime screenshots are not
available in this case; managed behavior and native execution are separate
acceptance evidence, and no Unity-versus-Bevy pixel-equivalence claim is made.

## Verification

The body-shape ECS test evaluates all 15 combinations through Bevy animation,
checks changing bone translations and unchanged base rotation, and restores
the static layers after a player reset. The creator GPU fixture accepts
`FFONE_REVIEW_BODY_SHAPE=1` with 15 appearance frames to cycle all combinations
on the same preview, for each gender. Paused body layers are excluded from its
idle-motion check.

Accepted results: both ECS tests passed, including all 15 actual production-GLB
poses per gender. The GPU fixture saved 30 appearance captures under FFOne's
ignored `target/performance/body-shape`; the material-pass diagnostics retained
one unchanged rig root/generation per gender across all changes. Endpoints and
intermediate poses were visually inspected. The actual optimized Dev executable
also passed selection, introduction, name reservation, animated appearance,
female appearance, return to selection and resumed appearance (exit code zero).
The hashed source evidence, native inputs, captures, logs and Dev executable are
recorded in `docs/reference/evidence/cases/player-body-shape-20260912.json`.
