use super::*;
use crate::SourcePackIdentity;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn legacy_icon_types_have_explicit_non_inferred_categories() {
    assert_eq!(
        legacy_kind(0).unwrap().category,
        SemanticIconCategory::ItemWeapon
    );
    assert_eq!(
        legacy_kind(3).unwrap().category,
        SemanticIconCategory::ItemCosmetic
    );
    assert_eq!(
        legacy_kind(8).unwrap().category,
        SemanticIconCategory::EntityMob
    );
    assert_eq!(
        legacy_kind(9).unwrap().category,
        SemanticIconCategory::EntityFusion
    );
    assert_eq!(
        legacy_kind(4).unwrap().category,
        SemanticIconCategory::EntityNpc
    );
    assert_eq!(
        legacy_kind(10).unwrap().category,
        SemanticIconCategory::EntityHnpc
    );
    assert_eq!(
        legacy_kind(11).unwrap().category,
        SemanticIconCategory::ItemTransport
    );
    assert_eq!(
        legacy_kind(12).unwrap().category,
        SemanticIconCategory::ItemVehicle
    );
    assert!(legacy_kind(13).is_none());
}

#[test]
fn installer_publishes_only_exact_table_data_matches_and_reports_the_rest() {
    let fixture = Fixture::new();
    let report = install_semantic_icons(&SemanticIconInstallOptions::new(
        &fixture.asset_root,
        &fixture.table_set,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(report.published, 6);
    assert_eq!(report.unmatched, 3);
    assert!(
        fs::read_dir(fixture.asset_root.join("textures"))
            .unwrap()
            .filter_map(std::result::Result::ok)
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("wpnicon_01--"))
    );
    assert!(
        fixture
            .asset_root
            .join("icons/items/weapons/wpnicon_01.png")
            .exists()
    );
    assert!(
        fixture
            .asset_root
            .join("icons/entities/mobs/mobicon_02.png")
            .exists()
    );
    assert!(
        fixture
            .asset_root
            .join("icons/entities/npc/npcicon_04.png")
            .exists()
    );
    assert!(
        fixture
            .asset_root
            .join("icons/entities/hnpc/hnpcicon_10.png")
            .exists()
    );
    assert!(
        fixture
            .asset_root
            .join("icons/transport/transport_11.png")
            .exists()
    );
    assert!(
        fixture
            .asset_root
            .join("icons/items/vehicles/vehicle_12.png")
            .exists()
    );
    assert!(fixture.asset_root.join("icons/entities/fusion").is_dir());
    assert!(!fixture.asset_root.join("icons/catalog.json").exists());
    assert!(!fixture.asset_root.join("icons/unmatched.json").exists());
    assert_eq!(report.catalog.counts.published, 6);
    assert_eq!(report.catalog.counts.missing, 1);
    assert_eq!(report.catalog.counts.name_only_unclassified, 1);
    assert_eq!(report.catalog.counts.unreferenced_legacy_textures, 1);
    assert_eq!(report.unmatched_entries.len(), 3);
    assert!(report.catalog.assets.iter().all(|asset| {
        asset.classification.method == "table_data_icon_type_exact_manifest_stem"
    }));

    let manifest: ProjectAssetManifest = serde_json::from_slice(
        &fs::read(fixture.asset_root.join(ASSET_MANIFEST_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest
            .files
            .iter()
            .filter(|entry| entry.path.starts_with("icons/"))
            .count(),
        6
    );

    let first_manifest = fs::read(fixture.asset_root.join(ASSET_MANIFEST_FILE)).unwrap();
    let second = install_semantic_icons(&SemanticIconInstallOptions::new(
        &fixture.asset_root,
        &fixture.table_set,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(second.published, 6);
    assert_eq!(
        fs::read(fixture.asset_root.join(ASSET_MANIFEST_FILE)).unwrap(),
        first_manifest,
        "reinstall must not hash its own prior manifest entries into provenance"
    );
}

#[test]
fn exact_destination_taxonomy_is_stable() {
    assert_eq!(
        SemanticIconCategory::all()
            .into_iter()
            .map(SemanticIconCategory::directory)
            .collect::<Vec<_>>(),
        CATEGORY_DIRECTORIES
    );
}

struct Fixture {
    _temp: TempDir,
    asset_root: PathBuf,
    table_set: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let asset_root = temp.path().join("game");
        let textures = asset_root.join("textures");
        let tables = asset_root.join("data/tables");
        let source_build = temp.path().join("source-build");
        fs::create_dir_all(&textures).unwrap();
        fs::create_dir_all(&tables).unwrap();
        fs::create_dir(&source_build).unwrap();

        let mut files = Vec::new();
        write_texture(&textures, &mut files, "wpnicon_01", b"weapon");
        write_texture(&textures, &mut files, "mobicon_02", b"mob");
        write_texture(&textures, &mut files, "npcicon_04", b"npc");
        write_texture(&textures, &mut files, "hnpcicon_10", b"hnpc");
        write_texture(&textures, &mut files, "transport_11", b"transport");
        write_texture(&textures, &mut files, "vehicle_12", b"vehicle");
        write_texture(&textures, &mut files, "cosicon_99", b"unreferenced");
        write_texture(&textures, &mut files, "social_icon", b"name-only");
        let table_set = tables.join("table-set.json");
        let table = json!({
            "schema": TABLE_SET_SCHEMA,
            "tables": [{
                "key": "fixture",
                "name": "fixture",
                "value": {
                    "m_pWeaponItemTable": {
                        "m_pItemIconData": [
                            {"m_iIconType": 0, "m_iIconNumber": 1}
                        ]
                    },
                    "m_pNpcTable": {
                        "m_pNpcIconData": [
                            {"m_iIconType": 8, "m_iIconNumber": 2},
                            {"m_iIconType": 9, "m_iIconNumber": 3},
                            {"m_iIconType": 4, "m_iIconNumber": 4},
                            {"m_iIconType": 10, "m_iIconNumber": 10}
                        ],
                        "m_pVehicleIconData": [
                            {"m_iIconType": 11, "m_iIconNumber": 11},
                            {"m_iIconType": 12, "m_iIconNumber": 12}
                        ]
                    }
                }
            }]
        });
        let mut table_bytes = serde_json::to_vec_pretty(&table).unwrap();
        table_bytes.push(b'\n');
        fs::write(&table_set, &table_bytes).unwrap();
        files.push(ProjectAssetFile {
            source_path: "tables/table-set.json".to_owned(),
            path: "data/tables/table-set.json".to_owned(),
            kind: ProjectAssetKind::Data,
            bytes: table_bytes.len() as u64,
            blake3: blake3::hash(&table_bytes).to_hex().to_string(),
        });
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let manifest = ProjectAssetManifest {
            schema: PROJECT_ASSET_SCHEMA.to_owned(),
            protocol: 104,
            locale: "en-US".to_owned(),
            source_pack: SourcePackIdentity {
                schema: "fixture".to_owned(),
                manifest_blake3: "fixture".to_owned(),
            },
            files,
        };
        let mut manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        manifest_bytes.push(b'\n');
        fs::write(asset_root.join(ASSET_MANIFEST_FILE), manifest_bytes).unwrap();
        Self {
            _temp: temp,
            asset_root,
            table_set,
        }
    }
}

fn write_texture(
    textures: &Path,
    files: &mut Vec<ProjectAssetFile>,
    true_name: &str,
    bytes: &[u8],
) {
    let hash = blake3::hash(bytes).to_hex().to_string();
    let file_name = format!("{true_name}--{}.png", &hash[..16]);
    fs::write(textures.join(&file_name), bytes).unwrap();
    files.push(ProjectAssetFile {
        source_path: format!("textures/{file_name}"),
        path: format!("textures/{file_name}"),
        kind: ProjectAssetKind::Texture,
        bytes: bytes.len() as u64,
        blake3: hash,
    });
}
