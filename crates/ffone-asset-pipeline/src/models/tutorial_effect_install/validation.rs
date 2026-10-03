use super::*;

#[derive(Clone, Debug)]
pub(super) struct SourceAudit {
    pub(super) bundle: TutorialSourceFileProof,
    pub(super) dump: TutorialSourceFileProof,
    pub(super) source_assets: Vec<TutorialSourceAssetProof>,
    pub(super) effect_closures: Vec<TutorialEffectClosureFile>,
    pub(super) projectile_effect_closures: Vec<TutorialEffectClosureFile>,
    pub(super) bullet_closure: TutorialEffectClosureFile,
    pub(super) bullets: Vec<TutorialBulletRowFile>,
}

pub(super) fn audit_source(
    source_bundle: &Path,
    object_dump: &Path,
    dependencies: &[(PathBuf, PathBuf)],
    source_build: &str,
) -> Result<SourceAudit> {
    let primary = load_source_asset(source_bundle, object_dump)?;
    if primary.proof.asset != PRIMARY_EFFECTS_ASSET {
        return invalid(format!(
            "primary Effects dump asset is {:?}, expected exact Retrotribution asset {:?}",
            primary.proof.asset, PRIMARY_EFFECTS_ASSET
        ));
    }
    let mut loaded = Vec::with_capacity(dependencies.len().saturating_add(1));
    loaded.push(primary);
    for (bundle, dump) in dependencies {
        loaded.push(load_source_asset(bundle, dump)?);
    }
    let actual_dependencies = loaded
        .iter()
        .skip(1)
        .map(|source| source.proof.asset.clone())
        .collect::<BTreeSet<_>>();
    let expected_dependencies = [EFFECTS_DEPENDENCY_B4, EFFECTS_DEPENDENCY_BD5]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual_dependencies != expected_dependencies || loaded.len() != 3 {
        return invalid(format!(
            "Retrotribution tutorial Effects requires exactly dependency assets {expected_dependencies:?}, got {actual_dependencies:?}"
        ));
    }
    loaded[1..].sort_by(|left, right| left.proof.asset.cmp(&right.proof.asset));

    let bundle_proof = loaded[0].proof.serialized_asset.clone();
    let dump_proof = loaded[0].proof.object_dump.clone();
    let source_assets = loaded
        .iter()
        .map(|source| source.proof.clone())
        .collect::<Vec<_>>();
    validate_exact_source_asset_proofs(&source_assets)?;
    let mut index = BTreeMap::<UnityObjectKey, &DumpObject>::new();
    for source in &loaded {
        for object in &source.objects {
            let key = UnityObjectKey {
                asset: object.asset.clone(),
                path_id: object.path_id,
            };
            if index.insert(key, object).is_some() {
                return invalid(format!(
                    "dump for asset {:?} contains duplicate pathId {}",
                    object.asset, object.path_id
                ));
            }
        }
    }
    let asset_bundle_objects = loaded[0]
        .objects
        .iter()
        .filter(|object| object.object_type == "AssetBundle")
        .collect::<Vec<_>>();
    if asset_bundle_objects.len() != 1 {
        return invalid(format!(
            "Effects dump must contain exactly one AssetBundle object, found {}",
            asset_bundle_objects.len()
        ));
    }
    let container = find_unique_field(&asset_bundle_objects[0].value, "m_Container")?;
    let routes = collect_container_routes(container, PRIMARY_EFFECTS_ASSET)?;

    let expected_routes = RETROBUTION_TUTORIAL_EFFECT_IDS
        .iter()
        .map(|effect_id| effect_route(*effect_id))
        .chain(
            RETROBUTION_FUSION_ACTOR_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(
            RETROBUTION_CHARACTER_ACTOR_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(
            RETROBUTION_WORLD_EP_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(
            RETROBUTION_PLAYER_STATUS_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(
            RETROBUTION_NPC_WARP_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(
            RETROBUTION_WEAPON_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(
            RETROBUTION_NPC_GAME_ICON_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(
            RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS
                .iter()
                .map(|effect_id| effect_route(*effect_id)),
        )
        .chain(std::iter::once(BULLET_TABLE_ROUTE.to_owned()))
        .collect::<BTreeSet<_>>();
    validate_exact_routes(&routes, &expected_routes)?;

    let mut effect_closures = Vec::with_capacity(
        RETROBUTION_TUTORIAL_EFFECT_IDS.len()
            + RETROBUTION_FUSION_ACTOR_EFFECT_IDS.len()
            + RETROBUTION_CHARACTER_ACTOR_EFFECT_IDS.len()
            + RETROBUTION_WORLD_EP_EFFECT_IDS.len()
            + RETROBUTION_PLAYER_STATUS_EFFECT_IDS.len()
            + RETROBUTION_NPC_WARP_EFFECT_IDS.len()
            + RETROBUTION_WEAPON_EFFECT_IDS.len()
            + RETROBUTION_NPC_GAME_ICON_EFFECT_IDS.len(),
    );
    for effect_id in RETROBUTION_TUTORIAL_EFFECT_IDS
        .into_iter()
        .chain(RETROBUTION_FUSION_ACTOR_EFFECT_IDS)
        .chain(RETROBUTION_CHARACTER_ACTOR_EFFECT_IDS)
        .chain(RETROBUTION_WORLD_EP_EFFECT_IDS)
        .chain(RETROBUTION_PLAYER_STATUS_EFFECT_IDS)
        .chain(RETROBUTION_NPC_WARP_EFFECT_IDS)
        .chain(RETROBUTION_WEAPON_EFFECT_IDS)
        .chain(RETROBUTION_NPC_GAME_ICON_EFFECT_IDS)
    {
        let route = effect_route(effect_id);
        let root = unique_route_root(&routes, &route)?;
        effect_closures.push(build_closure(
            Some(effect_id),
            &route,
            &root.asset,
            root.path_id,
            &index,
            &bundle_proof.blake3,
            &dump_proof.blake3,
            &source_assets,
        )?);
    }

    let mut projectile_effect_closures =
        Vec::with_capacity(RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS.len());
    for effect_id in RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS {
        let route = effect_route(effect_id);
        let root = unique_route_root(&routes, &route)?;
        projectile_effect_closures.push(build_closure(
            Some(effect_id),
            &route,
            &root.asset,
            root.path_id,
            &index,
            &bundle_proof.blake3,
            &dump_proof.blake3,
            &source_assets,
        )?);
    }

    let bullet_root = unique_route_root(&routes, BULLET_TABLE_ROUTE)?;
    let bullet_closure = build_closure(
        None,
        BULLET_TABLE_ROUTE,
        &bullet_root.asset,
        bullet_root.path_id,
        &index,
        &bundle_proof.blake3,
        &dump_proof.blake3,
        &source_assets,
    )?;
    let bullet_object = index.get(&bullet_root).copied().ok_or_else(|| {
        PipelineError::InvalidManifest(format!(
            "bullettable root {}#{} is missing from dump",
            bullet_root.asset, bullet_root.path_id
        ))
    })?;
    let table = find_unique_field(&bullet_object.value, "m_pBulletData")?;
    let rows = table.as_array().ok_or_else(|| {
        PipelineError::InvalidManifest("m_pBulletData is not an array".to_owned())
    })?;
    let mut bullets = Vec::with_capacity(RETROBUTION_TUTORIAL_BULLET_TYPES.len());
    for bullet_type in RETROBUTION_TUTORIAL_BULLET_TYPES {
        let row = rows.get(bullet_type as usize).ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "m_pBulletData has {} rows; tutorial needs index {bullet_type}",
                rows.len()
            ))
        })?;
        let serialized_row = canonical_json(row);
        validate_bullet_row_shape(&serialized_row, bullet_type)?;
        let serialized_bytes = serde_json::to_vec(&serialized_row).map_err(json_error)?;
        let serialized_row_blake3 = hash(&serialized_bytes);
        let expected_row_blake3 = ffone_runtime_contracts::retrobution_bullet_row_proof(
            bullet_type,
        )
        .ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "BulletTable row {bullet_type} has no clean-primary proof"
            ))
        })?;
        if serialized_row_blake3 != expected_row_blake3 {
            return invalid(format!(
                "BulletTable row {bullet_type} differs from clean primary: actual={serialized_row_blake3}, expected={expected_row_blake3}"
            ));
        }
        let parameters = parse_bullet_parameters(&serialized_row, bullet_type)?;
        validate_exact_bullet_parameters(&parameters, bullet_type)?;
        bullets.push(TutorialBulletRowFile {
            schema: TUTORIAL_BULLET_ROW_SCHEMA.to_owned(),
            bullet_type,
            source_route: BULLET_TABLE_ROUTE.to_owned(),
            source_root_path_id: bullet_root.path_id,
            serialized_row_blake3,
            parameters,
            serialized_row,
        });
    }

    let _ = source_build;
    Ok(SourceAudit {
        bundle: bundle_proof,
        dump: dump_proof,
        source_assets,
        effect_closures,
        projectile_effect_closures,
        bullet_closure,
        bullets,
    })
}

pub(super) fn validate_exact_source_asset_proofs(sources: &[TutorialSourceAssetProof]) -> Result<()> {
    if sources.len() != RETROBUTION_TUTORIAL_SOURCE_ASSET_PROOFS.len() {
        return invalid(format!(
            "tutorial source proof count is {}, expected {}",
            sources.len(),
            RETROBUTION_TUTORIAL_SOURCE_ASSET_PROOFS.len()
        ));
    }
    for (actual, (asset, serialized_bytes, serialized_hash, dump_bytes, dump_hash)) in
        sources.iter().zip(RETROBUTION_TUTORIAL_SOURCE_ASSET_PROOFS)
    {
        if actual.asset != asset
            || actual.serialized_asset.logical_name != format!("{asset}/serialized-asset")
            || actual.serialized_asset.bytes != serialized_bytes
            || actual.serialized_asset.blake3 != serialized_hash
            || actual.object_dump.logical_name
                != format!("{asset}/fusionforge-dump-object-all.json")
            || actual.object_dump.bytes != dump_bytes
            || actual.object_dump.blake3 != dump_hash
        {
            return invalid(format!(
                "source proof for asset {:?} differs from exact Retrotribution evidence: actual={actual:?}",
                actual.asset
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_exact_routes(
    routes: &BTreeMap<String, Vec<UnityObjectKey>>,
    expected: &BTreeSet<String>,
) -> Result<()> {
    for expected_route in expected {
        let exact = routes.get(expected_route).map(Vec::len).unwrap_or_default();
        if exact != 1 {
            let casefold = routes
                .keys()
                .filter(|route| route.eq_ignore_ascii_case(expected_route))
                .cloned()
                .collect::<Vec<_>>();
            return invalid(format!(
                "required exact AssetBundle route {expected_route:?} resolves {exact} entries; case-insensitive candidates={casefold:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_bullet_row_shape(row: &JsonValue, bullet_type: i32) -> Result<()> {
    let object = row.as_object().ok_or_else(|| {
        PipelineError::InvalidManifest(format!("BulletTable row {bullet_type} is not an object"))
    })?;
    let actual = object.keys().cloned().collect::<BTreeSet<_>>();
    let expected = [
        "m_fBulletModelScale",
        "m_fCancelModelScale",
        "m_fCurveHeight",
        "m_fFireModelScale",
        "m_fHideTime",
        "m_fMaxTimer",
        "m_fSuccModelScale",
        "m_iCancelScript",
        "m_iFireScript",
        "m_iParticleScript",
        "m_iSuccScript",
        "m_strFireLink",
        "m_strSuccLink",
        "m_strSuccSound",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if actual != expected {
        return invalid(format!(
            "BulletTable row {bullet_type} fields differ from the exact Retrotribution row: actual={actual:?}, expected={expected:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_exact_bullet_parameters(
    parameters: &TutorialBulletParameters,
    bullet_type: i32,
) -> Result<()> {
    let expected = match bullet_type {
        5 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: -1,
            particle_script: 100,
            success_script: 757,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 1.0,
            bullet_model_scale: 0.0,
            success_model_scale: 1.5,
            hide_time_seconds: 0.4000000059604645,
            maximum_time_seconds: 0.0,
            fire_link: "\"".to_owned(),
            success_link: "center".to_owned(),
            success_sound: "MeleeMedTarget-03".to_owned(),
        },
        13 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: 809,
            particle_script: 808,
            success_script: 4,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 1.0,
            bullet_model_scale: 1.0,
            success_model_scale: 1.0,
            hide_time_seconds: 0.0,
            maximum_time_seconds: 0.30000001192092896,
            fire_link: "Gtag01".to_owned(),
            success_link: "center".to_owned(),
            success_sound: "LaserHvyTarget-05".to_owned(),
        },
        66 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: -1,
            particle_script: 17,
            success_script: 653,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 1.0,
            bullet_model_scale: 1.0,
            success_model_scale: 3.0,
            hide_time_seconds: 0.30000001192092896,
            maximum_time_seconds: 0.5,
            fire_link: "tag01".to_owned(),
            success_link: "Bip01 center".to_owned(),
            success_sound: "......".to_owned(),
        },
        76 | 77 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: 0,
            particle_script: if bullet_type == 76 { 15 } else { 391 },
            success_script: 0,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 0.0,
            bullet_model_scale: 1.0,
            success_model_scale: 0.0,
            hide_time_seconds: 0.0,
            maximum_time_seconds: 2.0,
            fire_link: "\"".to_owned(),
            success_link: "\"".to_owned(),
            success_sound: "\"".to_owned(),
        },
        106 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: -1,
            particle_script: 0,
            success_script: 653,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 1.0,
            bullet_model_scale: 0.0,
            success_model_scale: 1.5,
            hide_time_seconds: 0.30000001192092896,
            maximum_time_seconds: 0.30000001192092896,
            fire_link: "\"".to_owned(),
            success_link: "Bip01 Spine1".to_owned(),
            success_sound: "\"".to_owned(),
        },
        113 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: 812,
            particle_script: 718,
            success_script: 723,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 1.0,
            bullet_model_scale: 0.800000011920929,
            success_model_scale: 1.399999976158142,
            hide_time_seconds: 0.30000001192092896,
            maximum_time_seconds: 0.5,
            fire_link: "Gtag01".to_owned(),
            success_link: "center".to_owned(),
            success_sound: "SonicTarget-02".to_owned(),
        },
        145 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: -1,
            particle_script: 100,
            success_script: 772,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 1.0,
            bullet_model_scale: 0.0,
            success_model_scale: 2.4000000953674316,
            hide_time_seconds: 0.4000000059604645,
            maximum_time_seconds: 0.0,
            fire_link: "\"".to_owned(),
            success_link: "center".to_owned(),
            success_sound: "MeleeMedTarget-01".to_owned(),
        },
        151 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: 813,
            particle_script: 795,
            success_script: 778,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 1.0,
            bullet_model_scale: 0.800000011920929,
            success_model_scale: 1.899999976158142,
            hide_time_seconds: 0.30000001192092896,
            maximum_time_seconds: 0.5,
            fire_link: "Gtag01".to_owned(),
            success_link: "center".to_owned(),
            success_sound: "SonicTarget-02".to_owned(),
        },
        152 => TutorialBulletParameters {
            cancel_script: 0,
            fire_script: 817,
            particle_script: 789,
            success_script: 774,
            cancel_model_scale: 0.0,
            curve_height: 0.0,
            fire_model_scale: 2.0,
            bullet_model_scale: 1.0,
            success_model_scale: 2.0,
            hide_time_seconds: 0.0,
            maximum_time_seconds: 0.30000001192092896,
            fire_link: "Gtag01".to_owned(),
            success_link: "center".to_owned(),
            success_sound: "LaserLtTarget-04".to_owned(),
        },
        _ => {
            if ffone_runtime_contracts::retrobution_bullet_row_proof(bullet_type).is_some() {
                // The expanded clean weapon set is pinned by the canonical
                // serialized-row proof above. The original ten rows retain
                // their typed assertions as an additional regression guard.
                return Ok(());
            }
            return invalid(format!(
                "bullet type {bullet_type} is outside the exact contract"
            ));
        }
    };
    if parameters != &expected {
        return invalid(format!(
            "BulletTable row {bullet_type} typed parameters differ from the exact Retrotribution contract: actual={parameters:?}, expected={expected:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_prepared_paths(files: &[PreparedFile]) -> Result<()> {
    let mut casefold = BTreeSet::new();
    for file in files {
        validate_relative(&file.relative)?;
        if !casefold.insert(file.relative.to_ascii_lowercase()) {
            return invalid(format!(
                "case-folded tutorial publication collision at {:?}",
                file.relative
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_relative(path: &str) -> Result<()> {
    let normalized = path.replace('\\', "/");
    if normalized != path
        || path.starts_with('/')
        || path.ends_with('/')
        || path.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.contains(':')
                || part.contains('\0')
        })
    {
        return invalid(format!("unsafe tutorial publication path {path:?}"));
    }
    Ok(())
}

pub(super) fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() {
        return invalid(format!("{label} must not be a symlink: {}", path.display()));
    }
    Ok(())
}

pub(super) fn json_error(source: serde_json::Error) -> PipelineError {
    PipelineError::Json {
        path: "<generated tutorial effect JSON>".to_owned(),
        source,
    }
}
