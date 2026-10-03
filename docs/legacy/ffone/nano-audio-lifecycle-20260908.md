# Nano voice lifecycle and dismissal

Evidence question: why do all native Nanos omit farewell on recall, and does
the native animation/audio handoff preserve primary waiting, ownership and
departure semantics? Consumers are FFOne's gameplay audio, Nano animation
machine, gameplay Nano command queue and ordinary-world presenter. Acceptance
requires a dismissal request on actual recall, survival of current speech after
model deletion, waiting before farewell load/play, localized catalog resolution,
and no farewell on replacement or stamina withdrawal.

## Primary authority and reproduction

Only current primary (`retrobution-20260821`) establishes the behavior. No
historical build or alternate donor establishes a state-machine rule here.
All commands below run from FusionForge; generated files stay under `work/`.

```powershell
./tools/legacy-sources/find.cmd -Source primary -Query main.unity3d
New-Item -ItemType Directory -Force work/cases/nano-dismiss-20260908/decompiled
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/cases/nano-dismiss-20260908/assembly.evidence.json --payload-out work/cases/nano-dismiss-20260908/Assembly-CSharp.dll
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp - first pass.dll" --level 0 --out work/cases/nano-dismiss-20260908/firstpass.evidence.json --payload-out work/cases/nano-dismiss-20260908/Assembly-CSharp-firstpass.dll
dotnet vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.dll work/cases/nano-dismiss-20260908/Assembly-CSharp.dll -t NanoContainer -o work/cases/nano-dismiss-20260908/decompiled --preserve-iterator-state-machines
dotnet vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.dll work/cases/nano-dismiss-20260908/Assembly-CSharp.dll -t NanoAnimation -o work/cases/nano-dismiss-20260908/decompiled --preserve-iterator-state-machines
dotnet vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.dll work/cases/nano-dismiss-20260908/Assembly-CSharp.dll -t cnOwnAvatarStatus -o work/cases/nano-dismiss-20260908/decompiled --preserve-iterator-state-machines
dotnet vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.dll work/cases/nano-dismiss-20260908/Assembly-CSharp-firstpass.dll -t AnimationEventHandler -o work/cases/nano-dismiss-20260908/decompiled --preserve-iterator-state-machines
```

Producer checkout: `857406bc95b5dd7a5983cefc3838c86b302600a8` (working tree).
The exact managed-entry exporter includes container, payload and CLR hashes.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| primary `main.unity3d` | 8221718 | `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF` |
| level0 `Assembly - CSharp.dll` | 1762816 | `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792` |
| level0 `Assembly - CSharp - first pass.dll` | 396288 | `AFFF470BEC30BF06703EB0664DE364CE37EA7B8092C848420A92FBA63E91DA81` |
| `NanoContainer.decompiled.cs` | 17890 | `881C680382044F6E1D303365981E06E08F5CA21BD5EADFE0B726F1EFA242AD4F` |
| `NanoAnimation.decompiled.cs` | 11388 | `91A9AD93CDC0DA2080271A17FC2D74E4934F209F47B09972FB3A7370FF884ECA` |
| `cnOwnAvatarStatus.decompiled.cs` | 42179 | `0C913BD4181F683FAEE94149A39CF1CF924AD2E8CE43908DE0BAE6E0412237A2` |
| `AnimationEventHandler.decompiled.cs` | 18411 | `284C5338EFB86333CA24ECD2CCB19155D14F4D67A5C50FBF4E8DB0BCD64A47F2` |

The four decompiled artifacts are in the case's `decompiled/` directory.
This investigation changes managed behavior contracts only; no serialized
model/audio payload is republished and no new PPtr match is accepted.

## Proven branches

`cnOwnAvatarStatus.ReceiveNanoActive` maps a negative active-slot reply to
`NanoContainer.AddNano(-1)`. `ReceiveNanoEquip`/`ReceiveNanoUnEquip` also use
that branch on confirmed deactivation. `PCTick` uses `AddNano(0)` at zero
stamina. Positive IDs replace the previous Nano directly.

`NanoContainer.AddNano(-1)` returns immediately if no current Nano exists.
After checking the current move/string data and SoundUtil's range/count gate,
it selects take 1–3 and starts its own `VOLoad(<name>_NanDismiss0N.wav)`.
It requests effect 10 at the Nano, calls `Hide`, and clears the Nano pointers.
The farewell is not an AnimationEvent in `withdraw`.

`NanoContainer.VOLoad` snapshots the current animation handler's `currentVO`.
If playing, it waits for the remaining clip time, then requests the asset.
On successful loading it creates an independent VOSound GameObject at the
avatar's then-current position, with voice gain, non-looping playback and
rolloff factor 0.1. It is not parented to the removed Nano or the avatar.
The container coroutine is cancelled if its avatar disappears before playback.

`AnimationEventHandler.VOLoad` likewise waits for its current speech before
loading, spawns a detached voice at the model's then-current position, and
updates `currentVO`. Requests wait on the source current at request time;
this is not a newly invented global FIFO or an NPC-style replacement channel.
Model removal cancels pending animation coroutines, not a started VOSound.

`AddNano(0)` calls `NanoAnimation.Withdraw`: effect 10, crossfade to withdraw
for 0.1 seconds, removal at its animation completion. It does not request the
manual-dismissal family. A positive replacement also omits that family.

`NanoAnimation.IsStand` permits 1/20 idle sound events unless `bForcesound` is
set. `SetStandMotion` chooses discharge below 20% of max stamina, forces its
first pass when entering from a different clip, and otherwise chooses stand1–3
with missing normal-idle fallback to stand1. `_SFX_Dance` bypasses the idle
sound suppression. These gates precede the sound payload random expansion.

## Native correction

- Explicit dismissal queues its voice before destroying the model; repeated
  absence produces no additional request. Replacement/cleanup remain silent.
- Dedicated Nano pending/current voice state replaces model-child playback for
  semantic Nano voices. Waiting requests hold no audio handle until the previous
  source finishes; loading failures and loss of the coroutine owner release them.
- Playback is detached at the emission position and carries `LocalizedVoice`
  and the normal voice mixer channel. The catalog resolves every physical path.
  A language change during pending loading re-resolves the handle.
- Dismissal uses the model's native semantic Nano owner and take 1–3. Catalog
  ownership, not localized display copy or a guessed voice-prefix spelling,
  selects the asset. The production-registry test validates this owner mapping.
- Zero-stamina presentation uses Withdraw separately from explicit Dismiss;
  repeated synchronization cannot restart the withdrawal. Missing presentation
  definitions use silent Hide. Optional withdraw/discharge clips enter the graph
  when the model provides them.
- Idle sound admission and low-stamina first-pass behavior are restored; normal
  Buttercup model events outside the separately owned fixed call/skill1 clips are
  no longer skipped wholesale.

All 183 selected farewell takes for 61 native Nano owners resolve in both voice
languages: 183 have English files; 12 have Russian files, and 171 use the catalog's
English fallback for Russian. Five other registry models have no dismissal family
in the installed catalog: `nano_alienx`, `nano_ghostfreak`, `nano_holonano`,
`nano_runty`, `nano_upgrade`. No speech is substituted for these owners.

## Acceptance and limits

`crates/ffone-client/tests/nano_audio_standalone.rs` imports the production audio,
Nano gameplay and animation-machine source modules. The first direct-rustc run
passed all 50 tests, including production EN/RU paths, deferred loading, current
voice survival, farewell timing and independent source ownership. Follow-up
tests additionally cover repeat-dismiss suppression, replacement and depletion.
The harness is usable while other tasks hold the shared Cargo build lock.

Run the normal Cargo checks from FFOne when its shared target is available:

```powershell
cargo test -p ffone-client --test nano_audio_standalone -- --test-threads=1
cargo build -p ffone-client --bin ffone-client
```

The case's direct build/test logs and executable are below FFOne's ignored
`target/performance/nano-audio/`; this is native build output, not legacy evidence.
Test audio readiness/completion is injected deterministically without a sound
device. This establishes ECS transitions and catalog selection, not acoustic
equality to a live Unity capture. Full Unity runtime audio capture, remote-avatar
presentation coverage and a general SoundShotManager periodic-distance audit are
not claimed by this case. No FPS or sound-quality improvement is inferred.

Final verification: all **51** tests passed (0 failed), including repeated recall,
replacement and depletion. The production library and main executable built via
direct rustc with existing Cargo development dependencies; `--validate-assets`
returned `status: ok`. The only compiler warning was the unrelated existing
`CameraProjection` import in app/mod.rs. The queued redundant Cargo test was
cancelled after successful isolated verification; other tasks were not stopped.
The verified executable is `../FFOneClient/target/performance/nano-audio/ffone-client.exe`
(257683968 bytes, SHA-256 `d47e644126e7592eb95ace9330423d07ccda05a8a353453df5856613e0474812`).
Exact native verification artifact hashes are recorded in
`docs/reference/evidence/cases/nano-dismiss-audio-20260908.json`.
