# Native glow overexposure correction

Question: why do FFOne glass, summon silhouettes and foliage become saturated
white/cyan/yellow with broad halos? Consumer: FFOne `legacy_glow.wgsl`.
Acceptance: the GPU blur must preserve a constant field, opaque framebuffer
alpha must suppress glow, and intensity must scale the contribution once.

The owner explicitly requested correcting this behavior on 2026-09-11. This is
a **native extension**, not a claim of primary parity. No model, texture or
material publication is needed.

Primary authority was re-exported from `retrobution-20260821/main.unity3d`:

```powershell
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d 'Assembly - CSharp - first pass.dll' --level 0 --out work/cases/glow-overexposure-20260911/assembly.evidence.json --payload-out work/cases/glow-overexposure-20260911/firstpass.dll
```

Raw container SHA-256:
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
Assembly SHA-256:
`AFFF470BEC30BF06703EB0664DE364CE37EA7B8092C848420A92FBA63E91DA81`.
This matches the assembly used by the existing
`work/projects/retrobution-ui-20260821.ffclient/reports/nano-summon-primary/decompiled/GlowEffect.cs`.

`GlowEffect.blurMatString` weights the first tap by constant alpha and adds
three taps weighted by constant RGB. `OnRenderImage` sets RGB=1 and
alpha=0.25+clamp01((intensity-1)/4). At native intensity 1.8, a constant field
is multiplied by 3.45 per pass, or 141.6695 over four passes before saturation.
The existing FFOne equation faithfully copied this problematic amplification.

The correction averages four taps equally and applies nonnegative intensity
once in composition. It retains alpha masking, tint, filter color, four blur
passes, offsets, and the composition factor of two. It deliberately removes
the exponential gain and directional weighting. The expected constant-field
encoded RGB is `source * filter + source * tint * (1-alpha) * 2 * intensity`.

Native verification entry points:

```powershell
cargo test -p ffone-client --lib legacy_glow::tests --locked
cargo run -p ffone-client --example glow_energy_gpu_probe --locked -- 0 1.8
cargo run -p ffone-client --example glow_energy_gpu_probe --locked -- 1 1.8
cargo run -p ffone-client --example glow_energy_gpu_probe --locked -- 0 0
cargo build -p ffone-client --bin ffone-client --locked
```

GPU outputs belong below FFOne `target/performance/glow-energy`. The probe
compares actual framebuffer RGB against the constant-field energy contract;
it does not establish visual parity for every affected world placement.

Verification results (2026-09-11): both unit tests passed; the actual development
client built and completed the offline world/NPC fixture without errors, saving
`target/performance/glow-corrected-world/frame.png`. GPU checks passed:
alpha=0/intensity=1.8 produced RGB `[66,131,196]`; alpha=1/intensity=1.8 and
alpha=0/intensity=0 both preserved source RGB `[51,102,153]`.
The specific Titan creation frame, Courage foliage, catacombs and terrafuser
placements were not replayed; no claim of per-scene visual acceptance is made.
