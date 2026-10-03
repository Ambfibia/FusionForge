use super::*;

pub(super) fn verify_table_set(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    table_set: &Path,
) -> Result<ProjectAssetFile> {
    let relative = manifest_relative_path(asset_root, table_set)?;
    let entry = manifest
        .files
        .iter()
        .find(|entry| entry.path == relative)
        .ok_or_else(|| invalid_error(format!("TableData {relative:?} is not manifest-listed")))?;
    if entry.kind != ProjectAssetKind::Data {
        return invalid(format!("TableData {relative:?} is not kind=data"));
    }
    let bytes = fs::read(table_set).map_err(|error| io_at(table_set, error))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if entry.bytes != bytes.len() as u64 || entry.blake3 != actual {
        return invalid(format!(
            "TableData {relative:?} does not match manifest bytes/hash"
        ));
    }
    Ok(entry.clone())
}

pub(super) fn classify_references(
    references: &BTreeMap<(u8, u32), IconReferenceGroup>,
    textures: &BTreeMap<String, Vec<ProjectAssetFile>>,
) -> Result<(
    Vec<PreparedIcon>,
    Vec<SemanticIconUnmatched>,
    BTreeSet<String>,
)> {
    let mut prepared = Vec::new();
    let mut unmatched = Vec::new();
    let mut consumed = BTreeSet::new();
    for group in references.values() {
        let Some(kind) = legacy_kind(group.icon_type) else {
            unmatched.push(SemanticIconUnmatched {
                reason: SemanticIconUnmatchedReason::UnsupportedIconType,
                legacy_true_name: format!(
                    "unsupported_type_{}_{}",
                    group.icon_type, group.icon_number
                ),
                legacy_icon_type: Some(group.icon_type),
                legacy_icon_number: Some(group.icon_number),
                candidates: Vec::new(),
                table_references: group.references.clone(),
            });
            continue;
        };
        let true_name = format!("{}_{:02}", kind.prefix, group.icon_number);
        let candidates = textures
            .get(&true_name)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for candidate in candidates {
            consumed.insert(candidate.path.clone());
        }
        if candidates.len() != 1 {
            unmatched.push(SemanticIconUnmatched {
                reason: if candidates.is_empty() {
                    SemanticIconUnmatchedReason::MissingTexture
                } else {
                    SemanticIconUnmatchedReason::AmbiguousTexture
                },
                legacy_true_name: true_name,
                legacy_icon_type: Some(group.icon_type),
                legacy_icon_number: Some(group.icon_number),
                candidates: candidates.iter().map(source_proof).collect(),
                table_references: group.references.clone(),
            });
            continue;
        }
        let source = candidates[0].clone();
        let destination = format!("icons/{}/{}.png", kind.category.directory(), true_name);
        let catalog = SemanticIconAsset {
            key: format!("{}/{}", kind.category.directory(), true_name),
            legacy_true_name: true_name,
            legacy_icon_number: group.icon_number,
            path: destination.clone(),
            source: source_proof(&source),
            classification: SemanticIconClassificationProof {
                method: "table_data_icon_type_exact_manifest_stem",
                legacy_icon_type: group.icon_type,
                legacy_icon_prefix: kind.prefix.to_owned(),
                category: kind.category,
            },
            table_references: group.references.clone(),
        };
        prepared.push(PreparedIcon {
            source,
            destination,
            catalog,
        });
    }
    Ok((prepared, unmatched, consumed))
}

pub(super) fn append_unreferenced_textures(
    manifest: &ProjectAssetManifest,
    consumed: &BTreeSet<String>,
    unmatched: &mut Vec<SemanticIconUnmatched>,
) -> Result<()> {
    for entry in &manifest.files {
        if entry.kind != ProjectAssetKind::Texture
            || !entry.path.starts_with("textures/")
            || consumed.contains(&entry.path)
        {
            continue;
        }
        let Some(stem) = texture_true_name(&entry.path) else {
            continue;
        };
        let recognized = parse_legacy_texture_name(&stem).is_some();
        if !recognized && !stem.contains("icon") {
            continue;
        }
        let (icon_type, icon_number) = parse_legacy_texture_name(&stem)
            .map(|(kind, number)| (Some(kind.icon_type), Some(number)))
            .unwrap_or((None, None));
        unmatched.push(SemanticIconUnmatched {
            reason: if recognized {
                SemanticIconUnmatchedReason::UnreferencedLegacyTexture
            } else {
                SemanticIconUnmatchedReason::NameOnlyNoTableDataProof
            },
            legacy_true_name: stem,
            legacy_icon_type: icon_type,
            legacy_icon_number: icon_number,
            candidates: vec![source_proof(entry)],
            table_references: Vec::new(),
        });
    }
    Ok(())
}

pub(super) fn legacy_kind(icon_type: u8) -> Option<&'static LegacyIconKind> {
    LEGACY_ICON_KINDS
        .iter()
        .find(|kind| kind.icon_type == icon_type)
}

pub(super) fn source_proof(entry: &ProjectAssetFile) -> SemanticIconSourceAsset {
    SemanticIconSourceAsset {
        manifest_path: entry.path.clone(),
        manifest_source_path: entry.source_path.clone(),
        bytes: entry.bytes,
        blake3: entry.blake3.clone(),
    }
}

pub(super) fn unmatched_reason_counts(entries: &[SemanticIconUnmatched]) -> BTreeMap<String, u64> {
    let mut counts = [
        "missing_texture",
        "ambiguous_texture",
        "unsupported_icon_type",
        "unreferenced_legacy_texture",
        "name_only_no_table_data_proof",
    ]
    .into_iter()
    .map(|reason| (reason.to_owned(), 0_u64))
    .collect::<BTreeMap<_, _>>();
    for entry in entries {
        let reason = match entry.reason {
            SemanticIconUnmatchedReason::MissingTexture => "missing_texture",
            SemanticIconUnmatchedReason::AmbiguousTexture => "ambiguous_texture",
            SemanticIconUnmatchedReason::UnsupportedIconType => "unsupported_icon_type",
            SemanticIconUnmatchedReason::UnreferencedLegacyTexture => "unreferenced_legacy_texture",
            SemanticIconUnmatchedReason::NameOnlyNoTableDataProof => {
                "name_only_no_table_data_proof"
            }
        };
        *counts.get_mut(reason).expect("all reasons are initialized") += 1;
    }
    counts
}

pub(super) fn reason_count(counts: &BTreeMap<String, u64>, reason: &str) -> u64 {
    counts.get(reason).copied().unwrap_or_default()
}

pub(super) fn invalid<T>(reason: impl Into<String>) -> Result<T> {
    Err(invalid_error(reason))
}
