use super::*;

pub const PLAYER_SHARED_RIG_SCHEMA: &str = "ffone.player-shared-rig.v3";

pub const PLAYER_SHARED_RIG_REPORT_SCHEMA: &str = "ffone.player-shared-rig-publish-report.v1";

pub const PLAYER_SHARED_RIG_CONTRACT_PATH: &str =
    "characters/player/shared/player_rig_contract.json";

pub const MALE_SHARED_SKELETON_GLB_PATH: &str = "characters/player/male/base/male_skeleton.glb";

pub const FEMALE_SHARED_SKELETON_GLB_PATH: &str =
    "characters/player/female/base/female_skeleton.glb";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerSharedRigPublishOptions {
    pub objects_dump: PathBuf,
    pub asset_root: PathBuf,
    pub supplemental_creator_sources: Vec<PlayerRigSupplementalCreatorSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerRigAnimationPublishOptions {
    pub asset_root: PathBuf,
    pub clip_source_root: PathBuf,
    pub custom_clip_source_root: Option<PathBuf>,
}

impl PlayerRigAnimationPublishOptions {
    pub fn new(asset_root: impl Into<PathBuf>, clip_source_root: impl Into<PathBuf>) -> Self {
        Self {
            asset_root: asset_root.into(),
            clip_source_root: clip_source_root.into(),
            custom_clip_source_root: None,
        }
    }

    #[must_use]
    pub fn with_custom_clip_source_root(mut self, source_root: impl Into<PathBuf>) -> Self {
        self.custom_clip_source_root = Some(source_root.into());
        self
    }
}

impl PlayerSharedRigPublishOptions {
    pub fn new(objects_dump: impl Into<PathBuf>, asset_root: impl Into<PathBuf>) -> Self {
        let asset_root = asset_root.into();
        let supplemental_root = asset_root
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."))
            .join("work/player_shared_rig_sources");
        Self {
            objects_dump: objects_dump.into(),
            asset_root,
            supplemental_creator_sources: vec![
                PlayerRigSupplementalCreatorSource {
                    gender: PlayerRigGender::Female,
                    exact_route: "wear/f_pants_gothgirl.nif".to_owned(),
                    true_name: "f_pants_gothgirl".to_owned(),
                    category: "pants".to_owned(),
                    source: supplemental_root.join("f_pants_gothgirl.source.json"),
                },
                PlayerRigSupplementalCreatorSource {
                    gender: PlayerRigGender::Female,
                    exact_route: "wear/f_pants_stylistdandy.nif".to_owned(),
                    true_name: "f_pants_stylistdandy".to_owned(),
                    category: "pants".to_owned(),
                    source: supplemental_root.join("f_pants_stylistdandy.source.json"),
                },
            ],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerRigSupplementalCreatorSource {
    pub gender: PlayerRigGender,
    pub exact_route: String,
    pub true_name: String,
    pub category: String,
    pub source: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerRigGender {
    Male,
    Female,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerRigSourceIdentity {
    pub object_dump: String,
    pub object_dump_bytes: u64,
    pub object_dump_blake3: String,
    pub appearance: String,
    pub appearance_blake3: String,
    pub avatar_items: String,
    pub avatar_items_blake3: String,
    pub source_build: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerRigNode {
    pub actor_bone_index: u32,
    pub source_transform_path_id: i64,
    pub source_game_object_path_id: i64,
    pub true_name: String,
    pub full_path: String,
    pub parent_actor_bone_index: Option<u32>,
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerRigClipContract {
    pub name: String,
    pub source_path_id: i64,
    pub gltf_animation_index: u32,
    pub channel_count: u32,
    pub source_key_count: u64,
    pub duration_seconds_bits: u64,
    pub playback: String,
    pub runtime_status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerRigSkinRemap {
    pub renderer_true_name: String,
    pub renderer_path_id: i64,
    pub actor_wear_index_table_path_id: i64,
    pub actor_bone_indices: Vec<u32>,
    pub actor_bone_paths: Vec<String>,
    pub gltf_joint_paths: Vec<String>,
    pub exact_transform_index_parity: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerRigPartContract {
    pub exact_route: String,
    pub true_name: String,
    pub glb: String,
    pub actor_skin_combiner_clothes_index: u8,
    pub skins: Vec<PlayerRigSkinRemap>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerRigCreatorChoiceContract {
    pub appearance_category: CharacterAppearanceCategory,
    pub creation_index: u16,
    pub item_number: u32,
    pub exact_route: String,
    pub glb: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerGenderRigContract {
    pub gender: PlayerRigGender,
    pub exact_actor_route: String,
    pub actor_root_path_id: i64,
    pub actor_skin_combiner_path_id: i64,
    pub animation_component_path_id: i64,
    pub skeleton_glb: String,
    pub nodes: Vec<PlayerRigNode>,
    pub clips: Vec<PlayerRigClipContract>,
    pub creator_parts: Vec<PlayerRigPartContract>,
    pub default_creator_part_routes: Vec<String>,
    pub creator_choices: Vec<PlayerRigCreatorChoiceContract>,
    pub stand1_runtime_ready: bool,
    pub height_shape_assets_published: bool,
    pub height_shape_runtime_status: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerSharedRigContract {
    pub schema: String,
    pub status: String,
    pub source: PlayerRigSourceIdentity,
    pub native_coordinate_contract: ffone_skinned_model::NativeCoordinateContract,
    pub creator_preview_ready: bool,
    pub fake_animation_used: bool,
    pub unity_runtime_required: bool,
    pub genders: Vec<PlayerGenderRigContract>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerSharedRigPublishReport {
    pub schema: String,
    pub status: String,
    pub contract_path: String,
    pub male_skeleton_glb: String,
    pub female_skeleton_glb: String,
    pub male_actor_bones: u32,
    pub female_actor_bones: u32,
    pub published_animation_clips: u32,
    pub verified_creator_parts: u32,
    pub verified_creator_choices: u32,
    pub verified_skin_palettes: u32,
    pub manifest_files: u64,
    pub creator_preview_ready: bool,
    pub height_shape_runtime_status: String,
}

#[derive(Clone, Copy)]
pub(super) struct CustomRuntimeClipSpec {
    pub(super) semantic_name: &'static str,
    pub(super) female_source_name: &'static str,
    pub(super) female_source_path_id: i64,
    pub(super) female_source_file: &'static str,
    pub(super) male_source_name: &'static str,
    pub(super) male_source_path_id: i64,
    pub(super) male_source_file: &'static str,
    pub(super) female_minimum_mapped_paths: usize,
    pub(super) male_minimum_mapped_paths: usize,
}

impl CustomRuntimeClipSpec {
    pub(super) fn source_for(self, gender: PlayerRigGender) -> (&'static str, i64, &'static str, usize) {
        match gender {
            PlayerRigGender::Male => (
                self.male_source_name,
                self.male_source_path_id,
                self.male_source_file,
                self.male_minimum_mapped_paths,
            ),
            PlayerRigGender::Female => (
                self.female_source_name,
                self.female_source_path_id,
                self.female_source_file,
                self.female_minimum_mapped_paths,
            ),
        }
    }
}

pub(super) const fn custom_runtime_clip(
    semantic_name: &'static str,
    source_name: &'static str,
    source_path_id: i64,
    source_file: &'static str,
    female_minimum_mapped_paths: usize,
    male_minimum_mapped_paths: usize,
) -> CustomRuntimeClipSpec {
    CustomRuntimeClipSpec {
        semantic_name,
        female_source_name: source_name,
        female_source_path_id: source_path_id,
        female_source_file: source_file,
        male_source_name: source_name,
        male_source_path_id: source_path_id,
        male_source_file: source_file,
        female_minimum_mapped_paths,
        male_minimum_mapped_paths,
    }
}

pub(super) fn runtime_clip_loops(name: &str) -> bool {
    matches!(
        name,
        "stand1"
            | "death"
            | "riflestand1"
            | "stickstand1"
            | "pistolstand1"
            | "stickready"
            | "stickrun"
            | "stickrunback"
            | "stickjump"
            | "pistolready"
            | "pistolrun"
            | "pistolrunback"
            | "pistoljump"
            | "run"
            | "runback"
            | "jump"
            | "rifleready"
            | "riflerun"
            | "riflerunback"
            | "riflejump"
            | "bombstand1"
            | "bombready"
            | "bombrun"
            | "bombrunback"
            | "bombjump"
            | "rocketstand1"
            | "rocketready"
            | "rocketrun"
            | "rocketrunback"
            | "rocketjump"
            | "swim"
            | "swimback"
            | "swimidle"
            | "swimleft"
            | "swimright"
            | "slide"
            | "ropedown"
            | "inven"
            | "board_inven"
            | "scooter_inven"
            | "ropedrop"
            | "ropeleft"
            | "roperight"
            | "ropestand1"
            | "ropestand2"
            | "ropeturn"
            | "ropeup"
            | "mount1"
            | "mount2"
    )
}

pub(super) fn runtime_clip_status(name: &str) -> &'static str {
    match name {
        "stand1" | "riflestand1" | "stickstand1" | "pistolstand1" | "run" | "staying"
        | "standup" | "runback" | "jumpstart" | "jump" | "jumpend" | "jumplandrun" | "die"
        | "death" | "rifleready" | "stickready" | "stickrun" | "stickrunback"
        | "stickjumpstart" | "stickjump" | "stickjumpend" | "stickjumplandrun" | "pistolready"
        | "pistolrun" | "pistolrunback" | "pistoljumpstart" | "pistoljump" | "pistoljumpend"
        | "pistoljumplandrun" | "stickattack1" | "stickattack1upper" | "pistolattack1"
        | "pistolattack1upper" | "rifleattack1" | "rifleattack1upper" | "riflerun"
        | "riflerunback" | "riflejumpstart" | "riflejump" | "riflejumpend" | "riflejumplandrun"
        | "bombstand1" | "bombready" | "bombattack1" | "bombattack1upper" | "bombrun"
        | "bombrunback" | "bombjumpstart" | "bombjump" | "bombjumpend" | "bombjumplandrun"
        | "rocketstand1" | "rocketready" | "rocketattack1" | "rocketattack1upper" | "rocketrun"
        | "rocketrunback" | "rocketjumpstart" | "rocketjump" | "rocketjumpend"
        | "rocketjumplandrun" | "swim" | "swimback" | "swimidle" | "swimleft" | "swimright"
        | "slide" | "ropedown" | "inven" | "board_inven" | "scooter_inven" | "ropedrop"
        | "ropeleft" | "roperight" | "ropestand1" | "ropestand2" | "ropeturn" | "ropeup"
        | "mount1" | "mount2" | "attack1" | "attack1upper" | "cry" | "angry" | "shocked"
        | "hello" | "thank" | "dance1" | "kiss" | "agree" | "laugh" | "no" | "flex" | "tease"
        | "ok" | "applaud" | "cheer" | "dance2" | "dance3" | "dance4" | "dance5" | "goodbye"
        | "beach1" | "beach2" | "beach3" => "native-bevy-ready",
        "height_Add" | "shape_Add" | "turnleft" | "turnright" | "rifleturnleft"
        | "rifleturnright" | "scooter_turnleft" | "scooter_turnright" => "native-bevy-additive-delta-ready",
        "height" | "shape" => "native-bevy-static-scale-ready",
        name if name.starts_with("ffr_") => "native-bevy-ready",
        _ => "asset-published-runtime-pending",
    }
}

pub(super) struct RigOutput {
    pub(super) relative: String,
    pub(super) kind: ProjectAssetKind,
    pub(super) bytes: Vec<u8>,
    pub(super) source_path: String,
}

/// Encode an explicitly scoped set of additional clips against the native rig.
/// This writes Editor staging only; the append publisher preserves existing GLB
/// animations, buffers and contract entries when installing these additions.
pub fn export_player_rig_clip_additions(asset_root: &Path, objects: &Path, output: &Path) -> Result<()> {
    let contract_path = asset_root.join(PLAYER_SHARED_RIG_CONTRACT_PATH);
    let contract: PlayerSharedRigContract = serde_json::from_reader(BufReader::new(
        File::open(&contract_path).map_err(|error| io_at(&contract_path, error))?
    )).map_err(|source| PipelineError::Json { path: contract_path.display().to_string(), source })?;
    std::fs::create_dir_all(output).map_err(|error| io_at(output, error))?;
    for gender in &contract.genders {
        let (label, root) = match gender.gender { PlayerRigGender::Male => ("male", "m"), PlayerRigGender::Female => ("female", "w") };
        let catalog = DumpCatalog::read(&objects.join(format!("{label}.json")))?;
        let mut animations = Vec::new();
        for object in &catalog.objects {
            let mut clip = decode_clip(&catalog, &gender.nodes, root, &object.name, object.path_id)?;
            rebase_legacy_additive_clip(&mut clip, &gender.nodes)?;
            animations.push(clip);
        }
        let model = NativeModel {
            schema: ffone_skinned_model::MODEL_SCHEMA.to_owned(), name: root.to_owned(),
            native_coordinate_contract: exact_native_coordinate_contract(), roots: vec![0],
            nodes: gender.nodes.iter().map(|node| ModelNode {
                name: node.true_name.clone(), legacy_name: None, legacy_sibling_ordinal: None,
                parent: node.parent_actor_bone_index, translation: node.translation,
                rotation: node.rotation, scale: node.scale, mesh: None, skin: None,
            }).collect(),
            meshes: Vec::new(), skins: Vec::new(), materials: Vec::new(), textures: Vec::new(), samplers: Vec::new(), animations,
        };
        let glb = encode_glb(&model).map_err(|error| rig_message(format!("clip additions: {error}")))?;
        let path = output.join(format!("{label}.glb"));
        std::fs::write(&path, glb).map_err(|error| io_at(&path, error))?;
    }
    Ok(())
}

pub fn publish_player_rig_animations(options: &PlayerRigAnimationPublishOptions) -> Result<()> {
    let contract_path = options.asset_root.join(PLAYER_SHARED_RIG_CONTRACT_PATH);
    let contract_file = File::open(&contract_path).map_err(|error| io_at(&contract_path, error))?;
    let mut contract: PlayerSharedRigContract =
        serde_json::from_reader(BufReader::new(contract_file)).map_err(|source| {
            PipelineError::Json {
                path: contract_path.display().to_string(),
                source,
            }
        })?;

    if contract.schema != PLAYER_SHARED_RIG_SCHEMA {
        return rig_error(format!(
            "runtime animation contract schema is {:?}; expected {PLAYER_SHARED_RIG_SCHEMA:?}",
            contract.schema
        ));
    }
    let male_count = contract
        .genders
        .iter()
        .filter(|gender| gender.gender == PlayerRigGender::Male)
        .count();
    let female_count = contract
        .genders
        .iter()
        .filter(|gender| gender.gender == PlayerRigGender::Female)
        .count();
    if male_count != 1 || female_count != 1 {
        return rig_error(format!(
            "runtime animation contract resolves male={male_count}, female={female_count}; expected exactly one of each"
        ));
    }

    let female_reference = contract.genders.iter()
        .find(|gender| gender.gender == PlayerRigGender::Female).unwrap().nodes.clone();
    let mut prepared_outputs = Vec::with_capacity(contract.genders.len() + 2);
    for gender in &mut contract.genders {
        let (root_name, expected_skeleton, clip_specs): (&str, &str, &[(&str, i64, &str)]) =
            match gender.gender {
                PlayerRigGender::Male => ("m", MALE_SHARED_SKELETON_GLB_PATH, &MALE_RUNTIME_CLIPS),
                PlayerRigGender::Female => {
                    ("w", FEMALE_SHARED_SKELETON_GLB_PATH, &FEMALE_RUNTIME_CLIPS)
                }
            };
        if gender.skeleton_glb != expected_skeleton {
            return rig_error(format!(
                "{:?} runtime animation skeleton is {:?}; expected {expected_skeleton:?}",
                gender.gender, gender.skeleton_glb
            ));
        }
        // Normal publishing consumes the checked one-object-per-file cache.
        // A full, immutable primary dump is also accepted so newly admitted
        // clips can be published without creating another navigation cache.
        let catalog = if options.clip_source_root.is_file() {
            DumpCatalog::read(&options.clip_source_root)?
        } else {
            let source_paths = clip_specs
                .iter()
                .map(|(_, _, file)| options.clip_source_root.join(file))
                .collect::<Vec<_>>();
            DumpCatalog::read_objects(&source_paths)?
        };
        let custom_catalog = options
            .custom_clip_source_root
            .as_ref()
            .map(|source_root| {
                let paths = CUSTOM_RUNTIME_CLIPS
                    .iter()
                    .map(|spec| {
                        let (_, _, source_file, _) = spec.source_for(gender.gender);
                        source_root.join(source_file)
                    })
                    .collect::<Vec<_>>();
                DumpCatalog::read_objects(&paths)
            })
            .transpose()?;
        let custom_count = custom_catalog
            .as_ref()
            .map_or(0, |_| CUSTOM_RUNTIME_CLIPS.len());
        let mut animations = Vec::with_capacity(clip_specs.len() + custom_count);
        let mut clip_contracts = Vec::with_capacity(clip_specs.len() + custom_count);
        for (animation_index, (name, path_id, _)) in clip_specs.iter().copied().enumerate() {
            let mut clip = decode_clip(&catalog, &gender.nodes, root_name, name, path_id)?;
            rebase_legacy_additive_clip(&mut clip, &gender.nodes)?;
            let source_key_count = clip
                .channels
                .iter()
                .map(|channel| u64::from(channel.source_key_count))
                .sum();
            clip_contracts.push(PlayerRigClipContract {
                name: name.to_owned(),
                source_path_id: path_id,
                gltf_animation_index: animation_index as u32,
                channel_count: clip.channels.len() as u32,
                source_key_count,
                duration_seconds_bits: clip.duration.to_bits(),
                playback: if runtime_clip_loops(name) {
                    "loop".to_owned()
                } else {
                    "clamp".to_owned()
                },
                runtime_status: runtime_clip_status(name).to_owned(),
            });
            animations.push(clip);
        }
        if let Some(custom_catalog) = custom_catalog.as_ref() {
            for spec in CUSTOM_RUNTIME_CLIPS {
                let (source_name, source_path_id, _, minimum_mapped_paths) =
                    spec.source_for(gender.gender);
                let mut clip = decode_retargeted_custom_clip(
                    custom_catalog,
                    &gender.nodes,
                    root_name,
                    spec.semantic_name,
                    source_name,
                    source_path_id,
                    minimum_mapped_paths,
                )?;
                if gender.gender == PlayerRigGender::Male && source_name.starts_with("f_") {
                    super::retarget::retarget_rest_pose(&mut clip, &gender.nodes, &female_reference)?;
                }
                let source_key_count = clip
                    .channels
                    .iter()
                    .map(|channel| u64::from(channel.source_key_count))
                    .sum();
                clip_contracts.push(PlayerRigClipContract {
                    name: spec.semantic_name.to_owned(),
                    source_path_id,
                    gltf_animation_index: animations.len() as u32,
                    channel_count: clip.channels.len() as u32,
                    source_key_count,
                    duration_seconds_bits: clip.duration.to_bits(),
                    playback: "clamp".to_owned(),
                    runtime_status: runtime_clip_status(spec.semantic_name).to_owned(),
                });
                animations.push(clip);
            }
        }
        let model = NativeModel {
            schema: ffone_skinned_model::MODEL_SCHEMA.to_owned(),
            name: root_name.to_owned(),
            native_coordinate_contract: exact_native_coordinate_contract(),
            roots: vec![0],
            nodes: gender
                .nodes
                .iter()
                .map(|node| ModelNode {
                    name: node.true_name.clone(),
                    legacy_name: None,
                    legacy_sibling_ordinal: None,
                    parent: node.parent_actor_bone_index,
                    translation: node.translation,
                    rotation: node.rotation,
                    scale: node.scale,
                    mesh: None,
                    skin: None,
                })
                .collect(),
            meshes: Vec::new(),
            skins: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
            animations,
        };
        let glb = encode_glb(&model).map_err(|error| {
            rig_message(format!(
                "{:?} runtime animation GLB encoding failed: {error}",
                gender.gender
            ))
        })?;
        let required = clip_contracts
            .iter()
            .map(|clip| (clip.name.as_str(), clip.source_path_id))
            .collect::<Vec<_>>();
        validate_published_animations(&parse_glb_json(&glb)?, &required)?;
        let glb_path = options.asset_root.join(&gender.skeleton_glb);
        prepared_outputs.push((glb_path, glb));
        gender.clips = clip_contracts;
        gender.height_shape_runtime_status =
            "native-bevy-static-scale-and-additive-delta-ready".to_owned();
    }

    contract.status = if options.custom_clip_source_root.is_some() {
        "native-shared-skeleton-ffr-standard-replacements-and-custom-emotes-ready"
    } else {
        "native-shared-skeleton-tutorial-runtime-clips-and-complete-creator-routing-ready"
    }
    .to_owned();
    let mut contract_bytes =
        serde_json::to_vec_pretty(&contract).map_err(|source| PipelineError::Json {
            path: contract_path.display().to_string(),
            source,
        })?;
    contract_bytes.push(b'\n');
    prepared_outputs.push((contract_path, contract_bytes));
    // New native asset roots are domain-catalog owned and intentionally have
    // no global asset-manifest.json. Keep legacy build roots compatible while
    // allowing the three player-rig outputs to remain one transaction.
    if options.asset_root.join(ASSET_MANIFEST_FILE).is_file() {
        let manifest_output =
            prepare_player_rig_animation_manifest(&options.asset_root, &prepared_outputs)?;
        prepared_outputs.push(manifest_output);
    }
    commit_player_rig_animation_outputs(&prepared_outputs)
}

pub(super) fn prepare_player_rig_animation_manifest(
    asset_root: &Path,
    outputs: &[(PathBuf, Vec<u8>)],
) -> Result<(PathBuf, Vec<u8>)> {
    let expected_output_count = 3;
    if outputs.len() != expected_output_count {
        return rig_error(format!(
            "runtime animation manifest refresh received {} outputs; expected {expected_output_count}",
            outputs.len()
        ));
    }
    let mut relative_outputs = Vec::with_capacity(outputs.len());
    let mut unique_paths = BTreeSet::new();
    for (destination, bytes) in outputs {
        let relative = destination.strip_prefix(asset_root).map_err(|_| {
            rig_message(format!(
                "runtime animation output {} escapes asset root {}",
                destination.display(),
                asset_root.display()
            ))
        })?;
        let relative = relative.to_str().ok_or_else(|| {
            rig_message(format!(
                "runtime animation output path is not UTF-8: {}",
                destination.display()
            ))
        })?;
        let relative = relative.replace('\\', "/");
        if relative.is_empty()
            || relative.starts_with('/')
            || relative.contains(':')
            || relative
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
        {
            return rig_error(format!(
                "runtime animation output has unsafe manifest path {relative:?}"
            ));
        }
        if !unique_paths.insert(relative.clone()) {
            return rig_error(format!(
                "runtime animation manifest refresh repeats {relative:?}"
            ));
        }
        let expected_kind = match relative.as_str() {
            MALE_SHARED_SKELETON_GLB_PATH | FEMALE_SHARED_SKELETON_GLB_PATH => {
                ProjectAssetKind::Model
            }
            PLAYER_SHARED_RIG_CONTRACT_PATH => ProjectAssetKind::Data,
            _ => {
                return rig_error(format!(
                    "runtime animation manifest refresh received unexpected output {relative:?}"
                ));
            }
        };
        relative_outputs.push((relative, expected_kind, bytes));
    }

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return rig_error(format!(
            "runtime animation asset manifest schema is {:?}; expected {PROJECT_ASSET_SCHEMA:?}",
            manifest.schema
        ));
    }
    for (relative, expected_kind, bytes) in relative_outputs {
        let mut matches = manifest
            .files
            .iter_mut()
            .filter(|entry| entry.path == relative);
        let Some(entry) = matches.next() else {
            return rig_error(format!(
                "runtime animation asset manifest has no existing entry for {relative:?}"
            ));
        };
        if matches.next().is_some() {
            return rig_error(format!(
                "runtime animation asset manifest repeats entry {relative:?}"
            ));
        }
        if entry.kind != expected_kind {
            return rig_error(format!(
                "runtime animation asset manifest entry {relative:?} has kind {:?}; expected {expected_kind:?}",
                entry.kind
            ));
        }
        entry.bytes = u64::try_from(bytes.len()).map_err(|_| {
            rig_message(format!(
                "runtime animation output {relative:?} exceeds u64 byte accounting"
            ))
        })?;
        entry.blake3 = blake3::hash(bytes).to_hex().to_string();
    }
    let mut next_manifest =
        serde_json::to_vec_pretty(&manifest).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    next_manifest.push(b'\n');
    Ok((manifest_path, next_manifest))
}

pub(super) const PLAYER_RIG_ANIMATION_STAGE_SUFFIX: &str = ".player-rig-animation.next";

pub(super) const PLAYER_RIG_ANIMATION_BACKUP_SUFFIX: &str = ".player-rig-animation.backup";

pub(super) fn player_rig_animation_sidecar(path: &Path, suffix: &str) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            rig_message(format!(
                "runtime animation output has no UTF-8 file name: {}",
                path.display()
            ))
        })?;
    Ok(path.with_file_name(format!("{file_name}{suffix}")))
}

pub(super) fn commit_player_rig_animation_outputs(outputs: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    commit_player_rig_animation_outputs_with_gate(outputs, |_, _| Ok(()))
}
