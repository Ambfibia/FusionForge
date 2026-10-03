use super::*;

pub(super) fn verify_physical_library(
    asset_root: &Path,
    map_root: &Path,
    allow_unregistered_shared_files: bool,
) -> Result<PublishedTerrainDedupVerification> {
    let catalog_path = map_root.join("catalog.json");
    let catalog_bytes = read_file(&catalog_path, "map catalog")?;
    let catalog_blake3 = hash(&catalog_bytes);
    let catalog: JsonValue =
        serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    let mut expected = BTreeMap::from([(
        "map/catalog.json".to_owned(),
        PhysicalArtifact {
            bytes: catalog_bytes.len() as u64,
            blake3: catalog_blake3.clone(),
        },
    )]);
    let mut pending = Vec::<String>::new();
    discover_physical_artifacts(&catalog, &mut expected, &mut pending)?;
    let mut visited_documents = BTreeSet::from(["map/catalog.json".to_owned()]);
    while let Some(route) = pending.pop() {
        let proof = expected
            .get(&route)
            .ok_or_else(|| invalid_error("physical artifact disappeared"))?;
        let bytes = read_asset_from_roots(asset_root, map_root, &route, "published artifact")?;
        if bytes.len() as u64 != proof.bytes || hash(&bytes) != proof.blake3 {
            return invalid(format!(
                "published artifact {route:?} differs from its physical acceptance proof"
            ));
        }
        if route.ends_with(".json") && visited_documents.insert(route.clone()) {
            let document: JsonValue =
                serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
                    path: route,
                    source,
                })?;
            discover_physical_artifacts(&document, &mut expected, &mut pending)?;
        }
    }

    // Verify non-JSON leaves too. JSON artifacts were already checked above.
    for (route, proof) in &expected {
        if route == "map/catalog.json" || route.ends_with(".json") {
            continue;
        }
        let bytes = read_asset_from_roots(asset_root, map_root, route, "published artifact")?;
        if bytes.len() as u64 != proof.bytes || hash(&bytes) != proof.blake3 {
            return invalid(format!(
                "published artifact {route:?} differs from its physical acceptance proof"
            ));
        }
    }

    let mut actual = collect_files(map_root)?
        .into_iter()
        .map(|path| {
            Ok(format!(
                "map/{}",
                slash_path(
                    path.strip_prefix(map_root)
                        .map_err(|_| invalid_error("map file escaped map root"))?
                )
            ))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let object_root = asset_root.join("objects");
    if object_root.is_dir() {
        actual.extend(
            collect_files(&object_root)?
                .into_iter()
                .map(|path| {
                    Ok(format!(
                        "objects/{}",
                        slash_path(
                            path.strip_prefix(&object_root)
                                .map_err(|_| invalid_error("object file escaped object root"))?
                        )
                    ))
                })
                .collect::<Result<Vec<_>>>()?,
        );
    }
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    let missing = expected_paths
        .difference(&actual)
        .cloned()
        .collect::<Vec<_>>();
    let orphan = actual
        .difference(&expected_paths)
        .cloned()
        .collect::<Vec<_>>();
    let allowed_orphans = allow_unregistered_shared_files
        && orphan.iter().all(|route| route.starts_with("map/shared/"));
    if !missing.is_empty() || (!orphan.is_empty() && !allowed_orphans) {
        return invalid(format!(
            "published map physical closure mismatch: missing={:?}, orphan={:?}",
            missing.iter().take(5).collect::<Vec<_>>(),
            orphan.iter().take(5).collect::<Vec<_>>()
        ));
    }
    let mut result_set = blake3::Hasher::new();
    for (route, proof) in &expected {
        append_set_hash(&mut result_set, route, &proof.blake3);
    }
    Ok(PublishedTerrainDedupVerification {
        catalog_blake3,
        files: expected.len() as u64,
        bytes: expected.values().map(|proof| proof.bytes).sum(),
        result_set_blake3: result_set.finalize().to_hex().to_string(),
        scene_links: 0,
    })
}

pub(super) fn exact_hashed_files_match(
    root: &Path,
    left_path: &str,
    left_hash: &str,
    right_path: &str,
    right_hash: &str,
    context: &str,
) -> Result<bool> {
    let left_expected = normalized_hash(left_hash)?;
    let right_expected = normalized_hash(right_hash)?;
    if left_expected != right_expected {
        return Ok(false);
    }
    let left = read_file(&checked_join(root, left_path)?, context)?;
    let right = read_file(&checked_join(root, right_path)?, context)?;
    if hash(&left) != left_expected || hash(&right) != right_expected {
        return invalid(format!(
            "{context} bytes differ from their declared BLAKE3 acceptance hash"
        ));
    }
    Ok(left == right)
}

pub(super) fn normalized_hash(value: &str) -> Result<&str> {
    let value = value.strip_prefix("blake3:").unwrap_or(value);
    value
        .parse::<blake3::Hash>()
        .map_err(|error| invalid_error(format!("invalid BLAKE3 {value:?}: {error}")))?;
    Ok(value)
}

pub(super) fn verify_artifact(asset_root: &Path, artifact: &JsonValue, label: &str) -> Result<Vec<u8>> {
    let route = required_string(artifact, "path", &format!("{label} path"))?;
    let bytes = read_file(&checked_join(asset_root, route)?, label)?;
    verify_artifact_bytes(artifact, &bytes, label)?;
    Ok(bytes)
}

pub(super) fn verify_artifact_from_roots(
    asset_root: &Path,
    map_root: &Path,
    artifact: &JsonValue,
    label: &str,
) -> Result<Vec<u8>> {
    let route = required_string(artifact, "path", &format!("{label} path"))?;
    let bytes = read_asset_from_roots(asset_root, map_root, route, label)?;
    verify_artifact_bytes(artifact, &bytes, label)?;
    Ok(bytes)
}

pub(super) fn verify_artifact_bytes(artifact: &JsonValue, bytes: &[u8], label: &str) -> Result<()> {
    let expected_bytes = artifact
        .get("bytes")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid_error(format!("{label} has no byte length")))?;
    let expected_hash = required_string(artifact, "blake3", &format!("{label} hash"))?;
    if bytes.len() as u64 != expected_bytes || hash(bytes) != normalized_hash(expected_hash)? {
        return invalid(format!("{label} differs from its acceptance artifact"));
    }
    Ok(())
}

pub(super) fn artifact(path: &str, bytes: &[u8]) -> JsonValue {
    serde_json::json!({
        "blake3": hash(bytes),
        "bytes": bytes.len() as u64,
        "path": path,
    })
}

pub(super) fn insert_bytes(
    files: &mut BTreeMap<String, Vec<u8>>,
    route: String,
    bytes: Vec<u8>,
    label: &str,
) -> Result<()> {
    if let Some(existing) = files.insert(route.clone(), bytes.clone())
        && existing != bytes
    {
        return invalid(format!("{label} collision at {route:?}"));
    }
    Ok(())
}

pub(super) fn acquire_lock(asset_root: &Path) -> Result<TransactionLock> {
    let path = asset_root.join(LOCK_FILE);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| io_at(&path, error))?;
    writeln!(file, "pid={}", std::process::id()).map_err(|error| io_at(&path, error))?;
    file.sync_all().map_err(|error| io_at(&path, error))?;
    Ok(TransactionLock { path })
}

pub(super) fn clone_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir(destination).map_err(|error| io_at(destination, error))?;
    let result = (|| -> Result<()> {
        let mut pending = vec![(source.to_path_buf(), destination.to_path_buf())];
        while let Some((from, to)) = pending.pop() {
            for entry in fs::read_dir(&from).map_err(|error| io_at(&from, error))? {
                let entry = entry.map_err(|error| io_at(&from, error))?;
                let source_path = entry.path();
                let destination_path = to.join(entry.file_name());
                let kind = entry
                    .file_type()
                    .map_err(|error| io_at(&source_path, error))?;
                if kind.is_symlink() {
                    return invalid(format!(
                        "map transaction refuses symlink {}",
                        source_path.display()
                    ));
                }
                if kind.is_dir() {
                    fs::create_dir(&destination_path)
                        .map_err(|error| io_at(&destination_path, error))?;
                    pending.push((source_path, destination_path));
                } else if kind.is_file() {
                    if fs::hard_link(&source_path, &destination_path).is_err() {
                        fs::copy(&source_path, &destination_path)
                            .map_err(|error| io_at(&destination_path, error))?;
                    }
                }
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}

pub(super) fn replace_staged_file(stage: &Path, route: &str, bytes: &[u8], must_exist: bool) -> Result<()> {
    let relative = map_relative(route)?;
    let path = checked_join(stage, relative)?;
    if must_exist && !path.is_file() {
        return invalid(format!("staged replacement is absent: {route}"));
    }
    if path.exists() {
        fs::remove_file(&path).map_err(|error| io_at(&path, error))?;
    }
    write_new(&path, bytes)
}

pub(super) fn map_relative(route: &str) -> Result<&str> {
    route
        .strip_prefix("map/")
        .ok_or_else(|| invalid_error(format!("route escaped map/: {route:?}")))
}

pub(super) fn remove_transaction_directory(asset_root: &Path, path: &Path, expected_name: &str) -> Result<()> {
    if path.parent() != Some(asset_root)
        || path.file_name().and_then(|name| name.to_str()) != Some(expected_name)
    {
        return invalid(format!(
            "refusing to remove unexpected transaction path {}",
            path.display()
        ));
    }
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| io_at(path, error))?;
    }
    Ok(())
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !path.is_dir() {
        return invalid(format!("{label} is not a directory: {}", path.display()));
    }
    Ok(path)
}

pub(super) fn checked_join(root: &Path, relative: impl AsRef<Path>) -> Result<PathBuf> {
    let mut output = root.to_path_buf();
    for component in relative.as_ref().components() {
        match component {
            Component::Normal(value) => output.push(value),
            Component::CurDir => {}
            _ => return invalid(format!("unsafe relative path: {:?}", relative.as_ref())),
        }
    }
    Ok(output)
}

pub(super) fn safe_slug(value: &str, fallback: &str, maximum: usize) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('_');
            }
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
        if slug.len() >= maximum {
            break;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug
    }
}

pub(super) fn required_string<'a>(value: &'a JsonValue, key: &str, label: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_error(format!("{label} is absent")))
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(generated_json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn append_set_hash(hasher: &mut blake3::Hasher, path: &str, hash: &str) {
    hasher.update(path.as_bytes());
    hasher.update(&[0]);
    hasher.update(hash.as_bytes());
    hasher.update(&[0xff]);
}

pub(super) fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
