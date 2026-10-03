use super::*;

pub(super) fn manifest_entry(route: &VerifiedIconRoute, bytes: &[u8]) -> ProjectAssetManifest {
    ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: 104,
        locale: "ru-RU".to_owned(),
        source_pack: SourcePackIdentity {
            schema: "test".to_owned(),
            manifest_blake3: "test".to_owned(),
        },
        files: vec![ProjectAssetFile {
            source_path: "test".to_owned(),
            path: route.source.to_owned(),
            kind: ProjectAssetKind::Texture,
            bytes: bytes.len() as u64,
            blake3: route.expected_blake3.to_owned(),
        }],
    }
}

#[test]
fn table_data_icon_source_fails_closed_on_byte_or_manifest_drift() {
    let (route, bytes) = first_verified_bytes();
    let manifest = manifest_entry(route, &bytes);

    let mut tampered = bytes.clone();
    tampered[0] ^= 0xff;
    assert!(verify_table_data_icon_source(&manifest, route, &tampered).is_err());

    let mut wrong_hash = manifest.clone();
    wrong_hash.files[0].blake3 = "0".repeat(64);
    assert!(verify_table_data_icon_source(&wrong_hash, route, &bytes).is_err());

    let mut missing = manifest;
    missing.files.clear();
    assert!(verify_table_data_icon_source(&missing, route, &bytes).is_err());
}

#[test]
fn strict_source_backed_ui_routes_match_manifest_and_disk() {
    assert_eq!(STRICT_ROUTE_PROOFS.len(), 49);
    let asset_root = workspace_root().join("assets").join("game");
    let manifest_bytes = fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap();
    let manifest: ProjectAssetManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    for proof in STRICT_ROUTE_PROOFS {
        let path = asset_root.join(proof.source.replace('/', std::path::MAIN_SEPARATOR_STR));
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len() as u64, proof.bytes, "{}", proof.source);
        assert_eq!(
            blake3::hash(&bytes).to_hex().as_str(),
            proof.blake3,
            "{}",
            proof.source
        );
        verify_route_source(&manifest, proof.source, ProjectAssetKind::Texture, &bytes)
            .unwrap();
    }
    let exact_routes = ROUTES
        .iter()
        .filter(|route| {
            route.destination.starts_with("mission/")
                || route.destination.starts_with("group/")
                || route.destination.starts_with("buddy/")
                || route.destination.starts_with("overheat/")
                || route.destination.starts_with("quick-slot/")
                || route.destination == "chat/alert.png"
                || route.destination == "nanocom/message/npc.png"
                || matches!(
                    route.destination,
                    "minimap/map_icon_02.png"
                        | "minimap/map_icon_03.png"
                        | "minimap/plus_but_1.png"
                        | "minimap/minus_but_1.png"
                        | "system/systemDialogBox.png"
                )
        })
        .collect::<Vec<_>>();
    let exact_sources = exact_routes
        .iter()
        .map(|route| route.source)
        .collect::<BTreeSet<_>>();
    let proof_sources = STRICT_ROUTE_PROOFS
        .iter()
        .map(|proof| proof.source)
        .collect::<BTreeSet<_>>();
    assert_eq!(exact_routes.len(), 50);
    assert_eq!(exact_sources.len(), STRICT_ROUTE_PROOFS.len());
    assert_eq!(proof_sources.len(), STRICT_ROUTE_PROOFS.len());
    assert_eq!(exact_sources, proof_sources);
    assert!(
        exact_routes
            .iter()
            .all(|route| strict_route_proof(route.source).is_some())
    );
}

#[test]
fn chat_alert_route_matches_clean_cngui_chat_serialized_authority() {
    let route = ROUTES
        .iter()
        .find(|route| route.destination == "chat/alert.png")
        .expect("missing exact CnGuiChat.alertIcon route");
    assert_eq!(route.legacy_name, "chatAlert");
    assert_eq!(route.source, "ui/gameplay/chat/alert.png");
    assert_eq!(route.kind, ProjectAssetKind::Texture);

    let proof = strict_route_proof(route.source).expect("missing chatAlert byte proof");
    assert_eq!(
        proof.blake3,
        "df2f08815d97ce578c4624d011fd4bd0e110f63401bc9e58d50eaf9a97201822"
    );
    assert_eq!(proof.bytes, 553);
}
