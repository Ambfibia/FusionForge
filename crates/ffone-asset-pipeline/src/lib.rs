//! Native, reproducible project-asset imports for FFOneClient.
//!
//! This crate accepts only a validated [`ffone_content::ContentPack`]. It has no source-client,
//! Unity, Gamebryo, or editor integration. Native mesh JSON is converted to GLB 2.0; already
//! runtime-ready PNG, Ogg Vorbis, OpenType/TrueType, JSON, and WGSL files are copied by value.

#![forbid(unsafe_code)]
#![recursion_limit = "256"]

use catalogs::asset_composition;
pub mod cli;
use audio::audio_taxonomy_migration;
pub use characters::avatar_assets;
use characters::avatar_item_model_refresh;
use characters::character_creation_data;
use characters::character_registry_alias_repair;
use ui::equipment_gpu_batch;
use ui::equipment_logical_model_batch_publish;
use ui::equipment_logical_model_install;
mod error;
mod glb;
mod importer;
use ui::legacy_gui_skin;
use models::legacy_shader_state;
use audio::localized_voice_install;
use characters::logical_character_install;
use models::logical_model_batch_publish;
use models::logical_model_gpu_evidence_audit;
use models::logical_model_publish;
use models::logical_model_tree_audit;
mod manifest;
mod manifest_refresh;
mod model_audit;
mod model_contract;
mod native_model_encode;
use ui::native_ui_icon_routes;
use ui::native_ui_install;
use catalogs::object_route_normalization;
use characters::player_avatar_cooker;
use characters::player_item_model_install;
use characters::player_shared_rig_publish;
mod policy;
use world::published_terrain_dedup;
use catalogs::resource_set_organizer;
use characters::runtime_character_model_replace;
use catalogs::runtime_metadata_cleanup;
use world::runtime_world_migration;
use catalogs::semantic_asset_organizer;
use audio::semantic_audio_install;
use characters::semantic_character_install;
use ui::semantic_icon_install;
use world::static_world_winding_repair;
use audio::strict_voice_install;
use characters::tutorial_character_promotion;
use models::tutorial_effect_install;
use models::tutorial_model_dedupe;
use models::tutorial_model_install;
use ui::tutorial_npc_building_update;
use catalogs::tutorial_prop_promotion;
use world::tutorial_static_world_install;
use world::world_behaviour_install;
use world::world_prefab_organizer;

pub use asset_composition::{
    ASSET_COMPOSITION_REPORT, ASSET_COMPOSITION_SCHEMA, AssetCompositionOptions,
    AssetCompositionReport, compose_asset_roots,
};
pub use audio_taxonomy_migration::{
    AUDIO_TAXONOMY_MIGRATION_SCHEMA, AudioTaxonomyMigrationOptions, AudioTaxonomyMigrationReport,
    migrate_audio_taxonomy,
};
pub use avatar_item_model_refresh::{
    AVATAR_ITEM_MODEL_REFRESH_SCHEMA, AvatarItemModelChange, AvatarItemModelRefreshOptions,
    AvatarItemModelRefreshReport, refresh_avatar_item_models,
};
pub use character_creation_data::{
    avatar_item_counts, build_item_category,
    AvatarIconReference, AvatarItemCategory, AvatarItemLookup, AvatarItemVisual,
    AvatarModelReference, AvatarTextureReference, CHARACTER_CREATION_APPEARANCE_PATH,
    CHARACTER_CREATION_APPEARANCE_SCHEMA, CHARACTER_CREATION_AVATAR_ITEMS_PATH,
    CHARACTER_CREATION_AVATAR_ITEMS_SCHEMA, CHARACTER_CREATION_INSTALL_SCHEMA,
    CHARACTER_CREATION_NAME_WHEEL_PATH, CHARACTER_CREATION_NAME_WHEEL_SCHEMA,
    CHARACTER_CREATION_ROOT, CHARACTER_CREATION_RUNTIME_TEXTURES_PATH,
    CHARACTER_CREATION_RUNTIME_TEXTURES_SCHEMA, CharacterAppearanceCategory,
    CharacterAppearanceConstraints, CharacterColorContract, CharacterCreationAppearance,
    CharacterCreationAssetReference, CharacterCreationAvatarItemCounts,
    CharacterCreationAvatarItems, CharacterCreationChoice, CharacterCreationDataInstallOptions,
    CharacterCreationDataInstallReport, CharacterCreationMaxima, CharacterCreationNameWheel,
    CharacterCreationProvenance, CharacterCreationRow, CharacterCreationRuntimeTextures,
    CharacterGender, CharacterPaletteColor, CharacterRuntimeTextureContract,
    CharacterRuntimeTextureCoverage, CharacterRuntimeTextureSource, CharacterTextureRules,
    CharacterTextureSuffix, NameWheelEntry, NativeLookupStatus, install_character_creation_data,
    register_character_creation_runtime_textures,
};
pub use character_registry_alias_repair::{
    CHARACTER_REGISTRY_ALIAS_PLAN_SCHEMA, CHARACTER_REGISTRY_ALIAS_REPAIR_SCHEMA,
    CharacterRegistryAliasChange, CharacterRegistryAliasPlan, CharacterRegistryAliasPlanEntry,
    CharacterRegistryAliasPlanOptions, CharacterRegistryAliasPlanReport,
    CharacterRegistryAliasRepairOptions, CharacterRegistryAliasRepairReport,
    apply_character_registry_alias_plan, repair_character_registry_aliases,
};
pub use equipment_gpu_batch::{
    EQUIPMENT_GPU_BATCH_SCHEMA, EQUIPMENT_GPU_SCOPE, EquipmentGpuBatchCounts,
    EquipmentGpuBatchMode, EquipmentGpuBatchOptions, EquipmentGpuBatchReport,
    EquipmentGpuBatchTiming, EquipmentGpuBlocker, EquipmentGpuCoverage, EquipmentGpuFactsSummary,
    EquipmentGpuModelRun, EquipmentGpuShard, EquipmentGpuSlotCoverage, GpuBatchFileEvidence,
    run_equipment_gpu_batch,
};
pub use equipment_logical_model_batch_publish::{
    EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE, EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA,
    EquipmentLogicalModelBatchCounts, EquipmentLogicalModelBatchPublishOptions,
    EquipmentLogicalModelBatchReport, EquipmentLogicalModelBatchTiming,
    EquipmentLogicalModelBlocker, EquipmentLogicalModelMapping,
    publish_equipment_logical_model_batch,
};
pub use equipment_logical_model_install::{
    PLAYER_EQUIPMENT_CATALOG_PATH, PLAYER_EQUIPMENT_CATALOG_SCHEMA, PLAYER_EQUIPMENT_ROOT,
    PlayerEquipmentAssemblyBlocker, PlayerEquipmentCatalog, PlayerEquipmentCatalogCounts,
    PlayerEquipmentCatalogModel, PlayerEquipmentCatalogProofs, PlayerEquipmentInstallOptions,
    PlayerEquipmentInstallReport, install_player_equipment,
};
pub use error::{PipelineError, Result};
pub use glb::{MESH_SCHEMA, NativeMesh, NativeSubmesh, mesh_json_to_glb};
pub use importer::{DEFAULT_OUTPUT, ImportOptions, import_content_pack};
pub use legacy_gui_skin::{
    LEGACY_GUI_SKIN_EVIDENCE_LEVEL, LEGACY_GUI_SKIN_SCHEMA, LegacyGuiColor, LegacyGuiInsets,
    LegacyGuiObjectPointer, LegacyGuiSkin, LegacyGuiSkinCandidate,
    LegacyGuiSkinCandidateDiagnostic, LegacyGuiSkinContract, LegacyGuiSkinConversionOptions,
    LegacyGuiSkinConversionReport, LegacyGuiStyle, LegacyGuiStyleState, LegacyGuiVector2,
    convert_legacy_gui_skins,
};
pub use localized_voice_install::{
    DEFAULT_VOICE_LOCALE, LOCALIZED_AUDIO_CATALOG_SCHEMA, LOCALIZED_VOICE_REPORT,
    LOCALIZED_VOICE_REPORT_SCHEMA, LocalizedAudioAsset, LocalizedAudioCatalog,
    LocalizedAudioCatalogCounts, LocalizedAudioMatchingPolicy, LocalizedAudioSourceCatalogProof,
    LocalizedAudioVariant, LocalizedVoiceAmbiguousFile, LocalizedVoiceInstallOptions,
    LocalizedVoiceInstallReport, LocalizedVoiceMatchMode, LocalizedVoiceMatchedFile,
    LocalizedVoiceReportCounts, LocalizedVoiceReportDocument, LocalizedVoiceSourceProof,
    LocalizedVoiceUnmatchedFile, RUSSIAN_VOICE_LOCALE, install_localized_voice,
};
pub use logical_character_install::{
    CHARACTER_SCHEMA_UPGRADE_REPORT_PATH, LogicalCharacterInstallOptions,
    LogicalCharacterInstallReport, SEMANTIC_CHARACTER_CATALOG_PATH,
    SEMANTIC_CHARACTER_CATALOG_SCHEMA, SemanticAnimationSummary, SemanticAuthoredRoot,
    SemanticCharacterCatalog, SemanticCharacterModel, SemanticCharacterProofs,
    SemanticMaterialShaderMetadata, SemanticRigSummary, SemanticShader, SemanticTexture,
    install_logical_characters,
};
pub use logical_model_batch_publish::{
    LOGICAL_MODEL_BATCH_REPORT_FILE, LOGICAL_MODEL_BATCH_REPORT_SCHEMA, LogicalModelBatchBlocker,
    LogicalModelBatchCounts, LogicalModelBatchMapping, LogicalModelBatchPublishOptions,
    LogicalModelBatchPublishReport, publish_logical_model_batch,
};
pub use logical_model_gpu_evidence_audit::{
    GPU_EVIDENCE_AUDIT_SCHEMA, GpuEvidenceAuditCounts, GpuEvidenceModelAudit,
    LogicalModelGpuEvidenceAuditReport, audit_logical_model_gpu_evidence,
};
pub use logical_model_publish::{
    LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA, LogicalModelPublishOptions, LogicalModelPublishReport,
    MaterialPublishReport, publish_logical_model,
};
pub use logical_model_tree_audit::{
    LOGICAL_MODEL_TREE_AUDIT_SCHEMA, LogicalModelFeatureCounts, LogicalModelFileAudit,
    LogicalModelFileCounts, LogicalModelTreeAuditReport, LogicalModelTreeCounts,
    LogicalModelTreeViolation, audit_logical_model_tree,
};
pub use manifest::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, SourcePackIdentity,
};
pub use manifest_refresh::{
    ProjectAssetManifestRefresh, refresh_project_asset_manifest_entry,
    register_project_asset_manifest_entry,
};
pub use model_audit::{
    GEOMETRY_DUMP_STATUS, GameplayRoots, GlbAuditStats, MODEL_AUDIT_SCHEMA, ModelAuditOptions,
    ModelAuditReport, RoutingCoverage, SourceCoverage, audit_models,
};
pub use model_contract::{MODEL_PUBLISH_SCHEMA, ModelFeatureCounts, ModelPublishContract};
pub use native_model_encode::{
    NATIVE_MODEL_ENCODE_SCHEMA, NativeModelEncodeOptions, NativeModelEncodeReport,
    encode_native_model,
};
pub use native_ui_install::{
    GAMEPLAY_UI_CATALOG, GAMEPLAY_UI_CATALOG_SCHEMA, GAMEPLAY_UI_LAYOUT, GAMEPLAY_UI_ROOT,
    NativeGameplayUiInstallOptions, NativeGameplayUiInstallReport, install_native_gameplay_ui,
};
pub use object_route_normalization::{
    OBJECT_ROUTE_NORMALIZATION_SCHEMA, ObjectPackageRoute, ObjectRouteNormalizationCounts,
    ObjectRouteNormalizationOptions, ObjectRouteNormalizationReport, ObjectRoutePathLengths,
    normalize_object_routes,
};
pub use player_avatar_cooker::{
    PLAYER_AVATAR_COOK_REPORT_SCHEMA, PLAYER_SKELETON_SOURCE_SCHEMA, PlayerAvatarCookOptions,
    PlayerAvatarCookReport, cook_player_avatar,
};
pub use player_item_model_install::{
    InstalledPlayerItemModel, PLAYER_ITEM_MODEL_INSTALL_SCHEMA, PlayerItemModelInstallOptions,
    PlayerItemModelInstallReport, install_player_item_models,
};
pub use player_shared_rig_publish::{
    FEMALE_SHARED_SKELETON_GLB_PATH, MALE_SHARED_SKELETON_GLB_PATH,
    PLAYER_SHARED_RIG_CONTRACT_PATH, PLAYER_SHARED_RIG_REPORT_SCHEMA, PLAYER_SHARED_RIG_SCHEMA,
    PlayerGenderRigContract, PlayerRigAnimationPublishOptions, PlayerRigClipContract,
    PlayerRigCreatorChoiceContract, PlayerRigGender, PlayerRigNode, PlayerRigPartContract,
    PlayerRigSkinRemap, PlayerRigSourceIdentity, PlayerRigSupplementalCreatorSource,
    PlayerSharedRigContract, PlayerSharedRigPublishOptions, PlayerSharedRigPublishReport,
    export_player_rig_clip_additions, publish_player_rig_animations, publish_player_shared_rigs,
    rebuild_male_emote_payloads, encode_player_rig_clip_additions,
};
pub use published_terrain_dedup::{
    PUBLISHED_TERRAIN_DEDUP_SCHEMA, PUBLISHED_TERRAIN_SHIFT_RESTORE_SCHEMA,
    PublishedTerrainDedupCounts, PublishedTerrainDedupOptions, PublishedTerrainDedupReport,
    PublishedTerrainDedupVerification, PublishedTerrainDetailRoute,
    PublishedTerrainShiftRestoreCounts, PublishedTerrainShiftRestoreOptions,
    PublishedTerrainShiftRestoreReport, PublishedTerrainShiftRestoreTile, dedupe_published_terrain,
    restore_published_terrain_shifts,
};
pub use resource_set_organizer::{
    PLAYER_ITEM_SCHEMA, PLAYER_ITEM_SET_CATALOG_SCHEMA, PLAYER_ITEM_SET_REPORT_SCHEMA,
    PlayerItemCatalogModel, PlayerItemDefinition, PlayerItemSetCatalog,
    PlayerItemSetOrganizerReport, RESOURCE_SET_ORGANIZER_REPORT_SCHEMA, RESOURCE_SET_SCHEMA,
    ResourceSetArtifact, ResourceSetCatalogEntry, ResourceSetDocument, ResourceSetMember,
    ResourceSetOrganizerReport, organize_player_item_sets, organize_resource_sets,
    verify_player_item_sets,
};
pub use runtime_character_model_replace::{
    RUNTIME_CHARACTER_MODEL_LIFECYCLE_SCHEMA, RuntimeCharacterModelInstallOptions,
    RuntimeCharacterModelInstallReport, RuntimeCharacterModelLifecycleReport,
    RuntimeCharacterModelRemoveOptions, RuntimeCharacterModelRenameOptions,
    RuntimeCharacterModelReplaceOptions, RuntimeCharacterModelReplaceReport,
    install_runtime_character_model, remove_runtime_character_model,
    rename_runtime_character_model, replace_runtime_character_model,
};
pub use runtime_metadata_cleanup::{
    ArchivedRuntimeMetadata, CLEAN_RUNTIME_METADATA_INDEX_FILE,
    CLEAN_RUNTIME_METADATA_INDEX_SCHEMA, CLEAN_RUNTIME_METADATA_REPORT_FILE,
    CLEAN_RUNTIME_METADATA_REPORT_SCHEMA, CleanRuntimeMetadataCounts, CleanRuntimeMetadataMode,
    CleanRuntimeMetadataOptions, CleanRuntimeMetadataReport, DeferredWorldMetadata,
    clean_runtime_metadata,
};
pub use runtime_world_migration::{
    ArchivedWorldTechnicalMetadata, LEGACY_WORLD_CATALOG_PATH, LegacyWorldCatalogHashDrift,
    RUNTIME_WORLD_ARCHIVE_INDEX_SCHEMA, RUNTIME_WORLD_MIGRATION_REPORT_SCHEMA,
    RUNTIME_WORLD_REGISTRY_PATH, RUNTIME_WORLD_REGISTRY_SCHEMA, RewrittenRuntimeWorldMetadata,
    RuntimeWorldContentReference, RuntimeWorldMigrationCounts, RuntimeWorldMigrationMode,
    RuntimeWorldMigrationOptions, RuntimeWorldMigrationReport, RuntimeWorldRegistry,
    RuntimeWorldRegistryArtifact, RuntimeWorldRegistryEntry, migrate_runtime_world,
};
pub use semantic_asset_organizer::{
    BlockerCode, DependencyKind, DependencyProposal, DependencyResolution, EntityProposal,
    ModelFeatureClosure, ModelFeatureClosureStatus, ModelMaterialPlan, ModelMaterialStatus,
    ModelProposal, NativeAssetReference, NativeOwnership, OrganizerCounts, RootTrsAction,
    SEMANTIC_ASSET_ORGANIZATION_SCHEMA, SemanticAssetOrganizationReport,
    SemanticAssetOrganizerOptions, SemanticCategory, SpawnRootPolicy, SpawnRootPolicyKind,
    plan_semantic_assets,
};
pub use semantic_audio_install::{
    ClassificationCertainty, RETROBUTION_AUDIO_ASSET_COUNT, SEMANTIC_AUDIO_CATALOG,
    SEMANTIC_AUDIO_CATALOG_SCHEMA, SemanticAudioAsset, SemanticAudioCatalog,
    SemanticAudioCatalogCounts, SemanticAudioCategory, SemanticAudioClassificationProof,
    SemanticAudioCookReportProof, SemanticAudioInstallOptions, SemanticAudioInstallReport,
    SemanticAudioManifestProof, SemanticAudioVariant, install_semantic_audio,
};
pub use semantic_character_install::{
    CharacterInstallInput, CharacterRegistryProofs, CharacterSkipReason, PublishedCharacter,
    PublishedCharacterClassification, PublishedCharacterMaterialSummary, PublishedCharacterTexture,
    RuntimeCharacterCategory, RuntimeCharacterModel, SEMANTIC_CHARACTER_INSTALL_REPORT_SCHEMA,
    SEMANTIC_CHARACTER_REGISTRY_PATH, SEMANTIC_CHARACTER_REGISTRY_SCHEMA,
    SemanticCharacterInstallCounts, SemanticCharacterInstallOptions,
    SemanticCharacterInstallReport, SemanticCharacterRegistry, SkippedCharacter,
    install_semantic_characters,
};
pub use semantic_icon_install::{
    SEMANTIC_ICON_CATALOG_SCHEMA, SemanticIconAsset, SemanticIconCatalog,
    SemanticIconCatalogCounts, SemanticIconCatalogProofs, SemanticIconCategory,
    SemanticIconClassificationProof, SemanticIconInstallOptions, SemanticIconInstallReport,
    SemanticIconSourceAsset, SemanticIconTableReference, SemanticIconUnmatched,
    SemanticIconUnmatchedReason, install_semantic_icons,
};
pub use static_world_winding_repair::{
    STATIC_WORLD_WINDING_ARCHIVE_SCHEMA, STATIC_WORLD_WINDING_REPAIR_PROOF_SCHEMA,
    STATIC_WORLD_WINDING_REPAIR_SCHEMA, STATIC_WORLD_WINDING_REPAIR_TOOL,
    StaticWorldWindingArchiveReport, StaticWorldWindingRepairAction,
    StaticWorldWindingRepairCounts, StaticWorldWindingRepairFile, StaticWorldWindingRepairMode,
    StaticWorldWindingRepairOptions, StaticWorldWindingRepairReport,
    StaticWorldWindingRepairScopeReport, StaticWorldWindingVerification,
    archive_repaired_tutorial_winding_ownership, repair_static_world_winding,
    verify_static_world_winding,
};
pub use strict_voice_install::{
    DEFAULT_STRICT_VOICE_REPORT_DIRECTORY, STRICT_AUDIO_CATALOG_PATH, STRICT_AUDIO_CATALOG_SCHEMA,
    STRICT_AUDIO_CATALOG_SCHEMA_V3, STRICT_AUDIO_CATALOG_SCHEMA_V4, STRICT_VOICE_REPORT_SCHEMA,
    StrictAudioAsset, StrictAudioCatalog, StrictAudioCatalogCounts, StrictAudioFile,
    StrictVoiceInstallOptions, StrictVoiceInstallReport, install_strict_localized_voice,
};
pub use tutorial_character_promotion::{
    TUTORIAL_CHARACTER_PROMOTION_SCHEMA, TutorialCharacterPromotionBlocker,
    TutorialCharacterPromotionCounts, TutorialCharacterPromotionMode,
    TutorialCharacterPromotionModel, TutorialCharacterPromotionOptions,
    TutorialCharacterPromotionProof, TutorialCharacterPromotionReport,
    TutorialCharacterPromotionStatus, promote_tutorial_characters,
};
pub use tutorial_effect_install::{
    RETROBUTION_NPC_GAME_ICON_EFFECT_IDS, RETROBUTION_NPC_WARP_EFFECT_IDS,
    RETROBUTION_PLAYER_STATUS_EFFECT_IDS, RETROBUTION_TUTORIAL_BUILD_ID,
    RETROBUTION_TUTORIAL_BULLET_TYPES, RETROBUTION_TUTORIAL_EFFECT_IDS,
    RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS, RETROBUTION_TUTORIAL_SOURCE_ASSET_PROOFS,
    RETROBUTION_WEAPON_EFFECT_IDS, RETROBUTION_WORLD_EP_EFFECT_IDS, TUTORIAL_BULLET_ROW_SCHEMA,
    TUTORIAL_EFFECT_CATALOG_PATH, TUTORIAL_EFFECT_CATALOG_SCHEMA, TUTORIAL_EFFECT_CLOSURE_SCHEMA,
    TUTORIAL_EFFECT_INSTALL_REPORT_SCHEMA, TUTORIAL_EFFECT_ROOT, TUTORIAL_PROJECTILE_CATALOG_PATH,
    TUTORIAL_PROJECTILE_CATALOG_SCHEMA, TUTORIAL_PROJECTILE_ROOT, TutorialBulletCatalogEntry,
    TutorialBulletParameters, TutorialBulletRowFile, TutorialEffectCatalog,
    TutorialEffectCatalogEntry, TutorialEffectClosureFile, TutorialEffectDependencyInput,
    TutorialEffectInstallOptions, TutorialEffectInstallReport, TutorialProjectileCatalog,
    TutorialSourceAssetProof, TutorialSourceFileProof, TutorialUnityObjectProof,
    install_tutorial_effects,
};
pub use tutorial_model_dedupe::{
    TUTORIAL_CHARACTER_MODEL_DEDUPE_SCHEMA, TutorialCharacterModelDedupeCounts,
    TutorialCharacterModelDedupeMode, TutorialCharacterModelDedupeOptions,
    TutorialCharacterModelDedupePackage, TutorialCharacterModelDedupeReport,
    TutorialCharacterModelDedupeStatus, dedupe_tutorial_character_models,
};
pub use tutorial_model_install::{
    TUTORIAL_MODEL_CATALOG_PATH, TUTORIAL_MODEL_CATALOG_SCHEMA,
    TUTORIAL_MODEL_INSTALL_REPORT_SCHEMA, TUTORIAL_MODEL_ROOT, TutorialModelBatchProof,
    TutorialModelCatalog, TutorialModelCatalogEntry, TutorialModelClosureFile,
    TutorialModelGpuProof, TutorialModelInstallOptions, TutorialModelInstallReport,
    TutorialModelRuntimeFacts, install_tutorial_models,
};
pub use tutorial_npc_building_update::{
    TUTORIAL_NPC_BUILDING_UPDATE_SCHEMA, TutorialNpcBuildingUpdateMode,
    TutorialNpcBuildingUpdateOptions, TutorialNpcBuildingUpdateProof,
    TutorialNpcBuildingUpdateReport, update_tutorial_npc_building,
};
pub use tutorial_prop_promotion::{
    TUTORIAL_PROP_PROMOTION_SCHEMA, TutorialPropPromotionBlocker, TutorialPropPromotionCounts,
    TutorialPropPromotionMode, TutorialPropPromotionModel, TutorialPropPromotionOptions,
    TutorialPropPromotionReport, TutorialPropPromotionStatus, promote_tutorial_props,
};
pub use tutorial_static_world_install::{
    StaticWorldWindingRepairProof, TUTORIAL_STATIC_WORLD_INSTALL_REPORT_SCHEMA,
    TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH, TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA,
    TUTORIAL_STATIC_WORLD_SOURCE_BUILD, TutorialStaticWorldInstallOptions,
    TutorialStaticWorldInstallReport, TutorialStaticWorldOwnedFile, TutorialStaticWorldOwnership,
    TutorialStaticWorldSceneProof, TutorialStaticWorldTileProof,
    WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA, WORLD_MAP_STATIC_WORLD_INSTALL_REPORT_SCHEMA,
    WORLD_MAP_STATIC_WORLD_OWNERSHIP_PATH, WORLD_MAP_STATIC_WORLD_OWNERSHIP_SCHEMA,
    WorldMapStaticWorldInstallOptions, install_tutorial_static_world,
    install_world_map_static_world,
};
pub use world_behaviour_install::{
    WORLD_BEHAVIOUR_DOCUMENT_SCHEMA, WORLD_BEHAVIOUR_INSTALL_REPORT_SCHEMA,
    WORLD_BEHAVIOUR_OWNERSHIP_PATH, WORLD_BEHAVIOUR_OWNERSHIP_SCHEMA, WORLD_BEHAVIOUR_ROOT,
    WorldBehaviourInstallOptions, WorldBehaviourInstallReport, WorldBehaviourOwnership,
    WorldBehaviourTileProof, install_world_behaviours,
};
pub use world_prefab_organizer::{
    WORLD_PREFAB_CATALOG_PATH, WORLD_PREFAB_CATALOG_SCHEMA, WORLD_PREFAB_ORGANIZER_REPORT_SCHEMA,
    WORLD_PREFAB_PLACEMENTS_SCHEMA, WORLD_PREFAB_SCHEMA, WORLD_PREFAB_TOOL,
    WORLD_PREFAB_VERIFICATION_SCHEMA, WorldPrefabArtifact, WorldPrefabBounds, WorldPrefabCatalog,
    WorldPrefabCatalogPrefab, WorldPrefabDefinition, WorldPrefabOrganizerCounts,
    WorldPrefabOrganizerMode, WorldPrefabOrganizerOptions, WorldPrefabOrganizerReport,
    WorldPrefabPart, WorldPrefabPlacement, WorldPrefabPlacementDocument,
    WorldPrefabPlacementSetReference, WorldPrefabResource, WorldPrefabResourceKind,
    WorldPrefabSourceIdentity, WorldPrefabVerification, organize_world_prefabs,
    verify_world_prefab_library, verify_world_prefab_library_to_report,
};

pub use logical_model_publish::index_native_textures;

pub mod model_animation_append;

pub mod commands;

pub use catalogs::direct_output;
pub use logical_model_publish::{convert_logical_model_bytes, prepare_direct_model};


pub(crate) mod shared;

pub mod catalogs;

mod audio;

pub mod characters;

mod ui;

mod models;

mod world;
