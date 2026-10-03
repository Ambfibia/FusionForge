use super::*;

pub const CHARACTER_CREATION_INSTALL_SCHEMA: &str = "ffone.character-creation-data-install.v1";

pub(super) static INSTALL_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterCreationDataInstallOptions {
    pub table_set_path: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
}

impl CharacterCreationDataInstallOptions {
    pub fn new(
        table_set_path: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            table_set_path: table_set_path.into(),
            asset_root: asset_root.into(),
            source_build: source_build.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationDataInstallReport {
    pub schema: String,
    pub status: String,
    pub destination: String,
    pub manifest_files: u64,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub first_names: u64,
    pub middle_names: u64,
    pub last_names: u64,
    pub creation_rows: u64,
    pub avatar_items: u64,
    pub avatar_lookup_complete: bool,
}

pub fn install_character_creation_data(
    options: &CharacterCreationDataInstallOptions,
) -> Result<CharacterCreationDataInstallReport> {
    if options.source_build.trim().is_empty() {
        return invalid("source build must not be empty");
    }
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    let table_set_path = canonical_file(&options.table_set_path, "native table set")?;
    if table_set_path.starts_with(asset_root.join(CHARACTER_CREATION_ROOT)) {
        return invalid("source table set cannot be inside the character-creation destination");
    }

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
    if manifest.files.iter().any(|entry| {
        entry.path == CHARACTER_CREATION_ROOT
            || entry
                .path
                .starts_with(&format!("{CHARACTER_CREATION_ROOT}/"))
    }) {
        return invalid("project manifest already owns a character-creation data tree");
    }

    let manifest_index = index_manifest(&manifest.files)?;
    let table_set_relative = project_relative(&asset_root, &table_set_path)?;
    let table_set_entry = manifest_index.get(&table_set_relative).ok_or_else(|| {
        invalid_error(format!(
            "native source table set {table_set_relative:?} is not manifest-listed"
        ))
    })?;
    let table_set_bytes = read_verified(&asset_root, table_set_entry)?;
    let table_set_json: Value =
        serde_json::from_slice(&table_set_bytes).map_err(|source| PipelineError::Json {
            path: table_set_path.display().to_string(),
            source,
        })?;
    let tables = select_table_root(&table_set_json)?;

    let equipment_entry = manifest_index
        .get(PLAYER_EQUIPMENT_CATALOG_PATH)
        .ok_or_else(|| invalid_error("player-equipment catalog is not manifest-listed"))?;
    let equipment_bytes = read_verified(&asset_root, equipment_entry)?;
    let equipment_catalog: PlayerEquipmentCatalog = serde_json::from_slice(&equipment_bytes)
        .map_err(|source| PipelineError::Json {
            path: asset_root
                .join(PLAYER_EQUIPMENT_CATALOG_PATH)
                .display()
                .to_string(),
            source,
        })?;
    validate_equipment_catalog(&asset_root, &manifest_index, &equipment_catalog)?;

    let provenance = CharacterCreationProvenance {
        source_build: options.source_build.clone(),
        table_set: native_reference(table_set_entry),
        player_equipment_catalog: native_reference(equipment_entry),
    };
    let documents = build_documents(
        &asset_root,
        tables,
        provenance,
        &manifest.files,
        &manifest_index,
        &equipment_catalog.models,
    )?;
    validate_documents(&documents)?;

    let name_bytes = pretty_json(&documents.name_wheel, CHARACTER_CREATION_NAME_WHEEL_PATH)?;
    let appearance_bytes = pretty_json(&documents.appearance, CHARACTER_CREATION_APPEARANCE_PATH)?;
    let items_bytes = pretty_json(
        &documents.avatar_items,
        CHARACTER_CREATION_AVATAR_ITEMS_PATH,
    )?;
    let runtime_texture_bytes = pretty_json(
        &documents.runtime_textures,
        CHARACTER_CREATION_RUNTIME_TEXTURES_PATH,
    )?;

    let destination = asset_root.join(CHARACTER_CREATION_ROOT);
    if fs::symlink_metadata(&destination).is_ok() {
        return invalid("character-creation data destination already exists");
    }
    let stage = create_stage(&asset_root)?;
    let write_result = (|| -> Result<Vec<ProjectAssetFile>> {
        let files = [
            ("name_wheel.json", name_bytes.as_slice()),
            ("appearance.json", appearance_bytes.as_slice()),
            ("avatar_items.json", items_bytes.as_slice()),
            ("runtime_textures.json", runtime_texture_bytes.as_slice()),
        ];
        let existing = manifest
            .files
            .iter()
            .map(|entry| entry.path.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        let mut planned = BTreeSet::new();
        let mut entries = Vec::new();
        for (name, bytes) in files {
            let relative = format!("{CHARACTER_CREATION_ROOT}/{name}");
            reserve_path(&existing, &mut planned, &relative)?;
            write_new_file(&stage, name, bytes)?;
            entries.push(ProjectAssetFile {
                source_path: format!(
                    "native-character-creation/{}/{name}",
                    portable_component(&options.source_build)
                ),
                path: relative,
                kind: ProjectAssetKind::Data,
                bytes: bytes.len() as u64,
                blake3: blake3::hash(bytes).to_hex().to_string(),
            });
        }
        Ok(entries)
    })();
    let new_entries = match write_result {
        Ok(entries) => entries,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };
    fs::rename(&stage, &destination).map_err(|error| {
        let _ = fs::remove_dir_all(&stage);
        io_at(&destination, error)
    })?;

    manifest.files.extend(new_entries.iter().cloned());
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    if let Err(error) = replace_manifest(&asset_root, &manifest) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }

    Ok(CharacterCreationDataInstallReport {
        schema: CHARACTER_CREATION_INSTALL_SCHEMA.to_owned(),
        status: "installed-native-0104-character-creation-data".to_owned(),
        destination: normalize_path(&destination),
        manifest_files: manifest.files.len() as u64,
        installed_files: new_entries.len() as u64,
        installed_bytes: new_entries.iter().map(|entry| entry.bytes).sum(),
        first_names: documents.name_wheel.first_names.len() as u64,
        middle_names: documents.name_wheel.middle_names.len() as u64,
        last_names: documents.name_wheel.last_names.len() as u64,
        creation_rows: documents.appearance.creation_rows.len() as u64,
        avatar_items: documents.avatar_items.items.len() as u64,
        avatar_lookup_complete: documents.avatar_items.lookup_complete,
    })
}

pub(super) fn write_new_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    validate_relative(relative)?;
    let path = root.join(relative);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("staged file has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| io_at(&path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(&path, error))?;
    output.sync_all().map_err(|error| io_at(&path, error))
}
