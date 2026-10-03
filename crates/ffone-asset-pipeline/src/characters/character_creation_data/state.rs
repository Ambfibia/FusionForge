use super::*;

pub const CHARACTER_CREATION_RUNTIME_TEXTURES_SCHEMA: &str =
    "ffone.character-creation.runtime-textures.v2";

pub(super) const EXPECTED_CREATOR_RUNTIME_TEXTURES: usize = 214;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationRuntimeTextures {
    pub schema: String,
    pub protocol: u16,
    pub provenance: CharacterCreationProvenance,
    pub coverage: CharacterRuntimeTextureCoverage,
    pub textures: Vec<CharacterRuntimeTextureContract>,
}

/// Rebuilds and registers the readable creator runtime-texture closure from the
/// checked-in offline Texture2D evidence. Unity archives are never opened by
/// this publisher or by the game runtime.
pub fn register_character_creation_runtime_textures(
    asset_root: impl AsRef<Path>,
) -> Result<ProjectAssetFile> {
    let asset_root = canonical_directory(asset_root.as_ref(), "asset root")?;
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA || manifest.protocol != PROTOCOL_0104 {
        return invalid("asset manifest is not the native 0104 project manifest");
    }
    let manifest_index = index_manifest(&manifest.files)?;
    let appearance_entry = manifest_index
        .get(CHARACTER_CREATION_APPEARANCE_PATH)
        .ok_or_else(|| invalid_error("character appearance document is not manifest-listed"))?;
    let avatar_items_entry = manifest_index
        .get(CHARACTER_CREATION_AVATAR_ITEMS_PATH)
        .ok_or_else(|| invalid_error("avatar item document is not manifest-listed"))?;
    let appearance_bytes = read_verified(&asset_root, appearance_entry)?;
    let avatar_items_bytes = read_verified(&asset_root, avatar_items_entry)?;
    let appearance: CharacterCreationAppearance = serde_json::from_slice(&appearance_bytes)
        .map_err(|source| PipelineError::Json {
            path: CHARACTER_CREATION_APPEARANCE_PATH.to_owned(),
            source,
        })?;
    let mut avatar_items: CharacterCreationAvatarItems =
        serde_json::from_slice(&avatar_items_bytes).map_err(|source| PipelineError::Json {
            path: CHARACTER_CREATION_AVATAR_ITEMS_PATH.to_owned(),
            source,
        })?;
    if appearance.provenance != avatar_items.provenance {
        return invalid("appearance and avatar item provenance contradict each other");
    }
    let textures_by_name = texture_index(&manifest.files);
    repair_exact_creator_texture_aliases(&mut avatar_items, &textures_by_name)?;
    repair_exact_equipment_texture_routes(&mut avatar_items.items)?;
    avatar_items.counts = avatar_item_counts(&avatar_items.items);
    avatar_items.lookup_complete = avatar_items.counts.model_references
        == avatar_items.counts.resolved_models
        && avatar_items.counts.texture_references == avatar_items.counts.resolved_textures
        && avatar_items.counts.icon_references == avatar_items.counts.resolved_icons;
    let repaired_avatar_items_bytes =
        pretty_json(&avatar_items, CHARACTER_CREATION_AVATAR_ITEMS_PATH)?;
    let repaired_avatar_items_entry = ProjectAssetFile {
        source_path: avatar_items_entry.source_path.clone(),
        path: CHARACTER_CREATION_AVATAR_ITEMS_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: repaired_avatar_items_bytes.len() as u64,
        blake3: blake3::hash(&repaired_avatar_items_bytes)
            .to_hex()
            .to_string(),
    };
    let document = build_runtime_texture_contracts(
        &asset_root,
        appearance.provenance.clone(),
        &manifest_index,
        &textures_by_name,
        &appearance,
        &avatar_items,
    )?;
    let contract_bytes = pretty_json(&document, CHARACTER_CREATION_RUNTIME_TEXTURES_PATH)?;

    for contract in &document.textures {
        let entry = manifest_index
            .get(&contract.native_asset.path)
            .ok_or_else(|| invalid_error("runtime texture native asset is not manifest-listed"))?;
        let bytes = read_verified(&asset_root, entry)?;
        if entry.kind != ProjectAssetKind::Texture
            || entry.bytes != contract.native_asset.bytes
            || entry.blake3 != contract.native_asset.blake3
            || format!("{:x}", Sha256::digest(&bytes)) != contract.native_png_sha256
        {
            return invalid(format!(
                "runtime texture {} failed native file/hash verification",
                contract.true_name
            ));
        }
    }

    let entry = ProjectAssetFile {
        source_path: "native-character-creation/retrobution-20260613/runtime_textures.json"
            .to_owned(),
        path: CHARACTER_CREATION_RUNTIME_TEXTURES_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: contract_bytes.len() as u64,
        blake3: blake3::hash(&contract_bytes).to_hex().to_string(),
    };
    let existing_contract = fs::read(asset_root.join(CHARACTER_CREATION_RUNTIME_TEXTURES_PATH))
        .map_err(|error| {
            io_at(
                &asset_root.join(CHARACTER_CREATION_RUNTIME_TEXTURES_PATH),
                error,
            )
        })?;
    let avatar_items_changed = avatar_items_bytes != repaired_avatar_items_bytes;
    let contract_changed = existing_contract != contract_bytes;
    let avatar_manifest_changed = manifest
        .files
        .iter()
        .find(|existing| existing.path == repaired_avatar_items_entry.path)
        != Some(&repaired_avatar_items_entry);
    let contract_manifest_changed = manifest
        .files
        .iter()
        .find(|existing| existing.path == entry.path)
        != Some(&entry);
    if !avatar_items_changed
        && !contract_changed
        && !avatar_manifest_changed
        && !contract_manifest_changed
    {
        return Ok(entry);
    }
    upsert_manifest_entry(&mut manifest, repaired_avatar_items_entry);
    upsert_manifest_entry(&mut manifest, entry.clone());
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    if avatar_items_changed {
        replace_character_creation_document(
            &asset_root,
            CHARACTER_CREATION_AVATAR_ITEMS_PATH,
            "avatar-items",
            &repaired_avatar_items_bytes,
        )?;
    }
    if contract_changed {
        replace_character_creation_document(
            &asset_root,
            CHARACTER_CREATION_RUNTIME_TEXTURES_PATH,
            "runtime-textures",
            &contract_bytes,
        )?;
    }
    replace_manifest(&asset_root, &manifest)?;
    Ok(entry)
}

pub(super) fn candidate_status(candidates: &[CharacterCreationAssetReference]) -> NativeLookupStatus {
    match candidates.len() {
        0 => NativeLookupStatus::Missing,
        1 => NativeLookupStatus::VerifiedUnique,
        _ => NativeLookupStatus::Ambiguous,
    }
}
