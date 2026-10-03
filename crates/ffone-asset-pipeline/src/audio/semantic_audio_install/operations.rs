use super::*;

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn is_owned_entry(entry: &ProjectAssetFile) -> bool {
    entry.path == SEMANTIC_AUDIO_CATALOG
        || OWNED_MANIFEST_PREFIXES
            .iter()
            .any(|prefix| entry.path.starts_with(prefix))
}

pub(super) fn is_original_hashed_ogg(entry: &ProjectAssetFile) -> bool {
    entry.kind == ProjectAssetKind::Audio
        && entry.path == entry.source_path
        && is_hashed_audio_path(&entry.path)
}

pub(super) fn classify(
    true_name: &str,
    provenance: &[BTreeMap<String, serde_json::Value>],
) -> Result<Classification> {
    let normalized_name = normalized_identity(true_name);
    let source_strings = provenance_strings(provenance);

    if let Some(classification) =
        classify_character_creation_audio(true_name, &normalized_name, provenance)
    {
        return Ok(classification);
    }

    if let Some(evidence) =
        name_or_source_evidence(true_name, &normalized_name, &source_strings, &["ambient"])
    {
        return Ok(Classification {
            category: SemanticAudioCategory::Ambient,
            owner: "shared".to_owned(),
            proof: SemanticAudioClassificationProof {
                category_rule: "ambient-explicit-marker".to_owned(),
                owner_rule: "ambient-category-has-no-owner-subdirectory".to_owned(),
                certainty: evidence.1,
                evidence: vec![evidence.0],
                reason:
                    "The exact m_Name or source provenance explicitly identifies ambient audio."
                        .to_owned(),
                uncertain: false,
            },
        });
    }

    if let Some(evidence) = name_or_source_evidence(
        true_name,
        &normalized_name,
        &source_strings,
        &["music", "theme"],
    ) {
        return Ok(Classification {
            category: SemanticAudioCategory::Music,
            owner: "shared".to_owned(),
            proof: SemanticAudioClassificationProof {
                category_rule: "music-explicit-marker".to_owned(),
                owner_rule: "music-category-has-no-owner-subdirectory".to_owned(),
                certainty: evidence.1,
                evidence: vec![evidence.0],
                reason: "The exact m_Name or source provenance explicitly identifies music."
                    .to_owned(),
                uncertain: false,
            },
        });
    }

    let voice_name_markers = [
        "_qgreeting",
        "_greeting",
        "_farewell",
        "_goodluck",
        "_nicejob",
        "_commout",
        "_firstuse",
        "_dialog",
        "_voice",
        "_nan",
        "_vc_",
    ];
    let voice_source = source_strings.iter().find(|(_, value)| {
        let value = normalized_identity(value);
        value.contains("npcvoiceshared")
            || value.contains("npc_voice")
            || value.contains("voice")
            || value.contains("nano_pack_")
            || value.ends_with("nano.resourcefile")
    });
    let voice_name_marker = voice_name_markers
        .iter()
        .find(|marker| normalized_name.contains(**marker));
    if voice_source.is_some() || voice_name_marker.is_some() {
        let (certainty, evidence, category_rule) = if let Some((field, value)) = voice_source {
            (
                ClassificationCertainty::SourceProvenance,
                format!("source.{field}={value:?} identifies a voice-bearing archive"),
                "voice-source-archive",
            )
        } else {
            let marker = *voice_name_marker.expect("checked above");
            (
                ClassificationCertainty::TrueName,
                format!("true m_Name {true_name:?} contains voice marker {marker:?}"),
                "voice-true-name-marker",
            )
        };
        let (owner, owner_rule) = voice_owner(&normalized_name, &voice_name_markers)?;
        return Ok(Classification {
            category: SemanticAudioCategory::Voice,
            owner,
            proof: SemanticAudioClassificationProof {
                category_rule: category_rule.to_owned(),
                owner_rule,
                certainty,
                evidence: vec![evidence],
                reason:
                    "Voice classification is backed by an explicit voice archive or dialogue marker."
                        .to_owned(),
                uncertain: false,
            },
        });
    }

    let (owner, owner_rule, evidence) = classify_sfx_owner(&normalized_name, &source_strings);
    let uncertain = owner == "shared";
    Ok(Classification {
        category: SemanticAudioCategory::Sfx,
        owner,
        proof: SemanticAudioClassificationProof {
            category_rule: if uncertain {
                "explicit-unclassified-sfx-fallback".to_owned()
            } else {
                "sfx-system-marker".to_owned()
            },
            owner_rule,
            certainty: if uncertain {
                ClassificationCertainty::ExplicitSharedFallback
            } else {
                ClassificationCertainty::TrueName
            },
            evidence,
            reason: if uncertain {
                "No authoritative music, ambient, voice, or SFX-system rule matched; ownership is retained explicitly as shared instead of being guessed."
                    .to_owned()
            } else {
                "The true m_Name contains an explicit SFX system marker.".to_owned()
            },
            uncertain,
        },
    })
}

pub(super) fn name_or_source_evidence(
    true_name: &str,
    normalized_name: &str,
    source_strings: &[(String, String)],
    markers: &[&str],
) -> Option<(String, ClassificationCertainty)> {
    if let Some(marker) = markers
        .iter()
        .find(|marker| normalized_name.contains(**marker))
    {
        return Some((
            format!("true m_Name {true_name:?} contains explicit marker {marker:?}"),
            ClassificationCertainty::TrueName,
        ));
    }
    source_strings.iter().find_map(|(field, value)| {
        let normalized = normalized_identity(value);
        markers
            .iter()
            .find(|marker| normalized.contains(**marker))
            .map(|marker| {
                (
                    format!("source.{field}={value:?} contains explicit marker {marker:?}"),
                    ClassificationCertainty::SourceProvenance,
                )
            })
    })
}

pub(super) fn provenance_strings(provenance: &[BTreeMap<String, serde_json::Value>]) -> Vec<(String, String)> {
    fn collect(prefix: &str, value: &serde_json::Value, output: &mut Vec<(String, String)>) {
        match value {
            serde_json::Value::String(value) => {
                output.push((prefix.to_owned(), value.clone()));
            }
            serde_json::Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    collect(&format!("{prefix}[{index}]"), value, output);
                }
            }
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    collect(&format!("{prefix}.{key}"), value, output);
                }
            }
            _ => {}
        }
    }

    let mut output = Vec::new();
    for (index, source) in provenance.iter().enumerate() {
        for (key, value) in source {
            collect(&format!("{index}.{key}"), value, &mut output);
        }
    }
    output.sort();
    output
}

pub(super) fn assign_destinations(prepared: &mut [PreparedAudio]) -> Result<()> {
    let mut name_groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, audio) in prepared.iter().enumerate() {
        name_groups
            .entry(normalized_identity(&audio.true_name))
            .or_default()
            .push(index);
    }
    for (normalized_name, indices) in name_groups {
        if indices.len() <= 1 {
            continue;
        }
        let mut ordered = indices;
        ordered.sort_by(|left, right| {
            variant_order_key(&prepared[*left]).cmp(&variant_order_key(&prepared[*right]))
        });
        let total = ordered.len() as u32;
        for (offset, index) in ordered.into_iter().enumerate() {
            prepared[index].variant = Some(SemanticAudioVariant {
                normalized_true_name: normalized_name.clone(),
                index: offset as u32 + 1,
                total,
                ordering:
                    "ascending canonical source provenance JSON, then source blake3, then nativePath"
                        .to_owned(),
            });
        }
    }

    let mut destinations = BTreeMap::<String, String>::new();
    for audio in prepared {
        let file_name = portable_component(&audio.true_name, "true m_Name")?;
        let mut path = format!("audio/{}/", audio.classification.category.directory());
        if matches!(
            audio.classification.category,
            SemanticAudioCategory::Voice | SemanticAudioCategory::Sfx
        ) {
            path.push_str(&portable_component(
                &audio.classification.owner,
                "semantic audio owner",
            )?);
            path.push('/');
        }
        if let Some(variant) = &audio.variant {
            path.push_str(&format!("variants/variant_{:02}/", variant.index));
        }
        path.push_str(&file_name);
        path.push_str(".ogg");
        validate_relative_semantic_path(&path)?;
        if contains_hash_suffix(&path) {
            return invalid(format!(
                "semantic destination unexpectedly contains a hash suffix: {path:?}"
            ));
        }
        let identity = canonical_path_identity(&path);
        if let Some(first) = destinations.insert(identity, audio.native_path.clone()) {
            return invalid(format!(
                "semantic destination collision for {path:?}: {first:?} and {:?}",
                audio.native_path
            ));
        }
        audio.destination = path;
    }
    Ok(())
}

pub(super) fn normalized_identity(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).collect()
}

pub(super) fn portable_component(value: &str, context: &str) -> Result<String> {
    let normalized = value.nfkc().collect::<String>();
    let mut output = String::new();
    let mut separator_pending = false;
    for character in normalized.chars() {
        if character.is_alphanumeric() {
            if separator_pending && !output.is_empty() {
                output.push('_');
            }
            separator_pending = false;
            output.extend(character.to_lowercase());
        } else {
            separator_pending = true;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() || output.len() > 180 {
        return invalid(format!(
            "{context} cannot be represented as a non-empty portable component without truncation"
        ));
    }
    let windows_stem = output
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(
        windows_stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        output.insert_str(0, "audio_");
    }
    Ok(output)
}

pub(super) fn variant_order_key(audio: &PreparedAudio) -> (String, String, String) {
    let provenance = audio
        .provenance
        .iter()
        .map(canonical_provenance)
        .collect::<Vec<_>>()
        .join("\n");
    (
        provenance,
        audio.source.blake3.clone(),
        audio.native_path.clone(),
    )
}

pub(super) fn canonical_provenance(source: &BTreeMap<String, serde_json::Value>) -> String {
    serde_json::to_string(source).expect("BTreeMap JSON serialization cannot fail")
}

pub(super) fn contains_hash_suffix(path: &str) -> bool {
    let Some(stem) = path.strip_suffix(".ogg") else {
        return false;
    };
    let Some((_, suffix)) = stem.rsplit_once("--") else {
        return false;
    };
    suffix.len() == 16
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn join_relative(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe relative asset path {relative:?}"));
    }
    Ok(root.join(relative_path))
}

pub(super) fn verify_source_bytes(audio: &PreparedAudio, bytes: &[u8]) -> Result<()> {
    let actual = blake3::hash(bytes).to_hex().to_string();
    if audio.source.bytes != bytes.len() as u64 || audio.source.blake3 != actual {
        return invalid(format!(
            "source byte identity mismatch for {:?}: manifest bytes={} blake3={}, actual bytes={} blake3={actual}",
            audio.source.path,
            audio.source.bytes,
            audio.source.blake3,
            bytes.len()
        ));
    }
    if !bytes.starts_with(b"OggS") {
        return invalid(format!(
            "source audio {:?} is not an Ogg bitstream",
            audio.source.path
        ));
    }
    Ok(())
}

pub(super) fn commit_transaction(
    audio_root: &Path,
    stage: &Path,
    backup: &Path,
    manifest_path: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<()> {
    fs::create_dir(backup).map_err(|error| io_at(backup, error))?;
    let targets = ["music", "ambient", "voice", "sfx", "catalog.json"];
    let mut backed_up = Vec::new();
    for target in targets {
        let current = audio_root.join(target);
        if current.exists() {
            let saved = backup.join(target);
            fs::rename(&current, &saved).map_err(|error| {
                rollback_components(audio_root, stage, backup, &[], &backed_up);
                io_at(&current, error)
            })?;
            backed_up.push(target.to_owned());
        }
    }

    let mut installed = Vec::new();
    for target in targets {
        let source = stage.join(target);
        let destination = audio_root.join(target);
        if let Err(error) = fs::rename(&source, &destination) {
            rollback_components(audio_root, stage, backup, &installed, &backed_up);
            return Err(io_at(&destination, error));
        }
        installed.push(target.to_owned());
    }

    if let Err(error) = replace_manifest(manifest_path, manifest) {
        rollback_components(audio_root, stage, backup, &installed, &backed_up);
        return Err(error);
    }

    fs::remove_dir_all(backup).map_err(|error| io_at(backup, error))?;
    fs::remove_dir(stage).map_err(|error| io_at(stage, error))
}

pub(super) fn rollback_components(
    audio_root: &Path,
    stage: &Path,
    backup: &Path,
    installed: &[String],
    backed_up: &[String],
) {
    for target in installed.iter().rev() {
        let destination = audio_root.join(target);
        let source = stage.join(target);
        let _ = fs::rename(&destination, &source);
    }
    for target in backed_up.iter().rev() {
        let saved = backup.join(target);
        let destination = audio_root.join(target);
        let _ = fs::rename(&saved, &destination);
    }
}

pub(super) fn invalid<T>(reason: impl Into<String>) -> Result<T> {
    Err(invalid_error(reason))
}
