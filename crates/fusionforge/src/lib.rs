// Shared legacy decoders and native export backend for the FusionForge CLI.
#![allow(dead_code)]

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    env, fs,
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

mod bundle_layout;
pub mod fusionforge;
mod legacy_bundle_layout;
mod legacy_semantic_index;
mod logical_model_material;
pub mod npc_animation;
mod npc_editor;
mod npc_legacy_animation;
mod npc_usage;

thread_local! {
    /// Exact batch exports are intentionally grouped by physical bundle. Keep
    /// only the most recent parsed environment so thousands of routes from the
    /// same CharacterSelection archive do not reparse 240k Unity objects, while
    /// a full multi-bundle catalog can never retain every archive at once.
    static LAST_UNITY_ENVIRONMENT: RefCell<Option<(Vec<(String, u64, u128)>, Rc<fusionforge::UnityEnvironment>)>> =
        const { RefCell::new(None) };
    static LAST_PREVIEW_CONTAINER_INDEX: RefCell<Option<(usize, Vec<(String, usize)>, Rc<Vec<PreviewContainerEntry>>)>> =
        const { RefCell::new(None) };
    /// Exact equipment routes in one bundle share most of their serialized
    /// dependency graph (rig, common transforms, materials). Cache only the
    /// pointer adjacency here: route traversal no longer reparses the same
    /// object merely to rediscover identical outgoing PPtrs. The actual object
    /// values remain authoritative in the Unity assets and are not retained.
    static LAST_EXACT_POINTER_GRAPH: RefCell<Option<(usize, BTreeMap<(usize, i64), Rc<Vec<fusionforge::Pointer>>>)>> =
        const { RefCell::new(None) };
    static LAST_EXACT_BATCH_ENVIRONMENT: RefCell<Option<ExactBatchEnvironment>> =
        const { RefCell::new(None) };
    static LAST_CLIENT_FILE_INDEX: RefCell<Option<(String, u64, u128, Rc<ClientFileIndex>)>> =
        const { RefCell::new(None) };
}

#[cfg(test)]
mod build_output_sync_tests;

pub use fusionforge::run_cli_from_env;

#[cfg(test)]
mod world_editor_tests;

mod ffone_chartexture_metadata;

use backend::validation::EditorError;
use backend::types::{
    EditorResult, TileDocument, ClientExtractedFile, ClientFontRecord,
    ScriptAssemblyRecord, ScriptDecompileResult, TableDataInspection, TableDataSectionSummary,
    NpcIconRecord, NpcBarkerRecord, NpcBlueprint, NpcIconPreview, NpcSourceBuildInspection,
    CacheMetadata, BuildSourceFileSnapshot, BuildSourceSnapshot, PreviewSemanticDedup,
    ExactBatchEnvironment, NpcGeneratedIconBinding, NpcTargetExternalRef, NativeBuildTempDir, LegacyLayoutInputFiles
};
pub(crate) use backend::types::{
    Vec3, NpcGeneratedIcon, NpcGeneratedIconPlan,
    NpcGeneratedIconAttachResult
};
use backend::terrain::{TerrainTextureLayer, TerrainDocument};
use backend::models_nif_preview_collector::{
    ModelSource, nif_bytes_to_preview_meshes,
    nif_tex_desc_path, nif_image_has_alpha, nif_image_has_partial_alpha, apply_nif_black_key_alpha, apply_exact_preload_materials_to_nif_previews,
    kfm_preload_nif_previews, selected_contains_mesh
};
use backend::models_preview_bundle_container_model_exact::{
    preview_bundle_container_model, preview_bundle_container_model_exact
};
pub(crate) use backend::models_preview_bundle_container_model_exact::prewarm_exact_logical_model_environment;
use backend::models_preview_bundle_container_model_with_mode::{
    preview_bundle_container_model_with_mode, preview_mesh_semantic_key
};
use backend::models_read_clean_gltf_joints::{
    preview_renderable_mesh_keys, preview_renderable_mesh_keys_from_selection,
    is_authoring_model_path, npc_blueprint_uses_authoring_model_bundle,
    npc_model_paths_from_table, find_npc_template_model_bundle, model_container_path_matches,
    target_model_path_for_template_path, set_gameobject_static_mesh_components,
    renderer_mesh_key, retarget_mesh_filter, retarget_mesh_renderer,
    retarget_skinned_mesh_renderer, mesh_bind_poses_from_value, reset_imported_mesh_transform,
    npc_converted_authoring_model_path, CleanGltfJoint,
    read_clean_gltf_joints, legacy_skinned_mesh_basis_rotation,
    read_clean_gltf_mesh_node_transform, legacy_safe_clean_gltf_joints, clean_gltf_palette_joint_indices
};
use backend::models_sanitize_clean_gltf_joints_for_runtime::{
    clean_gltf_needs_bip01_wrapper,
    clean_gltf_runtime_joint_paths_by_node, clean_gltf_runtime_joint_paths,
    clean_gltf_root_gameobject_name, clean_mesh_filter, clean_mesh_renderer,
    preserve_legacy_mesh_compressed_defaults
};
use backend::models_write_clean_gltf_npc_resource_file::write_clean_gltf_npc_resource_file;
use backend::models_refresh_npc_authoring_model_bundle_for_build::{
    authoring_model_path_from_manifest, prepare_npc_authoring_model_bundle,
    refresh_npc_authoring_model_bundle_for_build, npc_imported_model_target_path,
    npc_model_sibling_path, npc_expected_model_name_variants
};
use backend::containers_analyze_world_bundle_plan_value::{
    SceneObject, ClientBundleFile, TableDataObjectSummary, project_source_table_data_bundle,
    extract_bundle_cached, inspect_cached_bundle_assets, inspect_cached_bundle_fonts, source_bundle_signatures, cached_bundle_payloads_are_current,
    infer_cache_container, add_container_to_entries,
    walk_unity_strings, cache_container_aliases, normalize_entry_container, active_bundle_cache_dirs, is_unity_or_managed_file, analyze_world_bundle_plan_value,
    unity_value_owned_bytes,
    suppress_exact_kfm_parse_warning_for_serialized_hierarchy
};
use backend::containers_find_npc_template_icon_bundle::{
    PreviewContainerEntry, cached_exact_object_pointers,
    cached_preview_container_entries, cached_unity_environment_from_extract_dirs,
    dependency_bundle_extract_dir, preview_container_dependency_refs,
    unity_value_to_json, json_to_unity_value, is_unity_metadata_field,
    container_name_from_bundle, unity_string_field, set_optional_unity_string,
    unity_usize_field, patch_unity_object_fields, table_data_patch_body_for_object,
    set_unity_usize_field, set_unity_i64_field,
    set_existing_unity_i64_field, set_existing_unity_string_field,
    patch_existing_unity_object_fields, npc_container_alias_paths,
    staged_icon_container_should_retarget, first_npc_character_source_bundle,
    npc_blueprint_bundle_name, copy_serialized_assets_into_dir, find_npc_template_icon_bundle,
    rename_assetbundle_container_paths
};
use backend::containers_sync_build_manifests_for_bundle::{
    remove_assetbundle_container_prefix, unity_value_points_to_key, unity_color,
    clean_object_info, unity_value_shape, unity_vec2, unity_vec3, unity_quat, unity_pair,
    unity_named, set_object_field, clean_game_object_with_active,
    first_object_value_for_class, maybe_copy_object_field, remap_bundle_file_external_refs,
    sync_build_manifests_for_bundle, unity_local_pointer, unity_external_pointer,
    unity_null_pointer, add_unique_npc_source_bundle,
    is_npc_shared_dependency_bundle,
    npc_standalone_required_container_paths, npc_container_should_follow_preload,
    set_unity_object_i64, set_unity_object_string, npc_target_external_bundle_priority,
    npc_target_external_bundle_name, npc_bundle_should_externalize_dependencies,
    npc_bundle_should_use_safe_imported_fallback, assetbundle_value_has_container_prefix,
    unity_object_type_tree_key, serialize_npc_output_object_value,
    normalize_imported_npc_output_object, npc_container_expected_object_type,
    npc_container_expected_object_types, npc_container_pointer_quality,
    npc_resolved_container_paths_from_sources, npc_container_preload_root_pointer
};
use backend::containers_pack_npc_source_assets_fallback_bundle::{
    npc_container_named_object_pointer, npc_source_container_paths_by_key,
    npc_external_ref_for_source_object,
    direct_npc_character_bundle_source, read_table_data_object_body,
    npc_standalone_bundle_inputs,
    npc_standalone_bundle_is_fresh, extract_bundle_native_to_dir,
    npc_container_paths_from_source_bundles, pack_bundle_native_from_dir,
    pack_npc_source_assets_fallback_bundle
};
use backend::collision::{ColliderObject, rewrite_collision_flags_masks};
use backend::projects_compile_script_project::{
    ScriptPatchEntry,
    TableDataSectionPatchResult, TableDataCellPatch, ClientPatchConfig,
    workspace_root_dir, patch_config_from_json,
    project_source_dir_from_patch_config,
    default_table_data_project_dir, table_data_patch_value,
    npc_source_import_project_dir, npc_source_import_project_candidates, patch_npc_linked_row_fields, patch_table_section_row_fields
};
use backend::projects_stage_npc_friendly_patch::{
    load_staged_npc_icon_patch,
    retarget_staged_npc_import_table_patch,
    table_data_patch_file_parent_and_name, write_table_data_patch_document, ensure_patch_project_config, seed_active_table_data_patch,
    apply_npc_build_table_patch_step, npc_import_dir_from_patch_config
};
use backend::assets_index_client_project::{
    ClientFileIndex, ClientAssetSummary, ScriptPatchManifest, TableDataAssetSummary,
    NpcCatalogInspection, NpcCatalogRecord, NpcCatalogLinks, NpcImportManifestContext,
    NpcAssetHints, BUILD_SOURCE_INDEX_FILE,
    PROJECT_SCRIPT_MANIFEST_RELATIVE, path_hash, normalized_server_tdata_path, project_patch_config_path, resolve_project_path,
    project_script_manifest_path, path_relative_to_project, load_script_patch_manifest,
    load_script_patch_manifest_from_path, write_script_patch_manifest, is_asset_filename_source,
    should_export_unity_field_path,
    is_internal_npc_alias_field_path, is_shiny_internal_name_field_path,
    is_filter_dictionary_field_path, is_internal_help_page_field_path,
    is_internal_first_use_field_path, is_internal_rules_field_path,
    path_to_string, collect_npc_import_manifest_paths, load_project_client_index,
    client_index_matches_project_source,
    world_asset_kind, index_client_project
};
pub(crate) use backend::assets_index_client_project::{
    CONTAINER_ASSET_POINTER_LIMIT, ContainerAssetBytesResolution, ContainerAssetBytesSearch,
    container_asset_bytes_with_source_budget
};
pub(crate) use backend::assets_inspect_npc_catalog_impl::kfm_text_asset_bytes_with_source_exact;
use backend::assets_inspect_npc_catalog_impl::{
    container_asset_bytes, ascii_asset_strings, path_stem_token, preview_path_matches_tokens,
    matched_wanted_container_path, staged_preview_sibling_asset_dirs, exact_batch_path_key,
    cached_client_file_index, pointer_referenced_asset_is_loaded, normalized_asset_ref_name,
    client_bundle_contains_asset_ref, table_value_at_path, table_value_at_path_mut,
    extracted_asset_path, inspect_npc_catalog_impl, table_data_patch_path_for_object,
    manifest_entry_file, table_data_manifest_patch_paths, blueprint_asset_stem,
    copy_npc_row_to_index, write_npc_row_at_index, blueprint_asset_value,
    npc_icon_runtime_asset_path, parse_npc_icon_asset_path, preferred_npc_icon_asset_path,
    retargeted_npc_icon_asset_path, staged_manifest_icon_paths,
    staged_manifest_icon_bundle_path
};
use backend::assets_ensure_staged_npc_icon_manifest::{
    staged_npc_table_icon_asset_path, staged_icon_asset_files,
    sync_staged_npc_icon_asset_file, sync_staged_npc_manifest_icon_paths,
    asset_stem_from_container_path, write_npc_import_manifest,
    safe_project_relative_path, staged_npc_import_manifest_for_id,
    set_staged_generated_icon_manifest_fields,
    is_unity_bundle_path
};
use backend::assets_stage_npc_manifest_source_assets_to_root::{
    set_npc_manifest_build_source_assets,
    write_npc_build_assets_manifest, npc_manifest_uses_build_source_assets,
    update_npc_manifest_build_source_assets, is_npc_build_asset_source_path,
    recovered_or_original_npc_source_path, stage_npc_manifest_source_assets_to_root,
    stage_npc_manifest_source_assets, component_path_id_from_gameobject,
    transform_path_id_from_gameobject, transform_path_id_from_gameobject_key,
    clean_asset_bundle,
    build_manifest_paths, asset_max_path_id,
    load_npc_source_index, client_index_container_paths, npc_source_index_container_paths,
    npc_clean_asset_name, npc_asset_basename, npc_icon_asset_paths, collect_npc_asset_hints_from_table, npc_asset_hints_from_patch
};
use backend::assets_patch_caching_manifest_npc_bundles::{
    npc_asset_hints_from_source, extend_npc_asset_hints, normalized_asset_path,
    npc_imported_asset_unique_stem,
    npc_hint_container_path_matches, npc_standalone_container_path_matches,
    standalone_npc_asset_name, NpcTargetExternalIndex, NpcTargetTypeTreeIndex,
    build_npc_target_external_index, build_npc_target_type_tree_index,
    npc_standalone_output_path_overrides, npc_referenced_external_asset_names,
    npc_compact_output_path_ids, npc_asset_with_target_type_trees,
    npc_output_asset_with_target_type_trees
};
use backend::assets_npc_collect_explicit_external_asset_refs::{
    first_matching_resolved_path, npc_key_has_required_container_path,
    npc_external_asset_refs, explicit_asset_ref_key, npc_collect_explicit_external_asset_refs,
    ensure_external_asset_ref
};
use backend::assets_pack_standalone_npc_bundle_to_path::pack_standalone_npc_bundle_to_path;
use backend::assets_hydrate_npc_import_manifest_source_table_bun::{
    patch_table_data_asset_file,
    path_from_normalized_relative, npc_import_manifest_context,
    migrate_npc_import_manifest_source_table_bundle,
    hydrate_npc_import_manifest_source_table_bundle, external_resource_import_metadata_path,
    npc_source_assets_from_path, manifest_entries, unique_manifest_values, build_status_path
};
use backend::audio::{
    ClientAudioRecord, inspect_cached_bundle_audio,
    add_npc_audio_asset_hints_from_index, add_npc_audio_asset_hints_from_source_bundles
};
use backend::output_copy_npc_table_rows::{
    NpcImportStageResult,
    npc_source_import_name, staged_npc_import_ids, allocate_import_npc_id,
    npc_copy_source_row,
    copy_npc_table_rows,
    materialize_npc_import_source_assets, copy_file_if_different,
    npc_generated_icon_import, copy_source_relative_file,
    apply_npc_import_to_table_body, ExternalResourceImportSpec, external_import_value
};
use backend::output_materialize_external_resource_import::external_resource_import_specs;
use backend::localization_rust_unity_translation_entries_impl::{
    PROJECT_TRANSLATION_INDEX_RELATIVE, project_translation_index_path, translation_entries_from_file,
    is_unsupported_unity_translation_field_path,
    translation_dedup_location_key, unity_translation_entry_key, translation_text, set_translation_text,
    normalize_translation_document,
    project_from_translation_index_path, gltf_clean_translation
};
use backend::localization_compact_translation_document::normalized_translation_compare_text;
use backend::operations_add_decompiler_aliases::{
    default_repo_root, stable_hash, stable_bytes_hash, metadata_modified_ms, json_string,
    json_string_array, json_bool, json_f64_string, default_script_reference_roots,
    stage_script_reference_dlls, sanitize_decompiled_sources, rewrite_csharp_accessor_calls,
    rewrite_known_out_argument_calls, repair_empty_gui_color_temporaries,
    rewrite_unknown_isinst_placeholders, remove_standalone_csharp_expression_statement,
    remove_known_csharp_noop_statements, dedupe_csharp_using_aliases,
    rewrite_value_type_constructor_calls, strip_redundant_value_type_casts
};
use backend::operations_list_world_assets::{
    split_top_level_comma, replace_csharp_call_with_argument, contains_public_class,
    contains_token, normalize_decompiled_csproj, scan_client_files, cached_extract_dir,
    source_cache_metadata, make_cache_tree_writable, normalize_font_family,
    is_managed_code_expression_source, is_localizable_source,
    remove_translated_source_duplicate_entries, has_managed_assemblies,
    unique_source_translations, extracted_files_in_dir, scan_recursive,
    migrate_world_document_json, json_array_len,
    stripped_world_authoring_document, imported_vec3_curves_to_preview,
    imported_quat_curves_to_preview, kfm_reference_paths, kfm_preview_from_bytes,
    kfm_preview_from_bytes_with_references
};
pub(crate) use backend::operations_list_world_assets::kfm_reference_paths_exact;
use backend::operations_preview_dependency_extract_dirs::{
    preview_name_tokens, add_fuzzy_named_meshes, native_coordinate_contract_json,
    preview_value_without_fields, preview_skin_semantic_value,
    deduplicate_preview_semantic_copies, cache_exact_batch_environment, explicit_dependency_extract_dirs,
    preview_dependency_extract_dirs, add_pointer_dependency_root,
    preview_renderer_matches_tokens, preview_named_value_matches_tokens,
    preview_add_external_pointer_ref, run_table_data_task,
    table_section_fields, count_table_scalars, npc_row_has_text, npc_table_from_body,
    npc_table_from_body_mut, npc_table_array, npc_related_row_has_data,
    npc_related_rows, blueprint_profile_protected_field,
    empty_npc_row_like, npc_table_occupied_ids, requested_or_allocated_npc_id,
    requested_or_existing_staged_npc_id, blueprint_icon_number, npc_target_rows_mut,
    blank_npc_row_from_section
};
use backend::operations_plan_staged_npc_generated_icon_from_loaded::{
    npc_icon_type_for_prefix,
    npc_icon_prefix_for_type, json_nested_string, set_json_nested_string,
    blueprint_has_greeting, ensure_npc_barker_row, bind_unique_generated_npc_icon_row, plan_staged_npc_generated_icon_from_loaded
};
use backend::operations_prune_npc_staged_source_extract_dir::{
    npc_blueprint_source_paths, npc_stage_root,
    npc_build_work_root, npc_build_assets_root, npc_snapshot_assets_root,
    cleanup_npc_build_work_dir, npc_staged_source_label, stage_npc_source_extract_dir,
    cleanup_generated_authoring_bundles, npc_icon_paths_from_table, disable_renderer,
    enable_renderer, set_gameobject_active, local_component_pair, set_local_pointer_field,
    renderer_gameobject_key, transform_gameobject_key, template_transform_z_offset,
    compensate_static_template_transform, clean_mul_vec3, clean_add_vec3, clean_quat_mul,
    clean_rotate_vec3, clean_trs_matrix4, clean_multiply_matrix4, clean_invert_affine_matrix4, clean_decompose_affine_matrix4
};
use backend::operations_npc_augmented_source_bundles::{
    collapse_secondary_iktarget_roots_under_primary,
    legacy_bip01_wrapper_rotation, ensure_legacy_bip01_root_curves,
    rgb_to_565, rgb_from_565, color_distance_sq, clean_transform, clean_skinned_renderer, materialize_npc_authoring_source_assets,
    imported_npc_character_name, standalone_npc_schema_candidates,
    infer_authoring_npc_icon_from_template, npc_augmented_source_bundles,
    npc_token_variants, add_npc_token_hint, collapsed_ascii_token,
    retarget_imported_npc_blueprint_paths_for_build, metadata_preload_range
};
use backend::operations_merge_assetbundle_replacements_to_primary::{
    pair_value_mut, npc_target_external_ref_from_key, rename_pair_key, replace_pair_value,
    filtered_assetbundle_value, filter_assetbundle_preloads_to_output_paths,
    normalize_clean_npc_assetbundle_preloads, merge_assetbundle_replacements_to_primary,
    pointer_candidate_keys_for_rewrite, rewrite_npc_output_pointers,
    npc_referenced_external_output_pointers, pointer_candidate_keys,
    add_resolved_pointer_root, rebind_imported_npc_blueprint_assets_from_sources,
    npc_externalized_pointer_map
};
use backend::operations_legacy_layout_cache_input_digest::{
    prune_unreferenced_table_data_patches, safe_segment, native_build_temp_dir, normalized_relative_file,
    build_source_snapshot, remove_empty_dirs,
    newest_modified_ms, configured_npc_icon_fallbacks, sha256_file_content,
    is_empty_translated_source
};
pub(crate) use backend::operations_legacy_layout_cache_input_digest::repository_root;
use backend::input::{
    collect_reference_assemblies, find_matching_paren, find_decompiled_csproj,
    read_cache_metadata, collect_table_sections,
    collect_npc_staged_source_aliases, collect_value_pointers, collect_file_keys, collect_source_snapshot_files,
    read_build_source_snapshot, collect_existing_files_recursive
};
use backend::systems::{
    apply_known_decompiler_patches,
    apply_blueprint_npc_profile, apply_blueprint_npc_text,
    apply_blueprint_npc_assets, sync_build_output_from_source_preserving,
    apply_configured_npc_icon_fallbacks, update_layout_input_field
};
use backend::codec::{
    has_cached_payload, object_payload_bytes, exact_kfm_text_asset_payload,
    decode_png_data_url, encode_dxt3_rgba_mip_chain, npc_payload_fingerprint
};
use backend::state::{indexed_runtime_file, status_patched_count};
use backend::textures_preview_bundle_container_texture::{
    texture_preview_from_key,
    nif_texturing_property_texture_paths,
    normalized_nif_texture_name, texture_preview_for_nif_texture,
    fill_icon_preview_from_texture_value, preview_bundle_container_texture,
    rename_assetbundle_texture_container_path, material_main_texture_matches,
    rewrite_texture_to_unity25_dxt3, add_npc_texture_asset_hints_from_index,
    npc_imported_texture_target_path, set_material_texenv_texture_pointer,
    npc_local_texture_output_pointers
};
use backend::textures_stage_texture_replacement::npc_material_main_texture_pointer;
use backend::materials::{
    imported_material_to_preview,
    preload_material_previews, material_exact_name_key, preview_material_semantic_value,
    preview_material_reference_semantic, preview_material_references_semantic, material_name_suggests_glass,
    retarget_imported_material, renderer_uses_material_key,
    clean_material, find_npc_inline_toon_shader_assets,
    npc_shader_name_from_body, NPC_SKINNED_TOON_SHADER_NAME, NPC_WORLD_TOON_SHADER_NAME,
    default_npc_shared_shader_ref, material_shader_needs_default, material_texenv_key_name, normalize_imported_material_output
};
use backend::animation::{
    imported_skeleton_to_preview, imported_animation_to_preview,
    preview_animation_semantic_key, skinned_renderer_bone_names, clean_gltf_runtime_bone_name,
    clean_gltf_legacy_safe_bone_name, clean_gltf_side_bone_group_rank,
    remap_imported_clip_paths_for_runtime,
    missing_clean_gltf_animation_paths, normalize_clean_gltf_npc_clip_names,
    expand_single_idle_clip_aliases, clean_animation, find_animation_component_key,
    first_animation_clip_info, append_animation_clip_pointers,
    infer_authoring_npc_animation_set_from_template
};
use backend::constants::LEGACY_LAYOUT_INPUT_FORMAT;
#[cfg(test)]
use backend::models_nif_preview_collector::preview_authoring_model;
#[cfg(test)]
use backend::models_preview_bundle_container_model_exact::{suppress_exact_nif_parse_warning_for_serialized_game_object, resolve_exact_unrelated_mesh_filter_warning};
#[cfg(test)]
use backend::models_sanitize_clean_gltf_joints_for_runtime::sanitize_clean_gltf_joints_for_runtime;
#[cfg(test)]
use backend::models_refresh_npc_authoring_model_bundle_for_build::legacy_manifest_authoring_model_path;
#[cfg(test)]
use backend::assets_index_client_project::resolve_server_tdata_path;
#[cfg(test)]
pub(crate) use backend::assets_ensure_staged_npc_icon_manifest::ensure_staged_npc_icon_manifest;
#[cfg(test)]
use backend::audio::tutorial_voice_durations;
#[cfg(test)]
use backend::output_copy_npc_table_rows::write_build_source_snapshot;
#[cfg(test)]
use backend::output_materialize_external_resource_import::external_resource_import_fingerprint;
#[cfg(test)]
use backend::localization_rust_unity_translation_entries_impl::sanitize_translation_entries;
#[cfg(test)]
use backend::localization_compact_translation_document::{compact_translation_document, load_compact_translation_document};
#[cfg(test)]
use backend::operations_plan_staged_npc_generated_icon_from_loaded::npc_icon_row_type_and_number;
#[cfg(test)]
pub(crate) use backend::operations_plan_staged_npc_generated_icon_from_loaded::plan_staged_npc_generated_icon;
#[cfg(test)]
use backend::operations_npc_augmented_source_bundles::hoist_nonaccum_rest_transform_to_wrapper;
#[cfg(test)]
use backend::operations_legacy_layout_cache_input_digest::restore_pristine_build_output;
#[cfg(test)]
use backend::materials::NPC_SHARED_SHADER_PATH_ID;
#[cfg(test)]
pub(crate) use backend::entities::attach_staged_npc_generated_icon;

mod backend;
