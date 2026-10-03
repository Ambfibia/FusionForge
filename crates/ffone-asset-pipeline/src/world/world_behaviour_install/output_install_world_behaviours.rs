use super::*;

pub const WORLD_BEHAVIOUR_INSTALL_REPORT_SCHEMA: &str = "ffone.world-behaviour-install-report.v1";

pub(super) const EXPORT_SCHEMA: &str = "ffone.native-static-world-behaviours.v1";

pub(super) const EXPORT_STATUS: &str = "complete-exact-serialized-state";

#[derive(Clone, Debug)]
pub struct WorldBehaviourInstallOptions {
    pub export_root: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
}

impl WorldBehaviourInstallOptions {
    pub fn new(
        export_root: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            export_root: export_root.into(),
            asset_root: asset_root.into(),
            source_build: source_build.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldBehaviourInstallReport {
    pub schema: String,
    pub source_build: String,
    pub tile_count: u64,
    pub billboard_count: u64,
    pub visibility_switch_count: u64,
    pub effect_emitter_count: u64,
    pub animation_count: u64,
    pub animation_clip_count: u64,
    pub trigger_count: u64,
    pub waypoint_count: u64,
    pub trigger_volume_count: u64,
    pub rigid_body_count: u64,
    pub blocker_count: u64,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub manifest_files: u64,
    pub replaced_previous_install: bool,
    pub ownership_path: String,
    pub source_set_blake3: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code, reason = "fields are kept for strict schema validation")]
pub(super) struct BehaviourExport {
    pub(super) schema: String,
    pub(super) status: String,
    pub(super) source_build: String,
    pub(super) map_id: String,
    pub(super) tile: [i32; 2],
    pub(super) source_archive_blake3: String,
    pub(super) scripts: Vec<ExportScript>,
    #[serde(default)]
    pub(super) animation_clips: Vec<ExportAnimationClip>,
    #[serde(default)]
    pub(super) effect_prefab_closures: Vec<ExportEffectPrefabClosure>,
    pub(super) behaviours: Vec<ExportBehaviour>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportScript {
    pub(super) id: String,
    pub(super) class_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportBehaviour {
    pub(super) node: String,
    pub(super) world_matrix: [[f64; 4]; 4],
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) enabled: bool,
    #[serde(default)]
    pub(super) script: Option<String>,
    #[serde(default)]
    pub(super) resolved_default_clip: Option<String>,
    #[serde(default)]
    pub(super) resolved_clips: Vec<String>,
    #[serde(default)]
    pub(super) resolved_particle_prefabs: Vec<Option<String>>,
    pub(super) fields: JsonMap<String, JsonValue>,
}

pub fn install_world_behaviours(
    options: &WorldBehaviourInstallOptions,
) -> Result<WorldBehaviourInstallReport> {
    let export_root = canonical_directory(&options.export_root, "behaviour export root")?;
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    if export_root.starts_with(&asset_root) || asset_root.starts_with(&export_root) {
        return invalid("behaviour export root and asset root must be disjoint");
    }

    let registry = load_registry(&asset_root)?;
    let previous = load_previous_install(&asset_root)?;

    let mut documents = Vec::<(String, String, Vec<u8>, String)>::new();
    let mut tiles = Vec::<WorldBehaviourTileProof>::new();
    let mut totals = Totals::default();

    for (tile_id, scope) in &registry {
        let export_path = export_root.join(format!("{tile_id}.json"));
        let export_bytes = fs::read(&export_path).map_err(|error| io_at(&export_path, error))?;
        let export: BehaviourExport = parse_json(&export_bytes, &export_path)?;
        if export.schema != EXPORT_SCHEMA
            || export.status != EXPORT_STATUS
            || export.source_build != options.source_build
        {
            return invalid(format!(
                "{} is not a {EXPORT_SCHEMA} export of {:?}",
                export_path.display(),
                options.source_build
            ));
        }

        let hierarchy = load_hierarchy(&asset_root, tile_id, scope, &export.source_archive_blake3)?;
        let document = convert_tile(tile_id, scope, &export, &hierarchy, &mut totals)?;
        let bytes = pretty_json(&document)?;
        let relative = format!("{WORLD_BEHAVIOUR_ROOT}/{scope}/{tile_id}.json");
        tiles.push(WorldBehaviourTileProof {
            tile_id: tile_id.clone(),
            scope: scope.clone(),
            source_archive_blake3: export.source_archive_blake3.clone(),
            export_blake3: hash_bytes(&export_bytes),
            path: relative.clone(),
            bytes: bytes.len() as u64,
            blake3: hash_bytes(&bytes),
        });
        documents.push((
            relative,
            tile_id.clone(),
            bytes,
            export.source_archive_blake3.clone(),
        ));
    }

    if documents.is_empty() {
        return invalid("behaviour install found no world tiles");
    }
    tiles.sort_by(|left, right| left.path.cmp(&right.path));
    let source_set_blake3 = source_set_blake3(&tiles);

    let ownership = WorldBehaviourOwnership {
        schema: WORLD_BEHAVIOUR_OWNERSHIP_SCHEMA.to_owned(),
        installer: INSTALLER_ID.to_owned(),
        source_build: options.source_build.clone(),
        source_set_blake3: source_set_blake3.clone(),
        tiles: tiles.clone(),
    };
    let ownership_bytes = pretty_json(&serde_json::to_value(&ownership).map_err(json_error)?)?;

    // Stage the complete tree, then swap it in with one rename so a failed
    // conversion can never expose a partial behaviour set.
    let token = unique_token();
    let stage = asset_root.join(format!(".world-behaviour-stage-{token}"));
    let backup = asset_root.join(format!(".world-behaviour-backup-{token}"));
    if stage.exists() || backup.exists() {
        return invalid("world behaviour transaction path collision");
    }
    let staged = (|| -> Result<()> {
        for (relative, _, bytes, _) in &documents {
            let target = stage.join(relative);
            let parent = target
                .parent()
                .ok_or_else(|| invalid_error("staged behaviour document has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            fs::write(&target, bytes).map_err(|error| io_at(&target, error))?;
        }
        let ownership_target = stage.join(WORLD_BEHAVIOUR_OWNERSHIP_PATH);
        fs::write(&ownership_target, &ownership_bytes)
            .map_err(|error| io_at(&ownership_target, error))?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let live_root = asset_root.join(WORLD_BEHAVIOUR_ROOT);
    let replaced_previous_install = previous.is_some();
    let commit = (|| -> Result<()> {
        if live_root.exists() {
            fs::rename(&live_root, &backup).map_err(|error| io_at(&live_root, error))?;
        }
        let staged_root = stage.join(WORLD_BEHAVIOUR_ROOT);
        if let Some(parent) = live_root.parent() {
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        }
        fs::rename(&staged_root, &live_root).map_err(|error| io_at(&staged_root, error))
    })();
    if let Err(error) = commit {
        if backup.exists() && !live_root.exists() {
            let _ = fs::rename(&backup, &live_root);
        }
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_dir_all(&backup);
        return Err(error);
    }
    let _ = fs::remove_dir_all(&stage);
    let _ = fs::remove_dir_all(&backup);

    let installed_bytes = documents
        .iter()
        .map(|(_, _, bytes, _)| bytes.len() as u64)
        .sum::<u64>()
        + ownership_bytes.len() as u64;
    Ok(WorldBehaviourInstallReport {
        schema: WORLD_BEHAVIOUR_INSTALL_REPORT_SCHEMA.to_owned(),
        source_build: options.source_build.clone(),
        tile_count: documents.len() as u64,
        billboard_count: totals.billboards,
        visibility_switch_count: totals.visibility_switches,
        effect_emitter_count: totals.effect_emitters,
        animation_count: totals.animations,
        animation_clip_count: totals.animation_clips,
        trigger_count: totals.triggers,
        waypoint_count: totals.waypoints,
        trigger_volume_count: totals.trigger_volumes,
        rigid_body_count: totals.rigid_bodies,
        blocker_count: totals.blockers,
        installed_files: documents.len() as u64 + 1,
        installed_bytes,
        // The current asset tree is manifestless. Domain ownership is proved
        // by this install manifest and independently traversed by `xtask
        // assets --full`.
        manifest_files: 0,
        replaced_previous_install,
        ownership_path: WORLD_BEHAVIOUR_OWNERSHIP_PATH.to_owned(),
        source_set_blake3,
    })
}
