# FusionForge export reuse and source roles — 2026-09-13

The source registry and reverse-engineering skill now distinguish current Retrobution
parity, previous-build appearance retention, Academy donations, early beta orientation,
unassigned Retro references, and the unverified Reborn cache candidate. Production
NPC/Nano identities and retained UI take precedence over raw source row numbers.
See the skill's `references/native-divergences.md` and the source registry.

## Export changes

- Exact extraction resolves non-null Unity pointers against their real owner/external
  table and fails on missing dependencies. It cannot substitute an unrelated loaded
  object with the same path ID or silently turn that failure into an empty binding.
- Exact texture caching belongs to a loaded Unity environment. Reusing a memory address
  for another environment cannot reuse the first environment's decoded texture.
- `fusionforge index-native-textures` creates a reusable installed-texture index.
  `fusionforge publish-logical-model --reuse-texture-index` uses it inside the existing
  native publisher. All mip metadata, sampler/color contracts and PNG bytes must match;
  matched owner GLBs are hash-checked again. A changed owner requires index refresh.
- Exact static material records share a material index while primitives and slot order
  remain. Animated or dynamic ownership is conservatively excluded from this merging.
- Shared texture paths inside the native asset tree survive schema validation and tree
  audit, including existing names such as `toonramp9_variant_02.png`. Root escape,
  missing dependencies and changed mip bytes still fail validation.
- Default scratch creation is anchored to Editor `work/sessions`; source work remains
  in `work/sources`. Retired project/scratch defaults are no longer needed. Historical
  cache folders were preserved; this change does not delete them.

## Reproduction and retained data

Run from FusionForge:

```powershell
.\fusionforge.cmd index-native-textures ..\FFOneClient\assets\game work\native-texture-reuse\index.json
.\fusionforge.cmd publish-logical-model work\cases\export-regression-20260913\building.source.json characters work\cases\export-regression-20260913\new-stage --semantic-directory npc --reuse-texture-index work\native-texture-reuse\index.json
```

Use a fresh staging root. Install only reviewed final native files at the same relative
paths; preserve byte-identical installed dependencies. Indexes and publication reports
remain Editor data. The index currently contains 1,226 immutable texture chains. Four
missing ToonRamp9 bindings were excluded and recorded: melee_eduardoclub, melee_razor,
rocket_mojotankcannon and shattergun_wilt. Exclusion does not repair those installed files.

## Verification

- FusionForge debug binary built successfully and exercised actual export/publication.
- Logical-model publisher tests: 53 passed, 1 ignored.
- Structural tree audit tests: 11 passed, including shared variant paths, retained exact
  names, missing shared files and root-escape rejection.
- Native model schema tests: 13 passed.
- Full Editor library suite after updating two live fixture cache paths: 389 passed,
  4 failed, 1 ignored. Remaining failures are the negative-file-ID preview fixture,
  conflicting-constant-curve fixture and two Otto tests requiring an absent dump.
  The first two fail assertions in unchanged code; this work does not declare the
  overall suite green. See `work/fusionforge-unit-tests-final.log`.
- Real previous-build `npc_building` exact export and both baseline/reuse publications
  pass structural checks: all 19 textures (173 mip PNGs) match baseline and installed
  bytes; 25 primitives and 19 material records remain.
- Real `f_shirt_secretagent` publication reuses the existing shared ToonRamp9 variant
  with identical bytes; two material records remain. No post-export migration was used.

Evidence: `docs/reference/evidence/legacy/ffone/sources/export-reuse-regression-20260913.json`.
These are staging regressions, not a runtime visual acceptance or content installation.

## Reborn limitation

`../builds/reborn-cache-candidate-20260913` preserves 978 shared Unity-cache files and
the historical Reborn runner (979 files, 2,559,038,431 bytes), with verified copy hashes.
The July 22 task identified that cache, but later sessions changed it. It is incomplete,
and attribution of individual files to Reborn is not proven. No main.unity3d was recovered.
The site's configured client endpoints returned 404; the runner's historical origin
timed out. A complete Reborn download remains unavailable from those endpoints.
See `docs/reference/evidence/legacy/ffone/sources/reborn-cache-20260913.json` before using this donor.
