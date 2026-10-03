use super::*;

pub(super) fn is_false(value: &bool) -> bool {
    !*value
}

pub(super) fn group_named_oggs<'a>(sources: &'a [NamedOgg]) -> BTreeMap<String, Vec<&'a NamedOgg>> {
    let mut grouped = BTreeMap::<String, Vec<&NamedOgg>>::new();
    for source in sources {
        grouped
            .entry(folded(&source.true_name))
            .or_default()
            .push(source);
    }
    grouped
}

pub(super) fn fresh_matches_binding(fresh: &FreshAudio, source: &NamedOgg) -> bool {
    fresh.sources.iter().any(|proof| {
        let path_id_matches =
            proof.get("pathId").and_then(serde_json::Value::as_u64) == Some(source.path_id);
        if !path_id_matches {
            return false;
        }
        if proof
            .get("asset")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|asset| folded(asset) == folded(&source.container))
        {
            return true;
        }
        proof
            .get("file")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|file| {
                let folded_file = folded(&file.replace('\\', "/"));
                let needle = format!("/{}/{}__", folded(&source.container), source.path_id);
                folded_file.contains(&needle)
                    || folded_file.starts_with(needle.trim_start_matches('/'))
            })
    })
}

pub(super) fn unique_named_by_hash<'a>(sources: impl Iterator<Item = &'a NamedOgg>) -> Vec<&'a NamedOgg> {
    let mut unique = BTreeMap::<&str, &NamedOgg>::new();
    for source in sources {
        unique.entry(source.blake3.as_str()).or_insert(source);
    }
    unique.into_values().collect()
}

pub(super) fn unique_fresh_by_hash<'a>(sources: impl Iterator<Item = &'a FreshAudio>) -> Vec<&'a FreshAudio> {
    let mut unique = BTreeMap::<&str, &FreshAudio>::new();
    for source in sources {
        unique.entry(source.blake3.as_str()).or_insert(source);
    }
    unique.into_values().collect()
}

pub(super) fn canonical_true_name(
    base: Option<&Vec<&BaseAsset>>,
    russian: Option<&Vec<&NamedOgg>>,
    clean: Option<&Vec<&NamedOgg>>,
    fresh: Option<&Vec<&FreshAudio>>,
) -> Result<String> {
    let names = base
        .into_iter()
        .flatten()
        .map(|asset| asset.true_name.as_str())
        .chain(
            russian
                .into_iter()
                .flatten()
                .map(|asset| asset.true_name.as_str()),
        )
        .chain(
            clean
                .into_iter()
                .flatten()
                .map(|asset| asset.true_name.as_str()),
        )
        .chain(
            fresh
                .into_iter()
                .flatten()
                .map(|asset| asset.true_name.as_str()),
        )
        .collect::<Vec<_>>();
    let Some(first) = names.first() else {
        return invalid("voice group has no true name");
    };
    if names.iter().any(|name| folded(name) != folded(first)) {
        return invalid("case-folded voice group contains disagreeing true names");
    }
    Ok((*first).to_owned())
}

pub(super) fn canonical_owner(true_name: &str, base: Option<&Vec<&BaseAsset>>) -> Result<String> {
    let mut owners = base
        .into_iter()
        .flatten()
        .map(|asset| portable_component(&asset.owner, "voice owner"))
        .collect::<Result<BTreeSet<_>>>()?;
    let inferred = infer_voice_owner(true_name)?;
    if owners.is_empty() {
        return Ok(inferred);
    }
    if owners.len() == 1 {
        let owner = owners.pop_first().unwrap_or_default();
        if matches!(owner.as_str(), "f" | "m") && inferred.starts_with(&format!("{owner}_")) {
            return Ok(inferred);
        }
        return Ok(owner);
    }
    if owners.contains(&inferred) {
        return Ok(inferred);
    }
    invalid(format!(
        "voice owner is ambiguous for {true_name:?}: {owners:?}"
    ))
}

pub(super) fn canonical_scope(base: Option<&Vec<&BaseAsset>>) -> Option<String> {
    base.into_iter()
        .flatten()
        .any(|asset| asset.scope.as_deref() == Some(CHARACTER_CREATION_SCOPE))
        .then(|| CHARACTER_CREATION_SCOPE.to_owned())
}

pub(super) fn plan_nonvoice_destinations(assets: &[BaseAsset]) -> Result<BTreeMap<String, String>> {
    let nonvoice = assets
        .iter()
        .filter(|asset| asset.category != SemanticAudioCategory::Voice)
        .collect::<Vec<_>>();
    if nonvoice.len() != EXPECTED_NONVOICE_ASSETS {
        return invalid(format!(
            "strict non-voice closure expected {EXPECTED_NONVOICE_ASSETS} assets, found {}",
            nonvoice.len()
        ));
    }
    let mut flattened = BTreeMap::<String, Vec<&BaseAsset>>::new();
    for asset in &nonvoice {
        let source = asset
            .file
            .as_ref()
            .ok_or_else(|| invalid_error("non-voice base asset has no file"))?;
        flattened
            .entry(flatten_legacy_variant_path(&source.path)?)
            .or_default()
            .push(asset);
    }

    let mut plan = BTreeMap::new();
    let mut destinations = BTreeMap::<String, String>::new();
    for (flat_path, group) in flattened {
        for asset in &group {
            let source = asset.file.as_ref().unwrap();
            let destination = if group.len() == 1 {
                flat_path.clone()
            } else {
                let context = asset.semantic_context.as_deref().ok_or_else(|| {
                    invalid_error(format!(
                        "distinct non-voice collision {:?} has no semantic context",
                        source.path
                    ))
                })?;
                insert_semantic_context(&flat_path, context)?
            };
            if destination.contains("/variants/") {
                return invalid(format!(
                    "non-voice canonicalizer retained a variant segment: {destination:?}"
                ));
            }
            if let Some(first) = destinations.insert(folded(&destination), source.path.clone()) {
                return invalid(format!(
                    "semantic non-voice destination collision at {destination:?}: {first:?} and {:?}",
                    source.path
                ));
            }
            plan.insert(source.path.clone(), destination);
        }
    }
    if plan.len() != EXPECTED_NONVOICE_ASSETS {
        return invalid("strict non-voice destination plan lost one or more logical assets");
    }
    Ok(plan)
}

pub(super) fn insert_semantic_context(path: &str, context: &str) -> Result<String> {
    let (parent, filename) = path
        .rsplit_once('/')
        .ok_or_else(|| invalid_error(format!("non-voice path has no parent: {path:?}")))?;
    let context = portable_component(context, "non-voice semantic context")?;
    let destination = format!("{parent}/{context}/{filename}");
    validate_relative_path(&destination)?;
    Ok(destination)
}

pub(super) fn semantic_context_from_provenance(
    proofs: &[BTreeMap<String, serde_json::Value>],
) -> Option<String> {
    let bundles = proofs
        .iter()
        .filter_map(|proof| proof.get("bundle"))
        .filter_map(serde_json::Value::as_str)
        .map(folded)
        .collect::<BTreeSet<_>>();
    if bundles
        .iter()
        .any(|bundle| bundle.starts_with("dongresources_"))
    {
        Some("zone_local".to_owned())
    } else if bundles
        .iter()
        .any(|bundle| bundle.starts_with("worldshared_") || bundle == "retro_shared.resourcefile")
    {
        Some("world_shared".to_owned())
    } else if bundles
        .iter()
        .any(|bundle| bundle == "tutorialaudio.resourcefile")
    {
        Some("tutorial_audio".to_owned())
    } else if bundles.iter().any(|bundle| bundle.starts_with("npc_pack_")) {
        Some("npc_pack".to_owned())
    } else {
        None
    }
}

pub(super) fn commit_transaction(
    audio_root: &Path,
    obsolete_catalog_target: &Path,
    runtime_catalog_target: &Path,
    generated_target: &Path,
    stage: &Path,
    backup: &Path,
    manifest_path: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<()> {
    let generated_parent = generated_target
        .parent()
        .ok_or_else(|| invalid_error("generated report target has no parent"))?;
    fs::create_dir_all(generated_parent).map_err(|source| io_at(generated_parent, source))?;
    if let Some(parent) = runtime_catalog_target.parent() {
        fs::create_dir_all(parent).map_err(|source| io_at(parent, source))?;
    }
    fs::create_dir(backup).map_err(|source| io_at(backup, source))?;

    let mut targets = ["music", "ambient", "voice", "sfx"]
        .into_iter()
        .map(|directory| TransactionTarget {
            target: audio_root.join(directory),
            staged: Some(stage.join(directory)),
            backup: backup.join(directory),
        })
        .collect::<Vec<_>>();
    targets.extend([
        TransactionTarget {
            target: obsolete_catalog_target.to_owned(),
            staged: None,
            backup: backup.join("obsolete-audio-catalog.json"),
        },
        TransactionTarget {
            target: runtime_catalog_target.to_owned(),
            staged: Some(stage.join("runtime/audio.json")),
            backup: backup.join("runtime-audio.json"),
        },
        TransactionTarget {
            target: generated_target.to_owned(),
            staged: Some(stage.join("generated")),
            backup: backup.join("generated"),
        },
    ]);
    let mut pending_targets = Vec::new();
    for item in targets {
        if let Some(staged) = &item.staged
            && item.target.exists()
            && paths_are_identical(&item.target, staged)?
        {
            remove_transaction_path(staged)?;
            continue;
        }
        if item.staged.is_none() && !item.target.exists() {
            continue;
        }
        pending_targets.push(item);
    }
    let targets = pending_targets;
    let mut backed_up = Vec::new();
    for (index, item) in targets.iter().enumerate() {
        if item.target.exists() {
            if let Err(source) = fs::rename(&item.target, &item.backup) {
                rollback_transaction(&targets, &[], &backed_up);
                return Err(io_at(&item.target, source));
            }
            backed_up.push(index);
        }
    }
    let mut installed = Vec::new();
    for (index, item) in targets.iter().enumerate() {
        let Some(source) = &item.staged else {
            continue;
        };
        if let Err(error) = fs::rename(source, &item.target) {
            rollback_transaction(&targets, &installed, &backed_up);
            return Err(io_at(&item.target, error));
        }
        installed.push(index);
    }
    if let Err(error) = replace_manifest(manifest_path, manifest) {
        rollback_transaction(&targets, &installed, &backed_up);
        return Err(error);
    }
    fs::remove_dir_all(backup).map_err(|source| io_at(backup, source))?;
    fs::remove_dir_all(stage).map_err(|source| io_at(stage, source))
}

pub(super) fn rollback_transaction(targets: &[TransactionTarget], installed: &[usize], backed_up: &[usize]) {
    for index in installed.iter().rev() {
        let item = &targets[*index];
        if let Some(stage) = &item.staged {
            let _ = fs::rename(&item.target, stage);
        }
    }
    for index in backed_up.iter().rev() {
        let item = &targets[*index];
        let _ = fs::rename(&item.backup, &item.target);
    }
}

pub(super) fn paths_are_identical(left: &Path, right: &Path) -> Result<bool> {
    let left_metadata = fs::metadata(left).map_err(|source| io_at(left, source))?;
    let right_metadata = fs::metadata(right).map_err(|source| io_at(right, source))?;
    if left_metadata.is_file() != right_metadata.is_file()
        || left_metadata.is_dir() != right_metadata.is_dir()
    {
        return Ok(false);
    }
    if left_metadata.is_file() {
        return Ok(
            left_metadata.len() == right_metadata.len() && hash_file(left)? == hash_file(right)?
        );
    }
    if !left_metadata.is_dir() {
        return invalid(format!(
            "transaction identity comparison encountered a non-file entry at {}",
            left.display()
        ));
    }
    Ok(collect_tree_identities(left)? == collect_tree_identities(right)?)
}

pub(super) fn inspect_ogg(path: &Path) -> Result<(u64, String)> {
    let mut file = fs::File::open(path).map_err(|source| io_at(path, source))?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)
        .map_err(|source| io_at(path, source))?;
    if &magic != b"OggS" {
        return invalid(format!(
            "source is not an Ogg bitstream: {}",
            path.display()
        ));
    }
    let bytes = file.metadata().map_err(|source| io_at(path, source))?.len();
    drop(file);
    Ok((bytes, hash_file(path)?))
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|source| io_at(path, source))?;
    if !canonical.is_dir() {
        return invalid(format!("{label} is not a directory: {}", path.display()));
    }
    Ok(canonical)
}

pub(super) fn portable_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        invalid_error(format!(
            "{} is outside source root {}",
            path.display(),
            root.display()
        ))
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return invalid(format!("unsafe source path {}", path.display()));
        };
        let value = value
            .to_str()
            .ok_or_else(|| invalid_error(format!("non-UTF-8 source path {}", path.display())))?;
        parts.push(value);
    }
    Ok(parts.join("/"))
}

pub(super) fn portable_component(value: &str, context: &str) -> Result<String> {
    let normalized = value.nfkc().collect::<String>().to_lowercase();
    let mut output = String::new();
    let mut separator = false;
    for character in normalized.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            separator = false;
        } else if !separator && !output.is_empty() {
            output.push('_');
            separator = true;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() || matches!(output.as_str(), "." | "..") {
        return invalid(format!("{context} has no portable identity: {value:?}"));
    }
    Ok(output)
}

pub(super) fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn folded(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).collect()
}

pub(super) fn transaction_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
