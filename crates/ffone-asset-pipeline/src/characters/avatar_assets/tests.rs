use super::*;
use crate::SourcePackIdentity;
use tempfile::TempDir;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn manifest_file(
    source_path: &str,
    path: &str,
    kind: ProjectAssetKind,
    bytes: &[u8],
) -> ProjectAssetFile {
    ProjectAssetFile {
        source_path: source_path.to_owned(),
        path: path.to_owned(),
        kind,
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
    }
}

fn fixture() -> (TempDir, AvatarCatalogOptions, String) {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let evidence_root = workspace.join("FFOneClient");
    let asset_root = evidence_root.join("assets/game");
    let table_relative = "data/tables/table-set.json";
    let content_relative = "data/catalog/content-index.json";
    let texture_relative = "textures/m_face_001_a.png";
    let texture_bytes = b"png";

    let item_row = |item_number: i64, mesh: i64, equip_type: i64| {
        serde_json::json!({
            "m_iItemNumber": item_number,
            "m_iMesh": mesh,
            "m_iEquipType": equip_type
        })
    };
    let mesh_row = serde_json::json!({
        "m_pstrMMeshModelString": "m_face_001",
        "m_pstrFMeshModelString": "f_face_001",
        "m_pstrMTextureString": "m_face_001",
        "m_pstrFTextureString": "f_face_001",
        "m_pstrMTextureString2": "null",
        "m_pstrFTextureString2": "null"
    });
    let table = |row: Value| {
        serde_json::json!({
            "m_pItemData": [item_row(0, 0, 0), row],
            "m_pItemMeshData": [Value::Null, mesh_row.clone()]
        })
    };
    let table_bytes = serde_json::to_vec(&serde_json::json!({
        "schema": TABLE_SET_SCHEMA,
        "tables": [{
            "key": "fixture",
            "name": "fixture",
            "value": {
                "m_pBackItemTable": table(item_row(1, 1, 0)),
                "m_pFaceItemTable": table(item_row(1, 1, 0)),
                "m_pGlassItemTable": table(item_row(1, 1, 0)),
                "m_pHatItemTable": table(item_row(1, 1, 2)),
                "m_pHeadItemTable": table(item_row(1, 1, 0)),
                "m_pPantsItemTable": table(item_row(1, 1, 0)),
                "m_pShirtsItemTable": table(item_row(1, 1, 0)),
                "m_pShoesItemTable": table(item_row(1, 1, 0)),
                "m_pVehicleItemTable": table(item_row(1, 1, 0)),
                "m_pWeaponItemTable": table(item_row(1, 1, 0))
            }
        }]
    }))
    .unwrap();
    let content_bytes = serde_json::to_vec(&serde_json::json!({
        "schema": CONTENT_INDEX_SCHEMA,
        "nativeOnly": true,
        "assets": [{
            "key": "face-key",
            "kind": "texture",
            "name": "m_face_001_a",
            "path": texture_relative
        }]
    }))
    .unwrap();
    write(&asset_root.join(table_relative), &table_bytes);
    write(&asset_root.join(content_relative), &content_bytes);
    write(&asset_root.join(texture_relative), texture_bytes);

    let mut manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: PROTOCOL_0104,
        locale: "ru-RU".to_owned(),
        source_pack: SourcePackIdentity {
            schema: "fixture".to_owned(),
            manifest_blake3: "0".repeat(64),
        },
        files: vec![
            manifest_file(
                "tables.json",
                table_relative,
                ProjectAssetKind::Data,
                &table_bytes,
            ),
            manifest_file(
                "content.json",
                content_relative,
                ProjectAssetKind::Data,
                &content_bytes,
            ),
            manifest_file(
                "face.png",
                texture_relative,
                ProjectAssetKind::Texture,
                texture_bytes,
            ),
        ],
    };
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    write(&asset_root.join(ASSET_MANIFEST_FILE), &manifest_bytes);

    let plan_path = evidence_root.join("target/avatar-plan.json");
    let plan_value = serde_json::json!({
        "schema": SEMANTIC_PLAN_SCHEMA,
        "mode": "plan-only",
        "status": "ready",
        "productionAssetsMutated": false,
        "inputs": [
            {
                "role": "assetManifest",
                "path": "assets/game/asset-manifest.json",
                "schema": PROJECT_ASSET_SCHEMA,
                "bytes": manifest_bytes.len(),
                "blake3": blake3::hash(&manifest_bytes).to_hex().to_string()
            },
            {
                "role": "contentIndex",
                "path": format!("assets/game/{content_relative}"),
                "schema": CONTENT_INDEX_SCHEMA,
                "bytes": content_bytes.len(),
                "blake3": blake3::hash(&content_bytes).to_hex().to_string()
            },
            {
                "role": "tableSet",
                "path": format!("assets/game/{table_relative}"),
                "schema": TABLE_SET_SCHEMA,
                "bytes": table_bytes.len(),
                "blake3": blake3::hash(&table_bytes).to_hex().to_string()
            }
        ],
        "entities": [
            {
                "id": "player:male:base",
                "tableOwner": null,
                "modelRoutes": ["actor/m.kfm"],
                "modelProposalIds": [],
                "dependencies": [],
                "blockerIds": []
            },
            {
                "id": "equipment:mask:row:1:number:1",
                "tableOwner": {
                    "table": "m_pFaceItemTable.m_pItemData",
                    "rowIndex": 1
                },
                "modelRoutes": ["wear/m_face_001_type02.nif"],
                "modelProposalIds": [],
                "blockerIds": [],
                "dependencies": [{
                    "kind": "texture",
                    "sourceField": "runtime-alias:m_pstrMTextureString",
                    "sourceMName": "m_face_001_a",
                    "legacyRoute": "texture/m_face_001_a.dds",
                    "resolution": "unique_name_candidate_unproven",
                    "ownership": "unresolved",
                    "candidates": [{
                        "nativeKey": "face-key",
                        "projectPath": texture_relative
                    }]
                }]
            }
        ],
        "models": []
    });
    let plan_bytes = serde_json::to_vec(&plan_value).unwrap();
    write(&plan_path, &plan_bytes);
    let plan_hash = blake3::hash(&plan_bytes).to_hex().to_string();
    (
        temp,
        AvatarCatalogOptions::new(&asset_root, &evidence_root, &plan_path)
            .with_expected_semantic_plan_blake3(&plan_hash),
        plan_hash,
    )
}

#[test]
fn protocol_styles_normalize_to_one_typed_selection() {
    let login = CharacterStyle0104 {
        name_check: 1,
        gender: 2,
        face_style: 5,
        hair_style: 21,
        hair_color: 3,
        skin_color: 4,
        eye_color: 2,
        height: 1,
        body: 2,
        class: 3,
        appearance_flag: 0,
        tutorial_flag: 1,
        payzone_flag: 0,
    };
    let normalized = AvatarStyleSelection::try_from(&login).unwrap();
    assert_eq!(normalized.gender, AvatarGender::Female);
    assert_eq!(normalized.face_style, 5);
    assert_eq!(normalized.hair_style, 21);
    assert_eq!(normalized.body, 2);
}

#[test]
fn resolver_never_promotes_name_only_assets_or_invents_a_dexter_fallback() {
    let (_temp, options, _) = fixture();
    let catalog = AvatarAssetCatalog::open(&options).unwrap();
    assert!(catalog.provenance().trusted);
    let style = AvatarStyleSelection {
        gender: AvatarGender::Male,
        face_style: 1,
        hair_style: 0,
        hair_color: 1,
        skin_color: 1,
        eye_color: 1,
        height: 0,
        body: 0,
    };
    let mut equipment = [AvatarItemSelection::default(); 9];
    equipment[4] = AvatarItemSelection {
        item_type: 4,
        item_id: 1,
        ..Default::default()
    };
    let report = catalog.resolve(style, equipment);
    let face = report
        .parts
        .iter()
        .find(|part| part.part == AvatarPartKind::Face)
        .unwrap();
    assert_eq!(
        face.model.as_ref().unwrap().source_route,
        "wear/m_face_001_type02.nif"
    );
    assert_eq!(
        face.model.as_ref().unwrap().status,
        NativeResolutionStatus::Missing
    );
    assert_eq!(
        face.textures[0].status,
        NativeResolutionStatus::HashVerifiedCandidateUnproven
    );
    let json = report.to_pretty_json().unwrap().to_ascii_lowercase();
    assert!(!json.contains("dexter"));
    assert!(!report.complete);
}

#[test]
fn unpinned_plan_blocks_verified_resolution_but_keeps_exact_missing_report() {
    let (_temp, mut options, _) = fixture();
    options.expected_semantic_plan_blake3 = None;
    let catalog = AvatarAssetCatalog::open(&options).unwrap();
    assert!(!catalog.provenance().trusted);
    assert!(
        catalog
            .provenance()
            .issues
            .iter()
            .any(|issue| issue.contains("no trusted expected BLAKE3"))
    );
}

#[test]
fn item_type_mismatch_fails_closed_before_table_lookup() {
    let (_temp, options, _) = fixture();
    let catalog = AvatarAssetCatalog::open(&options).unwrap();
    let part = catalog.resolve_item(
        AvatarPartKind::Hat,
        AvatarPartParticipation::SocketAttachment,
        "m_pHatItemTable",
        "hat",
        4,
        AvatarItemSelection {
            item_type: 6,
            item_id: 1,
            ..Default::default()
        },
        AvatarGender::Male,
        false,
    );
    assert_eq!(part.disposition, AvatarPartDisposition::Missing);
    assert!(part.blockers[0].contains("does not match"));
    assert!(part.model.is_none());
}

#[test]
#[ignore = "hashes the large Retrobution evidence set; run explicitly as an offline audit"]
fn retrobution_creation_row_one_reports_exact_missing_player_roots() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog = AvatarAssetCatalog::open(&AvatarCatalogOptions::new(
        project.join("assets/game"),
        &project,
        project.join("target/semantic-asset-organization-plan.json"),
    ))
    .unwrap();
    let style = AvatarStyleSelection {
        gender: AvatarGender::Male,
        face_style: 5,
        hair_style: 23,
        hair_color: 1,
        skin_color: 1,
        eye_color: 1,
        height: 0,
        body: 0,
    };
    let mut equipment = [AvatarItemSelection::default(); 9];
    equipment[0] = AvatarItemSelection {
        item_type: 0,
        item_id: 1,
        ..Default::default()
    };
    equipment[1] = AvatarItemSelection {
        item_type: 1,
        item_id: 30,
        ..Default::default()
    };
    equipment[2] = AvatarItemSelection {
        item_type: 2,
        item_id: 30,
        ..Default::default()
    };
    equipment[3] = AvatarItemSelection {
        item_type: 3,
        item_id: 30,
        ..Default::default()
    };

    let report = catalog.resolve(style, equipment);
    let routes = report
        .parts
        .iter()
        .filter_map(|part| part.model.as_ref())
        .map(|model| model.source_route.as_str())
        .collect::<BTreeSet<_>>();
    assert!(routes.contains("actor/m.kfm"));
    assert!(routes.contains("wear/m_face_001_type01.nif"));
    assert!(routes.contains("wear/m_head_023_type01.nif"));
    assert!(routes.contains("wear/m_shirt_baseballset.nif"));
    assert!(routes.contains("wear/m_pants_beltarmorset.nif"));
    assert!(routes.contains("wear/m_shoes_blooarmorset.nif"));
    assert!(routes.contains("wear/theown_discobomb.nif"));
    assert!(
        report
            .parts
            .iter()
            .filter_map(|part| part.model.as_ref())
            .all(|model| model.status == NativeResolutionStatus::Missing)
    );
    let texture_candidates = report
        .parts
        .iter()
        .flat_map(|part| &part.textures)
        .flat_map(|texture| &texture.candidates)
        .map(|candidate| candidate.project_path.as_str())
        .collect::<BTreeSet<_>>();
    for expected in [
        "characters/player/shared/runtime-textures/m_skin.png",
        "characters/player/hnpc-runtime-textures/m_face_005_a.png",
        "characters/player/shared/runtime-textures/m_head_023_a.png",
        "characters/player/shared/runtime-textures/m_shirt_baseballset_c.png",
        "characters/player/shared/runtime-textures/m_pants_beltarmorset_b.png",
        "characters/player/shared/runtime-textures/shoes_blooarmorset_d.png",
        "textures/thrown_spideregg--a5f966511e7ae356.png",
    ] {
        assert!(
            texture_candidates.contains(expected),
            "missing native texture candidate {expected}"
        );
    }
    assert!(
        !report
            .to_pretty_json()
            .unwrap()
            .to_ascii_lowercase()
            .contains("dexter")
    );
    assert!(!report.complete);
}
