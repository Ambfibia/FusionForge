# Galaxy Gardens purple atmosphere investigation

The owner requested restoration of a purple appearance remembered across the
whole location. The desired reference remains unresolved; no native environment
or runtime change was published. This investigation supersedes neither the
owner's new request nor the earlier turquoise acceptance with an inferred choice.

## Raw map comparison

All selections are MonoBehaviour 25605 in `BuildPlayer-Map_05_05`, inside the
source's registered `Map_05_05.unity3d` route. Full hashes, field values and replay
commands are in `docs/reference/evidence/cases/galaxy-purple-filter-20260913.json`.

| Sources | Fog RGB | Light RGB | Fog depth |
| --- | --- | --- | --- |
| primary 20260821, previous 20260613, retro-010920 | 0.1, 0.25, 0.25 | 0.9, 1, 1 | 1 |
| retro-110219, retro-112219, retro-121719 | 0, 0.5411765, 0 | 0.7843137, 0.9607843, 0.7372549 | 1 |

The installed native `map_05_05` environment matches primary values exactly.
These are serialized atmosphere comparisons, not proof of complete frame parity.
Primary's named external Tutorial asset resolves MonoScript 1907 to DongColorSetup.
The historical script pointers remain triage evidence, not independent ownership
proof. Map and resource discovery dumps stay below Editor `work/`.

## Purple candidate in primary managed code

`HalloweenEvent.ApplyAmbience` sets a purple fog color:

```
fog.rgb = (white * 0.01 + RGB(120, 0, 255) * 0.00035) * 0.8
        = RGB(0.0416, 0.008, 0.0794)
light.rgb = RGB(0.5, 0.5, 0.6)
fogDensity = fogDepth * 0.007  // paid world, including Galaxy Gardens
```

Tutorial/free-world uses the separate 0.0165 density factor. Sky tint alpha
becomes 0.95 after assigning RenderSettings fog. The event also replaces the
skybox and Fusion star. Its ambience callback has no Galaxy Gardens gate.
`GameEventManager` obtains the active event from `themedEvent.php` and activates
Halloween by name. A remembered screenshot cannot yet be attributed to this event.

Primary Assembly-CSharp was freshly extracted with FusionForge and matches the
existing decompilation input: SHA-256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The evidence record includes the raw main container and exact assembly entry.

## Tooling validation

Historical Retro maps use the version-2 `streamed` header signature. The
ffbuildtool reader now accepts it while retaining version/player/engine checks
and preserving the signature during roundtrip. New bundle creation still uses
UnityWeb. Four original historical maps successfully decoded through FusionForge.
Header roundtrip and invalid-version tests passed (2 tests). The FusionForge
development binary built successfully. Source builds were not modified.

To finish the requested restoration, identify the owner's reference appearance
or establish that Halloween is the intended donor. A location-only Halloween
override would be an explicit native divergence, not the recovered default zone.
