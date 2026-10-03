# Nano summon coverage and consecutive acquisition

Evidence question: can a missing optional animation hide a summoned Nano, and
can delayed or stale acquisition animation work shorten a selected ability's
result or reveal a preview after it was hidden? The reported sequence is a
successful award, selection, fast disappearance, then a brief reappearance on
the next award. Belladonna is included explicitly in regression coverage.

## Authority

Primary is `retrobution-20260821`. No model or texture payload was changed by
this repair. A fresh exact assembly export and focused decompilation were run
from FusionForge:

```powershell
New-Item -ItemType Directory -Force work/cases/nano-acquisition-20260908/decompiled
./work/build/debug/fusionforge.exe export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/cases/nano-acquisition-20260908/assembly.evidence.json --payload-out work/cases/nano-acquisition-20260908/Assembly-CSharp.dll
dotnet vendor/FFSpy/ICSharpCode.Decompiler.Console/bin/Debug/net6.0/ilspycmd.dll work/cases/nano-acquisition-20260908/Assembly-CSharp.dll -t NanoFreeTuningMode -o work/cases/nano-acquisition-20260908/decompiled --preserve-iterator-state-machines
```

Raw `main.unity3d` SHA-256:
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
Exact `level0/Assembly - CSharp.dll`, 1,762,816 bytes, SHA-256:
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
`NanoFreeTuningMode.decompiled.cs` SHA-256:
`84ab2ae46e221198430015868fc6af6ff1c25224bf4ec78e85a1301e9440c1a2`.

`ReceivePacket` calls the selected `Skill1/2/3`, enters phase 6, and resets the
clock in that order. Phase 6 waits for the current animation's length, then
`Hide` removes the model and phase 7 completes the acquisition. Native queued
playback must therefore start the result clock when the animation is actually
bound, and cannot reveal the preview after hide/close. The ordinary idle
fallback comes from `NanoAnimation.SetStandMotion`; see
`swampfire-summon-20260907.md` and `nano-audio-lifecycle-20260908.md` for its
independent source identities and exact spelling evidence.

## Findings and native changes

- The installed Belladonna GLB contains all three skills. Its durations are
  approximately 2.467, 1.400 and 1.633 seconds. Its initial single-model GPU
  summon succeeded with the current assets; the reported full acquisition
  sequence was not established by that single-model probe.
- Across all 66 registry models there are no missing call/base-idle clips or
  missing directly referenced images. Native `Stand1` is retained for the
  Ghostfreak model. Some models lack optional idle/skill variants. The audit
  is written to FFOne's ignored `target/performance/nano-animation-audit.json`.
- World readiness now depends on the call/base-idle contract, not availability
  of a selected skill animation. An unavailable skill never selects a different
  ability, changes server authority, or hides the entire model.
- Acquisition uses the source base-idle fallback for missing random variants.
  Its result timer begins at playback binding, once per result. Hide/close
  clears pending animation work before it can reveal the old preview.
- Explicit recovery extension: if an installed model has no selected skill
  clip, acquisition holds its own neutral base idle for one full cycle before
  closing. This avoids the former zero-duration disappearance without claiming
  that an unavailable ability animation was recovered or borrowing another
  skill. The server-selected ability is unchanged.

## Tuning table collision (confirmed acquisition failure)

The native and server tables retained duplicate `m_iTuneNumber` values after
extension rows were appended. OpenFusion builds a map by that value, so later
rows overwrite the earlier Nano's skill. Native acquisition correctly rejects
the resulting reply as `SkillMismatch`; weakening that check would conceal the
wrong server ability. The preimage reproduces 11 wrong choices on six Nanos:
41 (choices 2/3), 42 (choice 1), 43 (choices 2/3), 44 (all three), 45 (choice 1),
46 (choices 2/3). Belladonna expects skill 213 but receives Cheese's skill 239.
This also explains Belladonna's absent summoned model: `world_nano_skill_slot`
cannot map the saved foreign skill 239 to one of Nano 42's three skills, and the
production world presentation hides/declines the model at that gate. The table
and saved-skill repair restores the correct mapping without weakening it.

The native repair assigns unique tuning wire numbers equal to their existing
array-row identities, preserving every Nano's three references and skill data.
It updates both client and server data and resolves localized power labels from
each row's existing localized string reference. The 183 current choices now
resolve to the same intended skill on both sides.

Cheese additionally had a null first skill, preventing projection of its tuning
window. Its previously accepted Academy extension is repaired from a fresh raw
`alternate/TableData.resourceFile` export, 751,481 bytes, SHA-256
`d724c9417de5ff35e9bdb7bdfcf9bb93b4c8f715f42623fa2e1834d835be60a5`.
`list-contents` establishes the unique `xdtdatas` MonoBehaviour, path ID
2139558964, in `CustomAssetBundle-TableData`. `dump-xdt` exports that exact object.
Source Nano array row 41 references tune 211, which references skill 211.
That exact healing skill is appended as native skill 286, preserving every
other field including sound 7 and icon reference 5 (full icon record equal).
Native skill 279 was rejected as a substitute because its sound value differs.

Replayable repair: `tools/native/repair-nano-tuning.py`. Four preimages and staged
outputs are retained below `work/cases/nano-acquisition-20260908/table-repair`.
An independent idempotent replay in `table-replay` reproduced all four hashes.
The semantic diff contains only tune numbers, Cheese's first skill reference,
the new skill record, and `content.nano_tune.*` localization entries. EN/RU key
sets and placeholders match. Receipts are in `recipes/native/tables` under
`nano-tuning-identities-20260908-client/server`.

## Verification results

Final production-library Nano tests: 114 passed, including the 61 catalog Nanos with
all three skill selections (183 summon cases), delayed result binding,
localization/audio tests, and reentry state clearing.

Additional production-app test exercises consecutive Belladonna, Swampfire,
Cheese, Chowder, every current Nano and Belladonna awards with all three selections
(198 cases). It uses actual
GLB animation names/durations and the production preview synchronization system.

```powershell
cargo test -p ffone-client --lib nano_ -- --test-threads=1
cargo test -p ffone-client --bin ffone-client app::tests::nano_free_tuning -- --test-threads=1
$env:CARGO_INCREMENTAL = '0'
cargo build -p ffone-client --bin ffone-client --example world_nano_summon_gpu_preview
./target/debug/examples/world_nano_summon_gpu_preview.exe all target/performance/nano-summon-audit
```

The batch GPU fixture switches the actual production model for each of the
183 Nano/skill cases and captures 30 simulation frames after activation.
GPU batch completed with exit 0: all 183 cases reached active state and produced
captures. The 61-model contact sheet was visually inspected; every model rendered.
This checks summon rendering, not an end-to-end player/server acquisition session.
Final production-app tests after the table repair: 6 passed, including all 198
consecutive acquisition cases. The independent table loader fixture also passed.
The final development binary build passed (`CARGO_INCREMENTAL=0 cargo build
-p ffone-client --bin ffone-client`); FFOne's `target/debug/ffone-client.exe`
was rebuilt at 06:06 Moscow time. The final library rerun passed all 114 tests
with zero failures, including the corrected localized tuning aliases and the
Cheese gallery's native skill/tuning identities.

The local OpenFusion server had only listening sockets and no connected players.
It was stopped through its Windows console Ctrl-C handler (console membership
verified before signaling), then started from its existing `bin` directory with
the repaired XDT. It loaded successfully and listens on ports 23000 and 23001;
no stderr was emitted. Startup evidence is in FFOne's ignored
`target/performance/nano-server-restart.log`.

Read-only inspection then found exactly one saved instance of the proven
Belladonna collision: Nano 42 had skill 239, which is not one of its valid skills.
`tools/native/repair-saved-nano-tunes.py` derives only unambiguous corrections
from the old server-map overwrite and the repaired native table, excluding
untuned zero skills and valid owned abilities. With no connected players, it
backed up SQLite to ignored `work/cases/nano-acquisition-20260908/`
`database-before-tune-repair.db` and changed that one skill from 239 to 213 in
one transaction. Row comparison confirms only that skill changed; Nano ownership
and stamina were preserved. SQLite `integrity_check` returned `ok`.

The current native table-set also contains `native_asset_routes`. The mission
consumer now selects the unique `npc_imports_consolidated` document by name,
matching the other gameplay table consumers, instead of rejecting any second
document. Duplicate gameplay documents still fail closed; a fixture exercises
the independent routes document before the gameplay document.
