use super::*;

pub(super) fn classify_sfx_owner(
    normalized_name: &str,
    source_strings: &[(String, String)],
) -> (String, String, Vec<String>) {
    let rules: &[(&str, &[&str])] = &[
        ("ui", &["ui_", "gui", "button", "menu", "interface"]),
        (
            "combat",
            &[
                "weapon", "sword", "rifle", "bazooka", "melee", "range", "impact", "attack",
            ],
        ),
        (
            "movement",
            &["footstep", "foot_step", "jump", "land", "walk", "run"],
        ),
        (
            "environment",
            &["environment", "weather", "wind", "water", "fire", "machine"],
        ),
    ];
    for (owner, markers) in rules {
        if let Some(marker) = markers
            .iter()
            .find(|marker| normalized_name.contains(**marker))
        {
            return (
                (*owner).to_owned(),
                format!("true m_Name contains {marker:?} system marker"),
                vec![format!(
                    "normalized true m_Name {normalized_name:?} contains {marker:?}"
                )],
            );
        }
    }
    (
        "shared".to_owned(),
        "no evidence-backed SFX system owner matched; explicit shared owner".to_owned(),
        vec![format!(
            "checked true m_Name {normalized_name:?} and {} serialized source provenance strings",
            source_strings.len()
        )],
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stage_semantic_audio(
    asset_root: &Path,
    stage: &Path,
    source_build: &str,
    prepared: &[PreparedAudio],
    manifest: &ProjectAssetManifest,
    cook_report: &CookReport,
    manifest_blake3: &str,
    cook_report_blake3: &str,
) -> Result<(Vec<ProjectAssetFile>, SemanticAudioCatalog, u64)> {
    for directory in OWNED_DIRECTORIES {
        let path = stage.join(directory);
        fs::create_dir(&path).map_err(|error| io_at(&path, error))?;
    }

    let mut entries = Vec::with_capacity(prepared.len() + 1);
    let mut catalog_assets = Vec::with_capacity(prepared.len());
    let mut installed_audio_bytes = 0_u64;
    for audio in prepared {
        let source = join_manifest_path(asset_root, &audio.source.path)?;
        let bytes = fs::read(&source).map_err(|error| io_at(&source, error))?;
        verify_source_bytes(audio, &bytes)?;
        let relative = audio
            .destination
            .strip_prefix("audio/")
            .ok_or_else(|| invalid_error("prepared semantic path lost audio/ prefix"))?;
        let target = join_relative(stage, relative)?;
        write_new(&target, &bytes)?;
        installed_audio_bytes = installed_audio_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid_error("semantic audio byte count overflow"))?;
        entries.push(ProjectAssetFile {
            source_path: audio.source.source_path.clone(),
            path: audio.destination.clone(),
            kind: ProjectAssetKind::Audio,
            bytes: audio.source.bytes,
            blake3: audio.source.blake3.clone(),
        });
        catalog_assets.push(SemanticAudioAsset {
            true_name: audio.true_name.clone(),
            path: audio.destination.clone(),
            category: audio.classification.category.clone(),
            owner: audio.classification.owner.clone(),
            classification: audio.classification.proof.clone(),
            variant: audio.variant.clone(),
            source_manifest_path: audio.source.path.clone(),
            source_manifest_source_path: audio.source.source_path.clone(),
            source_bytes: audio.source.bytes,
            source_blake3: audio.source.blake3.clone(),
            native_key: audio.native_key.clone(),
            native_path: audio.native_path.clone(),
            source_provenance: audio.provenance.clone(),
        });
    }
    catalog_assets.sort_by(|left, right| left.path.cmp(&right.path));
    entries.sort_by(|left, right| left.path.cmp(&right.path));

    let counts = catalog_counts(&catalog_assets)?;
    let audio_mapping_records = cook_report
        .mappings
        .iter()
        .filter(|mapping| mapping.kind == "audio")
        .count() as u64;
    let catalog = SemanticAudioCatalog {
        schema: SEMANTIC_AUDIO_CATALOG_SCHEMA.to_owned(),
        source_build: source_build.to_owned(),
        source_manifest: SemanticAudioManifestProof {
            schema: manifest.schema.clone(),
            blake3: manifest_blake3.to_owned(),
            original_hashed_ogg_entries: prepared.len() as u64,
        },
        source_cook_report: SemanticAudioCookReportProof {
            schema: cook_report.schema.clone(),
            blake3: cook_report_blake3.to_owned(),
            build_uuid: cook_report.build_uuid.clone(),
            locale: cook_report.locale.clone(),
            declared_audio_assets: cook_report.counts.audio,
            audio_mapping_records,
            unique_native_audio_paths: prepared.len() as u64,
        },
        path_policy:
            "portable NFKC lowercase true m_Name components; no content hashes in destination paths"
                .to_owned(),
        duplicate_policy:
            "same normalized true m_Name variants sorted by canonical source provenance JSON, source blake3, nativePath"
                .to_owned(),
        classification_policy:
            "ordered explicit ambient/music/voice evidence; evidence-backed SFX systems; unmatched assets are explicitly sfx/shared with uncertain=true"
                .to_owned(),
        counts,
        assets: catalog_assets,
    };
    let mut catalog_bytes =
        serde_json::to_vec_pretty(&catalog).map_err(|source| PipelineError::Json {
            path: SEMANTIC_AUDIO_CATALOG.to_owned(),
            source,
        })?;
    catalog_bytes.push(b'\n');
    let catalog_target = stage.join("catalog.json");
    write_new(&catalog_target, &catalog_bytes)?;
    entries.push(ProjectAssetFile {
        source_path: "semantic-audio-installer/catalog".to_owned(),
        path: SEMANTIC_AUDIO_CATALOG.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: catalog_bytes.len() as u64,
        blake3: blake3::hash(&catalog_bytes).to_hex().to_string(),
    });
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    validate_staged_files(stage, &entries)?;
    Ok((entries, catalog, installed_audio_bytes))
}
