
pub(super) const USAGE: &str = "Native FFOneClient project-asset importer

Usage:
  ffone-asset-pipeline import --pack <CONTENT_PACK_DIR> [--output <ASSET_DIR>]
  ffone-asset-pipeline compose-assets <BASE_ASSET_ROOT> <OVERLAY_ASSET_ROOT> <FRESH_OUTPUT_ROOT> \
    --overlay-prefix <RELATIVE_DIR/> [--overlay-prefix <RELATIVE_DIR/> ...]
  ffone-asset-pipeline publish-logical-model <SOURCE_JSON> <FAMILY> <OUTPUT_ROOT> \
    [--texture-rebinds <REVIEWED_JSON>] [--reuse-texture-index <JSON>] [--semantic-directory <NAME> ...]
  ffone-asset-pipeline publish-logical-model-batch <SOURCE_ROOT> <FRESH_OUTPUT_ROOT>
  ffone-asset-pipeline publish-equipment-logical-model-batch <SOURCE_ROOT> <FRESH_OUTPUT_ROOT>
  ffone-asset-pipeline accept-equipment-gpu-batch <CANDIDATE_ROOT> <EVIDENCE_ROOT> \
    <PREVIEW_EXE> <smoke|full> <NEW_REPORT_JSON> [--frames <COUNT>] [--timeout <SECONDS>] \
    [--shard <INDEX/COUNT>]
  ffone-asset-pipeline install-logical-characters <CANDIDATE_ROOT> <GPU_AUDIT_JSON> \
    <ASSET_ROOT> <SOURCE_BUILD>
  ffone-asset-pipeline install-tutorial-models <CANDIDATE_ROOT> <ASSET_ROOT> <SOURCE_BUILD> \
    --evidence-root <DIR> [--evidence-root <DIR> ...] \
    --model <models/.../*.glb> [--model <models/.../*.glb> ...]
  ffone-asset-pipeline dedupe-tutorial-character-models <ASSET_ROOT> [--apply]
  ffone-asset-pipeline promote-tutorial-characters <ASSET_ROOT> <CANDIDATE_ROOT> \
    <SOURCE_ROOT> <GPU_EVIDENCE_ROOT> [--apply]
  ffone-asset-pipeline promote-tutorial-props <PROJECT_ROOT> <CANDIDATE_ROOT> \
    <SOURCE_ROOT> <GPU_EVIDENCE_ROOT> [--apply]
  ffone-asset-pipeline update-tutorial-npc-building <PROJECT_ROOT> <CANDIDATE_ROOT> \
    <SOURCE_ROOT> <GPU_EVIDENCE_ROOT> <RUNTIME_SMOKE_JSON> \
    --expected-installed-sha256 <SHA256> --expected-manifest-sha256 <SHA256> [--apply]
  ffone-asset-pipeline install-semantic-characters <CANDIDATE_ROOT> <GPU_AUDIT_JSON> \
    <TABLE_SET_JSON> <ASSET_ROOT> <SOURCE_BUILD> <REPORT_JSON> [--replace-existing]
  ffone-asset-pipeline replace-runtime-character-model <ASSET_ROOT> <RELATIVE_GLB> \
    <REPLACEMENT_GLB>
  ffone-asset-pipeline install-runtime-character-model <ASSET_ROOT> <ID> <CANDIDATE_GLB> \
    <GPU_EVIDENCE_JSON>
  ffone-asset-pipeline rename-runtime-character-model <ASSET_ROOT> <ID> <NEW_ID> \
    <REPORT_JSON> [--apply]
  ffone-asset-pipeline remove-runtime-character-model <ASSET_ROOT> <ID> <REPORT_JSON> [--apply]
  ffone-asset-pipeline encode-native-model <MODEL_JSON> <OUT_GLB> [REPORT_JSON]
  ffone-asset-pipeline repair-character-registry-aliases <ASSET_ROOT> <REPORT_JSON> [--apply]
  ffone-asset-pipeline apply-character-registry-alias-plan <ASSET_ROOT> <PLAN_JSON> <REPORT_JSON> [--apply]
  ffone-asset-pipeline install-player-item-models <CANDIDATE_ROOT> <GPU_BATCH_JSON> \
    <ASSET_ROOT> <SOURCE_BUILD> <REPORT_JSON> [--apply]
  ffone-asset-pipeline refresh-avatar-item-models <ASSET_ROOT> <REPORT_JSON> [--apply]
  ffone-asset-pipeline install-player-equipment <CANDIDATE_ROOT> <GPU_BATCH_JSON> \
    <ASSET_ROOT> <SOURCE_BUILD>
  ffone-asset-pipeline install-character-creation-data <TABLE_SET_JSON> <ASSET_ROOT> \
    <SOURCE_BUILD>
  ffone-asset-pipeline register-character-runtime-textures <ASSET_ROOT>
  ffone-asset-pipeline refresh-manifest-entry <ASSET_ROOT> <RELATIVE_PATH>
  ffone-asset-pipeline register-manifest-entry <ASSET_ROOT> <RELATIVE_PATH> <KIND>
  ffone-asset-pipeline clean-runtime-metadata <PROJECT_ROOT> <SOURCE_BUILD> [--apply]
  ffone-asset-pipeline migrate-runtime-world <PROJECT_ROOT> <SOURCE_BUILD> [--apply]
  ffone-asset-pipeline install-gameplay-ui <ASSET_ROOT> <SOURCE_BUILD>
  ffone-asset-pipeline convert-legacy-gui-skins <DUMP_OBJECT_ALL_JSON> <OUTPUT_JSON> <SOURCE_BUILD>
    (editor-only candidate; publication is forbidden)
  ffone-asset-pipeline install-tutorial-effects <ASSET_ROOT> <EFFECTS_SERIALIZED_ASSET> \
    <DUMP_OBJECT_ALL_JSON> <SOURCE_BUILD> \
    --dependency <SERIALIZED_ASSET> <DUMP_OBJECT_ALL_JSON> \
    --dependency <SERIALIZED_ASSET> <DUMP_OBJECT_ALL_JSON>
  ffone-asset-pipeline install-tutorial-static-world <EXPORT_ROOT> <ASSET_ROOT>
  ffone-asset-pipeline install-world-map-static-world <EXPORT_ROOT> <ASSET_ROOT> <CONTRACT_JSON>
  ffone-asset-pipeline repair-static-world-winding <PROJECT_ROOT> <REPORT_JSON> [--apply]
  ffone-asset-pipeline verify-static-world-winding <PROJECT_ROOT> <APPLY_REPORT_JSON>
  ffone-asset-pipeline archive-repaired-tutorial-winding <PROJECT_ROOT>
  ffone-asset-pipeline organize-map <PROJECT_ROOT> <REPORT_JSON> \
    [--tutorial-metadata-root <DIR>] [--apply] [--replace]
  ffone-asset-pipeline organize-resource-sets <PROJECT_ROOT> <REPORT_JSON>
  ffone-asset-pipeline normalize-object-routes <PROJECT_ROOT> <REPORT_JSON> [--apply]
  ffone-asset-pipeline dedupe-published-terrain <PROJECT_ROOT> <REPORT_JSON> [--apply]
  ffone-asset-pipeline restore-published-terrain-shifts <PROJECT_ROOT> <PRIMARY_EXPORT_ROOT> \
    <REPORT_JSON> [--apply]
  ffone-asset-pipeline organize-player-item-sets <PROJECT_ROOT> <REPORT_JSON>
  ffone-asset-pipeline verify-player-item-sets <PROJECT_ROOT>
  ffone-asset-pipeline verify-map <PROJECT_ROOT> <REPORT_JSON>
  ffone-asset-pipeline install-world-behaviours <EXPORT_ROOT> <ASSET_ROOT> <SOURCE_BUILD>
  ffone-asset-pipeline install-semantic-audio <ASSET_ROOT> <COOK_REPORT_JSON> <SOURCE_BUILD>
  ffone-asset-pipeline install-localized-voice <ASSET_ROOT> <RU_SOURCE_ROOT> <SOURCE_BUILD> \
    --cook-report <FRESH_COOK_REPORT_JSON> --cook-pack <FRESH_COOK_PACK_ROOT> \
    --english-root <CLEAN_ENGLISH_AUDIO_ROOT> --generated-root <GENERATED_REPORT_ROOT> \
    [--source-catalog <V1_OR_V2_OR_V3_CATALOG_JSON>]
  ffone-asset-pipeline migrate-audio-taxonomy <ASSET_ROOT> <NANO_RESOURCE_ROOT> [--apply]
  ffone-asset-pipeline install-semantic-icons <ASSET_ROOT> <TABLE_SET_JSON> <SOURCE_BUILD>
  ffone-asset-pipeline audit-logical-model-tree <OUTPUT_ROOT> [REPORT_JSON]
  ffone-asset-pipeline audit-logical-model-gpu-evidence <CANDIDATE_ROOT> <EVIDENCE_ROOT> [REPORT_JSON]
  ffone-asset-pipeline audit-models --assets <ASSET_DIR> --cook-report <JSON> \
    --layout-report <JSON> --table-set <JSON> --output <JSON>
  ffone-asset-pipeline plan-semantic-assets --asset-manifest <JSON> --content-index <JSON> \
    --cook-report <JSON> --table-set <JSON> --logical-model-plan <JSON> --output <JSON>
  ffone-asset-pipeline cook-player-avatar --bundle <CharacterSelection.resourceFile> \
    --objects <dump-object-all.json> --logical-plan <JSON> --output <ASSET_DIR> \
    --part-source <EXACT_ROUTE=SOURCE_JSON> [--part-source ...]

Options:
  --pack <DIR>      Validated ffone.content-pack.v1 directory
  --output <DIR>    New standalone staging tree (default: work/ffone/imported-native-assets)
  audit-models      Read-only audit; the JSON output must be outside ASSET_DIR
  plan-semantic-assets  Plan-only; reads JSON evidence and atomically creates one report
  -h, --help        Show this help
";
