use super::*;

pub(super) fn build_plan(asset_root: &Path, recovery_root: &Path) -> Result<MigrationPlan> {
    let catalog_path = asset_root.join(native_path(STRICT_AUDIO_CATALOG_PATH));
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let source_catalog_bytes =
        fs::read(&catalog_path).map_err(|source| io_at(&catalog_path, source))?;
    let schema =
        serde_json::from_slice::<serde_json::Value>(&source_catalog_bytes).map_err(|source| {
            PipelineError::Json {
                path: catalog_path.display().to_string(),
                source,
            }
        })?["schema"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
    let catalog: StrictAudioCatalog = if schema == STRICT_AUDIO_CATALOG_SCHEMA {
        editable_catalog_with_disk_identities(&catalog_path, &source_catalog_bytes)?
    } else {
        serde_json::from_slice(&source_catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?
    };
    let manifest: ProjectAssetManifest = read_json(&manifest_path)?;
    let source_schema = catalog.schema.clone();
    validate_source_documents(asset_root, &catalog, &manifest)?;

    let manifest_by_path = manifest
        .files
        .iter()
        .map(|file| (folded(&file.path), file))
        .collect::<BTreeMap<_, _>>();
    let mut planned_files = Vec::with_capacity(catalog.counts.files as usize + 300);
    let mut output_assets = Vec::with_capacity(catalog.assets.len() + 300);
    let mut source_audio_paths = BTreeSet::new();
    let mut output_paths = BTreeSet::new();
    let mut packet_hashes = BTreeSet::new();
    let mut logical_keys = BTreeSet::new();
    let mut nano_voice_assets = 0_u64;
    let mut nano_skill_assets = 0_u64;
    let mut computress_voice_assets = 0_u64;
    let mut moved_files = 0_u64;
    let mut preserved_files = 0_u64;
    let mut aliases_added = 0_u64;
    let mut normalized_scopes = 0_u64;
    let mut normalized_source_paths = 0_u64;

    for source_asset in &catalog.assets {
        let route = classify_existing(source_asset)?;
        let mut asset = source_asset.clone();
        if normalize_recovery_scope(&mut asset) {
            normalized_scopes += 1;
        }
        let old_key = asset.logical_key.clone();
        let line = logical_line(&old_key, &asset.true_name)?;
        match &route {
            Route::Preserve => {}
            Route::NanoVoice { canonical_owner } => {
                let owner = format!("nano_{canonical_owner}");
                asset.logical_key = format!("voice/nanos/{owner}/{line}");
                asset.owner = owner;
                nano_voice_assets += 1;
            }
            Route::NanoSkill { power, skill_owner } => {
                asset.logical_key = format!("sfx/nano_skills/{power}/{skill_owner}/{line}");
                asset.owner = "nano_skills".to_owned();
                asset.category = SemanticAudioCategory::Sfx;
                nano_skill_assets += 1;
            }
            Route::Computress { section } => {
                asset.logical_key = format!("voice/npcs/computress/{section}/{line}");
                asset.owner = if section == "dialogue" {
                    "computress".to_owned()
                } else {
                    "nano_computress".to_owned()
                };
                asset.category = SemanticAudioCategory::Voice;
                for file in &mut asset.files {
                    if file.locale.is_none() {
                        file.locale = Some(DEFAULT_LOCALE.to_owned());
                        file.language_neutral = false;
                    }
                }
                computress_voice_assets += 1;
            }
        }
        if route != Route::Preserve && folded(&old_key) != folded(&asset.logical_key) {
            if !asset
                .aliases
                .iter()
                .any(|alias| folded(alias) == folded(&old_key))
            {
                asset.aliases.push(old_key);
                aliases_added += 1;
            }
        }
        asset.aliases.sort_by_key(|alias| folded(alias));

        let routed_owner = asset.owner.clone();
        for (index, file) in asset.files.iter_mut().enumerate() {
            let old_path = &source_asset.files[index].path;
            let manifest_file = manifest_by_path.get(&folded(old_path)).ok_or_else(|| {
                invalid_error(format!(
                    "catalog audio path is absent from manifest: {old_path:?}"
                ))
            })?;
            source_audio_paths.insert(folded(old_path));
            let next_path = routed_file_path(&route, &routed_owner, file, &line)?;
            moved_files += u64::from(next_path != *old_path);
            preserved_files += u64::from(next_path == *old_path);
            file.path = next_path.clone();
            if matches!(route, Route::NanoSkill { .. }) {
                file.locale = None;
                file.language_neutral = true;
            }
            if !output_paths.insert(folded(&next_path)) {
                return invalid(format!(
                    "audio taxonomy output path collision at {next_path:?}"
                ));
            }
            let (source_path, normalized) =
                normalize_recovery_source_path(&manifest_file.source_path);
            normalized_source_paths += u64::from(normalized);
            planned_files.push(PlannedFile {
                source_absolute: asset_root.join(native_path(old_path)),
                source_path,
                target_path: next_path,
                bytes: file.bytes,
                blake3: file.blake3.clone(),
            });
            packet_hashes.insert(ogg_packet_hash(&asset_root.join(native_path(old_path)))?);
        }
        output_assets.push(asset);
    }

    let manifest_audio_paths = manifest
        .files
        .iter()
        .filter(|file| file.path.starts_with("audio/"))
        .map(|file| folded(&file.path))
        .collect::<BTreeSet<_>>();
    if manifest_audio_paths != source_audio_paths {
        return invalid(format!(
            "runtime catalog/audio manifest closure differs: catalogFiles={}, manifestFiles={}",
            source_audio_paths.len(),
            manifest_audio_paths.len()
        ));
    }

    for asset in &output_assets {
        register_keys(asset, &mut logical_keys)?;
    }
    let mut logical_true_names = output_assets
        .iter()
        .map(|asset| folded(&asset.true_name))
        .collect::<BTreeSet<_>>();

    let recovery = discover_recovery_oggs(recovery_root)?;
    let recovery_files_scanned = recovery.len() as u64;
    let mut recovery_exact_matches = 0_u64;
    let mut recovery_voice_added = 0_u64;
    let mut recovery_skills_added = 0_u64;
    for source in recovery {
        if source.true_name.trim().is_empty() {
            if packet_hashes.contains(&source.packet_blake3) {
                recovery_exact_matches += 1;
                continue;
            }
            return invalid(format!(
                "unnamed recovery OGG has no catalogued payload identity: {} ({})",
                source.relative_path, source.blake3
            ));
        }
        // A legacy TrueName is the authored logical identity. Different Ogg
        // encodings or payloads with the same name are not inferred as
        // variants; this is the same zero-inferred-variants rule used by the
        // strict localized-voice importer.
        if !logical_true_names.insert(folded(&source.true_name)) {
            recovery_exact_matches += 1;
            continue;
        }
        let (mut asset, route) = recovery_asset(&source)?;
        make_recovery_identity_unique(&mut asset, &source.blake3, &logical_keys, &output_paths)?;
        register_keys(&asset, &mut logical_keys)?;
        let file = asset
            .files
            .first()
            .ok_or_else(|| invalid_error("recovery asset has no file"))?;
        if !output_paths.insert(folded(&file.path)) {
            return invalid(format!("recovery output path collision at {:?}", file.path));
        }
        planned_files.push(PlannedFile {
            source_absolute: source.absolute_path.clone(),
            source_path: format!("nano-recovery/{}", source.relative_path),
            target_path: file.path.clone(),
            bytes: file.bytes,
            blake3: file.blake3.clone(),
        });
        match route {
            Route::NanoVoice { .. } => recovery_voice_added += 1,
            Route::NanoSkill { .. } => recovery_skills_added += 1,
            _ => return invalid("internal recovery route is not Nano-owned"),
        }
        output_assets.push(asset);
    }

    let source_already_migrated = catalog.assets.iter().any(|asset| {
        asset.logical_key.starts_with("voice/nanos/")
            || asset.logical_key.starts_with("sfx/nano_skills/")
            || asset.logical_key.starts_with("voice/npcs/computress/")
    });
    validate_exact_recovery_skill_routes(&output_assets)?;
    validate_generic_recovery_metadata(&output_assets, &planned_files, recovery_files_scanned)?;
    validate_production_closure(
        source_already_migrated,
        recovery_files_scanned,
        nano_voice_assets,
        nano_skill_assets,
        computress_voice_assets,
        recovery_exact_matches,
        recovery_voice_added,
        recovery_skills_added,
    )?;

    output_assets.sort_by(|left, right| left.logical_key.cmp(&right.logical_key));
    for asset in &mut output_assets {
        asset
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
    }
    let counts = catalog_counts(&output_assets)?;
    let output_catalog = StrictAudioCatalog {
        schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        fallback_locale: catalog.fallback_locale,
        locale_fallbacks: catalog.locale_fallbacks,
        counts,
        assets: output_assets,
    };
    validate_output_catalog(&output_catalog)?;

    let catalog_bytes = serialize_editable_runtime_catalog(&output_catalog)?;
    let mut next_manifest = manifest.clone();
    next_manifest
        .files
        .retain(|file| !file.path.starts_with("audio/") && file.path != STRICT_AUDIO_CATALOG_PATH);
    next_manifest
        .files
        .extend(planned_files.iter().map(|file| ProjectAssetFile {
            source_path: file.source_path.clone(),
            path: file.target_path.clone(),
            kind: ProjectAssetKind::Audio,
            bytes: file.bytes,
            blake3: file.blake3.clone(),
        }));
    next_manifest.files.push(ProjectAssetFile {
        source_path: "audio-taxonomy-v1/runtime-catalog".to_owned(),
        path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: catalog_bytes.len() as u64,
        blake3: blake3::hash(&catalog_bytes).to_hex().to_string(),
    });
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    validate_output_manifest(&next_manifest)?;

    let report = AudioTaxonomyMigrationReport {
        schema: AUDIO_TAXONOMY_MIGRATION_SCHEMA.to_owned(),
        applied: false,
        source_catalog_schema: source_schema,
        output_catalog_schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        catalog_assets: output_catalog.counts.assets,
        catalog_files: output_catalog.counts.files,
        catalog_file_bytes: output_catalog.counts.file_bytes,
        nano_voice_assets,
        nano_skill_assets,
        computress_voice_assets,
        recovery_files_scanned,
        recovery_exact_matches,
        recovery_voice_added,
        recovery_skills_added,
        moved_files,
        preserved_files,
        aliases_added,
        normalized_scopes,
        normalized_source_paths,
        manifest_files: next_manifest.files.len() as u64,
        catalog_path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
    };
    Ok(MigrationPlan {
        catalog: output_catalog,
        manifest: next_manifest,
        files: planned_files,
        report,
    })
}

pub(super) fn classify_existing(asset: &StrictAudioAsset) -> Result<Route> {
    if let Some(rest) = asset.logical_key.strip_prefix("voice/nanos/nano_") {
        let owner = rest
            .split('/')
            .next()
            .filter(|owner| !owner.is_empty())
            .ok_or_else(|| invalid_error("invalid migrated Nano voice key"))?;
        return Ok(Route::NanoVoice {
            canonical_owner: owner.to_owned(),
        });
    }
    if let Some(rest) = asset.logical_key.strip_prefix("sfx/nano_skills/") {
        let mut parts = rest.split('/');
        let power = parts.next().unwrap_or_default();
        let skill_owner = parts.next().unwrap_or_default();
        if power.is_empty() || skill_owner.is_empty() {
            return invalid(format!(
                "invalid migrated Nano skill key {:?}",
                asset.logical_key
            ));
        }
        return Ok(Route::NanoSkill {
            power: power.to_owned(),
            skill_owner: skill_owner.to_owned(),
        });
    }
    if let Some(rest) = asset.logical_key.strip_prefix("voice/npcs/computress/") {
        let section = [
            "skills/skill2",
            "skills/skill3",
            "nano_events",
            "summon",
            "dialogue",
        ]
        .into_iter()
        .find(|section| {
            rest.strip_prefix(section)
                .is_some_and(|tail| tail.starts_with('/'))
        })
        .ok_or_else(|| {
            invalid_error(format!(
                "invalid migrated Computress section in {:?}",
                asset.logical_key
            ))
        })?;
        return Ok(Route::Computress {
            section: section.to_owned(),
        });
    }

    if PROVEN_COMPUTRESS_DIALOGUE_SFX
        .iter()
        .any(|(logical_key, true_name, blake3)| {
            asset.logical_key == *logical_key
                && asset.true_name == *true_name
                && asset.category == SemanticAudioCategory::Sfx
                && asset.files.len() == 1
                && asset.files[0].blake3 == *blake3
        })
    {
        return Ok(Route::Computress {
            section: "dialogue".to_owned(),
        });
    }

    if asset.category == SemanticAudioCategory::Voice {
        if let Some(owner) = canonical_current_nano_owner(&asset.owner) {
            let true_name_owner = nano_owner_from_true_name(&asset.true_name);
            if true_name_owner.as_deref() == Some(owner) {
                return Ok(Route::NanoVoice {
                    canonical_owner: owner.to_owned(),
                });
            }
        }
        if PROVEN_NANO_SKILL_OWNERS.contains(&asset.owner.as_str()) {
            if asset.files.len() != 1 || asset.files[0].locale.as_deref() != Some(DEFAULT_LOCALE) {
                return invalid(format!(
                    "proven Nano skill must have exactly one English source file: {:?}",
                    asset.logical_key
                ));
            }
            let (power, skill_owner) = nano_skill_route_from_owner(&asset.owner)?;
            return Ok(Route::NanoSkill { power, skill_owner });
        }
        if matches!(
            asset.owner.as_str(),
            "computer" | "computress" | "computress_skill2" | "computress_skill3"
        ) {
            return Ok(Route::Computress {
                section: computress_section(asset),
            });
        }
    }
    Ok(Route::Preserve)
}

pub(super) fn canonical_current_nano_owner(owner: &str) -> Option<&'static str> {
    Some(match owner {
        "aku" => "aku",
        "billy" => "billy",
        "bloo" => "bloo",
        "blossom" => "blossom",
        "btrcup" | "buttercup" => "buttercup",
        "bubbles" => "bubbles",
        "cheese" => "cheese",
        "coco" => "coco",
        "courage" => "courage",
        "deedee" => "deedee",
        "demongo" => "demongo",
        "dexter" => "dexter",
        "ed" => "ed",
        "edd" => "edd",
        "eddy" => "eddy",
        "eduardo" => "eduardo",
        "finn" => "finn",
        "fourarms" => "4arms",
        "grim" => "grim",
        "hex" => "hex",
        "him" => "him",
        "humongo" | "humongosaur" => "humongousaur",
        "jbravo" | "johnnybravo" => "johnnybravo",
        "juniper" | "juniper_lee" => "juniper_lee",
        "mac" => "mac",
        "mandark" => "mandark",
        "mandy" => "mandy",
        "megas" => "megas",
        "mojo" | "mojo_jojo" | "mojojojo" => "mojojo",
        "numfive" | "numbuh_five" | "numbuhfive" => "numbuhfive",
        "numfour" | "numbuh_four" | "numbuhfour" => "numbuhfour",
        "numone" | "numbuh_one" | "numbuhone" => "numbuhone",
        "numthree" | "numbuh_three" | "numbuhthree" => "numbuhthree",
        "numtwo" | "numbuh_two" | "numbuhtwo" => "numbuhtwo",
        "sjack" | "samurai_jack" | "samuraijack" => "samuraijack",
        "swampfire" => "swampfire",
        "utonium" => "utonium",
        "vilgax" => "vilgax",
        "wilt" => "wilt",
        _ => return None,
    })
}

pub(super) fn nano_owner_from_true_name(true_name: &str) -> Option<String> {
    let compact = alphanumeric_identity(true_name);
    NANO_IDENTITY_ALIASES.iter().find_map(|(prefix, owner)| {
        let remainder = compact.strip_prefix(prefix)?;
        (remainder.starts_with("nan")
            || remainder.starts_with("summon")
            || remainder.starts_with("pwr"))
        .then(|| (*owner).to_owned())
    })
}

pub(super) fn canonical_skill_owner(owner: &str) -> &str {
    match owner {
        "fourarms" => "4arms",
        "humongosaur" | "humongo" => "humongousaur",
        "juniper" | "juniperlee" => "juniper_lee",
        "mojojojo" => "mojojo",
        "numbuh1" => "numbuhone",
        "numbuh2" => "numbuhtwo",
        "numbuh4" => "numbuhfour",
        "numbuh5" => "numbuhfive",
        other => other,
    }
}

pub(super) fn computress_section(asset: &StrictAudioAsset) -> String {
    match asset.owner.as_str() {
        "computer" => "summon".to_owned(),
        "computress_skill2" => "skills/skill2".to_owned(),
        "computress_skill3" => "skills/skill3".to_owned(),
        _ if asset
            .true_name
            .to_ascii_lowercase()
            .starts_with("computress_nan") =>
        {
            "nano_events".to_owned()
        }
        _ => "dialogue".to_owned(),
    }
}

pub(super) fn make_recovery_identity_unique(
    asset: &mut StrictAudioAsset,
    hash: &str,
    existing_keys: &BTreeSet<String>,
    existing_paths: &BTreeSet<String>,
) -> Result<()> {
    let key_free = !existing_keys.contains(&folded(&asset.logical_key));
    let path_free = asset
        .files
        .first()
        .is_some_and(|file| !existing_paths.contains(&folded(&file.path)));
    if key_free && path_free {
        return Ok(());
    }
    let suffix = hash
        .get(..12)
        .ok_or_else(|| invalid_error("recovery hash is too short"))?;
    asset.logical_key.push_str(&format!("_take_{suffix}"));
    let file = asset
        .files
        .first_mut()
        .ok_or_else(|| invalid_error("recovery asset has no file"))?;
    let stem = file
        .path
        .strip_suffix(".ogg")
        .ok_or_else(|| invalid_error("recovery path has no .ogg suffix"))?;
    file.path = format!("{stem}_take_{suffix}.ogg");
    if existing_keys.contains(&folded(&asset.logical_key))
        || existing_paths.contains(&folded(&file.path))
    {
        return invalid(format!(
            "recovery take identity still collides for {:?}",
            asset.true_name
        ));
    }
    Ok(())
}

pub(super) fn logical_line(logical_key: &str, true_name: &str) -> Result<String> {
    logical_key
        .rsplit('/')
        .next()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .map(Ok)
        .unwrap_or_else(|| portable_component(true_name, "audio line"))
}

pub(super) fn register_keys(asset: &StrictAudioAsset, keys: &mut BTreeSet<String>) -> Result<()> {
    for key in std::iter::once(&asset.logical_key).chain(&asset.aliases) {
        if key.trim().is_empty() || !keys.insert(folded(key)) {
            return invalid(format!("audio logical key/alias collision at {key:?}"));
        }
    }
    Ok(())
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

pub(super) fn normalize_recovery_scope(asset: &mut StrictAudioAsset) -> bool {
    if asset.scope.as_deref() != Some("retrobution_nano_recovery") {
        return false;
    }
    asset.scope = Some("nano_recovery".to_owned());
    true
}

pub(super) fn stage_plan(stage: &Path, plan: &MigrationPlan) -> Result<()> {
    for file in &plan.files {
        let target = stage.join(native_path(&file.target_path));
        link_or_copy_and_verify(file, &target)?;
    }
    let catalog_path = stage.join(native_path(STRICT_AUDIO_CATALOG_PATH));
    let mut catalog_bytes =
        serde_json::to_vec_pretty(&plan.catalog).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    catalog_bytes.push(b'\n');
    write_new(&catalog_path, &catalog_bytes)?;
    let manifest_path = stage.join(ASSET_MANIFEST_FILE);
    let mut manifest_bytes =
        serde_json::to_vec_pretty(&plan.manifest).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    manifest_bytes.push(b'\n');
    write_new(&manifest_path, &manifest_bytes)?;

    for file in &plan.files {
        let target = stage.join(native_path(&file.target_path));
        validate_identity(&target, file.bytes, &file.blake3)?;
    }
    let catalog_entry = plan
        .manifest
        .files
        .iter()
        .find(|entry| entry.path == STRICT_AUDIO_CATALOG_PATH)
        .ok_or_else(|| invalid_error("staged manifest lost audio catalog"))?;
    validate_identity(&catalog_path, catalog_entry.bytes, &catalog_entry.blake3)?;
    Ok(())
}

pub(super) fn commit_plan(
    asset_root: &Path,
    stage: &Path,
    backup: &Path,
    files: &[PlannedFile],
) -> Result<()> {
    let mut audio_moves = Vec::new();
    for file in files {
        let target = asset_root.join(native_path(&file.target_path));
        if file.source_absolute == target {
            continue;
        }
        if target.exists() {
            return invalid(format!(
                "incremental audio target already exists: {}",
                target.display()
            ));
        }
        let staged = stage.join(native_path(&file.target_path));
        if !staged.is_file() {
            return invalid(format!(
                "incremental staged audio target is missing: {}",
                staged.display()
            ));
        }
        let source_relative = file.source_absolute.strip_prefix(asset_root).ok();
        let source_backup = match source_relative {
            Some(relative) if relative.starts_with("audio") => {
                Some(backup.join("moved-audio").join(relative))
            }
            Some(_) => {
                return invalid(format!(
                    "incremental audio source is inside the asset root but outside audio: {}",
                    file.source_absolute.display()
                ));
            }
            None => None,
        };
        audio_moves.push(AudioMoveTarget {
            source: source_backup.as_ref().map(|_| file.source_absolute.clone()),
            target,
            staged,
            backup: source_backup,
        });
    }

    fs::create_dir(backup).map_err(|source| io_at(backup, source))?;
    let metadata_targets = [
        TransactionTarget {
            target: asset_root.join(native_path(STRICT_AUDIO_CATALOG_PATH)),
            staged: stage.join(native_path(STRICT_AUDIO_CATALOG_PATH)),
            backup: backup.join("runtime-audio.json"),
        },
        TransactionTarget {
            target: asset_root.join(ASSET_MANIFEST_FILE),
            staged: stage.join(ASSET_MANIFEST_FILE),
            backup: backup.join(ASSET_MANIFEST_FILE),
        },
    ];

    let mut backed_up_audio = Vec::new();
    for (index, item) in audio_moves.iter().enumerate() {
        let (Some(source), Some(backup_path)) = (&item.source, &item.backup) else {
            continue;
        };
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        }
        if let Err(error) = fs::rename(source, backup_path) {
            rollback_incremental(
                &audio_moves,
                &[],
                &backed_up_audio,
                &metadata_targets,
                &[],
                &[],
            );
            return Err(io_at(source, error));
        }
        backed_up_audio.push(index);
    }

    let mut backed_up_metadata = Vec::new();
    for (index, item) in metadata_targets.iter().enumerate() {
        if item.target.exists() {
            if let Err(source) = fs::rename(&item.target, &item.backup) {
                rollback_incremental(
                    &audio_moves,
                    &[],
                    &backed_up_audio,
                    &metadata_targets,
                    &[],
                    &backed_up_metadata,
                );
                return Err(io_at(&item.target, source));
            }
            backed_up_metadata.push(index);
        }
    }

    let mut installed_audio = Vec::new();
    for (index, item) in audio_moves.iter().enumerate() {
        if let Some(parent) = item.target.parent()
            && let Err(source) = fs::create_dir_all(parent)
        {
            rollback_incremental(
                &audio_moves,
                &installed_audio,
                &backed_up_audio,
                &metadata_targets,
                &[],
                &backed_up_metadata,
            );
            return Err(io_at(parent, source));
        }
        if let Err(source) = fs::rename(&item.staged, &item.target) {
            rollback_incremental(
                &audio_moves,
                &installed_audio,
                &backed_up_audio,
                &metadata_targets,
                &[],
                &backed_up_metadata,
            );
            return Err(io_at(&item.target, source));
        }
        installed_audio.push(index);
    }

    let mut installed_metadata = Vec::new();
    for (index, item) in metadata_targets.iter().enumerate() {
        if let Some(parent) = item.target.parent()
            && let Err(source) = fs::create_dir_all(parent)
        {
            rollback_incremental(
                &audio_moves,
                &installed_audio,
                &backed_up_audio,
                &metadata_targets,
                &installed_metadata,
                &backed_up_metadata,
            );
            return Err(io_at(parent, source));
        }
        if let Err(source) = fs::rename(&item.staged, &item.target) {
            rollback_incremental(
                &audio_moves,
                &installed_audio,
                &backed_up_audio,
                &metadata_targets,
                &installed_metadata,
                &backed_up_metadata,
            );
            return Err(io_at(&item.target, source));
        }
        installed_metadata.push(index);
    }
    fs::remove_dir_all(backup).map_err(|source| io_at(backup, source))?;
    fs::remove_dir_all(stage).map_err(|source| io_at(stage, source))
}
