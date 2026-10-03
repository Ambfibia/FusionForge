# Retrobution logical-model extraction

This records the strict, non-production extraction from
`builds/retrobution-20260613`. The generated trees are intentionally under
`work/legacy-sources`; they do not replace `assets/game/models` or install a
partial catalog.

## Reproducible commands

Run from the workspace root:

```powershell
.\FusionForge\target-build\debug\fusionforge.exe fusionforge catalog-logical-models `
  .\builds\retrobution-20260613.ffclient\cache\bundle-index.json `
  .\work\legacy-sources\retrobution.logical-model-catalog.json

.\FusionForge\target-build\debug\fusionforge.exe fusionforge export-logical-model-sources `
  .\builds\retrobution-20260613.ffclient\cache\bundle-index.json `
  .\work\legacy-sources\logical-model-sources-retrobution
```

Both output paths must be fresh. The source exporter recomputes the complete
plan from the bundle index and publishes its source tree plus sibling manifest
atomically.

Run from `FFOneClient`:

```powershell
.\target\debug\ffone-asset-pipeline.exe publish-logical-model-batch `
  .\work\legacy-sources\logical-model-sources-retrobution `
  .\work\legacy-sources\logical-models-candidate-retrobution

.\target\debug\ffone-asset-pipeline.exe audit-logical-model-tree `
  .\work\legacy-sources\logical-models-candidate-retrobution `
  .\work\legacy-sources\logical-models-candidate-retrobution.audit.json

node .\tools\legacy-sources\ffone-migration\report-logical-prop-candidates.mjs `
  .\work\legacy-sources\retrobution.logical-model-catalog.json `
  .\work\legacy-sources\retrobution.logical-prop-candidates.json

node .\tools\legacy-sources\ffone-migration\run-logical-model-gpu-batch.mjs plan `
  --candidate .\work\legacy-sources\logical-models-candidate-retrobution `
  --preview .\target\debug\examples\logical_model_gpu_preview.exe `
  --output .\work\legacy-sources\logical-model-gpu-batch-plan-retrobution.json

node .\tools\legacy-sources\ffone-migration\run-logical-model-gpu-batch.mjs run `
  --plan .\work\legacy-sources\logical-model-gpu-batch-plan-retrobution.json `
  --candidate .\work\legacy-sources\logical-models-candidate-retrobution `
  --evidence .\work\legacy-sources\logical-model-gpu-evidence-retrobution `
  --preview .\target\debug\examples\logical_model_gpu_preview.exe `
  --output .\work\legacy-sources\logical-model-gpu-batch-run-retrobution.json

# Retry only failed models with a larger bounded warm-up budget.
node .\tools\legacy-sources\ffone-migration\run-logical-model-gpu-batch.mjs run `
  --plan .\work\legacy-sources\logical-model-gpu-batch-plan-retrobution.json `
  --candidate .\work\legacy-sources\logical-models-candidate-retrobution `
  --evidence .\work\legacy-sources\logical-model-gpu-evidence-retrobution `
  --preview .\target\debug\examples\logical_model_gpu_preview.exe `
  --output .\work\legacy-sources\logical-model-gpu-batch-run-retrobution.json `
  --resume --frames 900 --timeout 45

.\target\debug\ffone-asset-pipeline.exe audit-logical-model-gpu-evidence `
  .\work\legacy-sources\logical-models-candidate-retrobution `
  .\work\legacy-sources\logical-model-gpu-evidence-retrobution `
  .\work\legacy-sources\logical-model-gpu-evidence-retrobution.audit.json

node .\tools\legacy-sources\ffone-migration\summarize-logical-model-gpu-batch.mjs `
  --run .\work\legacy-sources\logical-model-gpu-batch-run-retrobution.json `
  --audit .\work\legacy-sources\logical-model-gpu-evidence-retrobution.audit.json `
  --output .\work\legacy-sources\logical-model-gpu-blockers-retrobution.json
```

## 2026-08-05 planner correction

The exact Unity archives `library/unity default resources` and
`resources/unity_builtin_extra` are system-owned external references, not
missing Retrobution bundles. The planner now retains those pointers as explicit
external evidence instead of reporting them as unresolved. It does not relax
resolution for any other archive name.

The corrected full primary plan has 395 catalog KFM routes, 254 parsed/ready
logical roots, zero blocked logical roots and zero unresolved model references.
It retains 114 Unity system pointers across 57 roots. The blocker inventory is
now 942: 800 NIF physical-target conflicts, 113 KFM physical-target conflicts,
25 failed self-contained GameObject proofs, two genuinely unresolved KFM
preload groups, and one each for incomplete standalone-NIF proof and a closure
without a matching NIF pointer.

This makes 59 additional primary KFM roots eligible for a fresh source export;
it does not retroactively validate or publish them. They remain
`primary_ready_unpublished` until the source batch, structural audit, GPU
evidence and runtime classification gates below have passed. No alternate donor
is appropriate for those roots because clean primary already owns them.

## 2026-07-20 result

The catalog contains 395 unique KFM routes and 3,079 unique NIF routes from
11,220 occurrences. The full plan found 195 ready self-contained KFM roots,
but `ownershipScanComplete=false` and `standaloneNifProofComplete=false`.
Its 1,001 blockers are:

- 800 `nifPhysicalTargetConflict`;
- 113 `kfmPhysicalTargetConflict`;
- 59 `kfmPreloadPointersUnresolved`;
- 27 `kfmSelfContainedGameObjectProofFailed`;
- one `standaloneNifProofIncomplete`;
- one `kfmPreloadClosureHasNoNifPointerMatches`.

The source batch exported 189/195 roots. Six roots failed closed instead of
losing animation data:

- `mob/npc_bubbie.kfm`, `mob/npc_ghostduck.kfm` and
  `mob/npc_unstablenano.kfm`: unsupported serialized float curves;
- `mob/npc_ed.kfm`, `nano/nano_ghostfreak.kfm` and
  `nano/nano_swampfire.kfm`: conflicting duplicate node/property animation
  channels that glTF cannot represent losslessly.

Publication produced 188 GLBs and 1,429 PNGs. The remaining
`mob/npc_spidermonkey` source is blocked by an empty serialized material texture
slot name; no ShaderLab slot was guessed.

The independent structural audit passes with zero violations:

- 12,054 nodes, 363 mesh parts and 287 materials;
- 238 skins, 9,143 joints and 9,143 inverse-bind matrices;
- 274,878 weighted vertices;
- 2,776 preserved clip records: 2,701 standard glTF animations and 75
  metadata-only clips;
- 301,082 animation channels and 2,832,767 keyframes;
- 4,808 animation events;
- zero orphan PNGs and zero extra model formats.

Every one of the 188 per-model reports has
`semanticProof.status=native-model-matches-redecoded-glb`, matching source and
published counters, and no unresolved source features. This proves the
inspectable artifact roundtrip. It does **not** prove complete Bevy GPU
rendering/playback or runtime spawn policy: the candidate report remains
`candidatePublishable=false`, `gpuGatePending=true`, and
`coordinateRuntimeSpawnPolicyPending=188`.

The aggregate real-Bevy run preflighted all 188 GLBs by exact root name and
animation name. It selected exact `stand1` for 180 models, another exact
standard clip for three models, and treated five genuinely static models as
static. Each animated model was sampled at 50% of the selected named clip.

After a bounded retry for pipeline warm-up, 176/188 models produced valid GPU
JSON plus PNG evidence. The independent evidence auditor matched all 176 pairs
to the current candidate and found no orphan evidence. Twelve exact runtime
blockers remain:

- nine models use exact legacy shader programs that the Bevy runtime does not
  yet implement;
- two models lack an authoritative required texture-slot binding for
  `Skin_FusionEffect_blendSrcalphaInvsrcalpha`;
- one model contains a typed shader pass whose cull state contradicts the
  canonical exact pass.

No shader behavior, texture binding or cull state was guessed. Consequently,
`structuralPassed=true`, but `automatedGpuPassed=false`,
`visualParityPending=true` and `publishable=false`. Nothing from this partial
candidate was installed into `assets/game`.

The machine-readable GPU artifacts are:

- `work/legacy-sources/logical-model-gpu-batch-plan-retrobution.json`;
- `work/legacy-sources/logical-model-gpu-batch-run-retrobution.json`;
- `work/legacy-sources/logical-model-gpu-evidence-retrobution.audit.json`;
- `work/legacy-sources/logical-model-gpu-blockers-retrobution.json`;
- per-model evidence under
  `work/legacy-sources/logical-model-gpu-evidence-retrobution`.

## Props evidence boundary

Tree, bush, grass and rock route tokens are useful only for discovery. Run the
candidate report tool above to create an `inferred_pending_review` queue. A
route name, a sole catalog owner, or a matching mesh hash never promotes a prop
to `verified`; the complete root/placement ownership closure and runtime gates
in `recipes/ffone/policies/props-evidence-policy.json` are required.
