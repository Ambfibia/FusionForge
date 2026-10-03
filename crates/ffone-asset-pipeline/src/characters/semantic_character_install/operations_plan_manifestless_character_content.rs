use super::*;

pub(super) fn legacy_aliases(legacy_route: &str, logical_name: &str) -> Result<Vec<String>> {
    let stem = Path::new(legacy_route)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("legacy character route has no UTF-8 file stem"))?;
    validate_file_name(stem)?;
    Ok((!stem.eq_ignore_ascii_case(logical_name))
        .then(|| vec![stem.to_owned()])
        .unwrap_or_default())
}

pub(super) fn plan_preserved_character_content(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<PreservedCharacterContent> {
    let mut modern_paths = BTreeSet::new();
    let mut package_ownership = BTreeMap::<String, (bool, bool)>::new();
    let mut external_entries = Vec::new();
    for entry in &manifest.files {
        let Some(package_root) = runtime_character_package_root(&entry.path)? else {
            continue;
        };
        if !modern_paths.insert(entry.path.to_ascii_lowercase()) {
            return invalid(format!(
                "duplicate modern character manifest path {:?}",
                entry.path
            ));
        }
        let installer_owned = entry
            .source_path
            .starts_with(SEMANTIC_CHARACTER_SOURCE_PREFIX);
        let ownership = package_ownership.entry(package_root).or_default();
        if installer_owned {
            ownership.0 = true;
        } else {
            ownership.1 = true;
            if entry.source_path.trim().is_empty()
                || !matches!(
                    entry.kind,
                    ProjectAssetKind::Model | ProjectAssetKind::Texture
                )
            {
                return invalid(format!(
                    "external character package entry has invalid provenance or kind: {:?}",
                    entry.path
                ));
            }
            external_entries.push(entry.clone());
        }
    }

    for (package_root, (installer_owned, external)) in &package_ownership {
        if *installer_owned && *external {
            return invalid(format!(
                "character package mixes semantic-installer and external ownership: {package_root:?}"
            ));
        }
    }
    if external_entries.is_empty() {
        return Ok(PreservedCharacterContent::default());
    }

    let external_model_paths = external_entries
        .iter()
        .filter(|entry| entry.kind == ProjectAssetKind::Model)
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let external_package_roots = external_entries
        .iter()
        .map(|entry| {
            runtime_character_package_root(&entry.path)?
                .ok_or_else(|| invalid_error("external character entry lost its package root"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    for package_root in &external_package_roots {
        let prefix = format!("{package_root}/");
        if !external_entries
            .iter()
            .any(|entry| entry.kind == ProjectAssetKind::Model && entry.path.starts_with(&prefix))
        {
            return invalid(format!(
                "external character package has no manifest-owned GLB: {package_root:?}"
            ));
        }
    }

    let registry_entries = manifest
        .files
        .iter()
        .filter(|entry| entry.path == SEMANTIC_CHARACTER_REGISTRY_PATH)
        .collect::<Vec<_>>();
    let [registry_entry] = registry_entries.as_slice() else {
        return invalid(
            "external character packages require exactly one runtime character registry",
        );
    };
    if registry_entry.kind != ProjectAssetKind::Data
        || !registry_entry
            .source_path
            .starts_with(SEMANTIC_CHARACTER_SOURCE_PREFIX)
    {
        return invalid("runtime character registry has unknown ownership or kind");
    }
    let registry_path = join_relative(asset_root, SEMANTIC_CHARACTER_REGISTRY_PATH)?;
    let registry_bytes = read_file(&registry_path)?;
    let registry: SemanticCharacterRegistry =
        serde_json::from_slice(&registry_bytes).map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return invalid(format!(
            "unsupported semantic character registry schema {:?}",
            registry.schema
        ));
    }
    ensure_unique_registry_models(&registry.models)?;
    let registry_by_glb = registry
        .models
        .iter()
        .map(|model| (model.glb.to_ascii_lowercase(), model))
        .collect::<BTreeMap<_, _>>();
    let external_models = external_entries
        .iter()
        .filter(|entry| entry.kind == ProjectAssetKind::Model)
        .map(|entry| {
            let model = registry_by_glb
                .get(&entry.path.to_ascii_lowercase())
                .ok_or_else(|| {
                    invalid_error(format!(
                        "external character GLB is absent from the runtime registry: {:?}",
                        entry.path
                    ))
                })?;
            if model.glb != entry.path || model.glb_blake3 != entry.blake3 {
                return invalid(format!(
                    "external character registry identity differs for {:?}",
                    entry.path
                ));
            }
            Ok((*model).clone())
        })
        .collect::<Result<Vec<_>>>()?;
    if external_models.len() != external_model_paths.len() {
        return invalid("external character registry/model closure is not one-to-one");
    }

    external_entries.sort_by(|left, right| left.path.cmp(&right.path));
    let mut external_models = external_models;
    external_models.sort_by(|left, right| left.glb.cmp(&right.glb));
    Ok(PreservedCharacterContent {
        entries: external_entries,
        models: external_models,
    })
}

/// The current runtime layout is owned by the typed character catalog and
/// deliberately has no global asset manifest. Reconstruct only the character
/// domain's exact file closure so an incremental install can prove that every
/// preserved package still matches `_runtime/characters.json`.
pub(super) fn plan_manifestless_character_content(asset_root: &Path) -> Result<PreservedCharacterContent> {
    let registry_path = join_relative(asset_root, SEMANTIC_CHARACTER_REGISTRY_PATH)?;
    let registry_bytes = read_file(&registry_path)?;
    let registry: SemanticCharacterRegistry =
        serde_json::from_slice(&registry_bytes).map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return invalid(format!(
            "unsupported semantic character registry schema {:?}",
            registry.schema
        ));
    }
    ensure_unique_registry_models(&registry.models)?;

    let mut expected_packages = BTreeMap::<String, &RuntimeCharacterModel>::new();
    for model in &registry.models {
        let package_root = runtime_character_package_root(&model.glb)?.ok_or_else(|| {
            invalid_error(format!(
                "manifestless character registry GLB is outside a runtime package: {:?}",
                model.glb
            ))
        })?;
        let expected_prefix = format!("{}/", model.category.directory());
        if !model.glb.starts_with(&expected_prefix) {
            return invalid(format!(
                "manifestless character registry category contradicts GLB path: {:?}",
                model.glb
            ));
        }
        if expected_packages
            .insert(package_root.clone(), model)
            .is_some()
        {
            return invalid(format!(
                "manifestless character package has more than one registry model: {package_root:?}"
            ));
        }
        let glb_path = join_relative(asset_root, &model.glb)?;
        let glb_bytes = read_file(&glb_path)?;
        if blake3::hash(&glb_bytes).to_hex().as_str() != model.glb_blake3 {
            return invalid(format!(
                "manifestless character registry hash differs for {:?}",
                model.glb
            ));
        }
    }

    let mut actual_packages = BTreeSet::new();
    for category_root in RUNTIME_CHARACTER_PACKAGE_ROOTS {
        let category_path = join_relative(asset_root, category_root)?;
        let entries = match fs::read_dir(&category_path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io_at(&category_path, error)),
        };
        for entry in entries {
            let entry = entry.map_err(|error| io_at(&category_path, error))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| io_at(&path, error))?;
            if file_type.is_symlink() || !file_type.is_dir() {
                return invalid(format!(
                    "manifestless character category contains a non-directory package: {path:?}"
                ));
            }
            let stable_id = entry.file_name().into_string().map_err(|_| {
                invalid_error("manifestless character package name is not valid UTF-8")
            })?;
            validate_file_name(&stable_id)?;
            let package_root = format!("{category_root}/{stable_id}");
            if MANIFESTLESS_EXTERNAL_NON_PACKAGE_ROOTS.contains(&package_root.as_str()) {
                let files = regular_files(&path)?;
                if files.is_empty() {
                    return invalid(format!(
                        "manifestless external character support directory is empty: {package_root:?}"
                    ));
                }
                if files.iter().any(|relative| {
                    Path::new(relative)
                        .extension()
                        .and_then(|value| value.to_str())
                        != Some("png")
                }) {
                    return invalid(format!(
                        "manifestless external character support directory contains a non-PNG file: {package_root:?}"
                    ));
                }
                continue;
            }
            actual_packages.insert(package_root);
        }
    }
    let expected_package_names = expected_packages.keys().cloned().collect::<BTreeSet<_>>();
    if actual_packages != expected_package_names {
        let missing = expected_package_names
            .difference(&actual_packages)
            .cloned()
            .collect::<Vec<_>>();
        let orphan = actual_packages
            .difference(&expected_package_names)
            .cloned()
            .collect::<Vec<_>>();
        return invalid(format!(
            "manifestless character package/catalog closure differs: missing={missing:?}, orphan={orphan:?}"
        ));
    }

    let mut entries = Vec::new();
    for (package_root, model) in &expected_packages {
        let package_path = join_relative(asset_root, package_root)?;
        let package_files = regular_files(&package_path)?;
        if package_files.is_empty() {
            return invalid(format!(
                "manifestless character package is empty: {package_root:?}"
            ));
        }
        let collision_contract = model
            .collision
            .as_deref()
            .map(|descriptor_relative| {
                if runtime_character_package_root(descriptor_relative)?.as_deref()
                    != Some(package_root.as_str())
                {
                    return invalid(
                        "manifestless collision descriptor escaped its character package",
                    );
                }
                let descriptor_path = join_relative(asset_root, descriptor_relative)?;
                let descriptor_bytes = read_file(&descriptor_path)?;
                let descriptor: Value =
                    serde_json::from_slice(&descriptor_bytes).map_err(|source| {
                        PipelineError::Json {
                            path: descriptor_path.display().to_string(),
                            source,
                        }
                    })?;
                if required_string(&descriptor, "schema")? != "ffone.native-character-collision.v1"
                    || required_string(&descriptor, "glb")? != model.glb
                    || required_string(&descriptor, "glbBlake3")? != model.glb_blake3
                {
                    return invalid(
                        "manifestless collision descriptor contradicts its registry model",
                    );
                }
                let collider_relative =
                    string(&descriptor, "colliderGlb").filter(|value| !value.is_empty());
                let collider_hash =
                    string(&descriptor, "colliderGlbBlake3").filter(|value| !value.is_empty());
                let collider_relative = match (collider_relative, collider_hash) {
                    (None, None) => None,
                    (Some(collider_relative), Some(collider_hash)) => {
                        if runtime_character_package_root(collider_relative)?.as_deref()
                            != Some(package_root.as_str())
                        {
                            return invalid(
                                "manifestless collider GLB escaped its character package",
                            );
                        }
                        let collider_path = join_relative(asset_root, collider_relative)?;
                        let collider_bytes = read_file(&collider_path)?;
                        if blake3::hash(&collider_bytes).to_hex().as_str() != collider_hash {
                            return invalid(
                                "manifestless collider GLB hash contradicts its descriptor",
                            );
                        }
                        Some(collider_relative.to_owned())
                    }
                    _ => {
                        return invalid(
                            "manifestless collision descriptor must declare both colliderGlb and colliderGlbBlake3 or neither",
                        );
                    }
                };
                Ok((descriptor_relative.to_owned(), collider_relative))
            })
            .transpose()?;
        let mut exact_glb_seen = false;
        for package_relative in package_files {
            let relative = format!("{package_root}/{package_relative}");
            let extension = Path::new(&package_relative)
                .extension()
                .and_then(|value| value.to_str());
            let kind = match extension {
                Some("glb") if relative == model.glb => {
                    if exact_glb_seen {
                        return invalid(format!(
                            "manifestless character package duplicates its registry GLB: {package_root:?}"
                        ));
                    }
                    exact_glb_seen = true;
                    ProjectAssetKind::Model
                }
                Some("glb")
                    if collision_contract.as_ref().is_some_and(|(_, collider)| {
                        collider.as_deref() == Some(relative.as_str())
                    }) =>
                {
                    ProjectAssetKind::Model
                }
                Some("json")
                    if collision_contract
                        .as_ref()
                        .is_some_and(|(descriptor, _)| descriptor == &relative) =>
                {
                    ProjectAssetKind::Data
                }
                Some("png") => ProjectAssetKind::Texture,
                _ => {
                    return invalid(format!(
                        "manifestless character package contains an unsupported runtime file: {relative:?}"
                    ));
                }
            };
            let path = join_relative(asset_root, &relative)?;
            let bytes = read_file(&path)?;
            entries.push(ProjectAssetFile {
                source_path: format!("manifestless-character-domain/{relative}"),
                path: relative,
                kind,
                bytes: bytes.len() as u64,
                blake3: blake3::hash(&bytes).to_hex().to_string(),
            });
        }
        if !exact_glb_seen {
            return invalid(format!(
                "manifestless character package has no registry GLB: {package_root:?}"
            ));
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(PreservedCharacterContent {
        entries,
        models: registry.models,
    })
}

pub(super) fn stage_preserved_character_content(
    asset_root: &Path,
    stage: &Path,
    preserved: &PreservedCharacterContent,
) -> Result<()> {
    for entry in &preserved.entries {
        let source = join_relative(asset_root, &entry.path)?;
        let bytes = read_file(&source)?;
        if bytes.len() as u64 != entry.bytes
            || blake3::hash(&bytes).to_hex().as_str() != entry.blake3
        {
            return invalid(format!(
                "external character changed while staging: {:?}",
                entry.path
            ));
        }
        write_new_file(stage, &entry.path, &bytes)?;
    }
    Ok(())
}

pub(super) fn commit_manifestless_transaction(
    asset_root: &Path,
    stage: &Path,
    package_roots: &BTreeSet<String>,
    replacement_package_roots: &BTreeSet<String>,
    report_output: &Path,
    report_bytes: &[u8],
) -> Result<()> {
    if package_roots.is_empty() {
        return invalid("manifestless semantic character transaction has no packages");
    }
    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let backup = asset_root.join(format!(".semantic-characters-backup-{token}"));
    fs::create_dir(&backup).map_err(|error| io_at(&backup, error))?;
    let report_next = sibling_transaction_path(report_output, &token, "next")?;
    let report_backup = sibling_transaction_path(report_output, &token, "backup")?;
    write_exact_new_file(&report_next, report_bytes)?;

    let registry_relative = SEMANTIC_CHARACTER_REGISTRY_PATH;
    let registry_stage = join_relative(stage, registry_relative)?;
    let registry_path = join_relative(asset_root, registry_relative)?;
    let registry_backup = join_relative(&backup, registry_relative)?;
    let registry_metadata =
        fs::symlink_metadata(&registry_path).map_err(|error| io_at(&registry_path, error))?;
    if !registry_metadata.is_file() || registry_metadata.file_type().is_symlink() {
        return invalid("manifestless runtime character registry is not a regular file");
    }
    if !registry_stage.is_file() {
        return invalid("manifestless semantic character stage has no runtime registry");
    }

    let mut moved_packages = Vec::<(PathBuf, PathBuf)>::new();
    let mut replaced_packages = Vec::<(PathBuf, PathBuf)>::new();
    let mut registry_old_moved = false;
    let mut registry_swapped = false;
    let mut report_old_moved = false;
    let mut report_swapped = false;
    let result = (|| -> Result<()> {
        for package_root in package_roots {
            let source = join_relative(stage, package_root)?;
            let destination = join_relative(asset_root, package_root)?;
            if !source.is_dir() {
                return invalid(format!(
                    "manifestless semantic character stage has no package {package_root:?}"
                ));
            }
            match fs::symlink_metadata(&destination) {
                Ok(metadata) if replacement_package_roots.contains(package_root) => {
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return invalid(format!(
                            "manifestless replacement target is not a plain directory: {package_root:?}"
                        ));
                    }
                    let archived = join_relative(&backup, package_root)?;
                    create_parent(&archived)?;
                    fs::rename(&destination, &archived)
                        .map_err(|error| io_at(&destination, error))?;
                    replaced_packages.push((destination.clone(), archived));
                }
                Ok(_) => {
                    return invalid(format!(
                        "manifestless semantic character destination already exists: {package_root:?}"
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if replacement_package_roots.contains(package_root) {
                        return invalid(format!(
                            "declared manifestless replacement target disappeared: {package_root:?}"
                        ));
                    }
                }
                Err(error) => return Err(io_at(&destination, error)),
            }
            create_parent(&destination)?;
            fs::rename(&source, &destination).map_err(|error| io_at(&source, error))?;
            moved_packages.push((source, destination));
        }

        create_parent(&registry_backup)?;
        fs::rename(&registry_path, &registry_backup)
            .map_err(|error| io_at(&registry_path, error))?;
        registry_old_moved = true;
        fs::rename(&registry_stage, &registry_path)
            .map_err(|error| io_at(&registry_stage, error))?;
        registry_swapped = true;

        match fs::symlink_metadata(report_output) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                fs::rename(report_output, &report_backup)
                    .map_err(|error| io_at(report_output, error))?;
                report_old_moved = true;
            }
            Ok(_) => return invalid("existing semantic character report is not a regular file"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_at(report_output, error)),
        }
        fs::rename(&report_next, report_output).map_err(|error| io_at(&report_next, error))?;
        report_swapped = true;
        Ok(())
    })();

    if let Err(error) = result {
        if report_swapped {
            let _ = fs::rename(report_output, &report_next);
        }
        if report_old_moved {
            let _ = fs::rename(&report_backup, report_output);
        }
        if registry_swapped {
            let _ = create_parent(&registry_stage);
            let _ = fs::rename(&registry_path, &registry_stage);
        }
        if registry_old_moved {
            let _ = create_parent(&registry_path);
            let _ = fs::rename(&registry_backup, &registry_path);
        }
        for (source, destination) in moved_packages.iter().rev() {
            let _ = create_parent(source);
            let _ = fs::rename(destination, source);
        }
        for (destination, archived) in replaced_packages.iter().rev() {
            let _ = create_parent(destination);
            let _ = fs::rename(archived, destination);
        }
        remove_file_if_exists(&report_next);
        remove_file_if_exists(&report_backup);
        let _ = fs::remove_dir_all(&backup);
        let _ = fs::remove_dir_all(stage);
        return Err(error);
    }

    remove_file_if_exists(&report_backup);
    let _ = fs::remove_dir_all(&backup);
    let _ = fs::remove_dir_all(stage);
    Ok(())
}

pub(super) fn commit_transaction(
    asset_root: &Path,
    stage: &Path,
    manifest_bytes: &[u8],
    report_output: &Path,
    report_bytes: &[u8],
) -> Result<()> {
    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let backup = asset_root.join(format!(".semantic-characters-backup-{token}"));
    fs::create_dir(&backup).map_err(|error| io_at(&backup, error))?;
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_next =
        asset_root.join(format!(".asset-manifest.semantic-characters-{token}.next"));
    let manifest_backup = asset_root.join(format!(
        ".asset-manifest.semantic-characters-{token}.backup"
    ));
    write_exact_new_file(&manifest_next, manifest_bytes)?;
    let report_next = sibling_transaction_path(report_output, &token, "next")?;
    let report_backup = sibling_transaction_path(report_output, &token, "backup")?;
    write_exact_new_file(&report_next, report_bytes)?;

    let mut moved_old = Vec::<(PathBuf, PathBuf)>::new();
    let mut moved_new = Vec::<(PathBuf, PathBuf)>::new();
    let mut manifest_swapped = false;
    let mut report_old_moved = false;
    let mut report_swapped = false;
    let result = (|| -> Result<()> {
        for target in MANAGED_TARGETS {
            let source = join_relative(asset_root, target)?;
            match fs::symlink_metadata(&source) {
                Ok(_) => {
                    let destination = join_relative(&backup, target)?;
                    create_parent(&destination)?;
                    fs::rename(&source, &destination).map_err(|error| io_at(&source, error))?;
                    moved_old.push((source, destination));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_at(&source, error)),
            }
        }
        for target in NEW_TARGETS {
            let source = join_relative(stage, target)?;
            let destination = join_relative(asset_root, target)?;
            create_parent(&destination)?;
            fs::rename(&source, &destination).map_err(|error| io_at(&source, error))?;
            moved_new.push((source, destination));
        }

        fs::rename(&manifest_path, &manifest_backup)
            .map_err(|error| io_at(&manifest_path, error))?;
        if let Err(error) = fs::rename(&manifest_next, &manifest_path) {
            let _ = fs::rename(&manifest_backup, &manifest_path);
            return Err(io_at(&manifest_next, error));
        }
        manifest_swapped = true;

        match fs::symlink_metadata(report_output) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                fs::rename(report_output, &report_backup)
                    .map_err(|error| io_at(report_output, error))?;
                report_old_moved = true;
            }
            Ok(_) => return invalid("existing semantic character report is not a regular file"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_at(report_output, error)),
        }
        fs::rename(&report_next, report_output).map_err(|error| io_at(&report_next, error))?;
        report_swapped = true;
        Ok(())
    })();

    if let Err(error) = result {
        if report_swapped {
            let _ = fs::rename(report_output, &report_next);
        }
        if report_old_moved {
            let _ = fs::rename(&report_backup, report_output);
        }
        if manifest_swapped {
            let _ = fs::rename(&manifest_path, &manifest_next);
            let _ = fs::rename(&manifest_backup, &manifest_path);
        }
        for (source, destination) in moved_new.iter().rev() {
            let _ = create_parent(source);
            let _ = fs::rename(destination, source);
        }
        for (source, destination) in moved_old.iter().rev() {
            let _ = create_parent(source);
            let _ = fs::rename(destination, source);
        }
        remove_file_if_exists(&manifest_next);
        remove_file_if_exists(&manifest_backup);
        remove_file_if_exists(&report_next);
        remove_file_if_exists(&report_backup);
        let _ = fs::remove_dir_all(&backup);
        let _ = fs::remove_dir_all(stage);
        return Err(error);
    }

    remove_file_if_exists(&manifest_backup);
    remove_file_if_exists(&report_backup);
    let _ = fs::remove_dir_all(&backup);
    let _ = fs::remove_dir_all(stage);
    Ok(())
}

pub(super) fn create_transaction_directory(asset_root: &Path, label: &str) -> Result<PathBuf> {
    for _ in 0..128 {
        let token = TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = asset_root.join(format!(
            ".semantic-characters-{label}-{}-{token}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_at(&path, error)),
        }
    }
    Err(PipelineError::StagingCollision(
        asset_root.join("characters"),
    ))
}

pub(super) fn canonical_output_file(path: &Path, asset_root: &Path) -> Result<PathBuf> {
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return invalid("semantic character report output must be JSON");
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|error| io_at(parent, error))?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid_error("semantic character report has no filename"))?;
    let output = parent.join(name);
    if output.starts_with(asset_root) {
        return invalid("semantic character report must live outside the runtime asset tree");
    }
    Ok(output)
}

pub(super) fn input_proof(role: &str, path: &Path, bytes: &[u8]) -> CharacterInstallInput {
    CharacterInstallInput {
        role: role.to_owned(),
        path: slash_path(path),
        bytes: bytes.len() as u64,
        sha256: sha256(bytes),
    }
}

pub(super) fn string_array(value: &Value, field: &str) -> Result<Vec<String>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_error(format!("{field} is not an array")))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_error(format!("{field} contains a non-string")))
        })
        .collect()
}

pub(super) fn value_array(value: Option<&Value>) -> &[Value] {
    value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(super) fn string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

pub(super) fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    string(value, field)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_error(format!("missing non-empty string {field}")))
}

pub(super) fn int_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().map(|value| value as i64))
    })
}

pub(super) fn pretty_json<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: label.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn create_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("transaction path has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))
}

pub(super) fn remove_file_if_exists(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}
