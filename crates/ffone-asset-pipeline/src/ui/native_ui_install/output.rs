use super::*;

#[derive(Clone, Debug)]
pub struct NativeGameplayUiInstallOptions {
    pub asset_root: PathBuf,
    pub source_build: String,
}

impl NativeGameplayUiInstallOptions {
    pub fn new(asset_root: impl Into<PathBuf>, source_build: impl Into<String>) -> Self {
        Self {
            asset_root: asset_root.into(),
            source_build: source_build.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGameplayUiInstallReport {
    pub source_build: String,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub manifest_files: u64,
    pub root: String,
}

pub fn install_native_gameplay_ui(
    options: &NativeGameplayUiInstallOptions,
) -> Result<NativeGameplayUiInstallReport> {
    install_native_gameplay_ui_with_failpoint(options, NativeGameplayUiInstallFailpoint::None)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeGameplayUiInstallFailpoint {
    None,
    CreateStage,
    CreateUiParent,
}

pub(super) fn install_native_gameplay_ui_with_failpoint(
    options: &NativeGameplayUiInstallOptions,
    failpoint: NativeGameplayUiInstallFailpoint,
) -> Result<NativeGameplayUiInstallReport> {
    let asset_root =
        fs::canonicalize(&options.asset_root).map_err(|error| io_at(&options.asset_root, error))?;
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;

    let archived_catalog = load_archived_catalog(&asset_root)?;
    validate_archived_route_model(&archived_catalog)?;
    let destination = asset_root.join("ui").join("gameplay");
    let ui_parent = destination
        .parent()
        .ok_or_else(|| invalid_error("gameplay UI destination has no parent"))?;
    let stage = asset_root.join(format!(".gameplay-ui-stage-{}", std::process::id()));
    if stage.exists() {
        return invalid(format!(
            "stale gameplay UI stage exists at {}",
            stage.display()
        ));
    }
    let previous = prepare_previous_install(&destination, &archived_catalog, &mut manifest)?;
    if failpoint == NativeGameplayUiInstallFailpoint::CreateStage {
        restore_previous_install(&destination, previous.as_deref());
        return invalid("injected gameplay UI stage creation failure");
    }
    if let Err(error) = fs::create_dir_all(&stage) {
        restore_previous_install(&destination, previous.as_deref());
        return Err(io_at(&stage, error));
    }

    let result = stage_install(&asset_root, &stage, &manifest);
    let (mut entries, installed_bytes) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            restore_previous_install(&destination, previous.as_deref());
            return Err(error);
        }
    };

    if failpoint == NativeGameplayUiInstallFailpoint::CreateUiParent {
        let _ = fs::remove_dir_all(&stage);
        restore_previous_install(&destination, previous.as_deref());
        return invalid("injected gameplay UI parent creation failure");
    }
    if let Err(error) = fs::create_dir_all(ui_parent) {
        let _ = fs::remove_dir_all(&stage);
        restore_previous_install(&destination, previous.as_deref());
        return Err(io_at(ui_parent, error));
    }
    if let Err(error) = fs::rename(&stage, &destination) {
        restore_previous_install(&destination, previous.as_deref());
        return Err(io_at(&destination, error));
    }

    manifest.files.append(&mut entries);
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    if let Err(error) = replace_manifest(&manifest_path, &manifest) {
        let _ = fs::remove_dir_all(&destination);
        restore_previous_install(&destination, previous.as_deref());
        return Err(error);
    }
    if let Some(previous) = previous {
        fs::remove_dir_all(&previous).map_err(|error| io_at(&previous, error))?;
    }

    Ok(NativeGameplayUiInstallReport {
        source_build: options.source_build.clone(),
        installed_files: (ROUTES.len() + VERIFIED_ICON_ROUTES.len() + 2) as u64,
        installed_bytes,
        manifest_files: manifest.files.len() as u64,
        root: GAMEPLAY_UI_ROOT.to_owned(),
    })
}

pub(super) fn prepare_previous_install(
    destination: &Path,
    archived_catalog: &ArchivedCatalog,
    manifest: &mut ProjectAssetManifest,
) -> Result<Option<PathBuf>> {
    if !destination.exists() {
        return Ok(None);
    }
    verify_existing_install(destination, archived_catalog, manifest)?;
    let asset_root = destination
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid_error("gameplay UI destination has no asset root"))?;
    let backup = asset_root.join(format!(".gameplay-ui-backup-{}", std::process::id()));
    if backup.exists() {
        return invalid(format!(
            "stale gameplay UI backup exists at {}",
            backup.display()
        ));
    }
    fs::rename(destination, &backup).map_err(|error| io_at(destination, error))?;
    manifest
        .files
        .retain(|entry| !entry.path.starts_with(&format!("{GAMEPLAY_UI_ROOT}/")));
    Ok(Some(backup))
}

pub(super) fn restore_previous_install(destination: &Path, previous: Option<&Path>) {
    let Some(previous) = previous else {
        return;
    };
    let _ = fs::remove_dir_all(destination);
    let _ = fs::rename(previous, destination);
}

pub(super) fn stage_install(
    asset_root: &Path,
    stage: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<(Vec<ProjectAssetFile>, u64)> {
    let existing = manifest
        .files
        .iter()
        .map(|file| file.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut entries = Vec::with_capacity(ROUTES.len() + VERIFIED_ICON_ROUTES.len() + 2);
    let mut installed_bytes = 0_u64;

    for route in ROUTES {
        let path = format!("{GAMEPLAY_UI_ROOT}/{}", route.destination);
        if existing.contains(&path.to_ascii_lowercase()) {
            return invalid(format!("manifest already owns semantic UI path {path:?}"));
        }
        let source = asset_root.join(
            route
                .source
                .replace('/', &std::path::MAIN_SEPARATOR.to_string()),
        );
        let bytes = fs::read(&source).map_err(|error| io_at(&source, error))?;
        verify_route_source(manifest, route.source, route.kind, &bytes)?;
        let target = stage.join(
            route
                .destination
                .replace('/', &std::path::MAIN_SEPARATOR.to_string()),
        );
        write_new(&target, &bytes)?;
        installed_bytes = installed_bytes.saturating_add(bytes.len() as u64);
        entries.push(project_entry(route.source, &path, route.kind, &bytes));
    }

    for route in VERIFIED_ICON_ROUTES {
        let path = format!("{GAMEPLAY_UI_ROOT}/{}", route.destination);
        if existing.contains(&path.to_ascii_lowercase()) {
            return invalid(format!(
                "manifest already owns semantic TableData icon path {path:?}"
            ));
        }
        let source = asset_root.join(
            route
                .source
                .replace('/', &std::path::MAIN_SEPARATOR.to_string()),
        );
        let bytes = fs::read(&source).map_err(|error| io_at(&source, error))?;
        verify_table_data_icon_source(manifest, route, &bytes)?;
        let target = stage.join(
            route
                .destination
                .replace('/', &std::path::MAIN_SEPARATOR.to_string()),
        );
        write_new(&target, &bytes)?;
        installed_bytes = installed_bytes.saturating_add(bytes.len() as u64);
        entries.push(project_entry(
            route.source,
            &path,
            ProjectAssetKind::Texture,
            &bytes,
        ));
    }

    let layout_target = stage.join("layout").join("gameplay_hud.json");
    write_new(&layout_target, LAYOUT_BYTES)?;
    installed_bytes = installed_bytes.saturating_add(LAYOUT_BYTES.len() as u64);
    entries.push(project_entry(
        "crates/ffone-asset-pipeline/fixtures/ui/gameplay_hud.layout.json",
        GAMEPLAY_UI_LAYOUT,
        ProjectAssetKind::Data,
        LAYOUT_BYTES,
    ));

    let actual_gui_skin_blake3 = blake3::hash(GUI_SKIN_BYTES).to_hex().to_string();
    if actual_gui_skin_blake3 != GUI_SKIN_BLAKE3 {
        return invalid(format!(
            "embedded Retrobution GUI skin identity mismatch: blake3={actual_gui_skin_blake3}"
        ));
    }
    let gui_skin_target = stage.join("skins").join("retrobution-20260613.json");
    write_new(&gui_skin_target, GUI_SKIN_BYTES)?;
    installed_bytes = installed_bytes.saturating_add(GUI_SKIN_BYTES.len() as u64);
    entries.push(project_entry(
        GUI_SKIN_PATH,
        GUI_SKIN_PATH,
        ProjectAssetKind::Data,
        GUI_SKIN_BYTES,
    ));
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((entries, installed_bytes))
}

pub(super) fn verify_existing_install(
    destination: &Path,
    catalog: &ArchivedCatalog,
    manifest: &ProjectAssetManifest,
) -> Result<()> {
    let mut disk = BTreeMap::new();
    collect_runtime_files(destination, destination, &mut disk)?;
    let mut manifested = BTreeMap::new();
    for entry in manifest
        .files
        .iter()
        .filter(|entry| entry.path.starts_with(&format!("{GAMEPLAY_UI_ROOT}/")))
    {
        if manifested.insert(entry.path.as_str(), entry).is_some() {
            return invalid(format!(
                "duplicate manifest gameplay UI route {:?}",
                entry.path
            ));
        }
    }
    let disk_paths = disk.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let manifest_paths = manifested.keys().copied().collect::<BTreeSet<_>>();
    if disk_paths != manifest_paths {
        return invalid("runtime gameplay UI files and manifest routes differ");
    }

    let desired = desired_route_sources();
    for archived in &catalog.assets {
        if !disk.contains_key(&archived.path) {
            return invalid(format!(
                "archived gameplay UI base route {:?} is missing",
                archived.path
            ));
        }
    }
    if !disk.contains_key(GAMEPLAY_UI_LAYOUT) {
        return invalid("runtime gameplay UI layout is missing");
    }

    for (path, bytes) in &disk {
        let entry = manifested[path.as_str()];
        let actual_blake3 = blake3::hash(bytes).to_hex().to_string();
        if entry.bytes != bytes.len() as u64 || entry.blake3 != actual_blake3 {
            return invalid(format!(
                "runtime gameplay UI manifest identity mismatch for {path:?}"
            ));
        }
        if path == GAMEPLAY_UI_LAYOUT {
            if entry.source_path
                != "crates/ffone-asset-pipeline/fixtures/ui/gameplay_hud.layout.json"
                || entry.kind != ProjectAssetKind::Data
                || !is_supported_layout_revision(bytes)
            {
                return invalid("runtime gameplay UI layout differs from the immutable layout");
            }
            continue;
        }
        let Some((source, kind)) = desired.get(path) else {
            return invalid(format!("unowned runtime gameplay UI route {path:?}"));
        };
        if entry.source_path != *source || entry.kind != *kind {
            return invalid(format!(
                "runtime gameplay UI ownership mismatch for {path:?}"
            ));
        }
        verify_route_source(manifest, source, *kind, bytes)?;
    }
    Ok(())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    use std::io::Write;
    let mut file = options.open(path).map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
