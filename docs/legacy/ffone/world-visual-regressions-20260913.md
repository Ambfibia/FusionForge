# World visual regressions, 13 September 2026

The owner reported disappearing vendor icons in Cul-de-Sac, missing Nano
status effects, and an intermittently invisible or incorrectly cycling vortex
above the liner in the Area 51 infected zone. The owner subsequently confirmed
that Galaxy Gardens should retain Retrobution's turquoise environment. No
Galaxy Gardens environment change was published.

## Source and classification

Retrobution 20260821 is `primary`. Academy was a comparison only, not a donor.
The source-scoped, hashed shader/animation evidence is recorded in
`docs/reference/evidence/cases/world-visual-regressions-20260913.json`. Native publication hashes
and replay commands are in:

- `recipes/native/effects/nano-status-effects-primary-20260913.receipt.json`
- `recipes/native/animations/area51-ship-vortex-repeat-20260913.receipt.json`

The vortex material repair and Nano resources follow recovered primary state.
The seamless vortex cycle is an explicit owner-requested repair: the primary
compressed tracks were not all authored as closed loops. It must not be described
as a byte-identical recovery of primary animation timing.

## Render ordering and the vortex

Native particle materials previously had zero transparent sort bias while world
materials had an independent positive bias. A depth-writing transparent world
surface could therefore paint over a foreground vendor particle as camera depth
ordering changed. Particles and mesh effects now use the same queue mapping as
world materials. A queue step of 2048 native units keeps adjacent queues distinct
across the supported camera range. The existing depth test remains active.

The ship vortex contains twelve model placements (`static-map_12_10-01067`
through `static-map_12_10-01078`). Eight used the unsupported exact shader
`normal_blendSrccolorInvsrcalpha_zwriteOff_cullOff`. Its recovered contract is
SrcColor / OneMinusSrcAlpha, RGB writes, no depth write, Cull Off, queue 3010,
lighting and fixed-function fog. The other four shells use additive queue 3011.
Supporting the missing shader restores those eight surfaces without replacing
their textures, blending, geometry, culling or placement.

The 3.99-second clip contains shorter rotation channels. Repeating only the
whole clip left short channels waiting at their last frame. Explicit per-channel
repeat now samples each shell at its own period. Cylinder06, Cylinder12 and
Cylinder13 ended at 0.96 seconds without closing; the tracked repair adds a
closing sample at 1.0 second, retaining every interior key. Other channel periods
and the containing clip duration remain unchanged. This repair is limited to
the confirmed vortex. Other world objects were not rewritten by a heuristic.

## Nano status presentation

`Status.UpdateSkillBuff` and `Status.ProcessInstantBuffEffect` define condition
mask ownership, SkillBuff table effect IDs, attachment priorities and NPC scaling.
Managed evidence comes from the hashed primary assembly export under
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary`:
`main.unity3d` SHA-256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`,
Assembly-CSharp SHA-256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.

The native world now owns persistent effects by combatant and condition bit.
Effects wait for attachment bones while models load, and are removed on mask
clear, death or despawn. Validated Nano results supply one-shot status visuals;
timeout packets also clear remote PC and NPC masks without changing HP or pose.
Thirteen missing effect definitions and seven mesh models were recovered from
Effects/Tutorial resources. A fresh Editor replay produced the same 267 final
files as the initial staging and installation.

The two newly encountered Nano shader programs retain their exact source state:
vertex-color AmbientAndDiffuse with SrcAlpha/One, and the SrcAlpha/One alpha-test
program with default depth writing and cutoff zero. Mesh material curves keep
instance ownership: shared materials detach before animation, and numeric edits
use the existing uniform upload queue. No localized UI text was introduced.
Material animation now visits only the effect's descendants, preserving the
original exclusion of the owning root itself. It no longer scans every material
in the loaded map for every curve. The unnamed production player root is supported
through direct entity ownership when no attachment bone is required.

This slice covers status/buff presentation and its native resources. It does not
establish complete parity for every Nano's cast animation or every protected-hit
special effect. Packet/lifecycle tests and offline production captures are distinct
from a live server combat acceptance run.

## Verification

The channel repeat tests cover all twelve shells at 1, 3 and 17 periods.
Lifecycle tests cover repeated updates without duplicates, removal, and an instant
result arriving before its attachment bone. Material ownership tests retain the
deferred asset extraction regression and the pending-final-pose path.

Runtime captures use the opt-in full-client fixture under the native client's
ignored `target/performance/visual-regressions` tree. Final capture paths and test
results are recorded in the case and publication receipts. The native client
material suite passed 66 tests, the focused contract suite passed 16, and the
world skill suite passed 4. The Editor shader suite passed 23 with 1 ignored.
`cul-nano-final` and `cul-nano-alpha-test` exercise both new Nano shader families
and vendor icons against houses at two headings. `vortex-west-verified` and
`vortex-south-verified` show the twelve-shell effect above the liner from two
sides. These runs reported no shader validation, unsupported material or missing
effect texture errors. The NPC fixture still reports existing unrelated missing
NPC catalog rows.

The frozen sleep-effect region (pixel rectangle 620,680 to 1400,1020) is identical
between numeric uniform updates and the normal asset-event baseline. Full-frame
mean absolute channel difference is 0.2658/255 because the background is not
completely identical. Animated captures retain the same appearance; their
animation phases and temporal particles differ, so they are not pixel-equality
evidence. The deferred extraction and independent-material tests also pass.
No FPS improvement is claimed: this is a visual repair, not a matched performance
comparison.
