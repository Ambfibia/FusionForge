# Slider authored texture binding

Question: should FFOne replace the current primary Slider's `-01` and `-02`
material atlases with the two transportation mesh-table textures?

Answer: no. Classification: correction of a native runtime conversion artifact.
`cnBusMoveController.SetupBus` loads the table textures, but assigns them only
when `renderer.material.name.Contains("sub")` or `.Contains("main")` succeeds.
The checks are case-sensitive; the main write follows the sub write. Neither
material name on the primary bus matches. Loading a texture is not proof that
the source assigns it. The previous native special case invented that assignment.

## Primary evidence

All raw recovery output is below `work/cases/slider-textures-20260910`.

- `main.unity3d`: SHA-256
  `01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
- Exact entry `level0/Assembly - CSharp.dll`: SHA-256
  `0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
- Fresh FFSpy `cnBusMoveController.decompiled.cs`: SHA-256
  `0d54b9504b7fc3fb462a1d261a065ce65ec383aa9629b5cafc857fa76c6f4153`;
  lines 195 and 202 establish the two name gates.
- `Tutorial.resourceFile`: 33327638 bytes, SHA-256
  `729e9aad557709d4a75937b7478b6784b82f202c000d13127d1074213991f916`.
- Serialized asset `CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a`,
  Material 1715:
  `dt_etc_downtownbus_a_00-01 - default-dt_etc_downtownbus_a_01.dds`;
  local `_MainTex` 340, `_BumpMap` 269.
- Same serialized asset, Material 1679:
  `dt_etc_downtownbus_a_00-02 - default-dt_etc_downtownbus_a_02.dds`;
  local `_MainTex` 18, `_BumpMap` 58.
- Strict material evidence has no unresolved pointers. Evidence hashes:
  `body-material.json`: `e74d9fbef4ce2795a149be21ba3726e4e8c81ad765627acaeaa1970ac3e50764`;
  `trim-material.json`: `fd00cbb485a2f39d31e05d947890a3363b7b3e422cd005b517d849656d43d034`.

## Replay from FusionForge

```powershell
New-Item -ItemType Directory -Force work/cases/slider-textures-20260910/decompiled
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d 'Assembly - CSharp.dll' --level 0 --out work/cases/slider-textures-20260910/assembly.json --payload-out work/cases/slider-textures-20260910/Assembly-CSharp.dll
dotnet vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.dll work/cases/slider-textures-20260910/Assembly-CSharp.dll -t cnBusMoveController -o work/cases/slider-textures-20260910/decompiled --preserve-iterator-state-machines
./fusionforge.cmd dump-object-evidence primary ../builds/retrobution-20260821 Tutorial.resourceFile 1715 --serialized-asset CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a --type Material --out work/cases/slider-textures-20260910/body-material.json
./fusionforge.cmd dump-object-evidence primary ../builds/retrobution-20260821 Tutorial.resourceFile 1679 --serialized-asset CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a --type Material --out work/cases/slider-textures-20260910/trim-material.json
```

## Native correction and validation

FFOne's `network_world_runtime/transportation.rs` now applies the original
case-sensitive name gates. All shipped bus surfaces retain their native texture
handles, authored mip chains, glow maps, geometry, collision, and animation.
No binary payload publication is needed: this changes runtime selection only.
The historical `data/transportation/downtown_bus.json` contains source and table
lineage; its `xdtRole` annotations do not establish actual runtime assignment.

`cargo test -p ffone-client --lib slider_ -- --nocapture`: 7 passed. Tests open
the production GLB, reject table assignment for every material, retain the five
glow primitives and UV/normal contracts, and check collision and animation.

An initial capture with the old September 7 preview executable was rejected:
its success report accompanied a frame containing only additive effects.
It is diagnostic output, not visual acceptance.

Fresh `cargo build -p ffone-client --bin ffone-client --example
logical_model_gpu_preview` passed. The rebuilt preview produced `current.png`
and `current.gpu.json`: all 13 meshes/materials loaded, 18 exact mip chains
(154 levels), zero material/shader errors, `stand1` running, and no table
overrides. Visual inspection confirms the complete body, deck, fins and glow.
This is isolated model acceptance; no live server ride was performed.

Replay capture from FusionForge after the build:

```powershell
../FFOneClient/target/debug/examples/logical_model_gpu_preview.exe --asset-root ../FFOneClient/assets/game --model characters/transportation/downtown_bus/DT_ETC_Downtownbus_A_00.glb --animation-name stand1 --screenshot work/cases/slider-textures-20260910/current.png --report work/cases/slider-textures-20260910/current.gpu.json --timeout 120
./fusionforge.cmd export-exact-texture ../builds/retrobution-20260821/Tutorial.resourceFile 340 work/cases/slider-textures-20260910/body-texture.json
./fusionforge.cmd export-exact-texture ../builds/retrobution-20260821/Tutorial.resourceFile 18 work/cases/slider-textures-20260910/trim-texture.json
```

Both freshly exported source chains match the GLB's existing provenance,
including 10 mip levels each: body
`632c9214a44cf7fee1cd60b0be2a5c1d56def5725accb9dd898948d1f5828634`, trim
`953d0e9c3880cd6aab7ef45d25d2e5a560a74c657fb6a9d7bdc7010fd7183de7`.
