# Swampfire summon: optional idle animation

Evidence question: does current primary require `stand2` before showing a
summoned Swampfire, and what must FFOne do when random idle selection chooses
an unavailable animation? Acceptance is the production summon path revealing
the existing model, preserving `call`, all three skill selections, available
`stand3`, and the source fallback to `stand1`.

## Primary authority

Source is `primary` (`retrobution-20260821`), not an alternate donor.
Fresh exact assembly extraction:

```powershell
New-Item -ItemType Directory -Force work/cases/swampfire-summon-20260907
./work/build/debug/fusionforge.exe export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/cases/swampfire-summon-20260907/assembly.evidence.json --payload-out work/cases/swampfire-summon-20260907/Assembly-CSharp.dll
```

`main.unity3d`: 8,221,718 bytes, SHA-256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
Exact entry `level0/Assembly - CSharp.dll`: 1,762,816 bytes, SHA-256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The fresh payload is byte-identical to the previously evidenced assembly in
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary`.
Its existing `decompiled/NanoAnimation.cs` is 11,388 bytes, SHA-256
`91a9ad93cdc0da2080271a17fc2d74e4934f209f47b09972fb3a7370ff884eca`.
Reproduce decompilation with the pinned FFSpy console from the skill's managed
code lane, using the fresh payload above and a disposable case output directory.

`NanoAnimation.SetStandMotion` chooses `stand` + `Random.Range(1, 4)`, checks
the selected animation state, and sets `strCurName = "stand1"` if absent.
It then crossfades for 0.1 seconds. `Call` independently crossfades `call`.
No all-idle-clips readiness requirement exists in this behavior.

The accepted model source is recorded in
`recipes/native/models/nano-editor-models-and-table-materials-primary-20260904.json`:
`Nano.resourceFile`, 41,656,047 bytes, SHA-256
`4bdec03c5aa92e5f922ab61fa794188815b20438fcc4fa646498eac9397ce8e7`;
serialized asset `CustomAssetBundle-d44c8d83281084c7c860065e437b031e`,
GameObject 6509, route `nano/nano_swampfire.kfm`.
The accepted source export
`work/legacy-sources/nano-editor-20260904/sources/swampfire.json` is 12,465,375
bytes, SHA-256 `c329506b1490e76987f35754faf041ae542a3b8cf18924d91eeddb8e68cfd433`.
It owns `stand1` and `stand3`, with no `stand2`.
Current native GLB is 3,025,056 bytes, SHA-256
`80d1c0c9ba3f72cb653585e9fdebd1285b86bf333b8d0b39fe0df8fdb1c1f559`.
It preserves that clip set. No binary payload is changed by this fix.

## Native contract and checks

Ordinary-world Nano readiness requires `call`, `stand1`, and the selected
skill. `stand2` and `stand3` enter the animation graph only if present.
Random idle selection is unchanged; missing idle requests resolve to `stand1`
before playback so the sound cursor also uses the real clip name. The fixed
tutorial contract remains unchanged.

The production-GLB regression in `tutorial_nano_gameplay` exercises all three
skill slots, model reveal on call playback, missing `stand2` fallback, and
retention of `stand3`. Run in FFOne:

```powershell
cargo test -p ffone-client --lib tutorial_nano_gameplay::tests -- --test-threads=1
cargo build -p ffone-client --bin ffone-client --example world_nano_summon_gpu_preview
./target/debug/examples/world_nano_summon_gpu_preview.exe 24 target/performance/swampfire-summon.png
```

The GPU fixture uses the production catalog, model loader, material plugin,
summon command queue and animation systems. It waits for an active Nano and
captures after 30 simulation frames. It does not connect to a server.

Verification: all 22 `tutorial_nano_gameplay::tests` passed. The first development
link failed on undefined anonymous LLVM symbols in the incremental client
library. The retry uses `CARGO_INCREMENTAL=0`, without changing source flags or
the runtime assets. The retry successfully built both the real client binary
and the GPU fixture. The fixture exited successfully with `Nano 24 active`;
the captured image was visually inspected and shows Swampfire during summon.
Use the catalog's model mapping: Swampfire's current native Nano ID is 24;
the legacy icon-name helper's 30 is not the authoritative model ID.
Final front-view capture: `../FFOneClient/target/performance/swampfire-summon.png`,
SHA-256 `49ce0340c34fa62899be9da96f08a1cd50ab0ecd1a521f7b7c7544d2fc2896ef`.
