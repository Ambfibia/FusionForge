use super::*;

pub(super) fn prepare_publication(audit: &SourceAudit, source_build: &str) -> Result<Vec<PreparedFile>> {
    let mut files = Vec::new();
    let mut effect_entries = Vec::new();
    for closure in &audit.effect_closures {
        let effect_id = closure.effect_id.ok_or_else(|| {
            PipelineError::InvalidManifest("effect closure is missing effectId".to_owned())
        })?;
        let relative = format!("effects/es{effect_id}.closure.json");
        let bytes = pretty_json(closure)?;
        let object_types = closure
            .objects
            .iter()
            .map(|object| object.object_type.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let component_types = closure
            .objects
            .iter()
            .filter(|object| {
                !matches!(
                    object.object_type.as_str(),
                    "AssetBundle" | "GameObject" | "Transform"
                )
            })
            .map(|object| object.object_type.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        effect_entries.push(TutorialEffectCatalogEntry {
            effect_id,
            container_route: closure.container_route.clone(),
            root_asset: closure.root_asset.clone(),
            root_path_id: closure.root_path_id,
            closure_path: format!("{TUTORIAL_EFFECT_ROOT}/es{effect_id}.closure.json"),
            closure_bytes: bytes.len() as u64,
            closure_blake3: hash(&bytes),
            object_count: closure.objects.len() as u64,
            object_types,
            component_types,
        });
        files.push(PreparedFile {
            relative,
            source: format!("Effects.resourceFile#{}", closure.container_route),
            bytes,
        });
    }

    effect_entries.sort_by_key(|entry| entry.effect_id);
    let effect_catalog = TutorialEffectCatalog {
        schema: TUTORIAL_EFFECT_CATALOG_SCHEMA.to_owned(),
        source_build: source_build.to_owned(),
        source_bundle: audit.bundle.clone(),
        source_dump: audit.dump.clone(),
        source_assets: audit.source_assets.clone(),
        renderer_status: EFFECT_RENDERER_STATUS.to_owned(),
        effects: effect_entries,
    };
    files.push(PreparedFile {
        relative: "effects/catalog.json".to_owned(),
        source: "Effects.resourceFile#tutorial-effect-catalog".to_owned(),
        bytes: pretty_json(&effect_catalog)?,
    });

    let bullet_closure_bytes = pretty_json(&audit.bullet_closure)?;
    let bullet_closure_relative = "projectiles/bullettable.closure.json".to_owned();
    let bullet_closure_path = format!("{TUTORIAL_PROJECTILE_ROOT}/bullettable.closure.json");
    let bullet_closure_hash = hash(&bullet_closure_bytes);
    files.push(PreparedFile {
        relative: bullet_closure_relative,
        source: format!("Effects.resourceFile#{BULLET_TABLE_ROUTE}"),
        bytes: bullet_closure_bytes,
    });

    let mut projectile_effect_entries = Vec::new();
    for closure in &audit.projectile_effect_closures {
        let effect_id = closure.effect_id.ok_or_else(|| {
            PipelineError::InvalidManifest(
                "projectile effect closure is missing effectId".to_owned(),
            )
        })?;
        let relative = format!("projectiles/effects/es{effect_id}.closure.json");
        let bytes = pretty_json(closure)?;
        let object_types = closure
            .objects
            .iter()
            .map(|object| object.object_type.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let component_types = closure
            .objects
            .iter()
            .filter(|object| {
                !matches!(
                    object.object_type.as_str(),
                    "AssetBundle" | "GameObject" | "Transform"
                )
            })
            .map(|object| object.object_type.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        projectile_effect_entries.push(TutorialEffectCatalogEntry {
            effect_id,
            container_route: closure.container_route.clone(),
            root_asset: closure.root_asset.clone(),
            root_path_id: closure.root_path_id,
            closure_path: format!("{TUTORIAL_PROJECTILE_ROOT}/effects/es{effect_id}.closure.json"),
            closure_bytes: bytes.len() as u64,
            closure_blake3: hash(&bytes),
            object_count: closure.objects.len() as u64,
            object_types,
            component_types,
        });
        files.push(PreparedFile {
            relative,
            source: format!("Effects.resourceFile#{}", closure.container_route),
            bytes,
        });
    }
    projectile_effect_entries.sort_by_key(|entry| entry.effect_id);

    let mut bullet_entries = Vec::new();
    for bullet in &audit.bullets {
        let bytes = pretty_json(bullet)?;
        let path = format!(
            "{TUTORIAL_PROJECTILE_ROOT}/bullet-{}.json",
            bullet.bullet_type
        );
        bullet_entries.push(TutorialBulletCatalogEntry {
            bullet_type: bullet.bullet_type,
            row_path: path,
            row_bytes: bytes.len() as u64,
            row_blake3: hash(&bytes),
            serialized_row_blake3: bullet.serialized_row_blake3.clone(),
            parameters: bullet.parameters.clone(),
        });
        files.push(PreparedFile {
            relative: format!("projectiles/bullet-{}.json", bullet.bullet_type),
            source: format!(
                "Effects.resourceFile#{BULLET_TABLE_ROUTE}[{}]",
                bullet.bullet_type
            ),
            bytes,
        });
    }
    bullet_entries.sort_by_key(|entry| entry.bullet_type);
    let projectile_catalog = TutorialProjectileCatalog {
        schema: TUTORIAL_PROJECTILE_CATALOG_SCHEMA.to_owned(),
        source_build: source_build.to_owned(),
        source_bundle: audit.bundle.clone(),
        source_dump: audit.dump.clone(),
        source_assets: audit.source_assets.clone(),
        renderer_status: PROJECTILE_RENDERER_STATUS.to_owned(),
        bullet_table_route: BULLET_TABLE_ROUTE.to_owned(),
        bullet_table_root_path_id: audit.bullet_closure.root_path_id,
        bullet_table_closure_path: bullet_closure_path,
        bullet_table_closure_bytes: files
            .iter()
            .find(|file| file.relative == "projectiles/bullettable.closure.json")
            .map(|file| file.bytes.len() as u64)
            .unwrap_or_default(),
        bullet_table_closure_blake3: bullet_closure_hash,
        particle_effects: projectile_effect_entries,
        rows: bullet_entries,
    };
    files.push(PreparedFile {
        relative: "projectiles/catalog.json".to_owned(),
        source: "Effects.resourceFile#tutorial-projectile-catalog".to_owned(),
        bytes: pretty_json(&projectile_catalog)?,
    });

    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(files)
}

pub(super) fn build_closure(
    effect_id: Option<i32>,
    route: &str,
    root_asset: &str,
    root: i64,
    index: &BTreeMap<UnityObjectKey, &DumpObject>,
    source_bundle_blake3: &str,
    source_dump_blake3: &str,
    source_assets: &[TutorialSourceAssetProof],
) -> Result<TutorialEffectClosureFile> {
    let mut queue = VecDeque::from([UnityObjectKey {
        asset: root_asset.to_owned(),
        path_id: root,
    }]);
    let mut selected = BTreeSet::new();
    let mut external = Vec::new();
    while let Some(key) = queue.pop_front() {
        if !selected.insert(key.clone()) {
            continue;
        }
        let object = index.get(&key).copied().ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "{route:?} closure references missing object {}#{}",
                key.asset, key.path_id
            ))
        })?;
        let mut pointers = Vec::new();
        collect_pointers(&object.value, "$", &mut pointers);
        for pointer in pointers {
            if pointer.path_id == 0 {
                continue;
            }
            let target_asset = if pointer.file_id == 0 {
                Some(key.asset.as_str())
            } else {
                referenced_asset_name(&key.asset, pointer.file_id)
            };
            if let Some(target_asset) = target_asset {
                queue.push_back(UnityObjectKey {
                    asset: target_asset.to_owned(),
                    path_id: pointer.path_id,
                });
            } else {
                external.push(format!(
                    "{}#{}{} -> fileId={}, pathId={}",
                    key.asset, object.path_id, pointer.json_path, pointer.file_id, pointer.path_id
                ));
            }
        }
    }
    if !external.is_empty() {
        external.sort();
        return invalid(format!(
            "{route:?} dependency closure is incomplete: unresolved external PPtrs [{}]",
            external.join(", ")
        ));
    }

    let mut objects = Vec::with_capacity(selected.len());
    for key in selected {
        let object = index.get(&key).copied().ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "{route:?} closure target is unavailable: {}#{}",
                key.asset, key.path_id
            ))
        })?;
        let value = canonical_json(&object.value);
        let canonical_bytes = serde_json::to_vec(&value).map_err(json_error)?;
        objects.push(TutorialUnityObjectProof {
            asset: object.asset.clone(),
            path_id: object.path_id,
            type_id: object.type_id,
            class_id: object.class_id,
            object_type: object.object_type.clone(),
            name: object.name.clone(),
            canonical_blake3: hash(&canonical_bytes),
            value,
        });
    }
    Ok(TutorialEffectClosureFile {
        schema: TUTORIAL_EFFECT_CLOSURE_SCHEMA.to_owned(),
        effect_id,
        container_route: route.to_owned(),
        root_asset: root_asset.to_owned(),
        root_path_id: root,
        source_bundle_blake3: source_bundle_blake3.to_owned(),
        source_dump_blake3: source_dump_blake3.to_owned(),
        source_assets: source_assets.to_vec(),
        objects,
    })
}

pub(super) fn required_i32(row: &JsonValue, field: &str, bullet_type: i32) -> Result<i32> {
    let value = find_unique_field(row, field)?.as_i64().ok_or_else(|| {
        PipelineError::InvalidManifest(format!(
            "BulletTable row {bullet_type} field {field:?} is not an integer"
        ))
    })?;
    i32::try_from(value).map_err(|_| {
        PipelineError::InvalidManifest(format!(
            "BulletTable row {bullet_type} field {field:?} is outside i32"
        ))
    })
}

pub(super) fn required_f64(row: &JsonValue, field: &str, bullet_type: i32) -> Result<f64> {
    let value = find_unique_field(row, field)?.as_f64().ok_or_else(|| {
        PipelineError::InvalidManifest(format!(
            "BulletTable row {bullet_type} field {field:?} is not numeric"
        ))
    })?;
    if !value.is_finite() {
        return invalid(format!(
            "BulletTable row {bullet_type} field {field:?} is not finite"
        ));
    }
    Ok(value)
}

pub(super) fn required_string(row: &JsonValue, field: &str, bullet_type: i32) -> Result<String> {
    find_unique_field(row, field)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "BulletTable row {bullet_type} field {field:?} is not a string"
            ))
        })
}

pub(super) fn stage_prepared(
    stage: &Path,
    prepared: &[PreparedFile],
    manifest: Option<&ProjectAssetManifest>,
) -> Result<(Vec<ProjectAssetFile>, u64)> {
    let existing = manifest
        .into_iter()
        .flat_map(|manifest| manifest.files.iter())
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut entries = Vec::with_capacity(prepared.len());
    let mut bytes = 0_u64;
    for file in prepared {
        let destination = if let Some(relative) = file.relative.strip_prefix("effects/") {
            format!("{TUTORIAL_EFFECT_ROOT}/{relative}")
        } else if let Some(relative) = file.relative.strip_prefix("projectiles/") {
            format!("{TUTORIAL_PROJECTILE_ROOT}/{relative}")
        } else {
            return invalid(format!(
                "invalid prepared tutorial path {:?}",
                file.relative
            ));
        };
        if existing.contains(&destination.to_ascii_lowercase()) {
            return invalid(format!(
                "asset manifest already owns tutorial effect path {destination:?}"
            ));
        }
        let target = safe_join(stage, &file.relative)?;
        write_new(&target, &file.bytes)?;
        bytes = bytes.saturating_add(file.bytes.len() as u64);
        entries.push(ProjectAssetFile {
            source_path: file.source.clone(),
            path: destination,
            kind: ProjectAssetKind::Data,
            bytes: file.bytes.len() as u64,
            blake3: hash(&file.bytes),
        });
    }
    Ok((entries, bytes))
}

pub(super) fn stage_preserved_effect_payloads(stage: &Path, previous: &[(OwnedTree, PathBuf)]) -> Result<()> {
    if let Some((_, effect_backup)) = previous
        .iter()
        .find(|(tree, _)| matches!(tree, OwnedTree::Effects))
    {
        for directory in ["models", "textures"] {
            let source = effect_backup.join(directory);
            if source.exists() {
                reject_symlink(&source, "existing tutorial effect payload directory")?;
                copy_directory_tree(&source, &stage.join("effects").join(directory))?;
            }
        }
        for entry in fs::read_dir(effect_backup).map_err(|error| io_at(effect_backup, error))? {
            let entry = entry.map_err(|error| io_at(effect_backup, error))?;
            let source = entry.path();
            let file_type = entry.file_type().map_err(|error| io_at(&source, error))?;
            if file_type.is_symlink() {
                return invalid(format!(
                    "existing tutorial effect payload contains a symlink at {}",
                    source.display()
                ));
            }
            let Some(file_name) = source.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if file_type.is_file() && file_name.ends_with(".nif-animation.json") {
                let destination = stage.join("effects").join(file_name);
                fs::copy(&source, &destination).map_err(|error| io_at(&destination, error))?;
            }
        }
    }
    if let Some((_, projectile_backup)) = previous
        .iter()
        .find(|(tree, _)| matches!(tree, OwnedTree::Projectiles))
    {
        let source = projectile_backup.join("effects").join("models");
        if source.exists() {
            reject_symlink(
                &source,
                "existing tutorial projectile effect models directory",
            )?;
            copy_directory_tree(
                &source,
                &stage.join("projectiles").join("effects").join("models"),
            )?;
        }
    }
    Ok(())
}

pub(super) fn restore_previous(
    effects_destination: &Path,
    projectiles_destination: &Path,
    previous: &[(OwnedTree, PathBuf)],
) {
    for (tree, backup) in previous {
        let destination = match tree {
            OwnedTree::Effects => effects_destination,
            OwnedTree::Projectiles => projectiles_destination,
        };
        if !destination.exists() && backup.exists() {
            let _ = fs::rename(backup, destination);
        }
    }
}

pub(super) fn cleanup_previous(previous: Vec<(OwnedTree, PathBuf)>) -> Result<()> {
    for (_, backup) in previous {
        fs::remove_dir_all(&backup).map_err(|error| io_at(&backup, error))?;
    }
    Ok(())
}

pub(super) fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
    }
    Ok(path)
}

pub(super) fn canonical_regular_file(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!(
            "{label} must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(PipelineError::InvalidManifest(message.into()))
}
