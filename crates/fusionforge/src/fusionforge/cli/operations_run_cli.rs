use super::*;

pub fn run_cli_from_env() -> Option<i32> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|arg| arg == "fusionforge" || arg == "unitypack")
    {
        args.remove(0);
    }

    Some(match run_cli(args) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    })
}

pub(super) fn run_large_stack_task(
    task: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    thread::Builder::new()
        .name("fusionforge-large-stack".to_string())
        .stack_size(256 * 1024 * 1024)
        .spawn(task)
        .map_err(|err| err.to_string())?
        .join()
        .map_err(|_| "large-stack fusionforge task panicked".to_string())?
}

pub(in super::super) fn run_cli(mut args: Vec<String>) -> Result<(), String> {
    if args.is_empty() || normalize_command(&args[0]) == "help" {
        print_usage();
        return Ok(());
    }
    let command = normalize_command(&args.remove(0));
    match command.as_str() {
        "convert-native-ui" => run_large_stack_task(move || super::super::direct_ui::run(&args)),
        "inspect" => run_large_stack_task(move || super::super::inspect::run(&args)),
        "convert-player-emotes" => run_large_stack_task(move || super::super::native_player_emotes::run(&args)),
        "chartexture-metadata" => run_large_stack_task(move || crate::ffone_chartexture_metadata::run(&args)),
        "utility" => { let command = args.first().ok_or("utility requires a command")?; println!("{}", ffone_asset_pipeline::commands::run(command, &args[1..])?); Ok(()) },
        "reference-audit" => { println!("{}", ffone_reference_audit::cli::run(args.iter().map(std::ffi::OsString::from))?); Ok(()) },
        "convert-native" | "publish-native" => run_large_stack_task(move || super::super::native_publication::run(&args)),
        "publish-skill-hit-effects" => {
            let status = std::process::Command::new("python")
                .arg(crate::repository_root().join("tools/legacy-sources/publish-nano-status-effects.py"))
                .args(["--preset", "skill-hits"]).args(&args)
                .status().map_err(|e| e.to_string())?;
            if status.success() { Ok(()) } else { Err(format!("skill-hit publication failed: {status}")) }
        },

        "repair-native" => run_large_stack_task(move || super::super::native_repairs::run(&args)),
        "append-model-animations" => {
            ffone_asset_pipeline::model_animation_append::run(args)
        }
        "native" => {
            let message = ffone_asset_pipeline::cli::run(args.iter().map(std::ffi::OsString::from))?;
            println!("{message}");
            Ok(())
        }
        "index-native-textures" => {
            if args.len() != 2 { return Err("index-native-textures <native-asset-root> <Editor-index.json>".into()); }
            let count = ffone_asset_pipeline::index_native_textures(Path::new(&args[0]), Path::new(&args[1])).map_err(|e| e.to_string())?;
            println!("Indexed {count} immutable native texture chains");
            Ok(())
        }
        "publish-logical-model" => publish_native_logical_model(&args),
        "convert-native-model" => {
            run_large_stack_task(move || convert_native_model(&args))
        }
        "list-contents" => list_contents(&args),
        "list-assetbundle" => list_assetbundle(&args, false),
        "list-ab-alt" => list_assetbundle(&args, true),
        "show-caching-manifest" => show_caching_manifest(&args),
        "validate-bundle-refs" => validate_bundle_refs(&args),
        "validate-object-sizes" => validate_object_sizes(&args),
        "dump-object" | "unity2yaml" | "proto-extract" => dump_object(&args),
        "dump-object-evidence" => dump_object_evidence(&args),
        "dump-gameobject-layers" => dump_gameobject_layers(&args),
        "export-exact-texture" => export_exact_texture(&args),
        "export-exact-texture-batch" => export_exact_texture_batch(&args),
        "dumpxdt" | "dump-xdt" => dump_xdt(&args),
        "dump-terrain" => dump_terrain(&args),
        "export-native-terrain" => {
            let args = args.clone();
            run_large_stack_task(move || export_native_terrain(&args))
        }
        "export-native-terrains" => {
            let args = args.clone();
            run_large_stack_task(move || export_native_terrains(&args))
        }
        "enrich-native-terrains" => {
            let args = args.clone();
            run_large_stack_task(move || enrich_native_terrains(&args))
        }
        "export-native-static-world" => {
            let args = args.clone();
            run_large_stack_task(move || export_native_static_world(&args))
        }
        "export-native-static-behaviours" => {
            let args = args.clone();
            run_large_stack_task(move || export_native_static_behaviours(&args))
        }
        "index-client" | "index-client-project" => {
            let args = args.clone();
            run_large_stack_task(move || index_client_project_command(&args))
        }
        "compare-inventories" => super::super::reverse_engineering::compare_inventories_cli(&args),
        "compare-unity-json" => super::super::reverse_engineering::compare_unity_json_cli(&args),
        "unityextract" | "proto-mesh-extract" => {
            let args = args.clone();
            run_large_stack_task(move || unity_extract(&args))
        }
        "ffextract" => {
            let args = args.clone();
            run_large_stack_task(move || ff_extract(&args))
        }
        "npc-snapshot" => {
            let args = args.clone();
            run_large_stack_task(move || npc_snapshot(&args))
        }
        "preview-container-model" => {
            let args = args.clone();
            run_large_stack_task(move || preview_container_model(&args))
        }
        "export-equipment-model-sources" => {
            let args = args.clone();
            run_large_stack_task(move || export_equipment_model_sources(&args))
        }
        "export-logical-model-source" => {
            let args = args.clone();
            run_large_stack_task(move || export_logical_model_source(&args))
        }
        "export-logical-model-source-batch" => {
            let args = args.clone();
            run_large_stack_task(move || export_logical_model_source_batch(&args))
        }
        "export-logical-model-sources" => {
            let args = args.clone();
            run_large_stack_task(move || export_logical_model_sources(&args))
        }
        "export-logical-prop-sources" => {
            let args = args.clone();
            run_large_stack_task(move || export_logical_prop_sources(&args))
        }
        "audit-logical-model-root-transforms" => {
            let args = args.clone();
            run_large_stack_task(move || audit_logical_model_root_transforms(&args))
        }
        "audit-world-transform-contract" => {
            let args = args.clone();
            run_large_stack_task(move || audit_world_transform_contract(&args))
        }
        "catalog-logical-models" => catalog_logical_models(&args),
        "plan-logical-model-exports" => {
            let args = args.clone();
            run_large_stack_task(move || plan_logical_model_exports(&args))
        }
        "export-native-pack" => {
            let args = args.clone();
            run_large_stack_task(move || content_pack::export_native_pack_cli(&args))
        }
        "inspect-world" => inspect_world(&args),
        "show-gameobject" => show_gameobject(&args),
        "replace-terrain" => replace_terrain(&args),
        "replace-mesh" => replace_mesh(&args),
        "export-managed-assembly-evidence" => super::super::managed_assembly_evidence::export_managed_assembly_evidence_cli(&args),
        "export-ui-image-mode-evidence" => {
            super::super::ui_image_mode_evidence::export_ui_image_mode_evidence_cli(&args)
        }
        "export-ui-interaction-evidence" => {
            super::super::ui_interaction_evidence::export_ui_interaction_evidence_cli(&args)
        }
        "adapt-unity-text" => super::super::unity_text_adapter::adapt_unity_text_cli(&args),
        "export-managed-strings" => build_patch::export_managed_strings(&args),
        "patch-managed-strings" => build_patch::patch_managed_strings(&args),
        "patch-binary-reader-unicode-strings" => {
            build_patch::patch_binary_reader_unicode_strings(&args)
        }
        "patch-asset-loader-downloads" => build_patch::patch_asset_loader_downloads(&args),
        "patch-resource-locator-character-bundles" => {
            build_patch::patch_resource_locator_character_bundles(&args)
        }
        "merge-character-bundle-assets" => merge_character_bundle_assets(&args),
        "patch-caching-manifest-bundles" => patch_caching_manifest_bundles(&args),
        "remove-caching-manifest-bundles" => remove_caching_manifest_bundles(&args),
        "patch-unity-asset-strings" => build_patch::patch_unity_asset_strings(&args),
        "patch-texture-pngs" => build_patch::patch_texture_pngs(&args),
        "patch-audio-clips" => build_patch::patch_audio_clips(&args),
        "patch-gui-fonts" => {
            let args = args.clone();
            run_large_stack_task(move || build_patch::patch_gui_fonts(&args))
        }
        "additem" | "addmission" => Err(format!(
            "{command} was a hard-coded UnityPackFF mutation prototype. \
             It is intentionally not kept as a hidden Python path; use the FF Client Editor patch/export flow instead."
        )),
        _ => {
            print_usage();
            Err(format!("unknown fusionforge command: {command}"))
        }
    }
}

pub(super) fn print_usage() {
    println!("  convert-native-ui <raw-build-root> quit-menu <native-asset-root> [--text-metrics <capture.json>] [--check] [--replace-existing]");
    println!("  inspect <container> [--asset NAME] [--type TYPE] [--name TEXT] [--path-id ID] [--limit N] [--format json|markdown]");
    println!("  inspect <container> --assembly NAME [--class TEXT] [--method TEXT] [--limit N] [--format json|markdown]");
    println!("  chartexture-metadata <asset> <out.json> [options]
  reference-audit <scan|validate-ui-parity> [options]
  utility <operation> [arguments]");
    println!("  publish-skill-hit-effects --source-root <primary-raw-root> --navigation <existing-index> --work <case> [--target-root <native-assets>]");
    println!("  repair-native <banker|nano-voice-events|deduplicate-textures|nano-identities|unstable-powers|nano-tuning|sync-nano-tuning|retire-nano-alias|unstable-power-icon|nano-cel-shading|darwin-head-normals|fred-skin|vehicle-routes|area51-vortex|candy-cove|audio-paths> --target-root <root> --work <fresh-case> [--server-xdt <file>] [--source-root <raw-root>] [--primary-root <raw-root>] [--navigation-root <index-dir>] [--recipe <audio-plan.json>] [--input <GLB> --output <relative-path> --expected-input-sha256 <hash>] [--apply]");
    println!("  convert-native --recipe <recipe.json> --source-root <raw-root> --target-root <native-root> [--check] [--replace-existing]");
    println!("  repair-native hnpc-clips --source-root <primary-raw-root> --target-root <native-assets> --work <fresh-case> [--clips <comma-separated-names>] [--semantic-catalog <relative-json>] [--apply]");
    println!("  repair-native player-damage-animation --source-root <primary-raw-root> --target-root <native-assets> --work <fresh-case> [--apply]");
    println!("  repair-native player-emote-events --source-root <primary-raw-root> --target-root <native-repository> --work <fresh-case> [--apply]");
    println!("  append-model-animations <native.glb> <donor.glb> <fresh-output.glb> <clip>... [--retain-static-root-scale]");
    println!("  native <operation> ...   Built-in Rust asset pipeline (native --help lists operations)");
    println!("  convert-native-model <bundle> <exact-container-route> <family> <native-output-root>");
    println!("  index-native-textures <native-asset-root> <Editor-index.json>\n  publish-logical-model <source.json> <family> <Editor-stage-root> [--reuse-texture-index <index.json>] [--semantic-directory <name>] [--texture-rebinds <reviewed.json>]");
    println!(
        r#"FusionForge - console tools for FusionFall inspection and native export

Usage:
  fusionforge convert-player-emotes <FFR-bundle> <native-root> [--check] [--replace-existing]
  fusionforge list-contents <asset-or-bundle>
  fusionforge list-assetbundle <asset-or-bundle>
  fusionforge list-ab-alt <asset-or-bundle>
  fusionforge show-caching-manifest <asset-or-bundle> [filter]
  fusionforge validate-bundle-refs <asset-or-bundle>...
  fusionforge validate-object-sizes <asset-or-bundle>...
  fusionforge dump-object <asset-or-bundle> [path-id|all] [out.json]
  fusionforge dump-object-evidence <source-alias> <source-root> <source-relative-container> <path-id> [--serialized-asset <exact-name>] [--type <exact-unity-type>] [--out <out.json|->] [--allow-unresolved-pointers]
  fusionforge export-exact-texture <asset-or-bundle> <path-id> <out.json>
  fusionforge export-exact-texture-batch <asset-or-bundle> <selection.json> <fresh-out-dir>
  fusionforge dump-xdt <asset-or-bundle> [path-id] [out.json]
  fusionforge dump-terrain <asset-or-bundle> <out.png>
  fusionforge export-native-terrain <asset-or-bundle> <fresh-out-dir> [--first|--all]
  fusionforge export-native-terrains <client-project|effective-build-root> <fresh-output-root> [--tile <xx_yy>...]
  fusionforge enrich-native-terrains <client-project|effective-build-root> <committed-source-batch-root> <fresh-output-root>
  fusionforge export-native-static-world <Map_XX_YY.unity3d> <effective-build-root> <native-asset-root> <fresh-output-root>
  fusionforge export-native-static-behaviours <Map_XX_YY.unity3d> <effective-build-root> <fresh-out.json>
  fusionforge index-client <raw-build-root> [final-reference.json]
  fusionforge compare-inventories <left.tsv> <right.tsv> [--tag <tag>] [--out <report.json|->]
  fusionforge compare-unity-json <left.json> <right.json> [--max <count>] [--out <report.json|->]
  fusionforge unityextract [--all|--models|--images|--text|--shaders|--fonts] [-o outdir] [--filter text] <files...>
  fusionforge ffextract <input-dir> <out-dir>
  fusionforge npc-snapshot <Character_*.resourceFile> <out-dir>
  fusionforge preview-container-model <bundle> <container-route> <out.json> [work-dir]
  fusionforge export-equipment-model-sources <work-dir|cache/bundle-index.json> <semantic-plan.json> <fresh-source-root> [--limit N]
  fusionforge export-logical-model-source <bundle> <exact-container-route> <out.json> [work-dir]
  fusionforge export-logical-model-source-batch <bundle> <work-dir> <exact-container-route> <out.json> [<exact-container-route> <out.json> ...]
  fusionforge export-logical-model-sources <work-dir|cache/bundle-index.json> <fresh-source-root>
  fusionforge export-logical-prop-sources <work-dir|cache/bundle-index.json> <reviewed-props.json> <fresh-source-root>
  fusionforge audit-logical-model-root-transforms <logical-model-plan.json> <report.json>
  fusionforge audit-world-transform-contract <build-root> <fresh-report.json>
  fusionforge catalog-logical-models <bundle-index.json> [out.json|-]
  fusionforge plan-logical-model-exports <bundle-index.json> [out.json|-] [--kfm-route <exact-route>]
  fusionforge export-native-pack <source-manifest.json|source-build-dir> <fresh-output-dir> [locale]
  fusionforge inspect-world <map-bundle> [build-root] [out.json] [neighbor-radius]
  fusionforge show-gameobject <asset-or-bundle> <container-path-or-name> [out.dot]
  fusionforge replace-terrain <input.png> <offset> <output-asset>
  fusionforge replace-mesh <asset-or-bundle> <path-id> <mesh.obj> <output-asset> [name]
  fusionforge export-managed-assembly-evidence <source-alias> <source-root> <source-relative-container> <exact-entry> [--level <index>] [--out <report.json|->] [--payload-out <portable-relative-path>]
  fusionforge export-ui-image-mode-evidence <request.json> [--out <evidence.json|->]
  fusionforge adapt-unity-text <input.json> [--out <report.json|->] [--allow-unsatisfied]
  fusionforge export-ui-interaction-evidence <portable-request.json> --out <report.json|->
  fusionforge export-managed-strings <extracted-unityweb-dir> <output-json> [--assembly <name>...] [--all-assemblies] [--ui-only] [--merge <json>]
  fusionforge patch-managed-strings <extracted-unityweb-dir> <translation-json> [--allow-missing]
  fusionforge patch-binary-reader-unicode-strings <extracted-unityweb-dir>
  fusionforge patch-asset-loader-downloads <extracted-unityweb-dir> <bundle-name>...
  fusionforge patch-resource-locator-character-bundles <extracted-unityweb-dir> <bundle-name>...
  fusionforge merge-character-bundle-assets <input.resourceFile> [output.resourceFile]
  fusionforge patch-caching-manifest-bundles <sharedassets.assets> <bundle-name>...
  fusionforge remove-caching-manifest-bundles <sharedassets.assets> <bundle-name>...
  fusionforge patch-unity-asset-strings <extracted-dir> <translation-json> [--container name] [--allow-missing] [--status-file path]
  fusionforge patch-texture-pngs <extracted-dir> <texture-dir> --asset name [--container name] [--allow-resize] [--status-file path]
  fusionforge patch-audio-clips <extracted-dir> <audio-dir> --asset name [--container name] [--status-file path]
  fusionforge patch-gui-fonts <input-asset> <output-asset> [--report path] [--ttf-font-dir dir] [--font-manifest path]
"#
    );
}

pub(super) fn cli_metadata_preload_range(metadata: &UnityValue, preload_len: usize) -> (usize, usize) {
    let start = metadata
        .get("preloadIndex")
        .and_then(UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
        .min(preload_len);
    let size = metadata
        .get("preloadSize")
        .and_then(UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    let end = start.saturating_add(size).min(preload_len);
    (start, end)
}

pub(super) fn cli_pair_value_mut(value: &mut UnityValue) -> Option<&mut UnityValue> {
    match value {
        UnityValue::Pair(_, right) => Some(right.as_mut()),
        UnityValue::Array(items) if items.len() >= 2 => items.get_mut(1),
        _ => None,
    }
}

pub(super) fn cli_merge_assetbundle_replacements_to_primary(
    assetbundle_replacements: &BTreeMap<(usize, i64), UnityValue>,
    env: &UnityEnvironment,
    primary_asset_index: usize,
    selected: &BTreeSet<(usize, i64)>,
) -> Option<((usize, i64), UnityValue)> {
    let primary_key = assetbundle_replacements
        .iter()
        .find_map(|(key, value)| {
            (key.0 == primary_asset_index
                && cli_assetbundle_value_has_container_prefix(value, "mob/"))
            .then_some(*key)
        })
        .or_else(|| {
            assetbundle_replacements
                .keys()
                .find(|key| key.0 == primary_asset_index)
                .copied()
        })?;
    let mut ordered = assetbundle_replacements
        .iter()
        .map(|(key, value)| {
            let priority = if *key == primary_key {
                0
            } else if cli_assetbundle_value_has_container_prefix(value, "mob/") {
                1
            } else if cli_assetbundle_value_has_container_prefix(value, "texture/") {
                2
            } else {
                3
            };
            (priority, *key, value)
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(priority, key, _)| (*priority, *key));

    let mut merged = assetbundle_replacements.get(&primary_key)?.clone();
    let mut merged_preloads = Vec::<UnityValue>::new();
    let mut merged_container = Vec::<UnityValue>::new();
    let mut seen_paths = BTreeSet::<String>::new();
    let mut main_asset = None::<(String, UnityValue)>;

    for (_, _, value) in ordered {
        let old_preloads = value_array(value.get("m_PreloadTable")).to_vec();
        for entry in value_array(value.get("m_Container")) {
            let Some((path, metadata)) = pair_name_value(entry) else {
                continue;
            };
            if !seen_paths.insert(cli_normalized_asset_path(path)) {
                continue;
            }
            let mut entry = entry.clone();
            let preload_start = merged_preloads.len();
            let (old_start, old_end) = cli_metadata_preload_range(metadata, old_preloads.len());
            for preload in &old_preloads[old_start..old_end] {
                let mut preload = preload.clone();
                cli_rewrite_selected_external_pointers_to_local(
                    &mut preload,
                    env,
                    primary_asset_index,
                    selected,
                );
                merged_preloads.push(preload);
            }
            let preload_size = merged_preloads.len().saturating_sub(preload_start);
            cli_rewrite_selected_external_pointers_to_local(
                &mut entry,
                env,
                primary_asset_index,
                selected,
            );
            if let Some(metadata) = cli_pair_value_mut(&mut entry) {
                cli_set_unity_object_i64(metadata, "preloadIndex", preload_start as i64);
                cli_set_unity_object_i64(metadata, "preloadSize", preload_size as i64);
                if main_asset.is_none() {
                    if let Some(asset_pointer) = metadata.get("asset").cloned() {
                        main_asset = Some((path.to_string(), asset_pointer));
                    }
                }
            }
            merged_container.push(entry);
        }
    }

    if let Some(object) = merged.as_object_mut() {
        object.insert(
            "m_Container".to_string(),
            UnityValue::Array(merged_container),
        );
        object.insert(
            "m_PreloadTable".to_string(),
            UnityValue::Array(merged_preloads),
        );
        if let Some((path, asset_pointer)) = main_asset {
            if let Some(main_asset) = object
                .get_mut("m_MainAsset")
                .and_then(UnityValue::as_object_mut)
            {
                main_asset.insert("name".to_string(), UnityValue::String(path));
                main_asset.insert("asset".to_string(), asset_pointer);
            }
        }
    }

    Some((primary_key, merged))
}

pub(super) fn list_contents(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        let path = required_path(&args, 0, "list-contents <asset-or-bundle>")?;
        let loaded = load_input(&path)?;
        for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
            if loaded.env.assets.len() > 1 {
                println!("# {}", asset.name);
            }
            for (path_id, info) in &asset.objects {
                let name = asset
                    .read_object(asset_index, info)
                    .ok()
                    .map(|value| object_name(&value))
                    .unwrap_or_default();
                println!(
                    "{}\t{}\t{}\t{}",
                    path_id,
                    info.type_id,
                    asset.object_type_name(info),
                    name
                );
            }
        }
        Ok(())
    })
}

pub(super) fn list_assetbundle(args: &[String], resolve_type: bool) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        let path = required_path(&args, 0, "list-assetbundle <asset-or-bundle>")?;
        let loaded = load_input(&path)?;
        for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
            let Some(assetbundle) = asset.objects.get(&1) else {
                continue;
            };
            if asset.object_type_name(assetbundle) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, assetbundle)?;
            for (path, metadata) in container_entries(&body) {
                let asset_pointer = metadata.get("asset").and_then(UnityValue::as_pointer);
                if resolve_type {
                    let obj_type = asset_pointer
                        .and_then(|pointer| resolved_object_info(&loaded.env, pointer))
                        .map(|(asset, info)| asset.object_type_name(info))
                        .unwrap_or_else(|| "Unknown".to_string());
                    println!(
                        "{}\t{}\t{}\t{}",
                        asset_pointer.map(|value| value.path_id).unwrap_or_default(),
                        asset_pointer.map(|value| value.file_id).unwrap_or_default(),
                        obj_type,
                        path
                    );
                } else {
                    println!(
                        "{}\t{}\t{}\t{}",
                        asset_pointer.map(|value| value.path_id).unwrap_or_default(),
                        metadata
                            .get("preloadIndex")
                            .and_then(UnityValue::as_i64)
                            .unwrap_or_default(),
                        metadata
                            .get("preloadSize")
                            .and_then(UnityValue::as_i64)
                            .unwrap_or_default(),
                        path
                    );
                }
            }
        }
        Ok(())
    })
}

pub(super) fn pointer_label(pointer: &Pointer) -> String {
    format!(
        "sourceAsset={} fileID={} pathID={}",
        pointer.source_asset, pointer.file_id, pointer.path_id
    )
}

pub(super) fn local_pointer_exists(env: &UnityEnvironment, pointer: &Pointer) -> bool {
    env.assets
        .get(pointer.source_asset)
        .is_some_and(|asset| asset.objects.contains_key(&pointer.path_id))
}

pub(super) fn component_class_and_pointer(value: &UnityValue) -> Option<(i64, &Pointer)> {
    match value {
        UnityValue::Array(items) if items.len() >= 2 => {
            Some((items[0].as_i64()?, items[1].as_pointer()?))
        }
        UnityValue::Pair(left, right) => Some((left.as_i64()?, right.as_pointer()?)),
        _ => None,
    }
}

pub(super) fn gameobject_transform_pointer(value: &UnityValue) -> Option<&Pointer> {
    value_array(value.get("m_Component"))
        .iter()
        .find_map(|component| {
            let (class_id, pointer) = component_class_and_pointer(component)?;
            (class_id == 4).then_some(pointer)
        })
}

pub(super) fn resolved_body<'a>(
    env: &'a UnityEnvironment,
    pointer: &Pointer,
) -> Result<(ObjectKey, &'a Asset, &'a ObjectInfo, UnityValue), String> {
    let key = env.resolve_pointer(pointer)?;
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("asset {} missing", key.asset))?;
    let info = asset
        .objects
        .get(&key.path_id)
        .ok_or_else(|| format!("{}#{} missing", asset.name, key.path_id))?;
    let body = asset.read_object(key.asset, info)?;
    Ok((key, asset, info, body))
}

pub(super) fn transform_relative_paths(
    env: &UnityEnvironment,
    root_transform: &Pointer,
) -> Result<BTreeSet<String>, String> {
    let mut paths = BTreeSet::new();
    let mut seen = HashSet::new();
    paths.insert(String::new());
    collect_transform_relative_paths(env, root_transform, "", &mut paths, &mut seen)?;
    Ok(paths)
}

pub(in super::super) fn scoped_evidence_batch(
    root: &Path, bundle: &str, asset: &str, sha: &str,
    requests: &[(i64, &str)],
) -> Result<Vec<JsonValue>, String> {
    let (path, route) = resolve_source_relative_container(root, bundle)?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    if super::super::native_publication::digest(&bytes) != sha {
        return Err("raw container differs from accepted source".into());
    }
    let loaded = load_input(&path)?;
    let mut result = Vec::new();
    for &(path_id, kind) in requests {
        let evidence = super::super::object_evidence::build_object_evidence(
            &loaded.env, super::super::object_evidence::ObjectEvidenceRequest {
                source_alias: "primary", relative_container: &route,
                container_bytes: &bytes, serialized_asset: Some(asset),
                expected_type: Some(kind), path_id, allow_unresolved_pointers: false,
            },
        )?;
        if evidence["triage"]["unresolvedPointerCount"] != 0 {
            return Err("unresolved pointer in scoped batch".into());
        }
        result.push(evidence);
    }
    if super::super::native_publication::digest(&fs::read(&path).map_err(|e| e.to_string())?) != sha {
        return Err("source changed during evidence recovery".into());
    }
    Ok(result)
}

pub(super) fn dump_gameobject_layers(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        let path = required_path(
            &args,
            0,
            "dump-gameobject-layers <asset-or-bundle> <out.json>",
        )?;
        let output = args
            .get(1)
            .map(PathBuf::from)
            .ok_or_else(|| "dump-gameobject-layers requires an output JSON path".to_owned())?;
        let loaded = load_input(&path)?;
        let mut layers = BTreeMap::<String, i64>::new();
        for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
            for (path_id, info) in &asset.objects {
                if asset.object_type_name(info) != "GameObject" {
                    continue;
                }
                let body = asset.read_object(asset_index, info)?;
                let layer = body
                    .get("m_Layer")
                    .and_then(UnityValue::as_i64)
                    .unwrap_or(0);
                layers.insert(format!("{}#{}", asset.name, path_id), layer);
            }
        }
        write_or_print_json(Some(&output), &json!({ "layers": layers }))
    })
}

pub(super) fn dump_xdt(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        let path = required_path(&args, 0, "dump-xdt <asset-or-bundle> [path-id] [out.json]")?;
        let path_id = args
            .get(1)
            .map(|value| parse_i64(value))
            .transpose()?
            .unwrap_or(7);
        let output = args.get(2).map(PathBuf::from);
        let loaded = load_input(&path)?;
        let (asset_index, asset, info) = find_object(&loaded.env, path_id)?;
        let body = asset.read_object(asset_index, info)?;
        write_or_print_json(output.as_deref(), &unity_to_json(&body))
    })
}

pub(super) fn enrich_native_terrains(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err(
            "enrich-native-terrains <client-project|effective-build-root> <committed-source-batch-root> <fresh-output-root>"
                .to_string(),
        );
    }
    let summary = super::super::native_terrain_batch::enrich_native_terrains_batch(
        Path::new(&args[0]),
        Path::new(&args[1]),
        Path::new(&args[2]),
    )?;
    println!(
        "Native terrain enrichment {}: scenes={}, environments={}, ambienceExact={}, detailExact={}, placementExact={}, mismatches={}, blocked={}, hardLinks={}, output={}",
        summary.status,
        summary.scanned_scene_count,
        summary.environment_document_count,
        summary.exact_ambience_count,
        summary.exact_terrain_detail_count,
        summary.exact_placement_count,
        summary.source_resource_scene_mismatch_count,
        summary.blocker_count,
        summary.hard_linked_file_count,
        summary.output_root,
    );
    Ok(())
}

pub(super) fn ff_extract(args: &[String]) -> Result<(), String> {
    let indir = required_path(args, 0, "ffextract <input-dir> <out-dir>")?;
    let outdir = required_path(args, 1, "ffextract <input-dir> <out-dir>")?;
    let files = fs::read_dir(&indir)
        .map_err(|err| err.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    let options = ExtractOptions {
        outdir,
        types: HashSet::from([
            "Mesh".to_string(),
            "Texture2D".to_string(),
            "TextAsset".to_string(),
            "Shader".to_string(),
            "Font".to_string(),
            "MovieTexture".to_string(),
            "AudioClip".to_string(),
        ]),
        filters: Vec::new(),
        dry_run: false,
        files: files.clone(),
    };
    for file in files {
        let loaded = match load_input(&file) {
            Ok(value) => value,
            Err(_) => continue,
        };
        for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
            if let Some(assetbundle) = asset.objects.get(&1) {
                if asset.object_type_name(assetbundle) == "AssetBundle" {
                    let body = asset.read_object(asset_index, assetbundle)?;
                    let preloads = value_array(body.get("m_PreloadTable")).to_vec();
                    for (path, metadata) in container_entries(&body) {
                        let Some((target_index, pointer, info)) = resolve_container_export_target(
                            &loaded.env,
                            asset_index,
                            &path,
                            metadata,
                            &preloads,
                        ) else {
                            continue;
                        };
                        let target_type = loaded.env.assets[target_index].object_type_name(info);
                        let export_path = fix_export_path_for_type(&path, &target_type);
                        export_object(
                            &loaded.env,
                            target_index,
                            pointer.path_id,
                            info,
                            &options,
                            Some(&export_path),
                        )?;
                    }
                    continue;
                }
            }
            for (path_id, info) in &asset.objects {
                export_object(&loaded.env, asset_index, *path_id, info, &options, None)?;
            }
        }
    }
    Ok(())
}

pub(super) fn npc_snapshot(args: &[String]) -> Result<(), String> {
    let bundle = required_path(args, 0, "npc-snapshot <Character_*.resourceFile> <out-dir>")?;
    let outdir = required_path(args, 1, "npc-snapshot <Character_*.resourceFile> <out-dir>")?;
    snapshot_npc_bundle(&bundle, &outdir)
}

pub(super) fn snapshot_key_type(env: &UnityEnvironment, key: (usize, i64)) -> Option<String> {
    let asset = env.assets.get(key.0)?;
    let info = asset.objects.get(&key.1)?;
    Some(asset.object_type_name(info))
}

pub(super) fn clear_readonly_recursive(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        for entry in fs::read_dir(path).map_err(|err| format!("{}: {err}", path.display()))? {
            let entry = entry.map_err(|err| err.to_string())?;
            clear_readonly_recursive(&entry.path())?;
        }
    }
    let metadata = fs::metadata(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
            .map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(())
}

pub(super) fn add_snapshot_key(
    selected: &mut BTreeSet<(usize, i64)>,
    queue: &mut VecDeque<(usize, i64)>,
    key: (usize, i64),
) {
    if selected.insert(key) {
        queue.push_back(key);
    }
}

pub(super) fn snapshot_pointer_candidate_keys(
    env: &UnityEnvironment,
    source_asset_index: usize,
    pointer: &Pointer,
) -> Vec<(usize, i64)> {
    if let Some(key) = resolved_object_key_for_snapshot(env, source_asset_index, pointer) {
        return vec![(key.asset, key.path_id)];
    }
    Vec::new()
}
