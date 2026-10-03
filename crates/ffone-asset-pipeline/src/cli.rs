use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use crate::{
    AssetCompositionOptions, AudioTaxonomyMigrationOptions, AvatarItemModelRefreshOptions,
    CharacterCreationDataInstallOptions, CharacterRegistryAliasPlanOptions,
    CharacterRegistryAliasRepairOptions, CleanRuntimeMetadataOptions, DEFAULT_OUTPUT,
    EquipmentGpuBatchMode, EquipmentGpuBatchOptions, EquipmentGpuShard,
    EquipmentLogicalModelBatchPublishOptions, ImportOptions, LegacyGuiSkinConversionOptions,
    LogicalCharacterInstallOptions, LogicalModelBatchPublishOptions, LogicalModelPublishOptions,
    ModelAuditOptions, NativeGameplayUiInstallOptions, NativeModelEncodeOptions,
    ObjectRouteNormalizationOptions, PlayerAvatarCookOptions, PlayerEquipmentInstallOptions,
    PlayerItemModelInstallOptions, PublishedTerrainDedupOptions,
    PublishedTerrainShiftRestoreOptions, RuntimeCharacterModelInstallOptions,
    RuntimeCharacterModelLifecycleReport, RuntimeCharacterModelRemoveOptions,
    RuntimeCharacterModelRenameOptions, RuntimeCharacterModelReplaceOptions,
    RuntimeWorldMigrationOptions, SemanticAssetOrganizerOptions, SemanticAudioInstallOptions,
    SemanticCharacterInstallOptions, SemanticIconInstallOptions, StaticWorldWindingRepairOptions,
    StrictVoiceInstallOptions, TutorialCharacterModelDedupeOptions,
    TutorialCharacterPromotionOptions, TutorialEffectInstallOptions, TutorialModelInstallOptions,
    TutorialNpcBuildingUpdateOptions, TutorialPropPromotionOptions,
    TutorialStaticWorldInstallOptions, WorldBehaviourInstallOptions,
    WorldMapStaticWorldInstallOptions, WorldPrefabOrganizerOptions,
    apply_character_registry_alias_plan, archive_repaired_tutorial_winding_ownership,
    audit_logical_model_gpu_evidence, audit_logical_model_tree, audit_models,
    clean_runtime_metadata, compose_asset_roots, convert_legacy_gui_skins, cook_player_avatar,
    dedupe_published_terrain, dedupe_tutorial_character_models, encode_native_model,
    import_content_pack, install_character_creation_data, install_logical_characters,
    install_native_gameplay_ui, install_player_equipment, install_player_item_models,
    install_runtime_character_model, install_semantic_audio, install_semantic_characters,
    install_semantic_icons, install_strict_localized_voice, install_tutorial_effects,
    install_tutorial_models, install_tutorial_static_world, install_world_behaviours,
    install_world_map_static_world, migrate_audio_taxonomy, migrate_runtime_world,
    normalize_object_routes, organize_player_item_sets, organize_resource_sets,
    organize_world_prefabs, plan_semantic_assets, promote_tutorial_characters,
    promote_tutorial_props, publish_equipment_logical_model_batch, publish_logical_model,
    publish_logical_model_batch, refresh_avatar_item_models, refresh_project_asset_manifest_entry,
    register_character_creation_runtime_textures, remove_runtime_character_model,
    rename_runtime_character_model, repair_character_registry_aliases, repair_static_world_winding,
    replace_runtime_character_model, restore_published_terrain_shifts, run_equipment_gpu_batch,
    update_tutorial_npc_building, verify_player_item_sets, verify_static_world_winding,
    verify_world_prefab_library_to_report,
};

#[cfg(test)]
mod tests;

mod constants;
mod operations;
mod state;
mod audio;
mod assets;
mod output;
mod terrain;
mod containers;
mod localization;
mod models;
mod systems;
mod codec;
mod textures;

use constants::USAGE;
pub use operations::run;
use state::{run_runtime_metadata_cleanup, run_runtime_world_migration};
use audio::{run_audio_taxonomy_migration, run_semantic_audio_install};
use assets::{
    run_asset_composition, run_manifest_entry_refresh, run_manifest_entry_registration,
    run_object_route_normalization, run_character_registry_alias_repair,
    run_character_registry_alias_plan, run_semantic_asset_plan
};
use output::{
    run_native_gameplay_ui_install, run_tutorial_effect_install, run_world_behaviour_install,
    run_world_map_static_world_install, run_tutorial_static_world_install,
    run_semantic_icon_install, run_logical_character_install, run_semantic_character_install,
    write_lifecycle_report, run_player_equipment_install, run_character_creation_data_install
};
use terrain::{run_published_terrain_dedup, run_published_terrain_shift_restore};
use containers::{run_world_prefab_organizer, run_world_prefab_verification};
use localization::run_localized_voice_install;
use models::{
    run_tutorial_model_install, run_tutorial_character_model_dedupe,
    run_runtime_character_model_replace, run_runtime_character_model_install,
    run_runtime_character_model_rename, run_runtime_character_model_remove,
    run_avatar_item_model_refresh, run_player_item_model_install, run_logical_model_publish,
    run_logical_model_batch_publish, run_equipment_logical_model_batch_publish,
    run_logical_model_tree_audit, run_logical_model_gpu_evidence_audit, run_model_audit
};
use systems::run_tutorial_npc_building_update;
use codec::run_native_model_encode;
use textures::run_character_runtime_texture_registration;
#[cfg(test)]
use operations::run_legacy_gui_skin_conversion;
